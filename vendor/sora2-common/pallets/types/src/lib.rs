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

#![cfg_attr(not(feature = "std"), no_std)]

pub mod channel_abi;
pub mod evm;
pub mod substrate;
#[cfg(any(feature = "test", test))]
pub mod test_utils;
pub mod ton;
pub mod traits;
pub mod types;
pub mod utils;

use codec::{Decode, Encode};
use derivative::Derivative;
pub use ethereum_types::{H128, H64};
pub use log::Log;
use serde::{Deserialize, Serialize};
pub use sp_core::{H160, H256, H512, U256};
use sp_core::{Get, RuntimeDebug};
use staging_xcm as xcm;
use ton::{TonAddress, TonBalance, TonNetworkId, TonTransactionId};

#[derive(Debug)]
pub enum DecodeError {
    // Unexpected RLP data
    InvalidRLP(rlp::DecoderError),
    // Data does not match expected ABI
    InvalidABI(ethabi::Error),
    // Invalid message payload
    InvalidPayload,
}

impl From<rlp::DecoderError> for DecodeError {
    fn from(err: rlp::DecoderError) -> Self {
        DecodeError::InvalidRLP(err)
    }
}

impl From<ethabi::Error> for DecodeError {
    fn from(err: ethabi::Error) -> Self {
        DecodeError::InvalidABI(err)
    }
}

pub type EVMChainId = H256;
pub type Address = H160;
pub type VersionedMultiLocation = xcm::VersionedLocation;

#[inline]
pub fn h256_from_sp_core(hash: sp_core::H256) -> H256 {
    hash
}

#[derive(
    Encode,
    Decode,
    Copy,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
    Default,
    Serialize,
    Deserialize,
)]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum SubNetworkId {
    #[default]
    Mainnet,
    Kusama,
    Polkadot,
    Rococo,
    Alphanet,
    Liberland,
}

#[derive(
    Encode,
    Decode,
    Copy,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum GenericNetworkId {
    // deserializes value either from hex or decimal
    #[cfg_attr(feature = "std", serde(rename = "evm"))]
    EVM(EVMChainId),
    Sub(SubNetworkId),
    #[cfg_attr(feature = "std", serde(rename = "evmLegacy"))]
    EVMLegacy(u32),
    TON(TonNetworkId),
}

impl Default for GenericNetworkId {
    fn default() -> Self {
        Self::Sub(Default::default())
    }
}

impl GenericNetworkId {
    pub fn evm(self) -> Option<EVMChainId> {
        match self {
            Self::EVM(chain_id) => Some(chain_id),
            _ => None,
        }
    }

    pub fn sub(self) -> Option<SubNetworkId> {
        match self {
            Self::Sub(network_id) => Some(network_id),
            _ => None,
        }
    }

    pub fn evm_legacy(self) -> Option<u32> {
        match self {
            Self::EVMLegacy(network_id) => Some(network_id),
            _ => None,
        }
    }
}

impl From<EVMChainId> for GenericNetworkId {
    fn from(id: EVMChainId) -> Self {
        GenericNetworkId::EVM(id)
    }
}

impl From<SubNetworkId> for GenericNetworkId {
    fn from(id: SubNetworkId) -> Self {
        GenericNetworkId::Sub(id)
    }
}

impl From<TonNetworkId> for GenericNetworkId {
    fn from(id: TonNetworkId) -> Self {
        GenericNetworkId::TON(id)
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub enum GenericAccount {
    EVM(H160),
    Sora(MainnetAccountId),
    Liberland(MainnetAccountId),
    Parachain(VersionedMultiLocation),
    Unknown,
    Root,
    TON(TonAddress),
}

impl TryInto<MainnetAccountId> for GenericAccount {
    type Error = ();
    fn try_into(self) -> Result<MainnetAccountId, Self::Error> {
        match self {
            GenericAccount::Sora(a) => Ok(a),
            GenericAccount::Liberland(a) => Ok(a),
            _ => Err(()),
        }
    }
}

#[derive(
    Encode,
    Decode,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Debug,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
    Default,
)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum GenericTimepoint {
    #[cfg_attr(feature = "std", serde(rename = "evm"))]
    EVM(u64),
    Sora(u32),
    Parachain(u32),
    Pending,
    #[default]
    Unknown,
    TON(TonTransactionId),
}

#[derive(Encode, Decode, scale_info::TypeInfo, codec::MaxEncodedLen, Derivative)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derivative(
    Debug(bound = ""),
    Clone(bound = ""),
    PartialEq(bound = ""),
    Eq(bound = "")
)]
#[scale_info(skip_type_params(MaxMessages, MaxPayload))]
#[cfg_attr(feature = "std", serde(bound = ""))]
pub enum GenericCommitment<MaxMessages: Get<u32>, MaxPayload: Get<u32>> {
    Sub(substrate::Commitment<MaxMessages, MaxPayload>),
    #[cfg_attr(feature = "std", serde(rename = "evm"))]
    EVM(evm::Commitment<MaxMessages, MaxPayload>),
    #[cfg_attr(feature = "std", serde(rename = "ton"))]
    TON(ton::Commitment<MaxPayload>),
}

impl<MaxMessages: Get<u32>, MaxPayload: Get<u32>> GenericCommitment<MaxMessages, MaxPayload> {
    pub fn hash(&self) -> H256 {
        match self {
            GenericCommitment::Sub(commitment) => commitment.hash(),
            GenericCommitment::EVM(commitment) => commitment.hash(),
            GenericCommitment::TON(commitment) => h256_from_sp_core(commitment.hash()),
        }
    }

    pub fn nonce(&self) -> u64 {
        match self {
            GenericCommitment::Sub(commitment) => commitment.nonce,
            GenericCommitment::EVM(commitment) => commitment.nonce(),
            GenericCommitment::TON(commitment) => commitment.nonce(),
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum GenericAssetId {
    Sora(MainnetAssetId),
    XCM(substrate::ParachainAssetId),
    EVM(H160),
    Liberland(LiberlandAssetId),
}

impl TryInto<LiberlandAssetId> for GenericAssetId {
    type Error = ();

    fn try_into(self) -> Result<LiberlandAssetId, Self::Error> {
        match self {
            GenericAssetId::Liberland(b) => Ok(b),
            _ => Err(()),
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub enum GenericBalance {
    Substrate(MainnetBalance),
    /// EVM ABI uses big endian for integers, but scale codec uses little endian
    EVM(H256),
    TON(TonBalance),
}

impl TryInto<MainnetBalance> for GenericBalance {
    type Error = ();

    fn try_into(self) -> Result<MainnetBalance, Self::Error> {
        match self {
            GenericBalance::Substrate(b) => Ok(b),
            _ => Err(()),
        }
    }
}

// Use predefined types to ensure data compatability

pub type MainnetAssetId = H256;

pub type MainnetAccountId = sp_runtime::AccountId32;

pub type MainnetBalance = u128;

#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum LiberlandAssetId {
    LLD,
    Asset(u32),
}

impl From<u32> for LiberlandAssetId {
    fn from(value: u32) -> Self {
        LiberlandAssetId::Asset(value)
    }
}

#[derive(Encode, Decode, scale_info::TypeInfo, codec::MaxEncodedLen, Derivative)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derivative(
    Debug(bound = ""),
    Clone(bound = ""),
    PartialEq(bound = ""),
    Eq(bound = "")
)]
#[scale_info(skip_type_params(MaxPayload))]
#[cfg_attr(feature = "std", serde(bound = ""))]
pub enum GenericBridgeMessage<MaxPayload: Get<u32>> {
    Sub(substrate::BridgeMessage<MaxPayload>),
    EVM(evm::Message<MaxPayload>),
}

impl<N: Get<u32>> GenericBridgeMessage<N> {
    pub fn payload(&self) -> &[u8] {
        match self {
            GenericBridgeMessage::Sub(message) => &message.payload,
            GenericBridgeMessage::EVM(message) => &message.payload,
        }
    }
}

macro_rules! impl_decode_with_mem_tracking {
    ($($ty:ty),* $(,)?) => {
        $(impl codec::DecodeWithMemTracking for $ty {})*
    };
}

impl_decode_with_mem_tracking!(
    SubNetworkId,
    GenericNetworkId,
    GenericAccount,
    GenericTimepoint,
    GenericAssetId,
    GenericBalance,
    LiberlandAssetId,
);

impl<MaxMessages: Get<u32>, MaxPayload: Get<u32>> codec::DecodeWithMemTracking
    for GenericCommitment<MaxMessages, MaxPayload>
{
}

impl<MaxPayload: Get<u32>> codec::DecodeWithMemTracking for GenericBridgeMessage<MaxPayload> {}
