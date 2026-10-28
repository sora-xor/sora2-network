//! Transactional executor for release-committed storage changes.
//!
//! There is deliberately no dispatchable call accepting a policy. A bootstrap
//! runtime must bind the policy and artifact hashes at compile time, authenticate
//! its checkpoint/consensus context, and invoke this engine only from its bounded
//! recovery inherent. This engine does not replace typed offline transformations
//! or the independent economic/consensus admission checks.

use super::*;

pub(super) fn read(scope: &StorageScope, key: &[u8]) -> Option<Vec<u8>> {
    match scope {
        StorageScope::Top => sp_io::storage::get(key).map(|v| v.to_vec()),
        StorageScope::Child(child) => sp_io::default_child_storage::get(child, key),
    }
}

/// Query length before allocating guest memory. Bound aggregate old-value reads
/// too: a small deletion envelope can otherwise reference enormous preimages.
pub(super) fn read_bounded(
    scope: &StorageScope,
    key: &[u8],
    budget: &mut usize,
) -> Result<Option<Vec<u8>>, RecoveryError> {
    let length = match scope {
        StorageScope::Top => sp_io::storage::read(key, &mut [], 0),
        StorageScope::Child(child) => sp_io::default_child_storage::read(child, key, &mut [], 0),
    };
    let Some(length) = length else {
        return Ok(None);
    };
    *budget = budget
        .checked_sub(length as usize)
        .ok_or(RecoveryError::ChunkBounds)?;
    Ok(read(scope, key))
}

fn write(scope: &StorageScope, key: &[u8], value: Option<&[u8]>) {
    match (scope, value) {
        (StorageScope::Top, Some(value)) => sp_io::storage::set(key, value),
        (StorageScope::Top, None) => sp_io::storage::clear(key),
        (StorageScope::Child(child), Some(value)) => {
            sp_io::default_child_storage::set(child, key, value)
        }
        (StorageScope::Child(child), None) => sp_io::default_child_storage::clear(child, key),
    }
}

fn hash(value: Option<Vec<u8>>) -> Option<[u8; 32]> {
    value.map(|value| sp_io::hashing::blake2_256(&value))
}

fn decode_all<T: Decode>(bytes: &[u8]) -> Result<T, RecoveryError> {
    let mut input = bytes;
    let value = T::decode(&mut input).map_err(|_| RecoveryError::WrongPolicy)?;
    if !input.is_empty() {
        return Err(RecoveryError::WrongPolicy);
    }
    Ok(value)
}

pub fn status() -> Result<Option<RecoveryStatus>, RecoveryError> {
    sp_io::storage::get(STATUS_STORAGE_KEY)
        .map(|bytes| decode_all(&bytes))
        .transpose()
}

pub(super) fn put_status(status: &RecoveryStatus) {
    sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
}

pub(super) fn checked_status(policy: &RecoveryPolicy) -> Result<RecoveryStatus, RecoveryError> {
    policy.validate()?;
    let expected = policy.commitment();
    if sp_io::storage::get(ACTIVE_POLICY_STORAGE_KEY).as_deref() != Some(expected.as_slice()) {
        return Err(RecoveryError::WrongPolicy);
    }
    let status = status()?.ok_or(RecoveryError::WrongPolicy)?;
    if status.format_version != 1
        || sp_io::storage::get(POLICY_STORAGE_KEY).as_deref() != Some(status.policy.as_slice())
    {
        return Err(RecoveryError::WrongPolicy);
    }
    Ok(status)
}

/// Called once after the bootstrap has verified the first-child parent and root.
/// Re-entry never resets cursors or council replay counters.
pub fn initialize(policy: &RecoveryPolicy) -> Result<RecoveryStatus, RecoveryError> {
    policy.validate()?;
    if sp_io::storage::exists(POLICY_STORAGE_KEY)
        || sp_io::storage::exists(STATUS_STORAGE_KEY)
        || sp_io::storage::exists(AUDIT_CURSOR_STORAGE_KEY)
    {
        return checked_status(policy);
    }
    let status = RecoveryStatus {
        format_version: 1,
        policy: policy.commitment(),
        phase: RecoveryPhase::Applying,
        next_chunk: 0,
        operation_counter: 0,
        failure: None,
    };
    sp_io::storage::set(POLICY_STORAGE_KEY, &status.policy);
    sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &status.policy);
    sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
    sp_io::storage::set(AUDIT_CURSOR_STORAGE_KEY, &0u32.encode());
    sp_io::storage::set(INCOMING_AUDIT_PENDING_KEY, &0u32.encode());
    put_status(&status);
    Ok(status)
}

/// Mark a validly committed chunk's state mismatch without disabling escape.
pub(super) fn freeze(mut status: RecoveryStatus, error: RecoveryError) -> RecoveryError {
    status.phase = RecoveryPhase::Failed;
    status.failure = Some(error);
    put_status(&status);
    error
}

pub fn apply_chunk(
    policy: &RecoveryPolicy,
    chunk: &RecoveryChunk,
) -> Result<RecoveryStatus, RecoveryError> {
    let mut status = checked_status(policy)?;
    if status.phase != RecoveryPhase::Applying {
        return Err(RecoveryError::WrongPhase);
    }
    chunk.validate()?;
    if chunk.index != status.next_chunk
        || policy.chunk_hashes.get(chunk.index as usize) != Some(&chunk.commitment())
    {
        // Uncommitted input cannot force the chain into a failed phase.
        return Err(RecoveryError::WrongChunk);
    }
    let mut read_budget = MAX_CHUNK_BYTES;
    for edit in &chunk.edits {
        let old = read_bounded(&edit.scope, &edit.key, &mut read_budget)
            .map_err(|error| freeze(status.clone(), error))?;
        if hash(old) != edit.before {
            return Err(freeze(status, RecoveryError::WrongPreimage));
        }
    }
    sp_io::storage::start_transaction();
    for edit in &chunk.edits {
        write(&edit.scope, &edit.key, edit.after.as_deref());
    }
    // The bounds above establish that this cannot overflow: the index is an
    // actual member of a host-memory-sized policy vector.
    status.next_chunk = match status.next_chunk.checked_add(1) {
        Some(next) => next,
        None => {
            sp_io::storage::rollback_transaction();
            return Err(freeze(status, RecoveryError::ChunkBounds));
        }
    };
    if status.next_chunk as usize == policy.chunk_hashes.len() {
        status.phase = RecoveryPhase::Auditing;
    }
    put_status(&status);
    sp_io::storage::commit_transaction();
    Ok(status)
}

/// Audit witnesses are committed independently of the write chunks. The release
/// compiler must include all transformed/deleted keys and conservation witnesses.
pub fn audit_chunk(
    policy: &RecoveryPolicy,
    audit: &RecoveryAuditChunk,
) -> Result<RecoveryStatus, RecoveryError> {
    let status = checked_status(policy)?;
    if status.phase != RecoveryPhase::Auditing {
        return Err(RecoveryError::WrongPhase);
    }
    let cursor = sp_io::storage::get(AUDIT_CURSOR_STORAGE_KEY)
        .ok_or(RecoveryError::IncompleteAudit)
        .and_then(|bytes| decode_all::<u32>(&bytes))?;
    audit.validate()?;
    if audit.index != cursor
        || policy.audit_chunk_hashes.get(cursor as usize) != Some(&audit.commitment())
    {
        return Err(RecoveryError::WrongChunk);
    }
    let mut read_budget = MAX_CHUNK_BYTES;
    for witness in &audit.witnesses {
        let value = read_bounded(&witness.scope, &witness.key, &mut read_budget)
            .map_err(|error| freeze(status.clone(), error))?;
        if hash(value) != witness.value_hash {
            return Err(freeze(status, RecoveryError::WrongPreimage));
        }
    }
    let next = cursor.checked_add(1).ok_or(RecoveryError::ChunkBounds)?;
    sp_io::storage::set(AUDIT_CURSOR_STORAGE_KEY, &next.encode());
    // Only the separately committed terminal invariants may mark Ready.
    Ok(status)
}

/// Share the ordered audit commitment/cursor with exact-key witnesses. Checking
/// both the prefix key itself and its successor also rejects an empty-valued row.
pub fn audit_prefix_chunk(
    policy: &RecoveryPolicy,
    audit: &RecoveryPrefixAuditChunk,
) -> Result<RecoveryStatus, RecoveryError> {
    let status = checked_status(policy)?;
    if status.phase != RecoveryPhase::Auditing {
        return Err(RecoveryError::WrongPhase);
    }
    let cursor = sp_io::storage::get(AUDIT_CURSOR_STORAGE_KEY)
        .ok_or(RecoveryError::IncompleteAudit)
        .and_then(|bytes| decode_all::<u32>(&bytes))?;
    audit.validate()?;
    if audit.index != cursor
        || policy.audit_chunk_hashes.get(cursor as usize) != Some(&audit.commitment())
    {
        return Err(RecoveryError::WrongChunk);
    }
    for witness in &audit.prefixes {
        let (exact, next) = match &witness.scope {
            StorageScope::Top => (
                sp_io::storage::read(&witness.prefix, &mut [], 0).is_some(),
                sp_io::storage::next_key(&witness.prefix),
            ),
            StorageScope::Child(child) => (
                sp_io::default_child_storage::read(child, &witness.prefix, &mut [], 0).is_some(),
                sp_io::default_child_storage::next_key(child, &witness.prefix),
            ),
        };
        if exact || next.is_some_and(|key| key.starts_with(&witness.prefix)) {
            return Err(freeze(status, RecoveryError::WrongPreimage));
        }
    }
    let next = cursor.checked_add(1).ok_or(RecoveryError::ChunkBounds)?;
    sp_io::storage::set(AUDIT_CURSOR_STORAGE_KEY, &next.encode());
    Ok(status)
}

/// Checks the pinned escape target before the runtime invokes its code installer.
/// The installer and `record_upgrade` must run in one enclosing transaction.
pub fn authorize_escape(
    policy: &RecoveryPolicy,
    authorization: &RepairAuthorization,
    signatures: &[(AccountId32, MultiSignature)],
    pinned_hold_hash: [u8; 32],
    code: &[u8],
) -> Result<RecoveryStatus, RecoveryError> {
    let status = checked_status(policy)?;
    authorization.verify_quorum(policy, &status, signatures)?;
    let current = hash(sp_io::storage::get(b":code").map(|value| value.to_vec()))
        .ok_or(RecoveryError::BadCode)?;
    if status.phase == RecoveryPhase::Complete
        || pinned_hold_hash == [0; 32]
        || authorization.current_code_hash != current
        || authorization.destination_code_hash != pinned_hold_hash
        || sp_io::hashing::blake2_256(code) != pinned_hold_hash
        || authorization.destination_spec != HOLD_SPEC_VERSION
        || authorization.revised_policy != [0; 32]
    {
        return Err(RecoveryError::BadCode);
    }
    Ok(status)
}

pub fn record_hold(mut status: RecoveryStatus) -> Result<(), RecoveryError> {
    status.operation_counter = status
        .operation_counter
        .checked_add(1)
        .ok_or(RecoveryError::Replay)?;
    status.phase = RecoveryPhase::Hold;
    status.failure = None;
    put_status(&status);
    Ok(())
}

pub fn mark_complete(policy: &RecoveryPolicy) -> Result<(), RecoveryError> {
    let mut status = checked_status(policy)?;
    if status.phase != RecoveryPhase::Ready
        || sp_io::storage::get(INCOMING_AUDIT_PENDING_KEY).as_deref()
            != Some(0u32.encode().as_slice())
    {
        return Err(RecoveryError::IncompleteAudit);
    }
    status.phase = RecoveryPhase::Complete;
    put_status(&status);
    Ok(())
}

/// Re-plan maintenance work, without rewriting the original fork
/// identity, replacement ownership or emergency council. Call atomically with
/// the signed forward code installation; no ordinary dispatch exposes this.
pub fn install_work_policy(
    original: &RecoveryPolicy,
    revised: &RecoveryPolicy,
    authorization: &RepairAuthorization,
    signatures: &[(AccountId32, MultiSignature)],
) -> Result<(), RecoveryError> {
    let mut state = status()?.ok_or(RecoveryError::WrongPolicy)?;
    original.validate()?;
    revised.validate()?;
    authorization.verify_quorum(original, &state, signatures)?;
    if state.phase == RecoveryPhase::Complete
        || sp_io::storage::get(POLICY_STORAGE_KEY).as_deref()
            != Some(original.commitment().as_slice())
        || authorization.revised_policy != revised.commitment()
        || original.replacements != revised.replacements
        || original.council != revised.council
        || authorization.destination_spec <= HOLD_SPEC_VERSION
        || authorization.destination_spec >= FINAL_SPEC_VERSION
    {
        return Err(RecoveryError::WrongPolicy);
    }
    state.operation_counter = state
        .operation_counter
        .checked_add(1)
        .ok_or(RecoveryError::Replay)?;
    state.phase = RecoveryPhase::Applying;
    state.next_chunk = 0;
    state.failure = None;
    sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &revised.commitment());
    sp_io::storage::set(AUDIT_CURSOR_STORAGE_KEY, &0u32.encode());
    sp_io::storage::clear(TERMINAL_AUDIT_STORAGE_KEY);
    put_status(&state);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (RecoveryPolicy, RecoveryChunk, RecoveryAuditChunk) {
        let chunk = RecoveryChunk {
            index: 0,
            edits: vec![
                GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: b"old".to_vec(),
                    before: Some(sp_io::hashing::blake2_256(b"balance")),
                    after: None,
                },
                GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: b"new".to_vec(),
                    before: None,
                    after: Some(b"balance".to_vec()),
                },
            ],
        };
        let audit = RecoveryAuditChunk {
            index: 0,
            witnesses: vec![
                StorageWitness {
                    scope: StorageScope::Top,
                    key: b"old".to_vec(),
                    value_hash: None,
                },
                StorageWitness {
                    scope: StorageScope::Top,
                    key: b"new".to_vec(),
                    value_hash: Some(sp_io::hashing::blake2_256(b"balance")),
                },
            ],
        };
        let policy = RecoveryPolicy {
            format_version: 1,
            checkpoint_hash: CHECKPOINT_HASH,
            checkpoint_state_root: CHECKPOINT_STATE_ROOT,
            replacements: Vec::new(),
            council: vec![AccountId32::from([1; 32])],
            chunk_hashes: vec![chunk.commitment()],
            audit_chunk_hashes: vec![audit.commitment()],
            evidence_commitment: [2; 32],
            terminal_audit_commitment: [3; 32],
        };
        (policy, chunk, audit)
    }

    #[test]
    fn complete_requires_independent_audit_and_replay_does_not_reapply() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, chunk, audit) = fixture();
            use super::super::invariants::{IssuanceInvariant, StoredAmount, TerminalAudit};
            let issuance = 10u128.encode();
            sp_io::storage::set(b"issuance", &issuance);
            let terminal = TerminalAudit {
                issuance: vec![IssuanceInvariant {
                    asset: [1; 32],
                    current: StoredAmount {
                        scope: StorageScope::Top,
                        key: b"issuance".to_vec(),
                        record_hash: sp_io::hashing::blake2_256(&issuance),
                        offset: 0,
                    },
                    checkpoint_issuance: 12,
                    authenticated_mints: 0,
                    burned: 2,
                }],
                collateral: vec![],
                zero_claims: vec![],
                absent: vec![],
            };
            policy.terminal_audit_commitment = terminal.commitment();
            sp_io::storage::set(b"old", b"balance");
            initialize(&policy).unwrap();
            assert_eq!(mark_complete(&policy), Err(RecoveryError::IncompleteAudit));
            assert_eq!(
                apply_chunk(&policy, &chunk).unwrap().phase,
                RecoveryPhase::Auditing
            );
            assert!(!sp_io::storage::exists(b"old"));
            assert_eq!(&sp_io::storage::get(b"new").unwrap()[..], b"balance");
            assert_eq!(apply_chunk(&policy, &chunk), Err(RecoveryError::WrongPhase));
            assert_eq!(initialize(&policy).unwrap().next_chunk, 1);
            assert_eq!(
                audit_chunk(&policy, &audit).unwrap().phase,
                RecoveryPhase::Auditing
            );
            assert_eq!(mark_complete(&policy), Err(RecoveryError::IncompleteAudit));
            invariants::verify_and_record(&policy, &terminal).unwrap();
            mark_complete(&policy).unwrap();
            assert_eq!(status().unwrap().unwrap().phase, RecoveryPhase::Complete);
            assert_eq!(
                &sp_io::storage::get(BRIDGE_HALT_STORAGE_KEY).unwrap()[..],
                &[1]
            );
        });
    }

    #[test]
    fn larger_exact_audit_checks_last_witness_and_the_unchanged_value_budget() {
        for (value_size, corrupt_last) in [(1024usize, false), (1024, true), (1025, false)] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (mut policy, chunk, mut audit) = fixture();
                sp_io::storage::set(b"old", b"balance");
                audit.witnesses = (0..MAX_AUDIT_WITNESSES)
                    .map(|index| {
                        let key = (b"retained", index as u32).encode();
                        let value = vec![7; value_size];
                        sp_io::storage::set(&key, &value);
                        StorageWitness {
                            scope: StorageScope::Top,
                            key,
                            value_hash: Some(sp_io::hashing::blake2_256(&value)),
                        }
                    })
                    .collect();
                policy.audit_chunk_hashes = vec![audit.commitment()];
                initialize(&policy).unwrap();
                apply_chunk(&policy, &chunk).unwrap();
                if corrupt_last {
                    sp_io::storage::set(&audit.witnesses.last().unwrap().key, &vec![8; value_size]);
                }
                let result = audit_chunk(&policy, &audit);
                if value_size == 1024 && !corrupt_last {
                    assert!(result.is_ok());
                    assert_eq!(
                        sp_io::storage::get(AUDIT_CURSOR_STORAGE_KEY)
                            .unwrap()
                            .as_ref(),
                        1u32.encode().as_slice()
                    );
                } else {
                    assert_eq!(
                        result,
                        Err(if corrupt_last {
                            RecoveryError::WrongPreimage
                        } else {
                            RecoveryError::ChunkBounds
                        })
                    );
                    assert_eq!(status().unwrap().unwrap().phase, RecoveryPhase::Failed);
                    assert_eq!(
                        sp_io::storage::get(AUDIT_CURSOR_STORAGE_KEY)
                            .unwrap()
                            .as_ref(),
                        0u32.encode().as_slice()
                    );
                }
            });
        }
        let audit = RecoveryAuditChunk {
            index: 0,
            witnesses: (0..=MAX_AUDIT_WITNESSES)
                .map(|index| StorageWitness {
                    scope: StorageScope::Top,
                    key: (index as u32).encode(),
                    value_hash: None,
                })
                .collect(),
        };
        assert_eq!(audit.validate(), Err(RecoveryError::ChunkBounds));
    }

    #[test]
    fn destination_collision_freezes_without_deleting_source() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, chunk, _) = fixture();
            sp_io::storage::set(b"old", b"balance");
            sp_io::storage::set(b"new", b"unexpected");
            initialize(&policy).unwrap();
            assert_eq!(
                apply_chunk(&policy, &chunk),
                Err(RecoveryError::WrongPreimage)
            );
            assert_eq!(&sp_io::storage::get(b"old").unwrap()[..], b"balance");
            let failed = status().unwrap().unwrap();
            assert_eq!(failed.phase, RecoveryPhase::Failed);
            assert_eq!(failed.next_chunk, 0);
        });
    }

    #[test]
    fn uncommitted_chunk_cannot_freeze_recovery() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, mut chunk, _) = fixture();
            initialize(&policy).unwrap();
            chunk.index = 1;
            assert_eq!(apply_chunk(&policy, &chunk), Err(RecoveryError::WrongChunk));
            assert_eq!(status().unwrap().unwrap().phase, RecoveryPhase::Applying);
        });
    }

    #[test]
    fn absent_and_empty_values_are_distinct_and_child_edits_are_guarded() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut chunk, _) = fixture();
            chunk.edits = vec![GuardedStorageEdit {
                scope: StorageScope::Child(b"owned-child".to_vec()),
                key: b"position".to_vec(),
                before: None,
                after: Some(Vec::new()),
            }];
            policy.chunk_hashes = vec![chunk.commitment()];
            initialize(&policy).unwrap();
            apply_chunk(&policy, &chunk).unwrap();
            assert_eq!(read(&chunk.edits[0].scope, b"position"), Some(Vec::new()));
            assert_ne!(hash(read(&chunk.edits[0].scope, b"position")), None);
        });
    }

    #[test]
    fn forward_repair_preserves_lineage_and_council_but_advances_work_and_counter() {
        use sp_core::{sr25519, Pair};
        sp_io::TestExternalities::default().execute_with(|| {
            let signer = sr25519::Pair::from_seed(&[42; 32]);
            let account = AccountId32::from(signer.public().0);
            let (mut original, _, _) = fixture();
            original.council = vec![account.clone()];
            let mut revised = original.clone();
            revised.chunk_hashes = vec![[9; 32]];
            let state = initialize(&original).unwrap();
            record_hold(state).unwrap();
            let state = status().unwrap().unwrap();
            let auth = RepairAuthorization {
                policy: original.commitment(),
                phase: state.phase,
                cursor: state.next_chunk,
                operation_counter: state.operation_counter,
                current_code_hash: [4; 32],
                current_executor_hash: [4; 32],
                destination_code_hash: [5; 32],
                destination_spec: 136,
                revised_policy: revised.commitment(),
            };
            let signatures = vec![(
                account,
                MultiSignature::from(signer.sign(&auth.signing_message())),
            )];
            let mut invalid = revised.clone();
            invalid.council = vec![AccountId32::from([8; 32])];
            assert_eq!(
                install_work_policy(&original, &invalid, &auth, &signatures),
                Err(RecoveryError::WrongPolicy)
            );
            install_work_policy(&original, &revised, &auth, &signatures).unwrap();
            let new_state = checked_status(&revised).unwrap();
            assert_eq!(new_state.policy, original.commitment());
            assert_eq!(new_state.operation_counter, 2);
            assert_eq!(new_state.phase, RecoveryPhase::Applying);
            assert_eq!(checked_status(&original), Err(RecoveryError::WrongPolicy));
            assert_eq!(
                install_work_policy(&original, &revised, &auth, &signatures),
                Err(RecoveryError::Replay)
            );
            assert!(!sp_io::storage::exists(TERMINAL_AUDIT_STORAGE_KEY));
        });
    }
}

#[cfg(test)]
mod prefix_tests {
    use super::*;

    fn setup(prefixes: Vec<StoragePrefixWitness>) -> (RecoveryPolicy, RecoveryPrefixAuditChunk) {
        let work = RecoveryChunk {
            index: 0,
            edits: vec![GuardedStorageEdit {
                scope: StorageScope::Top,
                key: b"work".to_vec(),
                before: None,
                after: Some(b"done".to_vec()),
            }],
        };
        let exact = RecoveryAuditChunk {
            index: 0,
            witnesses: vec![StorageWitness {
                scope: StorageScope::Top,
                key: b"work".to_vec(),
                value_hash: Some(sp_io::hashing::blake2_256(b"done")),
            }],
        };
        let prefix = RecoveryPrefixAuditChunk { index: 1, prefixes };
        let policy = RecoveryPolicy {
            format_version: 1,
            checkpoint_hash: CHECKPOINT_HASH,
            checkpoint_state_root: CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![AccountId32::from([1; 32])],
            chunk_hashes: vec![work.commitment()],
            audit_chunk_hashes: vec![exact.commitment(), prefix.commitment()],
            evidence_commitment: [2; 32],
            terminal_audit_commitment: [3; 32],
        };
        initialize(&policy).unwrap();
        apply_chunk(&policy, &work).unwrap();
        audit_chunk(&policy, &exact).unwrap();
        (policy, prefix)
    }

    #[test]
    fn empty_prefix_audit_advances_shared_cursor_and_rejects_replay_or_uncommitted_work() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, prefix) = setup(vec![StoragePrefixWitness {
                scope: StorageScope::Top,
                prefix: b"pending/".to_vec(),
            }]);
            sp_io::storage::set(b"pending0", b"outside namespace");
            let mut uncommitted = prefix.clone();
            uncommitted.prefixes[0].prefix = b"different/".to_vec();
            assert_eq!(
                audit_prefix_chunk(&policy, &uncommitted),
                Err(RecoveryError::WrongChunk)
            );
            assert_eq!(status().unwrap().unwrap().phase, RecoveryPhase::Auditing);
            assert_eq!(
                audit_prefix_chunk(&policy, &prefix).unwrap().phase,
                RecoveryPhase::Auditing
            );
            assert_eq!(
                sp_io::storage::get(AUDIT_CURSOR_STORAGE_KEY)
                    .unwrap()
                    .to_vec(),
                2u32.encode()
            );
            assert_eq!(
                audit_prefix_chunk(&policy, &prefix),
                Err(RecoveryError::WrongChunk)
            );
            assert_eq!(mark_complete(&policy), Err(RecoveryError::IncompleteAudit));
        });
    }

    #[test]
    fn exact_empty_value_and_successor_both_violate_namespace_absence() {
        for key in [b"pending/".as_slice(), b"pending/task".as_slice()] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, prefix) = setup(vec![StoragePrefixWitness {
                    scope: StorageScope::Top,
                    prefix: b"pending/".to_vec(),
                }]);
                sp_io::storage::set(key, b"");
                assert_eq!(
                    audit_prefix_chunk(&policy, &prefix),
                    Err(RecoveryError::WrongPreimage)
                );
                assert_eq!(status().unwrap().unwrap().phase, RecoveryPhase::Failed);
                assert_eq!(
                    sp_io::storage::get(AUDIT_CURSOR_STORAGE_KEY)
                        .unwrap()
                        .to_vec(),
                    1u32.encode()
                );
                assert!(sp_io::storage::exists(key));
            });
        }
    }

    #[test]
    fn child_prefixes_are_scoped_and_check_exact_and_successor_keys() {
        for key in [
            None,
            Some(b"pending/".as_slice()),
            Some(b"pending/task".as_slice()),
        ] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, prefix) = setup(vec![StoragePrefixWitness {
                    scope: StorageScope::Child(b"child".to_vec()),
                    prefix: b"pending/".to_vec(),
                }]);
                sp_io::storage::set(b"pending/top", b"other trie");
                sp_io::default_child_storage::set(b"other-child", b"pending/other", b"other trie");
                if let Some(key) = key {
                    sp_io::default_child_storage::set(b"child", key, b"");
                }
                assert_eq!(audit_prefix_chunk(&policy, &prefix).is_ok(), key.is_none());
            });
        }
    }

    #[test]
    fn prefix_audits_are_bounded_canonical_and_have_a_distinct_commitment_domain() {
        let witness = |prefix: &[u8]| StoragePrefixWitness {
            scope: StorageScope::Top,
            prefix: prefix.to_vec(),
        };
        for prefixes in [
            vec![],
            vec![witness(b"")],
            vec![witness(b"a"), witness(b"a")],
            vec![witness(b"b"), witness(b"a")],
            vec![witness(b"a"), witness(b"aa")],
            vec![witness(&vec![1; MAX_STORAGE_KEY_BYTES + 1])],
            vec![witness(b"a"); MAX_EDITS_PER_CHUNK + 1],
        ] {
            assert!(RecoveryPrefixAuditChunk { index: 0, prefixes }
                .validate()
                .is_err());
        }
        let audit = RecoveryPrefixAuditChunk {
            index: 0,
            prefixes: vec![witness(b"a"), witness(b"b")],
        };
        assert!(audit.validate().is_ok());
        assert_ne!(
            audit.commitment(),
            sp_io::hashing::blake2_256(&audit.encode())
        );
        assert_eq!(RecoveryAction::Incoming(vec![1]).encode()[0], 6);
        assert_eq!(RecoveryAction::Outgoing(vec![1]).encode()[0], 7);
        assert_eq!(RecoveryAction::PrefixAudit(audit.clone()).encode()[0], 8);
        let envelope = RecoveryInherent {
            format_version: 1,
            action: RecoveryAction::PrefixAudit(audit),
        };
        assert_eq!(
            RecoveryInherent::decode_remark(&envelope.encode_remark()).unwrap(),
            envelope
        );
    }
}
