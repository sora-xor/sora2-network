// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

//! Minimal TRON block-header helpers.
//!
//! This module implements:
//! - protobuf parsing for `BlockHeader.raw_data` (only fields SCCP needs)
//! - block-id derivation (TRON "blockID")
//! - witness signature recovery (secp256k1)
//!
//! It is intentionally tiny and fail-closed on any malformed input.

use sp_core::{H160, H256};
use sp_io::hashing::{keccak_256, sha2_256};
use sp_std::prelude::*;

/// Parsed subset of TRON `BlockHeader.raw_data` (protobuf).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TronHeaderRaw {
    pub parent_hash: H256,
    pub number: u64,
    /// TRON address bytes (typically `0x41 || eth_address20` on mainnet).
    pub witness_address: [u8; 21],
    /// Optional in TRON, but required for SCCP EVM MPT proofs.
    pub account_state_root: H256,
}

fn read_varint(input: &[u8], idx: &mut usize) -> Option<u64> {
    let mut shift = 0u32;
    let mut out = 0u64;
    for _ in 0..10 {
        let b = *input.get(*idx)?;
        *idx = idx.checked_add(1)?;
        out |= ((b & 0x7f) as u64).checked_shl(shift)?;
        if (b & 0x80) == 0 {
            return Some(out);
        }
        shift = shift.checked_add(7)?;
    }
    None
}

fn skip_field(input: &[u8], idx: &mut usize, wire: u8) -> Option<()> {
    match wire {
        0 => {
            let _ = read_varint(input, idx)?;
            Some(())
        }
        1 => {
            *idx = idx.checked_add(8)?;
            if *idx > input.len() {
                return None;
            }
            Some(())
        }
        2 => {
            let len = read_varint(input, idx)? as usize;
            *idx = idx.checked_add(len)?;
            if *idx > input.len() {
                return None;
            }
            Some(())
        }
        5 => {
            *idx = idx.checked_add(4)?;
            if *idx > input.len() {
                return None;
            }
            Some(())
        }
        _ => None,
    }
}

/// Parse TRON `BlockHeader.raw_data` protobuf bytes.
///
/// Field numbers are aligned with TRON protocol structs:
/// - parentHash: 3 (bytes)
/// - number: 7 (varint)
/// - witness_address: 9 (bytes)
/// - accountStateRoot: 11 (bytes)
pub fn parse_tron_header_raw(raw: &[u8]) -> Option<TronHeaderRaw> {
    let mut idx: usize = 0;
    let mut parent_hash: Option<H256> = None;
    let mut number: Option<u64> = None;
    let mut witness_address: Option<[u8; 21]> = None;
    let mut account_state_root: Option<H256> = None;

    while idx < raw.len() {
        let key = read_varint(raw, &mut idx)?;
        let field = (key >> 3) as u32;
        let wire = (key & 0x7) as u8;

        match (field, wire) {
            (3, 2) => {
                let len = read_varint(raw, &mut idx)? as usize;
                let bytes = raw.get(idx..idx.checked_add(len)?)?;
                idx = idx.checked_add(len)?;
                if bytes.len() != 32 {
                    return None;
                }
                parent_hash = Some(H256::from_slice(bytes));
            }
            (7, 0) => {
                let v = read_varint(raw, &mut idx)?;
                number = Some(v);
            }
            (9, 2) => {
                let len = read_varint(raw, &mut idx)? as usize;
                let bytes = raw.get(idx..idx.checked_add(len)?)?;
                idx = idx.checked_add(len)?;
                if bytes.len() != 21 {
                    return None;
                }
                let mut out = [0u8; 21];
                out.copy_from_slice(bytes);
                witness_address = Some(out);
            }
            (11, 2) => {
                let len = read_varint(raw, &mut idx)? as usize;
                let bytes = raw.get(idx..idx.checked_add(len)?)?;
                idx = idx.checked_add(len)?;
                if bytes.len() != 32 {
                    return None;
                }
                account_state_root = Some(H256::from_slice(bytes));
            }
            // Unknown field: skip.
            (_, w) => {
                skip_field(raw, &mut idx, w)?;
            }
        }
    }

    Some(TronHeaderRaw {
        parent_hash: parent_hash?,
        number: number?,
        witness_address: witness_address?,
        account_state_root: account_state_root?,
    })
}

/// TRON block raw_data hash used for witness signature verification.
pub fn raw_data_hash(raw: &[u8]) -> [u8; 32] {
    sha2_256(raw)
}

/// Derive TRON "blockID" (32 bytes) from a header raw_data hash and `number`.
///
/// TRON block IDs include the block number in the first 8 bytes (big-endian).
pub fn block_id_from_raw_hash(number: u64, raw_hash: &[u8; 32]) -> H256 {
    let mut out = *raw_hash;
    out[..8].copy_from_slice(&number.to_be_bytes());
    H256(out)
}

fn is_low_s(s: &[u8; 32], half_order: &[u8; 32]) -> bool {
    // Big-endian compare: s <= half_order
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

/// Recover the Ethereum-style address (H160) from a recoverable secp256k1 signature.
///
/// `sig65` must be `r(32) || s(32) || v(1)` where `v` is either `0/1` or `27/28` or `0..3`.
pub fn recover_eth_address_from_sig(
    msg_hash32: &[u8; 32],
    sig65: &[u8; 65],
    secp256k1n_half_order: &[u8; 32],
) -> Option<H160> {
    let mut sig = *sig65;

    // Normalize `v` to {0..3} if encoded as 27/28.
    if sig[64] >= 27 {
        sig[64] = sig[64].checked_sub(27)?;
    }
    if sig[64] > 3 {
        return None;
    }

    // Reject invalid/malleable ECDSA signatures.
    let r_bytes = &sig[0..32];
    let s_bytes = &sig[32..64];
    if !r_bytes.iter().any(|&b| b != 0) {
        return None;
    }
    if !s_bytes.iter().any(|&b| b != 0) {
        return None;
    }
    let mut s32 = [0u8; 32];
    s32.copy_from_slice(s_bytes);
    if !is_low_s(&s32, secp256k1n_half_order) {
        return None;
    }

    let pk = sp_io::crypto::secp256k1_ecdsa_recover(&sig, msg_hash32).ok()?;
    Some(H160::from_slice(&keccak_256(&pk)[12..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::collection::vec;
    use proptest::prelude::*;

    const SECP256K1N_HALF_ORDER: [u8; 32] = [
        0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b,
        0x20, 0xa0,
    ];

    fn encode_varint(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                break;
            }
        }
        out
    }

    fn append_varint_field(out: &mut Vec<u8>, field: u32, value: u64) {
        let key = ((field as u64) << 3) | 0;
        out.extend_from_slice(&encode_varint(key));
        out.extend_from_slice(&encode_varint(value));
    }

    fn append_bytes_field(out: &mut Vec<u8>, field: u32, value: &[u8]) {
        let key = ((field as u64) << 3) | 2;
        out.extend_from_slice(&encode_varint(key));
        out.extend_from_slice(&encode_varint(value.len() as u64));
        out.extend_from_slice(value);
    }

    fn append_fixed64_field(out: &mut Vec<u8>, field: u32, value: &[u8; 8]) {
        let key = ((field as u64) << 3) | 1;
        out.extend_from_slice(&encode_varint(key));
        out.extend_from_slice(value);
    }

    fn append_fixed32_field(out: &mut Vec<u8>, field: u32, value: &[u8; 4]) {
        let key = ((field as u64) << 3) | 5;
        out.extend_from_slice(&encode_varint(key));
        out.extend_from_slice(value);
    }

    fn sample_raw_header() -> Vec<u8> {
        let mut raw = Vec::new();
        append_bytes_field(&mut raw, 3, &[0x11u8; 32]);
        append_varint_field(&mut raw, 7, 42);
        let mut witness = [0u8; 21];
        witness[0] = 0x41;
        witness[20] = 0xaa;
        append_bytes_field(&mut raw, 9, &witness);
        append_bytes_field(&mut raw, 11, &[0x22u8; 32]);
        raw
    }

    #[test]
    fn parse_tron_header_raw_accepts_valid_minimal_header() {
        let raw = sample_raw_header();
        let parsed = parse_tron_header_raw(&raw).expect("header should parse");
        assert_eq!(parsed.parent_hash, H256([0x11u8; 32]));
        assert_eq!(parsed.number, 42);
        assert_eq!(parsed.witness_address[0], 0x41);
        assert_eq!(parsed.witness_address[20], 0xaa);
        assert_eq!(parsed.account_state_root, H256([0x22u8; 32]));
    }

    #[test]
    fn parse_tron_header_raw_rejects_missing_required_field() {
        let mut raw = Vec::new();
        append_bytes_field(&mut raw, 3, &[0x11u8; 32]);
        append_varint_field(&mut raw, 7, 7);
        append_bytes_field(&mut raw, 9, &[0x41u8; 21]);
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn parse_tron_header_raw_rejects_invalid_witness_length() {
        let mut raw = Vec::new();
        append_bytes_field(&mut raw, 3, &[0x11u8; 32]);
        append_varint_field(&mut raw, 7, 7);
        append_bytes_field(&mut raw, 9, &[0x41u8; 20]);
        append_bytes_field(&mut raw, 11, &[0x22u8; 32]);
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn parse_tron_header_raw_rejects_invalid_parent_hash_length() {
        let mut raw = Vec::new();
        append_bytes_field(&mut raw, 3, &[0x11u8; 31]);
        append_varint_field(&mut raw, 7, 7);
        append_bytes_field(&mut raw, 9, &[0x41u8; 21]);
        append_bytes_field(&mut raw, 11, &[0x22u8; 32]);
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn parse_tron_header_raw_rejects_invalid_account_state_root_length() {
        let mut raw = Vec::new();
        append_bytes_field(&mut raw, 3, &[0x11u8; 32]);
        append_varint_field(&mut raw, 7, 7);
        append_bytes_field(&mut raw, 9, &[0x41u8; 21]);
        append_bytes_field(&mut raw, 11, &[0x22u8; 31]);
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn parse_tron_header_raw_rejects_unknown_invalid_wire_type() {
        // field=3, wire=7 is invalid and must fail closed.
        let raw = vec![((3u32 << 3) | 7) as u8];
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn parse_tron_header_raw_rejects_truncated_length_delimited_field() {
        let mut raw = Vec::new();
        // field=3 (parentHash), wire=2 (bytes), declared len=32 but only 31 bytes follow.
        raw.push(((3u32 << 3) | 2) as u8);
        raw.push(32u8);
        raw.extend_from_slice(&[0x11u8; 31]);
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn parse_tron_header_raw_rejects_truncated_varint_field() {
        let raw = vec![((7u32 << 3) | 0) as u8, 0x80];
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn read_varint_rejects_overlong_sequence() {
        let input = [0x80u8; 11];
        let mut idx = 0usize;
        assert!(
            read_varint(&input, &mut idx).is_none(),
            "varint with >10 continuation bytes must fail closed"
        );
    }

    #[test]
    fn read_varint_accepts_max_u64_encoding() {
        let input = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01];
        let mut idx = 0usize;
        let value = read_varint(&input, &mut idx).expect("max u64 varint should decode");
        assert_eq!(value, u64::MAX);
        assert_eq!(idx, input.len());
    }

    #[test]
    fn parse_tron_header_raw_rejects_unknown_field_with_unsupported_wire_type() {
        // field=19, wire=3 is unsupported by skip_field and must fail closed.
        let raw = vec![((19u32 << 3) | 3) as u8];
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn parse_tron_header_raw_skips_unknown_fixed_width_fields() {
        let mut raw = sample_raw_header();
        // field=19, wire=1 (fixed64), 8 bytes payload
        append_fixed64_field(&mut raw, 19, &[0xabu8; 8]);
        // field=20, wire=5 (fixed32), 4 bytes payload
        append_fixed32_field(&mut raw, 20, &[0xcdu8; 4]);

        assert!(
            parse_tron_header_raw(&raw).is_some(),
            "unknown fixed-width fields with valid payload should be skipped"
        );
    }

    #[test]
    fn parse_tron_header_raw_rejects_truncated_unknown_fixed64_field() {
        let mut raw = sample_raw_header();
        // field=19, wire=1 requires 8 bytes; provide only 7 to force fail-closed.
        raw.extend_from_slice(&encode_varint(((19u32 as u64) << 3) | 1));
        raw.extend_from_slice(&[0x11u8; 7]);
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn parse_tron_header_raw_rejects_truncated_unknown_fixed32_field() {
        let mut raw = sample_raw_header();
        // field=20, wire=5 requires 4 bytes; provide only 2 to force fail-closed.
        raw.extend_from_slice(&encode_varint(((20u32 as u64) << 3) | 5));
        raw.extend_from_slice(&[0x22u8; 2]);
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn parse_tron_header_raw_rejects_truncated_unknown_length_delimited_field() {
        let mut raw = sample_raw_header();
        // field=21, wire=2, declared length=4 but only 2 bytes follow.
        raw.extend_from_slice(&encode_varint(((21u32 as u64) << 3) | 2));
        raw.extend_from_slice(&encode_varint(4));
        raw.extend_from_slice(&[0x33u8; 2]);
        assert!(parse_tron_header_raw(&raw).is_none());
    }

    #[test]
    fn raw_data_hash_is_deterministic() {
        let raw = sample_raw_header();
        let a = raw_data_hash(&raw);
        let b = raw_data_hash(&raw);
        assert_eq!(a, b, "same input must produce same hash");
    }

    #[test]
    fn parse_tron_header_raw_skips_unknown_supported_wire_types() {
        let mut raw = sample_raw_header();
        append_varint_field(&mut raw, 19, 999);
        assert!(parse_tron_header_raw(&raw).is_some());
    }

    #[test]
    fn parse_tron_header_raw_skips_unknown_length_delimited_fields() {
        let mut raw = sample_raw_header();
        append_bytes_field(&mut raw, 22, &[0x44u8, 0x55u8, 0x66u8]);
        assert!(
            parse_tron_header_raw(&raw).is_some(),
            "unknown wire=2 fields with valid length should be skipped"
        );
    }

    #[test]
    fn parse_tron_header_raw_last_repeated_number_wins() {
        let mut raw = sample_raw_header();
        append_varint_field(&mut raw, 7, 99);
        let parsed = parse_tron_header_raw(&raw).expect("header should parse");
        assert_eq!(parsed.number, 99);
    }

    #[test]
    fn parse_tron_header_raw_last_repeated_parent_hash_wins() {
        let mut raw = sample_raw_header();
        append_bytes_field(&mut raw, 3, &[0x99u8; 32]);
        let parsed = parse_tron_header_raw(&raw).expect("header should parse");
        assert_eq!(parsed.parent_hash, H256([0x99u8; 32]));
    }

    #[test]
    fn parse_tron_header_raw_last_repeated_witness_and_state_root_win() {
        let mut raw = sample_raw_header();
        append_bytes_field(&mut raw, 9, &[0x55u8; 21]);
        append_bytes_field(&mut raw, 11, &[0x77u8; 32]);
        let parsed = parse_tron_header_raw(&raw).expect("header should parse");
        assert_eq!(parsed.witness_address, [0x55u8; 21]);
        assert_eq!(parsed.account_state_root, H256([0x77u8; 32]));
    }

    #[test]
    fn block_id_from_raw_hash_prefixes_block_number() {
        let raw_hash = [0xabu8; 32];
        let number = 0x0102_0304_0506_0708u64;
        let block_id = block_id_from_raw_hash(number, &raw_hash);
        let bytes = block_id.as_bytes();
        assert_eq!(&bytes[..8], &number.to_be_bytes());
        assert_eq!(&bytes[8..], &raw_hash[8..]);
    }

    #[test]
    fn recover_eth_address_from_sig_rejects_invalid_or_malleable_signatures() {
        let msg_hash = [0x55u8; 32];

        let mut zero_r = [0u8; 65];
        zero_r[32] = 1;
        assert!(recover_eth_address_from_sig(&msg_hash, &zero_r, &SECP256K1N_HALF_ORDER).is_none());

        let mut high_s = [0u8; 65];
        high_s[0] = 1;
        high_s[32..64].fill(0xff);
        high_s[64] = 0;
        assert!(recover_eth_address_from_sig(&msg_hash, &high_s, &SECP256K1N_HALF_ORDER).is_none());

        let mut bad_v = [0u8; 65];
        bad_v[0] = 1;
        bad_v[32] = 1;
        bad_v[64] = 9;
        assert!(recover_eth_address_from_sig(&msg_hash, &bad_v, &SECP256K1N_HALF_ORDER).is_none());

        let mut zero_s = [0u8; 65];
        zero_s[0] = 1;
        zero_s[64] = 0;
        assert!(
            recover_eth_address_from_sig(&msg_hash, &zero_s, &SECP256K1N_HALF_ORDER).is_none(),
            "zero-s signatures must be rejected"
        );

        let mut v27 = [0u8; 65];
        v27[32] = 1;
        v27[64] = 27;
        assert!(
            recover_eth_address_from_sig(&msg_hash, &v27, &SECP256K1N_HALF_ORDER).is_none(),
            "v=27 should normalize but still fail for invalid signature body"
        );
    }

    #[test]
    fn is_low_s_accepts_boundary_and_rejects_above() {
        assert!(is_low_s(&SECP256K1N_HALF_ORDER, &SECP256K1N_HALF_ORDER));

        let mut below = SECP256K1N_HALF_ORDER;
        below[31] = below[31].saturating_sub(1);
        assert!(is_low_s(&below, &SECP256K1N_HALF_ORDER));

        let mut above = SECP256K1N_HALF_ORDER;
        above[31] = above[31].saturating_add(1);
        assert!(!is_low_s(&above, &SECP256K1N_HALF_ORDER));
    }

    #[test]
    fn tron_proof_helpers_fail_closed_on_fuzzed_inputs() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for len in 0..128usize {
            let mut buf = vec![0u8; len];
            for byte in &mut buf {
                seed ^= seed >> 12;
                seed ^= seed << 25;
                seed ^= seed >> 27;
                let mixed = seed.wrapping_mul(0x2545_f491_4f6c_dd1d);
                *byte = (mixed & 0xff) as u8;
            }

            let _ = parse_tron_header_raw(&buf);
            let hash = raw_data_hash(&buf);
            let _ = block_id_from_raw_hash(123, &hash);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn tron_proof_helpers_property_no_panic_on_arbitrary_bytes(
            raw in vec(any::<u8>(), 0..512),
            msg in vec(any::<u8>(), 32),
            sig in vec(any::<u8>(), 65),
            number in any::<u64>(),
        ) {
            let _ = parse_tron_header_raw(&raw);
            let hash = raw_data_hash(&raw);
            let _ = block_id_from_raw_hash(number, &hash);

            let mut msg_hash = [0u8; 32];
            msg_hash.copy_from_slice(&msg);
            let mut sig65 = [0u8; 65];
            sig65.copy_from_slice(&sig);
            let _ = recover_eth_address_from_sig(&msg_hash, &sig65, &SECP256K1N_HALF_ORDER);
        }
    }
}
