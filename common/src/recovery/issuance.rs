//! Issuance deltas observed only around an authenticated native incoming call.
//! Records live under the protected recovery namespace. No input amount or JSON
//! receipt grants a delta, and ordinary storage edits cannot manufacture a head.
use super::*;
use invariants::IssuanceInvariant;

const HEAD_PREFIX: &[u8] = b":sora-nexus:recovery:incoming-issuance:v1:";
const INDEX_PREFIX: &[u8] = b":sora-nexus:recovery:incoming-issuance-index:v1:";
const COUNT_KEY: &[u8] = b":sora-nexus:recovery:incoming-issuance-count:v1";
const MAX_ISSUANCE_RECORD: usize = 16;

#[derive(Clone, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
struct Head {
    lineage: [u8; 32],
    key: Vec<u8>,
    baseline: [u8; 32],
    current: [u8; 32],
    initial_amount: u128,
    amount: u128,
    minted: sp_core::U256,
    burned: sp_core::U256,
    count: u32,
}
fn head_key(asset: [u8; 32]) -> Vec<u8> {
    [HEAD_PREFIX, &asset].concat()
}
fn decode<T: DecodeWithMemTracking>(bytes: &[u8]) -> Result<T, RecoveryError> {
    let mut input = bytes;
    let value =
        T::decode_with_mem_limit(&mut input, 4096).map_err(|_| RecoveryError::WrongPreimage)?;
    if !input.is_empty() {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(value)
}
fn read_head(asset: [u8; 32]) -> Result<Option<Head>, RecoveryError> {
    let mut budget = 4096;
    storage::read_bounded(&StorageScope::Top, &head_key(asset), &mut budget)?
        .map(|bytes| decode(&bytes))
        .transpose()
}
fn count() -> Result<u32, RecoveryError> {
    sp_io::storage::get(COUNT_KEY)
        .map(|bytes| decode(&bytes))
        .transpose()
        .map(|v| v.unwrap_or(0))
}
fn record(key: &[u8]) -> Result<(u128, [u8; 32]), RecoveryError> {
    if key.is_empty() || key.len() > MAX_STORAGE_KEY_BYTES || key.starts_with(STORAGE_PREFIX) {
        return Err(RecoveryError::ChunkBounds);
    }
    let mut budget = MAX_ISSUANCE_RECORD;
    // FRAME ValueQuery's absent issuance is exactly zero. Hash the canonical
    // u128 encoding, so an absent zero and stored zero share the same semantics.
    let bytes =
        storage::read_bounded(&StorageScope::Top, key, &mut budget)?.unwrap_or(0u128.encode());
    if bytes.len() != MAX_ISSUANCE_RECORD {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok((decode(&bytes)?, sp_io::hashing::blake2_256(&bytes)))
}

/// Cannot be constructed from deserialized evidence. The runtime supplies keys
/// from its native Balances/Tokens storage aliases, inside the incoming transaction.
pub struct Prepared {
    lineage: [u8; 32],
    policy: [u8; 32],
    counter: u64,
    index: u32,
    rows: Vec<([u8; 32], Vec<u8>, u128, [u8; 32], Option<Head>)>,
}
pub fn prepare(
    policy: &RecoveryPolicy,
    index: u32,
    rows: Vec<([u8; 32], Vec<u8>)>,
) -> Result<Prepared, RecoveryError> {
    let state = storage::checked_status(policy)?;
    if state.phase != RecoveryPhase::Applying
        || state.next_chunk != index
        || rows.len() > MAX_EDITS_PER_CHUNK
    {
        return Err(RecoveryError::WrongPhase);
    }
    let mut assets = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut prepared = Vec::new();
    for (asset, key) in rows {
        if !assets.insert(asset) || !keys.insert(key.clone()) {
            return Err(RecoveryError::DuplicateEdit);
        }
        let (before, hash) = record(&key)?;
        let head = read_head(asset)?;
        if head.as_ref().is_some_and(|h| {
            h.lineage != state.policy
                || h.key != key
                || h.current != hash
                || h.amount != before
                || h.count == 0
                || h.count as usize >= MAX_WORK_CHUNKS
        }) {
            return Err(RecoveryError::WrongPreimage);
        }
        prepared.push((asset, key, before, hash, head));
    }
    Ok(Prepared {
        lineage: state.policy,
        policy: policy.commitment(),
        counter: state.operation_counter,
        index,
        rows: prepared,
    })
}

/// Call only after native authentication, execution and consumption recording
/// succeed, within their same transaction. Failed calls roll these writes back.
pub fn record_consumption(
    policy: &RecoveryPolicy,
    prepared: Prepared,
) -> Result<(), RecoveryError> {
    let state = storage::checked_status(policy)?;
    if state.phase != RecoveryPhase::Applying
        || state.policy != prepared.lineage
        || state.operation_counter != prepared.counter
        || state.next_chunk != prepared.index
        || policy.commitment() != prepared.policy
    {
        return Err(RecoveryError::WrongPolicy);
    }
    let mut updated = Vec::new();
    let mut added = 0u32;
    for (asset, key, before, before_hash, previous) in prepared.rows {
        if read_head(asset)? != previous {
            return Err(RecoveryError::WrongPreimage);
        }
        let (after, after_hash) = record(&key)?;
        let mut head = previous.unwrap_or_else(|| {
            added += 1;
            Head {
                lineage: prepared.lineage,
                key,
                baseline: before_hash,
                current: before_hash,
                initial_amount: before,
                amount: before,
                minted: sp_core::U256::zero(),
                burned: sp_core::U256::zero(),
                count: 0,
            }
        });
        if after >= before {
            head.minted = head
                .minted
                .checked_add(sp_core::U256::from(after - before))
                .ok_or(RecoveryError::ChunkBounds)?;
        } else {
            // Prepared attacker sidechain claims may be burned by native
            // cancellation. Track that real decrease rather than hiding it.
            head.burned = head
                .burned
                .checked_add(sp_core::U256::from(before - after))
                .ok_or(RecoveryError::ChunkBounds)?;
        }
        head.current = after_hash;
        head.amount = after;
        head.count += 1;
        updated.push((head_key(asset), head.encode()));
        updated.push((
            [INDEX_PREFIX, &sp_io::hashing::blake2_256(&head.key)].concat(),
            asset.encode(),
        ));
    }
    let total = count()?
        .checked_add(added)
        .ok_or(RecoveryError::ChunkBounds)?;
    if total as usize > invariants::MAX_TERMINAL_PAGES * MAX_EDITS_PER_CHUNK {
        return Err(RecoveryError::ChunkBounds);
    }
    for (key, value) in updated {
        sp_io::storage::set(&key, &value);
    }
    sp_io::storage::set(COUNT_KEY, &total.encode());
    Ok(())
}

/// Returns whether this exact terminal supply consumed a native journal. The
/// terminal page manifest must cover every journaled asset, including assets not
/// held by any mapped account before recovery.
pub(super) fn check(supply: &IssuanceInvariant, budget: &mut usize) -> Result<bool, RecoveryError> {
    let Some(head) = read_head(supply.asset)? else {
        return Ok(false);
    };
    let state = storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
    if head.lineage != state.policy
        || supply.current.scope != StorageScope::Top
        || supply.current.offset != 0
        || supply.current.key != head.key
        || head.count == 0
        || supply.authenticated_mints != 0
        || supply.current.record_hash != head.baseline
        || sp_core::U256::from(supply.checkpoint_issuance)
            != sp_core::U256::from(head.initial_amount) + sp_core::U256::from(supply.burned)
    {
        return Err(RecoveryError::WrongPreimage);
    }
    let bytes =
        storage::read_bounded(&StorageScope::Top, &head.key, budget)?.unwrap_or(0u128.encode());
    if bytes.len() != 16
        || sp_io::hashing::blake2_256(&bytes) != head.current
        || decode::<u128>(&bytes)? != head.amount
        || sp_core::U256::from(supply.checkpoint_issuance) + head.minted
            != sp_core::U256::from(head.amount) + sp_core::U256::from(supply.burned) + head.burned
    {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(true)
}
pub(super) fn verify_coverage(covered: u32) -> Result<(), RecoveryError> {
    if covered != count()? {
        return Err(RecoveryError::IncompleteAudit);
    }
    Ok(())
}

/// An exact baseline storage witness may follow its native-only issuance head.
/// Other records, missing baselines and caller-provided amounts get no exception.
pub(super) fn check_witness(witness: &StorageWitness) -> Result<bool, RecoveryError> {
    if witness.scope != StorageScope::Top {
        return Ok(false);
    }
    let index = [INDEX_PREFIX, &sp_io::hashing::blake2_256(&witness.key)].concat();
    let Some(raw) = sp_io::storage::get(&index) else {
        return Ok(false);
    };
    let asset = decode::<[u8; 32]>(&raw)?;
    let head = read_head(asset)?.ok_or(RecoveryError::WrongPreimage)?;
    let state = storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
    if witness.value_hash != Some(head.baseline) {
        return Ok(false);
    }
    if head.lineage != state.policy || head.key != witness.key || head.count == 0 {
        return Err(RecoveryError::WrongPreimage);
    }
    let (amount, hash) = record(&head.key)?;
    if hash != head.current
        || amount != head.amount
        || sp_core::U256::from(head.initial_amount) + head.minted
            != sp_core::U256::from(head.amount) + head.burned
    {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> RecoveryPolicy {
        RecoveryPolicy {
            format_version: 2,
            checkpoint_hash: CHECKPOINT_HASH,
            checkpoint_state_root: CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![AccountId32::new([8; 32])],
            chunk_hashes: vec![[2; 32]; 3],
            audit_chunk_hashes: vec![[3; 32]],
            evidence_commitment: [4; 32],
            terminal_audit_commitment: [5; 32],
            authorization_domain: RecoveryDomain::public_fixture(),
        }
    }
    #[test]
    fn actual_native_deltas_extend_baseline_and_preserve_burn_accounting() {
        sp_io::TestExternalities::default().execute_with(|| {
            let policy = policy();
            storage::initialize(&policy).unwrap();
            let key = b"supply".to_vec();
            sp_io::storage::set(&key, &80u128.encode());
            let supply = IssuanceInvariant {
                asset: [1; 32],
                current: invariants::StoredAmount {
                    scope: StorageScope::Top,
                    key: key.clone(),
                    record_hash: sp_io::hashing::blake2_256(&80u128.encode()),
                    offset: 0,
                },
                checkpoint_issuance: 100,
                authenticated_mints: 0,
                burned: 20,
            };
            let captured = prepare(&policy, 0, vec![([1; 32], key.clone())]).unwrap();
            sp_io::storage::set(&key, &105u128.encode());
            record_consumption(&policy, captured).unwrap();
            assert_eq!(check(&supply, &mut 4096), Ok(true));
            verify_coverage(1).unwrap();
            assert_eq!(verify_coverage(0), Err(RecoveryError::IncompleteAudit));
            let mut status = storage::status().unwrap().unwrap();
            status.next_chunk = 1;
            storage::put_status(&status);
            let captured = prepare(&policy, 1, vec![([1; 32], key.clone())]).unwrap();
            sp_io::storage::set(&key, &95u128.encode());
            record_consumption(&policy, captured).unwrap();
            assert_eq!(check(&supply, &mut 4096), Ok(true));
            assert!(check_witness(&StorageWitness {
                scope: StorageScope::Top,
                key: key.clone(),
                value_hash: Some(supply.current.record_hash)
            })
            .unwrap());
            // A raw edit cannot hide between native calls or after the last one.
            sp_io::storage::set(&key, &96u128.encode());
            assert!(prepare(&policy, 1, vec![([1; 32], key.clone())]).is_err());
            assert!(check(&supply, &mut 4096).is_err());
            sp_io::storage::set(&key, &95u128.encode());
            let mut fabricated = supply.clone();
            fabricated.authenticated_mints = 1;
            assert!(check(&fabricated, &mut 4096).is_err());
        });
    }
    #[test]
    fn absent_zero_supply_and_rolled_back_consumption_cannot_forge_journals() {
        sp_io::TestExternalities::default().execute_with(|| {
            let policy = policy();
            storage::initialize(&policy).unwrap();
            let key = b"new-token-supply".to_vec();
            sp_io::storage::start_transaction();
            let captured = prepare(&policy, 0, vec![([2; 32], key.clone())]).unwrap();
            sp_io::storage::set(&key, &12u128.encode());
            record_consumption(&policy, captured).unwrap();
            assert_eq!(count().unwrap(), 1);
            sp_io::storage::rollback_transaction();
            assert_eq!(count().unwrap(), 0);
            assert!(read_head([2; 32]).unwrap().is_none());
            let captured = prepare(&policy, 0, vec![([2; 32], key.clone())]).unwrap();
            sp_io::storage::set(&key, &12u128.encode());
            record_consumption(&policy, captured).unwrap();
            let supply = IssuanceInvariant {
                asset: [2; 32],
                current: invariants::StoredAmount {
                    scope: StorageScope::Top,
                    key,
                    record_hash: sp_io::hashing::blake2_256(&0u128.encode()),
                    offset: 0,
                },
                checkpoint_issuance: 0,
                authenticated_mints: 0,
                burned: 0,
            };
            assert_eq!(check(&supply, &mut 4096), Ok(true));
        });
    }
}
