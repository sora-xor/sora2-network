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

#[subxt::subxt(
    runtime_metadata_path = "src/bytes/metadata.scale",
    derive_for_all_types = "Clone",
    substitute_type(type = "eth_bridge::requests::AssetKind", with = "crate::AssetKind"),
    substitute_type(
        type = "eth_bridge::BridgeSignatureVersion",
        with = "crate::BridgeSignatureVersion"
    ),
    substitute_type(
        type = "eth_bridge::offchain::SignatureParams",
        with = "crate::SignatureParams"
    ),
    substitute_type(
        type = "beefy_light_client::ProvedSubstrateBridgeMessage",
        with = "::beefy_light_client::ProvedSubstrateBridgeMessage"
    ),
    substitute_type(
        type = "bridge_common::beefy_types::BeefyMMRLeaf",
        with = "::bridge_common::beefy_types::BeefyMMRLeaf"
    ),
    substitute_type(
        type = "bridge_common::beefy_types::Commitment",
        with = "::bridge_common::beefy_types::Commitment"
    ),
    substitute_type(
        type = "bridge_common::beefy_types::ValidatorProof",
        with = "::bridge_common::beefy_types::ValidatorProof"
    ),
    substitute_type(
        type = "bridge_common::beefy_types::ValidatorSet",
        with = "::bridge_common::beefy_types::ValidatorSet"
    ),
    substitute_type(
        type = "bridge_common::simplified_mmr_proof::SimplifiedMMRProof",
        with = "::bridge_common::simplified_mmr_proof::SimplifiedMMRProof"
    ),
    substitute_type(
        type = "bridge_types::ethashproof::DoubleNodeWithMerkleProof",
        with = "::bridge_types::ethashproof::DoubleNodeWithMerkleProof"
    ),
    substitute_type(
        type = "bridge_types::network_config::NetworkConfig",
        with = "::bridge_types::network_config::NetworkConfig"
    ),
    substitute_type(
        type = "bridge_types::types::AssetKind",
        with = "::bridge_types::types::AssetKind"
    ),
    substitute_type(
        type = "bridge_types::types::AuxiliaryDigest",
        with = "::bridge_types::types::AuxiliaryDigest"
    ),
    substitute_type(
        type = "bridge_types::types::LeafExtraData",
        with = "::bridge_types::types::LeafExtraData"
    ),
    substitute_type(
        type = "bridge_types::types::Message",
        with = "::bridge_types::types::Message"
    ),
    substitute_type(
        type = "bridge_types::types::ParachainMessage",
        with = "::bridge_types::types::ParachainMessage"
    ),
    substitute_type(
        type = "bridge_types::GenericNetworkId",
        with = "::bridge_types::GenericNetworkId"
    ),
    substitute_type(type = "bridge_types::header::Header", with = "::bridge_types::Header"),
    substitute_type(
        type = "bridge_types::header::HeaderId",
        with = "::bridge_types::HeaderId"
    ),
    substitute_type(
        type = "bridge_types::SubNetworkId",
        with = "::bridge_types::SubNetworkId"
    ),
    substitute_type(type = "common::primitives::AssetId32", with = "::common::AssetId32"),
    substitute_type(type = "common::primitives::AssetName", with = "::common::AssetName"),
    substitute_type(
        type = "common::primitives::AssetSymbol",
        with = "::common::AssetSymbol"
    ),
    substitute_type(
        type = "common::primitives::LiquiditySourceType",
        with = "::common::LiquiditySourceType"
    ),
    substitute_type(
        type = "common::primitives::PredefinedAssetId",
        with = "::common::PredefinedAssetId"
    ),
    substitute_type(
        type = "common::primitives::RewardReason",
        with = "::common::RewardReason"
    ),
    substitute_type(type = "sp_beefy::crypto::Public", with = "::sp_beefy::crypto::Public"),
    substitute_type(
        type = "sp_beefy::mmr::BeefyAuthoritySet",
        with = "::sp_beefy::mmr::BeefyAuthoritySet"
    ),
    substitute_type(type = "sp_beefy::mmr::MmrLeaf", with = "::sp_beefy::mmr::MmrLeaf"),
    substitute_type(
        type = "sp_beefy::commitment::Commitment",
        with = "::sp_beefy::Commitment"
    ),
    substitute_type(type = "sp_core::ecdsa::Public", with = "::sp_core::ecdsa::Public"),
    substitute_type(type = "primitive_types::H160", with = "::sp_core::H160"),
    substitute_type(type = "primitive_types::H256", with = "::sp_core::H256"),
    substitute_type(type = "primitive_types::H128", with = "::sp_core::H512"),
    substitute_type(type = "primitive_types::U256", with = "::sp_core::U256"),
    substitute_type(
        type = "sp_runtime::MultiSignature",
        with = "::sp_runtime::MultiSignature"
    ),
    substitute_type(type = "sp_runtime::MultiSigner", with = "::sp_runtime::MultiSigner"),
    substitute_type(
        type = "sp_runtime::bounded::bounded_vec::BoundedVec",
        with = "::std::vec::Vec"
    )
)]
pub mod runtime {}

pub use config::*;
pub mod config {
    use std::fmt::Debug;
    use subxt::config::substrate::{BlakeTwo256, SubstrateHeader};
    use subxt::{config::polkadot::PolkadotExtrinsicParams, Config};

    pub type SoraExtrinsicParams = PolkadotExtrinsicParams<DefaultConfig>;

    #[derive(Clone, Debug, Default, Eq, PartialEq)]
    pub struct DefaultConfig;
    impl Config for DefaultConfig {
        type Index = u32;
        type Hash = sp_core::H256;
        type Hasher = BlakeTwo256;
        type AccountId = sp_runtime::AccountId32;
        type Address = Self::AccountId;
        type Header = SubstrateHeader<u32, Self::Hasher>;
        type Signature = sp_runtime::MultiSignature;
        type ExtrinsicParams = SoraExtrinsicParams;
    }
}
