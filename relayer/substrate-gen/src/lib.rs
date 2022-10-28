#[macro_use]
extern crate codec;

#[macro_use]
extern crate serde;
/// Separated components of a secp256k1 signature.
#[derive(
    Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, scale_info::TypeInfo, Default, Debug,
)]
#[repr(C)]
pub struct SignatureParams {
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub v: u8,
}

#[derive(
    Clone, Copy, Encode, Decode, PartialEq, Eq, Debug, scale_info::TypeInfo, Serialize, Deserialize,
)]
pub enum AssetKind {
    Thischain,
    Sidechain,
    SidechainOwned,
}

#[derive(
    Clone, Copy, Encode, Decode, PartialEq, Eq, Debug, scale_info::TypeInfo, Serialize, Deserialize,
)]
pub enum BridgeSignatureVersion {
    V1,
    V2,
}

#[subxt::subxt(
    runtime_metadata_path = "src/bytes/metadata.scale",
    derive_for_all_types = "Clone"
)]
pub mod runtime {
    #[subxt(substitute_type = "eth_bridge::requests::AssetKind")]
    use crate::AssetKind;
    #[subxt(substitute_type = "eth_bridge::BridgeSignatureVersion")]
    use crate::BridgeSignatureVersion;
    #[subxt(substitute_type = "eth_bridge::offchain::SignatureParams")]
    use crate::SignatureParams;
    #[subxt(substitute_type = "beefy_primitives::crypto::Public")]
    use ::beefy_primitives::crypto::Public;
    #[subxt(substitute_type = "bridge_types::ethashproof::DoubleNodeWithMerkleProof")]
    use ::bridge_types::ethashproof::DoubleNodeWithMerkleProof;
    #[subxt(substitute_type = "bridge_types::network_config::NetworkConfig")]
    use ::bridge_types::network_config::NetworkConfig;
    #[subxt(substitute_type = "bridge_types::types::AssetKind")]
    use ::bridge_types::types::AssetKind;
    #[subxt(substitute_type = "bridge_types::types::Message")]
    use ::bridge_types::types::Message;
    #[subxt(substitute_type = "bridge_types::header::Header")]
    use ::bridge_types::Header;
    #[subxt(substitute_type = "bridge_types::header::HeaderId")]
    use ::bridge_types::HeaderId;
    #[subxt(substitute_type = "common::primitives::AssetId32")]
    use ::common::AssetId32;
    #[subxt(substitute_type = "common::primitives::AssetName")]
    use ::common::AssetName;
    #[subxt(substitute_type = "common::primitives::AssetSymbol")]
    use ::common::AssetSymbol;
    #[subxt(substitute_type = "common::primitives::LiquiditySourceType")]
    use ::common::LiquiditySourceType;
    #[subxt(substitute_type = "common::primitives::PredefinedAssetId")]
    use ::common::PredefinedAssetId;
    #[subxt(substitute_type = "common::primitives::RewardReason")]
    use ::common::RewardReason;
    #[subxt(substitute_type = "sp_core::ecdsa::Public")]
    use ::sp_core::ecdsa::Public;
    #[subxt(substitute_type = "primitive_types::H160")]
    use ::sp_core::H160;
    #[subxt(substitute_type = "primitive_types::H256")]
    use ::sp_core::H256;
    #[subxt(substitute_type = "primitive_types::H128")]
    use ::sp_core::H512;
    #[subxt(substitute_type = "primitive_types::U256")]
    use ::sp_core::U256;
    #[subxt(substitute_type = "sp_runtime::MultiSignature")]
    use ::sp_runtime::MultiSignature;
    #[subxt(substitute_type = "sp_runtime::MultiSigner")]
    use ::sp_runtime::MultiSigner;
    #[subxt(substitute_type = "sp_runtime::bounded::bounded_vec::BoundedVec")]
    use ::std::vec::Vec;
}

pub use config::*;
pub mod config {
    use std::fmt::Debug;
    use subxt::{tx::PolkadotExtrinsicParams, Config};

    pub type SoraExtrinsicParams = PolkadotExtrinsicParams<DefaultConfig>;

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    pub struct DefaultConfig;
    impl Config for DefaultConfig {
        type Index = u32;
        type BlockNumber = u32;
        type Hash = sp_core::H256;
        type Hashing = sp_runtime::traits::BlakeTwo256;
        type AccountId = sp_runtime::AccountId32;
        type Address = Self::AccountId;
        type Header =
            sp_runtime::generic::Header<Self::BlockNumber, sp_runtime::traits::BlakeTwo256>;
        type Signature = sp_runtime::MultiSignature;
        type Extrinsic = sp_runtime::OpaqueExtrinsic;
        type ExtrinsicParams = SoraExtrinsicParams;
    }
}
