//! Recover the transaction-signing actor bound to an authenticated HASHI tx hash.
//!
//! This does not identify an internal contract caller, depositor or beneficial
//! owner, prove a receipt/event association, or establish external finality.
//! Original native incoming request bytes must remain unchanged. The caller
//! must separately bind the returned chain ID to its approved native network.

use bridge_types::{H160, H256, U256};
use common::recovery::RecoveryError;
use rlp::{PayloadInfo, RlpStream};
use sp_std::vec::Vec;

pub const MAX_RAW_TRANSACTION_BYTES: usize = 128 * 1024;
const MAX_RLP_DEPTH: usize = 8;
const MAX_RLP_NODES: usize = 8192;
const CURVE_ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedEvmSource {
    /// Actual outer transaction signer. Never an inferred contract depositor.
    pub signer: H160,
    /// Outer call target, or None for contract creation. Not a recipient/owner.
    pub destination: Option<H160>,
    /// None is explicitly unprotected legacy v=27/28, not an inferred chain ID.
    pub chain_id: Option<U256>,
    /// 0 = legacy; 1/2/3/4 = the exact EIP-2718 type byte.
    pub transaction_type: u8,
}

#[derive(Clone, Copy)]
struct Node<'a> {
    raw: &'a [u8],
    payload: &'a [u8],
    list: bool,
}

fn bad() -> RecoveryError {
    RecoveryError::WrongPreimage
}

/// Parse one prefix, rejecting all RLP nonminimal length forms. Unlike an RLP
/// iterator, malformed trailing children cannot disappear as end-of-list.
fn prefix(raw: &[u8]) -> Result<Node<'_>, RecoveryError> {
    let first = *raw.first().ok_or_else(bad)?;
    let info = PayloadInfo::from(raw).map_err(|_| bad())?;
    let total = info
        .header_len
        .checked_add(info.value_len)
        .ok_or_else(bad)?;
    if total > raw.len() || total == 0 {
        return Err(bad());
    }
    let payload = &raw[info.header_len..total];
    if first == 0x81 && payload.first().is_some_and(|value| *value < 0x80) {
        return Err(bad());
    }
    Ok(Node {
        raw: &raw[..total],
        payload,
        list: first >= 0xc0,
    })
}

fn validate_tree(raw: &[u8], depth: usize, nodes: &mut usize) -> Result<(), RecoveryError> {
    *nodes = nodes.checked_add(1).ok_or(RecoveryError::ChunkBounds)?;
    if depth > MAX_RLP_DEPTH || *nodes > MAX_RLP_NODES {
        return Err(RecoveryError::ChunkBounds);
    }
    let node = prefix(raw)?;
    if node.raw.len() != raw.len() {
        return Err(bad());
    }
    if node.list {
        let mut remainder = node.payload;
        while !remainder.is_empty() {
            let child = prefix(remainder)?;
            validate_tree(child.raw, depth + 1, nodes)?;
            remainder = &remainder[child.raw.len()..];
        }
    }
    Ok(())
}

fn children(node: Node<'_>, maximum: usize) -> Result<Vec<Node<'_>>, RecoveryError> {
    if !node.list {
        return Err(bad());
    }
    let mut result = Vec::new();
    let mut remainder = node.payload;
    while !remainder.is_empty() {
        if result.len() == maximum {
            return Err(RecoveryError::ChunkBounds);
        }
        let child = prefix(remainder)?;
        result.push(child);
        remainder = &remainder[child.raw.len()..];
    }
    Ok(result)
}

fn data(node: Node<'_>) -> Result<&[u8], RecoveryError> {
    if node.list {
        Err(bad())
    } else {
        Ok(node.payload)
    }
}

fn uint(node: Node<'_>, maximum_bytes: usize) -> Result<U256, RecoveryError> {
    let bytes = data(node)?;
    if bytes.len() > maximum_bytes || bytes.first() == Some(&0) {
        return Err(bad());
    }
    Ok(U256::from_big_endian(bytes))
}

fn fixed(node: Node<'_>, size: usize) -> Result<(), RecoveryError> {
    if data(node)?.len() == size {
        Ok(())
    } else {
        Err(bad())
    }
}

fn access_list(node: Node<'_>) -> Result<(), RecoveryError> {
    for entry in children(node, MAX_RLP_NODES)? {
        let fields = children(entry, 2)?;
        if fields.len() != 2 {
            return Err(bad());
        }
        fixed(fields[0], 20)?;
        for key in children(fields[1], MAX_RLP_NODES)? {
            fixed(key, 32)?;
        }
    }
    // Repeated addresses/keys are valid EIP-2930 and must not be rejected here.
    Ok(())
}

fn authorization_list(node: Node<'_>) -> Result<(), RecoveryError> {
    let entries = children(node, MAX_RLP_NODES)?;
    if entries.is_empty() {
        return Err(bad());
    }
    for entry in entries {
        let fields = children(entry, 6)?;
        if fields.len() != 6 {
            return Err(bad());
        }
        uint(fields[0], 32)?;
        fixed(fields[1], 20)?;
        uint(fields[2], 8)?;
        uint(fields[3], 1)?;
        uint(fields[4], 32)?;
        uint(fields[5], 32)?;
        // Invalid authorization signatures/chain IDs may be skipped by EIP-7702
        // execution. They are not the outer transaction signer. Preserve them
        // in the signed preimage; do not infer their ownership or require ecrecover.
    }
    Ok(())
}

fn append_uint(stream: &mut RlpStream, value: U256) {
    let mut bytes = [0u8; 32];
    value.write_as_big_endian(&mut bytes);
    let first = bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len());
    stream.append(&&bytes[first..]);
}

struct Signing<'a> {
    kind: u8,
    fields: Vec<Node<'a>>,
    chain_id: Option<U256>,
    parity: u8,
    r: U256,
    s: U256,
    unsigned_count: usize,
}

fn signing_fields(raw: &[u8]) -> Result<Signing<'_>, RecoveryError> {
    let first = *raw.first().ok_or_else(bad)?;
    let (kind, payload) = if first >= 0xc0 {
        (0, raw)
    } else if (1..=4).contains(&first) {
        (first, &raw[1..])
    } else {
        return Err(bad());
    };
    validate_tree(payload, 0, &mut 0)?;
    let fields = children(prefix(payload)?, 14)?;
    let counts = [9, 11, 12, 14, 13];
    if fields.len() != counts[kind as usize] {
        return Err(bad());
    }
    let (chain_id, parity, unsigned_count) = if kind == 0 {
        uint(fields[0], 8)?; // nonce
        uint(fields[1], 32)?;
        uint(fields[2], 8)?; // gas limit
        let to = data(fields[3])?;
        if !to.is_empty() && to.len() != 20 {
            return Err(bad());
        }
        uint(fields[4], 32)?;
        data(fields[5])?;
        let v = uint(fields[6], 32)?;
        if v == U256::from(27u8) || v == U256::from(28u8) {
            (None, (v - U256::from(27u8)).low_u32() as u8, 6)
        } else if v >= U256::from(35u8) {
            let adjusted = v - U256::from(35u8);
            (
                Some(adjusted / U256::from(2u8)),
                (adjusted % U256::from(2u8)).low_u32() as u8,
                6,
            )
        } else {
            return Err(bad());
        }
    } else {
        let chain_id = uint(fields[0], 32)?;
        uint(fields[1], 8)?;
        uint(fields[2], 32)?;
        let to_index = if kind == 1 { 4 } else { 5 };
        if kind != 1 {
            uint(fields[3], 32)?;
            if uint(fields[2], 32)? > uint(fields[3], 32)? {
                return Err(bad());
            }
        }
        uint(fields[to_index - 1], 8)?;
        let to = data(fields[to_index])?;
        if to.len() != 20 && !(kind < 3 && to.is_empty()) {
            return Err(bad());
        }
        uint(fields[to_index + 1], 32)?;
        data(fields[to_index + 2])?;
        access_list(fields[to_index + 3])?;
        if kind == 3 {
            uint(fields[9], 32)?;
            let hashes = children(fields[10], MAX_RLP_NODES)?;
            if hashes.is_empty() {
                return Err(bad());
            }
            for hash in hashes {
                fixed(hash, 32)?;
                if data(hash)?[0] != 1 {
                    return Err(bad());
                }
            }
        }
        if kind == 4 {
            authorization_list(fields[9])?;
        }
        let parity = uint(fields[fields.len() - 3], 1)?;
        if parity > U256::one() {
            return Err(bad());
        }
        (Some(chain_id), parity.low_u32() as u8, fields.len() - 3)
    };
    let r = uint(fields[fields.len() - 2], 32)?;
    let s = uint(fields[fields.len() - 1], 32)?;
    let order = U256::from_big_endian(&CURVE_ORDER);
    if r.is_zero() || r >= order || s.is_zero() || s > order / U256::from(2u8) {
        return Err(bad());
    }
    Ok(Signing {
        kind,
        fields,
        chain_id,
        parity,
        r,
        s,
        unsigned_count,
    })
}

fn signing_hash(signing: &Signing<'_>) -> H256 {
    let extra = usize::from(signing.kind == 0 && signing.chain_id.is_some()) * 3;
    let mut stream = RlpStream::new_list(signing.unsigned_count + extra);
    for field in signing.fields.iter().take(signing.unsigned_count) {
        stream.append_raw(field.raw, 1);
    }
    if signing.kind == 0 {
        if let Some(chain) = signing.chain_id {
            append_uint(&mut stream, chain);
            stream.append_empty_data();
            stream.append_empty_data();
        }
    }
    let encoded = stream.out();
    if signing.kind == 0 {
        H256::from(sp_io::hashing::keccak_256(&encoded))
    } else {
        let mut message = Vec::with_capacity(encoded.len() + 1);
        message.push(signing.kind);
        message.extend_from_slice(&encoded);
        H256::from(sp_io::hashing::keccak_256(&message))
    }
}

/// Establish the signing actor for these exact execution-format transaction
/// bytes. The expected hash must come from the ORIGINAL native-authenticated
/// request; never substitute a caller-provided RPC transactionHash field.
pub fn authenticated_transaction_source(
    raw: &[u8],
    expected_tx_hash: H256,
) -> Result<AuthenticatedEvmSource, RecoveryError> {
    if raw.is_empty() || raw.len() > MAX_RAW_TRANSACTION_BYTES {
        return Err(RecoveryError::ChunkBounds);
    }
    if expected_tx_hash == H256::zero() || sp_io::hashing::keccak_256(raw) != expected_tx_hash.0 {
        return Err(bad());
    }
    let signing = signing_fields(raw)?;
    let mut signature = [0u8; 65];
    signing.r.write_as_big_endian(&mut signature[..32]);
    signing.s.write_as_big_endian(&mut signature[32..64]);
    signature[64] = signing.parity;
    let key = sp_io::crypto::secp256k1_ecdsa_recover(&signature, &signing_hash(&signing).0)
        .map_err(|_| bad())?;
    let digest = sp_io::hashing::keccak_256(&key);
    let signer = H160::from_slice(&digest[12..]);
    if signer == H160::zero() {
        return Err(bad());
    }
    let destination_index = if signing.kind == 0 {
        3
    } else if signing.kind == 1 {
        4
    } else {
        5
    };
    let destination = data(signing.fields[destination_index])?;
    let destination = if destination.is_empty() {
        None
    } else {
        Some(H160::from_slice(destination))
    };
    Ok(AuthenticatedEvmSource {
        signer,
        destination,
        chain_id: signing.chain_id,
        transaction_type: signing.kind,
    })
}

#[cfg(test)]
#[path = "incident_recovery_evm_source_tests.rs"]
mod tests;
