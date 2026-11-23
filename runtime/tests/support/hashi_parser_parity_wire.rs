//! Native test-only protocol, never a runtime API or release approval format.
use codec::{Decode, DecodeWithMemTracking, Encode};

pub const MAX_INPUT: usize = 1024 * 1024 * 1024;
pub const MAX_CASES: usize = 128;
pub const CHECKPOINT_ROOT: [u8; 32] = [
    0x79, 0xbd, 0xa4, 0x1a, 0x1d, 0xb5, 0x34, 0x70, 0x46, 0x36, 0x53, 0xf3, 0x58, 0x89, 0x0f, 0x2d,
    0x1f, 0xca, 0x8a, 0xd6, 0x22, 0xc3, 0x3a, 0x1d, 0x70, 0x95, 0x29, 0x4b, 0x0b, 0x14, 0x04, 0x52,
];

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub enum Scope {
    /// Invented public logs, rooted in unchanged actual checkpoint configuration.
    SyntheticCheckpoint,
    /// Original committed rows accepted by the separate association adapter.
    AssociatedRows,
}

#[derive(Clone, Encode, Decode, DecodeWithMemTracking)]
pub struct Blob {
    pub label: String,
    pub sha256: [u8; 32],
    pub bytes: Vec<u8>,
}

#[derive(Clone, Encode, Decode, DecodeWithMemTracking)]
pub struct Log {
    pub address: [u8; 20],
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
    pub block_hash: [u8; 32],
    pub transaction_hash: [u8; 32],
    pub transaction_index: u32,
    pub global_index: u32,
    pub receipt_index: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub enum Outcome {
    Accepted(Vec<u8>),
    UnsupportedLoadKind,
    EventError(String),
    RequestError(String),
}

#[derive(Clone, Encode, Decode, DecodeWithMemTracking)]
pub struct Case {
    pub label: String,
    pub network: u32,
    pub height: u64,
    pub load_scale: Vec<u8>,
    pub logs: Vec<Log>,
    pub expected: Outcome,
}

#[derive(Encode, Decode, DecodeWithMemTracking)]
pub struct Input {
    pub tag: [u8; 8],
    pub scope: Scope,
    /// Strictly ordered complete top trie; child tries are unsupported.
    pub top: Vec<(Vec<u8>, Vec<u8>)>,
    /// Original capture manifest/metadata/witness, then exact row evidence.
    pub evidence: Vec<Blob>,
    pub cases: Vec<Case>,
}

#[derive(Encode, Decode, DecodeWithMemTracking)]
pub struct Output {
    pub tag: [u8; 8],
    pub input_hash: [u8; 32],
    pub scope: Scope,
    pub spec_version: u32,
    pub transaction_version: u32,
    pub state_root_before: Vec<u8>,
    pub state_root_after: Vec<u8>,
    pub outcomes: Vec<Outcome>,
}

pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::from("0x");
    for byte in bytes {
        use core::fmt::Write;
        write!(out, "{byte:02x}").expect("String write");
    }
    out
}

pub fn unhex(value: &str) -> Vec<u8> {
    let raw = value.strip_prefix("0x").expect("0x hex prefix");
    assert_eq!(raw.len() % 2, 0);
    assert!(raw
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    raw.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
