#![no_main]

use libfuzzer_sys::fuzz_target;
use sccp::evm_proof::{
    parse_bsc_header_minimal, rlp_decode, rlp_encode_bytes, rlp_encode_list, RlpItem,
};

fuzz_target!(|data: &[u8]| {
    let parsed = parse_bsc_header_minimal(data);
    if let Some(h) = parsed {
        let mut encoded = Vec::with_capacity(15);
        encoded.push(rlp_encode_bytes(h.parent_hash));
        encoded.push(rlp_encode_bytes(&[]));
        encoded.push(rlp_encode_bytes(h.beneficiary));
        encoded.push(rlp_encode_bytes(h.state_root));
        encoded.push(rlp_encode_bytes(&[]));
        encoded.push(rlp_encode_bytes(&[]));
        encoded.push(rlp_encode_bytes(&[]));
        encoded.push(rlp_encode_bytes(&h.difficulty.to_be_bytes()));
        encoded.push(rlp_encode_bytes(&h.number.to_be_bytes()));
        encoded.push(rlp_encode_bytes(&[]));
        encoded.push(rlp_encode_bytes(&[]));
        encoded.push(rlp_encode_bytes(&[]));
        let mut extra = Vec::with_capacity(h.extra_data_no_sig.len() + h.signature.len());
        extra.extend_from_slice(h.extra_data_no_sig);
        extra.extend_from_slice(&h.signature);
        encoded.push(rlp_encode_bytes(&extra));
        encoded.push(rlp_encode_bytes(&[]));
        encoded.push(rlp_encode_bytes(&[]));
        let _ = rlp_decode(&rlp_encode_list(&encoded));
    } else {
        if let Some(RlpItem::List(items)) = rlp_decode(data) {
            for item in items {
                if let RlpItem::Bytes(b) = item {
                    let _ = rlp_encode_bytes(b);
                }
            }
        }
    }
});
