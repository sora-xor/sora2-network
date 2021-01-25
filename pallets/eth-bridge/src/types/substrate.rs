use crate::types::{H256, U64};
use serde::{Deserialize, Serialize};

#[serde(rename_all = "camelCase")]
#[derive(Serialize, Deserialize)]
pub struct SubstrateHeaderLimited {
    /// The parent hash.
    #[serde(skip)]
    pub parent_hash: H256,
    /// The block number (actually, 32-bit).
    pub number: U64,
    /// The state trie merkle root
    #[serde(skip)]
    pub state_root: H256,
    /// The merkle root of the extrinsics.
    #[serde(skip)]
    pub extrinsics_root: H256,
    /// A chain-specific digest of data useful for light clients or referencing auxiliary data.
    #[serde(skip)]
    pub digest: (),
}
