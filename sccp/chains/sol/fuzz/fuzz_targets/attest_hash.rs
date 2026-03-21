#![no_main]

use libfuzzer_sys::fuzz_target;
use sccp_sol::{attest_hash, burn_message_id, SCCP_MSG_PREFIX_ATTEST_V1, SCCP_MSG_PREFIX_BURN_V1};

fuzz_target!(|data: &[u8]| {
    let mut message_id = [0u8; 32];
    let copy = core::cmp::min(data.len(), message_id.len());
    message_id[..copy].copy_from_slice(&data[..copy]);

    let attest = attest_hash(&message_id);
    let burn_of_msg = burn_message_id(&message_id);

    let _ = attest != burn_of_msg;
    let _ = SCCP_MSG_PREFIX_ATTEST_V1 != SCCP_MSG_PREFIX_BURN_V1;
});
