#![no_main]

use libfuzzer_sys::fuzz_target;
use sccp::evm_proof::{
    decode_compact_path, evm_account_storage_root, keccak, mpt_get, rlp_decode,
    rlp_decode_bytes_payload,
};

fuzz_target!(|data: &[u8]| {
    let _ = rlp_decode(data);
    let _ = decode_compact_path(data);
    let _ = evm_account_storage_root(data);
    let _ = rlp_decode_bytes_payload(data);

    let mut key = [0u8; 32];
    let to_copy = core::cmp::min(data.len(), key.len());
    key[..to_copy].copy_from_slice(&data[..to_copy]);

    let mut proof = Vec::new();
    if !data.is_empty() {
        let chunks = usize::from(data[0] % 8).saturating_add(1);
        let mut cursor = 1usize;
        for _ in 0..chunks {
            if cursor >= data.len() {
                break;
            }
            let remaining = data.len().saturating_sub(cursor);
            let raw_len = usize::from(data[cursor]);
            cursor = cursor.saturating_add(1);
            let take = raw_len.min(remaining);
            let end = cursor.saturating_add(take).min(data.len());
            proof.push(data[cursor..end].to_vec());
            cursor = end;
        }
    }

    let root = keccak(data);
    let _ = mpt_get(root, &key, &proof);
});
