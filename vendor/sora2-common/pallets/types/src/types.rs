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

//! Types for representing messages

use crate::evm::{AdditionalEVMInboundData, EVMAppInfo, EVMAssetInfo, EVMLegacyAssetInfo};
use crate::substrate::SubAssetInfo;
use crate::ton::{AdditionalTONInboundData, TonAppInfo, TonAssetInfo};
use crate::{GenericTimepoint, H256};
use codec::{Decode, Encode};
use derivative::Derivative;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_consensus_beefy::mmr::{BeefyNextAuthoritySet, MmrLeafVersion};
use sp_core::{Get, RuntimeDebug};
use sp_runtime::traits::Hash;
use sp_runtime::{Digest, DigestItem};
use sp_std::vec::Vec;

use crate::GenericNetworkId;

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub enum MessageDirection {
    Inbound,
    Outbound,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct MessageId {
    sender: GenericNetworkId,
    receiver: GenericNetworkId,
    batch_nonce: Option<BatchNonce>,
    message_nonce: MessageNonce,
}

impl MessageId {
    /// Creates MessageId for message in batch.
    pub fn batched(
        sender: GenericNetworkId,
        receiver: GenericNetworkId,
        batch_nonce: BatchNonce,
        message_nonce: MessageNonce,
    ) -> Self {
        MessageId {
            sender,
            receiver,
            batch_nonce: Some(batch_nonce),
            message_nonce,
        }
    }

    /// Creates MessageId for basic message.
    pub fn basic(
        sender: GenericNetworkId,
        receiver: GenericNetworkId,
        message_nonce: MessageNonce,
    ) -> Self {
        MessageId {
            sender,
            receiver,
            batch_nonce: None,
            message_nonce,
        }
    }

    pub fn hash(&self) -> H256 {
        crate::h256_from_sp_core(sp_runtime::traits::Keccak256::hash_of(self))
    }
}

pub type BatchNonce = u64;
pub type MessageNonce = u64;

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct AuxiliaryDigest {
    pub logs: Vec<AuxiliaryDigestItem>,
}

impl From<Digest> for AuxiliaryDigest {
    fn from(digest: Digest) -> Self {
        Self {
            logs: digest
                .logs
                .into_iter()
                .filter_map(|log| AuxiliaryDigestItem::try_from(log).ok())
                .collect::<Vec<_>>(),
        }
    }
}

/// Auxiliary [`DigestItem`] to include in header digest.
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum AuxiliaryDigestItem {
    /// A batch of messages has been committed.
    Commitment(GenericNetworkId, H256),
}

impl From<AuxiliaryDigestItem> for DigestItem {
    fn from(item: AuxiliaryDigestItem) -> DigestItem {
        DigestItem::Other(item.encode())
    }
}

impl TryFrom<DigestItem> for AuxiliaryDigestItem {
    type Error = codec::Error;
    fn try_from(value: DigestItem) -> Result<Self, Self::Error> {
        match value {
            DigestItem::Other(data) => Ok(Decode::decode(&mut &*data)?),
            _ => Err(codec::Error::from("wrong digest item kind")),
        }
    }
}

/// Modified leaf data for SORA
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
pub struct MmrLeaf<BlockNumber, Hash, MerkleRoot, DigestHash> {
    /// Version of the leaf format.
    ///
    /// Can be used to enable future format migrations and compatibility.
    /// See [`MmrLeafVersion`] documentation for details.
    pub version: MmrLeafVersion,
    /// Current block parent number and hash.
    pub parent_number_and_hash: (BlockNumber, Hash),
    /// A merkle root of the next BEEFY authority set.
    pub beefy_next_authority_set: BeefyNextAuthoritySet<MerkleRoot>,
    /// Digest hash of previous block (because digest for current block can be incomplete)
    pub digest_hash: DigestHash,
}

/// A type of asset registered on a bridge.
///
/// - Thischain: a Sora asset.
/// - Sidechain: token from another chain.
/// - Native: native token from another chain.
#[derive(
    Clone,
    Copy,
    Encode,
    Decode,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
    serde::Serialize,
    serde::Deserialize,
)]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum AssetKind {
    Thischain,
    Sidechain,
}

#[derive(
    Clone,
    Copy,
    RuntimeDebug,
    Encode,
    Decode,
    PartialEq,
    Eq,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum MessageStatus {
    InQueue,
    Committed,
    Done,
    Failed,
    Refunded,
    Approved,
}

#[derive(
    Clone,
    Copy,
    RuntimeDebug,
    Encode,
    Decode,
    PartialEq,
    Eq,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
/// Additional leaf data for MMR
pub struct LeafExtraData<Hash, RandomSeed> {
    /// This chain randomness which could be used in sidechain
    pub random_seed: RandomSeed,
    /// Commitments digest hash
    pub digest_hash: Hash,
}

#[derive(
    Clone,
    Copy,
    RuntimeDebug,
    Encode,
    Decode,
    PartialEq,
    Eq,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
/// Information about bridge asset which could be used by client applications
pub enum BridgeAssetInfo {
    /// Legacy HASHI bridge token
    #[cfg_attr(feature = "std", serde(rename = "evmLegacy"))]
    EVMLegacy(EVMLegacyAssetInfo),
    /// EVM network asset info
    #[cfg_attr(feature = "std", serde(rename = "evm"))]
    EVM(EVMAssetInfo),
    /// Substrate network asset info
    Sub(SubAssetInfo),
    Liberland,
    Ton(TonAssetInfo),
}

#[derive(
    Clone,
    Copy,
    RuntimeDebug,
    Encode,
    Decode,
    PartialEq,
    Eq,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum BridgeAppInfo {
    #[cfg_attr(feature = "std", serde(rename = "evm"))]
    EVM(GenericNetworkId, EVMAppInfo),
    /// There's only one app supported for substrate bridge
    Sub(GenericNetworkId),
    TON(GenericNetworkId, TonAppInfo),
}

#[derive(
    Clone,
    Copy,
    RuntimeDebug,
    Encode,
    Decode,
    Default,
    PartialEq,
    Eq,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct CallOriginOutput<NetworkId, MessageId, Additional> {
    pub network_id: NetworkId,
    pub message_id: MessageId,
    pub timepoint: GenericTimepoint,
    pub additional: Additional,
}

impl<NetworkId: Default, Additional: Default> crate::traits::BridgeOriginOutput
    for CallOriginOutput<NetworkId, H256, Additional>
{
    type NetworkId = NetworkId;
    type Additional = Additional;

    fn new(
        network_id: NetworkId,
        message_id: H256,
        timepoint: GenericTimepoint,
        additional: Additional,
    ) -> Self {
        Self {
            network_id,
            message_id,
            timepoint,
            additional,
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<Self, ()> {
        Ok(Self {
            network_id: Default::default(),
            message_id: Default::default(),
            timepoint: Default::default(),
            additional: Default::default(),
        })
    }
}

pub struct RawAssetInfo {
    pub name: Vec<u8>,
    pub symbol: Vec<u8>,
    pub precision: u8,
}

#[derive(Encode, Decode, scale_info::TypeInfo, codec::MaxEncodedLen, Derivative)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derivative(
    Debug(bound = "BlockNumber: core::fmt::Debug"),
    Clone(bound = "BlockNumber: Clone"),
    PartialEq(bound = "BlockNumber: PartialEq"),
    Eq(bound = "BlockNumber: Eq")
)]
#[scale_info(skip_type_params(MaxMessages, MaxPayload))]
#[cfg_attr(
    feature = "std",
    serde(bound(
        serialize = "BlockNumber: Serialize",
        deserialize = "BlockNumber: Deserialize<'de>"
    ))
)]
pub struct GenericCommitmentWithBlock<BlockNumber, MaxMessages: Get<u32>, MaxPayload: Get<u32>> {
    pub block_number: BlockNumber,
    pub commitment: crate::GenericCommitment<MaxMessages, MaxPayload>,
}

#[derive(
    Clone,
    Copy,
    RuntimeDebug,
    Encode,
    Decode,
    PartialEq,
    Eq,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
    Default,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum GenericAdditionalInboundData {
    #[default]
    Sub,
    EVM(AdditionalEVMInboundData),
    TON(AdditionalTONInboundData),
}

impl From<AdditionalEVMInboundData> for GenericAdditionalInboundData {
    fn from(value: AdditionalEVMInboundData) -> Self {
        Self::EVM(value)
    }
}

impl From<AdditionalTONInboundData> for GenericAdditionalInboundData {
    fn from(value: AdditionalTONInboundData) -> Self {
        Self::TON(value)
    }
}

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"trustless-evm-bridge";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
pub const TECH_ACCOUNT_FEES: &[u8] = b"fees";
pub const TECH_ACCOUNT_TREASURY_PREFIX: &[u8] = b"treasury";

macro_rules! impl_decode_with_mem_tracking {
    ($($ty:ty),* $(,)?) => {
        $(impl codec::DecodeWithMemTracking for $ty {})*
    };
}

impl_decode_with_mem_tracking!(
    MessageDirection,
    MessageId,
    AuxiliaryDigest,
    AuxiliaryDigestItem,
    AssetKind,
    MessageStatus,
    BridgeAssetInfo,
    BridgeAppInfo,
    GenericAdditionalInboundData,
);

impl<BlockNumber, Hash, MerkleRoot, DigestHash> codec::DecodeWithMemTracking
    for MmrLeaf<BlockNumber, Hash, MerkleRoot, DigestHash>
where
    BlockNumber: codec::Decode,
    Hash: codec::Decode,
    MerkleRoot: codec::Decode,
    DigestHash: codec::Decode,
{
}

impl<Hash, RandomSeed> codec::DecodeWithMemTracking for LeafExtraData<Hash, RandomSeed>
where
    Hash: codec::Decode,
    RandomSeed: codec::Decode,
{
}

impl<NetworkId, MessageId, Additional> codec::DecodeWithMemTracking
    for CallOriginOutput<NetworkId, MessageId, Additional>
where
    NetworkId: codec::Decode,
    MessageId: codec::Decode,
    Additional: codec::Decode,
{
}

impl<BlockNumber, MaxMessages: Get<u32>, MaxPayload: Get<u32>> codec::DecodeWithMemTracking
    for GenericCommitmentWithBlock<BlockNumber, MaxMessages, MaxPayload>
where
    BlockNumber: codec::Decode,
{
}
