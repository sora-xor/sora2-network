//! Persistent SORA Nexus bridge maintenance latch. No call can clear it here.

pub(crate) const HALT_KEY: &[u8] = b":sora-nexus:recovery:bridge-halt:v1";
pub(crate) const INBOUND_CONTEXT_KEY: &[u8] = b":sora-nexus:recovery:bridge-inbound-context:v1";

pub(crate) fn halted() -> bool {
    sp_io::storage::exists(HALT_KEY)
}

/// The recovery executive sets this exact SCALE u32 only around a committed
/// authenticated inbound operation, and clears it transactionally before return.
pub(crate) fn inbound_allowed<T: frame_system::Config>() -> bool {
    if !halted() {
        return true;
    }
    let number: Result<u32, _> = frame_system::Pallet::<T>::block_number().try_into();
    number.ok().is_some_and(|number| {
        sp_io::storage::get(INBOUND_CONTEXT_KEY)
            .is_some_and(|bytes| bytes.as_ref() == number.to_le_bytes().as_slice())
    })
}
