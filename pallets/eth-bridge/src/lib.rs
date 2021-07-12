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

#[macro_use]
extern crate alloc;
extern crate jsonrpc_core as rpc;

use crate::contract::{
    functions, init_add_peer_by_peer_fn, init_remove_peer_by_peer_fn, ADD_PEER_BY_PEER_FN,
    ADD_PEER_BY_PEER_ID, ADD_PEER_BY_PEER_TX_HASH_ARG_POS, FUNCTIONS, METHOD_ID_SIZE,
    REMOVE_PEER_BY_PEER_FN, REMOVE_PEER_BY_PEER_ID, REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS,
};
#[cfg(test)]
use crate::mock::Mock;
use crate::offchain::SignedTransactionData;
use crate::types::{
    BlockNumber, Bytes, CallRequest, FilterBuilder, Log, SubstrateBlockLimited,
    SubstrateHeaderLimited, Transaction, TransactionReceipt,
};
use alloc::string::String;
use bridge_multisig::MultiChainHeight;
use codec::{Decode, Encode, FullCodec};
use common::prelude::Balance;
use common::{eth, AssetName, AssetSymbol, BalancePrecision, DEFAULT_BALANCE_PRECISION};
use core::convert::{TryFrom, TryInto};
use core::{iter, line, stringify};
use ethabi::{ParamType, Token};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::sp_runtime::app_crypto::{ecdsa, sp_core, Public};
use frame_support::sp_runtime::offchain::storage::StorageValueRef;
use frame_support::sp_runtime::offchain::storage_lock::{BlockNumberProvider, StorageLock, Time};
use frame_support::sp_runtime::traits::{
    AtLeast32Bit, IdentifyAccount, MaybeSerializeDeserialize, Member, One, Saturating, Zero,
};
use frame_support::sp_runtime::{
    offchain as rt_offchain, DispatchErrorWithPostInfo, KeyTypeId, MultiSigner,
};
use frame_support::traits::{Get, GetCallName};
use frame_support::weights::{PostDispatchInfo, Weight};
use frame_support::{
    debug, ensure, fail, sp_io, transactional, IterableStorageDoubleMap, Parameter, RuntimeDebug,
};
use frame_system::offchain::{Account, AppCrypto, CreateSignedTransaction, Signer};
use frame_system::pallet_prelude::OriginFor;
use frame_system::{ensure_root, ensure_signed};
use hex_literal::hex;
pub use pallet::*;
use permissions::{Scope, BURN, MINT};
use requests::*;
use rpc::Params;
use rustc_hex::ToHex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sp_core::{H160, H256};
use sp_io::hashing::blake2_256;
use sp_std::borrow::Cow;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::fmt::{self, Debug, Formatter};
use sp_std::marker::PhantomData;
use sp_std::prelude::*;
#[cfg(feature = "std")]
use std::collections::HashMap;

pub trait WeightInfo {
    fn register_bridge() -> Weight;
    fn add_asset() -> Weight;
    fn add_sidechain_token() -> Weight;
    fn transfer_to_sidechain() -> Weight;
    fn request_from_sidechain() -> Weight;
    fn add_peer() -> Weight;
    fn remove_peer() -> Weight;
    fn force_add_peer() -> Weight;
    fn prepare_for_migration() -> Weight;
    fn migrate() -> Weight;
    fn register_incoming_request() -> Weight;
    fn finalize_incoming_request() -> Weight;
    fn approve_request() -> Weight;
    fn approve_request_finalize() -> Weight;
    fn abort_request() -> Weight;
    fn import_incoming_request(is_ok: bool) -> Weight {
        let weight = Self::register_incoming_request()
            + if is_ok {
                Self::finalize_incoming_request()
            } else {
                Self::abort_request()
            };
        weight
    }
}

type Address = H160;
type EthereumAddress = Address;

pub mod weights;

mod benchmarking;
mod contract;
mod macros;
mod migrations;
#[cfg(test)]
mod mock;
pub mod offchain;
pub mod requests;
#[cfg(test)]
mod tests;
pub mod types;

/// Substrate node RPC URL.
const SUB_NODE_URL: &str = "http://127.0.0.1:9954";
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 10;
/// Substrate maximum amount of blocks for which an extrinsic is expecting to be finalized.
const SUBSTRATE_MAX_BLOCK_NUM_EXPECTING_UNTIL_FINALIZATION: u32 = 50;
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
pub const OFFCHAIN_TRANSACTION_WEIGHT_LIMIT: u64 = 10_000_000_000_000_000u64;
const MAX_PENDING_TX_BLOCKS_PERIOD: u32 = 100;
const RE_HANDLE_TXS_PERIOD: u32 = 200;

type AssetIdOf<T> = <T as assets::Config>::AssetId;
type Timepoint<T> = bridge_multisig::Timepoint<<T as frame_system::Config>::BlockNumber>;
type BridgeTimepoint<T> = Timepoint<T>;
type BridgeNetworkId<T> = <T as Config>::NetworkId;

/// Cryptography used by off-chain workers.
pub mod crypto {
    use crate::KEY_TYPE;

    use frame_support::sp_runtime::app_crypto::{app_crypto, ecdsa};
    use frame_support::sp_runtime::{MultiSignature, MultiSigner};

    app_crypto!(ecdsa, KEY_TYPE);

    pub struct TestAuthId;

    // implemented for ocw-runtime
    impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
        type RuntimeAppPublic = Public;
        type GenericSignature = ecdsa::Signature;
        type GenericPublic = ecdsa::Public;
    }
}

/// Ethereum node parameters (url, credentials).
#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, RuntimeDebug)]
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

/// Separated components of a secp256k1 signature.
#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "runtime-benchmarks"), derive(Default))]
#[repr(C)]
pub struct SignatureParams {
    r: [u8; 32],
    s: [u8; 32],
    v: u8,
}

impl fmt::Display for SignatureParams {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&format!(
            "SignatureParams {{\n\tr: {},\n\ts: {},\n\tv: {}\n}}",
            self.r.to_hex::<String>(),
            self.s.to_hex::<String>(),
            [self.v].to_hex::<String>()
        ))
    }
}

/// Outgoing (Thischain->Sidechain) request.
///
/// Each request, has the following properties: author, nonce, network ID, and hash (calculates
/// just-in-time).
/// And the following methods: validate, prepare, finalize, cancel.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub enum OutgoingRequest<T: Config> {
    /// Outgoing transfer from Substrate to Ethereum request.
    Transfer(OutgoingTransfer<T>),
    /// 'Add new Substrate asset' request.
    AddAsset(OutgoingAddAsset<T>),
    /// 'Add new Ethereum token' request.
    AddToken(OutgoingAddToken<T>),
    /// 'Add peer' request.
    AddPeer(OutgoingAddPeer<T>),
    /// 'Remove peer' request.
    RemovePeer(OutgoingRemovePeer<T>),
    /// 'Prepare for migration' request.
    PrepareForMigration(OutgoingPrepareForMigration<T>),
    /// 'Migrate' request.
    Migrate(OutgoingMigrate<T>),
    /// 'Add peer compat' request.
    AddPeerCompat(OutgoingAddPeerCompat<T>),
    /// 'Remove peer compat' request.
    RemovePeerCompat(OutgoingRemovePeerCompat<T>),
}

impl<T: Config> OutgoingRequest<T> {
    fn author(&self) -> &T::AccountId {
        match self {
            OutgoingRequest::Transfer(transfer) => &transfer.from,
            OutgoingRequest::AddAsset(request) => &request.author,
            OutgoingRequest::AddToken(request) => &request.author,
            OutgoingRequest::AddPeer(request) => &request.author,
            OutgoingRequest::RemovePeer(request) => &request.author,
            OutgoingRequest::PrepareForMigration(request) => &request.author,
            OutgoingRequest::Migrate(request) => &request.author,
            OutgoingRequest::AddPeerCompat(request) => &request.author,
            OutgoingRequest::RemovePeerCompat(request) => &request.author,
        }
    }

    /// Encodes the request to a corresponding Ethereum contract function's arguments.
    /// Also, serializes some parameters with `encode_packed` to be signed by peers.
    fn to_eth_abi(&self, tx_hash: H256) -> Result<OutgoingRequestEncoded, Error<T>> {
        match self {
            OutgoingRequest::Transfer(transfer) => transfer
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::Transfer),
            OutgoingRequest::AddAsset(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::AddAsset),
            OutgoingRequest::AddToken(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::AddToken),
            OutgoingRequest::AddPeer(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::AddPeer),
            OutgoingRequest::RemovePeer(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::RemovePeer),
            OutgoingRequest::PrepareForMigration(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::PrepareForMigration),
            OutgoingRequest::Migrate(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::Migrate),
            OutgoingRequest::AddPeerCompat(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::AddPeer),
            OutgoingRequest::RemovePeerCompat(request) => request
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::RemovePeer),
        }
    }

    fn network_id(&self) -> T::NetworkId {
        match self {
            OutgoingRequest::Transfer(request) => request.network_id,
            OutgoingRequest::AddAsset(request) => request.network_id,
            OutgoingRequest::AddToken(request) => request.network_id,
            OutgoingRequest::AddPeer(request) => request.network_id,
            OutgoingRequest::RemovePeer(request) => request.network_id,
            OutgoingRequest::PrepareForMigration(request) => request.network_id,
            OutgoingRequest::Migrate(request) => request.network_id,
            OutgoingRequest::AddPeerCompat(request) => request.network_id,
            OutgoingRequest::RemovePeerCompat(request) => request.network_id,
        }
    }

    fn timepoint(&self) -> Timepoint<T> {
        match self {
            OutgoingRequest::Transfer(request) => request.timepoint,
            OutgoingRequest::AddAsset(request) => request.timepoint,
            OutgoingRequest::AddToken(request) => request.timepoint,
            OutgoingRequest::AddPeer(request) => request.timepoint,
            OutgoingRequest::RemovePeer(request) => request.timepoint,
            OutgoingRequest::PrepareForMigration(request) => request.timepoint,
            OutgoingRequest::Migrate(request) => request.timepoint,
            OutgoingRequest::AddPeerCompat(request) => request.timepoint,
            OutgoingRequest::RemovePeerCompat(request) => request.timepoint,
        }
    }

    /// Checks that the request can be initiated (e.g., verifies that an account has
    /// sufficient funds for transfer).
    fn validate(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.validate(),
            OutgoingRequest::AddAsset(request) => request.validate(),
            OutgoingRequest::AddToken(request) => request.validate().map(|_| ()),
            OutgoingRequest::AddPeer(request) => request.validate().map(|_| ()),
            OutgoingRequest::RemovePeer(request) => request.validate().map(|_| ()),
            OutgoingRequest::PrepareForMigration(request) => request.validate().map(|_| ()),
            OutgoingRequest::Migrate(request) => request.validate().map(|_| ()),
            OutgoingRequest::AddPeerCompat(request) => request.validate().map(|_| ()),
            OutgoingRequest::RemovePeerCompat(request) => request.validate().map(|_| ()),
        }
    }

    fn prepare(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.prepare(),
            OutgoingRequest::AddAsset(request) => request.prepare(()),
            OutgoingRequest::AddToken(request) => request.prepare(()),
            OutgoingRequest::AddPeer(request) => request.prepare(()),
            OutgoingRequest::RemovePeer(request) => request.prepare(()),
            OutgoingRequest::PrepareForMigration(request) => request.prepare(()),
            OutgoingRequest::Migrate(request) => request.prepare(()),
            OutgoingRequest::AddPeerCompat(request) => request.prepare(()),
            OutgoingRequest::RemovePeerCompat(request) => request.prepare(()),
        }
    }

    fn finalize(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.finalize(),
            OutgoingRequest::AddAsset(request) => request.finalize(),
            OutgoingRequest::AddToken(request) => request.finalize(),
            OutgoingRequest::AddPeer(request) => request.finalize(),
            OutgoingRequest::RemovePeer(request) => request.finalize(),
            OutgoingRequest::PrepareForMigration(request) => request.finalize(),
            OutgoingRequest::Migrate(request) => request.finalize(),
            OutgoingRequest::AddPeerCompat(request) => request.finalize(),
            OutgoingRequest::RemovePeerCompat(request) => request.finalize(),
        }
    }

    fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.cancel(),
            OutgoingRequest::AddAsset(request) => request.cancel(),
            OutgoingRequest::AddToken(request) => request.cancel(),
            OutgoingRequest::AddPeer(request) => request.cancel(),
            OutgoingRequest::RemovePeer(request) => request.cancel(),
            OutgoingRequest::PrepareForMigration(request) => request.cancel(),
            OutgoingRequest::Migrate(request) => request.cancel(),
            OutgoingRequest::AddPeerCompat(request) => request.cancel(),
            OutgoingRequest::RemovePeerCompat(request) => request.cancel(),
        }
    }

    fn is_allowed_during_migration(&self) -> bool {
        matches!(self, OutgoingRequest::Migrate(_))
    }
}

/// Types of transaction-requests that can be made from a sidechain.
#[derive(Clone, Copy, Encode, Decode, RuntimeDebug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum IncomingTransactionRequestKind {
    Transfer,
    AddAsset,
    AddPeer,
    RemovePeer,
    PrepareForMigration,
    Migrate,
    AddPeerCompat,
    RemovePeerCompat,
    /// A special case of transferring XOR asset with post-taking fees.
    TransferXOR,
}

/// Types of meta-requests that can be made.
#[derive(Clone, Copy, Encode, Decode, RuntimeDebug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum IncomingMetaRequestKind {
    CancelOutgoingRequest,
    MarkAsDone,
}

/// Types of requests that can be made from a sidechain.
#[derive(Clone, Copy, Encode, Decode, RuntimeDebug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum IncomingRequestKind {
    Transaction(IncomingTransactionRequestKind),
    Meta(IncomingMetaRequestKind),
}

impl From<IncomingTransactionRequestKind> for IncomingRequestKind {
    fn from(v: IncomingTransactionRequestKind) -> Self {
        Self::Transaction(v)
    }
}

impl From<IncomingMetaRequestKind> for IncomingRequestKind {
    fn from(v: IncomingMetaRequestKind) -> Self {
        Self::Meta(v)
    }
}

impl IncomingTransactionRequestKind {
    /// Returns `true` if the request should be used with XOR and VAL contracts.
    pub fn is_compat(&self) -> bool {
        *self == Self::AddPeerCompat || *self == Self::RemovePeerCompat
    }
}

/// Incoming (Sidechain->Thischain) request.
///
/// Each request, has the following properties: transaction hash, height, network ID, and timepoint.
/// And the following methods: validate, prepare, finalize, cancel.
#[cfg_attr(feature = "std", derive(Serialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum IncomingRequest<T: Config> {
    Transfer(IncomingTransfer<T>),
    AddToken(IncomingAddToken<T>),
    ChangePeers(IncomingChangePeers<T>),
    CancelOutgoingRequest(IncomingCancelOutgoingRequest<T>),
    MarkAsDone(IncomingMarkAsDoneRequest<T>),
    PrepareForMigration(IncomingPrepareForMigration<T>),
    Migrate(IncomingMigrate<T>),
    ChangePeersCompat(IncomingChangePeersCompat<T>),
}

impl<T: Config> IncomingRequest<T> {
    pub fn try_from_contract_event(
        event: ContractEvent<Address, T::AccountId, Balance>,
        incoming_request: LoadIncomingTransactionRequest<T>,
        at_height: u64,
    ) -> Result<Self, Error<T>> {
        let network_id = incoming_request.network_id;
        let timepoint = incoming_request.timepoint;
        let author = incoming_request.author;
        let tx_hash = incoming_request.hash;

        let req = match event {
            ContractEvent::Deposit(DepositEvent {
                destination: to,
                amount,
                token: token_address,
                sidechain_asset: raw_asset_id,
            }) => {
                let (asset_id, asset_kind) = Module::<T>::get_asset_by_raw_asset_id(
                    raw_asset_id,
                    &token_address,
                    network_id,
                )?
                .ok_or(Error::<T>::UnsupportedAssetId)?;
                let amount = if !asset_kind.is_owned() {
                    let sidechain_precision =
                        SidechainAssetPrecision::<T>::get(network_id, &asset_id);
                    let thischain_precision = assets::Pallet::<T>::get_asset_info(&asset_id).2;
                    Pallet::<T>::convert_precision(
                        sidechain_precision,
                        thischain_precision,
                        amount,
                    )?
                    .0
                } else {
                    amount
                };
                IncomingRequest::Transfer(IncomingTransfer {
                    from: Default::default(),
                    to,
                    asset_id,
                    asset_kind,
                    amount,
                    author,
                    tx_hash,
                    at_height,
                    timepoint,
                    network_id,
                    should_take_fee: false,
                })
            }
            ContractEvent::ChangePeers(peer_address, added) => {
                let peer_account_id = PeerAccountId::<T>::get(network_id, &peer_address);
                ensure!(
                    peer_account_id != T::AccountId::default(),
                    Error::<T>::UnknownPeerAddress
                );
                IncomingRequest::ChangePeers(IncomingChangePeers {
                    peer_account_id,
                    peer_address,
                    added,
                    author,
                    tx_hash,
                    at_height,
                    timepoint,
                    network_id,
                })
            }
            ContractEvent::PreparedForMigration => {
                IncomingRequest::PrepareForMigration(IncomingPrepareForMigration {
                    author,
                    tx_hash,
                    at_height,
                    timepoint,
                    network_id,
                })
            }
            ContractEvent::Migrated(to) => IncomingRequest::Migrate(IncomingMigrate {
                new_contract_address: to,
                author,
                tx_hash,
                at_height,
                timepoint,
                network_id,
            }),
        };
        Ok(req)
    }

    fn hash(&self) -> H256 {
        match self {
            IncomingRequest::Transfer(request) => request.tx_hash,
            IncomingRequest::AddToken(request) => request.tx_hash,
            IncomingRequest::ChangePeers(request) => request.tx_hash,
            IncomingRequest::CancelOutgoingRequest(request) => request.initial_request_hash,
            IncomingRequest::MarkAsDone(request) => request.initial_request_hash,
            IncomingRequest::PrepareForMigration(request) => request.tx_hash,
            IncomingRequest::Migrate(request) => request.tx_hash,
            IncomingRequest::ChangePeersCompat(request) => request.tx_hash,
        }
    }

    pub fn network_id(&self) -> T::NetworkId {
        match self {
            IncomingRequest::Transfer(request) => request.network_id,
            IncomingRequest::AddToken(request) => request.network_id,
            IncomingRequest::ChangePeers(request) => request.network_id,
            IncomingRequest::CancelOutgoingRequest(request) => request.network_id,
            IncomingRequest::MarkAsDone(request) => request.network_id,
            IncomingRequest::PrepareForMigration(request) => request.network_id,
            IncomingRequest::Migrate(request) => request.network_id,
            IncomingRequest::ChangePeersCompat(request) => request.network_id,
        }
    }

    /// A sidechain height at which the sidechain transaction was added.
    fn at_height(&self) -> u64 {
        match self {
            IncomingRequest::Transfer(request) => request.at_height,
            IncomingRequest::AddToken(request) => request.at_height,
            IncomingRequest::ChangePeers(request) => request.at_height,
            IncomingRequest::CancelOutgoingRequest(request) => request.at_height,
            IncomingRequest::MarkAsDone(request) => request.at_height,
            IncomingRequest::PrepareForMigration(request) => request.at_height,
            IncomingRequest::Migrate(request) => request.at_height,
            IncomingRequest::ChangePeersCompat(request) => request.at_height,
        }
    }

    pub fn validate(&self) -> Result<(), DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.validate(),
            IncomingRequest::AddToken(_request) => Ok(()),
            IncomingRequest::ChangePeers(_request) => Ok(()),
            IncomingRequest::CancelOutgoingRequest(_request) => Ok(()),
            IncomingRequest::MarkAsDone(request) => request.validate(),
            IncomingRequest::PrepareForMigration(_request) => Ok(()),
            IncomingRequest::Migrate(_request) => Ok(()),
            IncomingRequest::ChangePeersCompat(_request) => Ok(()),
        }
    }

    pub fn prepare(&self) -> Result<(), DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.prepare(),
            IncomingRequest::AddToken(_request) => Ok(()),
            IncomingRequest::ChangePeers(_request) => Ok(()),
            IncomingRequest::CancelOutgoingRequest(request) => request.prepare(),
            IncomingRequest::MarkAsDone(request) => request.prepare(),
            IncomingRequest::PrepareForMigration(request) => request.prepare(),
            IncomingRequest::Migrate(request) => request.prepare(),
            IncomingRequest::ChangePeersCompat(_request) => Ok(()),
        }
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.cancel(),
            IncomingRequest::AddToken(_request) => Ok(()),
            IncomingRequest::ChangePeers(_request) => Ok(()),
            IncomingRequest::CancelOutgoingRequest(request) => request.cancel(),
            IncomingRequest::MarkAsDone(request) => request.cancel(),
            IncomingRequest::PrepareForMigration(request) => request.cancel(),
            IncomingRequest::Migrate(request) => request.cancel(),
            IncomingRequest::ChangePeersCompat(_request) => Ok(()),
        }
    }

    pub fn finalize(&self) -> Result<H256, DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.finalize(),
            IncomingRequest::AddToken(request) => request.finalize(),
            IncomingRequest::ChangePeers(request) => request.finalize(),
            IncomingRequest::CancelOutgoingRequest(request) => request.finalize(),
            IncomingRequest::MarkAsDone(request) => request.finalize(),
            IncomingRequest::PrepareForMigration(request) => request.finalize(),
            IncomingRequest::Migrate(request) => request.finalize(),
            IncomingRequest::ChangePeersCompat(request) => request.finalize(),
        }
    }

    /// A timepoint at which the request was registered in thischain. Used my the `bridge-multisig`
    /// pallet.
    pub fn timepoint(&self) -> Timepoint<T> {
        match self {
            IncomingRequest::Transfer(request) => request.timepoint(),
            IncomingRequest::AddToken(request) => request.timepoint(),
            IncomingRequest::ChangePeers(request) => request.timepoint(),
            IncomingRequest::CancelOutgoingRequest(request) => request.timepoint(),
            IncomingRequest::MarkAsDone(request) => request.timepoint(),
            IncomingRequest::PrepareForMigration(request) => request.timepoint(),
            IncomingRequest::Migrate(request) => request.timepoint(),
            IncomingRequest::ChangePeersCompat(request) => request.timepoint(),
        }
    }

    pub fn author(&self) -> &T::AccountId {
        match self {
            IncomingRequest::Transfer(request) => request.author(),
            IncomingRequest::AddToken(request) => request.author(),
            IncomingRequest::ChangePeers(request) => request.author(),
            IncomingRequest::CancelOutgoingRequest(request) => request.author(),
            IncomingRequest::MarkAsDone(request) => request.author(),
            IncomingRequest::PrepareForMigration(request) => request.author(),
            IncomingRequest::Migrate(request) => request.author(),
            IncomingRequest::ChangePeersCompat(request) => request.author(),
        }
    }

    /// Check that the incoming requests still exists on Sidechain.
    pub fn check_existence(&self) -> Result<bool, Error<T>> {
        let network_id = self.network_id();
        match self {
            IncomingRequest::CancelOutgoingRequest(request) => {
                let hash = request.tx_hash;
                let tx = Pallet::<T>::load_tx_receipt(hash, network_id)?;
                Ok(tx.is_approved() == false) // TODO: check for gas limit
            }
            IncomingRequest::MarkAsDone(request) => {
                Pallet::<T>::load_is_used(request.outgoing_request_hash, request.network_id)
            }
            _ => {
                let hash = self.hash();
                let tx = Pallet::<T>::load_tx_receipt(hash, network_id)?;
                Ok(tx.is_approved())
            }
        }
    }
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum LoadIncomingRequest<T: Config> {
    Transaction(LoadIncomingTransactionRequest<T>),
    Meta(LoadIncomingMetaRequest<T>, H256),
}

impl<T: Config> LoadIncomingRequest<T> {
    fn hash(&self) -> H256 {
        match self {
            Self::Transaction(request) => request.hash,
            Self::Meta(_, hash) => *hash,
        }
    }

    fn set_hash(&mut self, new_hash: H256) {
        match self {
            Self::Transaction(_request) => {
                debug::warn!("Attempt to set hash for a 'load transaction' request.");
            } // should not be the case
            Self::Meta(_, hash) => *hash = new_hash,
        }
    }

    pub fn network_id(&self) -> T::NetworkId {
        match self {
            Self::Transaction(request) => request.network_id,
            Self::Meta(request, _) => request.network_id,
        }
    }

    fn timepoint(&self) -> Timepoint<T> {
        match self {
            Self::Transaction(request) => request.timepoint,
            Self::Meta(request, _) => request.timepoint,
        }
    }

    fn author(&self) -> &T::AccountId {
        match self {
            Self::Transaction(request) => &request.author,
            Self::Meta(request, _) => &request.author,
        }
    }

    /// Checks that the request can be initiated.
    fn validate(&self) -> Result<(), DispatchError> {
        match self {
            Self::Transaction(_request) => Ok(()),
            Self::Meta(request, _) => {
                match request.kind {
                    IncomingMetaRequestKind::MarkAsDone => {
                        let request_status =
                            RequestStatuses::<T>::get(request.network_id, request.hash)
                                .ok_or(Error::<T>::UnknownRequest)?;
                        ensure!(
                            request_status == RequestStatus::ApprovalsReady,
                            Error::<T>::RequestIsNotReady
                        );
                    }
                    _ => (),
                }
                Ok(())
            }
        }
    }

    fn prepare(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn finalize(&self) -> DispatchResult {
        Ok(())
    }
}

/// Information needed for a request to be loaded from sidechain. Basically it's
/// a hash of the transaction and the type of the request.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct LoadIncomingTransactionRequest<T: Config> {
    author: T::AccountId,
    hash: H256,
    timepoint: BridgeTimepoint<T>,
    kind: IncomingTransactionRequestKind,
    network_id: BridgeNetworkId<T>,
}

impl<T: Config> LoadIncomingTransactionRequest<T> {
    pub fn new(
        author: T::AccountId,
        hash: H256,
        timepoint: Timepoint<T>,
        kind: IncomingTransactionRequestKind,
        network_id: T::NetworkId,
    ) -> Self {
        LoadIncomingTransactionRequest {
            author,
            hash,
            timepoint,
            kind,
            network_id,
        }
    }
}

/// Information needed for a request to be loaded from sidechain. Basically it's
/// a hash of the transaction and the type of the request.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct LoadIncomingMetaRequest<T: Config> {
    author: T::AccountId,
    hash: H256,
    timepoint: BridgeTimepoint<T>,
    kind: IncomingMetaRequestKind,
    network_id: BridgeNetworkId<T>,
}

impl<T: Config> LoadIncomingMetaRequest<T> {
    pub fn new(
        author: T::AccountId,
        hash: H256,
        timepoint: Timepoint<T>,
        kind: IncomingMetaRequestKind,
        network_id: T::NetworkId,
    ) -> Self {
        LoadIncomingMetaRequest {
            author,
            hash,
            timepoint,
            kind,
            network_id,
        }
    }
}

/// A bridge operation handled by off-chain workers.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub enum OffchainRequest<T: Config> {
    /// Thischain->Sidechain request with its hash.
    Outgoing(OutgoingRequest<T>, H256),
    /// Information required to load a corresponding incoming request from the sidechain network.
    LoadIncoming(LoadIncomingRequest<T>),
    /// Sidechain->Thischain request with its hash.
    Incoming(IncomingRequest<T>, H256),
}

impl<T: Config> OffchainRequest<T> {
    pub fn outgoing(request: OutgoingRequest<T>) -> Self {
        let mut request = Self::Outgoing(request, H256::zero());
        let hash = request.using_encoded(blake2_256);
        request.set_hash(H256(hash));
        request
    }

    pub fn load_incoming_meta(request: LoadIncomingMetaRequest<T>) -> Self {
        let mut request = Self::LoadIncoming(LoadIncomingRequest::Meta(request, H256::zero()));
        let hash = request.using_encoded(blake2_256);
        request.set_hash(H256(hash));
        request
    }

    pub fn incoming(request: IncomingRequest<T>) -> Self {
        let mut request = Self::Incoming(request, H256::zero());
        let hash = request.using_encoded(blake2_256);
        request.set_hash(H256(hash));
        request
    }

    /// Calculates or returns an already calculated request hash.
    fn hash(&self) -> H256 {
        match self {
            OffchainRequest::Outgoing(_request, hash) => *hash,
            OffchainRequest::LoadIncoming(request) => request.hash(),
            OffchainRequest::Incoming(_request, hash) => *hash,
        }
    }

    /// Calculates or returns an already calculated request hash.
    fn set_hash(&mut self, new_hash: H256) {
        match self {
            OffchainRequest::Outgoing(_request, hash) => *hash = new_hash,
            OffchainRequest::LoadIncoming(request) => request.set_hash(new_hash),
            OffchainRequest::Incoming(_request, hash) => *hash = new_hash,
        }
    }

    /// The request's network.
    fn network_id(&self) -> T::NetworkId {
        match self {
            OffchainRequest::Outgoing(request, _) => request.network_id(),
            OffchainRequest::LoadIncoming(request) => request.network_id(),
            OffchainRequest::Incoming(request, _) => request.network_id(),
        }
    }

    /// The request's timepoint.
    fn timepoint(&self) -> Timepoint<T> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.timepoint(),
            OffchainRequest::LoadIncoming(request) => request.timepoint(),
            OffchainRequest::Incoming(request, _) => request.timepoint(),
        }
    }

    /// An initiator of the request.
    fn author(&self) -> &T::AccountId {
        match self {
            OffchainRequest::Outgoing(request, _) => request.author(),
            OffchainRequest::LoadIncoming(request) => request.author(),
            OffchainRequest::Incoming(request, _) => request.author(),
        }
    }

    /// Checks that the request can be initiated (e.g., verifies that an account has
    /// sufficient funds for transfer).
    fn validate(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.validate(),
            OffchainRequest::LoadIncoming(request) => request.validate(),
            OffchainRequest::Incoming(request, _) => request.validate(),
        }
    }

    /// Performs additional state changes for the request (e.g., reserves funds for a transfer).
    fn prepare(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.prepare(),
            OffchainRequest::LoadIncoming(request) => request.prepare(),
            OffchainRequest::Incoming(request, _) => request.prepare(),
        }
    }

    /// Undos the state changes done in the `prepare` function.
    fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.cancel(),
            OffchainRequest::LoadIncoming(request) => request.cancel(),
            OffchainRequest::Incoming(request, _) => request.cancel(),
        }
    }

    pub fn finalize(&self) -> DispatchResult {
        match self {
            OffchainRequest::Outgoing(r, _) => r.finalize(),
            OffchainRequest::Incoming(r, _) => r.finalize().map(|_| ()),
            OffchainRequest::LoadIncoming(r) => r.finalize(),
        }
    }

    pub fn as_outgoing(&self) -> Option<(&OutgoingRequest<T>, H256)> {
        match self {
            OffchainRequest::Outgoing(r, h) => Some((r, *h)),
            _ => None,
        }
    }

    pub fn into_outgoing(self) -> Option<(OutgoingRequest<T>, H256)> {
        match self {
            OffchainRequest::Outgoing(r, h) => Some((r, h)),
            _ => None,
        }
    }

    pub fn into_incoming(self) -> Option<(IncomingRequest<T>, H256)> {
        match self {
            OffchainRequest::Incoming(r, h) => Some((r, h)),
            _ => None,
        }
    }

    pub fn as_incoming(&self) -> Option<(&IncomingRequest<T>, H256)> {
        match self {
            OffchainRequest::Incoming(r, h) => Some((r, *h)),
            _ => None,
        }
    }

    pub fn is_load_incoming(&self) -> bool {
        match self {
            OffchainRequest::LoadIncoming(..) => true,
            _ => false,
        }
    }

    pub fn is_incoming(&self) -> bool {
        match self {
            OffchainRequest::Incoming(..) => true,
            _ => false,
        }
    }
}

/// Ethereum-encoded `OutgoingRequest`. Contains a payload for signing by peers. Also, can be used
/// by client apps for more convenient contract function calls.
#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum OutgoingRequestEncoded {
    /// ETH-encoded incoming transfer from Substrate to Ethereum request.
    Transfer(OutgoingTransferEncoded),
    /// ETH-encoded 'add new asset' request.
    AddAsset(OutgoingAddAssetEncoded),
    /// ETH-encoded 'add new token' request.
    AddToken(OutgoingAddTokenEncoded),
    /// ETH-encoded 'add peer' request.
    AddPeer(OutgoingAddPeerEncoded),
    /// ETH-encoded 'remove peer' request.
    RemovePeer(OutgoingRemovePeerEncoded),
    /// ETH-encoded 'prepare for migration' request.
    PrepareForMigration(OutgoingPrepareForMigrationEncoded),
    /// ETH-encoded 'migrate' request.
    Migrate(OutgoingMigrateEncoded),
}

impl OutgoingRequestEncoded {
    #[allow(unused)]
    fn hash(&self) -> H256 {
        let hash = match self {
            OutgoingRequestEncoded::Transfer(transfer) => transfer.tx_hash,
            OutgoingRequestEncoded::AddAsset(request) => request.hash,
            OutgoingRequestEncoded::AddToken(request) => request.hash,
            OutgoingRequestEncoded::AddPeer(request) => request.tx_hash,
            OutgoingRequestEncoded::RemovePeer(request) => request.tx_hash,
            OutgoingRequestEncoded::PrepareForMigration(request) => request.tx_hash,
            OutgoingRequestEncoded::Migrate(request) => request.tx_hash,
        };
        H256(hash.0)
    }

    fn as_raw(&self) -> &[u8] {
        match self {
            OutgoingRequestEncoded::Transfer(transfer) => &transfer.raw,
            OutgoingRequestEncoded::AddAsset(request) => &request.raw,
            OutgoingRequestEncoded::AddToken(request) => &request.raw,
            OutgoingRequestEncoded::AddPeer(request) => &request.raw,
            OutgoingRequestEncoded::RemovePeer(request) => &request.raw,
            OutgoingRequestEncoded::PrepareForMigration(request) => &request.raw,
            OutgoingRequestEncoded::Migrate(request) => &request.raw,
        }
    }

    /// Returns Ethereum tokens needed for the corresponding contract function.
    pub fn input_tokens(&self, signatures: Option<Vec<SignatureParams>>) -> Vec<Token> {
        match self {
            OutgoingRequestEncoded::Transfer(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::AddAsset(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::AddToken(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::AddPeer(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::RemovePeer(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::PrepareForMigration(request) => {
                request.input_tokens(signatures)
            }
            OutgoingRequestEncoded::Migrate(request) => request.input_tokens(signatures),
        }
    }
}

/// Status of a registered request.
///
/// - Pending: request hasn't been approved yet.
/// - Frozen: request stopped receiving confirmations (signatures) from peers.
///   E.g. when the request is in 'cancellation' stage.
/// - ApprovalsReady: request was approved and can be used in the sidechain.
/// - Failed: an error occurred in one of the previous stages.
/// - Done: request was finalized.
#[derive(PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum RequestStatus {
    Pending,
    Frozen,
    ApprovalsReady,
    Failed(DispatchError),
    Done,
    /// Request is broken. Tried to abort with the first error but got another one when cancelling.
    Broken(DispatchError, DispatchError),
}

/// A type of asset registered on a bridge.
///
/// - Thischain: a Sora asset.
/// - Sidechain: an Ethereum token.
/// - SidechainOwned: an Ethereum token that can be minted on Sora.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum AssetKind {
    Thischain,
    Sidechain,
    SidechainOwned,
}

impl AssetKind {
    pub fn is_owned(&self) -> bool {
        self == &Self::Thischain || self == &Self::SidechainOwned
    }
}

/// Bridge asset parameters.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
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

/// Network-specific parameters.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct NetworkParams<AccountId: Ord> {
    pub bridge_contract_address: Address,
    pub initial_peers: BTreeSet<AccountId>,
}

/// Network configuration.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct NetworkConfig<T: Config> {
    pub initial_peers: BTreeSet<T::AccountId>,
    pub bridge_account_id: T::AccountId,
    pub assets: Vec<AssetConfig<T::AssetId>>,
    pub bridge_contract_address: Address,
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
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
pub enum BridgeStatus {
    Initialized,
    Migrating,
}

impl Default for BridgeStatus {
    fn default() -> Self {
        Self::Initialized
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use codec::Codec;
    use common::weights::{err_pays_no, pays_no, pays_no_with_maybe_weight};
    use frame_support::pallet_prelude::*;
    use frame_support::sp_runtime::traits::Zero;
    use frame_support::traits::schedule::{Anon, DispatchTime};
    use frame_support::traits::{GetCallMetadata, PalletVersion};
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + CreateSignedTransaction<Call<Self>>
        + CreateSignedTransaction<bridge_multisig::Call<Self>>
        + assets::Config
        + bridge_multisig::Config<Call = <Self as Config>::Call>
        + fmt::Debug
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// The identifier type for an offchain worker.
        type PeerId: AppCrypto<Self::Public, Self::Signature>;
        /// The overarching dispatch call type.
        type Call: From<Call<Self>>
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
        type Mock: mock::Mock;

        #[pallet::constant]
        type RemovePendingOutgoingRequestsAfter: Get<Self::BlockNumber>;

        #[pallet::constant]
        type TrackPendingIncomingRequestsAfter: Get<(Self::BlockNumber, u64)>;

        #[pallet::constant]
        type RemovePeerAccountIds: Get<Vec<(Self::AccountId, H160)>>;

        type SchedulerOriginCaller: From<frame_system::RawOrigin<Self::AccountId>>;
        type Scheduler: Anon<Self::BlockNumber, <Self as Config>::Call, Self::SchedulerOriginCaller>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
    where
        T: CreateSignedTransaction<<T as Config>::Call>,
    {
        /// Main off-chain worker procedure.
        ///
        /// Note: only one worker is expected to be used.
        fn offchain_worker(block_number: T::BlockNumber) {
            debug::debug!("Entering off-chain workers {:?}", block_number);
            if StorageValueRef::persistent(STORAGE_PEER_SECRET_KEY)
                .get::<Vec<u8>>()
                .is_none()
            {
                debug::debug!("Peer secret key not found. Skipping off-chain procedure.");
                return;
            }

            let mut lock = StorageLock::<'_, Time>::new(b"eth-bridge-ocw::lock");
            let _guard = lock.lock();
            Self::offchain();
        }

        fn on_runtime_upgrade() -> Weight {
            match Pallet::<T>::storage_version() {
                Some(version) if version == PalletVersion::new(0, 1, 0) => {
                    migrations::migrate_to_0_2_0::<T>()
                }
                _ => Weight::zero(),
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
        #[pallet::weight(<T as Config>::WeightInfo::register_bridge())]
        pub fn register_bridge(
            origin: OriginFor<T>,
            bridge_contract_address: EthereumAddress,
            initial_peers: Vec<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            let author = ensure_signed(origin)?;
            let net_id = NextNetworkId::<T>::get();
            let peers_account_id = bridge_multisig::Module::<T>::register_multisig_inner(
                author,
                initial_peers.clone(),
            )?;
            BridgeContractAddress::<T>::insert(net_id, bridge_contract_address);
            BridgeAccount::<T>::insert(net_id, peers_account_id);
            BridgeStatuses::<T>::insert(net_id, BridgeStatus::Initialized);
            Peers::<T>::insert(net_id, initial_peers.into_iter().collect::<BTreeSet<_>>());
            NextNetworkId::<T>::set(net_id + T::NetworkId::one());
            Ok(().into())
        }

        /// Add a Thischain asset to the bridge whitelist.
        ///
        /// Parameters:
        /// - `asset_id` - Thischain asset identifier.
        /// - `network_id` - network identifier to which the asset should be added.
        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::add_asset())]
        pub fn add_asset(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called add_asset");
            let from = ensure_signed(origin)?;
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Module::<T>::thischain_timepoint();
            Self::add_request(&OffchainRequest::outgoing(OutgoingRequest::AddAsset(
                OutgoingAddAsset {
                    author: from.clone(),
                    asset_id,
                    nonce,
                    network_id,
                    timepoint,
                },
            )))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
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
        #[pallet::weight(<T as Config>::WeightInfo::add_sidechain_token())]
        pub fn add_sidechain_token(
            origin: OriginFor<T>,
            token_address: EthereumAddress,
            symbol: String,
            name: String,
            decimals: u8,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called add_sidechain_token");
            ensure_root(origin)?;
            let from = Self::authority_account();
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Module::<T>::thischain_timepoint();
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
            frame_system::Module::<T>::inc_account_nonce(&from);
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
        #[pallet::weight(<T as Config>::WeightInfo::transfer_to_sidechain())]
        pub fn transfer_to_sidechain(
            origin: OriginFor<T>,
            asset_id: AssetIdOf<T>,
            to: EthereumAddress,
            amount: Balance,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called transfer_to_sidechain");
            let from = ensure_signed(origin)?;
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Module::<T>::thischain_timepoint();
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
            frame_system::Module::<T>::inc_account_nonce(&from);
            Ok(().into())
        }

        /// Load incoming request from Sidechain by the given transaction hash.
        ///
        /// Parameters:
        /// - `eth_tx_hash` - transaction hash on Sidechain.
        /// - `kind` - incoming request type.
        /// - `network_id` - network identifier.
        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::request_from_sidechain())]
        pub fn request_from_sidechain(
            origin: OriginFor<T>,
            eth_tx_hash: H256,
            kind: IncomingRequestKind,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called request_from_sidechain");
            let from = ensure_signed(origin)?;
            let timepoint = bridge_multisig::Module::<T>::thischain_timepoint();
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
                    let timepoint = bridge_multisig::Module::<T>::thischain_timepoint();
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
        #[pallet::weight(<T as Config>::WeightInfo::finalize_incoming_request())]
        pub fn finalize_incoming_request(
            origin: OriginFor<T>,
            hash: H256,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called finalize_incoming_request");
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
        #[pallet::weight(<T as Config>::WeightInfo::add_peer())]
        pub fn add_peer(
            origin: OriginFor<T>,
            account_id: T::AccountId,
            address: EthereumAddress,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called change_peers_out");
            ensure_root(origin)?;
            let from = Self::authority_account();
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Module::<T>::thischain_timepoint();
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
            frame_system::Module::<T>::inc_account_nonce(&from);
            if network_id == T::GetEthNetworkId::get() {
                let nonce = frame_system::Module::<T>::account_nonce(&from);
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
                frame_system::Module::<T>::inc_account_nonce(&from);
            }
            Ok(().into())
        }

        /// Remove peer from the the bridge peers set.
        ///
        /// Parameters:
        /// - `account_id` - account id on thischain.
        /// - `network_id` - network identifier.
        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::remove_peer())]
        pub fn remove_peer(
            origin: OriginFor<T>,
            account_id: T::AccountId,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called change_peers_out");
            ensure_root(origin)?;
            let from = Self::authority_account();
            let peer_address = Self::peer_address(network_id, &account_id);
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Module::<T>::thischain_timepoint();
            Self::add_request(&OffchainRequest::outgoing(OutgoingRequest::RemovePeer(
                OutgoingRemovePeer {
                    author: from.clone(),
                    peer_account_id: account_id.clone(),
                    peer_address,
                    nonce,
                    network_id,
                    timepoint,
                },
            )))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
            if network_id == T::GetEthNetworkId::get() {
                let nonce = frame_system::Module::<T>::account_nonce(&from);
                Self::add_request(&OffchainRequest::outgoing(
                    OutgoingRequest::RemovePeerCompat(OutgoingRemovePeerCompat {
                        author: from.clone(),
                        peer_account_id: account_id,
                        peer_address,
                        nonce,
                        network_id,
                        timepoint,
                    }),
                ))?;
                frame_system::Module::<T>::inc_account_nonce(&from);
            }
            Ok(().into())
        }

        /// Prepare the given bridge for migration.
        ///
        /// Can only be called by an authority.
        ///
        /// Parameters:
        /// - `network_id` - bridge network identifier.
        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::prepare_for_migration())]
        pub fn prepare_for_migration(
            origin: OriginFor<T>,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called prepare_for_migration");
            ensure_root(origin)?;
            let from = Self::authority_account();
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Module::<T>::thischain_timepoint();
            Self::add_request(&OffchainRequest::outgoing(
                OutgoingRequest::PrepareForMigration(OutgoingPrepareForMigration {
                    author: from.clone(),
                    nonce,
                    network_id,
                    timepoint,
                }),
            ))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
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
        #[pallet::weight(<T as Config>::WeightInfo::migrate())]
        pub fn migrate(
            origin: OriginFor<T>,
            new_contract_address: EthereumAddress,
            erc20_native_tokens: Vec<EthereumAddress>,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called prepare_for_migration");
            ensure_root(origin)?;
            let from = Self::authority_account();
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            let timepoint = bridge_multisig::Module::<T>::thischain_timepoint();
            Self::add_request(&OffchainRequest::outgoing(OutgoingRequest::Migrate(
                OutgoingMigrate {
                    author: from.clone(),
                    new_contract_address,
                    erc20_native_tokens,
                    nonce,
                    network_id,
                    timepoint,
                },
            )))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
            Ok(().into())
        }

        /// Register the given incoming request and add it to the queue.
        ///
        /// Calls `validate` and `prepare` on the request, adds it to the queue and maps it with the
        /// corresponding load-incoming-request and removes the load-request from the queue.
        ///
        /// Can only be called by a bridge account.
        #[pallet::weight(<T as Config>::WeightInfo::register_incoming_request())]
        pub fn register_incoming_request(
            origin: OriginFor<T>,
            incoming_request: IncomingRequest<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called register_incoming_request");
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
        #[pallet::weight(<T as Config>::WeightInfo::import_incoming_request(incoming_request_result.is_ok()))]
        pub fn import_incoming_request(
            origin: OriginFor<T>,
            load_incoming_request: LoadIncomingRequest<T>,
            incoming_request_result: Result<IncomingRequest<T>, DispatchError>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called import_incoming_request");
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
        #[pallet::weight(<T as Config>::WeightInfo::approve_request())]
        pub fn approve_request(
            origin: OriginFor<T>,
            ocw_public: ecdsa::Public,
            hash: H256,
            signature_params: SignatureParams,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called approve_request");
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
        #[pallet::weight(<T as Config>::WeightInfo::abort_request())]
        pub fn abort_request(
            origin: OriginFor<T>,
            hash: H256,
            error: DispatchError,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!(
                "called abort_request. Hash: {:?}, reason: {:?}",
                hash,
                error
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
        #[pallet::weight(<T as Config>::WeightInfo::force_add_peer())]
        pub fn force_add_peer(
            origin: OriginFor<T>,
            who: T::AccountId,
            address: EthereumAddress,
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

        #[pallet::weight(0)]
        pub fn migrate_to_0_2_0(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let weight = match Pallet::<T>::storage_version() {
                Some(version) if version == PalletVersion::new(0, 1, 0) => {
                    let bridge_multisig = crate::BridgeAccount::<T>::get(T::GetEthNetworkId::get())
                        .unwrap_or_default();
                    let mut migrating_requests = MigratingRequests::<T>::get();
                    migrating_requests.retain(|hash| {
                        bridge_multisig::Multisigs::<T>::contains_key(&bridge_multisig, &hash.0)
                    });
                    // Wait for all the previous requests to finish and then finish the migration
                    if migrating_requests.is_empty() {
                        migrations::remove_peers::<T>(&T::RemovePeerAccountIds::get());
                    } else {
                        // ...or postpone the migration.
                        let block_number = frame_system::Pallet::<T>::current_block_number();
                        let _ = T::Scheduler::schedule(
                            DispatchTime::At(block_number + 100u32.into()),
                            None,
                            1,
                            RawOrigin::Root.into(),
                            Call::migrate_to_0_2_0().into(),
                        );
                    }
                    MigratingRequests::<T>::set(migrating_requests);
                    <T as frame_system::Config>::DbWeight::get().reads_writes(2, 1)
                }
                _ => Weight::zero(),
            };
            Ok(Some(weight).into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
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
        T::BlockNumber,
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
        Address,
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
        Address,
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
        Address,
        T::AccountId,
        ValueQuery,
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
        Address,
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
    pub(super) type AuthorityAccount<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

    /// Bridge status.
    #[pallet::storage]
    #[pallet::getter(fn bridge_contract_status)]
    pub(super) type BridgeStatuses<T: Config> =
        StorageMap<_, Twox64Concat, BridgeNetworkId<T>, BridgeStatus>;

    /// Smart-contract address on Sidechain.
    #[pallet::storage]
    #[pallet::getter(fn bridge_contract_address)]
    pub(super) type BridgeContractAddress<T: Config> =
        StorageMap<_, Twox64Concat, BridgeNetworkId<T>, Address, ValueQuery>;

    /// Sora XOR master contract address.
    #[pallet::storage]
    #[pallet::getter(fn xor_master_contract_address)]
    pub(super) type XorMasterContractAddress<T: Config> = StorageValue<_, Address, ValueQuery>;

    /// Sora VAL master contract address.
    #[pallet::storage]
    #[pallet::getter(fn val_master_contract_address)]
    pub(super) type ValMasterContractAddress<T: Config> = StorageValue<_, Address, ValueQuery>;

    /// Next Network ID counter.
    #[pallet::storage]
    pub(super) type NextNetworkId<T: Config> = StorageValue<_, BridgeNetworkId<T>, ValueQuery>;

    /// Requests migrating from version '0.1.0' to '0.2.0'. These requests should be removed from
    /// `RequestsQueue` before migration procedure started.
    #[pallet::storage]
    pub(super) type MigratingRequests<T: Config> = StorageValue<_, Vec<H256>, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub authority_account: T::AccountId,
        pub xor_master_contract_address: Address,
        pub val_master_contract_address: Address,
        pub networks: Vec<NetworkConfig<T>>,
    }

    #[cfg(feature = "std")]
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
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            AuthorityAccount::<T>::put(&self.authority_account);
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
                        let token_address = Address::from(sidechain_id.0);
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
                    permissions::Module::<T>::assign_permission(
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

pub fn majority(peers_count: usize) -> usize {
    peers_count - (peers_count - 1) / 3
}

/// Contract's deposit event, means that someone transferred some amount of the token/asset to the
/// bridge contract.
#[cfg_attr(feature = "std", derive(PartialEq, Eq, RuntimeDebug))]
pub struct DepositEvent<Address, AccountId, Balance> {
    destination: AccountId,
    amount: Balance,
    token: Address,
    sidechain_asset: H256,
}

impl<Address, AccountId, Balance> DepositEvent<Address, AccountId, Balance> {
    pub fn new(
        destination: AccountId,
        amount: Balance,
        token: Address,
        sidechain_asset: H256,
    ) -> Self {
        DepositEvent {
            destination,
            amount,
            token,
            sidechain_asset,
        }
    }
}

/// Events that can be emitted by Sidechain smart-contract.
#[cfg_attr(feature = "std", derive(PartialEq, Eq, RuntimeDebug))]
pub enum ContractEvent<Address, AccountId, Balance> {
    Deposit(DepositEvent<Address, AccountId, Balance>),
    ChangePeers(Address, bool),
    PreparedForMigration,
    Migrated(Address),
}

/// A helper for encoding bridge types into ethereum tokens.
#[derive(PartialEq)]
pub struct Decoder<T: Config> {
    tokens: Vec<Token>,
    _phantom: PhantomData<T>,
}

impl<T: Config> Decoder<T> {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            _phantom: PhantomData,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn next_string(&mut self) -> Result<String, Error<T>> {
        self.tokens
            .pop()
            .and_then(|x| x.into_string())
            .ok_or_else(|| Error::<T>::InvalidString.into())
    }

    pub fn next_bool(&mut self) -> Result<bool, Error<T>> {
        self.tokens
            .pop()
            .and_then(|x| x.into_bool())
            .ok_or_else(|| Error::<T>::InvalidBool.into())
    }

    pub fn next_u8(&mut self) -> Result<u8, Error<T>> {
        self.tokens
            .pop()
            .and_then(|x| x.into_uint())
            .filter(|x| x.as_u32() <= u8::MAX as u32)
            .map(|x| x.as_u32() as u8)
            .ok_or_else(|| Error::<T>::InvalidByte.into())
    }

    pub fn next_address(&mut self) -> Result<Address, Error<T>> {
        Ok(H160(
            self.tokens
                .pop()
                .and_then(|x| x.into_address())
                .ok_or(Error::<T>::InvalidAddress)?
                .0,
        ))
    }

    pub fn next_balance(&mut self) -> Result<Balance, Error<T>> {
        Ok(Balance::from(
            u128::try_from(
                self.tokens
                    .pop()
                    .and_then(|x| x.into_uint())
                    .ok_or(Error::<T>::InvalidUint)?,
            )
            .map_err(|_| Error::<T>::InvalidBalance)?,
        ))
    }

    pub fn next_amount(&mut self) -> Result<Balance, Error<T>> {
        Ok(u128::try_from(
            self.tokens
                .pop()
                .and_then(|x| x.into_uint())
                .ok_or(Error::<T>::InvalidUint)?,
        )
        .map_err(|_| Error::<T>::InvalidAmount)?)
    }

    pub fn next_account_id(&mut self) -> Result<T::AccountId, Error<T>> {
        Ok(T::AccountId::decode(
            &mut &self
                .tokens
                .pop()
                .and_then(|x| x.into_fixed_bytes())
                .ok_or(Error::<T>::InvalidAccountId)?[..],
        )
        .map_err(|_| Error::<T>::InvalidAccountId)?)
    }

    pub fn next_asset_id(&mut self) -> Result<T::AssetId, Error<T>> {
        Ok(T::AssetId::decode(&mut &self.next_h256()?.0[..])
            .map_err(|_| Error::<T>::InvalidAssetId)?)
    }

    pub fn parse_h256(token: Token) -> Option<H256> {
        <[u8; 32]>::try_from(token.into_fixed_bytes()?)
            .ok()
            .map(H256)
    }

    pub fn next_h256(&mut self) -> Result<H256, Error<T>> {
        self.tokens
            .pop()
            .and_then(Self::parse_h256)
            .ok_or_else(|| Error::<T>::InvalidH256.into())
    }

    pub fn next_array(&mut self) -> Result<Vec<Token>, Error<T>> {
        self.tokens
            .pop()
            .and_then(|x| x.into_array())
            .ok_or_else(|| Error::<T>::InvalidArray.into())
    }

    pub fn next_array_map<U, F: FnMut(&mut Decoder<T>) -> Result<U, Error<T>>>(
        &mut self,
        mut f: F,
    ) -> Result<Vec<U>, Error<T>> {
        let mut decoder = Decoder::<T>::new(self.next_array()?);
        iter::repeat(())
            .map(|_| f(&mut decoder))
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn next_signature_params(&mut self) -> Result<Vec<SignatureParams>, Error<T>> {
        let rs = self.next_array_map(|d| d.next_h256().map(|x| x.0))?;
        let ss = self.next_array_map(|d| d.next_h256().map(|x| x.0))?;
        let vs = self.next_array_map(|d| d.next_u8())?;
        Ok(rs
            .into_iter()
            .zip(ss)
            .zip(vs)
            .map(|((r, s), v)| SignatureParams { r, s, v })
            .collect())
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
        let block_number = frame_system::Module::<T>::current_block_number();
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
            debug::warn!("{:?}", e);
            return Err(e.into());
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
            debug::error!("Incoming request failed {:?} {:?}", hash, e);
            Self::deposit_event(Event::IncomingRequestFinalizationFailed(hash));
            RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Failed(e));
            cancel!(request, hash, network_id, e);
            Self::remove_request_from_queue(network_id, &hash);
            return Err(e);
        } else {
            debug::warn!("Incoming request finalized {:?}", hash);
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

    fn parse_deposit_event(
        log: &Log,
    ) -> Result<DepositEvent<Address, T::AccountId, Balance>, Error<T>> {
        if log.removed.unwrap_or(true) {
            return Err(Error::<T>::EthLogWasRemoved);
        }
        let types = [
            ParamType::FixedBytes(32),
            ParamType::Uint(256),
            ParamType::Address,
            ParamType::FixedBytes(32),
        ];
        let decoded =
            ethabi::decode(&types, &log.data.0).map_err(|_| Error::<T>::EthAbiDecodingError)?;
        let mut decoder = Decoder::<T>::new(decoded);
        let sidechain_asset = decoder.next_h256()?;
        let token = decoder.next_address()?;
        let amount = decoder.next_amount()?;
        let destination = decoder.next_account_id()?;
        Ok(DepositEvent {
            destination,
            amount,
            token,
            sidechain_asset,
        })
    }

    /// Loops through the given array of logs and finds the first one that matches the type
    /// and topic.
    fn parse_main_event(
        logs: &[Log],
        kind: IncomingTransactionRequestKind,
    ) -> Result<ContractEvent<Address, T::AccountId, Balance>, Error<T>> {
        for log in logs {
            if log.removed.unwrap_or(true) {
                continue;
            }
            let topic = match log.topics.get(0) {
                Some(x) => &x.0,
                None => continue,
            };
            match *topic {
                topic
                    if topic == DEPOSIT_TOPIC.0
                        && (kind == IncomingTransactionRequestKind::Transfer
                            || kind == IncomingTransactionRequestKind::TransferXOR) =>
                {
                    return Ok(ContractEvent::Deposit(Self::parse_deposit_event(log)?));
                }
                // ChangePeers(address,bool)
                hex!("a9fac23eb012e72fbd1f453498e7069c380385436763ee2c1c057b170d88d9f9")
                    if kind == IncomingTransactionRequestKind::AddPeer
                        || kind == IncomingTransactionRequestKind::RemovePeer =>
                {
                    let types = [ParamType::Address, ParamType::Bool];
                    let decoded = ethabi::decode(&types, &log.data.0)
                        .map_err(|_| Error::<T>::EthAbiDecodingError)?;
                    let mut decoder = Decoder::<T>::new(decoded);
                    let added = decoder.next_bool()?;
                    let peer_address = decoder.next_address()?;
                    return Ok(ContractEvent::ChangePeers(H160(peer_address.0), added));
                }
                hex!("5389de9593f75e6515eefa796bd2d3324759f441f2c9b2dcda0efb25190378ff")
                    if kind == IncomingTransactionRequestKind::PrepareForMigration =>
                {
                    return Ok(ContractEvent::PreparedForMigration);
                }
                hex!("a2e7361c23d7820040603b83c0cd3f494d377bac69736377d75bb56c651a5098")
                    if kind == IncomingTransactionRequestKind::Migrate =>
                {
                    let types = [ParamType::Address];
                    let decoded = ethabi::decode(&types, &log.data.0)
                        .map_err(|_| Error::<T>::EthAbiDecodingError)?;
                    let mut decoder = Decoder::<T>::new(decoded);
                    let account_id = decoder.next_address()?;
                    return Ok(ContractEvent::Migrated(account_id));
                }
                _ => (),
            }
        }
        Err(Error::<T>::UnknownEvent.into())
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
        debug::info!("Verified request approve {:?}", request_encoded);
        let mut approvals = RequestApprovals::<T>::get(net_id, &hash);
        let pending_peers_len = if PendingPeer::<T>::get(net_id).is_some() {
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
            if let Err(err) = request.finalize() {
                debug::error!("Outgoing request finalization failed: {:?}", err);
                RequestStatuses::<T>::insert(net_id, hash, RequestStatus::Failed(err));
                Self::deposit_event(Event::RequestFinalizationFailed(hash));
                cancel!(request, hash, net_id, err);
            } else {
                debug::debug!("Outgoing request approvals collected {:?}", hash);
                RequestStatuses::<T>::insert(net_id, hash, RequestStatus::ApprovalsReady);
                Self::deposit_event(Event::ApprovalsCollected(hash));
            }
            Self::remove_request_from_queue(net_id, &hash);
            let weight_info = <T as Config>::WeightInfo::approve_request_finalize();
            return Ok(Some(weight_info));
        }
        Ok(None)
    }

    /// Verifies the message signed by a peer. Also, compares the given `AccountId` with the given
    /// public key.
    fn verify_message(
        msg: &[u8],
        signature: &SignatureParams,
        ecdsa_public_key: &ecdsa::Public,
        author: &T::AccountId,
    ) -> bool {
        let message = eth::prepare_message(msg);
        let mut arr = [0u8; 65];
        arr[..32].copy_from_slice(&signature.r[..]);
        arr[32..64].copy_from_slice(&signature.s[..]);
        arr[64] = signature.v;
        let res = secp256k1::Signature::parse_slice(&arr[..64]).and_then(|sig| {
            secp256k1::PublicKey::parse_slice(ecdsa_public_key.as_slice(), None).map(|pk| (sig, pk))
        });
        if let Ok((signature, public_key)) = res {
            let signer_account = MultiSigner::Ecdsa(ecdsa_public_key.clone()).into_account();
            let verified = secp256k1::verify(&message, &signature, &public_key);
            signer_account.encode() == author.encode() && verified
        } else {
            false
        }
    }

    /// Signs a message with a peer's secret key.
    fn sign_message(
        msg: &[u8],
        secret_key: &secp256k1::SecretKey,
    ) -> (SignatureParams, secp256k1::PublicKey) {
        let message = eth::prepare_message(msg);
        let (sig, v) = secp256k1::sign(&message, secret_key);
        let pk = secp256k1::PublicKey::from_secret_key(secret_key);
        let v = v.serialize();
        let sig_ser = sig.serialize();
        (
            SignatureParams {
                r: sig_ser[..32].try_into().unwrap(),
                s: sig_ser[32..].try_into().unwrap(),
                v,
            },
            pk,
        )
    }

    /// Procedure for handling incoming requests.
    ///
    /// For an incoming request, a premise for its finalization will be block confirmation in PoW
    /// consensus. Since the confirmation is probabilistic, we need to choose a relatively large
    /// number of how many blocks should be mined after a corresponding transaction
    /// (`CONFIRMATION_INTERVAL`).
    ///
    /// An off-chain worker keeps track of already handled requests in local storage.
    fn handle_pending_incoming_requests(
        request: IncomingRequest<T>,
        hash: H256,
    ) -> Result<(), Error<T>> {
        let network_id = request.network_id();
        // TODO (optimization): send results as batch.
        // Load the transaction receipt again to check that it still exists.
        let exists = request.check_existence()?;
        ensure!(exists, Error::<T>::EthTransactionIsFailed);
        Self::send_finalize_incoming_request(hash, request.timepoint(), network_id)
    }

    /// Removes all extrinsics presented in the `block` from pending transactions storage. If some
    /// extrinsic weren't sent or finalized for a long time
    /// (`SUBSTRATE_MAX_BLOCK_NUM_EXPECTING_UNTIL_FINALIZATION`), it's re-sent.
    fn handle_substrate_block(
        block: SubstrateBlockLimited,
        finalized_height: T::BlockNumber,
    ) -> Result<(), Error<T>>
    where
        T: CreateSignedTransaction<<T as Config>::Call>,
    {
        let s_pending_txs = StorageValueRef::persistent(STORAGE_PENDING_TRANSACTIONS_KEY);
        if let Some(mut txs) = s_pending_txs
            .get::<BTreeMap<H256, SignedTransactionData<T>>>()
            .flatten()
        {
            debug::debug!("Pending txs count: {}", txs.len());
            for ext in block.extrinsics {
                let vec = ext.encode();
                let hash = H256(blake2_256(&vec));
                // Transaction has been finalized, remove it from pending txs.
                txs.remove(&hash);
            }
            let signer = Self::get_signer()?;
            // Re-send all transactions that weren't sent or finalized.
            let mut resent_num = 0;
            let mut failed_txs_tmp = Vec::new();
            let txs = txs
                .into_iter()
                .filter_map(|(mut hash, mut tx)| {
                    let not_sent_or_too_long_finalization = tx
                        .submitted_at
                        .map(|submitted_height| {
                            finalized_height
                                > submitted_height
                                    + SUBSTRATE_MAX_BLOCK_NUM_EXPECTING_UNTIL_FINALIZATION.into()
                        })
                        .unwrap_or(true);
                    if not_sent_or_too_long_finalization {
                        let should_resend = resent_num < MAX_SUCCESSFUL_SENT_SIGNED_TX_PER_ONCE;
                        if should_resend {
                            if tx.resend(&signer) {
                                resent_num += 1;
                                // Update key = extrinsic hash.
                            } else {
                                let key = format!(
                                    "eth-bridge-ocw::pending-transactions-retries-v2-{:?}",
                                    tx.extrinsic_hash
                                );
                                let mut s_retries = StorageValueRef::persistent(key.as_bytes());
                                let mut retries: u16 =
                                    s_retries.get().flatten().unwrap_or_default();
                                retries = retries.saturating_add(1);
                                // If re-send limit exceeded - remove.
                                if retries > MAX_FAILED_SEND_SIGNED_TX_RETRIES {
                                    failed_txs_tmp.push(tx);
                                    s_retries.clear();
                                    return None;
                                } else {
                                    s_retries.set(&retries);
                                }
                            }
                        }
                        hash = tx.extrinsic_hash;
                    }
                    Some((hash, tx))
                })
                .collect::<BTreeMap<H256, SignedTransactionData<T>>>();
            if !failed_txs_tmp.is_empty() {
                let s_failed_pending_txs =
                    StorageValueRef::persistent(STORAGE_FAILED_PENDING_TRANSACTIONS_KEY);
                let mut failed_txs = s_failed_pending_txs
                    .get::<BTreeMap<H256, SignedTransactionData<T>>>()
                    .flatten()
                    .unwrap_or_default();
                for tx in failed_txs_tmp {
                    failed_txs.insert(tx.extrinsic_hash, tx);
                }
                s_failed_pending_txs.set(&failed_txs);
            }
            s_pending_txs.set(&txs);
        }
        Ok(())
    }

    /// Queries the current finalized block of the local node with `chain_getFinalizedHead` and
    /// `chain_getHeader` RPC calls.
    fn load_substrate_finalized_header() -> Result<SubstrateHeaderLimited, Error<T>>
    where
        T: CreateSignedTransaction<<T as Config>::Call>,
    {
        let hash =
            Self::substrate_json_rpc_request::<_, types::H256>("chain_getFinalizedHead", &())?;
        let header = Self::substrate_json_rpc_request::<_, types::SubstrateHeaderLimited>(
            "chain_getHeader",
            &[hash],
        )?;
        Ok(header)
    }

    /// Queries a block at the given height of the local node with `chain_getBlockHash` and
    /// `chain_getBlock` RPC calls.
    fn load_substrate_block(number: T::BlockNumber) -> Result<SubstrateBlockLimited, Error<T>>
    where
        T: CreateSignedTransaction<<T as Config>::Call>,
    {
        let int: u32 = number
            .try_into()
            .map_err(|_| ())
            .expect("block number is always at least u32; qed");
        let hash =
            Self::substrate_json_rpc_request::<_, types::H256>("chain_getBlockHash", &[int])?;
        let block = Self::substrate_json_rpc_request::<_, types::SubstrateSignedBlockLimited>(
            "chain_getBlock",
            &[hash],
        )?;
        Ok(block.block)
    }

    /// Queries the sidechain node for the transfer logs emitted within `from_block` and `to_block`.
    ///
    /// Uses the `eth_getLogs` method with a filter on log topic.
    fn load_transfers_logs(
        network_id: T::NetworkId,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<Log>, Error<T>> {
        Self::eth_json_rpc_request(
            "eth_getLogs",
            &[FilterBuilder::default()
                .topics(Some(vec![types::H256(DEPOSIT_TOPIC.0)]), None, None, None)
                .from_block(BlockNumber::Number(from_block.into()))
                .to_block(BlockNumber::Number(to_block.into()))
                .address(vec![types::H160(
                    BridgeContractAddress::<T>::get(network_id).0,
                )])
                .build()],
            network_id,
        )
    }

    /// Sends a multisig transaction to register the parsed (from pre-incoming) incoming request.
    /// (see `register_incoming_request`).
    fn send_register_incoming_request(
        incoming_request: IncomingRequest<T>,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        let register_call = Call::<T>::register_incoming_request(incoming_request);
        Self::send_multisig_transaction(register_call, timepoint, network_id)
    }

    /// Parses a 'cancel' incoming request from the given transaction receipt and pre-request.
    ///
    /// This special flow (unlike `parse_incoming_request`) requires transaction to be _failed_,
    /// because only in this case the initial request can be cancelled.
    fn parse_cancel_incoming_request(
        tx_receipt: TransactionReceipt,
        pre_request: LoadIncomingMetaRequest<T>,
        pre_request_hash: H256,
    ) -> Result<IncomingRequest<T>, Error<T>> {
        let tx_approved = tx_receipt.is_approved();
        ensure!(!tx_approved, Error::<T>::EthTransactionIsSucceeded);
        let at_height = tx_receipt
            .block_number
            .expect("'block_number' is null only when the log/transaction is pending; qed")
            .as_u64();
        let tx = Self::load_tx(H256(tx_receipt.transaction_hash.0), pre_request.network_id)?;
        ensure!(
            tx_receipt
                .gas_used
                .map(|used| used != tx.gas)
                .unwrap_or(false),
            Error::<T>::TransactionMightHaveFailedDueToGasLimit
        );
        ensure!(
            tx.input.0.len() >= METHOD_ID_SIZE,
            Error::<T>::InvalidFunctionInput
        );
        let mut method_id = [0u8; METHOD_ID_SIZE];
        method_id.clone_from_slice(&tx.input.0[..METHOD_ID_SIZE]);
        let funs = FUNCTIONS.get_or_init(functions);
        let fun_meta = funs.get(&method_id).ok_or(Error::<T>::UnknownMethodId)?;
        let fun = &fun_meta.function;
        let tokens = fun
            .decode_input(&tx.input.0)
            .map_err(|_| Error::<T>::InvalidFunctionInput)?;
        let hash = parse_hash_from_call::<T>(tokens, fun_meta.tx_hash_arg_pos)?;
        let oc_request: OffchainRequest<T> =
            crate::Requests::<T>::get(pre_request.network_id, hash)
                .ok_or(Error::<T>::UnknownRequest)?;
        let request = oc_request
            .into_outgoing()
            .ok_or(Error::<T>::ExpectedOutgoingRequest)?
            .0;
        let author = pre_request.author;
        ensure!(
            request.author() == &author,
            Error::<T>::RequestIsNotOwnedByTheAuthor
        );
        Ok(IncomingRequest::CancelOutgoingRequest(
            IncomingCancelOutgoingRequest {
                outgoing_request: request,
                outgoing_request_hash: hash,
                initial_request_hash: pre_request_hash,
                author,
                tx_input: tx.input.0,
                tx_hash: pre_request.hash,
                at_height,
                timepoint: pre_request.timepoint,
                network_id: pre_request.network_id,
            },
        ))
    }

    /// Handles a special case of an incoming request - marking as done.
    ///
    /// This special flow (unlike `parse_incoming_request`) only queries the contract's `used`
    /// variable to check if the request was actually made.
    fn handle_mark_as_done_incoming_request(
        pre_request: LoadIncomingMetaRequest<T>,
        pre_request_hash: H256,
    ) -> Result<(), Error<T>> {
        let network_id = pre_request.network_id;
        let at_height = Self::load_current_height(network_id)?;
        let timepoint = pre_request.timepoint;
        let request = IncomingRequest::MarkAsDone(IncomingMarkAsDoneRequest {
            outgoing_request_hash: pre_request_hash,
            initial_request_hash: pre_request.hash,
            author: pre_request.author,
            at_height,
            timepoint,
            network_id,
        });
        let is_used = request.check_existence()?;
        ensure!(is_used, Error::<T>::RequestNotFinalizedOnSidechain);
        Self::send_register_incoming_request(request, timepoint, network_id)
    }

    /// Handles the given off-chain request.
    ///
    /// The function delegates further handling depending on request type.
    /// There are 4 flows. 3 for incoming request: handle 'mark as done', handle 'cancel outgoing
    /// request' and for the rest, and only one for all outgoing requests.
    fn handle_offchain_request(request: OffchainRequest<T>) -> Result<(), Error<T>> {
        debug::debug!("Handling request: {:?}", request.hash());
        match request {
            OffchainRequest::LoadIncoming(request) => {
                let network_id = request.network_id();
                let timepoint = request.timepoint();
                match request {
                    LoadIncomingRequest::Transaction(request) => {
                        let tx_hash = request.hash;
                        let kind = request.kind;
                        debug::debug!("Loading approved tx {}", tx_hash);
                        let tx = Self::load_tx_receipt(tx_hash, network_id)?;
                        let mut incoming_request = Self::parse_incoming_request(tx, request)?;
                        // TODO: this flow was used to transfer XOR for free with the `request_from_sidechain`
                        // extrinsic. This could be used to spam network with transactions. Now it's unneeded,
                        // since all incoming transactions are loaded automatically. The extrinsic and related
                        // code should be considered for deletion.
                        if kind == IncomingTransactionRequestKind::TransferXOR {
                            if let IncomingRequest::Transfer(transfer) = &mut incoming_request {
                                ensure!(
                                    transfer.asset_id == common::XOR.into(),
                                    Error::<T>::ExpectedXORTransfer
                                );
                            } else {
                                fail!(Error::<T>::ExpectedXORTransfer)
                            }
                        }
                        Self::send_register_incoming_request(
                            incoming_request,
                            timepoint,
                            network_id,
                        )
                    }
                    LoadIncomingRequest::Meta(request, hash) => {
                        let kind = request.kind;
                        match kind {
                            IncomingMetaRequestKind::MarkAsDone => {
                                Self::handle_mark_as_done_incoming_request(request, hash)
                            }
                            IncomingMetaRequestKind::CancelOutgoingRequest => {
                                let tx_hash = request.hash;
                                let tx = Self::load_tx_receipt(tx_hash, network_id)?;
                                let incoming_request =
                                    Self::parse_cancel_incoming_request(tx, request, hash)?;
                                Self::send_register_incoming_request(
                                    incoming_request,
                                    timepoint,
                                    network_id,
                                )
                            }
                        }
                    }
                }
            }
            OffchainRequest::Outgoing(request, hash) => {
                Self::handle_outgoing_request(request, hash)
            }
            OffchainRequest::Incoming(request, hash) => {
                Self::handle_pending_incoming_requests(request, hash)
            }
        }
    }

    /// Parses the logs emitted on the Sidechain's contract to an `IncomingRequest` and imports it
    /// to Thischain.
    fn handle_logs(
        from_block: u64,
        to_block: u64,
        new_to_handle_height: &mut u64,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        let logs = Self::load_transfers_logs(network_id, from_block, to_block)?;
        for log in logs {
            // We assume that all events issued by our contracts are valid and, therefore, ignore
            // the invalid ones.
            let event = match Self::parse_deposit_event(&log) {
                Ok(v) => v,
                Err(e) => {
                    debug::info!("Skipped {:?}, error: {:?}", log, e);
                    continue;
                }
            };
            let tx_hash = H256(
                log.transaction_hash
                    .ok_or(Error::<T>::EthTransactionIsPending)?
                    .0,
            );
            if RequestStatuses::<T>::get(network_id, tx_hash).is_some() {
                // Skip already submitted requests.
                continue;
            }
            let at_height = log
                .block_number
                .ok_or(Error::<T>::EthTransactionIsPending)?
                .as_u64();
            let transaction_index = log
                .transaction_index
                .ok_or(Error::<T>::EthTransactionIsPending)?
                .as_u64();
            let timepoint = bridge_multisig::Pallet::<T>::sidechain_timepoint(
                at_height,
                transaction_index as u32, // TODO: can it exceed u32?
            );
            debug::info!("Got log [{}], {:?}", at_height, log);
            let load_incoming_transaction_request = LoadIncomingTransactionRequest::new(
                event.destination.clone(),
                tx_hash,
                timepoint,
                IncomingTransactionRequestKind::Transfer,
                network_id,
            );
            let inc_request_result = IncomingRequest::try_from_contract_event(
                ContractEvent::Deposit(event),
                load_incoming_transaction_request.clone(),
                at_height,
            );
            Self::send_import_incoming_request(
                LoadIncomingRequest::Transaction(load_incoming_transaction_request),
                inc_request_result.map_err(|e| e.into()),
                network_id,
            )?;
            // Update 'handled height' value when all logs at `at_height` were processed.
            if at_height > *new_to_handle_height {
                *new_to_handle_height = at_height;
                // Return to not overload the node.
                return Ok(());
            }
        }
        // All logs in the given range were processed.
        *new_to_handle_height = to_block + 1;
        Ok(())
    }

    fn handle_failed_transactions_queue() {
        let network_id = T::GetEthNetworkId::get();
        let s_failed_pending_txs =
            StorageValueRef::persistent(STORAGE_FAILED_PENDING_TRANSACTIONS_KEY);
        let mut failed_txs = s_failed_pending_txs
            .get::<BTreeMap<H256, SignedTransactionData<T>>>()
            .flatten()
            .unwrap_or_default();
        let mut to_remove = Vec::new();
        for (key, tx) in &failed_txs {
            let tx_call = bridge_multisig::Call::<T>::decode(&mut &tx.call.encode()[1..]);
            if let Ok(tx_call) = tx_call {
                let maybe_call = match &tx_call {
                    bridge_multisig::Call::as_multi_threshold_1(_, call, _) => {
                        Call::<T>::decode(&mut &call.encode()[1..])
                    }
                    bridge_multisig::Call::as_multi(_, _, ext_bytes, _, _) => {
                        Call::<T>::decode(&mut &ext_bytes[1..])
                    }
                    _ => continue,
                };
                match maybe_call {
                    Ok(Call::<T>::import_incoming_request(load_incoming_request, _)) => {
                        let tx_hash = load_incoming_request.hash();

                        if RequestStatuses::<T>::get(network_id, tx_hash).is_some() {
                            to_remove.push(*key);
                            // Skip already submitted requests.
                            continue;
                        }
                        let _ = Self::send_transaction::<bridge_multisig::Call<T>>(tx_call);
                    }
                    _ => (),
                };
            }
        }
        if !to_remove.is_empty() {
            for key in to_remove {
                failed_txs.remove(&key);
            }
            s_failed_pending_txs.set(&failed_txs);
        }
    }

    fn handle_substrate() -> Result<T::BlockNumber, Error<T>>
    where
        T: CreateSignedTransaction<<T as Config>::Call>,
    {
        let substrate_finalized_block = match Self::load_substrate_finalized_header() {
            Ok(v) => v,
            Err(e) => {
                debug::info!(
                    "Failed to load substrate finalized block ({:?}). Skipping off-chain procedure.",
                    e
                );
                return Err(e);
            }
        };

        if substrate_finalized_block.number.as_u64() % (RE_HANDLE_TXS_PERIOD as u64) == 0 {
            Self::handle_failed_transactions_queue();
        }

        let substrate_finalized_height =
            <T::BlockNumber as From<u32>>::from(substrate_finalized_block.number.as_u32());
        let s_sub_to_handle_from_height =
            StorageValueRef::persistent(b"eth-bridge-ocw::sub-to-handle-from-height");
        let from_block_opt = s_sub_to_handle_from_height.get::<T::BlockNumber>();
        if from_block_opt.is_none() {
            s_sub_to_handle_from_height.set(&substrate_finalized_height);
        }
        let from_block = from_block_opt
            .flatten()
            .unwrap_or(substrate_finalized_height);
        if from_block <= substrate_finalized_height {
            match Self::load_substrate_block(from_block)
                .and_then(|block| Self::handle_substrate_block(block, substrate_finalized_height))
            {
                Ok(_) => {}
                Err(e) => {
                    debug::info!(
                        "Failed to handle substrate block ({:?}). Skipping off-chain procedure.",
                        e
                    );
                    return Ok(substrate_finalized_height);
                }
            };
            if from_block != substrate_finalized_height {
                // Handle only one block per off-chain thread. Since soft-forks occur quite often,
                // it should always "catch up" the finalized height.
                s_sub_to_handle_from_height.set(&(from_block + T::BlockNumber::one()));
            }
        }
        Ok(substrate_finalized_height)
    }

    fn handle_ethereum(network_id: T::NetworkId) -> Result<u64, Error<T>> {
        let string = format!("eth-bridge-ocw::eth-height-{:?}", network_id);
        let s_eth_height = StorageValueRef::persistent(string.as_bytes());
        let current_eth_height = match Self::load_current_height(network_id) {
            Ok(v) => v,
            Err(e) => {
                debug::info!(
                    "Failed to load current ethereum height. Skipping off-chain procedure. {:?}",
                    e
                );
                return Err(e);
            }
        };
        s_eth_height.set(&current_eth_height);

        let string = format!("eth-bridge-ocw::eth-to-handle-from-height-{:?}", network_id);
        let s_eth_to_handle_from_height = StorageValueRef::persistent(string.as_bytes());
        let from_block_opt = s_eth_to_handle_from_height.get::<u64>();
        if from_block_opt.is_none() {
            s_eth_to_handle_from_height.set(&current_eth_height);
        }
        let from_block = from_block_opt.flatten().unwrap_or(current_eth_height);
        // The upper bound of range of blocks to download logs for. Limit the value to
        // `MAX_GET_LOGS_ITEMS` if the OCW is lagging behind Ethereum to avoid downloading too many
        // logs.
        let to_block_opt = current_eth_height
            .checked_sub(CONFIRMATION_INTERVAL)
            .map(|to_block| (from_block + MAX_GET_LOGS_ITEMS).min(to_block));
        if let Some(to_block) = to_block_opt {
            if to_block >= from_block {
                let mut new_height = from_block;
                let err_opt =
                    Self::handle_logs(from_block, to_block, &mut new_height, network_id).err();
                if new_height != from_block {
                    s_eth_to_handle_from_height.set(&new_height);
                }
                if let Some(err) = err_opt {
                    debug::warn!("Failed to load handle logs: {:?}.", err);
                }
            }
        }
        Ok(current_eth_height)
    }

    fn handle_pending_multisig_calls(network_id: T::NetworkId, current_eth_height: u64) {
        for ms in bridge_multisig::Multisigs::<T>::iter_values() {
            let from_block = match ms.when.height {
                MultiChainHeight::Sidechain(sh)
                    if current_eth_height.saturating_sub(sh)
                        >= MAX_PENDING_TX_BLOCKS_PERIOD as u64 =>
                {
                    let string = format!(
                        "eth-bridge-ocw::eth-to-re-handle-from-height-{:?}-{}-{}",
                        network_id, sh, ms.when.index
                    );
                    let s_eth_to_handle_from_height =
                        StorageValueRef::persistent(string.as_bytes());
                    let handled = s_eth_to_handle_from_height
                        .get::<bool>()
                        .flatten()
                        .unwrap_or(false);
                    if handled {
                        continue;
                    }
                    s_eth_to_handle_from_height.set(&true);
                    sh
                }
                _ => {
                    continue;
                }
            };
            debug::debug!("Re-handling ethereum height {}", from_block);
            // +1 block should be ok, because MAX_PENDING_TX_BLOCKS_PERIOD > CONFIRMATION_INTERVAL.
            let err_opt = Self::handle_logs(from_block, from_block + 1, &mut 0, network_id).err();
            if let Some(err) = err_opt {
                debug::warn!("Failed to re-handle logs: {:?}.", err);
            }
        }
    }

    /// Retrieves latest needed information about networks and handles corresponding
    /// requests queues.
    ///
    /// At first, it loads current Sidechain height and current finalized Thischain height.
    /// Then it handles each request in the requests queue if it was submitted at least at
    /// the finalized height. The same is done with incoming requests queue. All handled requests
    /// are added to local storage to not be handled twice by the off-chain worker.
    fn handle_network(network_id: T::NetworkId, substrate_finalized_height: T::BlockNumber)
    where
        T: CreateSignedTransaction<<T as Config>::Call>,
    {
        let current_eth_height = match Self::handle_ethereum(network_id) {
            Ok(v) => v,
            Err(_e) => {
                return;
            }
        };

        if substrate_finalized_height % RE_HANDLE_TXS_PERIOD.into() == T::BlockNumber::zero() {
            Self::handle_pending_multisig_calls(network_id, current_eth_height);
        }

        for request_hash in RequestsQueue::<T>::get(network_id) {
            let request = match Requests::<T>::get(network_id, request_hash) {
                Some(v) => v,
                _ => continue, // TODO: remove from queue
            };
            let request_submission_height: T::BlockNumber =
                Self::request_submission_height(network_id, &request_hash);
            let number = T::BlockNumber::from(MAX_PENDING_TX_BLOCKS_PERIOD);
            let diff = substrate_finalized_height.saturating_sub(request_submission_height);
            let should_reapprove = diff >= number && diff % number == T::BlockNumber::zero();
            if !should_reapprove && substrate_finalized_height < request_submission_height {
                continue;
            }
            let handled_key = format!("eth-bridge-ocw::handled-request-{:?}", request_hash);
            let s_handled_request = StorageValueRef::persistent(handled_key.as_bytes());
            let height_opt = s_handled_request.get::<T::BlockNumber>().flatten();

            let need_to_handle = match height_opt {
                Some(height) => should_reapprove || request_submission_height > height,
                None => true,
            };
            let confirmed = match &request {
                OffchainRequest::Incoming(request, _) => {
                    current_eth_height.saturating_sub(request.at_height()) >= CONFIRMATION_INTERVAL
                }
                _ => true,
            };
            if need_to_handle && confirmed {
                let timepoint = request.timepoint();
                let error = Self::handle_offchain_request(request).err();
                let mut is_handled = true;
                if let Some(e) = error {
                    debug::error!(
                        "An error occurred while processing off-chain request: {:?}",
                        e
                    );
                    if e.should_retry() {
                        is_handled = false;
                    } else if e.should_abort() {
                        if let Err(abort_err) =
                            Self::send_abort_request(request_hash, e, timepoint, network_id)
                        {
                            debug::error!(
                                "An error occurred while trying to send abort request: {:?}",
                                abort_err
                            );
                        }
                    }
                }
                if is_handled {
                    s_handled_request.set(&request_submission_height);
                }
            }
        }
    }

    /// Handles registered networks.
    fn offchain()
    where
        T: CreateSignedTransaction<<T as Config>::Call>,
    {
        let s_networks_ids = StorageValueRef::persistent(STORAGE_NETWORK_IDS_KEY);

        let substrate_finalized_height = match Self::handle_substrate() {
            Ok(v) => v,
            Err(_e) => {
                return;
            }
        };

        let network_ids = s_networks_ids
            .get::<BTreeSet<T::NetworkId>>()
            .flatten()
            .unwrap_or_default();
        for network_id in network_ids {
            Self::handle_network(network_id, substrate_finalized_height);
        }
    }

    /// Makes off-chain HTTP request.
    fn http_request(
        url: &str,
        body: Vec<u8>,
        headers: &[(&'static str, String)],
    ) -> Result<Vec<u8>, Error<T>> {
        debug::trace!("Sending request to: {}", url);
        let mut request = rt_offchain::http::Request::post(url, vec![body.clone()]);
        let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(
            HTTP_REQUEST_TIMEOUT_SECS * 1000,
        ));
        for (key, value) in headers {
            request = request.add_header(*key, &*value);
        }
        #[allow(unused_mut)]
        let mut pending = request.deadline(timeout).send().map_err(|e| {
            debug::error!("Failed to send a request {:?}", e);
            <Error<T>>::HttpFetchingError
        })?;
        #[cfg(test)]
        T::Mock::on_request(&mut pending, url, String::from_utf8_lossy(&body));
        let response = pending
            .try_wait(timeout)
            .map_err(|e| {
                debug::error!("Failed to get a response: {:?}", e);
                <Error<T>>::HttpFetchingError
            })?
            .map_err(|e| {
                debug::error!("Failed to get a response: {:?}", e);
                <Error<T>>::HttpFetchingError
            })?;
        if response.code != 200 {
            debug::error!("Unexpected http request status code: {}", response.code);
            return Err(<Error<T>>::HttpFetchingError);
        }
        let resp = response.body().collect::<Vec<u8>>();
        Ok(resp)
    }

    /// Makes JSON-RPC request.
    fn json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        url: &str,
        id: u64,
        method: &str,
        params: &I,
        headers: &[(&'static str, String)],
    ) -> Result<O, Error<T>> {
        let params = match serialize(params) {
            Value::Null => Params::None,
            Value::Array(v) => Params::Array(v),
            Value::Object(v) => Params::Map(v),
            _ => {
                debug::error!("json_rpc_request: got invalid params");
                fail!(Error::<T>::JsonSerializationError);
            }
        };

        let raw_response = Self::http_request(
            url,
            serde_json::to_vec(&rpc::Request::Single(rpc::Call::MethodCall(
                rpc::MethodCall {
                    jsonrpc: Some(rpc::Version::V2),
                    method: method.into(),
                    params,
                    id: rpc::Id::Num(id as u64),
                },
            )))
            .map_err(|_| Error::<T>::JsonSerializationError)?,
            &headers,
        )
        .and_then(|x| {
            String::from_utf8(x).map_err(|e| {
                debug::error!("json_rpc_request: from utf8 failed, {}", e);
                Error::<T>::HttpFetchingError
            })
        })?;
        let response = rpc::Response::from_json(&raw_response)
            .map_err(|e| {
                debug::error!("json_rpc_request: from_json failed, {}", e);
            })
            .map_err(|_| Error::<T>::FailedToLoadTransaction)?;
        let result = match response {
            rpc::Response::Batch(_xs) => unreachable!("we've just sent a `Single` request; qed"),
            rpc::Response::Single(x) => x,
        };
        match result {
            rpc::Output::Success(s) => {
                if s.result.is_null() {
                    Err(Error::<T>::FailedToLoadTransaction)
                } else {
                    serde_json::from_value(s.result).map_err(|e| {
                        debug::error!("json_rpc_request: from_value failed, {}", e);
                        Error::<T>::JsonDeserializationError.into()
                    })
                }
            }
            _ => {
                debug::error!("json_rpc_request: request failed");
                Err(Error::<T>::JsonDeserializationError.into())
            }
        }
    }

    /// Makes request to a Sidechain node. The node URL and credentials are stored in the local
    /// storage.
    fn eth_json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        method: &str,
        params: &I,
        network_id: T::NetworkId,
    ) -> Result<O, Error<T>> {
        let string = format!("{}-{:?}", STORAGE_ETH_NODE_PARAMS, network_id);
        let s_node_params = StorageValueRef::persistent(string.as_bytes());
        let node_params = match s_node_params.get::<NodeParams>().flatten() {
            Some(v) => v,
            None => {
                debug::warn!("Failed to make JSON-RPC request, make sure to set node parameters.");
                fail!(Error::<T>::FailedToLoadSidechainNodeParams);
            }
        };
        let mut headers: Vec<(_, String)> = vec![("content-type", "application/json".into())];
        if let Some(node_credentials) = node_params.credentials {
            headers.push(("Authorization", node_credentials));
        }
        Self::json_rpc_request(&node_params.url, 0, method, params, &headers)
    }

    /// Makes request to the local node. The node URL is stored in the local storage.
    fn substrate_json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        method: &str,
        params: &I,
    ) -> Result<O, Error<T>> {
        let s_node_url = StorageValueRef::persistent(STORAGE_SUB_NODE_URL_KEY);
        let node_url = s_node_url
            .get::<String>()
            .flatten()
            .unwrap_or_else(|| SUB_NODE_URL.into());
        let headers: Vec<(_, String)> = vec![("content-type", "application/json".into())];

        Self::json_rpc_request(&node_url, 0, method, params, &headers)
    }

    fn get_signer() -> Result<Signer<T, T::PeerId>, Error<T>> {
        let signer = Signer::<T, T::PeerId>::any_account();
        if !signer.can_sign() {
            debug::error!("[Ethereum bridge] No local account available");
            fail!(<Error<T>>::NoLocalAccountForSigning);
        }
        Ok(signer)
    }

    fn add_pending_extrinsic<LocalCall>(call: LocalCall, account: &Account<T>, added_to_pool: bool)
    where
        T: CreateSignedTransaction<LocalCall>,
        LocalCall: Clone + GetCallName + Encode + Into<<T as Config>::Call>,
    {
        let s_signed_txs = StorageValueRef::persistent(STORAGE_PENDING_TRANSACTIONS_KEY);
        let mut transactions = s_signed_txs
            .get::<BTreeMap<H256, SignedTransactionData<T>>>()
            .flatten()
            .unwrap_or_default();
        let submitted_at = if !added_to_pool {
            None
        } else {
            Some(frame_system::Pallet::<T>::current_block_number())
        };
        let signed_transaction_data =
            SignedTransactionData::from_local_call(call, account, submitted_at)
                .expect("we've just successfully signed the same data; qed");
        transactions.insert(
            signed_transaction_data.extrinsic_hash,
            signed_transaction_data,
        );
        s_signed_txs.set(&transactions);
    }

    /// Sends a substrate transaction signed by an off-chain worker. After a successful signing
    /// information about the extrinsic is added to pending transactions storage, because according
    /// to [`sp_runtime::ApplyExtrinsicResult`](https://substrate.dev/rustdocs/v3.0.0/sp_runtime/type.ApplyExtrinsicResult.html)
    /// an extrinsic may not be imported to the block and thus should be re-sent.
    fn send_transaction<LocalCall>(call: LocalCall) -> Result<(), Error<T>>
    where
        T: CreateSignedTransaction<LocalCall>,
        LocalCall: Clone + GetCallName + Encode + Into<<T as Config>::Call>,
    {
        let signer = Self::get_signer()?;
        debug::debug!("Sending signed transaction: {}", call.get_call_name());
        let result = Self::send_signed_transaction(&signer, &call);

        match result {
            Some((account, res)) => {
                Self::add_pending_extrinsic(call, &account, res.is_ok());
                if let Err(e) = res {
                    debug::error!(
                        "[{:?}] Failed to send signed transaction: {:?}",
                        account.id,
                        e
                    );
                    fail!(<Error<T>>::FailedToSendSignedTransaction);
                }
            }
            _ => {
                debug::error!("Failed to send signed transaction");
                fail!(<Error<T>>::NoLocalAccountForSigning);
            }
        };
        Ok(())
    }

    fn send_multisig_transaction(
        call: Call<T>,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        let bridge_account = get_bridge_account::<T>(network_id);
        let threshold = bridge_multisig::Accounts::<T>::get(&bridge_account)
            .unwrap()
            .threshold_num();
        let call = if threshold == 1 {
            bridge_multisig::Call::as_multi_threshold_1(
                bridge_account,
                Box::new(<<T as Config>::Call>::from(call)),
                timepoint,
            )
        } else {
            let vec = <<T as Config>::Call>::from(call).encode();
            bridge_multisig::Call::as_multi(
                bridge_account,
                Some(timepoint),
                vec,
                true,
                OFFCHAIN_TRANSACTION_WEIGHT_LIMIT,
            )
        };
        Self::send_transaction::<bridge_multisig::Call<T>>(call)
    }

    /// Queries Sidechain's contract variable `used`.
    fn load_is_used(hash: H256, network_id: T::NetworkId) -> Result<bool, Error<T>> {
        // `used(bytes32)`
        let mut data: Vec<_> = hex!("b07c411f").to_vec();
        data.extend(&hash.0);
        let contract_address = types::H160(BridgeContractAddress::<T>::get(network_id).0);
        let contracts = if network_id == T::GetEthNetworkId::get() {
            vec![
                contract_address,
                types::H160(Self::xor_master_contract_address().0),
                types::H160(Self::val_master_contract_address().0),
            ]
        } else {
            vec![contract_address]
        };
        for contract in contracts {
            let is_used = Self::eth_json_rpc_request::<_, bool>(
                "eth_call",
                &vec![
                    serialize(&CallRequest {
                        to: Some(contract),
                        data: Some(Bytes(data.clone())),
                        ..Default::default()
                    }),
                    Value::String("latest".into()),
                ],
                network_id,
            )?;
            if is_used {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Registers new sidechain asset and grants mint permission to the bridge account.
    fn register_sidechain_asset(
        token_address: Address,
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
        let asset_id = assets::Module::<T>::register_from(
            &bridge_account,
            symbol,
            name,
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
        )?;
        RegisteredAsset::<T>::insert(network_id, &asset_id, AssetKind::Sidechain);
        RegisteredSidechainAsset::<T>::insert(network_id, &token_address, asset_id);
        RegisteredSidechainToken::<T>::insert(network_id, &asset_id, token_address);
        SidechainAssetPrecision::<T>::insert(network_id, &asset_id, precision);
        let scope = Scope::Unlimited;
        let permission_ids = [MINT, BURN];
        for permission_id in &permission_ids {
            let permission_owner = permissions::Owners::<T>::get(permission_id, &scope)
                .pop()
                .unwrap_or_else(|| bridge_account.clone());
            permissions::Module::<T>::grant_permission_with_scope(
                permission_owner,
                bridge_account.clone(),
                *permission_id,
                scope,
            )?;
        }

        Ok(asset_id)
    }

    /// Gets Thischain asset id and its kind. If the `raw_asset_id` is `zero`, it means that it's
    /// a Sidechain(Owned) asset, otherwise, Thischain.
    fn get_asset_by_raw_asset_id(
        raw_asset_id: H256,
        token_address: &Address,
        network_id: T::NetworkId,
    ) -> Result<Option<(T::AssetId, AssetKind)>, Error<T>> {
        let is_sidechain_token = raw_asset_id == H256::zero();
        if is_sidechain_token {
            let asset_id = match Self::registered_sidechain_asset(network_id, &token_address) {
                Some(asset_id) => asset_id,
                _ => {
                    return Ok(None);
                }
            };
            Ok(Some((
                asset_id,
                Self::registered_asset(network_id, &asset_id).unwrap_or(AssetKind::Sidechain),
            )))
        } else {
            let asset_id = T::AssetId::from(H256(raw_asset_id.0));
            let asset_kind = Self::registered_asset(network_id, &asset_id);
            if asset_kind.is_none() || asset_kind.unwrap() == AssetKind::Sidechain {
                fail!(Error::<T>::UnknownAssetId);
            }
            Ok(Some((asset_id, AssetKind::Thischain)))
        }
    }

    /// Tries to parse a method call on one of old Sora contracts (XOR and VAL).
    ///
    /// Since the XOR and VAL contracts don't have the same interface and events that the modern
    /// bridge contract have, and since they can't be changed we have to provide a special parsing
    /// flow for some of the methods that we might use.
    fn parse_old_incoming_request_method_call(
        incoming_request: LoadIncomingTransactionRequest<T>,
        tx: Transaction,
    ) -> Result<IncomingRequest<T>, Error<T>> {
        let (fun, arg_pos, tail, added) = if let Some(tail) = tx
            .input
            .0
            .strip_prefix(&*ADD_PEER_BY_PEER_ID.get_or_init(init_add_peer_by_peer_fn))
        {
            (
                &ADD_PEER_BY_PEER_FN,
                ADD_PEER_BY_PEER_TX_HASH_ARG_POS,
                tail,
                true,
            )
        } else if let Some(tail) = tx
            .input
            .0
            .strip_prefix(&*REMOVE_PEER_BY_PEER_ID.get_or_init(init_remove_peer_by_peer_fn))
        {
            (
                &REMOVE_PEER_BY_PEER_FN,
                REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS,
                tail,
                false,
            )
        } else {
            fail!(Error::<T>::UnknownMethodId);
        };

        let tokens = (*fun.get().unwrap())
            .decode_input(tail)
            .map_err(|_| Error::<T>::EthAbiDecodingError)?;
        let request_hash = parse_hash_from_call::<T>(tokens, arg_pos)?;
        let author = incoming_request.author;
        let oc_request: OffchainRequest<T> =
            Requests::<T>::get(T::GetEthNetworkId::get(), request_hash)
                .ok_or(Error::<T>::UnknownRequest)?;
        match oc_request {
            OffchainRequest::Outgoing(
                OutgoingRequest::AddPeerCompat(OutgoingAddPeerCompat {
                    peer_address,
                    peer_account_id,
                    ..
                }),
                _,
            )
            | OffchainRequest::Outgoing(
                OutgoingRequest::RemovePeerCompat(OutgoingRemovePeerCompat {
                    peer_address,
                    peer_account_id,
                    ..
                }),
                _,
            ) => {
                let contract = match tx.to {
                    Some(x) if x.0 == Self::xor_master_contract_address().0 => {
                        ChangePeersContract::XOR
                    }
                    Some(x) if x.0 == Self::val_master_contract_address().0 => {
                        ChangePeersContract::VAL
                    }
                    _ => fail!(Error::<T>::UnknownContractAddress),
                };
                let at_height = tx
                    .block_number
                    .expect("'block_number' is null only when the log/transaction is pending; qed")
                    .as_u64();
                let request = IncomingRequest::ChangePeersCompat(IncomingChangePeersCompat {
                    peer_account_id,
                    peer_address,
                    added,
                    contract,
                    author,
                    tx_hash: incoming_request.hash,
                    at_height,
                    timepoint: incoming_request.timepoint,
                    network_id: incoming_request.network_id,
                });
                Ok(request)
            }
            _ => fail!(Error::<T>::InvalidFunctionInput),
        }
    }

    /// Tries to parse incoming request from the given pre-request and transaction receipt.
    ///
    /// The transaction should be approved and contain a known event from which the request
    /// is built.
    fn parse_incoming_request(
        tx_receipt: TransactionReceipt,
        incoming_pre_request: LoadIncomingTransactionRequest<T>,
    ) -> Result<IncomingRequest<T>, Error<T>> {
        let tx_approved = tx_receipt.is_approved();
        ensure!(tx_approved, Error::<T>::EthTransactionIsFailed);
        let kind = incoming_pre_request.kind;
        let network_id = incoming_pre_request.network_id;

        // For XOR and VAL contracts compatibility.
        if kind.is_compat() {
            let tx = Self::load_tx(H256(tx_receipt.transaction_hash.0), network_id)?;
            return Self::parse_old_incoming_request_method_call(incoming_pre_request, tx);
        }

        let at_height = tx_receipt
            .block_number
            .expect("'block_number' is null only when the log/transaction is pending; qed")
            .as_u64();

        let call = Self::parse_main_event(&tx_receipt.logs, kind)?;
        // TODO (optimization): pre-validate the parsed calls.
        IncomingRequest::<T>::try_from_contract_event(call, incoming_pre_request, at_height)
    }

    /// Send a transaction to finalize the incoming request.
    fn send_finalize_incoming_request(
        hash: H256,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        debug::debug!("send_incoming_request_result: {:?}", hash);
        let transfer_call = Call::<T>::finalize_incoming_request(hash, network_id);
        Self::send_multisig_transaction(transfer_call, timepoint, network_id)
    }

    fn send_import_incoming_request(
        load_incoming_request: LoadIncomingRequest<T>,
        incoming_request_result: Result<IncomingRequest<T>, DispatchError>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        let timepoint = load_incoming_request.timepoint();
        debug::debug!(
            "send_import_incoming_request: {:?}",
            incoming_request_result
        );
        let import_call =
            Call::<T>::import_incoming_request(load_incoming_request, incoming_request_result);
        Self::send_multisig_transaction(import_call, timepoint, network_id)
    }

    /// Send 'abort request' transaction.
    fn send_abort_request(
        request_hash: H256,
        request_error: Error<T>,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        debug::debug!("send_abort_request: {:?}", request_hash);
        ensure!(
            RequestStatuses::<T>::get(network_id, request_hash) == Some(RequestStatus::Pending),
            Error::<T>::ExpectedPendingRequest
        );
        let abort_request_call =
            Call::<T>::abort_request(request_hash, request_error.into(), network_id);
        Self::send_multisig_transaction(abort_request_call, timepoint, network_id)
    }

    /// Encodes the given outgoing request to Ethereum ABI, then signs the data by off-chain worker's
    /// key and sends the approve as a signed transaction.
    fn handle_outgoing_request(request: OutgoingRequest<T>, hash: H256) -> Result<(), Error<T>> {
        let signer = Signer::<T, T::PeerId>::any_account();
        if !signer.can_sign() {
            debug::error!("No local account available");
            return Err(<Error<T>>::NoLocalAccountForSigning);
        }
        let encoded_request = request.to_eth_abi(hash)?;
        let secret_s = StorageValueRef::persistent(STORAGE_PEER_SECRET_KEY);
        let sk = secp256k1::SecretKey::parse_slice(
            &secret_s
                .get::<Vec<u8>>()
                .flatten()
                .expect("Off-chain worker secret key is not specified."),
        )
        .expect("Invalid off-chain worker secret key.");
        // Signs `abi.encodePacked(tokenAddress, amount, to, txHash, from)`.
        let (signature, public) = Self::sign_message(encoded_request.as_raw(), &sk);
        let call = Call::approve_request(
            ecdsa::Public::from_slice(&public.serialize_compressed()),
            hash,
            signature,
            request.network_id(),
        );
        let result = Self::send_signed_transaction(&signer, &call);

        match result {
            Some((account, res)) => {
                Self::add_pending_extrinsic(call, &account, res.is_ok());
                match res {
                    Ok(_) => debug::trace!("Signed transaction sent"),
                    Err(e) => {
                        debug::error!(
                            "[{:?}] Failed in handle_outgoing_transfer: {:?}",
                            account.id,
                            e
                        );
                        return Err(<Error<T>>::FailedToSendSignedTransaction);
                    }
                }
            }
            _ => {
                debug::error!("Failed in handle_outgoing_transfer");
                return Err(<Error<T>>::NoLocalAccountForSigning);
            }
        };
        Ok(())
    }

    /// Queries current height of Sidechain.
    fn load_current_height(network_id: T::NetworkId) -> Result<u64, Error<T>> {
        Self::eth_json_rpc_request::<_, types::U64>("eth_blockNumber", &(), network_id)
            .map(|x| x.as_u64())
    }

    /// Checks that the given contract address is known to the bridge network.
    ///
    /// There are special cases for XOR and VAL contracts.
    fn ensure_known_contract(to: Address, network_id: T::NetworkId) -> Result<(), Error<T>> {
        if network_id == T::GetEthNetworkId::get() {
            ensure!(
                to == BridgeContractAddress::<T>::get(network_id)
                    || to == Self::xor_master_contract_address()
                    || to == Self::val_master_contract_address(),
                Error::<T>::UnknownContractAddress
            );
        } else {
            ensure!(
                to == BridgeContractAddress::<T>::get(network_id),
                Error::<T>::UnknownContractAddress
            );
        }
        Ok(())
    }

    /// Loads a Sidechain transaction by the hash and ensures that it came from a known contract.
    fn load_tx(hash: H256, network_id: T::NetworkId) -> Result<Transaction, Error<T>> {
        let hash = types::H256(hash.0);
        let tx_receipt = Self::eth_json_rpc_request::<_, Transaction>(
            "eth_getTransactionByHash",
            &vec![hash],
            network_id,
        )?;
        let to = tx_receipt
            .to
            .map(|x| H160(x.0))
            .ok_or(Error::<T>::UnknownContractAddress)?;
        Self::ensure_known_contract(to, network_id)?;
        Ok(tx_receipt)
    }

    /// Loads a Sidechain transaction receipt by the hash and ensures that it came from a known contract.
    // TODO: check if transaction failed due to gas limit
    fn load_tx_receipt(
        hash: H256,
        network_id: T::NetworkId,
    ) -> Result<TransactionReceipt, Error<T>> {
        let hash = types::H256(hash.0);
        let tx_receipt = Self::eth_json_rpc_request::<_, TransactionReceipt>(
            "eth_getTransactionReceipt",
            &vec![hash],
            network_id,
        )?;
        let to = tx_receipt
            .to
            .map(|x| H160(x.0))
            .ok_or(Error::<T>::UnknownContractAddress)?;
        Self::ensure_known_contract(to, network_id)?;
        Ok(tx_receipt)
    }

    /// Checks if the account is a bridge peer.
    pub fn is_peer(who: &T::AccountId, network_id: T::NetworkId) -> bool {
        Self::peers(network_id).into_iter().any(|i| i == *who)
    }

    /// Ensures that the account is a bridge peer.
    fn ensure_peer(who: &T::AccountId, network_id: T::NetworkId) -> DispatchResult {
        ensure!(Self::is_peer(who, network_id), Error::<T>::Forbidden);
        Ok(())
    }

    /// Ensures that the account is a bridge multisig account.
    fn ensure_bridge_account(
        origin: OriginFor<T>,
        network_id: T::NetworkId,
    ) -> Result<T::AccountId, DispatchErrorWithPostInfo<PostDispatchInfo>> {
        let who = ensure_signed(origin)?;
        let bridge_account_id =
            Self::bridge_account(network_id).ok_or(Error::<T>::UnknownNetwork)?;
        ensure!(who == bridge_account_id, Error::<T>::Forbidden);
        Ok(bridge_account_id)
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

    /// Converts amount from one precision to another and and returns it with a difference of the
    /// amounts. It also checks that no information was lost during multiplication, otherwise
    /// returns an error.
    pub fn convert_precision(
        precision_from: BalancePrecision,
        precision_to: BalancePrecision,
        amount: Balance,
    ) -> Result<(Balance, Balance), Error<T>> {
        if precision_from == precision_to {
            return Ok((amount, 0));
        }
        let pair = if precision_from < precision_to {
            let exp = (precision_to - precision_from) as u32;
            let coeff = 10_u128.pow(exp);
            let coerced_amount = amount.saturating_mul(coeff);
            ensure!(
                coerced_amount / coeff == amount,
                Error::<T>::UnsupportedAssetPrecision
            );
            (coerced_amount, 0)
        } else {
            let exp = (precision_from - precision_to) as u32;
            let coeff = 10_u128.pow(exp);
            let coerced_amount = amount / coeff;
            let diff = amount - coerced_amount * coeff;
            (coerced_amount, diff)
        };
        Ok(pair)
    }
}

impl<T: Config> Pallet<T> {
    const ITEMS_LIMIT: usize = 50;

    /// Get requests data and their statuses by hash.
    pub fn get_requests(
        hashes: &[H256],
        network_id: Option<T::NetworkId>,
        redirect_finished_load_requests: bool,
    ) -> Result<Vec<(OffchainRequest<T>, RequestStatus)>, DispatchError> {
        Ok(hashes
            .iter()
            .take(Self::ITEMS_LIMIT)
            .flat_map(|hash| {
                if let Some(net_id) = network_id {
                    Requests::<T>::get(net_id, hash)
                        .zip({
                            let status: Option<RequestStatus> = Self::request_status(net_id, hash);
                            status
                        })
                        .and_then(|(req, status)| {
                            let redirect_to_incoming = redirect_finished_load_requests
                                && req.is_load_incoming()
                                && status == RequestStatus::Done;
                            if redirect_to_incoming {
                                let redirect_hash =
                                    LoadToIncomingRequestHash::<T>::get(net_id, hash);
                                Requests::<T>::get(net_id, redirect_hash).map(|req| {
                                    let status: RequestStatus =
                                        Self::request_status(net_id, redirect_hash)
                                            .unwrap_or(RequestStatus::Pending);
                                    (req, status)
                                })
                            } else {
                                Some((req, status))
                            }
                        })
                        .map(|x| vec![x])
                        .unwrap_or_default()
                } else {
                    Requests::<T>::iter()
                        .filter(|(_, h, _)| h == hash)
                        .map(|(net_id, hash, request)| {
                            let status: RequestStatus = Self::request_status(net_id, hash)
                                .unwrap_or(RequestStatus::Pending);
                            (net_id, request, status)
                        })
                        .filter_map(|(net_id, req, status)| {
                            let redirect_to_incoming = redirect_finished_load_requests
                                && req.is_load_incoming()
                                && status == RequestStatus::Done;
                            if redirect_to_incoming {
                                let redirect_hash =
                                    LoadToIncomingRequestHash::<T>::get(net_id, hash);
                                Requests::<T>::get(net_id, redirect_hash).map(|req| {
                                    let status: RequestStatus =
                                        Self::request_status(net_id, redirect_hash)
                                            .unwrap_or(RequestStatus::Pending);
                                    (req, status)
                                })
                            } else {
                                Some((req, status))
                            }
                        })
                        .collect()
                }
            })
            .collect())
    }

    /// Get approved outgoing requests data and proofs.
    pub fn get_approved_requests(
        hashes: &[H256],
        network_id: Option<T::NetworkId>,
    ) -> Result<Vec<(OutgoingRequestEncoded, Vec<SignatureParams>)>, DispatchError> {
        let items = hashes
            .iter()
            .take(Self::ITEMS_LIMIT)
            .filter_map(|hash| {
                if let Some(net_id) = network_id {
                    if Self::request_status(net_id, hash)? == RequestStatus::ApprovalsReady {
                        let request: OffchainRequest<T> = Requests::get(net_id, hash)?;
                        match request {
                            OffchainRequest::Outgoing(request, hash) => {
                                let encoded = request
                                    .to_eth_abi(hash)
                                    .expect("this conversion was already tested; qed");
                                Self::get_approvals(&[hash], Some(net_id))
                                    .ok()?
                                    .pop()
                                    .map(|approvals| vec![(encoded, approvals)])
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    Some(
                        RequestStatuses::<T>::iter()
                            .filter(|(_, _hash, v)| v == &RequestStatus::ApprovalsReady)
                            .filter_map(|(net_id, hash, _v)| {
                                let request: OffchainRequest<T> = Requests::get(net_id, hash)?;
                                match request {
                                    OffchainRequest::Outgoing(request, hash) => {
                                        let encoded = request
                                            .to_eth_abi(hash)
                                            .expect("this conversion was already tested; qed");
                                        Self::get_approvals(&[hash], Some(net_id))
                                            .ok()?
                                            .pop()
                                            .map(|approvals| (encoded, approvals))
                                    }
                                    _ => None,
                                }
                            })
                            .collect(),
                    )
                }
            })
            .flatten()
            .collect();
        Ok(items)
    }

    /// Get requests approvals.
    pub fn get_approvals(
        hashes: &[H256],
        network_id: Option<T::NetworkId>,
    ) -> Result<Vec<Vec<SignatureParams>>, DispatchError> {
        Ok(hashes
            .iter()
            .take(Self::ITEMS_LIMIT)
            .flat_map(|hash| {
                if let Some(net_id) = network_id {
                    vec![RequestApprovals::<T>::get(net_id, hash)
                        .into_iter()
                        .collect()]
                } else {
                    RequestApprovals::<T>::iter()
                        .filter(|(_, h, _)| h == hash)
                        .map(|(_, _, v)| v.into_iter().collect::<Vec<_>>())
                        .collect()
                }
            })
            .collect())
    }

    /// Get account requests list.
    pub fn get_account_requests(
        account: &T::AccountId,
        status_filter: Option<RequestStatus>,
    ) -> Result<Vec<(T::NetworkId, H256)>, DispatchError> {
        let mut requests: Vec<(T::NetworkId, H256)> = Self::account_requests(account);
        if let Some(filter) = status_filter {
            requests.retain(|(net_id, h)| Self::request_status(net_id, h).unwrap() == filter)
        }
        Ok(requests)
    }

    /// Get registered assets and tokens.
    pub fn get_registered_assets(
        network_id: Option<T::NetworkId>,
    ) -> Result<
        Vec<(
            AssetKind,
            (AssetIdOf<T>, BalancePrecision),
            Option<(H160, BalancePrecision)>,
        )>,
        DispatchError,
    > {
        Ok(iter_storage::<RegisteredAsset<T>, _, _, _, _, _>(
            network_id,
            |(network_id, asset_id, kind)| {
                let token_info = RegisteredSidechainToken::<T>::get(network_id, &asset_id)
                    .map(|x| H160(x.0))
                    .map(|address| {
                        let precision = SidechainAssetPrecision::<T>::get(network_id, &asset_id);
                        (address, precision)
                    });
                let asset_precision = assets::Pallet::<T>::get_asset_info(&asset_id).2;
                (kind, (asset_id, asset_precision), token_info)
            },
        ))
    }
}

pub fn get_bridge_account<T: Config>(network_id: T::NetworkId) -> T::AccountId {
    crate::BridgeAccount::<T>::get(network_id).expect("networks can't be removed; qed")
}

pub fn serialize<T: serde::Serialize>(t: &T) -> rpc::Value {
    serde_json::to_value(t).expect("Types never fail to serialize.")
}

pub fn to_string<T: serde::Serialize>(request: &T) -> String {
    serde_json::to_string(&request).expect("String serialization never fails.")
}

fn iter_storage<S, K1, K2, V, F, O>(k1: Option<K1>, f: F) -> Vec<O>
where
    K1: FullCodec + Copy,
    K2: FullCodec,
    V: FullCodec,
    S: IterableStorageDoubleMap<K1, K2, V>,
    F: FnMut((K1, K2, V)) -> O,
{
    if let Some(k1) = k1 {
        S::iter_prefix(k1)
            .map(|(k2, v)| (k1, k2, v))
            .map(f)
            .collect()
    } else {
        S::iter().map(f).collect()
    }
}
