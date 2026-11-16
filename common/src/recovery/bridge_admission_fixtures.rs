//! Disposable recovery fixtures, never compiled into a production runtime.

use super::*;

pub fn clear_state() {
    for key in [
        POLICY_STORAGE_KEY,
        ACTIVE_POLICY_STORAGE_KEY,
        RECOVERY_ANCHOR_STORAGE_KEY,
        STATUS_STORAGE_KEY,
        TERMINAL_AUDIT_STORAGE_KEY,
        INCOMING_AUDIT_PENDING_KEY,
    ] {
        sp_io::storage::clear(key);
    }
    for key in TRANSIENT_CONTEXT_KEYS {
        sp_io::storage::clear(key);
    }
}

pub fn incomplete(policy: [u8; 32]) {
    applying(policy, 1);
    for key in TRANSIENT_CONTEXT_KEYS {
        sp_io::storage::clear(key);
    }
}

pub fn complete(policy: [u8; 32]) {
    sp_io::storage::set(POLICY_STORAGE_KEY, &policy);
    sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &policy);
    sp_io::storage::set(RECOVERY_ANCHOR_STORAGE_KEY, &[12; 32]);
    sp_io::storage::set(
        STATUS_STORAGE_KEY,
        &RecoveryStatus {
            format_version: 1,
            policy,
            phase: RecoveryPhase::Complete,
            next_chunk: 1,
            operation_counter: 0,
            failure: None,
        }
        .encode(),
    );
    sp_io::storage::set(
        TERMINAL_AUDIT_STORAGE_KEY,
        &(policy, [7u8; 32], 1u32).encode(),
    );
    sp_io::storage::set(INCOMING_AUDIT_PENDING_KEY, &0u32.encode());
    for key in TRANSIENT_CONTEXT_KEYS {
        sp_io::storage::clear(key);
    }
}

pub fn applying(policy: [u8; 32], block: u32) {
    complete(policy);
    sp_io::storage::set(
        STATUS_STORAGE_KEY,
        &RecoveryStatus {
            format_version: 1,
            policy,
            phase: RecoveryPhase::Applying,
            next_chunk: 1,
            operation_counter: 0,
            failure: None,
        }
        .encode(),
    );
    sp_io::storage::clear(TERMINAL_AUDIT_STORAGE_KEY);
    sp_io::storage::set(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY, &block.encode());
    // Pallet unit fixtures exercise only the bounded presence gate. Real runtime
    // maintenance tests supply and decode their full authenticated ActiveOperation.
    sp_io::storage::set(b":sora-nexus:recovery:incoming-active:v1", &[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocw_only_halt_completion_matches_actual_recovery_status_codec() {
        sp_io::TestExternalities::default().execute_with(|| {
            complete([3; 32]);
            let status = storage::status().unwrap().unwrap();
            assert_eq!(status.phase, RecoveryPhase::Complete);
            assert_eq!(status.encode().len(), 50);
            assert!(bridge_types::recovery::completion().unwrap().is_some());
            for latch in [
                None,
                Some(vec![]),
                Some(vec![0]),
                Some(vec![1]),
                Some(vec![255; 17]),
            ] {
                match latch {
                    Some(value) => sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &value),
                    None => sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY),
                }
                assert!(bridge_types::recovery::completion().unwrap().is_some());
            }
            for phase in [
                RecoveryPhase::Applying,
                RecoveryPhase::Auditing,
                RecoveryPhase::Ready,
                RecoveryPhase::Hold,
                RecoveryPhase::Failed,
            ] {
                let mut changed = status.clone();
                changed.phase = phase;
                sp_io::storage::set(STATUS_STORAGE_KEY, &changed.encode());
                assert!(bridge_types::recovery::completion().is_err());
            }
            let mut failed = status;
            failed.failure = Some(RecoveryError::WrongPolicy);
            sp_io::storage::set(STATUS_STORAGE_KEY, &failed.encode());
            assert!(bridge_types::recovery::completion().is_err());
        });
    }

    #[test]
    fn ocw_only_halt_maintenance_requires_applying_context_not_latch() {
        sp_io::TestExternalities::default().execute_with(|| {
            complete([3; 32]);
            sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
            sp_io::storage::set(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY, &9u32.encode());
            assert!(!bridge_types::recovery::maintenance_inbound_active(9));
            applying([3; 32], 9);
            sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);
            assert!(bridge_types::recovery::maintenance_inbound_active(9));
            assert!(!bridge_types::recovery::maintenance_inbound_active(10));
            sp_io::storage::set(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY, &[9]);
            assert!(!bridge_types::recovery::maintenance_inbound_active(9));
        });
    }
}
