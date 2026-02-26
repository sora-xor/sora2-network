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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::collection::vec;
    use proptest::prelude::*;

    fn encode_compact_path_from_nibbles(is_leaf: bool, nibbles: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + ((nibbles.len() + 1) / 2));
        let odd = nibbles.len() % 2 == 1;
        let flag = match (is_leaf, odd) {
            (false, false) => 0x00,
            (false, true) => 0x10,
            (true, false) => 0x20,
            (true, true) => 0x30,
        };

        if odd {
            out.push(flag | nibbles[0]);
            for pair in nibbles[1..].chunks(2) {
                out.push((pair[0] << 4) | pair[1]);
            }
        } else {
            out.push(flag);
            for pair in nibbles.chunks(2) {
                out.push((pair[0] << 4) | pair[1]);
            }
        }
        out
    }

    fn encode_leaf_compact_path_for_key(key32: &[u8; 32]) -> Vec<u8> {
        encode_compact_path_from_nibbles(true, &bytes_to_nibbles(key32))
    }

    fn encode_leaf_node_for_key(key32: &[u8; 32], value: &[u8]) -> Vec<u8> {
        let compact = encode_leaf_compact_path_for_key(key32);
        let items = vec![rlp_encode_bytes(&compact), rlp_encode_bytes(value)];
        rlp_encode_list(&items)
    }

    #[test]
    fn rlp_decode_rejects_trailing_bytes() {
        let encoded = rlp_encode_bytes(b"abc");
        let mut with_trailing = encoded.clone();
        with_trailing.push(0x00);
        assert!(rlp_decode(&encoded).is_some());
        assert!(rlp_decode(&with_trailing).is_none());
    }

    #[test]
    fn rlp_decode_accepts_long_string_encoding() {
        let payload = vec![0xabu8; 60];
        let encoded = rlp_encode_bytes(&payload);
        let decoded = rlp_decode(&encoded).expect("long string should decode");
        assert_eq!(decoded, RlpItem::Bytes(payload.as_slice()));
    }

    #[test]
    fn rlp_decode_accepts_long_list_encoding() {
        let mut items = Vec::new();
        for _ in 0..30 {
            items.push(rlp_encode_bytes(&[0x11u8, 0x22u8]));
        }
        let encoded = rlp_encode_list(&items);
        let decoded = rlp_decode(&encoded).expect("long list should decode");
        let RlpItem::List(decoded_items) = decoded else {
            panic!("decoded item should be a list");
        };
        assert_eq!(decoded_items.len(), 30);
        for item in decoded_items {
            assert_eq!(item, RlpItem::Bytes(&[0x11u8, 0x22u8]));
        }
    }

    #[test]
    fn decode_compact_path_rejects_even_path_with_non_zero_padding_nibble() {
        // Even length paths must have zero low nibble in the first byte.
        assert!(decode_compact_path(&[0x21, 0xab]).is_none());
    }

    #[test]
    fn decode_compact_path_accepts_odd_leaf_path() {
        let (is_leaf, nibbles) =
            decode_compact_path(&[0x31, 0xab]).expect("odd compact path should decode");
        assert!(is_leaf, "0x3 high nibble denotes odd leaf path");
        assert_eq!(nibbles, vec![0x1, 0xa, 0xb]);
    }

    #[test]
    fn decode_compact_path_accepts_empty_even_extension_path() {
        let (is_leaf, nibbles) =
            decode_compact_path(&[0x00]).expect("empty even extension path should decode");
        assert!(!is_leaf);
        assert!(nibbles.is_empty());
    }

    #[test]
    fn decode_compact_path_rejects_empty_input() {
        assert!(
            decode_compact_path(&[]).is_none(),
            "empty compact path must fail closed"
        );
    }

    #[test]
    fn node_ref_from_bytes_respects_length_boundaries() {
        assert!(
            node_ref_from_bytes(&[]).is_none(),
            "empty child ref should be treated as missing"
        );
        assert!(
            matches!(node_ref_from_bytes(&[0x11]), Some(NodeRef::Embedded(_))),
            "short child refs should be embedded"
        );
        assert!(
            matches!(node_ref_from_bytes(&[0x22; 31]), Some(NodeRef::Embedded(_))),
            "31-byte child refs should be embedded"
        );
        assert!(
            matches!(node_ref_from_bytes(&[0x33; 32]), Some(NodeRef::Hash(_))),
            "32-byte child refs should be treated as hash references"
        );
        assert!(
            node_ref_from_bytes(&[0x44; 33]).is_none(),
            "oversized child refs must fail closed"
        );
    }

    #[test]
    fn mpt_get_accepts_single_leaf_proof_for_exact_key() {
        let key = [0x11u8; 32];
        let value = vec![0xaa, 0xbb, 0xcc];
        let leaf = encode_leaf_node_for_key(&key, &value);
        let root = keccak(&leaf);
        let proof = vec![leaf];

        let got = mpt_get(root, &key, &proof).expect("proof should resolve exact key");
        assert_eq!(got, value);
    }

    #[test]
    fn mpt_get_rejects_single_leaf_proof_for_different_key() {
        let key = [0x22u8; 32];
        let value = vec![0x01u8];
        let leaf = encode_leaf_node_for_key(&key, &value);
        let root = keccak(&leaf);
        let proof = vec![leaf];
        let mut other_key = key;
        other_key[31] ^= 0x01;

        assert!(mpt_get(root, &other_key, &proof).is_none());
    }

    #[test]
    fn mpt_get_accepts_extension_then_leaf_proof() {
        let key = [0x00u8; 32];
        let value = vec![0xa1, 0xb2, 0xc3];
        let key_nibbles = bytes_to_nibbles(&key);
        let extension_nibbles = vec![key_nibbles[0]];
        let leaf_nibbles = key_nibbles[1..].to_vec();

        let leaf_node = rlp_encode_list(&[
            rlp_encode_bytes(&encode_compact_path_from_nibbles(true, &leaf_nibbles)),
            rlp_encode_bytes(&value),
        ]);
        let leaf_hash = keccak(&leaf_node);
        let extension_node = rlp_encode_list(&[
            rlp_encode_bytes(&encode_compact_path_from_nibbles(false, &extension_nibbles)),
            rlp_encode_bytes(leaf_hash.as_bytes()),
        ]);
        let root = keccak(&extension_node);
        let proof = vec![extension_node, leaf_node];

        let got = mpt_get(root, &key, &proof).expect("extension+leaf proof should resolve key");
        assert_eq!(got, value);
    }

    #[test]
    fn mpt_get_accepts_branch_child_hash_reference() {
        let key = [0x00u8; 32];
        let value = vec![0x99u8, 0x88u8];
        let key_nibbles = bytes_to_nibbles(&key);
        let leaf = rlp_encode_list(&[
            rlp_encode_bytes(&encode_compact_path_from_nibbles(true, &key_nibbles[1..])),
            rlp_encode_bytes(&value),
        ]);
        let leaf_hash = keccak(&leaf);

        let mut branch_items: Vec<Vec<u8>> = (0..17).map(|_| rlp_encode_bytes(&[])).collect();
        branch_items[0] = rlp_encode_bytes(leaf_hash.as_bytes());
        let branch = rlp_encode_list(&branch_items);
        let root = keccak(&branch);
        let proof = vec![branch, leaf];

        let got = mpt_get(root, &key, &proof).expect("branch hash child should resolve proof");
        assert_eq!(got, value);
    }

    #[test]
    fn mpt_get_returns_branch_value_when_key_consumed() {
        let key = [0x11u8; 32];
        let key_nibbles = bytes_to_nibbles(&key);
        let branch_value = vec![0xaau8, 0xbbu8];

        let mut branch_items: Vec<Vec<u8>> = (0..17).map(|_| rlp_encode_bytes(&[])).collect();
        branch_items[16] = rlp_encode_bytes(&branch_value);
        let branch = rlp_encode_list(&branch_items);
        let branch_hash = keccak(&branch);

        let extension = rlp_encode_list(&[
            rlp_encode_bytes(&encode_compact_path_from_nibbles(false, &key_nibbles)),
            rlp_encode_bytes(branch_hash.as_bytes()),
        ]);
        let root = keccak(&extension);
        let proof = vec![extension, branch];

        let got = mpt_get(root, &key, &proof).expect("consumed-key branch value should resolve");
        assert_eq!(got, branch_value);
    }

    #[test]
    fn mpt_get_rejects_extension_path_mismatch() {
        let key = [0x00u8; 32];
        let value = vec![0x77];
        let key_nibbles = bytes_to_nibbles(&key);
        let leaf_node = rlp_encode_list(&[
            rlp_encode_bytes(&encode_compact_path_from_nibbles(true, &key_nibbles[1..])),
            rlp_encode_bytes(&value),
        ]);
        let leaf_hash = keccak(&leaf_node);
        // Extension expects first nibble 0.
        let extension_node = rlp_encode_list(&[
            rlp_encode_bytes(&encode_compact_path_from_nibbles(false, &[0])),
            rlp_encode_bytes(leaf_hash.as_bytes()),
        ]);
        let root = keccak(&extension_node);
        let proof = vec![extension_node, leaf_node];

        let mut mismatched_key = key;
        mismatched_key[0] = 0x10; // first nibble becomes 1, path mismatch at extension
        assert!(mpt_get(root, &mismatched_key, &proof).is_none());
    }

    #[test]
    fn mpt_get_rejects_branch_child_when_entry_is_not_bytes() {
        let key = [0x00u8; 32];
        let mut branch_items: Vec<Vec<u8>> = (0..17).map(|_| rlp_encode_bytes(&[])).collect();
        // Child entry must be bytes; list entry should fail closed.
        branch_items[0] = rlp_encode_list(&[]);
        let branch = rlp_encode_list(&branch_items);
        let root = keccak(&branch);
        let proof = vec![branch];

        assert!(mpt_get(root, &key, &proof).is_none());
    }

    #[test]
    fn mpt_get_rejects_extension_with_non_bytes_child() {
        let key = [0x00u8; 32];
        let extension = rlp_encode_list(&[
            rlp_encode_bytes(&encode_compact_path_from_nibbles(false, &[0])),
            rlp_encode_list(&[]),
        ]);
        let root = keccak(&extension);
        let proof = vec![extension];
        assert!(mpt_get(root, &key, &proof).is_none());
    }

    #[test]
    fn mpt_get_rejects_leaf_with_empty_value_payload() {
        let key = [0x33u8; 32];
        let leaf = encode_leaf_node_for_key(&key, &[]);
        let root = keccak(&leaf);
        let proof = vec![leaf];
        assert!(
            mpt_get(root, &key, &proof).is_none(),
            "empty leaf value is treated as missing for SCCP"
        );
    }

    #[test]
    fn mpt_get_rejects_branch_child_reference_longer_than_32_bytes() {
        let key = [0x00u8; 32];
        let mut items: Vec<Vec<u8>> = (0..17).map(|_| rlp_encode_bytes(&[])).collect();
        // Invalid child ref length must fail closed.
        items[0] = rlp_encode_bytes(&[0xabu8; 33]);
        let branch = rlp_encode_list(&items);
        let root = keccak(&branch);
        let proof = vec![branch];

        assert!(mpt_get(root, &key, &proof).is_none());
    }

    #[test]
    fn mpt_get_rejects_when_root_hash_does_not_match_proof_node() {
        let key = [0x55u8; 32];
        let value = vec![0x99u8];
        let leaf = encode_leaf_node_for_key(&key, &value);
        let wrong_root = keccak(b"not-the-leaf");
        let proof = vec![leaf];

        assert!(
            mpt_get(wrong_root, &key, &proof).is_none(),
            "root hash mismatch should fail closed"
        );
    }

    #[test]
    fn mpt_get_rejects_empty_proof() {
        let key = [0x11u8; 32];
        assert!(
            mpt_get(H256::zero(), &key, &[]).is_none(),
            "empty proof should fail closed"
        );
    }

    #[test]
    fn evm_account_storage_root_accepts_valid_account_shape() {
        let root = [0x55u8; 32];
        let account = rlp_encode_list(&[
            rlp_encode_bytes(&[0x01]), // nonce
            rlp_encode_bytes(&[0x02]), // balance
            rlp_encode_bytes(&root),   // storage root
            rlp_encode_bytes(&[0x03]), // code hash placeholder
        ]);

        let parsed = evm_account_storage_root(&account).expect("valid account should parse");
        assert_eq!(parsed, H256::from(root));
    }

    #[test]
    fn evm_account_storage_root_rejects_malformed_account_shape() {
        let not_a_list = rlp_encode_bytes(&[0x01, 0x02]);
        assert!(
            evm_account_storage_root(&not_a_list).is_none(),
            "non-list account payload should be rejected"
        );

        let wrong_len_account = rlp_encode_list(&[
            rlp_encode_bytes(&[]),
            rlp_encode_bytes(&[]),
            rlp_encode_bytes(&[0x11u8; 32]),
        ]);
        assert!(
            evm_account_storage_root(&wrong_len_account).is_none(),
            "account list with wrong item count should be rejected"
        );

        let short_root_account = rlp_encode_list(&[
            rlp_encode_bytes(&[]),
            rlp_encode_bytes(&[]),
            rlp_encode_bytes(&[0x11u8; 31]),
            rlp_encode_bytes(&[]),
        ]);
        assert!(
            evm_account_storage_root(&short_root_account).is_none(),
            "account state root must be exactly 32 bytes"
        );

        let root_slot_is_list = rlp_encode_list(&[
            rlp_encode_bytes(&[]),
            rlp_encode_bytes(&[]),
            rlp_encode_list(&[]),
            rlp_encode_bytes(&[]),
        ]);
        assert!(
            evm_account_storage_root(&root_slot_is_list).is_none(),
            "account state root slot must be bytes, not list"
        );
    }

    #[test]
    fn rlp_decode_bytes_payload_rejects_list_items() {
        let encoded_list = rlp_encode_list(&[rlp_encode_bytes(b"abc")]);
        assert!(
            rlp_decode_bytes_payload(&encoded_list).is_none(),
            "list payload should not decode as raw bytes payload"
        );
    }

    #[test]
    fn rlp_decode_bytes_payload_accepts_string_items() {
        let encoded = rlp_encode_bytes(b"abc");
        assert_eq!(rlp_decode_bytes_payload(&encoded), Some(&b"abc"[..]));
    }

    #[test]
    fn mpt_get_rejects_empty_branch_value_for_consumed_key() {
        let key = [0x55u8; 32];
        let key_nibbles = bytes_to_nibbles(&key);

        let mut branch_items: Vec<Vec<u8>> = (0..17).map(|_| rlp_encode_bytes(&[])).collect();
        branch_items[16] = rlp_encode_bytes(&[]);
        let branch = rlp_encode_list(&branch_items);
        let branch_hash = keccak(&branch);

        let extension = rlp_encode_list(&[
            rlp_encode_bytes(&encode_compact_path_from_nibbles(false, &key_nibbles)),
            rlp_encode_bytes(branch_hash.as_bytes()),
        ]);
        let root = keccak(&extension);
        let proof = vec![extension, branch];

        assert!(
            mpt_get(root, &key, &proof).is_none(),
            "empty branch value is treated as missing for SCCP"
        );
    }

    #[test]
    fn evm_proof_helpers_fail_closed_on_fuzzed_inputs() {
        let mut seed = 0x1234_5678_9abc_def0u64;
        for len in 0..128usize {
            let mut buf = vec![0u8; len];
            for byte in &mut buf {
                // xorshift64* deterministic pseudo-random generator.
                seed ^= seed >> 12;
                seed ^= seed << 25;
                seed ^= seed >> 27;
                let mixed = seed.wrapping_mul(0x2545_f491_4f6c_dd1d);
                *byte = (mixed & 0xff) as u8;
            }

            let _ = rlp_decode(&buf);
            let _ = decode_compact_path(&buf);
            if buf.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&buf);
                let proof = vec![buf.clone()];
                let _ = mpt_get(keccak(&buf), &key, &proof);
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn evm_proof_helpers_property_no_panic_on_arbitrary_bytes(input in vec(any::<u8>(), 0..512)) {
            let _ = rlp_decode(&input);
            let _ = decode_compact_path(&input);

            let mut key = [0u8; 32];
            let to_copy = core::cmp::min(input.len(), key.len());
            key[..to_copy].copy_from_slice(&input[..to_copy]);

            let proof = vec![input.clone()];
            let _ = mpt_get(keccak(&input), &key, &proof);
            let _ = evm_account_storage_root(&input);
            let _ = rlp_decode_bytes_payload(&input);
        }
    }
}
