//! Bridge admission during authenticated, unfinished incident recovery.
//! Completed recovery and emergency OCW signals do not veto ordinary calls.

#[cfg(test)]
pub(crate) const HALT_KEY: &[u8] = b":sora-nexus:recovery:bridge-halt:v1";
#[cfg(test)]
pub(crate) const INBOUND_CONTEXT_KEY: &[u8] = b":sora-nexus:recovery:bridge-inbound-context:v1";

pub(crate) fn in_progress() -> bool {
    bridge_types::recovery::recovery_in_progress()
}

pub(crate) fn inbound_allowed<T: frame_system::Config>() -> bool {
    if !in_progress() {
        return true;
    }
    let number: Result<u32, _> = frame_system::Pallet::<T>::block_number().try_into();
    number
        .ok()
        .is_some_and(bridge_types::recovery::maintenance_inbound_active)
}

/// Unit-test representation of actual unfinished recovery. A raw halt marker
/// alone no longer blocks consensus calls.
#[cfg(test)]
pub(crate) fn install_test_recovery() {
    sp_io::storage::set(b":sora-nexus:recovery:policy:v1", &[11; 32]);
    sp_io::storage::set(b":sora-nexus:recovery:active-policy:v1", &[11; 32]);
    sp_io::storage::set(
        b":sora-nexus:recovery:status:v1",
        &codec::Encode::encode(&(1u32, [11u8; 32], 0u8, 1u32, 0u64, 0u8)),
    );
    // The application-level unit fixture supplies bounded context presence;
    // runtime integration tests verify the complete authenticated operation.
    sp_io::storage::set(b":sora-nexus:recovery:incoming-active:v1", &[1]);
}

/// Coherent completion for ordinary post-recovery channel tests. Historical
/// halt markers remain present and cannot veto these operations.
#[cfg(test)]
pub(crate) fn install_completed_test_recovery() {
    for suffix in [
        "bridge-inbound-context",
        "incoming-active",
        "incoming-failed",
        "hashi-resolution",
    ] {
        let key = format!(":sora-nexus:recovery:{suffix}:v1");
        assert!(
            !sp_io::storage::exists(key.as_bytes()),
            "unexpected transient recovery context"
        );
    }
    for (key, value) in [
        (b":sora-nexus:recovery:policy:v1".as_slice(), vec![11; 32]),
        (
            b":sora-nexus:recovery:active-policy:v1".as_slice(),
            vec![11; 32],
        ),
        (
            b":sora-nexus:recovery:branch-anchor:v1".as_slice(),
            vec![12; 32],
        ),
        (
            b":sora-nexus:recovery:status:v1".as_slice(),
            codec::Encode::encode(&(1u32, [11u8; 32], 4u8, 1u32, 0u64, 0u8)),
        ),
        (
            b":sora-nexus:recovery:terminal-audit:v1".as_slice(),
            codec::Encode::encode(&([11u8; 32], [13u8; 32], 1u32)),
        ),
        (
            b":sora-nexus:recovery:incoming-audit-pending:v1".as_slice(),
            codec::Encode::encode(&0u32),
        ),
        (HALT_KEY, vec![1]),
    ] {
        sp_io::storage::set(key, &value);
    }
    assert!(bridge_types::recovery::completion().unwrap().is_some());
}
