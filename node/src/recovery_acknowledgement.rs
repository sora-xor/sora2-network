//! Explicit, local operator acknowledgement of completed SORA Nexus recovery.
//!
//! Completion makes ordinary bridge calls available again, but does not turn a
//! node's signing workers on. Only an explicit digest-valued startup option may
//! record this node's acknowledgement. Guardian halts remain a separate gate.

use crate::service::{FullBackend, FullClient};
use bridge_types::recovery::{
    acknowledgement_matches, completion_with, RecoveryAcknowledgement, RecoveryCompletion,
    RECOVERY_ACK_STORAGE_KEY,
};
use codec::Encode;
use sc_client_api::{Backend, StorageProvider};
use sp_blockchain::HeaderBackend;
use sp_core::{offchain::OffchainStorage, storage::StorageKey, H256};
use sp_runtime::offchain::STORAGE_PREFIX;

type CompletionState = Result<Option<RecoveryCompletion>, String>;

/// Read every completion field from one fixed state. A storage error must not
/// masquerade as an absent optional key in the shared completion decoder.
pub(crate) fn completion_at(client: &FullClient, hash: H256) -> CompletionState {
    let mut read_failed = false;
    let result = completion_with(
        |key| match client.storage(hash, &StorageKey(key.to_vec())) {
            Ok(value) => value.map(|value| value.0),
            Err(_) => {
                read_failed = true;
                None
            }
        },
    );
    if read_failed {
        return Err("cannot read completed recovery context".into());
    }
    result.map_err(|_| "recovery is unfinished or its completion context is invalid".into())
}

fn confirm(
    requested: H256,
    genesis: H256,
    best: &CompletionState,
    finalized: &CompletionState,
) -> Result<RecoveryAcknowledgement, String> {
    let best = best.as_ref().map_err(Clone::clone)?;
    let finalized = finalized.as_ref().map_err(Clone::clone)?;
    let (Some(best), Some(finalized)) = (best, finalized) else {
        return Err("both best and finalized state must contain completed recovery".into());
    };
    let digest = finalized.context_hash(genesis.0);
    if best.context_hash(genesis.0) != digest {
        return Err("best and finalized recovery contexts differ; wait for finality".into());
    }
    if requested.0 != digest {
        return Err(format!(
            "requested recovery context does not match the finalized context {:#x}",
            H256(digest)
        ));
    }
    Ok(RecoveryAcknowledgement::new(genesis.0, finalized))
}

fn apply_requested(
    storage: &mut impl OffchainStorage,
    requested: Option<H256>,
    genesis: H256,
    best: &CompletionState,
    finalized: &CompletionState,
) -> Result<Option<RecoveryAcknowledgement>, String> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    let acknowledgement = confirm(requested, genesis, best, finalized)?;
    let encoded = acknowledgement.encode();
    storage.set(STORAGE_PREFIX, RECOVERY_ACK_STORAGE_KEY, &encoded);
    if storage
        .get(STORAGE_PREFIX, RECOVERY_ACK_STORAGE_KEY)
        .as_deref()
        != Some(encoded.as_slice())
    {
        return Err("local recovery acknowledgement write could not be verified".into());
    }
    Ok(Some(acknowledgement))
}

/// The pool has no execution context; an OCW does. A worker cannot cross a
/// recovery-context change even if its old acknowledgement was once valid.
pub(crate) fn workers_may_run(
    genesis: H256,
    best: &CompletionState,
    execution: Option<&CompletionState>,
    acknowledgement: Option<&[u8]>,
) -> bool {
    let Ok(best) = best else {
        return false;
    };
    if let Some(execution) = execution {
        let Ok(execution) = execution else {
            return false;
        };
        if execution
            .as_ref()
            .map(|value| value.context_hash(genesis.0))
            != best.as_ref().map(|value| value.context_hash(genesis.0))
        {
            return false;
        }
    }
    match best {
        None => true,
        Some(completion) => acknowledgement_matches(genesis.0, completion, acknowledgement),
    }
}

/// Called before any worker is constructed. This writes only the local node's
/// offchain acknowledgement and never submits a transaction or changes flags.
pub(crate) fn on_startup(client: &FullClient, backend: &FullBackend, requested: Option<H256>) {
    let Some(mut storage) = backend.offchain_storage() else {
        if requested.is_some() {
            log::error!("SORA Nexus recovery acknowledgement refused: no local offchain storage");
        }
        return;
    };
    let info = client.info();
    let best = completion_at(client, info.best_hash);
    let finalized = completion_at(client, info.finalized_hash);
    match apply_requested(&mut storage, requested, info.genesis_hash, &best, &finalized) {
        Ok(Some(_)) => log::warn!(
            "SORA Nexus completed recovery context {:#x} acknowledged locally; configured OCW mode, guardian halt and request quarantine still apply",
            requested.expect("an acknowledgement requires an explicit digest")
        ),
        Err(reason) => log::error!(
            "SORA Nexus recovery acknowledgement not verified: {reason}; HASHI access requires the exact stored recovery context"
        ),
        Ok(None) => {}
    }
    let acknowledged = storage.get(STORAGE_PREFIX, RECOVERY_ACK_STORAGE_KEY);
    if !workers_may_run(info.genesis_hash, &best, None, acknowledged.as_deref()) {
        match best {
            Ok(Some(completion)) => log::warn!(
                "SORA Nexus HASHI worker remains stopped after recovery. When reviewed, restart this node with --bridge-resume-after-recovery {:#x}; guardian acknowledgement is separate",
                H256(completion.context_hash(info.genesis_hash.0))
            ),
            Err(reason) => log::warn!("SORA Nexus HASHI worker remains stopped: {reason}"),
            Ok(None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::bridge_halt::{workers_may_run as guardian_open, GuardianHalt};
    use sp_core::offchain::storage::InMemOffchainStorage;

    fn completion() -> RecoveryCompletion {
        RecoveryCompletion {
            policy: [1; 32],
            active_policy: [2; 32],
            recovery_anchor: [3; 32],
            terminal_audit_hash: [4; 32],
            operation_counter: 5,
            completed_chunks: 6,
        }
    }

    #[test]
    fn recovery_acknowledgement_requires_explicit_exact_finalized_digest() {
        let genesis = H256::repeat_byte(7);
        let completion = completion();
        let state = Ok(Some(completion.clone()));
        let digest = H256(completion.context_hash(genesis.0));
        let mut storage = InMemOffchainStorage::default();
        assert!(apply_requested(&mut storage, None, genesis, &state, &state)
            .unwrap()
            .is_none());
        assert!(storage
            .get(STORAGE_PREFIX, RECOVERY_ACK_STORAGE_KEY)
            .is_none());
        assert!(!workers_may_run(genesis, &state, Some(&state), None));
        assert!(
            apply_requested(&mut storage, Some(H256::zero()), genesis, &state, &state).is_err()
        );
        assert!(storage
            .get(STORAGE_PREFIX, RECOVERY_ACK_STORAGE_KEY)
            .is_none());
        assert!(
            apply_requested(&mut storage, Some(digest), genesis, &state, &state)
                .unwrap()
                .is_some()
        );
        let saved = storage
            .get(STORAGE_PREFIX, RECOVERY_ACK_STORAGE_KEY)
            .unwrap();
        assert!(workers_may_run(genesis, &state, Some(&state), Some(&saved)));
        // An ordinary daemon restart never writes or refreshes an acknowledgement.
        assert!(apply_requested(&mut storage, None, genesis, &state, &state)
            .unwrap()
            .is_none());
        assert_eq!(
            storage.get(STORAGE_PREFIX, RECOVERY_ACK_STORAGE_KEY),
            Some(saved)
        );
    }

    #[derive(Clone, Default)]
    struct FailedWrites {
        inner: InMemOffchainStorage,
        corrupt: bool,
    }

    impl OffchainStorage for FailedWrites {
        fn get(&self, prefix: &[u8], key: &[u8]) -> Option<Vec<u8>> {
            self.inner.get(prefix, key)
        }
        fn set(&mut self, prefix: &[u8], key: &[u8], _: &[u8]) {
            if self.corrupt {
                self.inner.set(prefix, key, b"corrupt");
            }
        }
        fn remove(&mut self, prefix: &[u8], key: &[u8]) {
            self.inner.remove(prefix, key);
        }
        fn compare_and_set(&mut self, _: &[u8], _: &[u8], _: Option<&[u8]>, _: &[u8]) -> bool {
            false
        }
    }

    #[test]
    fn recovery_acknowledgement_failed_write_cannot_report_success_or_open_new_context() {
        let genesis = H256::repeat_byte(7);
        let previous = completion();
        let prior_ack = RecoveryAcknowledgement::new(genesis.0, &previous).encode();
        let mut current = previous;
        current.operation_counter += 1;
        let digest = H256(current.context_hash(genesis.0));
        let state = Ok(Some(current));
        for corrupt in [false, true] {
            for retain_previous in [false, true] {
                let mut storage = FailedWrites {
                    corrupt,
                    ..Default::default()
                };
                if retain_previous {
                    storage
                        .inner
                        .set(STORAGE_PREFIX, RECOVERY_ACK_STORAGE_KEY, &prior_ack);
                }
                assert_eq!(
                    apply_requested(&mut storage, Some(digest), genesis, &state, &state),
                    Err("local recovery acknowledgement write could not be verified".into())
                );
                let stored = storage.get(STORAGE_PREFIX, RECOVERY_ACK_STORAGE_KEY);
                assert!(!workers_may_run(
                    genesis,
                    &state,
                    Some(&state),
                    stored.as_deref()
                ));
                if !corrupt {
                    assert_eq!(stored, retain_previous.then(|| prior_ack.clone()));
                }
            }
        }
    }

    #[test]
    fn recovery_acknowledgement_never_confirms_missing_unfinished_or_unfinalized_context() {
        let genesis = H256::repeat_byte(7);
        let completion = completion();
        let complete = Ok(Some(completion.clone()));
        let digest = H256(completion.context_hash(genesis.0));
        let mut changed = completion;
        changed.operation_counter += 1;
        for other in [Ok(None), Err("unfinished".into()), Ok(Some(changed))] {
            assert!(confirm(digest, genesis, &complete, &other).is_err());
            assert!(confirm(digest, genesis, &other, &complete).is_err());
        }
    }

    #[test]
    fn recovery_acknowledgement_binds_every_context_field_and_genesis() {
        let genesis = H256::repeat_byte(7);
        let complete = completion();
        let acknowledgement = RecoveryAcknowledgement::new(genesis.0, &complete).encode();
        let mut changes = vec![];
        for field in 0..6 {
            let mut changed = complete.clone();
            match field {
                0 => changed.policy[0] ^= 1,
                1 => changed.active_policy[0] ^= 1,
                2 => changed.recovery_anchor[0] ^= 1,
                3 => changed.terminal_audit_hash[0] ^= 1,
                4 => changed.operation_counter += 1,
                _ => changed.completed_chunks += 1,
            }
            changes.push(changed);
        }
        for changed in changes {
            let state = Ok(Some(changed));
            assert!(!workers_may_run(
                genesis,
                &state,
                Some(&state),
                Some(&acknowledgement)
            ));
        }
        let state = Ok(Some(complete));
        assert!(!workers_may_run(
            H256::repeat_byte(8),
            &state,
            Some(&state),
            Some(&acknowledgement)
        ));
        let mut trailing = acknowledgement.clone();
        trailing.push(0);
        for malformed in [vec![], vec![1], trailing] {
            assert!(!workers_may_run(genesis, &state, None, Some(&malformed)));
        }
    }

    #[test]
    fn recovery_acknowledgement_cannot_cross_worker_context_or_guardian_halt() {
        let genesis = H256::repeat_byte(7);
        let complete = completion();
        let acknowledgement = RecoveryAcknowledgement::new(genesis.0, &complete).encode();
        let state = Ok(Some(complete.clone()));
        for other in [Ok(None), Err("storage error".into())] {
            assert!(!workers_may_run(
                genesis,
                &state,
                Some(&other),
                Some(&acknowledgement)
            ));
            assert!(!workers_may_run(
                genesis,
                &other,
                Some(&state),
                Some(&acknowledgement)
            ));
        }
        let previous = GuardianHalt {
            number: 1,
            block: 10,
        };
        let fresh = GuardianHalt {
            number: 2,
            block: 20,
        };
        let enabled = workers_may_run(genesis, &state, Some(&state), Some(&acknowledgement));
        assert!(enabled && guardian_open(&Ok(Some(previous)), Some(previous)));
        assert!(!(enabled && guardian_open(&Ok(Some(fresh)), Some(previous))));
        assert!(enabled && guardian_open(&Ok(Some(fresh)), Some(fresh)));
    }
}
