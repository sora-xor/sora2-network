//! Public fixture wire types, shared by the offline utility and cfg(test) exporter.
use codec::{Decode, DecodeWithMemTracking, Encode};
use common::recovery::{RecoveryChunk, RecoveryPolicy};
use sp_core::crypto::AccountId32;
use std::collections::BTreeMap;

pub type Values = BTreeMap<Vec<u8>, Option<Vec<u8>>>;
#[derive(Encode, Decode, DecodeWithMemTracking)]
pub struct Input {
    pub tag: [u8; 8],
    pub checkpoint_top: BTreeMap<Vec<u8>, Vec<u8>>,
    pub setup: RecoveryChunk,
    pub restore: RecoveryChunk,
    pub incoming: Vec<u8>,
    pub outgoing: Vec<u8>,
    pub policy: RecoveryPolicy,
    pub stable_keys: Vec<Vec<u8>>,
    pub old: AccountId32,
    pub replacement: AccountId32,
    pub mint: u128,
    pub incoming_nonce: u64,
    pub outgoing_nonce: u64,
    /// Empty is discovery-only. Bound previews execute every ordered audit.
    pub audits: Vec<Vec<u8>>,
}
#[derive(Encode, Decode, DecodeWithMemTracking)]
pub struct Output {
    pub tag: [u8; 8],
    pub input_hash: [u8; 32],
    pub values: Values,
    pub operations: Vec<(u32, Values)>,
}
