#![no_main]

use libfuzzer_sys::fuzz_target;
use sccp_sol::{attest_hash, burn_message_id, decode_burn_payload_v1, BurnPayloadV1};

fuzz_target!(|data: &[u8]| {
    let decoded = decode_burn_payload_v1(data);
    if data.len() != BurnPayloadV1::ENCODED_LEN {
        assert!(decoded.is_err());
        return;
    }

    let decoded = decoded.expect("exact-length payload must decode");
    let encoded = decoded.encode_scale();
    assert_eq!(encoded.len(), BurnPayloadV1::ENCODED_LEN);

    let roundtrip = decode_burn_payload_v1(&encoded).expect("re-encoded payload must decode");
    assert_eq!(roundtrip, decoded);

    let message_id = burn_message_id(&encoded);
    let attest_a = attest_hash(&message_id);
    let attest_b = attest_hash(&message_id);
    assert_eq!(attest_a, attest_b);
});
