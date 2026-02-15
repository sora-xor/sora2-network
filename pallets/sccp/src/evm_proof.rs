// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

//! Minimal Ethereum-style RLP + Merkle Patricia Trie verification utilities.
//!
//! This module is intentionally tiny and "sharp edged": it implements only what SCCP needs
//! for inbound EVM storage proofs.

use sp_core::H256;
use sp_io::hashing::keccak_256;
use sp_std::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RlpItem<'a> {
    Bytes(&'a [u8]),
    List(Vec<RlpItem<'a>>),
}

pub fn keccak(bytes: &[u8]) -> H256 {
    H256::from_slice(&keccak_256(bytes))
}

pub fn rlp_encode_bytes(raw: &[u8]) -> Vec<u8> {
    // RLP for byte strings.
    //
    // - single byte < 0x80 is its own encoding
    // - otherwise prefix length (short or long) then bytes.
    if raw.len() == 1 && raw[0] <= 0x7f {
        return vec![raw[0]];
    }
    if raw.len() <= 55 {
        let mut out = Vec::with_capacity(1 + raw.len());
        out.push(0x80 + (raw.len() as u8));
        out.extend_from_slice(raw);
        return out;
    }
    let len_bytes = be_len(raw.len() as u64);
    let mut out = Vec::with_capacity(1 + len_bytes.len() + raw.len());
    out.push(0xb7 + (len_bytes.len() as u8));
    out.extend_from_slice(&len_bytes);
    out.extend_from_slice(raw);
    out
}

pub fn rlp_encode_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload_len: usize = items.iter().map(|v| v.len()).sum();
    let mut payload = Vec::with_capacity(payload_len);
    for it in items {
        payload.extend_from_slice(it.as_slice());
    }
    if payload.len() <= 55 {
        let mut out = Vec::with_capacity(1 + payload.len());
        out.push(0xc0 + (payload.len() as u8));
        out.extend_from_slice(payload.as_slice());
        return out;
    }
    let len_bytes = be_len(payload.len() as u64);
    let mut out = Vec::with_capacity(1 + len_bytes.len() + payload.len());
    out.push(0xf7 + (len_bytes.len() as u8));
    out.extend_from_slice(&len_bytes);
    out.extend_from_slice(payload.as_slice());
    out
}

pub fn rlp_decode<'a>(input: &'a [u8]) -> Option<RlpItem<'a>> {
    let (item, used) = rlp_decode_item(input).ok()?;
    if used != input.len() {
        return None;
    }
    Some(item)
}

fn rlp_decode_item<'a>(input: &'a [u8]) -> Result<(RlpItem<'a>, usize), ()> {
    if input.is_empty() {
        return Err(());
    }
    let b0 = input[0];
    match b0 {
        0x00..=0x7f => Ok((RlpItem::Bytes(&input[0..1]), 1)),
        0x80..=0xb7 => {
            let len = (b0 - 0x80) as usize;
            if input.len() < 1 + len {
                return Err(());
            }
            Ok((RlpItem::Bytes(&input[1..1 + len]), 1 + len))
        }
        0xb8..=0xbf => {
            let ll = (b0 - 0xb7) as usize;
            if input.len() < 1 + ll {
                return Err(());
            }
            let len = be_u64(&input[1..1 + ll])? as usize;
            if input.len() < 1 + ll + len {
                return Err(());
            }
            Ok((RlpItem::Bytes(&input[1 + ll..1 + ll + len]), 1 + ll + len))
        }
        0xc0..=0xf7 => {
            let len = (b0 - 0xc0) as usize;
            if input.len() < 1 + len {
                return Err(());
            }
            let payload = &input[1..1 + len];
            let items = rlp_decode_list_payload(payload)?;
            Ok((RlpItem::List(items), 1 + len))
        }
        0xf8..=0xff => {
            let ll = (b0 - 0xf7) as usize;
            if input.len() < 1 + ll {
                return Err(());
            }
            let len = be_u64(&input[1..1 + ll])? as usize;
            if input.len() < 1 + ll + len {
                return Err(());
            }
            let payload = &input[1 + ll..1 + ll + len];
            let items = rlp_decode_list_payload(payload)?;
            Ok((RlpItem::List(items), 1 + ll + len))
        }
    }
}

fn rlp_decode_list_payload<'a>(mut payload: &'a [u8]) -> Result<Vec<RlpItem<'a>>, ()> {
    let mut out = Vec::new();
    while !payload.is_empty() {
        let (item, used) = rlp_decode_item(payload)?;
        out.push(item);
        payload = payload.get(used..).ok_or(())?;
    }
    Ok(out)
}

fn be_u64(bytes: &[u8]) -> Result<u64, ()> {
    if bytes.is_empty() || bytes.len() > 8 {
        return Err(());
    }
    let mut out = 0u64;
    for &b in bytes {
        out = out.checked_shl(8).ok_or(())?;
        out |= b as u64;
    }
    Ok(out)
}

fn be_len(mut v: u64) -> Vec<u8> {
    if v == 0 {
        return vec![0u8];
    }
    let mut out = Vec::new();
    while v > 0 {
        out.push((v & 0xff) as u8);
        v >>= 8;
    }
    out.reverse();
    out
}

pub fn bytes_to_nibbles(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(b >> 4);
        out.push(b & 0x0f);
    }
    out
}

/// Decode Ethereum's hex-prefix compact path encoding used in trie leaf/extension nodes.
///
/// Returns `(is_leaf, nibbles)`.
pub fn decode_compact_path(path: &[u8]) -> Option<(bool, Vec<u8>)> {
    let first = *path.first()?;
    let flag = first >> 4;
    let low = first & 0x0f;
    let is_leaf = (flag & 0x2) != 0;
    let odd = (flag & 0x1) != 0;

    let mut nibbles = Vec::new();
    if odd {
        nibbles.push(low);
        for &b in path.get(1..)? {
            nibbles.push(b >> 4);
            nibbles.push(b & 0x0f);
        }
    } else {
        // Even length: the low nibble must be 0 padding.
        if low != 0 {
            return None;
        }
        for &b in path.get(1..)? {
            nibbles.push(b >> 4);
            nibbles.push(b & 0x0f);
        }
    }
    Some((is_leaf, nibbles))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum NodeRef {
    Hash(H256),
    Embedded(Vec<u8>),
}

fn node_ref_from_bytes(child_ref: &[u8]) -> Option<NodeRef> {
    match child_ref.len() {
        0 => None,
        1..=31 => Some(NodeRef::Embedded(child_ref.to_vec())),
        32 => Some(NodeRef::Hash(H256::from_slice(child_ref))),
        _ => None,
    }
}

/// Verify a Merkle Patricia Trie proof for a single 32-byte key (already hashed for "secure" tries).
///
/// Returns the **raw stored value bytes**, i.e. the value payload from the trie leaf/branch.
/// For Ethereum account/storage tries this value is itself an RLP encoding.
pub fn mpt_get(root: H256, key32: &[u8; 32], proof: &[Vec<u8>]) -> Option<Vec<u8>> {
    let key_nibbles = bytes_to_nibbles(key32);
    let mut pos: usize = 0;
    let mut expected = NodeRef::Hash(root);
    let mut proof_idx: usize = 0;

    loop {
        let node_bytes: &[u8] = match &expected {
            NodeRef::Hash(expect_hash) => {
                let nb = proof.get(proof_idx)?;
                proof_idx = proof_idx.checked_add(1)?;
                let h = keccak(nb);
                if &h != expect_hash {
                    return None;
                }
                nb.as_slice()
            }
            NodeRef::Embedded(bytes) => bytes.as_slice(),
        };

        let node = rlp_decode(node_bytes)?;
        let RlpItem::List(items) = node else {
            return None;
        };

        match items.len() {
            17 => {
                // Branch node: 16 children + value at index 16.
                if pos == key_nibbles.len() {
                    return match &items[16] {
                        RlpItem::Bytes(v) if !v.is_empty() => Some(v.to_vec()),
                        _ => None,
                    };
                }
                let idx = *key_nibbles.get(pos)? as usize;
                pos = pos.checked_add(1)?;
                let child = match items.get(idx)? {
                    RlpItem::Bytes(b) => *b,
                    _ => return None,
                };
                expected = node_ref_from_bytes(child)?;
            }
            2 => {
                // Leaf or extension: [compact_path, value_or_child]
                let compact = match &items[0] {
                    RlpItem::Bytes(b) => *b,
                    _ => return None,
                };
                let (is_leaf, path_nibbles) = decode_compact_path(compact)?;
                // Path must match remaining key nibbles.
                if key_nibbles.len().checked_sub(pos)? < path_nibbles.len() {
                    return None;
                }
                for (i, &n) in path_nibbles.iter().enumerate() {
                    if key_nibbles.get(pos + i)? != &n {
                        return None;
                    }
                }
                pos = pos.checked_add(path_nibbles.len())?;

                if is_leaf {
                    if pos != key_nibbles.len() {
                        return None;
                    }
                    return match &items[1] {
                        RlpItem::Bytes(v) if !v.is_empty() => Some(v.to_vec()),
                        // Empty value is allowed in general, but for SCCP we treat it as missing.
                        _ => None,
                    };
                }

                // Extension: follow child ref
                let child = match &items[1] {
                    RlpItem::Bytes(b) => *b,
                    _ => return None,
                };
                expected = node_ref_from_bytes(child)?;
            }
            _ => return None,
        }
    }
}

/// Extract `storageRoot` from an Ethereum account RLP (value in the account trie).
pub fn evm_account_storage_root(account_rlp: &[u8]) -> Option<H256> {
    let item = rlp_decode(account_rlp)?;
    let RlpItem::List(items) = item else {
        return None;
    };
    if items.len() != 4 {
        return None;
    }
    let root_bytes = match &items[2] {
        RlpItem::Bytes(b) => *b,
        _ => return None,
    };
    if root_bytes.len() != 32 {
        return None;
    }
    Some(H256::from_slice(root_bytes))
}

/// Decode an RLP-encoded byte string and return its payload bytes.
pub fn rlp_decode_bytes_payload<'a>(input: &'a [u8]) -> Option<&'a [u8]> {
    match rlp_decode(input)? {
        RlpItem::Bytes(b) => Some(b),
        _ => None,
    }
}
