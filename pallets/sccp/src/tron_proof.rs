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
