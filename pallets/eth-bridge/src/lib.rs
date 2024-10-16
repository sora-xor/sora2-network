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

/*!
# Multi-network Ethereum Bridge pallet

## Description
Provides functionality for cross-chain transfers of assets between Sora and Ethereum-based networks.

## Overview
Bridge can be divided into 3 main parts:
1. Bridge pallet. A Substrate pallet (this one).
2. Middle-layer. A part of the bridge pallet, more precisely, off-chain workers.
3. [Bridge contracts](https://github.com/sora-xor/sora2-evm-contracts). A set of smart-contracts
deployed on Ethereum-based networks.

## Definitions
_Thischain_ - working chain/network.
_Sidechain_ - external chain/network.
_Network_ - an ethereum-based network with a bridge contract.

## Bridge pallet
Stores basic information about networks: peers' accounts, requests and registered assets/tokens.
Networks can be added and managed through requests. Requests can be [incoming](`IncomingRequest`)
(came from sidechain) or [outgoing](`OutgoingRequest`) (to sidechain). Each request has it's own
hash (differs from extrinsic hash), status (`RequestStatus`), some specific data and additional information.
The requests life-cycle consists of 3 stages: validation, preparation and finalization.
Requests are registered by accounts and finalized by _bridge peers_.

## Middle-layer
Works through off-chain workers. Any substrate node can be a bridge peer with its own
secret key (differs from validator's key) and participate in bridge consensus (after election).
The bridge peer set (`Peers` in storage) forms an n-of-m-multisignature account (`BridgeAccount`
in storage), which is used to finalize all requests.

## Bridge contract
Persists the same multi-sig account (+- 1 signatory) for validating all its incoming requests.
*/

#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

#[macro_use]
extern crate alloc;
extern crate jsonrpc_core as jsonrpc;

use crate::offchain::SignatureParams;
use crate::util::majority;
use alloc::string::String;
use bridge_types::traits::BridgeApp;
use bridge_types::GenericNetworkId;
use codec::{Decode, Encode};
use common::prelude::Balance;
use common::{
    AssetInfoProvider, AssetName, AssetSymbol, BalancePrecision, DEFAULT_BALANCE_PRECISION,
};
use core::stringify;
use frame_support::dispatch::DispatchResult;
use frame_support::sp_runtime::app_crypto::{ecdsa, sp_core};
use frame_support::sp_runtime::offchain::storage::StorageValueRef;
use frame_support::sp_runtime::offchain::storage_lock::{StorageLock, Time};
use frame_support::sp_runtime::traits::{
    AtLeast32Bit, BlockNumberProvider, MaybeSerializeDeserialize, Member, One, UniqueSaturatedInto,
};
use frame_support::sp_runtime::KeyTypeId;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, fail, Parameter};
use frame_system::offchain::{AppCrypto, CreateSignedTransaction};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::pallet_prelude::OriginFor;
use frame_system::{ensure_root, ensure_signed};
use hex_literal::hex;
use log::{debug, error, info, warn};
pub use pallet::*;
use permissions::{Scope, BURN, MINT};
use requests::*;
use serde::{Deserialize, Serialize};
use sp_core::{RuntimeDebug, H160, H256};
use sp_runtime::DispatchError;
use sp_std::borrow::Cow;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::fmt::{self, Debug};
use sp_std::marker::PhantomData;
use sp_std::prelude::*;
#[cfg(feature = "std")]
use std::collections::HashMap;
pub use weights::WeightInfo;

type EthAddress = H160;

pub mod weights;

mod benchmarking;
mod contract;
mod macros;
pub mod migration;
pub mod offchain;
pub mod requests;
mod rpc;
#[cfg(test)]
mod tests;
pub mod types;
mod util;

/// Substrate node RPC URL.
const SUB_NODE_URL: &str = "http://127.0.0.1:9954";
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 10;
/// Substrate maximum amount of blocks for which an extrinsic is expecting to be finalized.
const SUBSTRATE_MAX_BLOCK_NUM_EXPECTING_UNTIL_FINALIZATION: u32 = 50;
/// Maximum substrate blocks can be handled during single offchain procedure.
const SUBSTRATE_HANDLE_BLOCK_COUNT_PER_BLOCK: u32 = 20;
#[cfg(not(test))]
const MAX_FAILED_SEND_SIGNED_TX_RETRIES: u16 = 2000;
#[cfg(test)]
const MAX_FAILED_SEND_SIGNED_TX_RETRIES: u16 = 10;
const MAX_SUCCESSFUL_SENT_SIGNED_TX_PER_ONCE: u8 = 5;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"bridge";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
pub const TECH_ACCOUNT_AUTHORITY: &[u8] = b"authority";

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ethb");
/// A number of sidechain blocks needed to consider transaction as confirmed.
pub const CONFIRMATION_INTERVAL: u64 = 30;
/// Maximum number of `Log` items per `eth_getLogs` request.
pub const MAX_GET_LOGS_ITEMS: u64 = 50;

// Off-chain worker storage paths.
pub const STORAGE_SUB_NODE_URL_KEY: &[u8] = b"eth-bridge-ocw::sub-node-url";
pub const STORAGE_PEER_SECRET_KEY: &[u8] = b"eth-bridge-ocw::secret-key";
pub const STORAGE_ETH_NODE_PARAMS: &str = "eth-bridge-ocw::node-params";
pub const STORAGE_NETWORK_IDS_KEY: &[u8] = b"eth-bridge-ocw::network-ids";
pub const STORAGE_PENDING_TRANSACTIONS_KEY: &[u8] = b"eth-bridge-ocw::pending-transactions";
pub const STORAGE_FAILED_PENDING_TRANSACTIONS_KEY: &[u8] =
    b"eth-bridge-ocw::failed-pending-transactions";
pub const STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY: &[u8] =
    b"eth-bridge-ocw::sub-to-handle-from-height";

/// Contract's `Deposit(bytes32,uint256,address,bytes32)` event topic.
pub const DEPOSIT_TOPIC: H256 = H256(hex!(
    "85c0fa492ded927d3acca961da52b0dda1debb06d8c27fe189315f06bb6e26c8"
));
pub const OFFCHAIN_TRANSACTION_WEIGHT_LIMIT: Weight =
    Weight::from_parts(10_000_000_000_000_000u64, 10_000_000_000_000_000u64);
const MAX_PENDING_TX_BLOCKS_PERIOD: u32 = 100;
const RE_HANDLE_TXS_PERIOD: u32 = 200;
/// Minimum peers required to start bridge migration
pub const MINIMUM_PEERS_FOR_MIGRATION: usize = 3;

type AssetIdOf<T> = <T as assets::Config>::AssetId;
// type Timepoint<T> = bridge_multisig::BridgeTimepoint<<T as frame_system::Config>::BlockNumber>;
type Timepoint<T> = bridge_multisig::BridgeTimepoint<BlockNumberFor<T>>;
type BridgeTimepoint<T> = Timepoint<T>;
type BridgeNetworkId<T> = <T as Config>::NetworkId;

/// Ethereum node parameters (url, credentials).
#[derive(
    Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, RuntimeDebug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct NodeParams {
    url: String,
    credentials: Option<String>,
}

/// Local peer config. Contains a set of networks that the peer is responsible for.
#[cfg(feature = "std")]
#[derive(Clone, RuntimeDebug, Serialize, Deserialize)]
pub struct PeerConfig<NetworkId: std::hash::Hash + Eq> {
    pub networks: HashMap<NetworkId, NodeParams>,
}

/// Network-specific parameters.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct NetworkParams<AccountId: Ord> {
    pub bridge_contract_address: EthAddress,
    pub initial_peers: BTreeSet<AccountId>,
}

/// Network configuration.
#[derive(
    Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo, Serialize, Deserialize,
)]
#[scale_info(skip_type_params(T))]
pub struct NetworkConfig<T: Config> {
    pub initial_peers: BTreeSet<T::AccountId>,
    pub bridge_account_id: T::AccountId,
    pub assets: Vec<AssetConfig<T::AssetId>>,
    pub bridge_contract_address: EthAddress,
    pub reserves: Vec<(T::AssetId, Balance)>,
}

#[derive(Clone, Encode)]
pub struct BridgeAssetData<T: Config> {
    pub asset_id: T::AssetId,
    pub name: AssetName,
    pub symbol: AssetSymbol,
    pub sidechain_precision: BalancePrecision,
    pub sidechain_asset_id: sp_core::H160,
}

impl<T: Config> BridgeAssetData<T> {
    pub fn new(
        name: &'static str,
        symbol: &'static str,
        sidechain_precision: BalancePrecision,
        sidechain_asset_id: sp_core::H160,
    ) -> Self {
        let name: Cow<'_, str> = if name.contains(".") {
            name.replacen(".", " ", 2).into()
        } else {
            name.into()
        };
        let mut data = Self {
            asset_id: T::AssetId::from(H256::zero()),
            name: AssetName(name.as_bytes().to_vec()),
            symbol: AssetSymbol(symbol.as_bytes().to_vec()),
            sidechain_precision,
            sidechain_asset_id,
        };
        let asset_id =
            assets::Pallet::<T>::gen_asset_id_from_any(&("BridgeAssetData", data.clone()));
        data.asset_id = asset_id;
        data
    }
}

/// Bridge status.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
pub enum BridgeStatus {
    Initialized,
    Migrating,
}

impl Default for BridgeStatus {
    fn default() -> Self {
        Self::Initialized
    }
}

/// Bridge asset parameters.
#[derive(
    Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo, Serialize, Deserialize,
)]
pub enum AssetConfig<AssetId> {
    Thischain {
        id: AssetId,
    },
    Sidechain {
        id: AssetId,
        sidechain_id: sp_core::H160,
        owned: bool,
        precision: BalancePrecision,
    },
}

impl<AssetId> AssetConfig<AssetId> {
    pub fn sidechain(
        id: AssetId,
        sidechain_id: sp_core::H160,
        precision: BalancePrecision,
    ) -> Self {
        Self::Sidechain {
            id,
            sidechain_id,
            owned: false,
            precision,
        }
    }

    pub fn asset_id(&self) -> &AssetId {
        match self {
            AssetConfig::Thischain { id, .. } => id,
            AssetConfig::Sidechain { id, .. } => id,
        }
    }

    pub fn kind(&self) -> AssetKind {
        match self {
            AssetConfig::Thischain { .. } => AssetKind::Thischain,
            AssetConfig::Sidechain { owned: false, .. } => AssetKind::Sidechain,
            AssetConfig::Sidechain { owned: true, .. } => AssetKind::SidechainOwned,
        }
    }
}

/// Bridge function signature version
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub enum BridgeSignatureVersion {
    V1,
    // Fix signature overlapping for addPeer, removePeer and prepareForMigration
    // Add bridge contract address to the signature
    V2,
    // Use abi.encode instead of abi.encodePacked and add prefix for each function
    V3,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::offchain::SignatureParams;
    use crate::util::get_bridge_account;
    use bridge_types::traits::{BridgeAssetLockChecker, MessageStatusNotifier};
    use codec::Codec;
    use common::prelude::constants::EXTRINSIC_FIXED_WEIGHT;
    use common::weights::{err_pays_no, pays_no, pays_no_with_maybe_weight};
    use common::{ContentSource, Description};
    use frame_support::pallet_prelude::*;
    use frame_support::sp_runtime;
    use frame_support::traits::{GetCallMetadata, StorageVersion};
    use frame_support::transactional;
    use frame_support::weights::WeightToFeePolynomial;
    use frame_system::RawOrigin;
    use log;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + CreateSignedTransaction<Call<Self>>
        + CreateSignedTransaction<bridge_multisig::Call<Self>>
        + assets::Config
        + permissions::Config
        + bridge_multisig::Config<RuntimeCall = <Self as Config>::RuntimeCall>
        + fmt::Debug
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The identifier type for an offchain worker.
        type PeerId: AppCrypto<Self::Public, Self::Signature>;
        /// The overarching dispatch call type.
        type RuntimeCall: From<Call<Self>>
            + From<bridge_multisig::Call<Self>>
            + Codec
            + Clone
            + GetCallMetadata;
        /// Sidechain network ID.
        type NetworkId: Parameter
            + Member
            + AtLeast32Bit
            + Copy
            + MaybeSerializeDeserialize
            + Ord
            + Default
            + Debug;
        type GetEthNetworkId: Get<Self::NetworkId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        #[cfg(test)]
        type Mock: tests::mock::Mock;

        type MessageStatusNotifier: MessageStatusNotifier<Self::AssetId, Self::AccountId, Balance>;

        type BridgeAssetLockChecker: BridgeAssetLockChecker<Self::AssetId, Balance>;

        type WeightToFee: WeightToFeePolynomial<Balance = Balance>;

        /// To retrieve asset info
        type AssetInfoProvider: AssetInfoProvider<
            Self::AssetId,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
    where
        T: CreateSignedTransaction<<T as Config>::RuntimeCall>,
    {
        /// Main off-chain worker procedure.
        ///
        /// Note: only one worker is expected to be used.
        fn offchain_worker(block_number: BlockNumberFor<T>) {
            debug!("Entering off-chain workers {:?}", block_number);
            let value_ref = StorageValueRef::persistent(STORAGE_PEER_SECRET_KEY);
            if value_ref.get::<Vec<u8>>().ok().flatten().is_none() {
                debug!("Peer secret key not found. Skipping off-chain procedure.");
                return;
            }

            let mut lock = StorageLock::<'_, Time>::with_deadline(
                b"eth-bridge-ocw::lock",
                sp_core::offchain::Duration::from_millis(100000),
            );
            let guard = lock.try_lock();
            if let Ok(_guard) = guard {
                Self::offchain();
            } else {
                log::debug!("Skip worker {:?}", block_number);
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a new bridge.
        ///
        /// Parameters:
        /// - `bridge_contract_address` - address of smart-contract deployed on a corresponding
        /// network.
        /// - `initial_peers` - a set of initial network peers.
        #[transactional]
        #[pallet::call_index(0)]
        // eth-bridge pallet will be deprecated soon, so we set const weight here
        #[pallet::weight(EXTRINSIC_FIXED_WEIGHT)]
        pub fn register_bridge(
            origin: OriginFor<T>,
            bridge_contract_address: EthAddress,
            initial_peers: Vec<T::AccountId>,
            signature_version: BridgeSignatureVersion,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let net_id = NextNetworkId::<T>::get();
            ensure!(!initial_peers.is_empty(), Error::<T>::NotEnoughPeers);
            let peers_account_id = bridge_multisig::Pallet::<T>::register_multisig_inner(
                initial_peers[0].clone(),
                initial_peers.clone(),
            )?;
            BridgeContractAddress::<T>::insert(net_id, bridge_contract_address);
            BridgeAccount::<T>::insert(net_id, peers_account_id);
            BridgeStatuses::<T>::insert(net_id, BridgeStatus::Initialized);
            BridgeSignatureVersions::<T>::insert(net_id, signature_version);
            Peers::<T>::insert(net_id, initial_peers.into_iter().collect::<BTreeSet<_>>());
            NextNetworkId::<T>::set(net_id + T::NetworkId::one());
            Ok(().into())
        }

        /// Add a Thischain asset to the bridge whitelist.
        ///
        /// Can only be called by root.
        ///
        /// Parameters:
        /// - `asset_id` - Thischain asset identifier.
        /// - `network_id` - network identifier to which the asset should be added.
        #[transactional]
        #[pallet::call_index(1)]
        // eth-bridge pallet will be deprecated soon, so we set const weight here
        #[pallet::weight(EXTRINSIC_FIXED_WEIGHT)]
        pub fn add_asset(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let from = Self::authority_account().ok_or(Error::<T>::AuthorityAccountNotSet)?;
            let nonce = frame_system::Pallet::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
            Self::add_request(&OffchainRequest::outgoing(OutgoingRequest::AddAsset(
                OutgoingAddAsset {
                    author: from.clone(),
                    asset_id,
                    nonce,
                    network_id,
                    timepoint,
                },
            )))?;
            frame_system::Pallet::<T>::inc_account_nonce(&from);
            Ok(().into())
        }

        /// Add a Sidechain token to the bridge whitelist.
        ///
        /// Parameters:
        /// - `token_address` - token contract address.
        /// - `symbol` - token symbol (ticker).
        /// - `name` - token name.
        /// - `decimals` -  token precision.
        /// - `network_id` - network identifier.
        #[transactional]
        #[pallet::call_index(2)]
        // eth-bridge pallet will be deprecated soon, so we set const weight here
        #[pallet::weight(EXTRINSIC_FIXED_WEIGHT)]
        pub fn add_sidechain_token(
            origin: OriginFor<T>,
            token_address: EthAddress,
            symbol: String,
            name: String,
            decimals: u8,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug!("called add_sidechain_token");
            ensure_root(origin)?;
            let from = Self::authority_account().ok_or(Error::<T>::AuthorityAccountNotSet)?;
            let nonce = frame_system::Pallet::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
            Self::add_request(&OffchainRequest::outgoing(OutgoingRequest::AddToken(
                OutgoingAddToken {
                    author: from.clone(),
                    token_address,
                    symbol,
                    name,
                    decimals,
                    nonce,
                    network_id,
                    timepoint,
                },
            )))?;
            frame_system::Pallet::<T>::inc_account_nonce(&from);
            Ok(().into())
        }

        /// Transfer some amount of the given asset to Sidechain address.
        ///
        /// Note: if the asset kind is `Sidechain`, the amount should fit in the asset's precision
        /// on sidechain (`SidechainAssetPrecision`) without extra digits. For example, assume
        /// some ERC-20 (`T`) token has `decimals=6`, and the corresponding asset on substrate has
        /// `7`. Alice's balance on thischain is `0.1000009`. If Alice would want to transfer all
        /// the amount, she will get an error `NonZeroDust`, because of the `9` at the end, so, the
        /// correct amount would be `0.100000` (only 6 digits after the decimal point).
        ///
        /// Parameters:
        /// - `asset_id` - thischain asset id.
        /// - `to` - sidechain account id.
        /// - `amount` - amount of the asset.
        /// - `network_id` - network identifier.
        #[transactional]
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::transfer_to_sidechain())]
        pub fn transfer_to_sidechain(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            to: EthAddress,
            amount: Balance,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug!("called transfer_to_sidechain");
            let from = ensure_signed(origin)?;
            let nonce = frame_system::Pallet::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
            Self::add_request(&OffchainRequest::outgoing(OutgoingRequest::Transfer(
                OutgoingTransfer {
                    from: from.clone(),
                    to,
                    asset_id,
                    amount,
                    nonce,
                    network_id,
                    timepoint,
                },
            )))?;
            frame_system::Pallet::<T>::inc_account_nonce(&from);
            Ok(().into())
        }

        /// Load incoming request from Sidechain by the given transaction hash.
        ///
        /// Parameters:
        /// - `eth_tx_hash` - transaction hash on Sidechain.
        /// - `kind` - incoming request type.
        /// - `network_id` - network identifier.

        #[transactional]
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::request_from_sidechain())]
        pub fn request_from_sidechain(
            origin: OriginFor<T>,
            eth_tx_hash: H256,
            kind: IncomingRequestKind,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug!("called request_from_sidechain");
            let from = ensure_signed(origin)?;
            let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
            match kind {
                IncomingRequestKind::Transaction(kind) => {
                    Self::add_request(&OffchainRequest::LoadIncoming(
                        LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
                            from,
                            eth_tx_hash,
                            timepoint,
                            kind,
                            network_id,
                        )),
                    ))?;
                    Ok(().into())
                }
                IncomingRequestKind::Meta(kind) => {
                    if kind == IncomingMetaRequestKind::CancelOutgoingRequest {
                        fail!(Error::<T>::Unavailable);
                    }
                    let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
                    Self::add_request(&OffchainRequest::load_incoming_meta(
                        LoadIncomingMetaRequest::new(
                            from,
                            eth_tx_hash,
                            timepoint,
                            kind,
                            network_id,
                        ),
                    ))?;
                    Ok(().into())
                }
            }
        }

        /// Finalize incoming request (see `Pallet::finalize_incoming_request_inner`).
        ///
        /// Can be only called from a bridge account.
        ///
        /// Parameters:
        /// - `request` - an incoming request.
        /// - `network_id` - network identifier.
        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::finalize_incoming_request())]
        pub fn finalize_incoming_request(
            origin: OriginFor<T>,
            hash: H256,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug!("called finalize_incoming_request");
            let _ = Self::ensure_bridge_account(origin, network_id)?;
            let request = Requests::<T>::get(network_id, &hash)
                .ok_or_else(|| err_pays_no(Error::<T>::UnknownRequest))?;
            let (request, hash) = request
                .as_incoming()
                .ok_or_else(|| err_pays_no(Error::<T>::ExpectedIncomingRequest))?;
            pays_no(Self::finalize_incoming_request_inner(
                request, hash, network_id,
            ))
        }

        /// Add a new peer to the bridge peers set.
        ///
        /// Parameters:
        /// - `account_id` - account id on thischain.
        /// - `address` - account id on sidechain.
        /// - `network_id` - network identifier.

        #[transactional]
        #[pallet::call_index(6)]
        // eth-bridge pallet will be deprecated soon, so we set const weight here
        #[pallet::weight(EXTRINSIC_FIXED_WEIGHT)]
        pub fn add_peer(
            origin: OriginFor<T>,
            account_id: T::AccountId,
            address: EthAddress,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug!("called change_peers_out");
            ensure_root(origin)?;
            let from = Self::authority_account().ok_or(Error::<T>::AuthorityAccountNotSet)?;
            let nonce = frame_system::Pallet::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
            Self::add_request(&OffchainRequest::outgoing(OutgoingRequest::AddPeer(
                OutgoingAddPeer {
                    author: from.clone(),
                    peer_account_id: account_id.clone(),
                    peer_address: address,
                    nonce,
                    network_id,
                    timepoint,
                },
            )))?;
            frame_system::Pallet::<T>::inc_account_nonce(&from);
            if network_id == T::GetEthNetworkId::get() {
                let nonce = frame_system::Pallet::<T>::account_nonce(&from);
                Self::add_request(&OffchainRequest::outgoing(OutgoingRequest::AddPeerCompat(
                    OutgoingAddPeerCompat {
                        author: from.clone(),
                        peer_account_id: account_id,
                        peer_address: address,
                        nonce,
                        network_id,
                        timepoint,
                    },
                )))?;
                frame_system::Pallet::<T>::inc_account_nonce(&from);
            }
            Ok(().into())
        }

        /// Remove peer from the the bridge peers set.
        ///
        /// Parameters:
        /// - `account_id` - account id on thischain.
        /// - `network_id` - network identifier.

        #[transactional]
        #[pallet::call_index(7)]
        // eth-bridge pallet will be deprecated soon, so we set const weight here
        #[pallet::weight(EXTRINSIC_FIXED_WEIGHT)]
        pub fn remove_peer(
            origin: OriginFor<T>,
            account_id: T::AccountId,
            peer_address: Option<EthAddress>,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug!("called change_peers_out");
            ensure_root(origin)?;
            let from = Self::authority_account().ok_or(Error::<T>::AuthorityAccountNotSet)?;
            let peer_address = if PeerAddress::<T>::contains_key(network_id, &account_id) {
                if let Some(peer_address) = peer_address {
                    ensure!(
                        peer_address == Self::peer_address(network_id, &account_id),
                        Error::<T>::UnknownPeerId
                    );
                    peer_address
                } else {
                    Self::peer_address(network_id, &account_id)
                }
            } else {
                peer_address.ok_or(Error::<T>::UnknownPeerId)?
            };
            let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
            let compat_hash = if network_id == T::GetEthNetworkId::get() {
                let nonce = frame_system::Pallet::<T>::account_nonce(&from);
                let request = OffchainRequest::outgoing(OutgoingRequest::RemovePeerCompat(
                    OutgoingRemovePeerCompat {
                        author: from.clone(),
                        peer_account_id: account_id.clone(),
                        peer_address,
                        nonce,
                        network_id,
                        timepoint,
                    },
                ));
                Self::add_request(&request)?;
                frame_system::Pallet::<T>::inc_account_nonce(&from);
                Some(request.hash())
            } else {
                None
            };
            let nonce = frame_system::Pallet::<T>::account_nonce(&from);
            Self::add_request(&OffchainRequest::outgoing(OutgoingRequest::RemovePeer(
                OutgoingRemovePeer {
                    author: from.clone(),
                    peer_account_id: account_id,
                    peer_address,
                    nonce,
                    network_id,
                    timepoint,
                    compat_hash,
                },
            )))?;
            frame_system::Pallet::<T>::inc_account_nonce(&from);
            Ok(().into())
        }

        /// Prepare the given bridge for migration.
        ///
        /// Can only be called by an authority.
        ///
        /// Parameters:
        /// - `network_id` - bridge network identifier.

        #[transactional]
        #[pallet::call_index(8)]
        // eth-bridge pallet will be deprecated soon, so we set const weight here
        #[pallet::weight(EXTRINSIC_FIXED_WEIGHT)]
        pub fn prepare_for_migration(
            origin: OriginFor<T>,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug!("called prepare_for_migration");
            ensure_root(origin)?;
            if BridgeSignatureVersions::<T>::get(network_id) == BridgeSignatureVersion::V1
                && Peers::<T>::get(network_id).len() < MINIMUM_PEERS_FOR_MIGRATION
            {
                return Err(Error::<T>::UnsafeMigration.into());
            }
            let from = Self::authority_account().ok_or(Error::<T>::AuthorityAccountNotSet)?;
            let nonce = frame_system::Pallet::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
            Self::add_request(&OffchainRequest::outgoing(
                OutgoingRequest::PrepareForMigration(OutgoingPrepareForMigration {
                    author: from.clone(),
                    nonce,
                    network_id,
                    timepoint,
                }),
            ))?;
            frame_system::Pallet::<T>::inc_account_nonce(&from);
            Ok(().into())
        }

        /// Migrate the given bridge to a new smart-contract.
        ///
        /// Can only be called by an authority.
        ///
        /// Parameters:
        /// - `new_contract_address` - new sidechain ocntract address.
        /// - `erc20_native_tokens` - migrated assets ids.
        /// - `network_id` - bridge network identifier.

        #[transactional]
        #[pallet::call_index(9)]
        // eth-bridge pallet will be deprecated soon, so we set const weight here
        #[pallet::weight(EXTRINSIC_FIXED_WEIGHT)]
        pub fn migrate(
            origin: OriginFor<T>,
            new_contract_address: EthAddress,
            erc20_native_tokens: Vec<EthAddress>,
            network_id: BridgeNetworkId<T>,
            new_signature_version: BridgeSignatureVersion,
        ) -> DispatchResultWithPostInfo {
            debug!("called prepare_for_migration");
            ensure_root(origin)?;
            let from = Self::authority_account().ok_or(Error::<T>::AuthorityAccountNotSet)?;
            let nonce = frame_system::Pallet::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
            Self::add_request(&OffchainRequest::outgoing(OutgoingRequest::Migrate(
                OutgoingMigrate {
                    author: from.clone(),
                    new_contract_address,
                    erc20_native_tokens,
                    nonce,
                    network_id,
                    timepoint,
                    new_signature_version,
                },
            )))?;
            frame_system::Pallet::<T>::inc_account_nonce(&from);
            Ok(().into())
        }

        /// Register the given incoming request and add it to the queue.
        ///
        /// Calls `validate` and `prepare` on the request, adds it to the queue and maps it with the
        /// corresponding load-incoming-request and removes the load-request from the queue.
        ///
        /// Can only be called by a bridge account.
        #[pallet::call_index(10)]
        #[pallet::weight(<T as Config>::WeightInfo::register_incoming_request())]
        pub fn register_incoming_request(
            origin: OriginFor<T>,
            incoming_request: IncomingRequest<T>,
        ) -> DispatchResultWithPostInfo {
            debug!("called register_incoming_request");
            let net_id = incoming_request.network_id();
            let _ = Self::ensure_bridge_account(origin, net_id)?;
            pays_no(Self::register_incoming_request_inner(
                &OffchainRequest::incoming(incoming_request),
                net_id,
            ))
        }

        /// Import the given incoming request.
        ///
        /// Register's the load request, then registers and finalizes the incoming request if it
        /// succeeded, otherwise aborts the load request.
        ///
        /// Can only be called by a bridge account.
        #[pallet::call_index(11)]
        #[pallet::weight({
            <T as Config>::WeightInfo::register_incoming_request().saturating_add(
                if incoming_request_result.is_ok() {
                    <T as Config>::WeightInfo::finalize_incoming_request()
                } else {
                    <T as Config>::WeightInfo::abort_request()
                }
            )
        })]
        pub fn import_incoming_request(
            origin: OriginFor<T>,
            load_incoming_request: LoadIncomingRequest<T>,
            incoming_request_result: Result<IncomingRequest<T>, DispatchError>,
        ) -> DispatchResultWithPostInfo {
            debug!("called import_incoming_request");
            let net_id = load_incoming_request.network_id();
            let _ = Self::ensure_bridge_account(origin, net_id)?;
            pays_no(Self::inner_import_incoming_request(
                net_id,
                load_incoming_request,
                incoming_request_result,
            ))
        }

        /// Approve the given outgoing request. The function is used by bridge peers.
        ///
        /// Verifies the peer signature of the given request and adds it to `RequestApprovals`.
        /// Once quorum is collected, the request gets finalized and removed from request queue.
        #[pallet::call_index(12)]
        #[pallet::weight(<T as Config>::WeightInfo::approve_request())]
        pub fn approve_request(
            origin: OriginFor<T>,
            ocw_public: ecdsa::Public,
            hash: H256,
            signature_params: SignatureParams,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug!("called approve_request");
            let author = ensure_signed(origin)?;
            let net_id = network_id;
            Self::ensure_peer(&author, net_id)?;
            pays_no_with_maybe_weight(Self::inner_approve_request(
                ocw_public,
                hash,
                signature_params,
                author,
                net_id,
            ))
        }

        /// Cancels a registered request.
        ///
        /// Loads request by the given `hash`, cancels it, changes its status to `Failed` and
        /// removes it from the request queues.
        ///
        /// Can only be called from a bridge account.
        #[pallet::call_index(13)]
        #[pallet::weight(<T as Config>::WeightInfo::abort_request())]
        pub fn abort_request(
            origin: OriginFor<T>,
            hash: H256,
            error: DispatchError,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug!(
                "called abort_request. Hash: {:?}, reason: {:?}",
                hash, error
            );
            let _ = Self::ensure_bridge_account(origin, network_id)?;
            let request = Requests::<T>::get(network_id, hash)
                .ok_or_else(|| err_pays_no(Error::<T>::UnknownRequest))?;
            pays_no(Self::inner_abort_request(&request, hash, error, network_id))
        }

        /// Add the given peer to the peers set without additional checks.
        ///
        /// Can only be called by a root account.

        #[transactional]
        #[pallet::call_index(14)]
        // eth-bridge pallet will be deprecated soon, so we set const weight here
        #[pallet::weight(EXTRINSIC_FIXED_WEIGHT)]
        pub fn force_add_peer(
            origin: OriginFor<T>,
            who: T::AccountId,
            address: EthAddress,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_root(origin)?;
            if !Self::is_peer(&who, network_id) {
                bridge_multisig::Pallet::<T>::add_signatory(
                    RawOrigin::Signed(get_bridge_account::<T>(network_id)).into(),
                    who.clone(),
                )
                .map_err(|e| e.error)?;
                PeerAddress::<T>::insert(network_id, &who, address);
                PeerAccountId::<T>::insert(network_id, &address, who.clone());
                <Peers<T>>::mutate(network_id, |l| l.insert(who));
            }
            Ok(().into())
        }

        /// Remove asset
        ///
        /// Can only be called by root.
        #[pallet::call_index(15)]
        // eth-bridge pallet will be deprecated soon, so we set const weight here
        #[pallet::weight(EXTRINSIC_FIXED_WEIGHT)]
        pub fn remove_sidechain_asset(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            log::debug!("called remove_sidechain_asset. asset_id: {:?}", asset_id);
            ensure_root(origin)?;
            T::AssetInfoProvider::ensure_asset_exists(&asset_id)?;
            let token_address = RegisteredSidechainToken::<T>::get(network_id, &asset_id)
                .ok_or(Error::<T>::UnknownAssetId)?;
            RegisteredAsset::<T>::remove(network_id, &asset_id);
            RegisteredSidechainAsset::<T>::remove(network_id, &token_address);
            RegisteredSidechainToken::<T>::remove(network_id, &asset_id);
            SidechainAssetPrecision::<T>::remove(network_id, &asset_id);
            Ok(().into())
        }

        /// Register existing asset
        ///
        /// Can only be called by root.
        #[pallet::call_index(16)]
        // eth-bridge pallet will be deprecated soon, so we set const weight here
        #[pallet::weight(EXTRINSIC_FIXED_WEIGHT)]
        pub fn register_existing_sidechain_asset(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            token_address: EthAddress,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            log::debug!(
                "called register_existing_sidechain_asset. asset_id: {:?}",
                asset_id
            );
            ensure_root(origin)?;
            T::AssetInfoProvider::ensure_asset_exists(&asset_id)?;
            ensure!(
                !RegisteredAsset::<T>::contains_key(network_id, &asset_id),
                Error::<T>::TokenIsAlreadyAdded
            );

            let (_, _, precision, ..) = T::AssetInfoProvider::get_asset_info(&asset_id);
            RegisteredAsset::<T>::insert(network_id, &asset_id, AssetKind::Sidechain);
            RegisteredSidechainAsset::<T>::insert(network_id, &token_address, asset_id);
            RegisteredSidechainToken::<T>::insert(network_id, &asset_id, token_address);
            SidechainAssetPrecision::<T>::insert(network_id, &asset_id, precision);
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New request has been registered. [Request Hash]
        RequestRegistered(H256),
        /// The request's approvals have been collected. [Encoded Outgoing Request, Signatures]
        ApprovalsCollected(H256),
        /// The request finalization has been failed. [Request Hash]
        RequestFinalizationFailed(H256),
        /// The incoming request finalization has been failed. [Request Hash]
        IncomingRequestFinalizationFailed(H256),
        /// The incoming request has been finalized. [Request Hash]
        IncomingRequestFinalized(H256),
        /// The request was aborted and cancelled. [Request Hash]
        RequestAborted(H256),
        /// The request wasn't finalized nor cancelled. [Request Hash]
        CancellationFailed(H256),
        /// The request registration has been failed. [Request Hash, Error]
        RegisterRequestFailed(H256, DispatchError),
    }

    #[cfg_attr(test, derive(PartialEq, Eq))]
    #[pallet::error]
    pub enum Error<T> {
        /// Error fetching HTTP.
        HttpFetchingError,
        /// Account not found.
        AccountNotFound,
        /// Forbidden.
        Forbidden,
        /// Request is already registered.
        RequestIsAlreadyRegistered,
        /// Failed to load sidechain transaction.
        FailedToLoadTransaction,
        /// Failed to load token precision.
        FailedToLoadPrecision,
        /// Unknown method ID.
        UnknownMethodId,
        /// Invalid contract function input.
        InvalidFunctionInput,
        /// Invalid peer signature.
        InvalidSignature,
        /// Invalid uint value.
        InvalidUint,
        /// Invalid amount value.
        InvalidAmount,
        /// Invalid balance value.
        InvalidBalance,
        /// Invalid string value.
        InvalidString,
        /// Invalid byte value.
        InvalidByte,
        /// Invalid address value.
        InvalidAddress,
        /// Invalid asset id value.
        InvalidAssetId,
        /// Invalid account id value.
        InvalidAccountId,
        /// Invalid bool value.
        InvalidBool,
        /// Invalid h256 value.
        InvalidH256,
        /// Invalid array value.
        InvalidArray,
        /// Unknown contract event.
        UnknownEvent,
        /// Unknown token address.
        UnknownTokenAddress,
        /// No local account for signing available.
        NoLocalAccountForSigning,
        /// Unsupported asset id.
        UnsupportedAssetId,
        /// Failed to sign message.
        FailedToSignMessage,
        /// Failed to send signed message.
        FailedToSendSignedTransaction,
        /// Token is not owned by the author.
        TokenIsNotOwnedByTheAuthor,
        /// Token is already added.
        TokenIsAlreadyAdded,
        /// Duplicated request.
        DuplicatedRequest,
        /// Token is unsupported.
        UnsupportedToken,
        /// Unknown peer address.
        UnknownPeerAddress,
        /// Ethereum ABI encoding error.
        EthAbiEncodingError,
        /// Ethereum ABI decoding error.
        EthAbiDecodingError,
        /// Ethereum transaction is failed.
        EthTransactionIsFailed,
        /// Ethereum transaction is succeeded.
        EthTransactionIsSucceeded,
        /// Ethereum transaction is pending.
        EthTransactionIsPending,
        /// Ethereum log was removed.
        EthLogWasRemoved,
        /// No pending peer.
        NoPendingPeer,
        /// Wrong pending peer.
        WrongPendingPeer,
        /// Too many pending peers.
        TooManyPendingPeers,
        /// Failed to get an asset by id.
        FailedToGetAssetById,
        /// Can't add more peers.
        CantAddMorePeers,
        /// Can't remove more peers.
        CantRemoveMorePeers,
        /// Peer is already added.
        PeerIsAlreadyAdded,
        /// Unknown peer id.
        UnknownPeerId,
        /// Can't reserve funds.
        CantReserveFunds,
        /// Funds are already claimed.
        AlreadyClaimed,
        /// Failed to load substrate block header.
        FailedToLoadBlockHeader,
        /// Failed to load substrate finalized head.
        FailedToLoadFinalizedHead,
        /// Unknown contract address.
        UnknownContractAddress,
        /// Invalid contract input.
        InvalidContractInput,
        /// Request is not owned by the author.
        RequestIsNotOwnedByTheAuthor,
        /// Failed to parse transaction hash in a call.
        FailedToParseTxHashInCall,
        /// Request is not ready.
        RequestIsNotReady,
        /// Unknown request.
        UnknownRequest,
        /// Request is not finalized on Sidechain.
        RequestNotFinalizedOnSidechain,
        /// Unknown network.
        UnknownNetwork,
        /// Contract is in migration stage.
        ContractIsInMigrationStage,
        /// Contract is not on migration stage.
        ContractIsNotInMigrationStage,
        /// Contract is already in migration stage.
        ContractIsAlreadyInMigrationStage,
        /// Functionality is unavailable.
        Unavailable,
        /// Failed to unreserve asset.
        FailedToUnreserve,
        /// The sidechain asset is alredy registered.
        SidechainAssetIsAlreadyRegistered,
        /// Expected an outgoing request.
        ExpectedOutgoingRequest,
        /// Expected an incoming request.
        ExpectedIncomingRequest,
        /// Unknown asset id.
        UnknownAssetId,
        /// Failed to serialize JSON.
        JsonSerializationError,
        /// Failed to deserialize JSON.
        JsonDeserializationError,
        /// Failed to load sidechain node parameters.
        FailedToLoadSidechainNodeParams,
        /// Failed to load current sidechain height.
        FailedToLoadCurrentSidechainHeight,
        /// Failed to query sidechain 'used' variable.
        FailedToLoadIsUsed,
        /// Sidechain transaction might have failed due to gas limit.
        TransactionMightHaveFailedDueToGasLimit,
        /// A transfer of XOR was expected.
        ExpectedXORTransfer,
        /// Unable to pay transfer fees.
        UnableToPayFees,
        /// The request was purposely cancelled.
        Cancelled,
        /// Unsupported asset precision.
        UnsupportedAssetPrecision,
        /// Non-zero dust.
        NonZeroDust,
        /// Increment account reference error.
        IncRefError,
        /// Unknown error.
        Other,
        /// Expected pending request.
        ExpectedPendingRequest,
        /// Expected Ethereum network.
        ExpectedEthNetwork,
        /// Request was removed and refunded.
        RemovedAndRefunded,
        /// Authority account is not set.
        AuthorityAccountNotSet,
        /// Not enough peers provided, need at least 1
        NotEnoughPeers,
        /// Failed to read value from offchain storage.
        ReadStorageError,
        /// Bridge needs to have at least 3 peers for migration. Add new peer
        UnsafeMigration,
    }

    impl<T: Config> Error<T> {
        pub fn should_retry(&self) -> bool {
            match self {
                Self::HttpFetchingError
                | Self::NoLocalAccountForSigning
                | Self::FailedToSignMessage
                | Self::JsonDeserializationError => true,
                _ => false,
            }
        }

        pub fn should_abort(&self) -> bool {
            match self {
                Self::FailedToSendSignedTransaction => false,
                _ => true,
            }
        }
    }

    /// Registered requests queue handled by off-chain workers.
    #[pallet::storage]
    #[pallet::getter(fn requests_queue)]
    pub type RequestsQueue<T: Config> =
        StorageMap<_, Twox64Concat, BridgeNetworkId<T>, Vec<H256>, ValueQuery>;

    /// Registered requests.
    #[pallet::storage]
    #[pallet::getter(fn request)]
    pub type Requests<T: Config> =
        StorageDoubleMap<_, Twox64Concat, BridgeNetworkId<T>, Identity, H256, OffchainRequest<T>>;

    /// Used to identify an incoming request by the corresponding load request.
    #[pallet::storage]
    #[pallet::getter(fn load_to_incoming_request_hash)]
    pub type LoadToIncomingRequestHash<T: Config> =
        StorageDoubleMap<_, Twox64Concat, BridgeNetworkId<T>, Identity, H256, H256, ValueQuery>;

    /// Requests statuses.
    #[pallet::storage]
    #[pallet::getter(fn request_status)]
    pub type RequestStatuses<T: Config> =
        StorageDoubleMap<_, Twox64Concat, BridgeNetworkId<T>, Identity, H256, RequestStatus>;

    /// Requests submission height map (on substrate).
    #[pallet::storage]
    #[pallet::getter(fn request_submission_height)]
    pub type RequestSubmissionHeight<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        BridgeNetworkId<T>,
        Identity,
        H256,
        BlockNumberFor<T>,
        ValueQuery,
    >;

    /// Outgoing requests approvals.
    #[pallet::storage]
    #[pallet::getter(fn approvals)]
    pub(super) type RequestApprovals<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        BridgeNetworkId<T>,
        Identity,
        H256,
        BTreeSet<SignatureParams>,
        ValueQuery,
    >;

    /// Requests made by an account.
    #[pallet::storage]
    #[pallet::getter(fn account_requests)]
    pub(super) type AccountRequests<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<(BridgeNetworkId<T>, H256)>, ValueQuery>;

    /// Registered asset kind.
    #[pallet::storage]
    #[pallet::getter(fn registered_asset)]
    pub(super) type RegisteredAsset<T: Config> =
        StorageDoubleMap<_, Twox64Concat, BridgeNetworkId<T>, Identity, T::AssetId, AssetKind>;

    /// Precision (decimals) of a registered sidechain asset. Should be the same as in the ERC-20
    /// contract.
    #[pallet::storage]
    #[pallet::getter(fn sidechain_asset_precision)]
    pub(super) type SidechainAssetPrecision<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        BridgeNetworkId<T>,
        Identity,
        T::AssetId,
        BalancePrecision,
        ValueQuery,
    >;

    /// Registered token `AssetId` on Thischain.
    #[pallet::storage]
    #[pallet::getter(fn registered_sidechain_asset)]
    pub(super) type RegisteredSidechainAsset<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        BridgeNetworkId<T>,
        Blake2_128Concat,
        EthAddress,
        T::AssetId,
    >;

    /// Registered asset address on Sidechain.
    #[pallet::storage]
    #[pallet::getter(fn registered_sidechain_token)]
    pub(super) type RegisteredSidechainToken<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        BridgeNetworkId<T>,
        Blake2_128Concat,
        T::AssetId,
        EthAddress,
    >;

    /// Network peers set.
    #[pallet::storage]
    #[pallet::getter(fn peers)]
    pub(super) type Peers<T: Config> =
        StorageMap<_, Twox64Concat, BridgeNetworkId<T>, BTreeSet<T::AccountId>, ValueQuery>;

    /// Network pending (being added/removed) peer.
    #[pallet::storage]
    #[pallet::getter(fn pending_peer)]
    pub(super) type PendingPeer<T: Config> =
        StorageMap<_, Twox64Concat, BridgeNetworkId<T>, T::AccountId>;

    /// Used for compatibility with XOR and VAL contracts.
    #[pallet::storage]
    pub(super) type PendingEthPeersSync<T: Config> = StorageValue<_, EthPeersSync, ValueQuery>;

    /// Peer account ID on Thischain.
    #[pallet::storage]
    #[pallet::getter(fn peer_account_id)]
    pub(super) type PeerAccountId<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        BridgeNetworkId<T>,
        Blake2_128Concat,
        EthAddress,
        T::AccountId,
        OptionQuery,
    >;

    /// Peer address on Sidechain.
    #[pallet::storage]
    #[pallet::getter(fn peer_address)]
    pub(super) type PeerAddress<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        BridgeNetworkId<T>,
        Blake2_128Concat,
        T::AccountId,
        EthAddress,
        ValueQuery,
    >;

    /// Multi-signature bridge peers' account. `None` if there is no account and network with the given ID.
    #[pallet::storage]
    #[pallet::getter(fn bridge_account)]
    pub type BridgeAccount<T: Config> =
        StorageMap<_, Twox64Concat, BridgeNetworkId<T>, T::AccountId>;

    /// Thischain authority account.
    #[pallet::storage]
    #[pallet::getter(fn authority_account)]
    pub(super) type AuthorityAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Bridge status.
    #[pallet::storage]
    #[pallet::getter(fn bridge_contract_status)]
    pub(super) type BridgeStatuses<T: Config> =
        StorageMap<_, Twox64Concat, BridgeNetworkId<T>, BridgeStatus>;

    /// Smart-contract address on Sidechain.
    #[pallet::storage]
    #[pallet::getter(fn bridge_contract_address)]
    pub(super) type BridgeContractAddress<T: Config> =
        StorageMap<_, Twox64Concat, BridgeNetworkId<T>, EthAddress, ValueQuery>;

    /// Sora XOR master contract address.
    #[pallet::storage]
    #[pallet::getter(fn xor_master_contract_address)]
    pub(super) type XorMasterContractAddress<T: Config> = StorageValue<_, EthAddress, ValueQuery>;

    /// Sora VAL master contract address.
    #[pallet::storage]
    #[pallet::getter(fn val_master_contract_address)]
    pub(super) type ValMasterContractAddress<T: Config> = StorageValue<_, EthAddress, ValueQuery>;

    /// Next Network ID counter.
    #[pallet::storage]
    pub(super) type NextNetworkId<T: Config> = StorageValue<_, BridgeNetworkId<T>, ValueQuery>;

    /// Requests migrating from version '0.1.0' to '0.2.0'. These requests should be removed from
    /// `RequestsQueue` before migration procedure started.
    #[pallet::storage]
    pub(super) type MigratingRequests<T: Config> = StorageValue<_, Vec<H256>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn bridge_signature_version)]
    pub(super) type BridgeSignatureVersions<T: Config> = StorageMap<
        _,
        Twox64Concat,
        BridgeNetworkId<T>,
        BridgeSignatureVersion,
        ValueQuery,
        DefaultForBridgeSignatureVersion,
    >;

    #[pallet::type_value]
    pub fn DefaultForBridgeSignatureVersion() -> BridgeSignatureVersion {
        BridgeSignatureVersion::V3
    }

    #[pallet::storage]
    #[pallet::getter(fn pending_bridge_signature_version)]
    pub(super) type PendingBridgeSignatureVersions<T: Config> =
        StorageMap<_, Twox64Concat, BridgeNetworkId<T>, BridgeSignatureVersion>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub authority_account: Option<T::AccountId>,
        pub xor_master_contract_address: EthAddress,
        pub val_master_contract_address: EthAddress,
        pub networks: Vec<NetworkConfig<T>>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                authority_account: Default::default(),
                xor_master_contract_address: Default::default(),
                val_master_contract_address: Default::default(),
                networks: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            AuthorityAccount::<T>::put(&self.authority_account.as_ref().unwrap());
            XorMasterContractAddress::<T>::put(&self.xor_master_contract_address);
            ValMasterContractAddress::<T>::put(&self.val_master_contract_address);
            for network in &self.networks {
                let net_id = NextNetworkId::<T>::get();
                let peers_account_id = &network.bridge_account_id;
                BridgeContractAddress::<T>::insert(net_id, network.bridge_contract_address);
                frame_system::Pallet::<T>::inc_consumers(&peers_account_id).unwrap();
                BridgeAccount::<T>::insert(net_id, peers_account_id.clone());
                BridgeStatuses::<T>::insert(net_id, BridgeStatus::Initialized);
                Peers::<T>::insert(net_id, network.initial_peers.clone());
                for asset_config in &network.assets {
                    let kind = asset_config.kind();
                    let asset_id = asset_config.asset_id();
                    if let AssetConfig::Sidechain {
                        sidechain_id,
                        precision,
                        ..
                    } = &asset_config
                    {
                        let token_address = EthAddress::from(sidechain_id.0);
                        RegisteredSidechainAsset::<T>::insert(net_id, token_address, *asset_id);
                        RegisteredSidechainToken::<T>::insert(net_id, asset_id, token_address);
                        SidechainAssetPrecision::<T>::insert(net_id, asset_id, precision);
                    }
                    RegisteredAsset::<T>::insert(net_id, asset_id, kind);
                }
                // TODO: consider to change to Limited.
                let scope = Scope::Unlimited;
                let permission_ids = [MINT, BURN];
                for permission_id in &permission_ids {
                    permissions::Pallet::<T>::assign_permission(
                        peers_account_id.clone(),
                        &peers_account_id,
                        *permission_id,
                        scope,
                    )
                    .expect("failed to assign permissions for a bridge account");
                }
                for (asset_id, balance) in &network.reserves {
                    assets::Pallet::<T>::mint_to(
                        asset_id,
                        &peers_account_id,
                        &peers_account_id,
                        *balance,
                    )
                    .unwrap();
                }
                NextNetworkId::<T>::set(net_id + T::NetworkId::one());
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Registers the given off-chain request.
    ///
    /// Conditions for registering:
    /// 1. Network ID should be valid.
    /// 2. If the bridge is migrating and request is outgoing, it should be allowed during migration.
    /// 3. Request status should be empty or `Failed` (for resubmission).
    /// 4. There is no registered request with the same hash.
    /// 5. The request's `validate` and `prepare` should pass.
    fn add_request(request: &OffchainRequest<T>) -> Result<(), DispatchError> {
        let net_id = request.network_id();
        let bridge_status = BridgeStatuses::<T>::get(net_id).ok_or(Error::<T>::UnknownNetwork)?;
        if request.is_incoming() {
            Self::register_incoming_request_inner(request, net_id)?;
            return Ok(());
        }
        if let Some((outgoing_req, _)) = request.as_outgoing() {
            ensure!(
                bridge_status != BridgeStatus::Migrating
                    || outgoing_req.is_allowed_during_migration(),
                Error::<T>::ContractIsInMigrationStage
            );
        }
        let hash = request.hash();
        let can_resubmit = RequestStatuses::<T>::get(net_id, &hash)
            .map(|status| matches!(status, RequestStatus::Failed(_)))
            .unwrap_or(false);
        if !can_resubmit {
            ensure!(
                Requests::<T>::get(net_id, &hash).is_none(),
                Error::<T>::DuplicatedRequest
            );
        }
        request.validate()?;
        request.prepare()?;
        AccountRequests::<T>::mutate(&request.author(), |vec| vec.push((net_id, hash)));
        Requests::<T>::insert(net_id, &hash, request);
        RequestsQueue::<T>::mutate(net_id, |v| v.push(hash));
        RequestStatuses::<T>::insert(net_id, &hash, RequestStatus::Pending);
        let block_number = frame_system::Pallet::<T>::current_block_number();
        RequestSubmissionHeight::<T>::insert(net_id, &hash, block_number);
        Self::deposit_event(Event::RequestRegistered(hash));
        Ok(())
    }

    /// Prepares and validates the request, then adds it to the queue and maps it with the
    /// corresponding load request and removes the load request from the queue.
    fn register_incoming_request_inner(
        incoming_request: &OffchainRequest<T>,
        network_id: T::NetworkId,
    ) -> Result<H256, DispatchError> {
        let sidechain_tx_hash = incoming_request
            .as_incoming()
            .expect("request is always 'incoming'; qed")
            .0
            .hash();
        let incoming_request_hash = incoming_request.hash();
        let request_author = incoming_request.author().clone();
        ensure!(
            !Requests::<T>::contains_key(network_id, incoming_request_hash),
            Error::<T>::RequestIsAlreadyRegistered
        );
        Self::remove_request_from_queue(network_id, &sidechain_tx_hash);
        RequestStatuses::<T>::insert(network_id, sidechain_tx_hash, RequestStatus::Done);
        LoadToIncomingRequestHash::<T>::insert(
            network_id,
            sidechain_tx_hash,
            incoming_request_hash,
        );
        if let Err(e) = incoming_request
            .validate()
            .and_then(|_| incoming_request.prepare())
        {
            RequestStatuses::<T>::insert(
                network_id,
                incoming_request_hash,
                RequestStatus::Failed(e),
            );
            warn!("{:?}", e);
            Self::deposit_event(Event::RegisterRequestFailed(incoming_request_hash, e));
            return Ok(incoming_request_hash);
        }
        Requests::<T>::insert(network_id, &incoming_request_hash, incoming_request);
        RequestsQueue::<T>::mutate(network_id, |v| v.push(incoming_request_hash));
        RequestStatuses::<T>::insert(network_id, incoming_request_hash, RequestStatus::Pending);
        AccountRequests::<T>::mutate(request_author, |v| {
            v.push((network_id, incoming_request_hash))
        });
        Ok(incoming_request_hash)
    }

    /// At first, `finalize` is called on the request, if it fails, the `cancel` function
    /// gets called. Request status changes depending on the result (`Done` or `Failed`), and
    /// finally the request gets removed from the queue.
    fn finalize_incoming_request_inner(
        request: &IncomingRequest<T>,
        hash: H256,
        network_id: T::NetworkId,
    ) -> DispatchResult {
        ensure!(
            RequestStatuses::<T>::get(network_id, hash).ok_or(Error::<T>::UnknownRequest)?
                == RequestStatus::Pending,
            Error::<T>::ExpectedPendingRequest
        );
        let error_opt = request.finalize().err();
        if let Some(e) = error_opt {
            error!("Incoming request failed {:?} {:?}", hash, e);
            Self::deposit_event(Event::IncomingRequestFinalizationFailed(hash));
            RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Failed(e));
            cancel!(request, hash, network_id, e);
        } else {
            warn!("Incoming request finalized {:?}", hash);
            RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Done);
            Self::deposit_event(Event::IncomingRequestFinalized(hash));
        }
        Self::remove_request_from_queue(network_id, &hash);
        Ok(())
    }

    /// Finds and removes request from `RequestsQueue` by its hash and network id.
    fn remove_request_from_queue(network_id: T::NetworkId, hash: &H256) {
        RequestsQueue::<T>::mutate(network_id, |queue| {
            if let Some(pos) = queue.iter().position(|x| x == hash) {
                queue.remove(pos);
            }
        });
    }

    /// Registers new sidechain asset and grants mint permission to the bridge account.
    fn register_sidechain_asset(
        token_address: EthAddress,
        precision: BalancePrecision,
        symbol: AssetSymbol,
        name: AssetName,
        network_id: T::NetworkId,
    ) -> Result<T::AssetId, DispatchError> {
        ensure!(
            RegisteredSidechainAsset::<T>::get(network_id, &token_address).is_none(),
            Error::<T>::TokenIsAlreadyAdded
        );
        ensure!(
            precision <= DEFAULT_BALANCE_PRECISION,
            Error::<T>::UnsupportedAssetPrecision
        );
        let bridge_account =
            Self::bridge_account(network_id).expect("networks can't be removed; qed");
        let asset_id = assets::Pallet::<T>::register_from(
            &bridge_account,
            symbol,
            name,
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            common::AssetType::Regular,
            None,
            None,
        )?;
        RegisteredAsset::<T>::insert(network_id, &asset_id, AssetKind::Sidechain);
        RegisteredSidechainAsset::<T>::insert(network_id, &token_address, asset_id);
        RegisteredSidechainToken::<T>::insert(network_id, &asset_id, token_address);
        SidechainAssetPrecision::<T>::insert(network_id, &asset_id, precision);
        let scope = Scope::Limited(common::hash(&asset_id));
        let permission_ids = [MINT, BURN];
        for permission_id in &permission_ids {
            let permission_owner = permissions::Owners::<T>::get(permission_id, &scope)
                .pop()
                .unwrap_or_else(|| bridge_account.clone());
            permissions::Pallet::<T>::grant_permission_with_scope(
                permission_owner,
                bridge_account.clone(),
                *permission_id,
                scope,
            )?;
        }

        Ok(asset_id)
    }

    fn inner_abort_request(
        request: &OffchainRequest<T>,
        hash: H256,
        error: DispatchError,
        network_id: T::NetworkId,
    ) -> Result<(), DispatchError> {
        ensure!(
            RequestStatuses::<T>::get(network_id, hash).ok_or(Error::<T>::UnknownRequest)?
                == RequestStatus::Pending,
            Error::<T>::ExpectedPendingRequest
        );
        cancel!(request, hash, network_id, error);
        Self::remove_request_from_queue(network_id, &hash);
        Self::deposit_event(Event::RequestAborted(hash));
        Ok(())
    }

    fn inner_import_incoming_request(
        net_id: T::NetworkId,
        load_incoming_request: LoadIncomingRequest<T>,
        incoming_request_result: Result<IncomingRequest<T>, DispatchError>,
    ) -> Result<(), DispatchError> {
        let sidechain_tx_hash = load_incoming_request.hash();
        let load_incoming = OffchainRequest::LoadIncoming(load_incoming_request);
        Self::add_request(&load_incoming)?;
        match incoming_request_result {
            Ok(incoming_request) => {
                assert_eq!(net_id, incoming_request.network_id());
                let incoming = OffchainRequest::incoming(incoming_request.clone());
                let incoming_request_hash = incoming.hash();
                Self::add_request(&incoming)?;
                Self::finalize_incoming_request_inner(
                    &incoming_request,
                    incoming_request_hash,
                    net_id,
                )?;
            }
            Err(e) => {
                Self::inner_abort_request(&load_incoming, sidechain_tx_hash, e, net_id)?;
            }
        }
        Ok(().into())
    }

    fn inner_approve_request(
        ocw_public: ecdsa::Public,
        hash: H256,
        signature_params: SignatureParams,
        author: T::AccountId,
        net_id: T::NetworkId,
    ) -> Result<Option<Weight>, DispatchError> {
        let request = Requests::<T>::get(net_id, hash)
            .and_then(|x| x.into_outgoing().map(|x| x.0))
            .ok_or(Error::<T>::UnknownRequest)?;
        let request_encoded = request.to_eth_abi(hash)?;
        if !Self::verify_message(
            request_encoded.as_raw(),
            &signature_params,
            &ocw_public,
            &author,
        ) {
            // TODO: punish the peer.
            return Err(Error::<T>::InvalidSignature.into());
        }
        info!("Verified request approve {:?}", request_encoded);
        let mut approvals = RequestApprovals::<T>::get(net_id, &hash);
        let pending_peers_len = if Self::is_additional_signature_needed(net_id, &request) {
            1
        } else {
            0
        };
        let need_sigs = majority(Self::peers(net_id).len()) + pending_peers_len;
        let current_status =
            RequestStatuses::<T>::get(net_id, &hash).ok_or(Error::<T>::UnknownRequest)?;
        approvals.insert(signature_params);
        RequestApprovals::<T>::insert(net_id, &hash, &approvals);
        if current_status == RequestStatus::Pending && approvals.len() == need_sigs {
            if let Err(err) = request.finalize(hash) {
                error!("Outgoing request finalization failed: {:?}", err);
                RequestStatuses::<T>::insert(net_id, hash, RequestStatus::Failed(err));
                Self::deposit_event(Event::RequestFinalizationFailed(hash));
                cancel!(request, hash, net_id, err);
            } else {
                debug!("Outgoing request approvals collected {:?}", hash);
                RequestStatuses::<T>::insert(net_id, hash, RequestStatus::ApprovalsReady);
                Self::deposit_event(Event::ApprovalsCollected(hash));
            }
            Self::remove_request_from_queue(net_id, &hash);
            let weight_info = <T as Config>::WeightInfo::approve_request_finalize();
            return Ok(Some(weight_info));
        }
        Ok(None)
    }

    fn is_additional_signature_needed(net_id: T::NetworkId, request: &OutgoingRequest<T>) -> bool {
        PendingPeer::<T>::get(net_id).is_some()
            && !matches!(
                &request,
                OutgoingRequest::AddPeer(..) | OutgoingRequest::AddPeerCompat(..)
            )
    }

    fn ensure_generic_network(
        generic_network_id: GenericNetworkId,
    ) -> Result<T::NetworkId, DispatchError> {
        let network_id = T::GetEthNetworkId::get();
        if generic_network_id != GenericNetworkId::EVMLegacy(network_id.unique_saturated_into()) {
            return Err(Error::<T>::UnknownNetwork.into());
        }
        Ok(network_id)
    }
}

impl<T: Config> BridgeApp<T::AccountId, EthAddress, T::AssetId, Balance> for Pallet<T> {
    fn is_asset_supported(network_id: GenericNetworkId, asset_id: T::AssetId) -> bool {
        let Ok(network_id) = Self::ensure_generic_network(network_id) else {
            return false;
        };
        RegisteredAsset::<T>::contains_key(network_id, &asset_id)
    }

    fn transfer(
        network_id: GenericNetworkId,
        asset_id: T::AssetId,
        sender: T::AccountId,
        recipient: EthAddress,
        amount: Balance,
    ) -> Result<H256, DispatchError> {
        debug!("called BridgeApp::transfer");
        let network_id = Self::ensure_generic_network(network_id)?;
        let nonce = frame_system::Pallet::<T>::account_nonce(&sender);
        let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
        let request = OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer {
            from: sender.clone(),
            to: recipient,
            asset_id,
            amount,
            nonce,
            network_id,
            timepoint,
        }));
        let tx_hash = request.hash();
        Self::add_request(&request)?;
        frame_system::Pallet::<T>::inc_account_nonce(&sender);
        Ok(tx_hash)
    }

    fn refund(
        _network_id: GenericNetworkId,
        _message_id: H256,
        _recipient: T::AccountId,
        _asset_id: T::AssetId,
        _amount: Balance,
    ) -> DispatchResult {
        Err(Error::<T>::Unavailable.into())
    }

    fn list_supported_assets(
        network_id: GenericNetworkId,
    ) -> Vec<bridge_types::types::BridgeAssetInfo> {
        use bridge_types::evm::{EVMAppKind, EVMLegacyAssetInfo};
        use bridge_types::types::BridgeAssetInfo;
        let Ok(network_id) = Self::ensure_generic_network(network_id) else {
            return vec![];
        };
        RegisteredAsset::<T>::iter_prefix(network_id)
            .map(|(asset_id, _kind)| {
                let evm_address =
                    RegisteredSidechainToken::<T>::get(network_id, &asset_id).map(|x| H160(x.0));
                let precision = evm_address.map(|_address| {
                    let precision = SidechainAssetPrecision::<T>::get(network_id, &asset_id);
                    precision
                });

                let app_kind = if asset_id == common::XOR.into() {
                    EVMAppKind::XorMaster
                } else if asset_id == common::VAL.into() {
                    EVMAppKind::ValMaster
                } else {
                    EVMAppKind::HashiBridge
                };

                BridgeAssetInfo::EVMLegacy(EVMLegacyAssetInfo {
                    asset_id: asset_id.into(),
                    app_kind,
                    evm_address,
                    precision,
                })
            })
            .collect()
    }

    fn list_apps() -> Vec<bridge_types::types::BridgeAppInfo> {
        use bridge_types::evm::{EVMAppInfo, EVMAppKind};
        use bridge_types::types::BridgeAppInfo;
        let mut apps = vec![];
        let network_id = T::GetEthNetworkId::get();
        let generic_network_id = GenericNetworkId::EVMLegacy(network_id.unique_saturated_into());
        if let Ok(bridge_address) = BridgeContractAddress::<T>::try_get(network_id) {
            let app = BridgeAppInfo::EVM(
                generic_network_id,
                EVMAppInfo {
                    evm_address: bridge_address,
                    app_kind: EVMAppKind::HashiBridge,
                },
            );
            apps.push(app);
        }
        if let Ok(xor_master) = XorMasterContractAddress::<T>::try_get() {
            let app = BridgeAppInfo::EVM(
                generic_network_id,
                EVMAppInfo {
                    evm_address: xor_master,
                    app_kind: EVMAppKind::XorMaster,
                },
            );
            apps.push(app);
        }
        if let Ok(val_master) = ValMasterContractAddress::<T>::try_get() {
            let app = BridgeAppInfo::EVM(
                generic_network_id,
                EVMAppInfo {
                    evm_address: val_master,
                    app_kind: EVMAppKind::ValMaster,
                },
            );
            apps.push(app);
        }
        apps
    }

    fn is_asset_supported_weight() -> Weight {
        T::DbWeight::get().reads(1)
    }

    fn refund_weight() -> Weight {
        Default::default()
    }

    fn transfer_weight() -> Weight {
        <T as Config>::WeightInfo::transfer_to_sidechain()
    }
}
