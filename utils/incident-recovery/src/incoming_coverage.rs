//! Exact local incoming-work coverage. This proves neither external archive
//! completeness/finality nor admission: those still require exact runtime execution.

use crate::{
    hex_bytes,
    inventory::{Descriptor, Hasher, MetadataSchema},
    typed, Result,
};
use bridge_types::{
    substrate::{BridgeCall, JettonAppCall, ParachainAppCall, SubstrateAppCall},
    types::MessageId,
    GenericAccount, GenericCommitment, GenericNetworkId, GenericTimepoint, SubNetworkId, H256,
};
use codec::{Decode, DecodeLimit, Encode};
use common::recovery::{
    self, RecoveryAction, RecoveryInherent, RecoveryPolicy, StorageScope, StorageWitness,
};
use eth_bridge::requests::{
    IncomingRequest, IncomingTransactionRequestKind, LoadIncomingRequest,
    LoadIncomingTransactionRequest, OffchainRequest, RequestStatus,
};
use framenode_runtime::{
    incident_recovery_incoming::{
        authenticated_hashi_source, classify_substrate_control, ControlDisposition, IncomingAudit,
        IncomingChannel, IncomingOperation,
    },
    incident_recovery_outgoing::OutgoingOperation,
    Runtime,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sp_core::{crypto::AccountId32, storage::Storage};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path},
};

const MAX_ROWS: usize = 1_000_000;
const MAX_ITEMS: usize = recovery::MAX_WORK_CHUNKS;
type Identity = (Vec<u8>, [u8; 32]);

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkIndex {
    pub schema: String,
    pub policy_sha256: String,
    pub work: Vec<WorkFile>,
    pub audits: Vec<WorkFile>,
    pub terminal: WorkFile,
    #[serde(default)]
    pub terminal_prelude: Vec<WorkFile>,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkFile {
    pub file: String,
    pub bytes: usize,
    pub sha256: String,
}

/// Private certificate: only a complete policy-matching work/audit stream can
/// construct it. No caller-supplied incoming-only list can establish coverage.
pub struct CheckedWork {
    policy: Vec<u8>,
    incoming: Vec<IncomingOperation>,
    prior_incoming: Vec<IncomingOperation>,
    incoming_audits: BTreeMap<(u32, [u8; 32]), IncomingAudit>,
    replay: Option<crate::incoming_replay::CoverageBinding>,
    touched: BTreeSet<Vec<u8>>,
    witnesses: BTreeMap<Vec<u8>, Option<[u8; 32]>>,
}

pub fn load_work(root: &Path, index: WorkIndex, policy: &RecoveryPolicy) -> Result<CheckedWork> {
    load_work_inner(root, index, policy, None)
}

/// Admit prior-policy work only from a completed exact-artifact replay in this
/// process. A serialized replay report cannot construct this certificate.
pub fn load_work_with_replay(
    root: &Path,
    index: WorkIndex,
    policy: &RecoveryPolicy,
    replay: &crate::incoming_replay::VerifiedReplay,
) -> Result<CheckedWork> {
    load_work_inner(root, index, policy, Some(replay))
}

fn load_work_inner(
    root: &Path,
    index: WorkIndex,
    policy: &RecoveryPolicy,
    replay: Option<&crate::incoming_replay::VerifiedReplay>,
) -> Result<CheckedWork> {
    if index.schema != "sora-nexus-recovery-work-index-v1"
        || index.policy_sha256 != hex::encode(Sha256::digest(policy.encode()))
        || index.work.len() != policy.chunk_hashes.len()
        || index.audits.len() != policy.audit_chunk_hashes.len()
        || index.work.len() > MAX_ITEMS
        || index.audits.len() > MAX_ITEMS
        || index.terminal_prelude.len() >= recovery::invariants::MAX_TERMINAL_PAGES
    {
        return Err("incoming complete work-index/policy mismatch".into());
    }
    let mut total = 0usize;
    let mut paths = BTreeSet::new();
    let mut read = |reference: WorkFile| -> Result<RecoveryInherent> {
        if reference.bytes == 0
            || reference.bytes > recovery::MAX_INHERENT_BYTES
            || reference.file.is_empty()
            || reference.file.contains('\\')
            || reference
                .file
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !paths.insert(reference.file.clone())
        {
            return Err("invalid/duplicate incoming work shard reference".into());
        }
        let mut path = root.to_path_buf();
        for part in Path::new(&reference.file).components() {
            let Component::Normal(part) = part else {
                return Err("incoming work shard escapes root".into());
            };
            path.push(part);
            if fs::symlink_metadata(&path)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("incoming work shard symlink".into());
            }
        }
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() != reference.bytes as u64 {
            return Err("incoming work shard size/type mismatch".into());
        }
        total = total
            .checked_add(reference.bytes)
            .filter(|n| *n <= 512 * 1024 * 1024)
            .ok_or("incoming work aggregate bound")?;
        let mut bytes = Vec::new();
        file.take(recovery::MAX_INHERENT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() != reference.bytes || hex::encode(Sha256::digest(&bytes)) != reference.sha256
        {
            return Err("incoming work shard hash/size mismatch".into());
        }
        RecoveryInherent::decode_remark(&bytes)
            .map_err(|e| format!("incoming work envelope: {e:?}"))
    };
    let work_count = index.work.len();
    let audit_count = index.audits.len();
    let stream = index
        .work
        .into_iter()
        .enumerate()
        .map(|(i, file)| (0, i, file))
        .chain(
            index
                .audits
                .into_iter()
                .enumerate()
                .map(|(i, file)| (1, i, file)),
        )
        .chain(
            index
                .terminal_prelude
                .into_iter()
                .chain(std::iter::once(index.terminal))
                .enumerate()
                .map(|(i, file)| (2, i, file)),
        )
        .map(|(kind, index, file)| read(file).map(|envelope| (kind, index, envelope)));
    check_stream_with_replay(policy, work_count, audit_count, stream, replay)
}

#[cfg(test)]
fn check_stream(
    policy: &RecoveryPolicy,
    work_count: usize,
    audit_count: usize,
    stream: impl IntoIterator<Item = Result<(u8, usize, RecoveryInherent)>>,
) -> Result<CheckedWork> {
    check_stream_with_replay(policy, work_count, audit_count, stream, None)
}

fn check_stream_with_replay(
    policy: &RecoveryPolicy,
    work_count: usize,
    audit_count: usize,
    stream: impl IntoIterator<Item = Result<(u8, usize, RecoveryInherent)>>,
    replay: Option<&crate::incoming_replay::VerifiedReplay>,
) -> Result<CheckedWork> {
    policy
        .validate()
        .map_err(|e| format!("incoming work policy: {e:?}"))?;
    if work_count != policy.chunk_hashes.len() || audit_count != policy.audit_chunk_hashes.len() {
        return Err("incomplete incoming work/audit policy coverage".into());
    }
    let mut checked = CheckedWork {
        policy: policy.encode(),
        incoming: Vec::new(),
        prior_incoming: Vec::new(),
        incoming_audits: BTreeMap::new(),
        replay: None,
        touched: BTreeSet::new(),
        witnesses: BTreeMap::new(),
    };
    let (mut work_seen, mut audits_seen, mut terminal_seen) = (0usize, 0usize, false);
    let mut incoming_commitments = BTreeMap::new();
    let mut terminal_pages = Vec::new();
    for item in stream {
        let (kind, index, envelope) = item?;
        if terminal_seen {
            return Err("trailing incoming work-index action".into());
        }
        match kind {
            0 => {
                if index != work_seen || work_seen >= work_count || audits_seen != 0 {
                    return Err("incoming work-index stream order".into());
                }
                let (cursor, commitment) = match envelope.action {
                    RecoveryAction::Apply(chunk) => {
                        let result = (chunk.index, chunk.commitment());
                        for edit in chunk.edits {
                            if edit.scope == StorageScope::Top {
                                checked.touched.insert(edit.key);
                            }
                        }
                        result
                    }
                    RecoveryAction::Incoming(bytes) => {
                        let operation: IncomingOperation = decode(&bytes)?;
                        let result = (operation.index, operation.commitment());
                        if incoming_commitments.insert(result.0, result.1).is_some() {
                            return Err("duplicate incoming work operation index".into());
                        }
                        checked.incoming.push(operation);
                        result
                    }
                    RecoveryAction::Outgoing(bytes) => {
                        let operation: OutgoingOperation = decode(&bytes)?;
                        (operation.index, operation.commitment())
                    }
                    _ => return Err("invalid incoming work-index action".into()),
                };
                if cursor as usize != index || policy.chunk_hashes[index] != commitment {
                    return Err("incoming work-index cursor/commitment mismatch".into());
                }
                work_seen += 1;
            }
            1 => {
                if work_seen != work_count || index != audits_seen || audits_seen >= audit_count {
                    return Err("incoming audit-index stream order".into());
                }
                let (cursor, commitment) = match envelope.action {
                    RecoveryAction::Audit(audit) => {
                        let result = (audit.index, audit.commitment());
                        for witness in audit.witnesses {
                            if witness.scope == StorageScope::Top
                                && checked
                                    .witnesses
                                    .insert(witness.key, witness.value_hash)
                                    .is_some()
                            {
                                return Err("duplicate incoming preservation audit witness".into());
                            }
                        }
                        result
                    }
                    RecoveryAction::NativeBalanceAudit(bytes) => {
                        let audit: framenode_runtime::incident_recovery_incoming::NativeBalanceAudit = decode(&bytes)?;
                        audit
                            .validate()
                            .map_err(|e| format!("native balance audit shape: {e:?}"))?;
                        let result = (audit.baseline.index, audit.commitment());
                        // Only account-value witnesses may be rebased by this action.
                        // Retained bridge/request witnesses still require their exact bytes.
                        for witness in audit.baseline.witnesses {
                            if witness.scope == StorageScope::Top
                                && checked
                                    .witnesses
                                    .insert(witness.key, witness.value_hash)
                                    .is_some()
                            {
                                return Err("duplicate incoming preservation audit witness".into());
                            }
                        }
                        result
                    }
                    RecoveryAction::PrefixAudit(audit) => (audit.index, audit.commitment()),
                    RecoveryAction::IncomingAudit(bytes) => {
                        let audit: IncomingAudit = decode(&bytes)?;
                        audit
                            .validate()
                            .map_err(|error| format!("incoming audit shape: {error:?}"))?;
                        if audit.prior_policy.is_some() {
                            let replay = replay.ok_or("prior-policy incoming audit requires authenticated recovery-parent companion evidence; checkpoint-only coverage cannot admit it")?;
                            let operation = replay.operation(policy, &audit)?;
                            checked.prior_incoming.push(operation.clone());
                            checked.replay = Some(replay.coverage_binding());
                        } else {
                            let operation = incoming_commitments
                                .get(&audit.operation_index)
                                .ok_or("incoming semantic audit has no current work operation")?;
                            if *operation != audit.operation_commitment {
                                return Err(
                                    "incoming semantic audit differs from exact work operation"
                                        .into(),
                                );
                            }
                        }
                        let result = (audit.index, audit.commitment());
                        if checked
                            .incoming_audits
                            .insert((audit.operation_index, audit.operation_commitment), audit)
                            .is_some()
                        {
                            return Err("duplicate incoming semantic audit".into());
                        }
                        result
                    }
                    _ => return Err("invalid incoming audit-index action".into()),
                };
                if cursor as usize != index || policy.audit_chunk_hashes[index] != commitment {
                    return Err("incoming audit-index cursor/commitment mismatch".into());
                }
                audits_seen += 1;
            }
            2 => {
                if work_seen != work_count || audits_seen != audit_count {
                    return Err("incoming terminal-index commitment mismatch".into());
                }
                match envelope.action {
                    RecoveryAction::TerminalAudit(audit)
                        if index == 0
                            && terminal_pages.is_empty()
                            && audit.commitment() == policy.terminal_audit_commitment =>
                    {
                        terminal_seen = true;
                    }
                    RecoveryAction::TerminalAuditPage(page)
                        if index == terminal_pages.len()
                            && page.index as usize == index
                            && page.commitment() == policy.terminal_audit_commitment =>
                    {
                        let last = page.is_last();
                        terminal_pages.push(page);
                        if last {
                            recovery::invariants::join_terminal_pages(&terminal_pages)
                                .map_err(|e| format!("incoming terminal pages: {e:?}"))?;
                            terminal_seen = true;
                        }
                    }
                    _ => return Err("incoming terminal-index commitment mismatch".into()),
                }
            }
            _ => return Err("unknown incoming work-index stream kind".into()),
        }
    }
    if !terminal_seen || work_seen != work_count || audits_seen != audit_count {
        return Err("incomplete incoming work-index stream".into());
    }
    if let Some(replay) = replay {
        for (operation, audit) in replay.historical_operations(policy)? {
            if checked
                .incoming
                .len()
                .saturating_add(checked.prior_incoming.len())
                >= MAX_ITEMS
            {
                return Err("historical incoming coverage exceeds operation bound".into());
            }
            if checked
                .incoming_audits
                .insert(
                    (audit.operation_index, audit.operation_commitment),
                    audit.clone(),
                )
                .is_some()
            {
                return Err("historical incoming audit duplicates current policy evidence".into());
            }
            checked.prior_incoming.push(operation.clone());
            checked.replay = Some(replay.coverage_binding());
        }
    }
    if checked.incoming_audits.len() != checked.incoming.len() + checked.prior_incoming.len() {
        return Err("every incoming work operation requires one matching IncomingAudit".into());
    }
    Ok(checked)
}

#[cfg(test)]
fn check_work(
    policy: &RecoveryPolicy,
    work: Vec<RecoveryInherent>,
    audits: Vec<RecoveryInherent>,
    terminal: RecoveryInherent,
) -> Result<CheckedWork> {
    let work_count = work.len();
    let audit_count = audits.len();
    let stream = work
        .into_iter()
        .enumerate()
        .map(|(i, envelope)| Ok((0, i, envelope)))
        .chain(
            audits
                .into_iter()
                .enumerate()
                .map(|(i, envelope)| Ok((1, i, envelope))),
        )
        .chain(std::iter::once(Ok((2, 0, terminal))));
    check_stream(policy, work_count, audit_count, stream)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalInventory {
    pub schema: String,
    pub checkpoint_hash: String,
    pub frozen_heads_sha256: String,
    pub deliveries: Vec<ExternalDelivery>,
}

/// This is a reviewed obligation, not an authentication/pass flag. Admission
/// obligations can never be discharged by an evidence quarantine.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Obligation {
    Admit,
    Unresolved,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExternalDelivery {
    pub network_scale: String,
    pub message_id: String,
    pub evidence_hash: String,
    pub original_beneficiary: Option<String>,
    pub obligation: Obligation,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub schema: String,
    pub checkpoint_hash: String,
    /// Stable data commitment; no circular commitment to the final Wasm/policy.
    pub mapping_commitment: String,
    pub external_inventory_sha256: String,
    pub decisions: Vec<Decision>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Process,
    Exclude,
    RetainConsumed,
    Quarantine,
    JournalControl,
    ApplyVerifierPeer,
    ApplyAssetRegistration,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Decision {
    pub network_scale: String,
    pub message_id: String,
    pub action: Action,
    pub evidence_hash: String,
    pub original_beneficiary: Option<String>,
    pub effective_beneficiary: Option<String>,
    pub checkpoint_records: Vec<RecordBinding>,
    pub quarantine_reason: Option<QuarantineReason>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuarantineReason {
    MissingReceipt,
    UnresolvedFinality,
    ContradictoryEvidence,
    UnsupportedLegacyAsset,
    UnresolvedHistoricalError,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordBinding {
    pub key: String,
    pub value_hash: String,
    pub status_key: String,
    pub status_hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoveredDelivery {
    pub network_scale: String,
    pub message_id: String,
    pub action: Action,
    pub original_beneficiary: Option<String>,
    pub effective_beneficiary: Option<String>,
    pub checkpoint_records: usize,
    pub checkpoint_authors: Vec<(String, String)>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: &'static str,
    pub policy_commitment: String,
    pub mapping_commitment: String,
    pub checkpoint_hash: String,
    pub metadata_hash: String,
    pub checkpoint_request_records: usize,
    pub checkpoint_transfer_records: usize,
    pub checkpoint_load_records: usize,
    pub checkpoint_unconsumed_records: usize,
    pub checkpoint_unresolved_done_load_records: usize,
    pub checkpoint_request_commitment: String,
    pub external_deliveries: usize,
    pub work_deliveries: usize,
    pub work_control_messages: usize,
    pub deferred_control_execution_verified: bool,
    pub evidence_hashes: Vec<String>,
    pub deliveries: Vec<CoveredDelivery>,
    pub local_pending_coverage_verified: bool,
    pub declared_external_inventory_coverage_verified: bool,
    pub work_coverage_verified: bool,
    pub external_archive_completeness_verified: bool,
    pub external_finality_verified: bool,
    pub authentication_verified: bool,
    pub exact_artifact_execution_verified: bool,
    pub production_ready: bool,
    pub prior_policy_incoming_operations: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prior_policy_replay: Option<crate::incoming_replay::CoverageBinding>,
}

#[derive(Default)]
struct LocalDelivery {
    records: Vec<RecordBinding>,
    consumed_records: Vec<RecordBinding>,
    done_load_records: Vec<(RecordBinding, AccountId32)>,
    consumed: bool,
    beneficiary: Option<AccountId32>,
    authors: BTreeSet<AccountId32>,
    payload_hash: Option<[u8; 32]>,
    historical_adapter_required: bool,
}
struct PlannedControl {
    operation_index: u32,
    operation_commitment: [u8; 32],
    disposition: ControlDisposition,
    evidence: [u8; 32],
}

struct PlannedDelivery {
    operation_index: u32,
    operation_commitment: [u8; 32],
    message_index: usize,
    native_source: GenericAccount,
    start_timepoint: GenericTimepoint,
    native_asset_amount: Option<(framenode_runtime::AssetId, u128)>,
    beneficiary: AccountId32,
    excluded: bool,
    evidence: [u8; 32],
    payload_hash: [u8; 32],
}

fn hash(v: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(v)
}
fn hx(v: &[u8]) -> String {
    format!("0x{}", hex::encode(v))
}
fn h32(v: &str) -> Result<[u8; 32]> {
    let bytes = hex_bytes(v)?
        .try_into()
        .map_err(|_| "incoming identity/hash width")?;
    if bytes == [0; 32] {
        return Err("zero incoming identity/evidence hash".into());
    }
    Ok(bytes)
}
fn decode<T: Decode + Encode>(v: &[u8]) -> Result<T> {
    if v.is_empty() || v.len() > recovery::MAX_CHUNK_BYTES {
        return Err("incoming native value bound".into());
    }
    let mut input = v;
    let x = T::decode_with_depth_limit(
        32,
        &mut codec::MemTrackingInput::new(&mut input, recovery::MAX_CHUNK_BYTES * 4),
    )
    .map_err(|e| e.to_string())?;
    if !input.is_empty() || x.encode() != v {
        return Err("noncanonical incoming native value".into());
    }
    Ok(x)
}
fn identity(network: &str, message: &str) -> Result<Identity> {
    let bytes = hex_bytes(network)?;
    let _: GenericNetworkId = decode(&bytes)?;
    Ok((bytes, h32(message)?))
}
fn prefix(item: &str) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(b"EthBridge"),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat()
}
fn descriptor<'a>(s: &'a MetadataSchema, item: &str) -> Result<&'a Descriptor> {
    let d = s
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == format!("EthBridge.{item}"))
        .ok_or("missing incoming checkpoint descriptor")?;
    if d.prefix != prefix(item)
        || d.hashers != [Hasher::Twox64Concat, Hasher::Identity]
        || d.key_types.len() != 2
    {
        return Err("incoming checkpoint key schema differs".into());
    }
    Ok(d)
}
fn request_key(d: &Descriptor, network: u32, id: H256) -> Vec<u8> {
    [
        d.prefix.clone(),
        sp_io::hashing::twox_64(&network.encode()).to_vec(),
        network.encode(),
        id.encode(),
    ]
    .concat()
}
fn source_identity(key: &[u8], d: &Descriptor) -> Result<(u32, H256)> {
    if key.len() != d.prefix.len() + 44 {
        return Err("incoming checkpoint request key width".into());
    }
    let network: u32 = decode(&key[d.prefix.len() + 8..d.prefix.len() + 12])?;
    let id: H256 = decode(&key[d.prefix.len() + 12..])?;
    if request_key(d, network, id) != key {
        return Err("incoming checkpoint request key hash".into());
    }
    Ok((network, id))
}
fn load_is_transfer(load: &LoadIncomingRequest<Runtime>) -> bool {
    [
        IncomingTransactionRequestKind::Transfer,
        IncomingTransactionRequestKind::TransferXOR,
    ]
    .into_iter()
    .any(|kind| {
        load == &LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
            load.author().clone(),
            load.hash(),
            load.timepoint(),
            kind,
            load.network_id(),
        ))
    })
}
fn mapped(policy: &RecoveryPolicy, owner: &AccountId32) -> AccountId32 {
    policy
        .replacements
        .iter()
        .find(|entry| &entry.old == owner)
        .map(|entry| entry.new.clone())
        .unwrap_or_else(|| owner.clone())
}
fn named_account(value: &Option<String>) -> Result<Option<AccountId32>> {
    value.as_deref().map(crate::account).transpose()
}

fn add_planned(
    out: &mut BTreeMap<Identity, PlannedDelivery>,
    operation: &IncomingOperation,
    network: GenericNetworkId,
    id: H256,
    sender: GenericAccount,
    beneficiary: AccountId32,
    payload_hash: [u8; 32],
) -> Result<()> {
    let sora = GenericNetworkId::Sub(SubNetworkId::Mainnet);
    let source = operation
        .registries
        .iter()
        .filter(|r| r.network == network)
        .collect::<Vec<_>>();
    let receiver = operation
        .registries
        .iter()
        .filter(|r| r.network == sora)
        .collect::<Vec<_>>();
    if source.len() != 1
        || receiver.len() != 1
        || operation.registries.len() != 2
        || operation.finalized_source_evidence == [0; 32]
    {
        return Err("incoming work requires exact source/receiver registries and evidence".into());
    }
    // The runtime canonicalizes parachain identities to V4. Reject other
    // encodings here rather than claim exclusion parity without normalizing them.
    if let GenericAccount::Parachain(location) = &sender {
        if location.encode().first() != Some(&4) {
            return Err("incoming coverage requires canonical V4 parachain sender".into());
        }
    }
    let excluded = source[0].accounts.contains(&sender)
        || receiver[0]
            .accounts
            .contains(&GenericAccount::Sora(beneficiary.clone()));
    let message_index = match &operation.channel {
        IncomingChannel::Substrate {
            commitment: GenericCommitment::Sub(batch),
            ..
        } => (0..batch.messages.len())
            .find(|index| {
                MessageId::batched(network, sora, batch.nonce, *index as u64).hash() == id
            })
            .ok_or("incoming message does not belong to its operation")?,
        _ => 0,
    };
    let (native_source, start_timepoint, native_asset_amount) = match &operation.channel {
        IncomingChannel::Hashi { request, .. }
        | IncomingChannel::HashiAuthenticatedSource { request, .. } => {
            let IncomingRequest::Transfer(transfer) = decode::<IncomingRequest<Runtime>>(request)?
            else {
                return Err("incoming audit HASHI payload is not a transfer".into());
            };
            // The authenticated outer signer decides exclusion. The native
            // request's original `from` remains the application source field.
            (
                GenericAccount::EVM(transfer.from),
                GenericTimepoint::EVM(transfer.at_height),
                Some((transfer.asset_id, transfer.amount)),
            )
        }
        IncomingChannel::Substrate {
            commitment: GenericCommitment::Sub(batch),
            ..
        } => (sender, batch.messages[message_index].timepoint, None),
        IncomingChannel::Generic {
            commitment: GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(message)),
            ..
        } => (sender, GenericTimepoint::TON(message.transaction_id), None),
        _ => return Err("incoming audit has no static native request adapter".into()),
    };
    let message_index = match &operation.channel {
        IncomingChannel::Substrate {
            commitment: GenericCommitment::Sub(batch),
            ..
        } => batch.messages[..message_index]
            .iter()
            .try_fold(0usize, |count, message| {
                classify_substrate_control(&message.payload)
                    .map(|control| count + usize::from(control.is_none()))
                    .map_err(|e| format!("incoming control shape: {e:?}"))
            })?,
        _ => message_index,
    };
    if out.len() >= MAX_ITEMS {
        return Err("incoming work message bound".into());
    }
    if out
        .insert(
            (network.encode(), id.0),
            PlannedDelivery {
                operation_index: operation.index,
                operation_commitment: operation.commitment(),
                message_index,
                native_source,
                start_timepoint,
                native_asset_amount,
                beneficiary,
                excluded,
                evidence: operation.finalized_source_evidence,
                payload_hash,
            },
        )
        .is_some()
    {
        return Err("duplicate incoming message across work operations".into());
    }
    Ok(())
}
fn add_payload(
    out: &mut BTreeMap<Identity, PlannedDelivery>,
    operation: &IncomingOperation,
    network: GenericNetworkId,
    id: MessageId,
    payload: &[u8],
    substrate: bool,
) -> Result<()> {
    let (sender, beneficiary) = match decode::<BridgeCall>(payload)? {
        BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
            sender: GenericAccount::Unknown | GenericAccount::Root, ..
        }) if substrate => return Err("incoming transfer requires an independently authenticated source-attribution adapter; Unknown/Root is not an attacker-exclusion proof".into()),
        BridgeCall::ParachainApp(ParachainAppCall::Transfer { sender: None, .. }) if substrate =>
            return Err("incoming transfer requires an independently authenticated source-attribution adapter; absent parachain sender is not an attacker-exclusion proof".into()),
        BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
            sender,
            recipient: GenericAccount::Sora(recipient),
            ..
        }) if substrate => (sender, recipient),
        BridgeCall::ParachainApp(ParachainAppCall::Transfer {
            sender: Some(sender),
            recipient,
            ..
        }) if substrate => (GenericAccount::Parachain(sender), recipient),
        BridgeCall::JettonApp(JettonAppCall::Transfer {
            sender, recipient, ..
        }) if !substrate => (
            GenericAccount::TON(sender.address().ok_or("incoming TON sender address")?),
            recipient,
        ),
        _ => return Err("unsupported incoming transfer routing in production coverage".into()),
    };
    add_planned(
        out,
        operation,
        network,
        id.hash(),
        sender,
        beneficiary,
        hash(payload),
    )
}

/// Private native inventory certificate shared by coverage and storage
/// compilation. A Done load alone never certifies financial completion.
struct CheckpointInventory {
    local: BTreeMap<Identity, LocalDelivery>,
    rows: usize,
    transfer_rows: usize,
    load_rows: usize,
    unconsumed: usize,
    unresolved_done_loads: usize,
    commitment: [u8; 32],
}

fn scan_checkpoint(schema: &MetadataSchema, storage: &Storage) -> Result<CheckpointInventory> {
    let requests = descriptor(schema, "Requests")?;
    let statuses = descriptor(schema, "RequestStatuses")?;
    let mut local = BTreeMap::<Identity, LocalDelivery>::new();
    let (mut rows, mut transfer_rows, mut load_rows, mut unconsumed) =
        (0usize, 0usize, 0usize, 0usize);
    let mut commitment = hash(b"sora-nexus-complete-incoming-checkpoint-v1");
    for (key, value) in storage
        .top
        .range(requests.prefix.clone()..)
        .take_while(|(key, _)| key.starts_with(&requests.prefix))
    {
        rows += 1;
        if rows > MAX_ROWS {
            return Err("incoming checkpoint row bound".into());
        }
        typed::decode_exact(value, requests.value_type, &schema.registry)?;
        let (network, stored_id) = source_identity(key, requests)?;
        let request: OffchainRequest<Runtime> = decode(value)?;
        if request.hash() != stored_id {
            return Err("incoming stored wrapper identity mismatch".into());
        }
        commitment = hash(&(commitment, key, hash(value)).encode());
        let (message, beneficiary, author, transfer) = match &request {
            OffchainRequest::Incoming(IncomingRequest::Transfer(t), _) => {
                if t.network_id != network {
                    return Err("incoming transfer network mismatch".into());
                }
                transfer_rows += 1;
                (t.tx_hash, Some(t.to.clone()), t.author.clone(), true)
            }
            OffchainRequest::LoadIncoming(load) if load_is_transfer(load) => {
                if load.network_id() != network {
                    return Err("incoming load network mismatch".into());
                }
                load_rows += 1;
                (load.hash(), None, load.author().clone(), false)
            }
            _ => continue,
        };
        let status_key = request_key(statuses, network, stored_id);
        let status_value = storage
            .top
            .get(&status_key)
            .ok_or("incoming checkpoint status missing")?;
        typed::decode_exact(status_value, statuses.value_type, &schema.registry)?;
        let status: RequestStatus = decode(status_value)?;
        commitment = hash(&(commitment, &status_key, hash(status_value)).encode());
        let entry = local
            .entry((GenericNetworkId::EVMLegacy(network).encode(), message.0))
            .or_default();
        if let OffchainRequest::Incoming(IncomingRequest::Transfer(transfer), _) = &request {
            let request = IncomingRequest::Transfer(transfer.clone());
            let payload_hash = hash(&request.encode());
            if entry.payload_hash.is_some_and(|old| old != payload_hash) {
                return Err("conflicting checkpoint incoming payloads".into());
            }
            entry.payload_hash = Some(payload_hash);
            if !matches!(status, RequestStatus::Done)
                && OffchainRequest::incoming(request).hash() != stored_id
            {
                entry.historical_adapter_required = true;
            }
        }
        if let Some(owner) = beneficiary {
            if entry.beneficiary.as_ref().is_some_and(|old| old != &owner) {
                return Err("conflicting checkpoint incoming beneficiaries".into());
            }
            entry.beneficiary = Some(owner);
        }
        let binding = RecordBinding {
            key: hx(key),
            value_hash: hx(&hash(value)),
            status_key: hx(&status_key),
            status_hash: hx(&hash(status_value)),
        };
        if transfer && matches!(status, RequestStatus::Done) {
            entry.consumed = true;
            entry.consumed_records.push(binding.clone());
        }
        if !transfer && matches!(status, RequestStatus::Done) {
            entry
                .done_load_records
                .push((binding.clone(), author.clone()));
        }
        if !matches!(status, RequestStatus::Done) {
            unconsumed += 1;
            if unconsumed > MAX_ITEMS * 2 {
                return Err("unconsumed checkpoint incoming bound".into());
            }
            entry.authors.insert(author);
            entry.records.push(binding);
        }
    }
    // A load becomes Done when its incoming request is registered, before that
    // request credits the beneficiary. Only a Done native Transfer with this
    // exact network/external transaction identity establishes consumption.
    let mut unresolved_done_loads = 0usize;
    for entry in local.values_mut().filter(|entry| !entry.consumed) {
        for (record, author) in entry.done_load_records.drain(..) {
            unresolved_done_loads += 1;
            unconsumed += 1;
            if unconsumed > MAX_ITEMS * 2 {
                return Err("unconsumed checkpoint incoming bound".into());
            }
            entry.authors.insert(author);
            entry.records.push(record);
        }
    }
    Ok(CheckpointInventory {
        local,
        rows,
        transfer_rows,
        load_rows,
        unconsumed,
        unresolved_done_loads,
        commitment,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreservationReport {
    pub schema: &'static str,
    pub metadata_hash: String,
    pub checkpoint_request_commitment: String,
    pub checkpoint_request_records: usize,
    pub unresolved_records: usize,
    pub unresolved_done_load_records: usize,
    pub witnesses: usize,
    pub external_archive_completeness_verified: bool,
    pub incoming_execution_verified: bool,
    pub production_ready: bool,
}

pub struct RetainedStorage {
    pub witnesses: Vec<StorageWitness>,
    pub report: PreservationReport,
}

/// Preserve unresolved checkpoint obligations during storage-only compilation.
/// Consuming one later requires authenticated native execution and rederivation
/// of its final audit; removing preservation witnesses alone is never evidence.
pub fn retained_storage_witnesses(
    schema: &MetadataSchema,
    storage: &Storage,
) -> Result<RetainedStorage> {
    let inventory = scan_checkpoint(schema, storage)?;
    let mut witnesses = BTreeMap::new();
    // RetainConsumed must preserve its native completion proof as well as any
    // unresolved load for that identity. Unrelated completed history needs no
    // new preservation witnesses.
    for record in inventory
        .local
        .values()
        .filter(|entry| !entry.records.is_empty())
        .flat_map(|entry| entry.records.iter().chain(&entry.consumed_records))
    {
        for (key, value) in [
            (&record.key, &record.value_hash),
            (&record.status_key, &record.status_hash),
        ] {
            let key = hex_bytes(key)?;
            let value = h32(value)?;
            if witnesses
                .insert(key, value)
                .is_some_and(|prior| prior != value)
            {
                return Err("conflicting incoming preservation source records".into());
            }
        }
    }
    let report = PreservationReport {
        schema: "sora-nexus-incoming-checkpoint-preservation-v1",
        metadata_hash: schema.metadata_hash.clone(),
        checkpoint_request_commitment: hx(&inventory.commitment),
        checkpoint_request_records: inventory.rows,
        unresolved_records: inventory.unconsumed,
        unresolved_done_load_records: inventory.unresolved_done_loads,
        witnesses: witnesses.len(),
        external_archive_completeness_verified: false,
        incoming_execution_verified: false,
        production_ready: false,
    };
    Ok(RetainedStorage {
        witnesses: witnesses
            .into_iter()
            .map(|(key, value_hash)| StorageWitness {
                scope: StorageScope::Top,
                key,
                value_hash: Some(value_hash),
            })
            .collect(),
        report,
    })
}

/// Bound operator reconciliation and source-inventory artifacts. Their hashes
/// bind the exact admitted files; they do not establish external finality.
pub struct Evidence<'a> {
    pub inventory: &'a ExternalInventory,
    pub reconciliation: &'a Review,
    pub inventory_sha256: &'a str,
    pub finalized_heads_sha256: &'a str,
}

/// Strict coverage over a verified checkpoint, exact policy and typed work.
/// External inventory admission/finality still requires independently approved
/// source evidence and exact-runtime verification; this function cannot mint.
pub fn verify(
    schema: &MetadataSchema,
    storage: &Storage,
    policy: &RecoveryPolicy,
    work: &CheckedWork,
    evidence: Evidence<'_>,
) -> Result<Report> {
    let Evidence {
        inventory: external,
        reconciliation: review,
        inventory_sha256: external_sha256,
        finalized_heads_sha256: heads_sha256,
    } = evidence;
    policy
        .validate()
        .map_err(|e| format!("incoming policy: {e:?}"))?;
    if external.schema != "sora-nexus-incoming-delivery-inventory-v1"
        || review.schema != "sora-nexus-incoming-reconciliation-v1"
        || external.checkpoint_hash != hx(&recovery::CHECKPOINT_HASH)
        || review.checkpoint_hash != external.checkpoint_hash
        || external.frozen_heads_sha256 != heads_sha256
        || review.external_inventory_sha256 != external_sha256
        || h32(&review.mapping_commitment)? != hash(&policy.replacements.encode())
        || work.policy != policy.encode()
        || external.deliveries.len() > MAX_ITEMS
        || review.decisions.len() > MAX_ITEMS
    {
        return Err("incoming coverage context, mapping, policy or bound mismatch".into());
    }
    let CheckpointInventory {
        local,
        rows,
        transfer_rows,
        load_rows,
        unconsumed,
        unresolved_done_loads,
        commitment,
    } = scan_checkpoint(schema, storage)?;
    let mut planned = BTreeMap::new();
    let mut controls = BTreeMap::<Identity, PlannedControl>::new();
    if let Some(replay) = &work.replay {
        replay.verify_checkpoint_storage(storage)?;
    }
    for op in work.incoming.iter().chain(&work.prior_incoming) {
        match &op.channel {
            IncomingChannel::Hashi {
                network, request, ..
            } => {
                let IncomingRequest::Transfer(t) = decode::<IncomingRequest<Runtime>>(request)?
                else {
                    return Err("incoming HASHI work is not a transfer".into());
                };
                if t.network_id != *network {
                    return Err("incoming HASHI work network mismatch".into());
                }
                if t.amount == 0 || t.from == bridge_types::H160::zero() {
                    return Err("incoming HASHI work lacks its native source adapter".into());
                }
                add_planned(
                    &mut planned,
                    op,
                    GenericNetworkId::EVMLegacy(*network),
                    t.tx_hash,
                    GenericAccount::EVM(t.from),
                    t.to,
                    hash(request),
                )?;
            }
            IncomingChannel::HashiAuthenticatedSource {
                network,
                request,
                raw_transaction,
                expected_chain_id,
                ..
            } => {
                // Share the runtime's exact original-request/raw-transaction
                // binding. The recovered outer transaction signer identifies
                // an actor for exclusion; it never establishes beneficial
                // ownership, receipt inclusion or external finality.
                let source =
                    authenticated_hashi_source(request, raw_transaction, *expected_chain_id)
                        .map_err(|e| format!("incoming HASHI authenticated source: {e:?}"))?;
                let IncomingRequest::Transfer(t) = decode::<IncomingRequest<Runtime>>(request)?
                else {
                    return Err("incoming HASHI work is not a transfer".into());
                };
                if t.network_id != *network {
                    return Err("incoming HASHI work network mismatch".into());
                }
                add_planned(
                    &mut planned,
                    op,
                    GenericNetworkId::EVMLegacy(*network),
                    t.tx_hash,
                    GenericAccount::EVM(source.signer),
                    t.to,
                    hash(request),
                )?;
            }
            IncomingChannel::Substrate {
                network,
                commitment: GenericCommitment::Sub(batch),
                ..
            } => {
                // Substrate preparation is read-only structural admission: it
                // validates both registries, mapping conflicts and every payload.
                // Native proof/nonce execution remains an independent release gate.
                framenode_runtime::incident_recovery_incoming::audit_targets(op, policy)
                    .map_err(|e| format!("incoming Substrate admission shape (including source attribution): {e:?}"))?;
                let network = GenericNetworkId::Sub(*network);
                for (index, message) in batch.messages.iter().enumerate() {
                    let id = MessageId::batched(
                        network,
                        GenericNetworkId::Sub(SubNetworkId::Mainnet),
                        batch.nonce,
                        index as u64,
                    );
                    if let Some(disposition) = classify_substrate_control(&message.payload)
                        .map_err(|e| format!("incoming control shape: {e:?}"))?
                    {
                        if controls.len() >= MAX_ITEMS
                            || controls
                                .insert(
                                    (network.encode(), id.hash().0),
                                    PlannedControl {
                                        operation_index: op.index,
                                        operation_commitment: op.commitment(),
                                        disposition,
                                        evidence: op.finalized_source_evidence,
                                    },
                                )
                                .is_some()
                        {
                            return Err("duplicate or excessive incoming control message".into());
                        }
                    } else {
                        add_payload(&mut planned, op, network, id, &message.payload, true)?;
                    }
                }
            }
            IncomingChannel::Generic {
                network: GenericNetworkId::TON(network),
                commitment: GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(message)),
                ..
            } => {
                let network = GenericNetworkId::TON(*network);
                add_payload(
                    &mut planned,
                    op,
                    network,
                    MessageId::basic(
                        network,
                        GenericNetworkId::Sub(SubNetworkId::Mainnet),
                        message.nonce,
                    ),
                    &message.payload,
                    false,
                )?;
            }
            _ => return Err("incoming coverage has no production adapter for this channel".into()),
        }
    }
    if controls
        .keys()
        .any(|id| planned.contains_key(id) || local.contains_key(id))
    {
        return Err("incoming control overlaps a financial receipt".into());
    }
    let mut audited_messages = BTreeMap::<(u32, [u8; 32]), usize>::new();
    for control in controls.values() {
        let id = (control.operation_index, control.operation_commitment);
        if !work.incoming_audits.contains_key(&id) {
            return Err("incoming control lacks its semantic consumption audit".into());
        }
        audited_messages.entry(id).or_default();
    }
    for delivery in planned.values() {
        let audit = work
            .incoming_audits
            .get(&(delivery.operation_index, delivery.operation_commitment))
            .ok_or("incoming delivery lacks its semantic audit")?;
        let expected = audit
            .requests
            .get(delivery.message_index)
            .ok_or("incoming semantic audit omits a message")?;
        match (delivery.excluded, expected) {
            (true, None) => {}
            (false, Some(request))
                if request.beneficiary == mapped(policy, &delivery.beneficiary)
                    && request.source == delivery.native_source
                    && request.start_timepoint == delivery.start_timepoint
                    && delivery.native_asset_amount.is_none_or(|(asset, amount)| {
                        request.asset_id == asset && request.amount == amount
                    }) => {}
            _ => return Err(
                "incoming semantic audit exclusion/beneficiary differs from payload and registry"
                    .into(),
            ),
        }
        *audited_messages
            .entry((delivery.operation_index, delivery.operation_commitment))
            .or_default() += 1;
    }
    for (index, audit) in &work.incoming_audits {
        if audited_messages.get(index).copied() != Some(audit.requests.len()) {
            return Err("incoming semantic audit message count differs from exact work".into());
        }
    }
    let mut inventory = BTreeMap::new();
    for delivery in &external.deliveries {
        h32(&delivery.evidence_hash)?;
        if inventory
            .insert(
                identity(&delivery.network_scale, &delivery.message_id)?,
                delivery,
            )
            .is_some()
        {
            return Err("duplicate external incoming identity".into());
        }
    }
    let mut decisions = BTreeMap::new();
    for decision in &review.decisions {
        h32(&decision.evidence_hash)?;
        if decisions
            .insert(
                identity(&decision.network_scale, &decision.message_id)?,
                decision,
            )
            .is_some()
        {
            return Err("duplicate incoming disposition".into());
        }
    }
    let required: BTreeSet<_> = local
        .iter()
        .filter(|(_, entry)| !entry.records.is_empty())
        .map(|(id, _)| id.clone())
        .chain(inventory.keys().cloned())
        .chain(planned.keys().cloned())
        .chain(controls.keys().cloned())
        .collect();
    if required != decisions.keys().cloned().collect() {
        return Err("incomplete or extraneous incoming dispositions".into());
    }
    let mut evidence = BTreeSet::new();
    let mut deliveries = Vec::new();
    for id in required {
        let decision = decisions[&id];
        let source = local.get(&id);
        let instruction = inventory.get(&id);
        let operation = planned.get(&id);
        let control = controls.get(&id);
        let mut records = source.map(|s| s.records.clone()).unwrap_or_default();
        if decision.action == Action::RetainConsumed {
            records.extend(
                source
                    .map(|s| s.consumed_records.clone())
                    .unwrap_or_default(),
            );
        }
        records.sort();
        let mut supplied = decision.checkpoint_records.clone();
        supplied.sort();
        if supplied != records {
            return Err("incoming disposition does not bind every exact checkpoint record".into());
        }
        if let Some(instruction) = instruction {
            if instruction.evidence_hash != decision.evidence_hash {
                return Err("incoming external evidence substitution".into());
            }
        }
        let original = named_account(&decision.original_beneficiary)?;
        let effective = named_account(&decision.effective_beneficiary)?;
        if effective != original.as_ref().map(|owner| mapped(policy, owner)) {
            return Err("incoming original/effective beneficiary mismatch".into());
        }
        for expected in [
            source.and_then(|s| s.beneficiary.as_ref()),
            operation.map(|o| &o.beneficiary),
        ] {
            if expected.is_some() && original.as_ref() != expected {
                return Err("incoming beneficiary differs from native payload".into());
            }
        }
        if let Some(instruction) = instruction {
            let expected = named_account(&instruction.original_beneficiary)?;
            if expected.is_some() && original != expected {
                return Err("incoming beneficiary differs from external inventory".into());
            }
        }
        let expected = source
            .and_then(|s| s.beneficiary.clone())
            .or_else(|| operation.map(|o| o.beneficiary.clone()))
            .or(instruction
                .map(|i| named_account(&i.original_beneficiary))
                .transpose()?
                .flatten());
        if original != expected {
            return Err("incoming beneficiary has no payload or source evidence".into());
        }
        if control.is_some()
            && !matches!(
                decision.action,
                Action::JournalControl | Action::ApplyVerifierPeer | Action::ApplyAssetRegistration
            )
        {
            return Err("incoming control requires its explicit non-financial disposition".into());
        }
        match decision.action {
            Action::JournalControl | Action::ApplyVerifierPeer | Action::ApplyAssetRegistration => {
                let control =
                    control.ok_or("control disposition has no executable control work")?;
                let expected = match control.disposition {
                    ControlDisposition::Deferred => Action::JournalControl,
                    ControlDisposition::VerifierPeer => Action::ApplyVerifierPeer,
                    ControlDisposition::NativeRegistration => Action::ApplyAssetRegistration,
                };
                if decision.action != expected
                    || original.is_some()
                    || effective.is_some()
                    || instruction.is_none_or(|i| {
                        i.obligation != Obligation::Admit || i.original_beneficiary.is_some()
                    })
                    || control.evidence != h32(&decision.evidence_hash)?
                    || decision.quarantine_reason.is_some()
                    || !records.is_empty()
                    || operation.is_some()
                {
                    return Err(
                        "incoming control disposition/evidence or financial attribution differs"
                            .into(),
                    );
                }
            }
            Action::Process | Action::Exclude => {
                let operation = operation.ok_or("incoming disposition has no executable work")?;
                if source.is_some_and(|s| s.consumed) {
                    return Err("incoming work would replay a consumed checkpoint transfer".into());
                }
                if source.is_some_and(|s| {
                    s.historical_adapter_required
                        || s.payload_hash
                            .is_some_and(|hash| hash != operation.payload_hash)
                }) {
                    return Err(
                        "incoming prepared payload/wrapper requires its exact historical adapter"
                            .into(),
                    );
                }
                if instruction.is_none_or(|i| i.obligation != Obligation::Admit)
                    || operation.evidence != h32(&decision.evidence_hash)?
                    || operation.excluded != (decision.action == Action::Exclude)
                    || decision.quarantine_reason.is_some()
                {
                    return Err("incoming admission, exclusion or evidence mismatch".into());
                }
            }
            Action::RetainConsumed => {
                if !source.is_some_and(|s| s.consumed)
                    || operation.is_some()
                    || decision.quarantine_reason.is_some()
                {
                    return Err(
                        "incoming consumed disposition lacks native completion or duplicates work"
                            .into(),
                    );
                }
            }
            Action::Quarantine => {
                if operation.is_some()
                    || instruction.is_some_and(|i| i.obligation == Obligation::Admit)
                    || decision.quarantine_reason.is_none()
                    || source.is_some_and(|s| s.consumed)
                {
                    return Err("incoming quarantine cannot replace approved admission or consumed evidence".into());
                }
            }
        }
        // Neither quarantine nor already-consumed evidence may be silently
        // erased by storage chunks. Final audits must prove exact preservation.
        if matches!(decision.action, Action::Quarantine | Action::RetainConsumed) {
            for record in &records {
                for (key, value) in [
                    (&record.key, &record.value_hash),
                    (&record.status_key, &record.status_hash),
                ] {
                    let key = hex_bytes(key)?;
                    if work.touched.contains(&key)
                        || work.witnesses.get(&key) != Some(&Some(h32(value)?))
                    {
                        return Err("incoming retained obligation requires unchanged state and final preservation witnesses".into());
                    }
                }
            }
        }
        evidence.insert(decision.evidence_hash.clone());
        deliveries.push(CoveredDelivery {
            network_scale: hx(&id.0),
            message_id: hx(&id.1),
            action: decision.action,
            original_beneficiary: original.map(|a| hx(a.as_ref())),
            effective_beneficiary: effective.map(|a| hx(a.as_ref())),
            checkpoint_records: records.len(),
            checkpoint_authors: source
                .map(|s| {
                    s.authors
                        .iter()
                        .map(|owner| (hx(owner.as_ref()), hx(mapped(policy, owner).as_ref())))
                        .collect()
                })
                .unwrap_or_default(),
        });
    }
    Ok(Report {
        schema: "sora-nexus-incoming-coverage-verification-v1",
        policy_commitment: hx(&policy.commitment()),
        mapping_commitment: hx(&hash(&policy.replacements.encode())),
        checkpoint_hash: hx(&recovery::CHECKPOINT_HASH),
        metadata_hash: schema.metadata_hash.clone(),
        checkpoint_request_records: rows,
        checkpoint_transfer_records: transfer_rows,
        checkpoint_load_records: load_rows,
        checkpoint_unconsumed_records: unconsumed,
        checkpoint_unresolved_done_load_records: unresolved_done_loads,
        checkpoint_request_commitment: hx(&commitment),
        external_deliveries: external.deliveries.len(),
        work_deliveries: planned.len(),
        work_control_messages: controls.len(),
        deferred_control_execution_verified: false,
        evidence_hashes: evidence.into_iter().collect(),
        deliveries,
        local_pending_coverage_verified: true,
        declared_external_inventory_coverage_verified: true,
        work_coverage_verified: true,
        external_archive_completeness_verified: false,
        external_finality_verified: false,
        authentication_verified: false,
        exact_artifact_execution_verified: false,
        production_ready: false,
        prior_policy_incoming_operations: work.prior_incoming.len(),
        prior_policy_replay: work.replay.clone(),
    })
}

#[cfg(test)]
#[path = "incoming_coverage_tests.rs"]
mod tests;
