#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;
extern crate jsonrpc_core as rpc;

use crate::contract::{
    functions, init_add_peer_by_peer_fn, init_remove_peer_by_peer_fn, ADD_PEER_BY_PEER_FN,
    ADD_PEER_BY_PEER_ID, ADD_PEER_BY_PEER_TX_HASH_ARG_POS, FUNCTIONS, REMOVE_PEER_BY_PEER_FN,
    REMOVE_PEER_BY_PEER_ID, REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS,
};
use crate::types::{Bytes, CallRequest, Log, Transaction, TransactionReceipt};
use alloc::string::String;
use codec::{Decode, Encode, FullCodec};
use common::{prelude::Balance, AssetSymbol, BalancePrecision};
use core::{convert::TryFrom, fmt, iter, line, stringify};
use ethabi::{ParamType, Token};
use frame_support::sp_runtime::traits::{AtLeast32Bit, MaybeSerializeDeserialize, Member};
use frame_support::traits::{Get, GetCallName};
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure, fail, sp_io,
    sp_runtime::{
        app_crypto::{ecdsa, sp_core, Public},
        offchain::{
            self as rt_offchain,
            storage::StorageValueRef,
            storage_lock::{BlockNumberProvider, StorageLock, Time},
        },
        traits::{IdentifyAccount, One},
        KeyTypeId, MultiSigner, Percent,
    },
    weights::{Pays, Weight},
    IterableStorageDoubleMap, Parameter, RuntimeDebug, StorageDoubleMap,
};
use frame_system::{
    ensure_root, ensure_signed,
    offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer},
};
use hex_literal::hex;
use permissions::{Scope, MINT};
use requests::*;
use rpc::Params;
use rustc_hex::ToHex;
use secp256k1::PublicKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sp_core::{H160, H256};
use sp_io::hashing::{blake2_256, keccak_256};
use sp_std::marker::PhantomData;
use sp_std::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    convert::{identity, TryInto},
    fmt::{Debug, Formatter},
    prelude::*,
};
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
    fn finalize_mark_as_done() -> Weight;
    fn prepare_for_migration() -> Weight;
    fn migrate() -> Weight;
}

type Address = H160;
type EthereumAddress = Address;

mod weights;

mod contract;
#[cfg(test)]
mod mock;
pub mod requests;
#[cfg(test)]
mod tests;
pub mod types;

const SUB_NODE_URL: &str = "http://127.0.0.1:9954";
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 10;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"bridge";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
pub const TECH_ACCOUNT_AUTHORITY: &[u8] = b"authority";

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ethb");
pub const CONFIRMATION_INTERVAL: u64 = 30;
pub const STORAGE_SUB_NODE_URL_KEY: &[u8] = b"eth-bridge-ocw::sub-node-url";
pub const STORAGE_PEER_SECRET_KEY: &[u8] = b"eth-bridge-ocw::secret-key";
pub const STORAGE_ETH_NODE_PARAMS: &str = "eth-bridge-ocw::node-params";
pub const STORAGE_NETWORK_IDS_KEY: &[u8] = b"eth-bridge-ocw::network-ids";

type AssetIdOf<T> = <T as assets::Trait>::AssetId;
type Timepoint<T> = bridge_multisig::Timepoint<<T as frame_system::Trait>::BlockNumber>;
type BridgeNetworkId<T> = <T as Trait>::NetworkId;

pub mod crypto {
    use crate::KEY_TYPE;

    use frame_support::sp_runtime::{
        app_crypto::{app_crypto, ecdsa},
        MultiSignature, MultiSigner,
    };

    app_crypto!(ecdsa, KEY_TYPE);

    pub struct TestAuthId;

    // implemented for ocw-runtime
    impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
        type RuntimeAppPublic = Public;
        type GenericSignature = ecdsa::Signature;
        type GenericPublic = ecdsa::Public;
    }
}

#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct NodeParams {
    url: String,
    credentials: Option<String>,
}

#[cfg(feature = "std")]
#[derive(Clone, RuntimeDebug, Serialize, Deserialize)]
pub struct PeerConfig<NetworkId: std::hash::Hash + Eq> {
    pub networks: HashMap<NetworkId, NodeParams>,
}

#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
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

pub fn public_key_to_eth_address(pub_key: &PublicKey) -> Address {
    let hash = keccak_256(&pub_key.serialize()[1..]);
    Address::from_slice(&hash[12..])
}

/// The type of request we can send to the offchain worker
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub enum OutgoingRequest<T: Trait> {
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

impl<T: Trait> OutgoingRequest<T> {
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

    fn hash(&self) -> H256 {
        let hash = self.using_encoded(blake2_256);
        H256(hash)
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

#[derive(Clone, Copy, Encode, Decode, RuntimeDebug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum IncomingRequestKind {
    Transfer,
    AddAsset,
    AddPeer,
    RemovePeer,
    ClaimPswap,
    CancelOutgoingRequest,
    MarkAsDone,
    PrepareForMigration,
    Migrate,
    AddPeerCompat,
    RemovePeerCompat,
}

impl IncomingRequestKind {
    pub fn is_compat(&self) -> bool {
        *self == Self::AddPeerCompat || *self == Self::RemovePeerCompat
    }
}

/// The type of request we can send to the offchain worker
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum IncomingRequest<T: Trait> {
    Transfer(IncomingTransfer<T>),
    AddAsset(IncomingAddToken<T>),
    ChangePeers(IncomingChangePeers<T>),
    ClaimPswap(IncomingClaimPswap<T>),
    CancelOutgoingRequest(IncomingCancelOutgoingRequest<T>),
    PrepareForMigration(IncomingPrepareForMigration<T>),
    Migrate(IncomingMigrate<T>),
    ChangePeersCompat(IncomingChangePeersCompat<T>),
}

impl<T: Trait> IncomingRequest<T> {
    pub fn try_from_contract_event(
        event: ContractEvent<T::AssetId, Address, T::AccountId, Balance>,
        incoming_request: IncomingPreRequest<T>,
        at_height: u64,
        tx_hash: H256,
        tx_receipt: TransactionReceipt,
    ) -> Result<Self, DispatchError> {
        let network_id = incoming_request.network_id;
        let timepoint = incoming_request.timepoint;

        let req = match event {
            ContractEvent::Deposit(to, amount, token_address, raw_asset_id) => {
                let (asset_id, asset_kind) = Module::<T>::get_asset_by_raw_asset_id(
                    raw_asset_id,
                    &token_address,
                    network_id,
                )?
                .ok_or(Error::<T>::UnsupportedAssetId)?;
                IncomingRequest::Transfer(IncomingTransfer {
                    from: Default::default(),
                    to,
                    asset_id,
                    asset_kind,
                    amount,
                    tx_hash,
                    at_height,
                    timepoint,
                    network_id,
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
                    tx_hash,
                    at_height,
                    timepoint,
                    network_id,
                })
            }
            ContractEvent::ClaimPswap(account_id) => {
                IncomingRequest::ClaimPswap(IncomingClaimPswap {
                    account_id,
                    eth_address: H160(tx_receipt.from.0),
                    tx_hash,
                    at_height,
                    timepoint,
                    network_id,
                })
            }
            ContractEvent::PreparedForMigration => {
                IncomingRequest::PrepareForMigration(IncomingPrepareForMigration {
                    tx_hash,
                    at_height,
                    timepoint,
                    network_id,
                })
            }
            ContractEvent::Migrated(to) => IncomingRequest::Migrate(IncomingMigrate {
                new_contract_address: to,
                tx_hash,
                at_height,
                timepoint,
                network_id,
            }),
            _ => fail!(Error::<T>::UnknownMethodId),
        };
        Ok(req)
    }

    fn hash(&self) -> H256 {
        match self {
            IncomingRequest::Transfer(request) => request.tx_hash,
            IncomingRequest::AddAsset(request) => request.tx_hash,
            IncomingRequest::ChangePeers(request) => request.tx_hash,
            IncomingRequest::ClaimPswap(request) => request.tx_hash,
            IncomingRequest::CancelOutgoingRequest(request) => request.initial_request_hash,
            IncomingRequest::PrepareForMigration(request) => request.tx_hash,
            IncomingRequest::Migrate(request) => request.tx_hash,
            IncomingRequest::ChangePeersCompat(request) => request.tx_hash,
        }
    }

    fn network_id(&self) -> T::NetworkId {
        match self {
            IncomingRequest::Transfer(request) => request.network_id,
            IncomingRequest::AddAsset(request) => request.network_id,
            IncomingRequest::ChangePeers(request) => request.network_id,
            IncomingRequest::ClaimPswap(request) => request.network_id,
            IncomingRequest::CancelOutgoingRequest(request) => request.network_id,
            IncomingRequest::PrepareForMigration(request) => request.network_id,
            IncomingRequest::Migrate(request) => request.network_id,
            IncomingRequest::ChangePeersCompat(request) => request.network_id,
        }
    }

    fn at_height(&self) -> u64 {
        match self {
            IncomingRequest::Transfer(request) => request.at_height,
            IncomingRequest::AddAsset(request) => request.at_height,
            IncomingRequest::ChangePeers(request) => request.at_height,
            IncomingRequest::ClaimPswap(request) => request.at_height,
            IncomingRequest::CancelOutgoingRequest(request) => request.at_height,
            IncomingRequest::PrepareForMigration(request) => request.at_height,
            IncomingRequest::Migrate(request) => request.at_height,
            IncomingRequest::ChangePeersCompat(request) => request.at_height,
        }
    }

    pub fn prepare(&self) -> Result<(), DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.prepare(),
            IncomingRequest::AddAsset(_request) => Ok(()),
            IncomingRequest::ChangePeers(_request) => Ok(()),
            IncomingRequest::ClaimPswap(_request) => Ok(()),
            IncomingRequest::CancelOutgoingRequest(request) => request.prepare(),
            IncomingRequest::PrepareForMigration(request) => request.prepare(),
            IncomingRequest::Migrate(request) => request.prepare(),
            IncomingRequest::ChangePeersCompat(_request) => Ok(()),
        }
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.cancel(),
            IncomingRequest::AddAsset(_request) => Ok(()),
            IncomingRequest::ChangePeers(_request) => Ok(()),
            IncomingRequest::ClaimPswap(_request) => Ok(()),
            IncomingRequest::CancelOutgoingRequest(request) => request.cancel(),
            IncomingRequest::PrepareForMigration(request) => request.cancel(),
            IncomingRequest::Migrate(request) => request.cancel(),
            IncomingRequest::ChangePeersCompat(_request) => Ok(()),
        }
    }

    pub fn finalize(&self) -> Result<H256, DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.finalize(),
            IncomingRequest::AddAsset(request) => request.finalize(),
            IncomingRequest::ChangePeers(request) => request.finalize(),
            IncomingRequest::ClaimPswap(request) => request.finalize(),
            IncomingRequest::CancelOutgoingRequest(request) => request.finalize(),
            IncomingRequest::PrepareForMigration(request) => request.finalize(),
            IncomingRequest::Migrate(request) => request.finalize(),
            IncomingRequest::ChangePeersCompat(request) => request.finalize(),
        }
    }

    pub fn timepoint(&self) -> Timepoint<T> {
        match self {
            IncomingRequest::Transfer(request) => request.timepoint(),
            IncomingRequest::AddAsset(request) => request.timepoint(),
            IncomingRequest::ChangePeers(request) => request.timepoint(),
            IncomingRequest::ClaimPswap(request) => request.timepoint(),
            IncomingRequest::CancelOutgoingRequest(request) => request.timepoint(),
            IncomingRequest::PrepareForMigration(request) => request.timepoint(),
            IncomingRequest::Migrate(request) => request.timepoint(),
            IncomingRequest::ChangePeersCompat(request) => request.timepoint(),
        }
    }
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct IncomingPreRequest<T: Trait> {
    author: T::AccountId,
    hash: H256,
    timepoint: Timepoint<T>,
    kind: IncomingRequestKind,
    network_id: T::NetworkId,
}

impl<T: Trait> IncomingPreRequest<T> {
    pub fn new(
        author: T::AccountId,
        hash: H256,
        timepoint: Timepoint<T>,
        kind: IncomingRequestKind,
        network_id: T::NetworkId,
    ) -> Self {
        IncomingPreRequest {
            author,
            hash,
            timepoint,
            kind,
            network_id,
        }
    }
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub enum OffchainRequest<T: Trait> {
    Outgoing(OutgoingRequest<T>, H256),
    Incoming(IncomingPreRequest<T>),
}

impl<T: Trait> OffchainRequest<T> {
    pub fn outgoing(request: OutgoingRequest<T>) -> Self {
        let hash = request.hash();
        Self::Outgoing(request, hash)
    }

    fn hash(&self) -> H256 {
        match self {
            OffchainRequest::Outgoing(_request, hash) => *hash,
            OffchainRequest::Incoming(request) => match request.kind {
                IncomingRequestKind::CancelOutgoingRequest | IncomingRequestKind::MarkAsDone => {
                    H256(self.using_encoded(blake2_256))
                }
                _ => H256(request.hash.0),
            },
        }
    }

    fn network_id(&self) -> T::NetworkId {
        match self {
            OffchainRequest::Outgoing(request, _) => request.network_id(),
            OffchainRequest::Incoming(request) => request.network_id,
        }
    }

    fn author(&self) -> &T::AccountId {
        match self {
            OffchainRequest::Outgoing(request, _) => request.author(),
            OffchainRequest::Incoming(request) => &request.author,
        }
    }

    fn prepare(&mut self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.prepare(),
            OffchainRequest::Incoming(_) => Ok(()),
        }
    }

    #[allow(unused)]
    fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.cancel(),
            OffchainRequest::Incoming(_) => Ok(()),
        }
    }

    fn validate(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.validate(),
            OffchainRequest::Incoming(request) => {
                match request.kind {
                    IncomingRequestKind::MarkAsDone => {
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

    pub fn as_outgoing(&self) -> Option<&OutgoingRequest<T>> {
        match self {
            OffchainRequest::Outgoing(r, _) => Some(r),
            _ => None,
        }
    }
}

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

#[derive(PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum RequestStatus {
    Pending,
    Frozen,
    ApprovalsReady,
    Failed,
    Done,
}

pub trait Trait:
    frame_system::Trait
    + CreateSignedTransaction<Call<Self>>
    + CreateSignedTransaction<bridge_multisig::Call<Self>>
    + assets::Trait
    + bridge_multisig::Trait
    + fmt::Debug
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
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

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct NetworkParams<AccountId: Ord> {
    pub bridge_contract_address: Address,
    pub initial_peers: BTreeSet<AccountId>,
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct NetworkConfig<T: Trait> {
    pub initial_peers: BTreeSet<T::AccountId>,
    pub bridge_account_id: T::AccountId,
    pub tokens: Vec<(T::AssetId, Option<H160>, AssetKind)>,
    pub bridge_contract_address: Address,
    pub reserves: Vec<(T::AssetId, Balance)>,
}

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

decl_storage! {
    trait Store for Module<T: Trait> as EthBridge {
        pub RequestsQueue get(fn requests_queue): map hasher(twox_64_concat) T::NetworkId => Vec<OffchainRequest<T>>;

        pub IncomingRequests get(fn incoming_requests): double_map hasher(twox_64_concat) T::NetworkId, hasher(identity) H256 => Option<IncomingRequest<T>>;
        pub PendingIncomingRequests get(fn pending_incoming_requests): map hasher(twox_64_concat) T::NetworkId => BTreeSet<H256>;

        pub Request get(fn request): double_map hasher(twox_64_concat) T::NetworkId, hasher(identity) H256 => Option<OffchainRequest<T>>;
        pub RequestStatuses get(fn request_status): double_map hasher(twox_64_concat) T::NetworkId, hasher(identity) H256 => Option<RequestStatus>;
        pub RequestSubmissionHeight get(fn request_submission_height): double_map hasher(twox_64_concat) T::NetworkId, hasher(identity) H256 => T::BlockNumber;
        RequestApprovals get(fn approvals): double_map hasher(twox_64_concat) T::NetworkId, hasher(identity) H256 => BTreeSet<SignatureParams>;
        AccountRequests get(fn account_requests): map hasher(blake2_128_concat) T::AccountId => Vec<(T::NetworkId, H256)>; // TODO: should be a linked-set

        RegisteredAsset get(fn registered_asset): double_map hasher(twox_64_concat) T::NetworkId, hasher(identity) T::AssetId => Option<AssetKind>;
        RegisteredSidechainAsset get(fn registered_sidechain_asset): double_map hasher(twox_64_concat) T::NetworkId, hasher(blake2_128_concat) Address => Option<T::AssetId>;
        RegisteredSidechainToken get(fn registered_sidechain_token): double_map hasher(twox_64_concat) T::NetworkId, hasher(blake2_128_concat) T::AssetId => Option<Address>;

        Peers get(fn peers): map hasher(twox_64_concat) T::NetworkId => BTreeSet<T::AccountId>;
        PendingPeer get(fn pending_peer): map hasher(twox_64_concat) T::NetworkId => Option<T::AccountId>;
        /// Used for compatibility with XOR and VAL contracts.
        PendingEthPeersSync: EthPeersSync;
        PeerAccountId get(fn peer_account_id): double_map hasher(twox_64_concat) T::NetworkId, hasher(blake2_128_concat) Address => T::AccountId;
        PeerAddress get(fn peer_address): double_map hasher(twox_64_concat) T::NetworkId, hasher(blake2_128_concat) T::AccountId => Address;

        /// Multi-signature bridge peers' account. `None` if there is no network with the given ID.
        BridgeAccount get(fn bridge_account): map hasher(twox_64_concat) T::NetworkId => Option<T::AccountId>;
        AuthorityAccount get(fn authority_account) config(): T::AccountId;

        BridgeBridgeStatus get(fn bridge_contract_status): map hasher(twox_64_concat) T::NetworkId => BridgeStatus;
        BridgeContractAddress get(fn bridge_contract_address): map hasher(twox_64_concat) T::NetworkId => Address;
        XorMasterContractAddress get(fn xor_master_contract_address) config(): Address;
        ValMasterContractAddress get(fn val_master_contract_address) config(): Address;
        PswapContractAddress get(fn pswap_contract_address) config(): Address;

        // None means the address owns no pswap.
        // 0 means the address claimed them.
        PswapOwners: map hasher(identity) Address => Option<Balance>;

        LastNetworkId: T::NetworkId;
    }
    add_extra_genesis {
        config(networks): Vec<NetworkConfig<T>>;
        config(pswap_owners): Vec<(H160, Balance)>;
        build(|config| {
            for network in &config.networks {
                let net_id = LastNetworkId::<T>::get();
                let peers_account_id = &network.bridge_account_id;
                BridgeContractAddress::<T>::insert(net_id, network.bridge_contract_address);
                BridgeAccount::<T>::insert(net_id, peers_account_id.clone());
                Peers::<T>::insert(net_id, network.initial_peers.clone());

                for (asset_id, opt_token_address, kind) in &network.tokens {
                    if let Some(token_address) = opt_token_address {
                        let token_address = Address::from(token_address.0);
                        RegisteredSidechainAsset::<T>::insert(net_id, token_address, *asset_id);
                        RegisteredSidechainToken::<T>::insert(net_id, &asset_id, token_address);
                    }
                    RegisteredAsset::<T>::insert(net_id, asset_id, kind);
                }
                // TODO: consider to change to Limited.
                let scope = Scope::Unlimited;
                let permission_ids = [MINT];
                for permission_id in &permission_ids {
                    permissions::Module::<T>::assign_permission(
                        peers_account_id.clone(),
                        &peers_account_id,
                        *permission_id,
                        scope,
                    ).expect("failed to assign permissions for a bridge account");
                }
                LastNetworkId::<T>::set(net_id + T::NetworkId::one());
            }

            for (address, balance) in &config.pswap_owners {
                PswapOwners::insert(Address::from_slice(address.as_bytes()), balance);
            }
        })
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        SomethingStored(u32, AccountId),
        RequestRegistered(H256),
        ApprovalsCollected(OutgoingRequestEncoded, BTreeSet<SignatureParams>),
        RequestFinalizationFailed(H256),
        IncomingRequestFinalizationFailed(H256),
        IncomingRequestFinalized(H256),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        StorageOverflow,
        HttpFetchingError,
        AccountNotFound,
        Forbidden,
        TransferIsAlreadyRegistered,
        FailedToLoadTransaction,
        FailedToLoadPrecision,
        UnknownMethodId,
        InvalidFunctionInput,
        InvalidSignature,
        InvalidUint,
        InvalidAmount,
        InvalidBalance,
        InvalidString,
        InvalidByte,
        InvalidAddress,
        InvalidAssetId,
        InvalidAccountId,
        InvalidBool,
        InvalidH256,
        UnknownEvent,
        UnknownTokenAddress,
        NoLocalAccountForSigning,
        UnsupportedAssetId,
        FailedToSignMessage,
        FailedToSendSignedTransaction,
        TokenIsNotOwnedByTheAuthor,
        TokenIsAlreadyAdded,
        DuplicatedRequest,
        UnsupportedToken,
        UnknownPeerAddress,
        EthAbiEncodingError,
        EthAbiDecodingError,
        EthTransactionIsFailed,
        EthTransactionIsSucceeded,
        NoPendingPeer,
        WrongPendingPeer,
        TooManyPendingPeers,
        FailedToGetAssetById,
        CantAddMorePeers,
        CantRemoveMorePeers,
        PeerIsAlreadyAdded,
        UnknownPeerId,
        CantReserveFunds,
        AlreadyClaimed,
        FailedToLoadBlockHeader,
        FailedToLoadFinalizedHead,
        UnknownContractAddress,
        InvalidContractInput,
        RequestIsNotOwnedByTheAuthor,
        FailedToParseTxHashInCall,
        RequestIsNotReady,
        UnknownRequest,
        RequestNotFinalizedOnSidechain,
        UnknownNetwork,
        ContractIsInMigrationStage,
        ContractIsNotInMigrationStage,
        ContractIsAlreadyInMigrationStage,
        Unavailable,
        Other,
    }
}

pub fn majority(peers_count: usize) -> usize {
    peers_count - (peers_count - 1) / 3
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        #[weight = <T as Trait>::WeightInfo::register_bridge()]
        pub fn register_bridge(origin, bridge_contract_address: Address, initial_peers: BTreeSet<T::AccountId>) {
            let author = ensure_signed(origin)?;
            // TODO: governence
            let net_id = LastNetworkId::<T>::get();
            let peers_account_id = bridge_multisig::Module::<T>::register_multisig_inner(
                author,
                initial_peers.iter().cloned().collect(),
                Percent::from_parts(67)
            )?;
            BridgeContractAddress::<T>::insert(net_id, bridge_contract_address);
            BridgeAccount::<T>::insert(net_id, peers_account_id);
            Peers::<T>::insert(net_id, initial_peers);
            LastNetworkId::<T>::set(net_id + T::NetworkId::one());
        }

        #[weight = <T as Trait>::WeightInfo::add_asset()]
        pub fn add_asset(
            origin,
            asset_id: AssetIdOf<T>,
            network_id: BridgeNetworkId<T>,
        ) {
            debug::debug!("called add_asset");
            let from = ensure_signed(origin)?;
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::AddAsset(OutgoingAddAsset {
                author: from.clone(),
                asset_id,
                nonce,
                network_id,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
        }

        #[weight = <T as Trait>::WeightInfo::add_sidechain_token()]
        pub fn add_sidechain_token(
            origin,
            token_address: EthereumAddress,
            ticker: String,
            name: String,
            decimals: u8,
            network_id: BridgeNetworkId<T>,
        ) {
            debug::debug!("called add_sidechain_token");
            let from = ensure_signed(origin)?;
            let authority_account_id = Self::authority_account();
            ensure!(from == authority_account_id, Error::<T>::Forbidden);
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken {
                author: from.clone(),
                token_address,
                ticker,
                name,
                decimals,
                nonce,
                network_id,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
        }

        #[weight = <T as Trait>::WeightInfo::transfer_to_sidechain()]
        pub fn transfer_to_sidechain(
            origin,
            asset_id: AssetIdOf<T>,
            to: EthereumAddress,
            amount: Balance,
            network_id: BridgeNetworkId<T>,
        ) {
            debug::debug!("called transfer_to_sidechain");
            let from = ensure_signed(origin)?;
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer {
                from: from.clone(),
                to,
                asset_id,
                amount,
                nonce,
                network_id,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
        }

        #[weight = <T as Trait>::WeightInfo::request_from_sidechain()]
        pub fn request_from_sidechain(
            origin,
            eth_tx_hash: H256,
            kind: IncomingRequestKind,
            network_id: BridgeNetworkId<T>,
        ) {
            debug::debug!("called request_from_sidechain");
            let from = ensure_signed(origin)?;
            if kind == IncomingRequestKind::CancelOutgoingRequest {
                fail!(Error::<T>::Unavailable);
            }
            let timepoint = bridge_multisig::Module::<T>::timepoint();
            Self::add_request(OffchainRequest::Incoming(IncomingPreRequest::new(
                from,
                eth_tx_hash,
                timepoint,
                kind,
                network_id,
            )))?;
        }

        #[weight = (0, Pays::No)]
        pub fn finalize_incoming_request(
            origin,
            result: Result<IncomingRequest<T>, (H256, DispatchError)>,
            network_id: BridgeNetworkId<T>
        ) {
            debug::debug!("called finalize_incoming_request");

            let from = ensure_signed(origin)?;
            let _ = Self::ensure_bridge_account(&from, network_id)?;

            let result = result.and_then(|req| {
                let hash = H256(req.hash().0);
                let result = req.finalize().map_err(|e| (hash, e));
                if result.is_err() {
                    if let Err(e) = req.cancel() {
                        debug::error!("Request cancellation failed: {:?}, {:?}", e, req);
                    }
                }
                result
            });
            let hash = match result {
                Ok(hash) => {
                    debug::warn!("Incoming request finalized {:?}", hash);
                    RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Done);
                    Self::deposit_event(RawEvent::IncomingRequestFinalized(hash));
                    hash
                }
                Err((hash, e)) => {
                    debug::error!("Incoming request finalization failed {:?} {:?}", hash, e);
                    RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Failed);
                    Self::deposit_event(RawEvent::IncomingRequestFinalizationFailed(hash));
                    hash
                }
            };
            PendingIncomingRequests::<T>::mutate(network_id, |set| {
                set.remove(&hash)
            });
            Self::remove_request_from_queue(network_id, &hash);
        }

        #[weight = <T as Trait>::WeightInfo::add_peer()]
        pub fn add_peer(
            origin,
            account_id: T::AccountId,
            address: EthereumAddress,
            network_id: BridgeNetworkId<T>,
        ) {
            debug::debug!("called change_peers_out");
            let from = ensure_signed(origin)?;
            ensure!(from == Self::authority_account(), Error::<T>::Forbidden);
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::AddPeer(OutgoingAddPeer {
                author: from.clone(),
                peer_account_id: account_id.clone(),
                peer_address: address,
                nonce,
                network_id,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
            if network_id == T::GetEthNetworkId::get() {
                let nonce = frame_system::Module::<T>::account_nonce(&from);
                Self::add_request(OffchainRequest::outgoing(OutgoingRequest::AddPeerCompat(OutgoingAddPeerCompat {
                    author: from.clone(),
                    peer_account_id: account_id,
                    peer_address: address,
                    nonce,
                    network_id,
                })))?;
                frame_system::Module::<T>::inc_account_nonce(&from);
            }
        }

        #[weight = <T as Trait>::WeightInfo::remove_peer()]
        pub fn remove_peer(origin, account_id: T::AccountId, network_id: BridgeNetworkId<T>) {
            debug::debug!("called change_peers_out");
            let from = ensure_signed(origin)?;
            ensure!(from == Self::authority_account(), Error::<T>::Forbidden);
            let peer_address = Self::peer_address(network_id, &account_id);
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::RemovePeer(OutgoingRemovePeer {
                author: from.clone(),
                peer_account_id: account_id.clone(),
                peer_address,
                nonce,
                network_id,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
            if network_id == T::GetEthNetworkId::get() {
                let nonce = frame_system::Module::<T>::account_nonce(&from);
                Self::add_request(OffchainRequest::outgoing(OutgoingRequest::RemovePeerCompat(OutgoingRemovePeerCompat {
                    author: from.clone(),
                    peer_account_id: account_id,
                    peer_address,
                    nonce,
                    network_id,
                })))?;
                frame_system::Module::<T>::inc_account_nonce(&from);
            }
        }

        #[weight = <T as Trait>::WeightInfo::prepare_for_migration()]
        pub fn prepare_for_migration(origin, network_id: BridgeNetworkId<T>) {
            debug::debug!("called prepare_for_migration");
            let from = ensure_signed(origin)?;
            ensure!(from == Self::authority_account(), Error::<T>::Forbidden);
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::PrepareForMigration(OutgoingPrepareForMigration {
                author: from.clone(),
                nonce,
                network_id,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
        }

        #[weight = <T as Trait>::WeightInfo::migrate()]
        pub fn migrate(
            origin,
            new_contract_address: EthereumAddress,
            erc20_native_tokens: Vec<EthereumAddress>,
            network_id: BridgeNetworkId<T>
        ) {
            debug::debug!("called prepare_for_migration");
            let from = ensure_signed(origin)?;
            ensure!(from == Self::authority_account(), Error::<T>::Forbidden);
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::Migrate(OutgoingMigrate {
                author: from.clone(),
                new_contract_address,
                erc20_native_tokens,
                nonce,
                network_id,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
        }

        // TODO: handle incoming requests without register
        #[weight = (0, Pays::No)]
        pub fn register_incoming_request(origin, incoming_request: IncomingRequest<T>) {
            debug::debug!("called register_incoming_request");
            let author = ensure_signed(origin)?;
            let _ = Self::ensure_bridge_account(&author, incoming_request.network_id())?;
            let tx_hash = incoming_request.hash();
            let net_id = incoming_request.network_id();
            ensure!(
                !PendingIncomingRequests::<T>::get(net_id).contains(&tx_hash),
                Error::<T>::TransferIsAlreadyRegistered
            );
            incoming_request.prepare()?;
            PendingIncomingRequests::<T>::mutate(net_id, |transfers| transfers.insert(tx_hash));
            Self::remove_request_from_queue(net_id, &tx_hash);
            IncomingRequests::<T>::insert(net_id, &tx_hash, incoming_request);
        }

        #[weight = (0, Pays::No)]
        pub fn approve_request(
            origin,
            ocw_public: ecdsa::Public,
            request: OutgoingRequest<T>,
            request_encoded: OutgoingRequestEncoded,
            signature_params: SignatureParams
        ) {
            debug::debug!("called approve_request");
            let author = ensure_signed(origin)?;
            let net_id = request.network_id();
            Self::ensure_peer(&author, net_id)?;
            if !Self::verify_message(
                request_encoded.as_raw(),
                &signature_params,
                &ocw_public,
                &author,
            ) {
                // TODO: punish the off-chain worker
                return Err(Error::<T>::InvalidSignature.into());
            }
            debug::info!("Verified request approve {:?}", request_encoded);
            let hash = request.hash();
            let mut approvals = RequestApprovals::<T>::get(net_id, &hash);
            let pending_peers_len = if PendingPeer::<T>::get(net_id).is_some() {
                1
            } else {
                0
            };
            let need_sigs = majority(Self::peers(net_id).len()) + pending_peers_len;
            approvals.insert(signature_params);
            RequestApprovals::<T>::insert(net_id, &hash, &approvals);
            let current_status = RequestStatuses::<T>::get(net_id, &hash).unwrap_or(RequestStatus::Pending);
            if current_status == RequestStatus::Pending && approvals.len() == need_sigs {
                if let Err(err) = request.finalize() {
                    debug::error!("Outgoing request finalization failed: {:?}", err);
                    RequestStatuses::<T>::insert(net_id, hash, RequestStatus::Failed);
                    Self::deposit_event(RawEvent::RequestFinalizationFailed(hash));
                    if let Err(e) = request.cancel() {
                        debug::error!("Request cancellation failed: {:?}, {:?}", e, request)
                    }
                } else {
                    debug::debug!("Outgoing request finalized {:?}", hash);
                    RequestStatuses::<T>::insert(net_id, hash, RequestStatus::ApprovalsReady);
                    Self::deposit_event(RawEvent::ApprovalsCollected(
                        request_encoded,
                        approvals,
                    ));
                }
                Self::remove_request_from_queue(net_id, &hash);
            }
        }

        // TODO: maybe rewrite to finalize with `finalize_incoming_request`
        #[weight = <T as Trait>::WeightInfo::finalize_mark_as_done()]
        pub fn finalize_mark_as_done(origin, request_hash: H256, network_id: BridgeNetworkId<T>) {
            debug::debug!("called finalize_mark_as_done");
            let author = ensure_signed(origin)?;
            let bridge_account_id = get_bridge_account::<T>(network_id);
            ensure!(author == bridge_account_id, Error::<T>::Forbidden);
            let request_status = RequestStatuses::<T>::get(network_id, request_hash).ok_or(Error::<T>::UnknownRequest)?;
            ensure!(request_status == RequestStatus::ApprovalsReady, Error::<T>::RequestIsNotReady);
            RequestStatuses::<T>::insert(network_id, request_hash, RequestStatus::Done);
        }

        fn offchain_worker(block_number: T::BlockNumber) {
            debug::debug!("Entering off-chain workers {:?}", block_number);
            if StorageValueRef::persistent(STORAGE_PEER_SECRET_KEY).get::<Vec<u8>>().is_none() {
                debug::debug!("Peer secret key not found. Skipping off-chain procedure.");
                return;
            }

            let mut lock = StorageLock::<'_, Time>::new(b"eth-bridge-ocw::lock");
            let _guard = lock.lock();
            Self::offchain();
        }

        #[weight = <T as Trait>::WeightInfo::force_add_peer()]
        pub fn force_add_peer(origin, who: T::AccountId, address: EthereumAddress, network_id: BridgeNetworkId<T>) {
            let _ = ensure_root(origin)?;
            if !Self::is_peer(&who, network_id) {
                PeerAddress::<T>::insert(network_id, &who, address);
                PeerAccountId::<T>::insert(network_id, &address, who.clone());
                <Peers<T>>::mutate(network_id, |l| l.insert(who));
            }
        }
    }
}

#[cfg_attr(feature = "std", derive(PartialEq, Eq, RuntimeDebug))]
pub enum ContractEvent<AssetId, Address, AccountId, Balance> {
    Withdraw(AssetId, Balance, Address, AccountId),
    Deposit(AccountId, Balance, Address, H256),
    ChangePeers(Address, bool),
    ClaimPswap(AccountId),
    PreparedForMigration,
    Migrated(Address),
}

#[derive(PartialEq)]
pub struct Decoder<T: Trait> {
    tokens: Vec<Token>,
    _phantom: PhantomData<T>,
}

impl<T: Trait> Decoder<T> {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            _phantom: PhantomData,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn next_string(&mut self) -> Result<String, DispatchError> {
        self.tokens
            .pop()
            .and_then(|x| x.into_string())
            .ok_or_else(|| Error::<T>::InvalidString.into())
    }

    pub fn next_bool(&mut self) -> Result<bool, DispatchError> {
        self.tokens
            .pop()
            .and_then(|x| x.into_bool())
            .ok_or_else(|| Error::<T>::InvalidBool.into())
    }

    pub fn next_u8(&mut self) -> Result<u8, DispatchError> {
        self.tokens
            .pop()
            .and_then(|x| x.into_uint())
            .filter(|x| x.as_u32() <= u8::MAX as u32)
            .map(|x| x.as_u32() as u8)
            .ok_or_else(|| Error::<T>::InvalidByte.into())
    }

    pub fn next_address(&mut self) -> Result<Address, DispatchError> {
        Ok(H160(
            self.tokens
                .pop()
                .and_then(|x| x.into_address())
                .ok_or(Error::<T>::InvalidAddress)?
                .0,
        ))
    }

    pub fn next_balance(&mut self) -> Result<Balance, DispatchError> {
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

    pub fn next_amount(&mut self) -> Result<Balance, DispatchError> {
        Ok(u128::try_from(
            self.tokens
                .pop()
                .and_then(|x| x.into_uint())
                .ok_or(Error::<T>::InvalidUint)?,
        )
        .map_err(|_| Error::<T>::InvalidAmount)?)
    }

    pub fn next_account_id(&mut self) -> Result<T::AccountId, DispatchError> {
        Ok(T::AccountId::decode(
            &mut &self
                .tokens
                .pop()
                .and_then(|x| x.into_fixed_bytes())
                .ok_or(Error::<T>::InvalidAccountId)?[..],
        )
        .map_err(|_| Error::<T>::InvalidAccountId)?)
    }

    pub fn next_asset_id(&mut self) -> Result<T::AssetId, DispatchError> {
        Ok(T::AssetId::decode(&mut &self.next_h256()?.0[..])
            .map_err(|_| Error::<T>::InvalidAssetId)?)
    }

    pub fn parse_h256(token: Token) -> Option<H256> {
        <[u8; 32]>::try_from(token.into_fixed_bytes()?)
            .ok()
            .map(H256)
    }

    pub fn next_h256(&mut self) -> Result<H256, DispatchError> {
        self.tokens
            .pop()
            .and_then(Self::parse_h256)
            .ok_or_else(|| Error::<T>::InvalidH256.into())
    }

    pub fn next_array(&mut self) -> Result<Vec<Token>, DispatchError> {
        self.tokens
            .pop()
            .and_then(|x| x.into_array())
            .ok_or_else(|| Error::<T>::Other.into())
    }

    pub fn next_array_map<U, F: FnMut(&mut Decoder<T>) -> Result<U, DispatchError>>(
        &mut self,
        mut f: F,
    ) -> Result<Vec<U>, DispatchError> {
        let mut decoder = Decoder::<T>::new(self.next_array()?);
        iter::repeat(())
            .map(|_| f(&mut decoder))
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn next_signature_params(&mut self) -> Result<Vec<SignatureParams>, DispatchError> {
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

impl<T: Trait> Decoder<T> {
    pub fn write_string(&mut self, val: String) {
        self.tokens.push(Token::String(val));
    }
}

impl<T: Trait> Module<T> {
    fn add_request(mut request: OffchainRequest<T>) -> Result<(), DispatchError> {
        let net_id = request.network_id();
        if let Some(outgoing_req) = request.as_outgoing() {
            ensure!(
                Self::bridge_contract_status(net_id) != BridgeStatus::Migrating
                    || outgoing_req.is_allowed_during_migration(),
                Error::<T>::ContractIsInMigrationStage
            );
        }

        let hash = request.hash();
        ensure!(
            BridgeAccount::<T>::get(net_id).is_some(),
            Error::<T>::UnknownNetwork
        );
        let can_resubmit = RequestStatuses::<T>::get(net_id, &hash)
            .map(|status| status == RequestStatus::Failed)
            .unwrap_or(false);
        // TODO: should we cancel the request on resubmission?
        if !can_resubmit {
            ensure!(
                Request::<T>::get(net_id, &hash).is_none(),
                Error::<T>::DuplicatedRequest
            );
        }
        request.validate()?;
        request.prepare()?;
        AccountRequests::<T>::mutate(&request.author(), |vec| vec.push((net_id, hash)));
        Request::<T>::insert(net_id, &hash, request.clone());
        RequestsQueue::<T>::mutate(net_id, |v| v.push(request));
        RequestStatuses::<T>::insert(net_id, &hash, RequestStatus::Pending);
        let block_number = frame_system::Module::<T>::current_block_number();
        RequestSubmissionHeight::<T>::insert(net_id, &hash, block_number);
        Self::deposit_event(RawEvent::RequestRegistered(hash));
        Ok(())
    }

    fn remove_request_from_queue(network_id: T::NetworkId, hash: &H256) {
        RequestsQueue::<T>::mutate(network_id, |queue| {
            if let Some(pos) = queue.iter().position(|x| x.hash() == *hash) {
                queue.remove(pos);
            }
        });
    }

    fn parse_main_event(
        logs: &[Log],
        kind: IncomingRequestKind,
    ) -> Result<ContractEvent<T::AssetId, Address, T::AccountId, Balance>, DispatchError> {
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
                    if kind == IncomingRequestKind::Transfer =>
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
                    if kind == IncomingRequestKind::AddPeer
                        || kind == IncomingRequestKind::RemovePeer =>
                {
                    let types = [ParamType::Address, ParamType::Bool];
                    let decoded = ethabi::decode(&types, &log.data.0)
                        .map_err(|_| Error::<T>::EthAbiDecodingError)?;
                    let mut decoder = Decoder::<T>::new(decoded);
                    let added = decoder.next_bool()?;
                    let peer_address = decoder.next_address()?;
                    return Ok(ContractEvent::ChangePeers(H160(peer_address.0), added));
                }
                hex!("4eb3aea69bf61684354f60a43d355c3026751ddd0ea4e1f5afc1274b96c65505")
                    if kind == IncomingRequestKind::ClaimPswap =>
                {
                    let types = [ParamType::FixedBytes(32)];
                    let decoded =
                        ethabi::decode(&types, &log.data.0).map_err(|_| Error::<T>::Other)?;
                    let mut decoder = Decoder::<T>::new(decoded);
                    let account_id = decoder.next_account_id()?;
                    return Ok(ContractEvent::ClaimPswap(account_id));
                }
                hex!("5389de9593f75e6515eefa796bd2d3324759f441f2c9b2dcda0efb25190378ff")
                    if kind == IncomingRequestKind::PrepareForMigration =>
                {
                    return Ok(ContractEvent::PreparedForMigration);
                }
                hex!("a2e7361c23d7820040603b83c0cd3f494d377bac69736377d75bb56c651a5098")
                    if kind == IncomingRequestKind::Migrate =>
                {
                    let types = [ParamType::Address];
                    let decoded =
                        ethabi::decode(&types, &log.data.0).map_err(|_| Error::<T>::Other)?;
                    let mut decoder = Decoder::<T>::new(decoded);
                    let account_id = decoder.next_address()?;
                    return Ok(ContractEvent::Migrated(account_id));
                }
                _ => (),
            }
        }
        Err(Error::<T>::UnknownEvent.into())
    }

    fn prepare_message(msg: &[u8]) -> secp256k1::Message {
        let hash = keccak_256(msg);
        let mut prefix = b"\x19Ethereum Signed Message:\n32".to_vec();
        prefix.extend(&hash);
        let hash = keccak_256(&prefix);
        secp256k1::Message::parse_slice(&hash).expect("hash size == 256 bits; qed")
    }

    fn verify_message(
        msg: &[u8],
        signature: &SignatureParams,
        ecdsa_public_key: &ecdsa::Public,
        author: &T::AccountId,
    ) -> bool {
        let message = Self::prepare_message(msg);
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

    fn sign_message(msg: &[u8]) -> (SignatureParams, secp256k1::PublicKey) {
        let secret_s = StorageValueRef::persistent(STORAGE_PEER_SECRET_KEY);
        let sk = secp256k1::SecretKey::parse_slice(
            &secret_s
                .get::<Vec<u8>>()
                .flatten()
                .expect("Off-chain worker secret key is not specified."),
        )
        .expect("Invalid off-chain worker secret key.");
        let message = Self::prepare_message(msg);
        let (sig, v) = secp256k1::sign(&message, &sk);
        let pk = secp256k1::PublicKey::from_secret_key(&sk);
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

    fn handle_pending_incoming_requests(network_id: T::NetworkId, current_eth_height: u64) {
        let s_approved_pending_incoming_requests =
            StorageValueRef::persistent(b"eth-bridge-ocw::approved-pending-incoming-request");
        let mut approved = s_approved_pending_incoming_requests
            .get::<BTreeMap<H256, T::BlockNumber>>()
            .flatten()
            .unwrap_or_default();
        for hash in <Self as Store>::PendingIncomingRequests::get(network_id) {
            let request: IncomingRequest<T> =
                <Self as Store>::IncomingRequests::get(network_id, &hash)
                    .expect("request are never removed; qed");
            let request_submission_height: T::BlockNumber =
                Self::request_submission_height(network_id, &hash);
            let need_to_approve = match approved.get(&hash) {
                Some(height) => &request_submission_height > height,
                None => true,
            };
            let confirmed =
                current_eth_height.saturating_sub(request.at_height()) >= CONFIRMATION_INTERVAL;
            if need_to_approve && confirmed {
                // FIXME: load the transaction again to check if it is presented in the chain.
                let sent = Self::send_incoming_request_result(Ok(request), network_id).is_ok();
                if sent {
                    approved.insert(hash, request_submission_height);
                }
            }
        }
        s_approved_pending_incoming_requests.set(&approved);
    }

    fn load_substrate_finalized_height() -> Result<T::BlockNumber, Error<T>> {
        let hash =
            Self::substrate_json_rpc_request::<_, types::H256>("chain_getFinalizedHead", &())
                .ok_or(Error::<T>::HttpFetchingError)?
                .pop()
                .ok_or(Error::<T>::FailedToLoadFinalizedHead)?;
        let header = Self::substrate_json_rpc_request::<_, types::SubstrateHeaderLimited>(
            "chain_getHeader",
            &[hash],
        )
        .ok_or(Error::<T>::HttpFetchingError)?
        .pop()
        .ok_or(Error::<T>::FailedToLoadBlockHeader)?;
        let number = <T::BlockNumber as From<u32>>::from(header.number.as_u32());
        Ok(number)
    }

    fn handle_parsed_incoming_request_result(
        result: Result<IncomingRequest<T>, DispatchError>,
        timepoint: Timepoint<T>,
        hash: H256,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        match result {
            Ok(incoming_request) => {
                let register_call = Call::<T>::register_incoming_request(incoming_request);
                let call = bridge_multisig::Call::as_multi(
                    get_bridge_account::<T>(network_id),
                    Some(timepoint),
                    <<T as Trait>::Call>::from(register_call).encode(),
                    false,
                    10_000_000_000_000u64,
                );
                Self::send_signed_transaction::<bridge_multisig::Call<T>>(call)
            }
            Err(e) if e == Error::<T>::HttpFetchingError.into() => {
                Err(Error::<T>::HttpFetchingError)
            }
            Err(e) => Self::send_incoming_request_result(Err((hash, timepoint, e)), network_id),
        }
    }

    fn parse_cancel_incoming_request(
        tx_receipt: TransactionReceipt,
        pre_request: IncomingPreRequest<T>,
        pre_request_hash: H256,
    ) -> Result<IncomingRequest<T>, DispatchError> {
        let tx_approved = tx_receipt.is_approved();
        ensure!(!tx_approved, Error::<T>::EthTransactionIsSucceeded);
        let at_height = tx_receipt
            .block_number
            .expect("'block_number' is null only when the log/transaction is pending; qed")
            .as_u64();
        let tx = Self::load_tx(
            H256(tx_receipt.transaction_hash.0),
            pre_request.network_id,
            pre_request.kind,
        )?;
        ensure!(tx.input.0.len() >= 4, Error::<T>::Other);
        let mut method_id = [0u8; 4];
        method_id.clone_from_slice(&tx.input.0[..4]);
        let funs = FUNCTIONS.get_or_init(functions);
        let fun_meta = funs.get(&method_id).ok_or(Error::<T>::UnknownMethodId)?;
        let fun = &fun_meta.function;
        let tokens = fun
            .decode_input(&tx.input.0)
            .map_err(|_| Error::<T>::InvalidFunctionInput)?;
        let hash = parse_hash_from_call::<T>(tokens, fun_meta.tx_hash_arg_pos)?;
        let oc_request: OffchainRequest<T> =
            crate::Request::<T>::get(pre_request.network_id, hash).ok_or(Error::<T>::Other)?;
        let request = match oc_request {
            OffchainRequest::Outgoing(request, _) => request,
            OffchainRequest::Incoming(..) => fail!(Error::<T>::Other),
        };
        ensure!(
            request.author() == &pre_request.author,
            Error::<T>::RequestIsNotOwnedByTheAuthor
        );
        Ok(IncomingRequest::CancelOutgoingRequest(
            IncomingCancelOutgoingRequest {
                request,
                initial_request_hash: pre_request_hash,
                tx_input: tx.input.0,
                tx_hash: pre_request.hash,
                at_height,
                timepoint: pre_request.timepoint,
                network_id: pre_request.network_id,
            },
        ))
    }

    fn handle_mark_as_done_incoming_request(
        pre_request: IncomingPreRequest<T>,
    ) -> Result<(), Error<T>> {
        let is_used = Self::load_is_used(pre_request.hash, pre_request.network_id)?;
        ensure!(is_used, Error::<T>::RequestNotFinalizedOnSidechain);
        let finalize_mark_as_done =
            Call::<T>::finalize_mark_as_done(pre_request.hash, pre_request.network_id);
        let call = bridge_multisig::Call::as_multi(
            get_bridge_account::<T>(pre_request.network_id),
            Some(pre_request.timepoint),
            <<T as Trait>::Call>::from(finalize_mark_as_done).encode(),
            false,
            10_000_000_000_000u64,
        );
        Self::send_signed_transaction::<bridge_multisig::Call<T>>(call)
    }

    fn handle_offchain_request(
        request: OffchainRequest<T>,
        request_hash: H256,
    ) -> Result<(), Error<T>> {
        match request {
            OffchainRequest::Incoming(request) => {
                let network_id = request.network_id;
                let tx_hash = request.hash;
                let timepoint = request.timepoint;
                let kind = request.kind;
                match request.kind {
                    IncomingRequestKind::MarkAsDone => {
                        Self::handle_mark_as_done_incoming_request(request)
                    }
                    IncomingRequestKind::CancelOutgoingRequest => {
                        let result =
                            Self::load_tx_receipt(tx_hash, network_id, kind).and_then(|tx| {
                                Self::parse_cancel_incoming_request(tx, request, request_hash)
                            });
                        Self::handle_parsed_incoming_request_result(
                            result,
                            timepoint,
                            request_hash,
                            network_id,
                        )
                    }
                    _ => {
                        debug::debug!("Loading approved tx {}", tx_hash);
                        let incoming_request_result =
                            Self::load_tx_receipt(tx_hash, network_id, kind)
                                .and_then(|tx| Self::parse_incoming_request(tx, request));
                        Self::handle_parsed_incoming_request_result(
                            incoming_request_result,
                            timepoint,
                            request_hash,
                            network_id,
                        )
                    }
                }
            }
            OffchainRequest::Outgoing(request, _) => Self::handle_outgoing_request(request),
        }
    }

    fn handle_network(network_id: T::NetworkId) {
        let string = format!("eth-bridge-ocw::eth-height-{:?}", network_id);
        let s_eth_height = StorageValueRef::persistent(string.as_bytes());
        let current_eth_height = match Self::load_current_height(network_id) {
            Some(v) => v,
            None => {
                debug::info!(
                    "Failed to load current ethereum height. Skipping off-chain procedure."
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

        for request in <Self as Store>::RequestsQueue::get(network_id) {
            let request_hash = request.hash();
            let request_submission_height: T::BlockNumber =
                Self::request_submission_height(network_id, &request_hash);
            if substrate_finalized_height < request_submission_height {
                continue;
            }
            let need_to_handle = match handled.get(&request_hash) {
                Some(height) => &request_submission_height > height,
                None => true,
            };
            if need_to_handle {
                let error = Self::handle_offchain_request(request, request_hash).err();
                if let Some(e) = error {
                    debug::error!(
                        "An error occurred while processing off-chain request: {:?}",
                        e
                    );
                } else {
                    handled.insert(request_hash, request_submission_height);
                }
            }
        }
        s_handled_requests.set(&handled);

        Self::handle_pending_incoming_requests(network_id, current_eth_height);
    }

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

    fn json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        url: &str,
        id: u64,
        method: &str,
        params: &I,
        headers: &[(&'static str, String)],
    ) -> Option<Vec<O>> {
        let params = match serialize(params) {
            Value::Null => Params::None,
            Value::Array(v) => Params::Array(v),
            Value::Object(v) => Params::Map(v),
            _ => {
                debug::error!("json_rpc_request: got invalid params");
                return None;
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
            .ok()?,
            &headers,
        )
        .and_then(|x| {
            String::from_utf8(x).map_err(|e| {
                debug::error!("json_rpc_request: from utf8 failed, {}", e);
                Error::<T>::HttpFetchingError
            })
        })
        .ok()?;
        let response = rpc::Response::from_json(&raw_response)
            .map_err(|e| {
                debug::error!("json_rpc_request: from_json failed, {}", e);
            })
            .ok()?;
        let results = match response {
            rpc::Response::Batch(xs) => xs,
            rpc::Response::Single(x) => vec![x],
        };
        results
            .into_iter()
            .map(|x| match x {
                rpc::Output::Success(s) => serde_json::from_value(s.result)
                    .map_err(|e| {
                        debug::error!("json_rpc_request: from_value failed, {}", e);
                    })
                    .ok(),
                _ => {
                    debug::error!("json_rpc_request: request failed");
                    None
                }
            })
            .collect()
    }

    fn eth_json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        method: &str,
        params: &I,
        network_id: T::NetworkId,
    ) -> Option<Vec<O>> {
        let string = format!("{}-{:?}", STORAGE_ETH_NODE_PARAMS, network_id);
        let s_node_params = StorageValueRef::persistent(string.as_bytes());
        let node_params = match s_node_params.get::<NodeParams>().flatten() {
            Some(v) => v,
            None => {
                debug::warn!("Failed to make JSON-RPC request, make sure to set node parameters.");
                return None;
            }
        };
        let mut headers: Vec<(_, String)> = vec![("content-type", "application/json".into())];
        if let Some(node_credentials) = node_params.credentials {
            headers.push(("Authorization", node_credentials));
        }
        Self::json_rpc_request(&node_params.url, 1, method, params, &headers)
    }

    fn substrate_json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        method: &str,
        params: &I,
    ) -> Option<Vec<O>> {
        let s_node_url = StorageValueRef::persistent(STORAGE_SUB_NODE_URL_KEY);
        let node_url = s_node_url
            .get::<String>()
            .flatten()
            .unwrap_or_else(|| SUB_NODE_URL.into());
        let headers: Vec<(_, String)> = vec![("content-type", "application/json".into())];

        Self::json_rpc_request(&node_url, 0, method, params, &headers)
    }

    fn send_signed_transaction<LocalCall: Clone + GetCallName>(
        call: LocalCall,
    ) -> Result<(), Error<T>>
    where
        T: CreateSignedTransaction<LocalCall>,
    {
        let signer = Signer::<T, T::PeerId>::any_account();
        if !signer.can_sign() {
            debug::error!("No local account available");
            return Err(<Error<T>>::NoLocalAccountForSigning);
        }
        debug::debug!("Sending signed transaction: {}", call.get_call_name());
        let result = signer.send_signed_transaction(|_acc| call.clone());

        match result {
            Some((_acc, Ok(_))) => {}
            Some((acc, Err(e))) => {
                debug::error!("[{:?}] Failed to send signed transaction: {:?}", acc.id, e);
                return Err(<Error<T>>::FailedToSendSignedTransaction);
            }
            _ => {
                debug::error!("Failed to send signed transaction");
                return Err(<Error<T>>::FailedToSendSignedTransaction);
            }
        };
        Ok(())
    }

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
        let result = contracts
            .into_iter()
            .filter_map(|x| {
                Self::eth_json_rpc_request::<_, bool>(
                    "eth_call",
                    &vec![
                        serialize(&CallRequest {
                            to: Some(x),
                            data: Some(Bytes(data.clone())),
                            ..Default::default()
                        }),
                        Value::String("latest".into()),
                    ],
                    network_id,
                )
                .and_then(|mut xs| xs.pop())
            })
            .any(identity);
        Ok(result)
    }

    fn register_sidechain_asset(
        token_address: Address,
        precision: BalancePrecision,
        symbol: AssetSymbol,
        network_id: T::NetworkId,
    ) -> Result<T::AssetId, DispatchError> {
        ensure!(
            RegisteredSidechainAsset::<T>::get(network_id, &token_address).is_none(),
            Error::<T>::TokenIsAlreadyAdded
        );
        let bridge_account =
            Self::bridge_account(network_id).expect("networks can't be removed; qed");
        let asset_id = assets::Module::<T>::register_from(
            &bridge_account,
            symbol,
            precision,
            Balance::from(0u32),
            true,
        )?;
        RegisteredAsset::<T>::insert(network_id, &asset_id, AssetKind::Sidechain);
        RegisteredSidechainAsset::<T>::insert(network_id, &token_address, asset_id);
        RegisteredSidechainToken::<T>::insert(network_id, &asset_id, token_address);
        let scope = Scope::Unlimited;
        let permission_ids = [MINT];
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

    fn get_asset_by_raw_asset_id(
        raw_asset_id: H256,
        token_address: &Address,
        network_id: T::NetworkId,
    ) -> Result<Option<(T::AssetId, AssetKind)>, DispatchError> {
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
                fail!(Error::<T>::Other);
            }
            Ok(Some((asset_id, AssetKind::Thischain)))
        }
    }

    fn parse_old_incoming_request_method_call(
        incoming_request: IncomingPreRequest<T>,
        tx: Transaction,
    ) -> Result<IncomingRequest<T>, DispatchError> {
        let (fun, arg_pos, tail, added) = if let Some(tail) = strip_prefix(
            &tx.input.0,
            &*ADD_PEER_BY_PEER_ID.get_or_init(init_add_peer_by_peer_fn),
        ) {
            (
                &ADD_PEER_BY_PEER_FN,
                ADD_PEER_BY_PEER_TX_HASH_ARG_POS,
                tail,
                true,
            )
        } else if let Some(tail) = strip_prefix(
            &tx.input.0,
            &*REMOVE_PEER_BY_PEER_ID.get_or_init(init_remove_peer_by_peer_fn),
        ) {
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

        let oc_request: OffchainRequest<T> =
            Request::<T>::get(T::GetEthNetworkId::get(), request_hash)
                .ok_or(Error::<T>::UnknownRequest)?;
        match oc_request {
            OffchainRequest::Outgoing(
                OutgoingRequest::AddPeer(OutgoingAddPeer {
                    peer_address,
                    peer_account_id,
                    ..
                }),
                _,
            )
            | OffchainRequest::Outgoing(
                OutgoingRequest::RemovePeer(OutgoingRemovePeer {
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

    fn parse_incoming_request(
        tx_receipt: TransactionReceipt,
        incoming_pre_request: IncomingPreRequest<T>,
    ) -> Result<IncomingRequest<T>, DispatchError> {
        let tx_approved = tx_receipt.is_approved();
        ensure!(tx_approved, Error::<T>::EthTransactionIsFailed);
        let kind = incoming_pre_request.kind;
        let network_id = incoming_pre_request.network_id;

        // For XOR and VAL contracts compatibility.
        if kind.is_compat() {
            let tx = Self::load_tx(H256(tx_receipt.transaction_hash.0), network_id, kind)?;
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
            tx_receipt,
        )
    }

    fn send_incoming_request_result(
        incoming_request_result: Result<IncomingRequest<T>, (H256, Timepoint<T>, DispatchError)>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        debug::debug!(
            "send_incoming_request_result: {:?}",
            incoming_request_result
        );
        let transfer_call = Call::<T>::finalize_incoming_request(
            incoming_request_result.clone().map_err(|(h, _, e)| (h, e)),
            network_id,
        );
        let timepoint = match &incoming_request_result {
            Ok(r) => r.timepoint(),
            Err((_, t, ..)) => *t,
        };
        let call = bridge_multisig::Call::as_multi(
            Self::bridge_account(network_id).expect("networks can't be removed; qed"),
            Some(timepoint),
            <<T as Trait>::Call>::from(transfer_call).encode(),
            false,
            10_000_000_000_000_000u64,
        );
        Self::send_signed_transaction::<bridge_multisig::Call<T>>(call)?;
        Ok(())
    }

    fn handle_outgoing_request(request: OutgoingRequest<T>) -> Result<(), Error<T>> {
        let signer = Signer::<T, T::PeerId>::any_account();
        if !signer.can_sign() {
            debug::error!("No local account available");
            return Err(<Error<T>>::NoLocalAccountForSigning);
        }
        let encoded_request = request.to_eth_abi(request.hash())?;

        let result = signer.send_signed_transaction(|_acc| {
            // Signs `abi.encodePacked(tokenAddress, amount, to, txHash, from)`.
            let (signature, public) = Self::sign_message(encoded_request.as_raw());
            Call::approve_request(
                ecdsa::Public::from_slice(&public.serialize_compressed()),
                request.clone(),
                encoded_request.clone(),
                signature,
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

    fn load_current_height(network_id: T::NetworkId) -> Option<u64> {
        Self::eth_json_rpc_request::<_, types::U64>("eth_blockNumber", &(), network_id)?
            .first()
            .map(|x| x.as_u64())
    }

    fn ensure_known_contract(
        to: Address,
        network_id: T::NetworkId,
        kind: IncomingRequestKind,
    ) -> DispatchResult {
        match kind {
            IncomingRequestKind::ClaimPswap => {
                ensure!(
                    network_id == T::GetEthNetworkId::get(),
                    Error::<T>::UnknownContractAddress
                );
                ensure!(
                    to == Self::pswap_contract_address(),
                    Error::<T>::UnknownContractAddress
                );
            }
            _ => {
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
            }
        }
        Ok(())
    }

    fn load_tx(
        hash: H256,
        network_id: T::NetworkId,
        kind: IncomingRequestKind,
    ) -> Result<Transaction, DispatchError> {
        let hash = types::H256(hash.0);
        let tx_receipt = Self::eth_json_rpc_request::<_, Transaction>(
            "eth_getTransactionByHash",
            &vec![hash],
            network_id,
        )
        .ok_or(Error::<T>::HttpFetchingError)?
        .pop()
        .ok_or(Error::<T>::FailedToLoadTransaction)?;
        let to = tx_receipt
            .to
            .map(|x| H160(x.0))
            .ok_or(Error::<T>::UnknownContractAddress)?;
        Self::ensure_known_contract(to, network_id, kind)?;
        Ok(tx_receipt)
    }

    // TODO: check if transaction failed due to gas limit
    fn load_tx_receipt(
        hash: H256,
        network_id: T::NetworkId,
        kind: IncomingRequestKind,
    ) -> Result<TransactionReceipt, DispatchError> {
        let hash = types::H256(hash.0);
        let tx_receipt = Self::eth_json_rpc_request::<_, TransactionReceipt>(
            "eth_getTransactionReceipt",
            &vec![hash],
            network_id,
        )
        .ok_or(Error::<T>::HttpFetchingError)?
        .pop()
        .ok_or(Error::<T>::FailedToLoadTransaction)?;
        let to = tx_receipt
            .to
            .map(|x| H160(x.0))
            .ok_or(Error::<T>::UnknownContractAddress)?;
        Self::ensure_known_contract(to, network_id, kind)?;
        Ok(tx_receipt)
    }

    fn is_peer(who: &T::AccountId, network_id: T::NetworkId) -> bool {
        Self::peers(network_id).into_iter().any(|i| i == *who)
    }

    fn ensure_peer(who: &T::AccountId, network_id: T::NetworkId) -> DispatchResult {
        ensure!(Self::is_peer(who, network_id), Error::<T>::Forbidden);
        Ok(())
    }

    fn ensure_bridge_account(
        who: &T::AccountId,
        network_id: T::NetworkId,
    ) -> Result<T::AccountId, DispatchError> {
        let bridge_account_id =
            Self::bridge_account(network_id).ok_or(Error::<T>::UnknownNetwork)?;
        ensure!(who == &bridge_account_id, Error::<T>::Forbidden);
        Ok(bridge_account_id)
    }
}

impl<T: Trait> Module<T> {
    const ITEMS_LIMIT: usize = 50;

    /// Get requests data and their statuses by hash.
    pub fn get_requests(
        hashes: &[H256],
        network_id: Option<T::NetworkId>,
    ) -> Result<Vec<(OffchainRequest<T>, RequestStatus)>, DispatchError> {
        Ok(hashes
            .iter()
            .take(Self::ITEMS_LIMIT)
            .flat_map(|hash| {
                if let Some(net_id) = network_id {
                    Request::<T>::get(net_id, hash)
                        .zip(Self::request_status(net_id, hash))
                        .map(|x| vec![x])
                        .unwrap_or_default()
                } else {
                    Request::<T>::iter()
                        .filter(|(_, h, _)| h == hash)
                        .map(|(net_id, hash, v)| {
                            (
                                v,
                                Self::request_status(net_id, hash)
                                    .unwrap_or(RequestStatus::Pending),
                            )
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
                        let request: OffchainRequest<T> = Request::get(net_id, hash)?;
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
                            OffchainRequest::Incoming(_) => None,
                        }
                    } else {
                        None
                    }
                } else {
                    Some(
                        RequestStatuses::<T>::iter()
                            .filter(|(_, _hash, v)| v == &RequestStatus::ApprovalsReady)
                            .filter_map(|(net_id, hash, _v)| {
                                let request: OffchainRequest<T> = Request::get(net_id, hash)?;
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
                                    OffchainRequest::Incoming(_) => None,
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
    ) -> Result<Vec<(AssetKind, AssetIdOf<T>, Option<H160>)>, DispatchError> {
        Ok(iter_storage::<RegisteredAsset<T>, _, _, _, _, _>(
            network_id,
            |(network_id, asset_id, kind)| {
                let token_addr =
                    RegisteredSidechainToken::<T>::get(network_id, &asset_id).map(|x| H160(x.0));
                (kind, asset_id, token_addr)
            },
        ))
    }
}

pub fn get_bridge_account<T: Trait>(network_id: T::NetworkId) -> T::AccountId {
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

// TODO: remove when `[T]::strip_prefix` will be stabilized.
pub fn strip_prefix<'a, T>(slice: &'a [T], prefix: &'a [T]) -> Option<&'a [T]>
where
    T: PartialEq,
{
    let n = prefix.len();
    if n <= slice.len() {
        let (head, tail) = slice.split_at(n);
        if head == prefix {
            return Some(tail);
        }
    }
    None
}
