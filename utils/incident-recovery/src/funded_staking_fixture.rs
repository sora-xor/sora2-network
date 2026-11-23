//! Explicit PUBLIC FIXTURE ONLY funding of a real-signed replacement owner.
//! Configuration is derived through native registration; funds come only from
//! authenticated incoming operations in the actual replay. No live route exists.
use crate::{
    hex_bytes, maintenance::hashi_fixture, mixed_bridge_fixture as h, mixed_work, replay, Result,
    VerifiedCheckpoint,
};
use bridge_types::{GenericAccount, GenericNetworkId, SubNetworkId};
use codec::{Decode, DecodeWithMemLimit, DecodeWithMemTracking, Encode, MaxEncodedLen};
use common::recovery::{
    self, RecoveryAction, RecoveryAuditChunk, RecoveryChunk, RecoveryInherent, RecoveryPolicy,
    StorageScope, StorageWitness,
};
use frame_support::dispatch::GetDispatchInfo;
use framenode_runtime::{
    incident_recovery_incoming::{
        expected_requests, HashiPeerApproval, IncomingAudit, IncomingChannel, IncomingOperation,
        NetworkAttackerRegistry,
    },
    AccountId, Runtime, RuntimeCall, RuntimeOrigin, System,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use sp_core::{ed25519, storage::Storage, Pair, H160, H256, U256};
use sp_runtime::{generic::Era, MultiSignature};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, BufReader, Read},
    path::Path,
};

use crate::hashi_bridge_fixture::wire;
pub const INPUT_SCHEMA: &str = "sora-nexus-PUBLIC-FIXTURE-ONLY-funded-staking-input-v1";
pub const PROVENANCE_SCHEMA: &str = "sora-nexus-public-fixture-funded-hashi-inputs-v1";
pub const DOMAIN: &[u8] = b"sora-nexus/PUBLIC-FIXTURE-ONLY/funded-staking/v1/";
/// One million XOR per deposit, expressed in the unchanged local 18-place unit.
pub const CREDIT: u128 = 1_000_000_000_000_000_000_000_000;
pub const CREDIT_COUNT: u32 = 2;
const ONE: u128 = 1_000_000_000_000_000_000;
// A complete checkpoint composition currently retains ~193 MiB of work and
// audit evidence. Parse/hash one bounded stream instead of duplicating raw JSON.
const MAX_REPORT: usize = 512 * 1024 * 1024;
struct ReportReader<R> {
    inner: R,
    maximum: usize,
    bytes: usize,
    hash: Sha256,
}
impl<R: Read> Read for ReportReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let remaining = self.maximum.saturating_sub(self.bytes);
        let length = output.len().min(remaining.saturating_add(1));
        let count = self.inner.read(&mut output[..length])?;
        if count > remaining {
            return Err(io::Error::other("complete base report exceeds byte bound"));
        }
        self.bytes += count;
        self.hash.update(&output[..count]);
        Ok(count)
    }
}
fn parse_report<R: Read>(reader: R, expected: &str, maximum: usize) -> Result<Value> {
    let reader = ReportReader {
        inner: reader,
        maximum,
        bytes: 0,
        hash: Sha256::new(),
    };
    // Buffer outside the hashing reader: serde_json reads individual bytes,
    // while the checksum processes bounded file-sized chunks efficiently.
    let mut buffered = BufReader::with_capacity(64 * 1024, reader);
    let value =
        serde_json::from_reader(&mut buffered).map_err(|e| format!("complete base report: {e}"))?;
    let reader = buffered.into_inner();
    if hex::encode(reader.hash.finalize()) != expected {
        return Err("complete base report checksum mismatch".into());
    }
    Ok(value)
}
fn load_report(root: &Path, reference: &replay::Artifact) -> Result<Value> {
    let path = crate::local_path(root, &reference.file)?;
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_REPORT as u64 {
        return Err("complete base report exceeds byte bound".into());
    }
    parse_report(file, &reference.sha256, MAX_REPORT)
}
/// Committed birth is the setup block in the exact Init/base/setup/deposit
/// sequence. Its actual authored hash is supplied only by the rehearsal signer.
pub(crate) fn funding_birth(base_chunks: usize) -> Result<u32> {
    if base_chunks == 0 || base_chunks > recovery::MAX_WORK_CHUNKS - 3 {
        return Err("complete base plus setup/deposits exceeds work count bound".into());
    }
    let count = u32::try_from(base_chunks).map_err(|e| e.to_string())?;
    recovery::CHECKPOINT_NUMBER
        .checked_add(2)
        .and_then(|n| n.checked_add(count))
        .ok_or_else(|| "funding birth block overflow".into())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    schema: String,
    fixture_only: bool,
    base_work: replay::Artifact,
    signer_index: usize,
    /// Explicit expectation, never injected into state. Rehearsal checks full ledger.
    expected_era: u32,
    /// First ordinary final138 step in the final manifest, after all code stages.
    first_signed_step: usize,
}
impl Input {
    fn validate(&self) -> Result<()> {
        if self.schema != INPUT_SCHEMA
            || !self.fixture_only
            || self.signer_index >= 3
            || self.first_signed_step >= 100_000
        {
            return Err("explicit public fixture identity/bounds required".into());
        }
        Ok(())
    }
}
fn decode<T: DecodeWithMemTracking + Encode>(bytes: &[u8], bound: usize) -> Result<T> {
    if bytes.is_empty() || bytes.len() > bound {
        return Err("fixture typed bytes outside bound".into());
    }
    let mut input = bytes;
    let value =
        T::decode_with_mem_limit(&mut input, bound.saturating_mul(4)).map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != bytes {
        return Err("noncanonical fixture typed bytes".into());
    }
    Ok(value)
}
// SDK AccountInfo does not implement DecodeWithMemTracking. This concrete
// account type contains only fixed-width integers; keep variable-size policy
// decoding on the separately memory-limited path above.
fn decode_account(bytes: &[u8]) -> Result<framenode_runtime::AccountData> {
    if bytes.len() != framenode_runtime::AccountData::max_encoded_len() {
        return Err("fixed account record length differs".into());
    }
    let mut input = bytes;
    let value = framenode_runtime::AccountData::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != bytes {
        return Err("noncanonical fixed account record".into());
    }
    Ok(value)
}
fn action(text: &str) -> Result<RecoveryAction> {
    RecoveryInherent::decode_remark(&hex_bytes(text)?)
        .map(|v| v.action)
        .map_err(|e| format!("fixture envelope: {e:?}"))
}
fn wrap(action: RecoveryAction) -> String {
    h::hx(
        &RecoveryInherent {
            format_version: 1,
            action,
        }
        .encode_remark(),
    )
}
fn strings(value: &Value, field: &str) -> Result<Vec<String>> {
    let rows = value[field]
        .as_array()
        .ok_or_else(|| format!("missing {field}"))?;
    if (rows.is_empty() && field != "terminalPreludeEnvelopes")
        || rows.len() > recovery::MAX_WORK_CHUNKS
    {
        return Err("fixture work count outside bound".into());
    }
    rows.iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("non-string envelope".into())
        })
        .collect()
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .ok_or_else(|| format!("missing {field}"))
}
pub fn contract() -> H160 {
    H160::from_slice(&h::hash(&[DOMAIN, b"contract"].concat())[..20])
}
pub fn public_raw_transactions() -> Vec<Vec<u8>> {
    wire::source_transactions()
        .into_iter()
        .take(CREDIT_COUNT as usize)
        .collect()
}
pub fn token() -> H160 {
    H160::from_slice(&h::hash(&[DOMAIN, b"token"].concat())[..20])
}
fn external_amount(factor: u128, local: u128) -> Result<U256> {
    if factor == 0 || local == 0 {
        return Err("zero denomination or amount".into());
    }
    let amount = U256::from(local)
        .checked_mul(U256::from(factor))
        .ok_or("external U256 overflow")?;
    if amount / U256::from(factor) != U256::from(local) {
        return Err("denomination roundtrip differs".into());
    }
    Ok(amount)
}

/// The public replacement signing key never substitutes for council authority.
/// Normal-only funding must preserve the exact checkpoint member bytes; the
/// complete typed composer independently verifies this against executing metadata.
fn preserved_council(
    storage: &Storage,
    projected: &Storage,
    policy: &RecoveryPolicy,
) -> Result<()> {
    let key = h::key(b"Council", b"Members");
    let original = storage
        .top
        .get(&key)
        .ok_or("checkpoint Council.Members absent")?;
    if projected.top.get(&key) != Some(original) {
        return Err("funded fixture must preserve checkpoint Council.Members bytes".into());
    }
    let members: Vec<AccountId> = decode(original, 4096)?;
    if members.is_empty()
        || members.len() > 100
        || members.windows(2).any(|p| p[0] >= p[1])
        || members != policy.council
        || policy
            .replacements
            .iter()
            .any(|r| members.contains(&r.old) || members.contains(&r.new))
    {
        return Err("funded fixture requires the unchanged nonmapped checkpoint council".into());
    }
    Ok(())
}

struct Base {
    policy: RecoveryPolicy,
    chunks: Vec<RecoveryChunk>,
    audits: Vec<RecoveryAction>,
    terminal: recovery::invariants::TerminalAudit,
    projected: Storage,
    account: framenode_runtime::AccountData,
}
/// Re-execute the base shared storage engine and all its exact witnesses. A JSON
/// claim of successful composition is never substituted for these checks. Full
/// typed source coverage remains the separately retained fresh composer receipt.
fn base(storage: &Storage, report: &Value, signer: &AccountId) -> Result<Base> {
    if report["schema"] != "sora-nexus-storage-work-v1" || report["productionReady"] != false {
        return Err("requires exact public typed-composer storage report".into());
    }
    let policy: RecoveryPolicy = decode(
        &hex_bytes(text(report, "policyScale")?)?,
        recovery::MAX_INHERENT_BYTES,
    )?;
    policy
        .validate()
        .map_err(|e| format!("base policy: {e:?}"))?;
    if policy.replacements.len() != 1
        || policy.replacements[0].new != *signer
        || text(report, "policyCommitment")? != h::hx(&policy.commitment())
    {
        return Err("base must map one existing owner to the published fixture signer".into());
    }
    let mapping = &policy.replacements[0];
    let old_key = frame_system::Account::<Runtime>::hashed_key_for(&mapping.old);
    let new_key = frame_system::Account::<Runtime>::hashed_key_for(&mapping.new);
    if storage.top.contains_key(&new_key) {
        return Err("replacement already present at checkpoint".into());
    }
    let old = decode_account(
        storage
            .top
            .get(&old_key)
            .ok_or("mapped old owner has no checkpoint System.Account")?,
    )?;
    if old.data.free == 0 {
        return Err("mapped old owner has no positive checkpoint XOR".into());
    }
    let chunks: Vec<_> = strings(report, "applyEnvelopes")?
        .iter()
        .map(|s| match action(s)? {
            RecoveryAction::Apply(c) => Ok(c),
            _ => Err("base requires guarded storage work only".into()),
        })
        .collect::<Result<_>>()?;
    let audits: Vec<_> = strings(report, "auditEnvelopes")?
        .iter()
        .map(|s| match action(s)? {
            a @ (RecoveryAction::Audit(_) | RecoveryAction::PrefixAudit(_)) => Ok(a),
            _ => Err("base accepts original static audits only".into()),
        })
        .collect::<Result<_>>()?;
    let mut terminal_actions = report
        .get("terminalPreludeEnvelopes")
        .map(|_| strings(report, "terminalPreludeEnvelopes"))
        .transpose()?
        .unwrap_or_default()
        .iter()
        .map(|s| action(s))
        .collect::<Result<Vec<_>>>()?;
    terminal_actions.push(action(text(report, "terminalEnvelope")?)?);
    let terminal = if terminal_actions.len() == 1 {
        let RecoveryAction::TerminalAudit(audit) = &terminal_actions[0] else {
            return Err("base terminal missing".into());
        };
        audit.clone()
    } else {
        let pages = terminal_actions
            .iter()
            .map(|a| match a {
                RecoveryAction::TerminalAuditPage(page) => Ok(page.clone()),
                _ => Err("base terminal page differs".to_string()),
            })
            .collect::<Result<Vec<_>>>()?;
        recovery::invariants::join_terminal_pages(&pages)
            .map_err(|e| format!("base terminal pages: {e:?}"))?
    };
    let mut ext = sp_io::TestExternalities::new(storage.clone());
    ext.execute_with(|| -> Result<()> {
        recovery::storage::initialize(&policy).map_err(|e| format!("base initialize: {e:?}"))?;
        for chunk in &chunks {
            recovery::storage::apply_chunk(&policy, chunk)
                .map_err(|e| format!("base chunk: {e:?}"))?;
        }
        for audit in &audits {
            match audit {
                RecoveryAction::Audit(c) => recovery::storage::audit_chunk(&policy, c),
                RecoveryAction::PrefixAudit(c) => recovery::storage::audit_prefix_chunk(&policy, c),
                _ => unreachable!(),
            }
            .map_err(|e| format!("base audit: {e:?}"))?;
        }
        for action in &terminal_actions {
            match action {
                RecoveryAction::TerminalAudit(audit) => {
                    recovery::invariants::verify_and_record(&policy, audit)
                }
                RecoveryAction::TerminalAuditPage(page) => {
                    recovery::invariants::verify_page_and_record(&policy, page)
                }
                _ => unreachable!("typed terminal actions"),
            }
            .map_err(|e| format!("base terminal: {e:?}"))?;
        }
        recovery::storage::mark_complete(&policy).map_err(|e| format!("base complete: {e:?}"))?;
        Ok(())
    })?;
    // Materialize only already verified committed edits, never recovery-engine
    // bookkeeping. Native configuration derivation must run before maintenance.
    let mut projected = storage.clone();
    for edit in chunks.iter().flat_map(|c| &c.edits) {
        let map = match &edit.scope {
            StorageScope::Top => &mut projected.top,
            StorageScope::Child(id) => {
                &mut projected
                    .children_default
                    .get_mut(id)
                    .ok_or("base child absent")?
                    .data
            }
        };
        match &edit.after {
            Some(v) => {
                map.insert(edit.key.clone(), v.clone());
            }
            None => {
                map.remove(&edit.key);
            }
        }
    }
    preserved_council(storage, &projected, &policy)?;
    let account = decode_account(
        projected
            .top
            .get(&new_key)
            .ok_or("mapped replacement System.Account missing")?,
    )?;
    if projected.top.contains_key(&old_key) || account.data!=old.data || account.nonce!=old.nonce
        || !audits.iter().any(|a|matches!(a, RecoveryAction::Audit(c) if c.witnesses.iter().any(|w|w.scope==StorageScope::Top&&w.key==new_key&&w.value_hash==projected.top.get(&new_key).map(|v|h::hash(v))))) {
        return Err("base does not preserve original funded account and its exact baseline witness".into());
    }
    let mut projected_ext = sp_io::TestExternalities::new(projected.clone());
    if projected_ext.execute_with(|| {
        pallet_staking::Bonded::<Runtime>::contains_key(signer)
            || pallet_staking::Ledger::<Runtime>::contains_key(signer)
    }) {
        return Err("public bond scenario requires an unbonded replacement".into());
    }
    Ok(Base {
        policy,
        chunks,
        audits,
        terminal,
        projected,
        account,
    })
}
// Private storage aliases retain these exact source-declared hashers. Keys are
// admission checks for native-generated writes, never permission to invent values.
fn eth_key(item: &[u8], network: u32, tail: Option<&[u8]>, blake_tail: bool) -> Vec<u8> {
    let mut key = h::key(b"EthBridge", item);
    key.extend(sp_io::hashing::twox_64(&network.encode()));
    key.extend(network.encode());
    if let Some(tail) = tail {
        if blake_tail {
            key.extend(sp_io::hashing::blake2_128(tail));
        }
        key.extend(tail);
    }
    key
}
struct Setup {
    chunk: RecoveryChunk,
    network: u32,
    bridge: AccountId,
    operations: Vec<IncomingOperation>,
    configuration: Vec<StorageWitness>,
    external: U256,
}
fn setup(storage: &Storage, policy: &RecoveryPolicy, index: u32) -> Result<Setup> {
    let old = policy.replacements[0].old.clone();
    let peers: Vec<_> = hashi_fixture::pairs().into_iter().map(|(a, _)| a).collect();
    if peers.iter().any(|a| {
        storage
            .top
            .contains_key(&frame_system::Account::<Runtime>::hashed_key_for(a))
    }) {
        return Err("public HASHI peer collides with checkpoint".into());
    }
    let mut ext = sp_io::TestExternalities::new(storage.clone());
    let (network, bridge, external, transfers) = ext.execute_with(|| -> Result<_> {
        if recovery::storage::status()
            .map_err(|e| format!("setup status: {e:?}"))?
            .is_some()
        {
            return Err("network configuration must derive before recovery".into());
        }
        framenode_runtime::Assets::ensure_asset_is_mintable(&common::XOR)
            .map_err(|e| format!("native XOR mintability: {e:?}"))?;
        let network = framenode_runtime::EthBridge::next_network_id();
        if network == framenode_runtime::GetEthNetworkId::get()
            || network == u32::MAX
            || eth_bridge::BridgeAccount::<Runtime>::contains_key(network)
        {
            return Err("new non-Ethereum fixture network unavailable".into());
        }
        for peer in &peers {
            System::inc_providers(peer);
        }
        framenode_runtime::EthBridge::register_bridge(
            RuntimeOrigin::root(),
            contract(),
            peers.clone(),
            eth_bridge::BridgeSignatureVersion::V3,
        )
        .map_err(|e| format!("native register_bridge: {e:?}"))?;
        framenode_runtime::EthBridge::register_existing_sidechain_asset(
            RuntimeOrigin::root(),
            common::XOR,
            token(),
            network,
        )
        .map_err(|e| format!("native register_existing_sidechain_asset: {e:?}"))?;
        let bridge = eth_bridge::BridgeAccount::<Runtime>::get(network)
            .ok_or("registered bridge account absent")?;
        let scope = permissions::Scope::Limited(common::hash(&common::XOR));
        let owner = permissions::Owners::<Runtime>::get(permissions::MINT, scope)
            .pop()
            .or_else(|| {
                permissions::Owners::<Runtime>::get(
                    permissions::MINT,
                    permissions::Scope::Unlimited,
                )
                .pop()
            })
            .ok_or("native XOR mint permission owner absent")?;
        framenode_runtime::Permissions::grant_permission_with_scope(
            owner,
            bridge.clone(),
            permissions::MINT,
            scope,
        )
        .map_err(|e| format!("native scoped mint grant: {e:?}"))?;
        let external = external_amount(
            framenode_runtime::EthBridge::bridge_denomination_factor(network, &common::XOR),
            CREDIT,
        )?;
        let transfers = wire::source_transactions()
            .into_iter()
            .take(CREDIT_COUNT as usize)
            .map(|raw| {
                let tx_hash = H256::from(sp_io::hashing::keccak_256(&raw));
                let load = eth_bridge::requests::LoadIncomingTransactionRequest::new(
                    old.clone(),
                    tx_hash,
                    framenode_runtime::BridgeMultisig::thischain_timepoint(),
                    eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                    network,
                );
                let event = eth_bridge::PublicFixtureContractEvent::Deposit(
                    eth_bridge::PublicFixtureDepositEvent::new(
                        old.clone(),
                        eth_bridge::types::U256::from_little_endian(&external.to_little_endian()),
                        token(),
                        H256::zero(),
                    ),
                );
                let request =
                    eth_bridge::requests::IncomingRequest::<Runtime>::try_from_contract_event(
                        event,
                        load.clone(),
                        123,
                    )
                    .map_err(|e| format!("native U256 Deposit parser: {e:?}"))?;
                let eth_bridge::requests::IncomingRequest::Transfer(transfer) = &request else {
                    return Err("native deposit produced another request kind".into());
                };
                if transfer.amount != CREDIT
                    || transfer.asset_id != common::XOR
                    || transfer.asset_kind != eth_bridge::requests::AssetKind::Sidechain
                    || transfer.from != H160::zero()
                    || transfer.should_take_fee
                {
                    return Err("native funding conversion differs".into());
                }
                for key in [
                    eth_bridge::Requests::<Runtime>::hashed_key_for(network, tx_hash),
                    eth_bridge::RequestStatuses::<Runtime>::hashed_key_for(network, tx_hash),
                    eth_bridge::LoadToIncomingRequestHash::<Runtime>::hashed_key_for(
                        network, tx_hash,
                    ),
                ] {
                    if sp_io::storage::exists(&key) {
                        return Err("funding transaction already consumed".into());
                    }
                }
                Ok((request, load, raw))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok((network, bridge, external, transfers))
    })?;
    let scope = permissions::Scope::Limited(common::hash(&common::XOR));
    let allowed: BTreeSet<_> = peers
        .iter()
        .map(frame_system::Account::<Runtime>::hashed_key_for)
        .chain([
            frame_system::Account::<Runtime>::hashed_key_for(&bridge),
            bridge_multisig::Accounts::<Runtime>::hashed_key_for(&bridge),
            h::key(b"EthBridge", b"NextNetworkId"),
            eth_bridge::BridgeAccount::<Runtime>::hashed_key_for(network),
            eth_key(b"BridgeContractAddress", network, None, false),
            eth_key(b"BridgeStatuses", network, None, false),
            eth_key(b"BridgeSignatureVersions", network, None, false),
            eth_bridge::Peers::<Runtime>::hashed_key_for(network),
            eth_key(
                b"RegisteredAsset",
                network,
                Some(&common::XOR.encode()),
                false,
            ),
            eth_key(
                b"RegisteredSidechainAsset",
                network,
                Some(&token().encode()),
                true,
            ),
            eth_key(
                b"RegisteredSidechainToken",
                network,
                Some(&common::XOR.encode()),
                true,
            ),
            eth_key(
                b"SidechainAssetPrecision",
                network,
                Some(&common::XOR.encode()),
                false,
            ),
            permissions::Permissions::<Runtime>::hashed_key_for(&bridge, scope),
        ])
        .collect();
    let mut changed = h::differences(&h::Values::new(), &h::observed(&mut ext), storage);
    // Native setup events are not recovery edits. Only native-derived configuration
    // and public peer reference counters are committed; economic fields may not move.
    let transient: BTreeSet<_> = [
        h::key(b"System", b"Events"),
        h::key(b"System", b"EventCount"),
        h::key(b"System", b"EventTopics"),
    ]
    .into_iter()
    .collect();
    changed.retain(|k, _| !transient.contains(k));
    if changed.keys().cloned().collect::<BTreeSet<_>>() != allowed {
        return Err("unexpected/missing native configuration writes".into());
    }
    let chunk = h::apply_edits(storage, &changed, index)?;
    let mut configuration = Vec::new();
    for (key, value) in &changed {
        let mut value = value
            .clone()
            .ok_or("fixture setup unexpectedly removed configuration")?;
        if peers
            .iter()
            .take(3)
            .any(|a| frame_system::Account::<Runtime>::hashed_key_for(a) == *key)
        {
            let mut account = decode_account(&value)?;
            account.nonce = account
                .nonce
                .checked_add(CREDIT_COUNT)
                .ok_or("peer nonce overflow")?;
            value = account.encode();
        }
        configuration.push(StorageWitness {
            scope: StorageScope::Top,
            key: key.clone(),
            value_hash: Some(h::hash(&value)),
        });
    }
    let operations = transfers
        .into_iter()
        .enumerate()
        .map(|(offset, (request, load, raw))| -> Result<_> {
            let operation_index = index + 1 + offset as u32;
            let inner = RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
                load_incoming_request: eth_bridge::requests::LoadIncomingRequest::Transaction(load),
                incoming_request_result: Ok(request.clone()),
            });
            let timepoint = bridge_multisig::BridgeTimepoint {
                height: bridge_multisig::MultiChainHeight::Thischain(
                    recovery::CHECKPOINT_NUMBER + 2 + operation_index,
                ),
                index: 0,
            };
            let call = RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
                id: bridge.clone(),
                maybe_timepoint: Some(timepoint),
                call: inner.encode(),
                store_call: false,
                max_weight: ext.execute_with(|| inner.get_dispatch_info().total_weight()),
            })
            .encode();
            let extra = (
                Era::mortal(64, u64::from(funding_birth(index as usize)?)),
                codec::Compact(offset as u32),
                codec::Compact(0u128),
            )
                .encode();
            let source = framenode_runtime::incident_recovery_incoming::authenticated_hashi_source(
                &request.encode(),
                &raw,
                Some(U256::one()),
            )
            .map_err(|e| format!("native public source: {e:?}"))?;
            if source.signer != H160::from(wire::SOURCE_ACTOR) {
                return Err("public funding source differs".into());
            }
            Ok(IncomingOperation {
                index: operation_index,
                channel: IncomingChannel::HashiAuthenticatedSource {
                    network,
                    request: request.encode(),
                    raw_transaction: raw,
                    expected_chain_id: Some(U256::one()),
                    approvals: peers
                        .iter()
                        .take(3)
                        .map(|signer| HashiPeerApproval {
                            signer: signer.clone(),
                            call: call.clone(),
                            extra: extra.clone(),
                            signature: MultiSignature::Ed25519(ed25519::Signature::from_raw(
                                [0; 64],
                            )),
                        })
                        .collect(),
                },
                registries: vec![
                    NetworkAttackerRegistry {
                        network: GenericNetworkId::EVMLegacy(network),
                        accounts: vec![GenericAccount::EVM(H160::from(recovery::ATTACKER_EVM))],
                        no_known_attackers: false,
                        attestation: h::hash(&[DOMAIN, b"external registry"].concat()),
                    },
                    NetworkAttackerRegistry {
                        network: GenericNetworkId::Sub(SubNetworkId::Mainnet),
                        accounts: vec![GenericAccount::Sora(AccountId::new(
                            recovery::ATTACKER_ACCOUNT,
                        ))],
                        no_known_attackers: false,
                        attestation: h::hash(&[DOMAIN, b"local registry"].concat()),
                    },
                ],
                finalized_source_evidence: h::hash(
                    &[DOMAIN, b"SYNTHETIC ONLY NO EXTERNAL FINALITY"].concat(),
                ),
            })
        })
        .collect::<Result<_>>()?;
    Ok(Setup {
        chunk,
        network,
        bridge,
        operations,
        configuration,
        external,
    })
}

/// Disclosed deterministic key material only, for the complete typed composer's CSV/proofs.
pub fn identity(old: &str, signer_index: usize) -> Result<Value> {
    let old = crate::account(old)?;
    let mut pairs = crate::maintenance::fixture::pairs();
    if signer_index >= pairs.len() {
        return Err("public signer index outside bound".into());
    }
    let council: Vec<_> = pairs.iter().map(|(a, _)| h::hx(a.as_ref())).collect();
    let (new, pair) = pairs.remove(signer_index);
    let mapping = recovery::AccountReplacement { old, new };
    recovery::validate_replacements(std::slice::from_ref(&mapping))
        .map_err(|e| format!("fixture mapping: {e:?}"))?;
    let signature =
        MultiSignature::Ed25519(pair.sign(&recovery::replacement_ownership_message(&mapping)));
    if !recovery::verify_replacement_ownership(&mapping, &signature) {
        return Err("public proof failed".into());
    }
    Ok(
        json!({"schema":"sora-nexus-PUBLIC-FIXTURE-ONLY-funded-identity-v1","fixtureOnly":true,"productionReady":false,
        "signerIndex":signer_index,"signerScheme":crate::maintenance::fixture::SCHEME,
        "csv":format!("old account address,new account address\n{},{}\n",h::hx(mapping.old.as_ref()),h::hx(mapping.new.as_ref())),
        "proofs":[{"old":h::hx(mapping.old.as_ref()),"new":h::hx(mapping.new.as_ref()),"signature":h::hx(&signature.encode())}],"council":council,
        "victimAuthorizationProven":false,"checkpointFundsOrCompleteStorageCoverageProven":false}),
    )
}

/// Root-verified typed membership for normal-only public funding. This does not
/// authorize any council signature or report approval of a production release.
pub fn identity_at(verified: VerifiedCheckpoint, old: &str, signer_index: usize) -> Result<Value> {
    let mut result = identity(old, signer_index)?;
    let members = crate::release_checks::checkpoint_council(&verified.schema, &verified.storage)?;
    let original = crate::account(old)?;
    let replacement = crate::account(
        result["proofs"][0]["new"]
            .as_str()
            .ok_or("replacement absent")?,
    )?;
    if members.is_empty() || members.contains(&original) || members.contains(&replacement) {
        return Err("normal funded fixture requires a noncouncil owner and replacement".into());
    }
    let signers = result
        .as_object_mut()
        .ok_or("identity object absent")?
        .remove("council")
        .ok_or("public signing identities absent")?;
    result["schema"] = json!("sora-nexus-PUBLIC-FIXTURE-ONLY-funded-identity-v2");
    result["publicSigners"] = signers;
    result["checkpointCouncil"] = json!(members
        .iter()
        .map(|a| h::hx(a.as_ref()))
        .collect::<Vec<_>>());
    result["checkpointCouncilTypedVerified"] = json!(true);
    result["checkpoint"] = serde_json::to_value(&verified.state).map_err(|e| e.to_string())?;
    result["metadataWitness"] = verified.metadata_witness;
    result["councilApprovalVerified"] = json!(false);
    result["escapeOrRepairAuthorized"] = json!(false);
    Ok(result)
}

pub fn generate(verified: VerifiedCheckpoint, path: &Path) -> Result<Value> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("funded fixture manifest exceeds bound".into());
    }
    let input: Input = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let mut pairs = crate::maintenance::fixture::pairs();
    input.validate()?;
    let (signer, pair) = pairs.remove(input.signer_index);
    let report = load_report(
        path.parent().ok_or("manifest parent missing")?,
        &input.base_work,
    )?;
    let mut base = base(&verified.storage, &report, &signer)?;
    funding_birth(base.chunks.len())?;
    let mut eligibility = sp_io::TestExternalities::new(base.projected.clone());
    eligibility.execute_with(|| -> Result<()> {
        let min = pallet_staking::MinValidatorBond::<Runtime>::get()
            .min(pallet_staking::MinNominatorBond::<Runtime>::get());
        if min > 100 * ONE {
            return Err("public fixed bond is below observed minimum".into());
        }
        let current =
            pallet_staking::CurrentEra::<Runtime>::get().ok_or("checkpoint era missing")?;
        if input.expected_era < current || input.expected_era > current.saturating_add(1) {
            return Err("explicit expected era is outside one deferred boundary".into());
        }
        Ok(())
    })?;
    let setup = setup(&base.projected, &base.policy, base.chunks.len() as u32)?;
    let baseline_account_hash = h::hash(
        base.projected
            .top
            .get(&frame_system::Account::<Runtime>::hashed_key_for(&signer))
            .ok_or("baseline account missing")?,
    );
    let mut configured = base.projected.clone();
    for edit in &setup.chunk.edits {
        configured.top.insert(
            edit.key.clone(),
            edit.after.clone().ok_or("setup deletion")?,
        );
    }
    let mut ext = sp_io::TestExternalities::new(configured);
    let mut audits = ext.execute_with(|| -> Result<Vec<RecoveryAction>> {
        setup
            .operations
            .iter()
            .enumerate()
            .map(|(i, op)| {
                let requests = expected_requests(op, &base.policy)
                    .map_err(|e| format!("funding request descriptor: {e:?}"))?;
                if requests.len() != 1
                    || requests[0].as_ref().is_none_or(|r| {
                        r.beneficiary != signer || r.asset_id != common::XOR || r.amount != CREDIT
                    })
                {
                    return Err("funding must yield one exact remapped XOR credit".into());
                }
                let audit = IncomingAudit {
                    index: i as u32,
                    operation_index: op.index,
                    operation_commitment: op.commitment(),
                    requests,
                    prior_policy: None,
                };
                audit
                    .validate()
                    .map_err(|e| format!("funding audit: {e:?}"))?;
                Ok(RecoveryAction::IncomingAudit(audit.encode()))
            })
            .collect()
    })?;
    // Preserve every original static witness. The only explicit arithmetic update
    // here is the native XOR total issuance field, separately checked by terminal.
    let issuance_key = h::key(b"Balances", b"TotalIssuance");
    let baseline_issuance: u128 = decode(
        base.projected
            .top
            .get(&issuance_key)
            .ok_or("XOR issuance absent")?,
        16,
    )?;
    let total = CREDIT
        .checked_mul(u128::from(CREDIT_COUNT))
        .ok_or("credit overflow")?;
    let _final_issuance = baseline_issuance
        .checked_add(total)
        .ok_or("XOR issuance overflow")?;
    for action in &mut base.audits {
        match action {
            RecoveryAction::Audit(chunk) => {
                chunk.index = audits.len() as u32;
                for w in &chunk.witnesses {
                    // Preserve baseline; native issuance journal supplies the exact final record.
                    if w.scope == StorageScope::Top
                        && w.key == issuance_key
                        && w.value_hash != Some(h::hash(&baseline_issuance.encode()))
                    {
                        return Err("base issuance witness mismatch".into());
                    }
                }
            }
            RecoveryAction::PrefixAudit(chunk) => chunk.index = audits.len() as u32,
            _ => unreachable!(),
        }
        audits.push(action.clone());
    }
    for witnesses in setup.configuration.chunks(recovery::MAX_AUDIT_WITNESSES) {
        audits.push(RecoveryAction::Audit(RecoveryAuditChunk {
            index: audits.len() as u32,
            witnesses: witnesses.to_vec(),
        }));
    }
    let xor = common::XOR.encode();
    let issuance = base
        .terminal
        .issuance
        .iter_mut()
        .find(|row| row.asset.as_slice() == xor.as_slice())
        .ok_or("base terminal XOR invariant absent")?;
    if issuance.current.scope != StorageScope::Top
        || issuance.current.key != issuance_key
        || issuance.current.offset != 0
        || issuance.current.record_hash != h::hash(&baseline_issuance.encode())
    {
        return Err("base terminal XOR projection differs".into());
    }
    if issuance.authenticated_mints != 0 {
        return Err("funded terminal requires native-derived issuance only".into());
    }
    let mut policy = base.policy.clone();
    policy.chunk_hashes.clear();
    policy.audit_chunk_hashes.clear();
    policy.terminal_audit_commitment = [0; 32];
    policy.evidence_commitment = h::hash(
        &(
            DOMAIN,
            base.policy.commitment(),
            input.base_work.sha256.as_bytes(),
            setup.network,
            total,
        )
            .encode(),
    );
    let work: Vec<_> = base
        .chunks
        .into_iter()
        .map(|c| wrap(RecoveryAction::Apply(c)))
        .chain([wrap(RecoveryAction::Apply(setup.chunk))])
        .chain(
            setup
                .operations
                .iter()
                .map(|op| wrap(RecoveryAction::Incoming(op.encode()))),
        )
        .collect();
    let terminal_actions = recovery::invariants::terminal_actions(&base.terminal)
        .map_err(|e| format!("funded terminal pages: {e:?}"))?;
    let mixed = json!({"schema":"sora-nexus-mixed-work-input-v1","policyTemplateScale":h::hx(&policy.encode()),"workEnvelopes":work,"auditEnvelopes":audits.into_iter().map(wrap).collect::<Vec<_>>(),"terminalPreludeEnvelopes":terminal_actions[..terminal_actions.len()-1].iter().cloned().map(wrap).collect::<Vec<_>>(),"terminalEnvelope":wrap(terminal_actions.last().expect("nonempty terminal").clone()),"deriveNativeBalanceAudits":true});
    let compiled = ext.execute_with(|| {
        mixed_work::compile(serde_json::from_value(mixed.clone()).map_err(|e| e.to_string())?)
    })?;
    let policy: RecoveryPolicy = decode(
        &hex_bytes(&compiled.policy_scale)?,
        recovery::MAX_INHERENT_BYTES,
    )?;
    if compiled.native_balance_audit_indices.is_empty() {
        return Err("funded mapped baseline was not derived to native balance audit".into());
    }
    let proof = MultiSignature::Ed25519(pair.sign(&recovery::replacement_ownership_message(
        &policy.replacements[0],
    )));
    if !recovery::verify_replacement_ownership(&policy.replacements[0], &proof) {
        return Err("public replacement ownership signature rejected".into());
    }
    let owner = h::hx(signer.as_ref());
    let payee =
        h::hx(&pallet_staking::RewardDestination::<AccountId>::Account(signer.clone()).encode());
    let unlocking_era = input
        .expected_era
        .checked_add(<Runtime as pallet_staking::Config>::BondingDuration::get())
        .ok_or("unlock era overflow")?;
    let ledger = |total: u128, active: u128, unlocking: Value| json!({"stash":owner,"controller":owner,"total":total.to_string(),"active":active.to_string(),"unlocking":unlocking,"legacyClaimedRewards":[],"payeeScale":payee});
    let values = [
        (
            json!({"kind":"bond","value":(100*ONE).to_string()}),
            Value::Null,
            ledger(100 * ONE, 100 * ONE, json!([])),
        ),
        (
            json!({"kind":"bondExtra","value":(20*ONE).to_string()}),
            ledger(100 * ONE, 100 * ONE, json!([])),
            ledger(120 * ONE, 120 * ONE, json!([])),
        ),
        (
            json!({"kind":"unbond","value":(10*ONE).to_string()}),
            ledger(120 * ONE, 120 * ONE, json!([])),
            ledger(
                120 * ONE,
                110 * ONE,
                json!([[(10 * ONE).to_string(), unlocking_era]]),
            ),
        ),
        (
            json!({"kind":"rebond","value":(5*ONE).to_string()}),
            ledger(
                120 * ONE,
                110 * ONE,
                json!([[(10 * ONE).to_string(), unlocking_era]]),
            ),
            ledger(
                120 * ONE,
                115 * ONE,
                json!([[(5 * ONE).to_string(), unlocking_era]]),
            ),
        ),
    ];
    let transactions=values.into_iter().enumerate().map(|(i,(call,before,after))|->Result<_>{Ok(json!({"step":input.first_signed_step,"expectedNonce":base.account.nonce.checked_add(i as u32).ok_or("signer nonce overflow")?,"minimumFreeBalance":(1000*ONE).to_string(),"maximumFee":(1000*ONE).to_string(),"call":call,"expectedDispatchError":null,"beforeLedger":before,"afterLedger":after}))}).collect::<Result<Vec<_>>>()?;
    let plan = json!({"schema":"sora-nexus-public-fixture-staking-transactions-v1","fixtureOnly":true,"policyCommitment":compiled.policy_commitment,"signerScheme":crate::maintenance::fixture::SCHEME,"signerIndex":input.signer_index,"replacementOwnershipSignature":h::hx(&proof.encode()),"funding":setup.operations.iter().map(|op|json!({"operationIndex":op.index,"operationCommitment":h::hx(&op.commitment()),"minimumAmount":CREDIT.to_string()})).collect::<Vec<_>>(),"transactions":transactions});
    Ok(
        json!({"schema":"sora-nexus-PUBLIC-FIXTURE-ONLY-funded-staking-v1","fixtureOnly":true,"productionReady":false,"warning":"PUBLIC TEST KEYS AND SYNTHETIC EXTERNAL DEPOSITS. NEVER DEPLOY. No external finality, victim authorization or production release admission.","checkpoint":verified.state,"metadataWitness":verified.metadata_witness,"baseWork":{"file":input.base_work.file,"sha256":input.base_work.sha256},"basePolicyCommitment":h::hx(&base.policy.commitment()),"existingMappedXor":base.account.data.free.to_string(),"baselineAccountBlake2":h::hx(&baseline_account_hash),"nativeRegistrationDerived":true,"normalXorDenominationPreserved":true,"externalDepositAmount":setup.external.to_string(),"creditedLocalAmountEach":CREDIT.to_string(),"nativeIncomingExecuted":false,"exactWasmRehearsed":false,"maintenanceEpochLivenessAndStaking":false,"fullTypedMappingCoverageProvenByThisReport":false,"input":mixed,"work":compiled,"fixtureSignedTransactions":plan,"fixtureHashiProvenance":{"schema":PROVENANCE_SCHEMA,"fixtureOnly":true,"productionReady":false,"peerScheme":hashi_fixture::SCHEME,"publicPeers":hashi_fixture::pairs().iter().map(|(a,_)|h::hx(a.as_ref())).collect::<Vec<_>>(),"policyCommitment":h::hx(&policy.commitment()),"checkpointHash":h::hx(&recovery::CHECKPOINT_HASH),"network":setup.network,"bridge":h::hx(setup.bridge.as_ref()),"contract":h::hx(contract().as_bytes()),"token":h::hx(token().as_bytes()),"asset":h::hx(&common::XOR.encode()),"localCredit":CREDIT.to_string(),"creditCount":CREDIT_COUNT},"sourceActorScope":"Published signed raw transaction actor and native peer-approved synthetic Deposit association only; no receipt or external finality is claimed."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_report_stream_hashes_every_byte_and_rejects_overflow_and_changes() {
        let raw = br#"{"schema":"large typed report", "allWork":[1,2,3]}  "#;
        let expected = hex::encode(Sha256::digest(raw));
        let parsed = parse_report(raw.as_slice(), &expected, raw.len()).unwrap();
        assert_eq!(parsed["allWork"], json!([1, 2, 3]));
        assert!(parse_report(raw.as_slice(), &expected, raw.len() - 1).is_err());
        assert!(parse_report(raw.as_slice(), &"00".repeat(32), raw.len()).is_err());
        let mut trailing = raw.to_vec();
        trailing.extend(b"{}");
        assert!(parse_report(
            trailing.as_slice(),
            &hex::encode(Sha256::digest(&trailing)),
            trailing.len()
        )
        .is_err());
        assert_eq!(MAX_REPORT, 512 * 1024 * 1024);
    }
    #[test]
    fn late_birth_preserves_full_739_chunk_base_and_both_actual_deposit_eras() {
        for chunks in [1, 49, 739, recovery::MAX_WORK_CHUNKS - 3] {
            let birth = funding_birth(chunks).unwrap();
            assert_eq!(birth, recovery::CHECKPOINT_NUMBER + 2 + chunks as u32);
            let era = Era::mortal(64, u64::from(birth));
            for offset in 1..=CREDIT_COUNT {
                assert_eq!(era.birth(u64::from(birth + offset)), u64::from(birth));
                assert!(era.death(u64::from(birth + offset)) > u64::from(birth + offset));
            }
        }
        assert!(funding_birth(0).is_err());
        assert!(funding_birth(recovery::MAX_WORK_CHUNKS - 2).is_err());
        assert!(funding_birth(usize::MAX).is_err());
    }
    #[test]
    fn full_more_than_48_guarded_base_chunks_are_reexecuted_not_dropped() {
        let (storage, report, signer) = unit_base(0);
        let original = base(&storage, &report, &signer).unwrap();
        let mut groups: Vec<_> = original
            .chunks
            .into_iter()
            .map(|chunk| chunk.edits)
            .collect();
        for chunk in 0..49u32 {
            groups.push(
                (0..recovery::MAX_EDITS_PER_CHUNK as u32)
                    .map(|entry| recovery::GuardedStorageEdit {
                        scope: StorageScope::Top,
                        key: [
                            b"public-fixture-complete-base/".as_slice(),
                            &chunk.to_le_bytes(),
                            &entry.to_le_bytes(),
                        ]
                        .concat(),
                        before: None,
                        after: Some(vec![7; 16]),
                    })
                    .collect(),
            );
        }
        let composed = crate::composer::compose(
            &storage,
            original.policy.replacements,
            original.policy.council,
            crate::composer::PolicyIdentity::public_fixture([17; 32]),
            groups,
            vec![],
            original.terminal,
        )
        .unwrap();
        let report = serde_json::to_value(composed.report).unwrap();
        let accepted = base(&storage, &report, &signer).unwrap();
        assert!(accepted.chunks.len() > 48);
        assert_eq!(
            accepted
                .projected
                .top
                .iter()
                .filter(|(k, _)| k.starts_with(b"public-fixture-complete-base/"))
                .count(),
            49 * recovery::MAX_EDITS_PER_CHUNK
        );
        assert!(funding_birth(accepted.chunks.len()).is_ok());
        let mut broken = storage;
        let key = [
            b"public-fixture-complete-base/".as_slice(),
            &48u32.to_le_bytes(),
            &0u32.to_le_bytes(),
        ]
        .concat();
        broken.top.insert(key, vec![8; 16]);
        assert!(base(&broken, &report, &signer).is_err());
    }
    #[test]
    fn fixed_account_decoder_requires_exact_complete_record() {
        let mut account = framenode_runtime::AccountData {
            nonce: 17,
            providers: 2,
            ..Default::default()
        };
        account.data.free = CREDIT;
        let encoded = account.encode();
        assert_eq!(decode_account(&encoded).unwrap(), account);
        assert!(decode_account(&encoded[..encoded.len() - 1]).is_err());
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(decode_account(&trailing).is_err());
    }
    #[test]
    fn u256_funding_preserves_actual_large_denomination() {
        let factor = 10u128.pow(38);
        let amount = external_amount(factor, CREDIT).unwrap();
        assert!(amount > U256::from(u128::MAX));
        assert_eq!(amount / U256::from(factor), U256::from(CREDIT));
        assert!(external_amount(0, CREDIT).is_err());
        assert!(external_amount(factor, 0).is_err());
    }
    #[test]
    fn fixture_manifest_rejects_nonfixture_and_unknown_controls() {
        let value = json!({"schema":INPUT_SCHEMA,"fixtureOnly":true,"baseWork":{"file":"work.json","sha256":"00".repeat(32)},"signerIndex":0,"expectedEra":1,"firstSignedStep":42});
        serde_json::from_value::<Input>(value.clone())
            .unwrap()
            .validate()
            .unwrap();
        for field in ["schema", "fixtureOnly", "signerIndex", "firstSignedStep"] {
            let mut rejected = value.clone();
            rejected[field] = match field {
                "schema" => json!("production"),
                "fixtureOnly" => json!(false),
                "signerIndex" => json!(3),
                _ => json!(100_000),
            };
            assert!(serde_json::from_value::<Input>(rejected)
                .unwrap()
                .validate()
                .is_err());
        }
        let mut bad = value;
        bad["balanceOverride"] = json!(1);
        assert!(serde_json::from_value::<Input>(bad).is_err());
    }
    fn unit_base(extra: u128) -> (Storage, Value, AccountId) {
        let signer = crate::maintenance::fixture::pairs().remove(0).0;
        let old = AccountId::new([9; 32]);
        let old_key = frame_system::Account::<Runtime>::hashed_key_for(&old);
        let new_key = frame_system::Account::<Runtime>::hashed_key_for(&signer);
        let supply_key = h::key(b"Balances", b"TotalIssuance");
        let mut account = framenode_runtime::AccountData::default();
        account.data.free = 10 * ONE;
        account.nonce = 7;
        account.providers = 1;
        let mut storage = Storage::default();
        storage.top.insert(
            h::key(b"Council", b"Members"),
            vec![AccountId::new([31; 32]), AccountId::new([32; 32])].encode(),
        );
        storage.top.insert(old_key.clone(), account.encode());
        storage.top.insert(supply_key.clone(), (10 * ONE).encode());
        let mut changed = account.clone();
        changed.data.free += extra;
        let mut edits = vec![
            recovery::GuardedStorageEdit {
                scope: StorageScope::Top,
                key: old_key.clone(),
                before: Some(h::hash(&account.encode())),
                after: None,
            },
            recovery::GuardedStorageEdit {
                scope: StorageScope::Top,
                key: new_key,
                before: None,
                after: Some(changed.encode()),
            },
        ];
        if extra != 0 {
            edits.push(recovery::GuardedStorageEdit {
                scope: StorageScope::Top,
                key: supply_key.clone(),
                before: Some(h::hash(&(10 * ONE).encode())),
                after: Some((10 * ONE + extra).encode()),
            });
        }
        let terminal = recovery::invariants::TerminalAudit {
            issuance: vec![recovery::invariants::IssuanceInvariant {
                asset: common::XOR.encode().try_into().unwrap(),
                current: recovery::invariants::StoredAmount {
                    scope: StorageScope::Top,
                    key: supply_key,
                    record_hash: h::hash(&(10 * ONE + extra).encode()),
                    offset: 0,
                },
                checkpoint_issuance: 10 * ONE,
                authenticated_mints: extra,
                burned: 0,
            }],
            collateral: vec![],
            zero_claims: vec![],
            absent: vec![StorageWitness {
                scope: StorageScope::Top,
                key: old_key,
                value_hash: None,
            }],
        };
        let composed = crate::composer::compose(
            &storage,
            vec![recovery::AccountReplacement {
                old,
                new: signer.clone(),
            }],
            vec![AccountId::new([31; 32]), AccountId::new([32; 32])],
            crate::composer::PolicyIdentity::public_fixture([17; 32]),
            vec![edits],
            vec![],
            terminal,
        )
        .unwrap();
        (
            storage,
            serde_json::to_value(composed.report).unwrap(),
            signer,
        )
    }
    #[test]
    fn base_reexecutes_exact_original_balance_move_and_rejects_minted_baseline() {
        let (storage, report, signer) = unit_base(0);
        let accepted = base(&storage, &report, &signer).unwrap();
        assert_eq!(accepted.account.data.free, 10 * ONE);
        assert_eq!(accepted.account.nonce, 7);
        let (storage, report, signer) = unit_base(ONE);
        assert!(matches!(base(&storage,&report,&signer),Err(error) if error.contains("preserve")));
    }
    #[test]
    fn base_rejects_used_destination_and_tampered_guarded_work() {
        let (mut storage, report, signer) = unit_base(0);
        storage.top.insert(
            frame_system::Account::<Runtime>::hashed_key_for(&signer),
            framenode_runtime::AccountData::default().encode(),
        );
        assert!(base(&storage, &report, &signer).is_err());
        let (storage, mut report, signer) = unit_base(0);
        let RecoveryAction::Apply(mut chunk) =
            action(report["applyEnvelopes"][0].as_str().unwrap()).unwrap()
        else {
            unreachable!()
        };
        chunk.edits[0].before = Some([66; 32]);
        report["applyEnvelopes"][0] = json!(wrap(RecoveryAction::Apply(chunk)));
        assert!(base(&storage, &report, &signer).is_err());
    }
    #[test]
    fn fixture_identity_matches_existing_ownership_verifier_and_cannot_claim_coverage() {
        let output = identity(&h::hx(&[9; 32]), 0).unwrap();
        let map = crate::parse_mapping_csv(output["csv"].as_str().unwrap()).unwrap();
        let proofs: Vec<crate::OwnershipProof> =
            serde_json::from_value(output["proofs"].clone()).unwrap();
        crate::verify_ownership_proofs(&map, &proofs).unwrap();
        assert_eq!(output["productionReady"], false);
        assert_eq!(
            output["checkpointFundsOrCompleteStorageCoverageProven"],
            false
        );
        assert!(identity(&h::hx(&recovery::ATTACKER_ACCOUNT), 0).is_err());
        assert!(identity(&h::hx(&[9; 32]), 3).is_err());
    }
    #[test]
    fn canonical_bytes_reject_trailing_or_wrong_width() {
        assert_eq!(decode::<u128>(&42u128.encode(), 16).unwrap(), 42);
        assert!(decode::<u128>(&[0; 17], 16).is_err());
        assert!(decode::<u128>(&[0; 15], 16).is_err());
    }
    #[test]
    fn published_replacement_proof_is_checkpoint_and_pair_bound() {
        let (signer, pair) = crate::maintenance::fixture::pairs().remove(0);
        let mut mapping = recovery::AccountReplacement {
            old: AccountId::new([9; 32]),
            new: signer,
        };
        let signature =
            MultiSignature::Ed25519(pair.sign(&recovery::replacement_ownership_message(&mapping)));
        assert!(recovery::verify_replacement_ownership(&mapping, &signature));
        mapping.old = AccountId::new([8; 32]);
        assert!(!recovery::verify_replacement_ownership(
            &mapping, &signature
        ));
    }

    #[test]
    fn public_funding_preserves_independent_council_bytes_and_rejects_role_overlap() {
        let (storage, report, signer) = unit_base(0);
        let accepted = base(&storage, &report, &signer).unwrap();
        let members = vec![AccountId::new([31; 32]), AccountId::new([32; 32])];
        assert_eq!(accepted.policy.council, members);
        assert!(!members.contains(&signer));
        let key = h::key(b"Council", b"Members");
        assert_eq!(storage.top.get(&key), accepted.projected.top.get(&key));
        assert_eq!(storage.top.get(&key), Some(&members.encode()));
        let mut changed = accepted.projected.clone();
        changed
            .top
            .insert(key.clone(), vec![signer.clone()].encode());
        assert!(preserved_council(&storage, &changed, &accepted.policy).is_err());
        changed.top.remove(&key);
        assert!(preserved_council(&storage, &changed, &accepted.policy).is_err());
        let mut policy = accepted.policy.clone();
        policy.council = crate::maintenance::fixture::pairs()
            .into_iter()
            .map(|(a, _)| a)
            .collect();
        assert!(preserved_council(&storage, &accepted.projected, &policy).is_err());
        for old in [true, false] {
            let mut policy = accepted.policy.clone();
            if old {
                policy.replacements[0].old = members[0].clone();
            } else {
                policy.replacements[0].new = members[0].clone();
            }
            assert!(preserved_council(&storage, &accepted.projected, &policy).is_err());
        }
        for invalid in [
            vec![],
            vec![members[1].clone(), members[0].clone()],
            vec![members[0].clone(), members[0].clone()],
        ] {
            let mut source = storage.clone();
            source.top.insert(key.clone(), invalid.encode());
            let mut policy = accepted.policy.clone();
            policy.council = invalid;
            assert!(preserved_council(&source, &source, &policy).is_err());
        }
    }
}
