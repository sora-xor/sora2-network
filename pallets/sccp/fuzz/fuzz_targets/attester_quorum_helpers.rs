#![no_main]

use libfuzzer_sys::fuzz_target;
use sccp::{decode_attester_quorum_proof_for_fuzz, SCCP_MSG_PREFIX_ATTEST_V1, SECP256K1N_HALF_ORDER};

fn is_low_s(s: &[u8; 32], half_order: &[u8; 32]) -> bool {
    for i in 0..32 {
        if s[i] < half_order[i] {
            return true;
        }
        if s[i] > half_order[i] {
            return false;
        }
    }
    true
}

fuzz_target!(|data: &[u8]| {
    let _ = SCCP_MSG_PREFIX_ATTEST_V1;

    let sigs = decode_attester_quorum_proof_for_fuzz(data, 256);
    if let Some(sigs) = sigs {
        for mut sig in sigs {
            if sig[64] >= 27 {
                sig[64] = sig[64].saturating_sub(27);
            }
            let mut s = [0u8; 32];
            s.copy_from_slice(&sig[32..64]);
            let _ = is_low_s(&s, &SECP256K1N_HALF_ORDER);
            let _ = sig[64] <= 3;
        }
    }
});
