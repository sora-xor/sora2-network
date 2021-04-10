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
use crate::types::{Bytes, CallRequest, Log, Transaction, TransactionReceipt};
use alloc::string::String;
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
    AtLeast32Bit, IdentifyAccount, MaybeSerializeDeserialize, Member, One,
};
use frame_support::sp_runtime::{offchain as rt_offchain, KeyTypeId, MultiSigner, Percent};
use frame_support::traits::{Get, GetCallName};
use frame_support::weights::{Pays, Weight};
use frame_support::{
    debug, ensure, fail, sp_io, transactional, IterableStorageDoubleMap, Parameter, RuntimeDebug,
};
use frame_system::offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer};
use frame_system::{ensure_root, ensure_signed};
use hex_literal::hex;
use permissions::{Scope, BURN, MINT};
use requests::*;
use rpc::Params;
use rustc_hex::ToHex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sp_core::{H160, H256};
use sp_io::hashing::blake2_256;
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
    fn request_from_sidechain(kind: &IncomingRequestKind) -> (Weight, Pays);
    fn add_peer() -> Weight;
    fn remove_peer() -> Weight;
    fn force_add_peer() -> Weight;
    fn prepare_for_migration() -> Weight;
    fn migrate() -> Weight;
    fn register_incoming_request() -> (Weight, Pays);
    fn finalize_incoming_request() -> (Weight, Pays);
    fn approve_request() -> (Weight, Pays);
    fn approve_request_finalize() -> (Weight, Pays);
    fn abort_request() -> (Weight, Pays);
}

type Address = H160;
type EthereumAddress = Address;

pub mod weights;

mod benchmarking;
mod contract;
#[cfg(test)]
mod mock;
pub mod requests;
#[cfg(test)]
mod tests;
pub mod types;

/// Substrate node RPC URL.
const SUB_NODE_URL: &str = "http://127.0.0.1:9954";
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 10;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"bridge";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
pub const TECH_ACCOUNT_AUTHORITY: &[u8] = b"authority";

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ethb");
/// A number of sidechain blocks needed to consider transaction as confirmed.
pub const CONFIRMATION_INTERVAL: u64 = 30;
// Off-chain worker storage paths.
pub const STORAGE_SUB_NODE_URL_KEY: &[u8] = b"eth-bridge-ocw::sub-node-url";
pub const STORAGE_PEER_SECRET_KEY: &[u8] = b"eth-bridge-ocw::secret-key";
pub const STORAGE_ETH_NODE_PARAMS: &str = "eth-bridge-ocw::node-params";
pub const STORAGE_NETWORK_IDS_KEY: &[u8] = b"eth-bridge-ocw::network-ids";

type AssetIdOf<T> = <T as assets::Config>::AssetId;
type Timepoint<T> = bridge_multisig::Timepoint<<T as frame_system::Config>::BlockNumber>;
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

    fn prepare(&mut self) -> Result<(), DispatchError> {
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
        tx_hash: H256,
    ) -> Result<Self, Error<T>> {
        let network_id = incoming_request.network_id;
        let timepoint = incoming_request.timepoint;
        let author = incoming_request.author;

        let req = match event {
            ContractEvent::Deposit(to, amount, token_address, raw_asset_id) => {
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

    fn network_id(&self) -> T::NetworkId {
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

    fn network_id(&self) -> T::NetworkId {
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

    fn prepare(&mut self) -> Result<(), DispatchError> {
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
    timepoint: Timepoint<T>,
    kind: IncomingTransactionRequestKind,
    network_id: T::NetworkId,
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
    timepoint: Timepoint<T>,
    kind: IncomingMetaRequestKind,
    network_id: T::NetworkId,
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
    fn prepare(&mut self) -> Result<(), DispatchError> {
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

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::AssetConfig;
    use frame_support::pallet_prelude::*;
    use frame_support::weights::PostDispatchInfo;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;
    use permissions::BURN;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + CreateSignedTransaction<Call<Self>>
        + CreateSignedTransaction<bridge_multisig::Call<Self>>
        + assets::Config
        + bridge_multisig::Config
        + fmt::Debug
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// The identifier type for an offchain worker.
        type PeerId: AppCrypto<Self::Public, Self::Signature>;
        /// The overarching dispatch call type.
        type Call: From<Call<Self>> + Encode;
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
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
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
                Percent::from_parts(67),
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
            let timepoint = bridge_multisig::Module::<T>::timepoint();
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::AddAsset(
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
            let timepoint = bridge_multisig::Module::<T>::timepoint();
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::AddToken(
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
            let timepoint = bridge_multisig::Module::<T>::timepoint();
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::Transfer(
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
        #[pallet::weight(<T as Config>::WeightInfo::request_from_sidechain(kind))]
        pub fn request_from_sidechain(
            origin: OriginFor<T>,
            eth_tx_hash: H256,
            kind: IncomingRequestKind,
            network_id: BridgeNetworkId<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called request_from_sidechain");
            let from = ensure_signed(origin)?;
            let timepoint = bridge_multisig::Module::<T>::timepoint();
            match kind {
                IncomingRequestKind::Transaction(kind) => {
                    Self::add_request(OffchainRequest::LoadIncoming(
                        LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
                            from,
                            eth_tx_hash,
                            timepoint,
                            kind,
                            network_id,
                        )),
                    ))?;
                    let pays_fee = if kind == IncomingTransactionRequestKind::TransferXOR {
                        Pays::No
                    } else {
                        Pays::Yes
                    };
                    Ok(PostDispatchInfo {
                        actual_weight: None,
                        pays_fee,
                    })
                }
                IncomingRequestKind::Meta(kind) => {
                    if kind == IncomingMetaRequestKind::CancelOutgoingRequest {
                        fail!(Error::<T>::Unavailable);
                    }
                    let timepoint = bridge_multisig::Module::<T>::timepoint();
                    Self::add_request(OffchainRequest::load_incoming_meta(
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

        /// Finalize incoming request.
        ///
        /// At first, `finalize` is called on the request, if it fails, the `cancel` function
        /// gets called. Request status changes depending on the result (`Done` or `Failed`), and
        /// finally the request gets removed from the queue.
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
            let from = ensure_signed(origin)?;
            let _ = Self::ensure_bridge_account(&from, network_id)?;
            let request =
                Requests::<T>::get(network_id, &hash).ok_or(Error::<T>::UnknownRequest)?;
            let error_opt = request.finalize().err();
            if let Some(e) = error_opt {
                debug::error!("Incoming request failed {:?} {:?}", hash, e);
                Self::deposit_event(Event::IncomingRequestFinalizationFailed(hash));
                RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Failed(e));
                if let Err(e) = request.cancel() {
                    debug::error!("Request cancellation failed: {:?}, {:?}", e, request);
                    // Such errors should not occur in general, but we check it in tests, anyway.
                    #[cfg(not(test))]
                    debug_assert!(false, "unexpected cancellation error {:?}", e);
                }
            } else {
                debug::warn!("Incoming request finalized {:?}", hash);
                RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Done);
                Self::deposit_event(Event::IncomingRequestFinalized(hash));
            }
            Self::remove_request_from_queue(network_id, &hash);
            Ok(().into())
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
            let timepoint = bridge_multisig::Module::<T>::timepoint();
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::AddPeer(
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
                Self::add_request(OffchainRequest::outgoing(OutgoingRequest::AddPeerCompat(
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
            let timepoint = bridge_multisig::Module::<T>::timepoint();
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::RemovePeer(
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
                Self::add_request(OffchainRequest::outgoing(
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
            let timepoint = bridge_multisig::Module::<T>::timepoint();
            Self::add_request(OffchainRequest::outgoing(
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
            let timepoint = bridge_multisig::Module::<T>::timepoint();
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::Migrate(
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
        /// Calls `prepare` on the request, adds it to incoming requests queue and map, and removes
        /// corresponding pre-incoming request from requests queue.
        ///
        /// Can only be called by a bridge account.
        #[pallet::weight(<T as Config>::WeightInfo::register_incoming_request())]
        pub fn register_incoming_request(
            origin: OriginFor<T>,
            incoming_request: IncomingRequest<T>,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called register_incoming_request");
            let author = ensure_signed(origin.clone())?;
            let net_id = incoming_request.network_id();
            let _ = Self::ensure_bridge_account(&author, net_id)?;
            let sidechain_tx_hash = incoming_request.hash();
            let request_author = incoming_request.author().clone();
            let mut request = OffchainRequest::incoming(incoming_request);
            let incoming_request_hash = request.hash();
            ensure!(
                !Requests::<T>::contains_key(net_id, incoming_request_hash),
                Error::<T>::RequestIsAlreadyRegistered
            );
            Self::remove_request_from_queue(net_id, &sidechain_tx_hash);
            RequestStatuses::<T>::insert(net_id, sidechain_tx_hash, RequestStatus::Done);
            LoadToIncomingRequestHash::<T>::insert(
                net_id,
                sidechain_tx_hash,
                incoming_request_hash,
            );
            if let Err(e) = request.validate().and_then(|_| request.prepare()) {
                RequestStatuses::<T>::insert(
                    net_id,
                    incoming_request_hash,
                    RequestStatus::Failed(e),
                );
                return Err(e.into());
            }
            Requests::<T>::insert(net_id, &incoming_request_hash, request);
            RequestsQueue::<T>::mutate(net_id, |v| v.push(incoming_request_hash));
            RequestStatuses::<T>::insert(net_id, incoming_request_hash, RequestStatus::Pending);
            AccountRequests::<T>::mutate(request_author, |v| {
                v.push((net_id, incoming_request_hash))
            });
            Ok(().into())
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
            network_id: T::NetworkId,
        ) -> DispatchResultWithPostInfo {
            debug::debug!("called approve_request");
            let author = ensure_signed(origin)?;
            let net_id = network_id;
            Self::ensure_peer(&author, net_id)?;
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
            approvals.insert(signature_params);
            RequestApprovals::<T>::insert(net_id, &hash, &approvals);
            let current_status =
                RequestStatuses::<T>::get(net_id, &hash).ok_or(Error::<T>::UnknownRequest)?;
            if current_status == RequestStatus::Pending && approvals.len() == need_sigs {
                if let Err(err) = request.finalize() {
                    debug::error!("Outgoing request finalization failed: {:?}", err);
                    RequestStatuses::<T>::insert(net_id, hash, RequestStatus::Failed(err));
                    Self::deposit_event(Event::RequestFinalizationFailed(hash));
                    if let Err(e) = request.cancel() {
                        debug::error!("Request cancellation failed: {:?}, {:?}", e, request);
                        debug_assert!(false, "unexpected cancellation error {:?}", e);
                    }
                } else {
                    debug::debug!("Outgoing request approvals collected {:?}", hash);
                    RequestStatuses::<T>::insert(net_id, hash, RequestStatus::ApprovalsReady);
                    Self::deposit_event(Event::ApprovalsCollected(hash));
                }
                Self::remove_request_from_queue(net_id, &hash);
                let weight_info = <T as Config>::WeightInfo::approve_request_finalize();
                return Ok((Some(weight_info.0), weight_info.1).into());
            }
            Ok(().into())
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
            let author = ensure_signed(origin)?;
            let _ = Self::ensure_bridge_account(&author, network_id)?;
            let request = Requests::<T>::get(network_id, hash).ok_or(Error::<T>::UnknownRequest)?;
            Self::inner_abort_request(&request, hash, error, network_id);
            Self::deposit_event(Event::RequestAborted(hash));
            Ok(().into())
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
        /// Ethereum transaction is succeeded.
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
        /// Unknown asset id.
        UnknownAssetId,
        /// Failed to serialize JSON.
        JsonSerializationError,
        /// Failed to deserialize JSON.
        JsonDeserializationError,
        /// Failed to load sidechain node parameters.
        FailedToLoadSidechainNodeParams,
        /// Failed to load current sidechain height.
        LoadCurrentSidechainHeight,
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
    }

    impl<T: Config> Error<T> {
        pub fn should_retry(&self) -> bool {
            match self {
                Self::HttpFetchingError
                | Self::NoLocalAccountForSigning
                | Self::FailedToSendSignedTransaction
                | Self::FailedToSignMessage
                | Self::JsonDeserializationError => true,
                _ => false,
            }
        }
    }

    /// Registered requests queue handled by off-chain workers.
    #[pallet::storage]
    #[pallet::getter(fn requests_queue)]
    pub type RequestsQueue<T: Config> =
        StorageMap<_, Twox64Concat, T::NetworkId, Vec<H256>, ValueQuery>;

    /// Registered requests.
    #[pallet::storage]
    #[pallet::getter(fn request)]
    pub type Requests<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::NetworkId, Identity, H256, OffchainRequest<T>>;

    /// Used to identify an incoming request by the corresponding load request.
    #[pallet::storage]
    #[pallet::getter(fn load_to_incoming_request_hash)]
    pub type LoadToIncomingRequestHash<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::NetworkId, Identity, H256, H256, ValueQuery>;

    /// Requests statuses.
    #[pallet::storage]
    #[pallet::getter(fn request_status)]
    pub type RequestStatuses<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::NetworkId, Identity, H256, RequestStatus>;

    /// Requests submission height map (on substrate).
    #[pallet::storage]
    #[pallet::getter(fn request_submission_height)]
    pub type RequestSubmissionHeight<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::NetworkId, Identity, H256, T::BlockNumber, ValueQuery>;

    /// Outgoing requests approvals.
    #[pallet::storage]
    #[pallet::getter(fn approvals)]
    pub(super) type RequestApprovals<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::NetworkId,
        Identity,
        H256,
        BTreeSet<SignatureParams>,
        ValueQuery,
    >;

    /// Requests made by an account.
    #[pallet::storage]
    #[pallet::getter(fn account_requests)]
    pub(super) type AccountRequests<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<(T::NetworkId, H256)>, ValueQuery>;

    /// Registered asset kind.
    #[pallet::storage]
    #[pallet::getter(fn registered_asset)]
    pub(super) type RegisteredAsset<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::NetworkId, Identity, T::AssetId, AssetKind>;

    /// Precision (decimals) of a registered sidechain asset. Should be the same as in the ERC-20
    /// contract.
    #[pallet::storage]
    #[pallet::getter(fn sidechain_asset_precision)]
    pub(super) type SidechainAssetPrecision<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::NetworkId,
        Identity,
        T::AssetId,
        BalancePrecision,
        ValueQuery,
    >;

    /// Registered token `AssetId` on Thischain.
    #[pallet::storage]
    #[pallet::getter(fn registered_sidechain_asset)]
    pub(super) type RegisteredSidechainAsset<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::NetworkId, Blake2_128Concat, Address, T::AssetId>;

    /// Registered asset address on Sidechain.
    #[pallet::storage]
    #[pallet::getter(fn registered_sidechain_token)]
    pub(super) type RegisteredSidechainToken<T: Config> =
        StorageDoubleMap<_, Twox64Concat, T::NetworkId, Blake2_128Concat, T::AssetId, Address>;

    /// Network peers set.
    #[pallet::storage]
    #[pallet::getter(fn peers)]
    pub(super) type Peers<T: Config> =
        StorageMap<_, Twox64Concat, T::NetworkId, BTreeSet<T::AccountId>, ValueQuery>;

    /// Network pending (being added/removed) peer.
    #[pallet::storage]
    #[pallet::getter(fn pending_peer)]
    pub(super) type PendingPeer<T: Config> =
        StorageMap<_, Twox64Concat, T::NetworkId, T::AccountId>;

    /// Used for compatibility with XOR and VAL contracts.
    #[pallet::storage]
    pub(super) type PendingEthPeersSync<T: Config> = StorageValue<_, EthPeersSync, ValueQuery>;

    /// Peer account ID on Thischain.
    #[pallet::storage]
    #[pallet::getter(fn peer_account_id)]
    pub(super) type PeerAccountId<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        T::NetworkId,
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
        T::NetworkId,
        Blake2_128Concat,
        T::AccountId,
        Address,
        ValueQuery,
    >;

    /// Multi-signature bridge peers' account. `None` if there is no account and network with the given ID.
    #[pallet::storage]
    #[pallet::getter(fn bridge_account)]
    pub(super) type BridgeAccount<T: Config> =
        StorageMap<_, Twox64Concat, T::NetworkId, T::AccountId>;

    /// Thischain authority account.
    #[pallet::storage]
    #[pallet::getter(fn authority_account)]
    pub(super) type AuthorityAccount<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

    /// Bridge status.
    #[pallet::storage]
    #[pallet::getter(fn bridge_contract_status)]
    pub(super) type BridgeStatuses<T: Config> =
        StorageMap<_, Twox64Concat, T::NetworkId, BridgeStatus>;

    /// Smart-contract address on Sidechain.
    #[pallet::storage]
    #[pallet::getter(fn bridge_contract_address)]
    pub(super) type BridgeContractAddress<T: Config> =
        StorageMap<_, Twox64Concat, T::NetworkId, Address, ValueQuery>;

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
    pub(super) type NextNetworkId<T: Config> = StorageValue<_, T::NetworkId, ValueQuery>;

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

/// Events that can be emitted by Sidechain smart-contract.
#[cfg_attr(feature = "std", derive(PartialEq, Eq, RuntimeDebug))]
pub enum ContractEvent<Address, AccountId, Balance> {
    Deposit(AccountId, Balance, Address, H256),
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
    fn add_request(mut request: OffchainRequest<T>) -> Result<(), DispatchError> {
        let net_id = request.network_id();
        let bridge_status = BridgeStatuses::<T>::get(net_id).ok_or(Error::<T>::UnknownNetwork)?;
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
        Requests::<T>::insert(net_id, &hash, request.clone());
        RequestsQueue::<T>::mutate(net_id, |v| v.push(hash));
        RequestStatuses::<T>::insert(net_id, &hash, RequestStatus::Pending);
        let block_number = frame_system::Module::<T>::current_block_number();
        RequestSubmissionHeight::<T>::insert(net_id, &hash, block_number);
        Self::deposit_event(Event::RequestRegistered(hash));
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

    /// Loops through the given array of logs and finds the first one that matches the type
    /// and topic.
    fn parse_main_event(
        logs: &[Log],
        kind: IncomingTransactionRequestKind,
    ) -> Result<ContractEvent<Address, T::AccountId, Balance>, Error<T>> {
        for log in logs {
            if log.removed.unwrap_or(false) {
                continue;
            }
            let topic = match log.topics.get(0) {
                Some(x) => &x.0,
                None => continue,
            };
            match *topic {
                // Deposit(bytes32,uint256,address,bytes32)
                hex!("85c0fa492ded927d3acca961da52b0dda1debb06d8c27fe189315f06bb6e26c8")
                    if kind == IncomingTransactionRequestKind::Transfer =>
                {
                    let types = [
                        ParamType::FixedBytes(32),
                        ParamType::Uint(256),
                        ParamType::Address,
                        ParamType::FixedBytes(32),
                    ];
                    let decoded = ethabi::decode(&types, &log.data.0)
                        .map_err(|_| Error::<T>::EthAbiDecodingError)?;
                    let mut decoder = Decoder::<T>::new(decoded);
                    let asset_id = decoder.next_h256()?;
                    let token = decoder.next_address()?;
                    let amount = decoder.next_amount()?;
                    let to = decoder.next_account_id()?;
                    return Ok(ContractEvent::Deposit(to, amount, H160(token.0), asset_id));
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

    /// Queries the current finalized height of the local node with `chain_getFinalizedHead`
    /// RPC call.
    fn load_substrate_finalized_height() -> Result<T::BlockNumber, DispatchError> {
        let hash =
            Self::substrate_json_rpc_request::<_, types::H256>("chain_getFinalizedHead", &())?
                .pop()
                .ok_or(Error::<T>::FailedToLoadFinalizedHead)?;
        let header = Self::substrate_json_rpc_request::<_, types::SubstrateHeaderLimited>(
            "chain_getHeader",
            &[hash],
        )?
        .pop()
        .ok_or(Error::<T>::FailedToLoadBlockHeader)?;
        let number = <T::BlockNumber as From<u32>>::from(header.number.as_u32());
        Ok(number)
    }

    /// Sends a multisig transaction to register the parsed (from pre-incoming) incoming request.
    /// (see `register_incoming_request`).
    fn send_register_incoming_request(
        incoming_request: IncomingRequest<T>,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        let register_call = Call::<T>::register_incoming_request(incoming_request);
        let call = bridge_multisig::Call::as_multi(
            get_bridge_account::<T>(network_id),
            Some(timepoint),
            <<T as Config>::Call>::from(register_call).encode(),
            false,
            10_000_000_000_000u64,
        );
        Self::send_signed_transaction::<bridge_multisig::Call<T>>(call)
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
                        if kind == IncomingTransactionRequestKind::TransferXOR {
                            if let IncomingRequest::Transfer(transfer) = &mut incoming_request {
                                ensure!(
                                    transfer.asset_id == common::XOR.into(),
                                    Error::<T>::ExpectedXORTransfer
                                );
                                transfer.enable_taking_fee();
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

    /// Retrieves latest needed information about networks and handles corresponding
    /// requests queues.
    ///
    /// At first, it loads current Sidechain height and current finalized Thischain height.
    /// Then it handles each request in the requests queue if it was submitted at least at
    /// the finalized height. The same is done with incoming requests queue. All handled requests
    /// are added to local storage to not be handled twice by the off-chain worker.
    fn handle_network(network_id: T::NetworkId) {
        let string = format!("eth-bridge-ocw::eth-height-{:?}", network_id);
        let s_eth_height = StorageValueRef::persistent(string.as_bytes());
        let current_eth_height = match Self::load_current_height(network_id) {
            Ok(v) => v,
            Err(e) => {
                debug::info!(
                    "Failed to load current ethereum height. Skipping off-chain procedure. {:?}",
                    e
                );
                return;
            }
        };
        s_eth_height.set(&current_eth_height);
        let s_handled_requests = StorageValueRef::persistent(b"eth-bridge-ocw::handled-request");

        let substrate_finalized_height = match Self::load_substrate_finalized_height() {
            Ok(v) => v,
            Err(e) => {
                debug::info!(
                    "Failed to load substrate finalized block height ({:?}). Skipping off-chain procedure.",
                    e
                );
                return;
            }
        };

        let mut handled = s_handled_requests
            .get::<BTreeMap<H256, T::BlockNumber>>()
            .flatten()
            .unwrap_or_default();

        for request_hash in RequestsQueue::<T>::get(network_id) {
            let request = match Requests::<T>::get(network_id, request_hash) {
                Some(v) => v,
                _ => continue, // TODO: remove from queue
            };
            let request_submission_height: T::BlockNumber =
                Self::request_submission_height(network_id, &request_hash);
            if substrate_finalized_height < request_submission_height {
                continue;
            }
            let need_to_handle = match handled.get(&request_hash) {
                Some(height) => &request_submission_height > height,
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
                    } else {
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
                    handled.insert(request_hash, request_submission_height);
                }
            }
        }
        s_handled_requests.set(&handled);
    }

    /// Handles registered networks.
    fn offchain() {
        let s_networks_ids = StorageValueRef::persistent(STORAGE_NETWORK_IDS_KEY);
        let network_ids = s_networks_ids
            .get::<BTreeSet<T::NetworkId>>()
            .flatten()
            .unwrap_or_default();
        for network_id in network_ids {
            Self::handle_network(network_id);
        }
    }

    /// Makes off-chain HTTP request.
    fn http_request(
        url: &str,
        body: Vec<u8>,
        headers: &[(&'static str, String)],
    ) -> Result<Vec<u8>, Error<T>> {
        debug::trace!("Sending request to: {}", url);
        let mut request = rt_offchain::http::Request::post(url, vec![body]);
        let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(
            HTTP_REQUEST_TIMEOUT_SECS * 1000,
        ));
        for (key, value) in headers {
            request = request.add_header(*key, &*value);
        }
        let pending = request.deadline(timeout).send().map_err(|e| {
            debug::error!("Failed to send a request {:?}", e);
            <Error<T>>::HttpFetchingError
        })?;
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
    ) -> Result<Vec<O>, Error<T>> {
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
            serde_json::to_vec(&rpc::Call::MethodCall(rpc::MethodCall {
                jsonrpc: Some(rpc::Version::V2),
                method: method.into(),
                params,
                id: rpc::Id::Num(id as u64),
            }))
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
            .map_err(|_| Error::<T>::JsonDeserializationError)?;
        let results = match response {
            rpc::Response::Batch(xs) => xs,
            rpc::Response::Single(x) => vec![x],
        };
        results
            .into_iter()
            .map(|x| match x {
                rpc::Output::Success(s) => serde_json::from_value(s.result).map_err(|e| {
                    debug::error!("json_rpc_request: from_value failed, {}", e);
                    Error::<T>::JsonDeserializationError.into()
                }),
                _ => {
                    debug::error!("json_rpc_request: request failed");
                    Err(Error::<T>::JsonDeserializationError.into())
                }
            })
            .collect::<Result<Vec<_>, _>>()
    }

    /// Makes request to a Sidechain node. The node URL and credentials are stored in the local
    /// storage.
    fn eth_json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        method: &str,
        params: &I,
        network_id: T::NetworkId,
    ) -> Result<Vec<O>, Error<T>> {
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
        Self::json_rpc_request(&node_params.url, 1, method, params, &headers)
    }

    /// Makes request to the local node. The node URL is stored in the local storage.
    fn substrate_json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        method: &str,
        params: &I,
    ) -> Result<Vec<O>, Error<T>> {
        let s_node_url = StorageValueRef::persistent(STORAGE_SUB_NODE_URL_KEY);
        let node_url = s_node_url
            .get::<String>()
            .flatten()
            .unwrap_or_else(|| SUB_NODE_URL.into());
        let headers: Vec<(_, String)> = vec![("content-type", "application/json".into())];

        Self::json_rpc_request(&node_url, 0, method, params, &headers)
    }

    /// Sends a substrate transaction signed by an off-chain worker.
    fn send_signed_transaction<LocalCall: Clone + GetCallName>(
        call: LocalCall,
    ) -> Result<(), Error<T>>
    where
        T: CreateSignedTransaction<LocalCall>,
    {
        let signer = Signer::<T, T::PeerId>::any_account();
        if !signer.can_sign() {
            debug::error!("No local account available");
            fail!(<Error<T>>::NoLocalAccountForSigning);
        }
        debug::debug!("Sending signed transaction: {}", call.get_call_name());
        let result = signer.send_signed_transaction(|_acc| call.clone());

        match result {
            Some((_acc, Ok(_))) => {}
            Some((acc, Err(e))) => {
                debug::error!("[{:?}] Failed to send signed transaction: {:?}", acc.id, e);
                fail!(<Error<T>>::FailedToSendSignedTransaction);
            }
            _ => {
                debug::error!("Failed to send signed transaction");
                fail!(<Error<T>>::FailedToSendSignedTransaction);
            }
        };
        Ok(())
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
            )?
            .pop()
            .ok_or(Error::<T>::FailedToLoadIsUsed)?;
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
        let tx_hash = H256(tx_receipt.transaction_hash.0);

        let call = Self::parse_main_event(&tx_receipt.logs, kind)?;
        // TODO (optimization): pre-validate the parsed calls.
        IncomingRequest::<T>::try_from_contract_event(
            call,
            incoming_pre_request,
            at_height,
            tx_hash,
        )
    }

    /// Send a transaction to finalize the incoming request.
    fn send_finalize_incoming_request(
        hash: H256,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        debug::debug!("send_incoming_request_result: {:?}", hash);
        let transfer_call = Call::<T>::finalize_incoming_request(hash, network_id);
        let call = bridge_multisig::Call::as_multi(
            Self::bridge_account(network_id).expect("networks can't be removed; qed"),
            Some(timepoint),
            <<T as Config>::Call>::from(transfer_call).encode(),
            false,
            10_000_000_000_000_000u64,
        );
        Self::send_signed_transaction::<bridge_multisig::Call<T>>(call)?;
        Ok(())
    }

    /// Send 'abort request' transaction.
    fn send_abort_request(
        request_hash: H256,
        request_error: Error<T>,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        debug::debug!("send_abort_request: {:?}", request_hash);
        let abort_request_call =
            Call::<T>::abort_request(request_hash, request_error.into(), network_id);
        let call = bridge_multisig::Call::as_multi(
            Self::bridge_account(network_id).expect("networks can't be removed; qed"),
            Some(timepoint),
            <<T as Config>::Call>::from(abort_request_call).encode(),
            false,
            10_000_000_000_000_000u64,
        );
        Self::send_signed_transaction::<bridge_multisig::Call<T>>(call)?;
        Ok(())
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

        let result = signer.send_signed_transaction(|_acc| {
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
            Call::approve_request(
                ecdsa::Public::from_slice(&public.serialize_compressed()),
                hash,
                signature,
                request.network_id(),
            )
        });

        match result {
            Some((_acc, Ok(_))) => {
                debug::trace!("Signed transaction sent");
            }
            Some((acc, Err(e))) => {
                debug::error!("[{:?}] Failed in handle_outgoing_transfer: {:?}", acc.id, e);
                return Err(<Error<T>>::FailedToSendSignedTransaction);
            }
            _ => {
                debug::error!("Failed in handle_outgoing_transfer");
                return Err(<Error<T>>::FailedToSendSignedTransaction);
            }
        };
        Ok(())
    }

    /// Queries current height of Sidechain.
    fn load_current_height(network_id: T::NetworkId) -> Result<u64, Error<T>> {
        Self::eth_json_rpc_request::<_, types::U64>("eth_blockNumber", &(), network_id)?
            .first()
            .ok_or(Error::<T>::LoadCurrentSidechainHeight)
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
        )?
        .pop()
        .ok_or(Error::<T>::FailedToLoadTransaction)?;
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
        )?
        .pop()
        .ok_or(Error::<T>::FailedToLoadTransaction)?;
        let to = tx_receipt
            .to
            .map(|x| H160(x.0))
            .ok_or(Error::<T>::UnknownContractAddress)?;
        Self::ensure_known_contract(to, network_id)?;
        Ok(tx_receipt)
    }

    /// Checks if the account is a bridge peer.
    fn is_peer(who: &T::AccountId, network_id: T::NetworkId) -> bool {
        Self::peers(network_id).into_iter().any(|i| i == *who)
    }

    /// Ensures that the account is a bridge peer.
    fn ensure_peer(who: &T::AccountId, network_id: T::NetworkId) -> DispatchResult {
        ensure!(Self::is_peer(who, network_id), Error::<T>::Forbidden);
        Ok(())
    }

    /// Ensures that the account is a bridge multisig account.
    fn ensure_bridge_account(
        who: &T::AccountId,
        network_id: T::NetworkId,
    ) -> Result<T::AccountId, DispatchError> {
        let bridge_account_id =
            Self::bridge_account(network_id).ok_or(Error::<T>::UnknownNetwork)?;
        ensure!(who == &bridge_account_id, Error::<T>::Forbidden);
        Ok(bridge_account_id)
    }

    fn inner_abort_request(
        request: &OffchainRequest<T>,
        hash: H256,
        error: DispatchError,
        network_id: T::NetworkId,
    ) {
        RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Failed(error));
        if let Err(e) = request.cancel() {
            debug::error!("Request cancellation failed: {:?}, {:?}", e, request);
            debug_assert!(false, "unexpected cancellation error {:?}", e);
        }
        Self::remove_request_from_queue(network_id, &hash);
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

impl<T: Config> Module<T> {
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
