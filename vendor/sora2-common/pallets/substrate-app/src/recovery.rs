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
