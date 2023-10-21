// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

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

pub type MaxU32 = sp_runtime::traits::ConstU32<{ core::u32::MAX }>;
pub type UnboundedBridgeMessage = bridge_types::substrate::BridgeMessage<MaxU32>;
pub type UnboundedGenericCommitment = bridge_types::GenericCommitment<MaxU32, MaxU32>;
pub type UnboundedGenericCommitmentWithBlock<BlockNumber> =
    bridge_types::types::GenericCommitmentWithBlock<BlockNumber, MaxU32, MaxU32>;

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
    #[subxt(substitute_type = "bridge_types::substrate::BridgeMessage")]
    use crate::UnboundedBridgeMessage;
    #[subxt(substitute_type = "bridge_types::GenericCommitment")]
    use crate::UnboundedGenericCommitment;
    #[subxt(substitute_type = "bridge_types::types::GenericCommitmentWithBlock")]
    use crate::UnboundedGenericCommitmentWithBlock;
    #[subxt(substitute_type = "beefy_light_client::SubstrateBridgeMessageProof")]
    use ::beefy_light_client::SubstrateBridgeMessageProof;
    #[subxt(substitute_type = "bridge_common::beefy_types::BeefyMMRLeaf")]
    use ::bridge_common::beefy_types::BeefyMMRLeaf;
    #[subxt(substitute_type = "bridge_common::beefy_types::Commitment")]
    use ::bridge_common::beefy_types::Commitment;
    #[subxt(substitute_type = "bridge_common::beefy_types::ValidatorProof")]
    use ::bridge_common::beefy_types::ValidatorProof;
    #[subxt(substitute_type = "bridge_common::beefy_types::ValidatorSet")]
    use ::bridge_common::beefy_types::ValidatorSet;
    #[subxt(substitute_type = "bridge_common::simplified_proof::Proof")]
    use ::bridge_common::simplified_proof::Proof;
    #[subxt(substitute_type = "bridge_types::ethashproof::DoubleNodeWithMerkleProof")]
    use ::bridge_types::ethashproof::DoubleNodeWithMerkleProof;
    #[subxt(substitute_type = "bridge_types::evm::Proof")]
    use ::bridge_types::evm::Proof;
    #[subxt(substitute_type = "bridge_types::log::Log")]
    use ::bridge_types::log::Log;
    #[subxt(substitute_type = "bridge_types::network_config::NetworkConfig")]
    use ::bridge_types::network_config::NetworkConfig;
    #[subxt(substitute_type = "bridge_types::types::AssetKind")]
    use ::bridge_types::types::AssetKind;
    #[subxt(substitute_type = "bridge_types::types::AuxiliaryDigest")]
    use ::bridge_types::types::AuxiliaryDigest;
    #[subxt(substitute_type = "bridge_types::types::LeafExtraData")]
    use ::bridge_types::types::LeafExtraData;
    #[subxt(substitute_type = "bridge_types::types::Message")]
    use ::bridge_types::types::Message;
    #[subxt(substitute_type = "bridge_types::types::ParachainMessage")]
    use ::bridge_types::types::ParachainMessage;
    #[subxt(substitute_type = "bridge_types::GenericNetworkId")]
    use ::bridge_types::GenericNetworkId;
    #[subxt(substitute_type = "bridge_types::header::Header")]
    use ::bridge_types::Header;
    #[subxt(substitute_type = "bridge_types::header::HeaderId")]
    use ::bridge_types::HeaderId;
    #[subxt(substitute_type = "bridge_types::SubNetworkId")]
    use ::bridge_types::SubNetworkId;
    #[subxt(substitute_type = "common::balance_unit::BalanceUnit")]
    use ::common::prelude::BalanceUnit;
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
    #[subxt(substitute_type = "sp_beefy::crypto::Public")]
    use ::sp_beefy::crypto::Public;
    #[subxt(substitute_type = "sp_beefy::mmr::BeefyAuthoritySet")]
    use ::sp_beefy::mmr::BeefyAuthoritySet;
    #[subxt(substitute_type = "sp_beefy::mmr::MmrLeaf")]
    use ::sp_beefy::mmr::MmrLeaf;
    #[subxt(substitute_type = "sp_beefy::commitment::Commitment")]
    use ::sp_beefy::Commitment;
    #[subxt(substitute_type = "sp_core::ecdsa::Public")]
    use ::sp_core::ecdsa::Public;
    #[subxt(substitute_type = "sp_core::ecdsa::Signature")]
    use ::sp_core::ecdsa::Signature;
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
    #[subxt(substitute_type = "sp_core::bounded::bounded_btree_map::BoundedBTreeMap")]
    use ::std::collections::btree_map::BTreeMap;
    #[subxt(substitute_type = "sp_core::bounded::bounded_btree_set::BoundedBTreeSet")]
    use ::std::collections::btree_set::BTreeSet;
    #[subxt(substitute_type = "sp_core::bounded::bounded_vec::BoundedVec")]
    use ::std::vec::Vec;
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
        type ExtrinsicParams = SoraExtrinsicParams;
    }
}
