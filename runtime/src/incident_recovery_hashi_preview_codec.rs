//! Public fixture wire types shared by the offline utility and cfg(test) exporter.
use codec::{Decode, DecodeWithMemTracking, Encode};
use common::recovery::{RecoveryChunk, RecoveryPolicy};
use sp_core::crypto::AccountId32;
use std::collections::BTreeMap;

pub type Values = BTreeMap<Vec<u8>, Option<Vec<u8>>>;
pub const INPUT_TAG: [u8; 8] = *b"snxhain4";
pub const OUTPUT_TAG: [u8; 8] = *b"snxhao04";

#[derive(Encode, Decode, DecodeWithMemTracking)]
pub struct Input {
    pub tag: [u8; 8],
    pub checkpoint_top: BTreeMap<Vec<u8>, Vec<u8>>,
    pub setup: RecoveryChunk,
    pub incoming: Vec<Vec<u8>>,
    pub policy: RecoveryPolicy,
    pub stable_keys: Vec<Vec<u8>>,
    pub old: AccountId32,
    pub replacement: AccountId32,
    pub asset: common::AssetId32<common::PredefinedAssetId>,
    pub mint: u128,
    pub network: u32,
    /// Empty is discovery-only. Bound previews execute every ordered audit.
    pub audits: Vec<Vec<u8>>,
}

#[derive(Encode, Decode, DecodeWithMemTracking)]
pub struct Output {
    pub tag: [u8; 8],
    pub input_hash: [u8; 32],
    pub values: Values,
    pub operations: Vec<(u32, Values)>,
    /// Actual native storage after all operations; public configuration is retained.
    pub retained_configuration: Values,
    pub bridge_halt_verified: bool,
}

/// Fixed execution-format envelopes signed with PUBLIC 0x46/0x47 seed bytes.
/// Their signatures identify an outer actor only. The fixture supplies synthetic
/// peer-approved Deposit requests and proves no external event or finality.
pub const SOURCE_ACTOR: [u8; 20] = [
    0x9d, 0x8a, 0x62, 0xf6, 0x56, 0xa8, 0xd1, 0x61, 0x5c, 0x12, 0x94, 0xfd, 0x71, 0xe9, 0xcf, 0xb3,
    0xe4, 0x85, 0x5a, 0x4f,
];
/// Public excluded transaction actor; identical registry on all operations.
pub const EXCLUDED_SOURCE_ACTOR: [u8; 20] = [
    0xb5, 0x95, 0xb1, 0x8c, 0x88, 0xb1, 0xf6, 0x51, 0xca, 0x38, 0x74, 0x89, 0x06, 0x7f, 0x85, 0x5b,
    0x5c, 0x8e, 0x67, 0x20,
];
pub const INCOMING_COUNT: u32 = 3;
pub const CREDITED_COUNT: u128 = 2;
pub fn source_transactions() -> Vec<Vec<u8>> {
    [
        b"f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83".as_slice(),
        b"01f867010a8504a817c800830186a09435353535353535353535353535353535353535350180c080a03ffb42ff06eed52514d07fb2dc4fdb308eaee6a9833a7b9c00d04728dbbca890a01c007bdadd0209e5e149e01ee9b53a89160e9d04797cdbe1f0afee8e8bf6cf3f".as_slice(),
        b"02f8680180018504a817c800830186a09435353535353535353535353535353535353535350180c080a07ce65392d1c4c89301c3d3a9a9244766f8e71f6711e094bb0a47e337560f93c9a06a7b32a732d018ee2eb005f4c3e33d3acf2fb0bb2536d8ca590021a9984f0c7e".as_slice(),
    ]
    .iter()
    .map(|value| value.as_chunks::<2>().0.iter().map(|pair| {
        let digit = |byte| match byte {
            b'0'..=b'9' => byte-b'0', b'a'..=b'f' => byte-b'a'+10,
            _ => unreachable!("static public transaction hex"),
        };
        (digit(pair[0]) << 4) | digit(pair[1])
    }).collect()).collect()
}
