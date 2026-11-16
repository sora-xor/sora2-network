//! Explicit audits of mapped account records subsequently changed by authenticated incoming work.
//! Ordinary static audits remain exact. Only native-derived account values are journaled; no
//! supplied replacement value, global issuance, pooled custody or exclusion is admitted here.
use super::*;
use alloc::collections::BTreeMap;
use common::recovery::{RecoveryAuditChunk, StorageScope, StorageWitness};

const HEAD_PREFIX: &[u8] = b":sora-nexus:recovery:incoming-balance-head:v1:";
const LINK_PREFIX: &[u8] = b":sora-nexus:recovery:incoming-balance-link:v1:";
pub const MAX_BALANCE_SOURCES: usize = 128;
const MAX_ACCOUNT_BYTES: usize = 4096;

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub struct AuditedIncomingSource {
    pub audit: IncomingAudit,
    /// None selects the current audit policy without a circular policy commitment.
    pub audited_policy: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub struct NativeBalanceAudit {
    /// The original compiler-produced exact witnesses, with only its index remapped.
    pub baseline: RecoveryAuditChunk,
    pub sources: Vec<AuditedIncomingSource>,
}

pub fn request_balance_keys(request: &IncomingRequestExpectation) -> [Vec<u8>; 2] {
    [
        frame_system::Account::<Runtime>::hashed_key_for(&request.beneficiary),
        tokens::Accounts::<Runtime>::hashed_key_for(&request.beneficiary, request.asset_id),
    ]
}

fn source_keys(source: &AuditedIncomingSource) -> BTreeSet<Vec<u8>> {
    source
        .audit
        .requests
        .iter()
        .flatten()
        .flat_map(request_balance_keys)
        .collect()
}

fn mapped_source_keys(
    source: &AuditedIncomingSource,
    policy: &RecoveryPolicy,
) -> BTreeSet<Vec<u8>> {
    source
        .audit
        .requests
        .iter()
        .flatten()
        .filter(|request| {
            policy
                .replacements
                .iter()
                .any(|entry| entry.new == request.beneficiary)
        })
        .flat_map(request_balance_keys)
        .collect()
}

impl NativeBalanceAudit {
    pub fn validate(&self) -> Result<(), RecoveryError> {
        self.baseline.validate()?;
        if self.sources.is_empty()
            || self.sources.len() > MAX_BALANCE_SOURCES
            || self.encoded_size() > MAX_CHUNK_BYTES
            || !self
                .required_weight()
                .all_lte(<Runtime as frame_system::Config>::BlockWeights::get().max_block)
        {
            return Err(RecoveryError::ChunkBounds);
        }
        let mut seen = BTreeSet::new();
        for source in &self.sources {
            source.audit.validate()?;
            if source.audited_policy == Some([0; 32])
                || !seen.insert((
                    source.audited_policy,
                    source.audit.prior_policy,
                    source.audit.operation_index,
                    source.audit.operation_commitment,
                ))
            {
                return Err(RecoveryError::DuplicateEdit);
            }
            let keys = source_keys(source);
            if !self.baseline.witnesses.iter().any(|w| {
                matches!(w.scope, StorageScope::Top)
                    && w.value_hash.is_some()
                    && keys.contains(&w.key)
            }) {
                return Err(RecoveryError::WrongPreimage);
            }
        }
        Ok(())
    }

    pub fn commitment(&self) -> [u8; 32] {
        sp_io::hashing::blake2_256(
            &(b"sora-nexus/native-balance-audit/v1".as_slice(), self).encode(),
        )
    }

    pub fn required_weight(&self) -> Weight {
        audit::incoming_audit_weight()
            .saturating_mul(self.sources.len() as u64)
            .saturating_add(Weight::from_parts(
                20_000_000_000,
                (MAX_CHUNK_BYTES * 2) as u64,
            ))
            .saturating_add(
                <Runtime as frame_system::Config>::DbWeight::get().reads_writes(
                    24 + 8 * self.sources.len() as u64
                        + 4 * self.baseline.witnesses.len() as u64
                        + 2 * (self.baseline.witnesses.len() * self.sources.len()) as u64,
                    2,
                ),
            )
    }
}

type ValueHash = Option<[u8; 32]>;
#[derive(Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
struct Head {
    lineage: [u8; 32],
    baseline: ValueHash,
    current: ValueHash,
    count: u32,
    last: [u8; 32],
}
#[derive(Encode, Decode, DecodeWithMemTracking)]
struct Link {
    lineage: [u8; 32],
    policy: [u8; 32],
    operation: [u8; 32],
    index: u32,
    counter: u64,
    block: u32,
    previous: Option<[u8; 32]>,
    before: ValueHash,
    after: ValueHash,
}
fn key(prefix: &[u8], location: &[u8], operation: Option<[u8; 32]>) -> Vec<u8> {
    let mut key = prefix.to_vec();
    key.extend(sp_io::hashing::blake2_256(location));
    if let Some(operation) = operation {
        key.extend(operation);
    }
    key
}
fn identity(policy: [u8; 32], operation: [u8; 32]) -> [u8; 32] {
    sp_io::hashing::blake2_256(&(policy, operation).encode())
}
fn read(key: &[u8], budget: &mut usize) -> Result<Option<Vec<u8>>, RecoveryError> {
    let Some(length) = sp_io::storage::read(key, &mut [], 0) else {
        return Ok(None);
    };
    *budget = budget
        .checked_sub(length as usize)
        .ok_or(RecoveryError::ChunkBounds)?;
    Ok(sp_io::storage::get(key).map(|v| v.to_vec()))
}
fn account_hash(key: &[u8], budget: &mut usize) -> Result<ValueHash, RecoveryError> {
    let value = read(key, budget)?;
    if value.as_ref().is_some_and(|v| v.len() > MAX_ACCOUNT_BYTES) {
        return Err(RecoveryError::ChunkBounds);
    }
    Ok(value.map(|v| sp_io::hashing::blake2_256(&v)))
}
fn required<T: DecodeWithMemTracking>(key: &[u8], budget: &mut usize) -> Result<T, RecoveryError> {
    decode_bounded(&read(key, budget)?.ok_or(RecoveryError::IncompleteAudit)?)
}

pub(super) struct Prepared {
    lineage: [u8; 32],
    policy: [u8; 32],
    operation: [u8; 32],
    index: u32,
    counter: u64,
    block: u32,
    rows: BTreeMap<Vec<u8>, (ValueHash, Option<Head>)>,
}

pub(super) fn consumption_weight() -> Weight {
    let keys = 2 * u64::from(BridgeMaxMessagesPerCommit::get());
    Weight::from_parts(
        50_000_000 * keys,
        keys * (2 * MAX_ACCOUNT_BYTES as u64 + 1024),
    )
    .saturating_add(
        <Runtime as frame_system::Config>::DbWeight::get().reads_writes(
            12 + 10 * keys + 5 * u64::from(BridgeMaxMessagesPerCommit::get()),
            2 * keys,
        ),
    )
}

/// Called inside the incoming transaction, after authentication admission and before native writes.
pub(super) fn prepare(
    operation: &IncomingOperation,
    active: &ActiveOperation,
    status: &RecoveryStatus,
    policy: &RecoveryPolicy,
) -> Result<Prepared, RecoveryError> {
    let requests = audit::derive_requests(operation, active)?;
    let keys: BTreeSet<_> = requests
        .iter()
        .flatten()
        .filter(|request| {
            policy
                .replacements
                .iter()
                .any(|entry| entry.new == request.beneficiary)
        })
        .flat_map(request_balance_keys)
        .collect();
    if keys.len() > 2 * BridgeMaxMessagesPerCommit::get() as usize {
        return Err(RecoveryError::ChunkBounds);
    }
    let id = identity(active.policy, active.operation);
    let mut budget = MAX_CHUNK_BYTES;
    let mut rows = BTreeMap::new();
    for location in keys {
        let before = account_hash(&location, &mut budget)?;
        let head = read(&key(HEAD_PREFIX, &location, None), &mut budget)?
            .map(|bytes| decode_bounded::<Head>(&bytes))
            .transpose()?;
        if head.as_ref().is_some_and(|h| {
            h.lineage != status.policy
                || h.current != before
                || h.count == 0
                || h.count as usize >= MAX_BALANCE_SOURCES
        }) || sp_io::storage::exists(&key(LINK_PREFIX, &location, Some(id)))
        {
            return Err(RecoveryError::WrongPreimage);
        }
        rows.insert(location, (before, head));
    }
    Ok(Prepared {
        lineage: status.policy,
        policy: active.policy,
        operation: active.operation,
        index: active.index,
        counter: status.operation_counter,
        block: System::block_number(),
        rows,
    })
}

/// Only called after native consumption/receipt validation succeeded, in that same transaction.
pub(super) fn record(prepared: Prepared) -> Result<(), RecoveryError> {
    let id = identity(prepared.policy, prepared.operation);
    let mut budget = MAX_CHUNK_BYTES;
    for (location, (before, previous)) in prepared.rows {
        let head_key = key(HEAD_PREFIX, &location, None);
        let observed = read(&head_key, &mut budget)?
            .map(|b| decode_bounded::<Head>(&b))
            .transpose()?;
        if observed != previous || sp_io::storage::exists(&key(LINK_PREFIX, &location, Some(id))) {
            return Err(RecoveryError::WrongPreimage);
        }
        let after = account_hash(&location, &mut budget)?;
        let link = Link {
            lineage: prepared.lineage,
            policy: prepared.policy,
            operation: prepared.operation,
            index: prepared.index,
            counter: prepared.counter,
            block: prepared.block,
            previous: previous.as_ref().map(|h| h.last),
            before,
            after,
        };
        let head = Head {
            lineage: prepared.lineage,
            baseline: previous.as_ref().map_or(before, |h| h.baseline),
            current: after,
            count: previous.as_ref().map_or(1, |h| h.count + 1),
            last: id,
        };
        sp_io::storage::set(&key(LINK_PREFIX, &location, Some(id)), &link.encode());
        sp_io::storage::set(&head_key, &head.encode());
    }
    Ok(())
}

pub fn native_balance_audit_encoded_weight(bytes: &[u8]) -> Result<Weight, RecoveryError> {
    let audit: NativeBalanceAudit = decode_bounded(bytes)?;
    audit.validate()?;
    Ok(audit.required_weight())
}

fn check(
    policy: &RecoveryPolicy,
    audit: &NativeBalanceAudit,
    status: &RecoveryStatus,
) -> Result<(), RecoveryError> {
    let sources = audit
        .sources
        .iter()
        .map(|source| {
            audit::validate_audited_balance_source(policy, &source.audit, source.audited_policy)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let allowed: Vec<_> = audit
        .sources
        .iter()
        .map(|source| mapped_source_keys(source, policy))
        .collect();
    if allowed.iter().any(|keys| {
        !audit.baseline.witnesses.iter().any(|w| {
            matches!(w.scope, StorageScope::Top) && w.value_hash.is_some() && keys.contains(&w.key)
        })
    }) {
        return Err(RecoveryError::WrongPreimage);
    }
    let mut budget = MAX_CHUNK_BYTES * 2;
    for StorageWitness {
        scope,
        key: location,
        value_hash: baseline,
    } in &audit.baseline.witnesses
    {
        let relevant: Vec<_> = sources
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                matches!(scope, StorageScope::Top)
                    && baseline.is_some()
                    && allowed[*i].contains(location)
            })
            .collect();
        if relevant.is_empty() {
            let raw = match scope {
                StorageScope::Top => read(location, &mut budget)?,
                StorageScope::Child(child) => {
                    let length = sp_io::default_child_storage::read(child, location, &mut [], 0);
                    if let Some(length) = length {
                        budget = budget
                            .checked_sub(length as usize)
                            .ok_or(RecoveryError::ChunkBounds)?;
                    }
                    sp_io::default_child_storage::get(child, location)
                }
            };
            if raw.map(|v| sp_io::hashing::blake2_256(&v)) != *baseline {
                return Err(RecoveryError::WrongPreimage);
            }
            continue;
        }
        let head: Head = required(&key(HEAD_PREFIX, location, None), &mut budget)?;
        if head.lineage != status.policy
            || head.baseline != *baseline
            || head.count as usize != relevant.len()
        {
            return Err(RecoveryError::WrongPreimage);
        }
        let mut previous = None;
        let mut expected = *baseline;
        let mut last_block = recovery::CHECKPOINT_NUMBER;
        for (_, source) in relevant {
            let id = identity(source.consumed_policy, source.operation_commitment);
            let link: Link = required(&key(LINK_PREFIX, location, Some(id)), &mut budget)?;
            if link.lineage != status.policy
                || link.policy != source.consumed_policy
                || link.operation != source.operation_commitment
                || link.index != source.operation_index
                || link.counter != source.consumption_counter
                || link.block != source.execution_block
                || link.block <= last_block
                || link.previous != previous
                || link.before != expected
            {
                return Err(RecoveryError::WrongPreimage);
            }
            previous = Some(id);
            expected = link.after;
            last_block = link.block;
        }
        if previous != Some(head.last)
            || head.current != expected
            || account_hash(location, &mut budget)? != expected
        {
            return Err(RecoveryError::WrongPreimage);
        }
    }
    Ok(())
}

pub fn audit_native_balances_with_policy(
    policy: &RecoveryPolicy,
    bytes: &[u8],
) -> Result<(RecoveryStatus, Weight), RecoveryError> {
    policy.validate()?;
    let audit: NativeBalanceAudit = decode_bounded(bytes)?;
    audit.validate()?;
    let mut status = recovery::storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
    if status.format_version != 1
        || sp_io::storage::get(POLICY_STORAGE_KEY).as_deref() != Some(status.policy.as_slice())
        || sp_io::storage::get(ACTIVE_POLICY_STORAGE_KEY).as_deref()
            != Some(policy.commitment().as_slice())
    {
        return Err(RecoveryError::WrongPolicy);
    }
    if status.phase != RecoveryPhase::Auditing
        || System::block_number() <= recovery::CHECKPOINT_NUMBER
        || sp_io::storage::get(BRIDGE_HALT_STORAGE_KEY).as_deref() != Some(&[1])
        || sp_io::storage::get(recovery::INCOMING_AUDIT_PENDING_KEY).as_deref()
            != Some(&0u32.encode()[..])
        || recovery::TRANSIENT_CONTEXT_KEYS
            .iter()
            .any(|key| sp_io::storage::exists(key))
    {
        return Err(RecoveryError::WrongPhase);
    }
    let cursor: u32 = required(recovery::AUDIT_CURSOR_STORAGE_KEY, &mut 4usize)?;
    if audit.baseline.index != cursor
        || policy.audit_chunk_hashes.get(cursor as usize) != Some(&audit.commitment())
    {
        return Err(RecoveryError::WrongChunk);
    }
    let weight = audit.required_weight();
    if !weight.all_lte(<Runtime as frame_system::Config>::BlockWeights::get().max_block) {
        return Err(RecoveryError::ChunkBounds);
    }
    if let Err(error) = check(policy, &audit, &status) {
        status.phase = RecoveryPhase::Failed;
        status.failure = Some(error);
        sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
        return Err(error);
    }
    sp_io::storage::set(
        recovery::AUDIT_CURSOR_STORAGE_KEY,
        &cursor
            .checked_add(1)
            .ok_or(RecoveryError::ChunkBounds)?
            .encode(),
    );
    Ok((status, weight))
}

#[cfg(test)]
#[path = "incident_recovery_incoming_balance_tests.rs"]
mod tests;
