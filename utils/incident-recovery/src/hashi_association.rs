//! Conditional native HASHI Deposit association against authenticated checkpoint
//! state. No receipt finality, historical configuration approval or delivery is
//! established by this adapter; it only runs existing read-only native parsing.
use crate::{hex_bytes, incoming_coverage::WorkFile, Result, VerifiedCheckpoint};
use codec::{Decode, DecodeLimit, Encode};
use common::{
    recovery::{self, RecoveryPolicy},
    AssetInfoProvider,
};
use eth_bridge::requests::{
    IncomingRequest, IncomingTransactionRequestKind as Kind, LoadIncomingRequest,
    LoadIncomingTransactionRequest, OffchainRequest, RequestStatus,
};
use framenode_runtime::{
    incident_recovery_incoming::{authenticated_hashi_source, IncomingChannel, IncomingOperation},
    AccountId, Assets, BridgeMultisig, EthBridge, Runtime,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use sp_core::{storage::StateVersion, H160, H256, U256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path},
};

const MAX_INPUT: usize = 64 * 1024 * 1024;
const MAX_SOURCE: usize = 96 * 1024 * 1024;
const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema: String,
    checkpoint_hash: String,
    checkpoint_state_root: String,
    capture_manifest_sha256: String,
    metadata_hash: String,
    policy: WorkFile,
    rows: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Row {
    source_report: WorkFile,
    network: u32,
    transaction_hash: String,
    load_scale: String,
    incoming_operation_scale: String,
    load_provenance: LoadProvenance,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum LoadProvenance {
    CheckpointPendingLoad,
    AutomaticEventImport,
}

// These structs admit the frozen kernel's extra explanatory output fields but
// reject duplicate fields through serde's typed field visitor. No reported
// proof/approval boolean is used as an authority or cryptographic certificate.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceReport {
    schema: String,
    range: SourceRange,
    extraction: Extraction,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceRange {
    schema: String,
    declared_chain_id: String,
    logs: Vec<SourceLog>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Extraction {
    scope: SourceScope,
    deposits: Vec<DepositLocator>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceScope {
    native_network: String,
    declared_chain_id: String,
    declared_genesis_hash: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DepositLocator {
    native_message_id: String,
    transaction_hash: String,
    block_hash: String,
    block_number: String,
    transaction_index: u32,
    log_index: u32,
    receipt_log_index: u32,
    source_contract: String,
    raw_topic: String,
    raw_data: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceLog {
    block_number: String,
    block_hash: String,
    transaction_index: u32,
    transaction_hash: String,
    receipt_log_index: u32,
    log_index: u32,
    address: String,
    topics: Vec<String>,
    data: String,
    status: u8,
}

fn hx(data: &[u8]) -> String {
    format!("0x{}", hex::encode(data))
}
fn sha(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}
fn hash(data: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(data)
}
fn fixed<const N: usize>(data: &str) -> Result<[u8; N]> {
    if data.len() != 2 + N * 2 {
        return Err("HASHI association fixed-width hex".into());
    }
    hex_bytes(data)?
        .try_into()
        .map_err(|_| "HASHI association fixed-width hex".into())
}
fn number(data: &str) -> Result<u64> {
    let value = data.parse::<u64>().map_err(|_| "HASHI source height")?;
    if value.to_string() != data {
        return Err("noncanonical HASHI source height".into());
    }
    Ok(value)
}
fn decode<T: Decode + Encode>(data: &[u8]) -> Result<T> {
    if data.len() > recovery::MAX_INHERENT_BYTES {
        return Err("HASHI native value bound".into());
    }
    let mut input = data;
    let value = T::decode_with_depth_limit(
        16,
        &mut codec::MemTrackingInput::new(&mut input, recovery::MAX_INHERENT_BYTES * 8),
    )
    .map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != data {
        return Err("noncanonical HASHI native bytes".into());
    }
    Ok(value)
}
fn read_ref(base: &Path, reference: &WorkFile, maximum: usize) -> Result<Vec<u8>> {
    if reference.bytes == 0
        || reference.bytes > maximum
        || reference.file.is_empty()
        || reference.file.contains('\\')
        || reference
            .file
            .split('/')
            .any(|v| v.is_empty() || v == "." || v == "..")
    {
        return Err("HASHI artifact reference bound/path".into());
    }
    let mut path = base.to_path_buf();
    for component in Path::new(&reference.file).components() {
        let Component::Normal(part) = component else {
            return Err("HASHI artifact escapes manifest".into());
        };
        path.push(part);
        if fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("HASHI artifact symlink".into());
        }
    }
    let file = fs::File::open(&path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("HASHI artifact is not a file".into());
    }
    let mut data = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() != reference.bytes || sha(&data) != reference.sha256 {
        return Err("HASHI artifact bytes/hash differ".into());
    }
    Ok(data)
}
fn native_log(row: &SourceLog) -> Result<eth_bridge::types::Log> {
    if row.topics.len() > 4 || row.data.len() > 128 * 1024 + 2 || row.status != 1 {
        return Err("HASHI source log bound/success status".into());
    }
    Ok(eth_bridge::types::Log {
        address: eth_bridge::types::H160(fixed(&row.address)?),
        topics: row
            .topics
            .iter()
            .map(|v| fixed(v).map(eth_bridge::types::H256))
            .collect::<Result<_>>()?,
        data: eth_bridge::types::Bytes(hex_bytes(&row.data)?),
        block_hash: Some(eth_bridge::types::H256(fixed(&row.block_hash)?)),
        block_number: Some(number(&row.block_number)?.into()),
        transaction_hash: Some(eth_bridge::types::H256(fixed(&row.transaction_hash)?)),
        transaction_index: Some(row.transaction_index.into()),
        log_index: Some(row.log_index.into()),
        transaction_log_index: Some(row.receipt_log_index.into()),
        log_type: None,
        removed: Some(false),
    })
}
fn original_transfer(
    request: &IncomingRequest<Runtime>,
) -> Result<&eth_bridge::requests::IncomingTransfer<Runtime>> {
    match request {
        IncomingRequest::Transfer(t) if t.from==H160::zero() && t.amount>0 && !t.should_take_fee=>Ok(t),
        IncomingRequest::Transfer(t) if t.should_take_fee=>Err("quarantine required: fee-taking original Transfer has no supported Deposit association".into()),
        _=>Err("quarantine required: unsupported native Deposit request".into()),
    }
}
fn transfer_load(
    load: &LoadIncomingRequest<Runtime>,
) -> Result<LoadIncomingTransactionRequest<Runtime>> {
    let expected = LoadIncomingTransactionRequest::new(
        load.author().clone(),
        load.hash(),
        load.timepoint(),
        Kind::Transfer,
        load.network_id(),
    );
    if load == &LoadIncomingRequest::Transaction(expected.clone()) {
        return Ok(expected);
    }
    let legacy = LoadIncomingTransactionRequest::new(
        load.author().clone(),
        load.hash(),
        load.timepoint(),
        Kind::TransferXOR,
        load.network_id(),
    );
    if load == &LoadIncomingRequest::Transaction(legacy) {
        return Err("quarantine required: retained TransferXOR is unsupported; fee and decommission semantics cannot be coerced".into());
    }
    Err("quarantine required: non-Transfer load has no Deposit association".into())
}

fn source_evidence(
    network: u32,
    scope: &SourceScope,
    deposit: &DepositLocator,
    raw_transaction: &[u8],
) -> Result<[u8; 32]> {
    if scope.declared_chain_id.len() > 78 {
        return Err("HASHI declared source chain bound".into());
    }
    let chain =
        U256::from_dec_str(&scope.declared_chain_id).map_err(|_| "HASHI declared source chain")?;
    if chain.is_zero()
        || chain.to_string() != scope.declared_chain_id
        || scope.native_network != network.to_string()
    {
        return Err("HASHI canonical source identity differs".into());
    }
    let mut domain = b"sora-nexus/evm-required-source/v1\0".to_vec();
    domain.extend(network.to_le_bytes());
    domain.extend(chain.to_little_endian());
    domain.extend(fixed::<32>(&scope.declared_genesis_hash)?);
    let source_id: [u8; 32] = Sha256::digest(domain).into();
    let events = vec![(
        deposit.log_index,
        deposit.receipt_log_index,
        fixed::<20>(&deposit.source_contract)?,
        fixed::<32>(&deposit.raw_topic)?,
        hex_bytes(&deposit.raw_data)?,
    )];
    Ok(hash(
        &(
            b"sora-nexus/verified-hashi-source-record/v1".to_vec(),
            source_id,
            fixed::<32>(&deposit.block_hash)?,
            number(&deposit.block_number)?,
            fixed::<32>(&deposit.transaction_hash)?,
            deposit.transaction_index,
            raw_transaction.to_vec(),
            events,
        )
            .encode(),
    ))
}

/// Must be called in the isolated exact checkpoint externalities. Only native
/// read methods are used; no validation bypass, storage overlay, preparation,
/// registration, fee charge, reserve or finalization occurs.
fn associate(row: &Row, source: &SourceReport, policy: &RecoveryPolicy) -> Result<Value> {
    if source.schema != "sora-nexus-offline-evm-hashi-candidate-verification-v1"
        || source.range.schema != "sora-nexus-evm-receipt-range-consistency-v1"
        || source.extraction.scope.native_network != row.network.to_string()
        || source.range.declared_chain_id != source.extraction.scope.declared_chain_id
        || source.range.logs.len() > 65_536
        || source.extraction.deposits.len() > 65_536
    {
        return Err("HASHI source report schema/network/bound".into());
    }
    let tx_hash = H256(fixed(&row.transaction_hash)?);
    let op_bytes = hex_bytes(&row.incoming_operation_scale)?;
    let operation: IncomingOperation = decode(&op_bytes)?;
    if policy.chunk_hashes.get(operation.index as usize) != Some(&operation.commitment()) {
        return Err("HASHI operation is not committed by the exact policy cursor".into());
    }
    let IncomingChannel::HashiAuthenticatedSource {
        network,
        request,
        raw_transaction,
        expected_chain_id,
        ..
    } = &operation.channel
    else {
        return Err(
            "HASHI association requires original zero-sender authenticated-source variant".into(),
        );
    };
    if *network != row.network {
        return Err("HASHI work/native network differs".into());
    }
    let original: IncomingRequest<Runtime> = decode(request)?;
    let transfer = original_transfer(&original)?;
    let load_bytes = hex_bytes(&row.load_scale)?;
    let load: LoadIncomingRequest<Runtime> = decode(&load_bytes)?;
    let native_load = transfer_load(&load)?;
    if load.hash() != tx_hash
        || load.network_id() != row.network
        || transfer.tx_hash != tx_hash
        || transfer.network_id != row.network
        || load.author() != &transfer.author
        || load.timepoint() != transfer.timepoint
    {
        return Err("HASHI original request/load identity, author or timepoint differs".into());
    }
    let actor = authenticated_hashi_source(request, raw_transaction, *expected_chain_id)
        .map_err(|e| format!("HASHI original source transaction: {e:?}"))?;
    if actor
        .chain_id
        .is_some_and(|id| id.to_string() != source.range.declared_chain_id)
    {
        return Err("HASHI signature domain differs from declared source chain".into());
    }
    let deposits: Vec<_> = source
        .extraction
        .deposits
        .iter()
        .filter(|d| d.transaction_hash == row.transaction_hash)
        .collect();
    if deposits.len() != 1 {
        return Err(
            "quarantine required: one native tx identity must have exactly one matching Deposit"
                .into(),
        );
    }
    let deposit = deposits[0];
    if deposit.native_message_id != row.transaction_hash {
        return Err("HASHI Deposit native identity differs".into());
    }
    if operation.finalized_source_evidence
        != source_evidence(
            row.network,
            &source.extraction.scope,
            deposit,
            raw_transaction,
        )?
    {
        return Err(
            "HASHI committed evidence does not bind exact source event and transaction".into(),
        );
    }
    let log_rows: Vec<_> = source
        .range
        .logs
        .iter()
        .filter(|log| log.transaction_hash == row.transaction_hash)
        .collect();
    if log_rows.is_empty() {
        return Err("HASHI transaction has no actual receipt logs".into());
    }
    let mut seen = BTreeSet::new();
    for (index, log) in log_rows.iter().enumerate() {
        if log.block_number != deposit.block_number
            || log.block_hash != deposit.block_hash
            || log.transaction_index != deposit.transaction_index
            || log.receipt_log_index as usize != index
            || !seen.insert(log.log_index)
            || (index > 0 && Some(log.log_index) != log_rows[index - 1].log_index.checked_add(1))
        {
            return Err("HASHI receipt log locator/order differs".into());
        }
    }
    let selected = log_rows
        .iter()
        .find(|l| l.log_index == deposit.log_index)
        .ok_or("HASHI selected log missing")?;
    if selected.receipt_log_index != deposit.receipt_log_index
        || selected.address != deposit.source_contract
        || selected.topics != vec![deposit.raw_topic.clone()]
        || selected.data != deposit.raw_data
    {
        return Err("HASHI extracted event differs from exact receipt log".into());
    }
    let logs = log_rows
        .iter()
        .map(|row| native_log(row))
        .collect::<Result<Vec<_>>>()?;
    let matching: Vec<_> = logs
        .iter()
        .filter(|l| {
            l.topics
                .first()
                .is_some_and(|t| t.0 == eth_bridge::DEPOSIT_TOPIC.0)
                && EthBridge::ensure_known_contract(H160(l.address.0), row.network).is_ok()
        })
        .collect();
    if matching.len() != 1 || matching[0].log_index != Some(deposit.log_index.into()) {
        return Err("quarantine required: source contract epoch is not uniquely compatible with checkpoint native configuration".into());
    }
    let data = &matching[0].data.0;
    if matching[0].topics.len() != 1 || data.len() != 128 || data[64..76].iter().any(|v| *v != 0) {
        return Err("HASHI Deposit ABI must be exact, canonical and unambiguous".into());
    }
    let height = number(&deposit.block_number)?;
    let provenance = match row.load_provenance {
        LoadProvenance::CheckpointPendingLoad => {
            let stored = eth_bridge::Requests::<Runtime>::get(row.network, tx_hash)
                .ok_or("checkpoint LoadIncoming absent")?;
            if stored.encode() != OffchainRequest::<Runtime>::LoadIncoming(load.clone()).encode()
                || eth_bridge::RequestStatuses::<Runtime>::get(row.network, tx_hash)
                    != Some(RequestStatus::Pending)
                || eth_bridge::LoadToIncomingRequestHash::<Runtime>::contains_key(
                    row.network,
                    tx_hash,
                )
                || !eth_bridge::RequestSubmissionHeight::<Runtime>::contains_key(
                    row.network,
                    tx_hash,
                )
                || eth_bridge::RequestSubmissionHeight::<Runtime>::get(row.network, tx_hash)
                    > recovery::CHECKPOINT_NUMBER
                || eth_bridge::RequestsQueue::<Runtime>::get(row.network)
                    .iter()
                    .filter(|id| **id == tx_hash)
                    .count()
                    != 1
            {
                return Err("quarantine required: exact checkpoint Pending load/queue/replay provenance absent; failed/done loads are not readmitted".into());
            }
            json!({"kind":"checkpointPendingLoad","exactRetainedLoadVerified":true,"authorInferredFromBeneficiary":false,
                "checkpointSubmissionHeight":eth_bridge::RequestSubmissionHeight::<Runtime>::get(row.network,tx_hash)})
        }
        LoadProvenance::AutomaticEventImport => {
            let expected = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
                transfer.to.clone(),
                tx_hash,
                BridgeMultisig::sidechain_timepoint(height, deposit.transaction_index),
                Kind::Transfer,
                row.network,
            ));
            if load != expected
                || eth_bridge::Requests::<Runtime>::contains_key(row.network, tx_hash)
                || eth_bridge::RequestStatuses::<Runtime>::contains_key(row.network, tx_hash)
                || eth_bridge::RequestSubmissionHeight::<Runtime>::contains_key(
                    row.network,
                    tx_hash,
                )
                || eth_bridge::RequestsQueue::<Runtime>::get(row.network).contains(&tx_hash)
                || EthBridge::account_requests(load.author()).contains(&(row.network, tx_hash))
                || eth_bridge::LoadToIncomingRequestHash::<Runtime>::contains_key(
                    row.network,
                    tx_hash,
                )
            {
                return Err(
                    "HASHI automatic-load recipe differs or native identity already exists".into(),
                );
            }
            json!({"kind":"nativeAutomaticEventImportRecipe","exactRetainedLoadVerified":false,"authorDerivedByNativeRecipe":true,"accountOwnershipProved":false})
        }
    };
    // This is the same native path used by parse_incoming_request and the
    // automatic offchain import. It performs U256 denomination division BEFORE
    // u128 conversion and then native precision adjustment/flooring.
    let event = EthBridge::parse_main_event(row.network, &logs, Kind::Transfer)
        .map_err(|e| format!("native Deposit parse: {e:?}"))?;
    let expected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        IncomingRequest::<Runtime>::try_from_contract_event(event, native_load, height)
    }))
    .map_err(|_| "native Deposit conversion panicked; quarantine required")?
    .map_err(|e| format!("native Deposit conversion: {e:?}"))?;
    if expected.encode() != *request {
        return Err(
            "native Deposit conversion differs from complete original IncomingRequest bytes".into(),
        );
    }
    let expected = original_transfer(&expected)?;
    let incoming_hash = OffchainRequest::incoming(original.clone()).hash();
    if eth_bridge::Requests::<Runtime>::contains_key(row.network, incoming_hash)
        || eth_bridge::RequestStatuses::<Runtime>::contains_key(row.network, incoming_hash)
    {
        return Err("quarantine required: already-prepared/completed native incoming identity needs its existing-state adapter".into());
    }
    let effective = |owner: &AccountId| {
        policy
            .replacements
            .iter()
            .find(|p| &p.old == owner)
            .map(|p| p.new.clone())
            .unwrap_or_else(|| owner.clone())
    };
    Ok(
        json!({"network":row.network,"transactionHash":row.transaction_hash,"sourceReportSha256":row.source_report.sha256,
        "operationIndex":operation.index,"operationCommitment":hx(&operation.commitment()),"originalRequestScale":hx(request),"originalRequestHash":hx(&hash(request)),
        "nativeIncomingWrapperHash":hx(incoming_hash.as_bytes()),"loadScale":row.load_scale,"loadProvenance":provenance,
        "originalBeneficiary":hx(transfer.to.as_ref()),"originalAuthor":hx(transfer.author.as_ref()),
        "projectedReplacementBeneficiary":hx(effective(&transfer.to).as_ref()),"projectedReplacementQueryAuthor":hx(effective(&transfer.author).as_ref()),
        "sourceContract":deposit.source_contract,"rawEventData":deposit.raw_data,"sourceBlockHash":deposit.block_hash,"sourceHeight":deposit.block_number,
        "sourceLogIndex":deposit.log_index,"sourceTransactionIndex":deposit.transaction_index,
        "assetId":hx(&expected.asset_id.encode()),"assetKind":format!("{:?}",expected.asset_kind),"nativeAmount":expected.amount.to_string(),
        "checkpointDenominationFactor":EthBridge::bridge_denomination_factor(row.network,&expected.asset_id).to_string(),
        "checkpointSidechainPrecision":EthBridge::sidechain_asset_precision(row.network,expected.asset_id),"checkpointLocalPrecision":Assets::get_asset_info(&expected.asset_id).2,
        "nativeShouldTakeFee":expected.should_take_fee,"nativeFeeDebited":"0","nativeFromPreserved":hx(expected.from.as_bytes()),
        "authenticatedOuterTransactionActor":hx(actor.signer.as_bytes()),"outerActorIsBeneficialOwner":false,
        "completeOriginalRequestMatchesNativeCheckpointParsing":true,"postMigrationQueryIndexVerified":false,"nativeDeliveryAdmissionVerified":false}),
    )
}

pub fn verify_file(capture: &Path, manifest_path: &Path) -> Result<Value> {
    if !manifest_path.is_absolute()
        || manifest_path.canonicalize().map_err(|e| e.to_string())? != manifest_path
        || fs::metadata(manifest_path)
            .map_err(|e| e.to_string())?
            .len()
            > MAX_INPUT as u64
    {
        return Err("HASHI association manifest canonical path/bound".into());
    }
    let bytes = fs::read(manifest_path).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_INPUT {
        return Err("HASHI association manifest bound".into());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let verified = crate::verify_checkpoint(capture)?;
    let result = verify(
        &verified,
        manifest_path
            .parent()
            .ok_or("HASHI manifest parent absent")?,
        &manifest,
        sha(&bytes),
    )?;
    if fs::read(manifest_path).map_err(|e| e.to_string())? != bytes {
        return Err("HASHI association manifest changed during verification".into());
    }
    Ok(result)
}

fn verify(
    verified: &VerifiedCheckpoint,
    base: &Path,
    manifest: &Manifest,
    manifest_sha: String,
) -> Result<Value> {
    if manifest.schema != "sora-nexus-hashi-native-association-input-v1"
        || manifest.rows.is_empty()
        || manifest.rows.len() > 128
        || manifest.checkpoint_hash != hx(&recovery::CHECKPOINT_HASH)
        || manifest.checkpoint_state_root != hx(&recovery::CHECKPOINT_STATE_ROOT)
        || manifest.checkpoint_hash != verified.state.checkpoint_hash
        || manifest.checkpoint_state_root != verified.state.state_root
        || manifest.capture_manifest_sha256 != verified.state.capture_manifest_sha256
        || manifest.metadata_hash != METADATA
        || manifest.metadata_hash != verified.schema.metadata_hash
        || !verified.storage.children_default.is_empty()
    {
        return Err("HASHI association exact checkpoint/metadata identity or bound differs".into());
    }
    let policy_bytes = read_ref(base, &manifest.policy, 2 * 1024 * 1024)?;
    let policy: RecoveryPolicy = decode(&policy_bytes)?;
    policy
        .validate()
        .map_err(|e| format!("HASHI policy: {e:?}"))?;
    let mut reports = BTreeMap::new();
    let mut total = 0usize;
    let mut inputs = vec![manifest.policy.clone()];
    for row in &manifest.rows {
        if row.incoming_operation_scale.len() > recovery::MAX_INHERENT_BYTES * 2 + 2
            || row.load_scale.len() > 4096
        {
            return Err("HASHI row bound".into());
        }
        if let Some((old_sha, old_bytes, _)) = reports.get(&row.source_report.file) {
            if old_sha != &row.source_report.sha256 || old_bytes != &row.source_report.bytes {
                return Err("HASHI source report alias changed".into());
            }
        } else {
            let data = read_ref(base, &row.source_report, MAX_SOURCE)?;
            total += data.len();
            if total > 256 * 1024 * 1024 {
                return Err("HASHI aggregate source report bound".into());
            }
            let report: SourceReport = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
            reports.insert(
                row.source_report.file.clone(),
                (
                    row.source_report.sha256.clone(),
                    row.source_report.bytes,
                    report,
                ),
            );
            inputs.push(row.source_report.clone());
        }
    }
    let code = verified
        .storage
        .top
        .get(b":code".as_slice())
        .ok_or("authenticated checkpoint runtime code missing")?;
    let mut ext = sp_io::TestExternalities::new_with_code_and_state(
        code,
        verified.storage.clone(),
        StateVersion::V0,
    );
    let mut identities = BTreeSet::new();
    let (associations, state_root) = ext.execute_with(|| -> Result<_> {
        let before = sp_io::storage::root(StateVersion::V0);
        let mut associations = Vec::new();
        for row in &manifest.rows {
            if !identities.insert((row.network, row.transaction_hash.clone())) {
                return Err("duplicate HASHI association identity".into());
            }
            associations.push(associate(
                row,
                &reports[&row.source_report.file].2,
                &policy,
            )?);
        }
        if sp_io::storage::root(StateVersion::V0) != before
            || before.as_slice() != recovery::CHECKPOINT_STATE_ROOT
        {
            return Err(
                "HASHI native association mutated or misidentified checkpoint state".into(),
            );
        }
        Ok((associations, hx(&before)))
    })?;
    for reference in &inputs {
        read_ref(base, reference, MAX_SOURCE)?;
    }
    Ok(
        json!({"schema":"sora-nexus-hashi-checkpoint-native-association-v1","manifestSha256":manifest_sha,"policySha256":sha(&policy_bytes),
        "policyCommitment":hx(&policy.commitment()),"captureManifestSha256":verified.state.capture_manifest_sha256,"checkpointStateRoot":state_root,
        "metadataWitness":verified.metadata_witness,"inputs":inputs.iter().map(|r|json!({"file":r.file,"sha256":r.sha256,"bytes":r.bytes})).collect::<Vec<_>>(),
        "associations":associations,"checkpointNativeAssociationVerified":true,"nativeStateUnchanged":true,
        "sourceProofReverifiedByThisAdapter":false,"sourceConfigurationEpochsAuthenticated":false,"nativeParserSameAsDeployedCheckpointCode":false,
        "nativeParserRuntimeSpecVersion":framenode_runtime::VERSION.spec_version,"nativeParserTransactionVersion":framenode_runtime::VERSION.transaction_version,
        "legacyParserExecutionEquivalenceVerified":false,"legacyParserParityRequiredBeforeRelease":true,
        "externalFinalityVerified":false,"nativeQuorumAuthenticated":false,"allNativeObligationsCovered":false,"actorExclusionDecisionVerified":false,
        "economicExecutionPerformed":false,"bridgeHaltChanged":false,"releaseGatePromoted":false,"productionReady":false}),
    )
}

#[cfg(test)]
#[path = "hashi_association_tests.rs"]
mod tests;
