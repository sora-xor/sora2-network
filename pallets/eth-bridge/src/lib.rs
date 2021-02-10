#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;
extern crate jsonrpc_core as rpc;

use crate::contract::FUNCTIONS;
use crate::types::{Bytes, CallRequest, Log, Transaction, TransactionReceipt};
use alloc::string::String;
use codec::{Decode, Encode};
use common::{prelude::Balance, AssetSymbol, BalancePrecision, Fixed};
use core::{convert::TryFrom, fmt, iter, line, stringify};
use ethabi::{ParamType, Token};
use frame_support::traits::GetCallName;
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
        traits::IdentifyAccount,
        KeyTypeId, MultiSigner,
    },
    weights::{Pays, Weight},
    RuntimeDebug,
};
use frame_system::{
    ensure_root, ensure_signed,
    offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer},
};
use hex_literal::hex;
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
    convert::TryInto,
    fmt::Formatter,
    prelude::*,
};

type Address = H160;
type EthereumAddress = Address;

mod contract;
#[cfg(test)]
mod mock;
pub mod requests;
#[cfg(test)]
mod tests;
pub mod types;

const ETH_NODE_URL: &str = "https://eth-ropsten.s0.dev.soranet.soramitsu.co.jp";
const SUB_NODE_URL: &str = "http://127.0.0.1:9954";
const CONTRACT_ADDRESS: types::H160 = types::H160(hex!("146ba2cdf6bc7df15ffcff2ac1bc83eb33d8197e"));
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 10;

pub fn serialize<T: serde::Serialize>(t: &T) -> rpc::Value {
    serde_json::to_value(t).expect("Types never fail to serialize.")
}

pub fn to_string<T: serde::Serialize>(request: &T) -> String {
    serde_json::to_string(&request).expect("String serialization never fails.")
}

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"bridge";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
pub const TECH_ACCOUNT_AUTHORITY: &[u8] = b"authority";

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ethb");
pub const CONFIRMATION_INTERVAL: u64 = 30;
pub const STORAGE_PEER_SECRET_KEY: &[u8] = b"key";
pub const STORAGE_ETH_NODE_URL_KEY: &[u8] = b"eth-bridge-ocw::eth-node-url";
pub const STORAGE_SUB_NODE_URL_KEY: &[u8] = b"eth-bridge-ocw::sub-node-url";
pub const STORAGE_ETH_NODE_CREDENTIALS_KEY: &[u8] = b"eth-bridge-ocw::eth-node-credentials";

type AssetIdOf<T> = <T as assets::Trait>::AssetId;
type Timepoint<T> = bridge_multisig::Timepoint<<T as frame_system::Trait>::BlockNumber>;

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
    /// 'Add peer' request.
    RemovePeer(OutgoingRemovePeer<T>),
}

impl<T: Trait> OutgoingRequest<T> {
    fn author(&self) -> &T::AccountId {
        match self {
            OutgoingRequest::Transfer(transfer) => &transfer.from,
            OutgoingRequest::AddAsset(request) => &request.author,
            OutgoingRequest::AddToken(request) => &request.author,
            OutgoingRequest::AddPeer(request) => &request.author,
            OutgoingRequest::RemovePeer(request) => &request.author,
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
        }
    }

    fn hash(&self) -> H256 {
        let hash = self.using_encoded(blake2_256);
        H256(hash)
    }

    fn validate(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.validate(),
            OutgoingRequest::AddAsset(request) => request.validate(),
            OutgoingRequest::AddToken(request) => request.validate().map(|_| ()),
            OutgoingRequest::AddPeer(request) => request.validate().map(|_| ()),
            OutgoingRequest::RemovePeer(request) => request.validate().map(|_| ()),
        }
    }

    fn prepare(&mut self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.prepare(),
            OutgoingRequest::AddAsset(request) => request.prepare(()),
            OutgoingRequest::AddToken(request) => request.prepare(()),
            OutgoingRequest::AddPeer(request) => request.prepare(()),
            OutgoingRequest::RemovePeer(request) => request.prepare(()),
        }
    }

    fn finalize(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.finalize(),
            OutgoingRequest::AddAsset(request) => request.finalize(),
            OutgoingRequest::AddToken(request) => request.finalize(),
            OutgoingRequest::AddPeer(request) => request.finalize(),
            OutgoingRequest::RemovePeer(request) => request.finalize(),
        }
    }

    fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::Transfer(request) => request.cancel(),
            OutgoingRequest::AddAsset(request) => request.cancel(),
            OutgoingRequest::AddToken(request) => request.cancel(),
            OutgoingRequest::AddPeer(request) => request.cancel(),
            OutgoingRequest::RemovePeer(request) => request.cancel(),
        }
    }
}

#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum IncomingRequestKind {
    Transfer,
    AddAsset,
    AddPeer,
    RemovePeer,
    ClaimPswap,
    CancelOutgoingRequest,
    MarkAsDone,
}

/// The type of request we can send to the offchain worker
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum IncomingRequest<T: Trait> {
    Transfer(IncomingTransfer<T>),
    AddAsset(IncomingAddToken<T>),
    ChangePeers(IncomingChangePeers<T>),
    ClaimPswap(IncomingClaimPswap<T>),
    CancelOutgoingRequest(IncomingCancelOutgoingRequest<T>),
}

impl<T: Trait> IncomingRequest<T> {
    fn hash(&self) -> H256 {
        match self {
            IncomingRequest::Transfer(request) => request.tx_hash,
            IncomingRequest::AddAsset(request) => request.tx_hash,
            IncomingRequest::ChangePeers(request) => request.tx_hash,
            IncomingRequest::ClaimPswap(request) => request.tx_hash,
            IncomingRequest::CancelOutgoingRequest(request) => request.initial_request_hash,
        }
    }

    fn at_height(&self) -> u64 {
        match self {
            IncomingRequest::Transfer(request) => request.at_height,
            IncomingRequest::AddAsset(request) => request.at_height,
            IncomingRequest::ChangePeers(request) => request.at_height,
            IncomingRequest::ClaimPswap(request) => request.at_height,
            IncomingRequest::CancelOutgoingRequest(request) => request.at_height,
        }
    }

    pub fn prepare(&self) -> Result<(), DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.prepare(),
            IncomingRequest::AddAsset(_request) => Ok(()),
            IncomingRequest::ChangePeers(_request) => Ok(()),
            IncomingRequest::ClaimPswap(_request) => Ok(()),
            IncomingRequest::CancelOutgoingRequest(request) => request.prepare(),
        }
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.cancel(),
            IncomingRequest::AddAsset(_request) => Ok(()),
            IncomingRequest::ChangePeers(_request) => Ok(()),
            IncomingRequest::ClaimPswap(_request) => Ok(()),
            IncomingRequest::CancelOutgoingRequest(_request) => Ok(()),
        }
    }

    pub fn finalize(&self) -> Result<H256, DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.finalize(),
            IncomingRequest::AddAsset(request) => request.finalize(),
            IncomingRequest::ChangePeers(request) => request.finalize(),
            IncomingRequest::ClaimPswap(request) => request.finalize(),
            IncomingRequest::CancelOutgoingRequest(request) => request.finalize(),
        }
    }

    pub fn timepoint(&self) -> Timepoint<T> {
        match self {
            IncomingRequest::Transfer(request) => request.timepoint(),
            IncomingRequest::AddAsset(request) => request.timepoint(),
            IncomingRequest::ChangePeers(request) => request.timepoint(),
            IncomingRequest::ClaimPswap(request) => request.timepoint(),
            IncomingRequest::CancelOutgoingRequest(request) => request.timepoint(),
        }
    }
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub enum OffchainRequest<T: Trait> {
    Outgoing(OutgoingRequest<T>, H256),
    Incoming(T::AccountId, H256, Timepoint<T>, IncomingRequestKind),
}

impl<T: Trait> OffchainRequest<T> {
    pub fn outgoing(request: OutgoingRequest<T>) -> Self {
        let hash = request.hash();
        Self::Outgoing(request, hash)
    }

    fn hash(&self) -> H256 {
        match self {
            OffchainRequest::Outgoing(_request, hash) => *hash,
            OffchainRequest::Incoming(.., IncomingRequestKind::CancelOutgoingRequest)
            | OffchainRequest::Incoming(.., IncomingRequestKind::MarkAsDone) => {
                H256(self.using_encoded(blake2_256))
            }
            OffchainRequest::Incoming(_, hash, ..) => H256(hash.0.clone()),
        }
    }

    fn author(&self) -> &T::AccountId {
        match self {
            OffchainRequest::Outgoing(request, _) => request.author(),
            OffchainRequest::Incoming(author, ..) => author,
        }
    }

    fn prepare(&mut self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.prepare(),
            OffchainRequest::Incoming(..) => Ok(()),
        }
    }

    #[allow(unused)]
    fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.cancel(),
            OffchainRequest::Incoming(..) => Ok(()),
        }
    }

    fn validate(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.validate(),
            OffchainRequest::Incoming(_, hash, .., IncomingRequestKind::MarkAsDone) => {
                let request_status =
                    RequestStatuses::get(hash).ok_or(Error::<T>::UnknownRequest)?;
                ensure!(
                    request_status == RequestStatus::ApprovesReady,
                    Error::<T>::RequestIsNotReady
                );
                Ok(())
            }
            OffchainRequest::Incoming(..) => Ok(()),
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
        }
    }

    pub fn input_tokens(&self, signatures: Option<Vec<SignatureParams>>) -> Vec<Token> {
        match self {
            OutgoingRequestEncoded::Transfer(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::AddAsset(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::AddToken(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::AddPeer(request) => request.input_tokens(signatures),
            OutgoingRequestEncoded::RemovePeer(request) => request.input_tokens(signatures),
        }
    }
}

#[derive(PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum RequestStatus {
    Pending,
    Frozen,
    ApprovesReady,
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

decl_storage! {
    trait Store for Module<T: Trait> as EthBridge {
        pub RequestsQueue get(fn requests_queue): Vec<OffchainRequest<T>>;

        pub IncomingRequests get(fn incoming_requests): map hasher(identity) H256 => Option<IncomingRequest<T>>;
        pub PendingIncomingRequests get(fn pending_incoming_requests): BTreeSet<H256>;

        pub Request get(fn request): map hasher(identity) H256 => Option<OffchainRequest<T>>;
        pub RequestStatuses get(fn request_status): map hasher(identity) H256 => Option<RequestStatus>;
        pub RequestSubmissionHeight get(fn request_submission_height): map hasher(identity) H256 => T::BlockNumber;
        RequestApprovals get(fn approvals): map hasher(identity) H256 => BTreeSet<SignatureParams>;
        AccountRequests get(fn account_requests): map hasher(identity) T::AccountId => Vec<H256>; // TODO: should be a linked-set

        RegisteredAsset get(fn registered_asset): map hasher(identity) T::AssetId => Option<AssetKind>;
        RegisteredSidechainAsset get(fn registered_sidechain_asset): map hasher(identity) Address => Option<T::AssetId>;
        RegisteredSidechainToken get(fn registered_sidechain_token): map hasher(identity) T::AssetId => Option<Address>;

        Peers get(fn peers) config(): BTreeSet<T::AccountId>;
        PendingPeer get(fn pending_peer): Option<T::AccountId>;
        PeerAccountId get(fn peer_account_id): map hasher(identity) Address => T::AccountId;
        PeerAddress get(fn peer_address): map hasher(identity) T::AccountId => Address;

        BridgeAccount get(fn bridge_account) config(): T::AccountId;
        AuthorityAccount get(fn authority_account) config(): T::AccountId;

        // None means the address owns no pswap.
        // 0 means the address claimed them.
        PswapOwners: map hasher(identity) Address => Option<Balance>;
    }
    add_extra_genesis {
        config(tokens): Vec<(T::AssetId, Option<H160>, AssetKind)>;
        config(pswap_owners): Vec<(H160, Balance)>;
        build(|config| {
            for (asset_id, opt_token_address, kind) in &config.tokens {
                if let Some(token_address) = opt_token_address {
                    let token_address = Address::from(token_address.0);
                    RegisteredSidechainAsset::<T>::insert(token_address, *asset_id);
                    RegisteredSidechainToken::<T>::insert(&asset_id, token_address);
                }
                RegisteredAsset::<T>::insert(asset_id, kind);
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

        #[weight = 0]
        pub fn add_asset(
            origin,
            asset_id: AssetIdOf<T>,
            supply: Balance,
        ) {
            debug::debug!("called add_asset");
            let from = ensure_signed(origin)?;
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::AddAsset(OutgoingAddAsset {
                author: from.clone(),
                asset_id,
                supply,
                nonce,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
        }

        #[weight = 0]
        pub fn add_eth_token(
            origin,
            token_address: EthereumAddress,
            ticker: String,
            name: String,
            decimals: u8,
        ) {
            debug::debug!("called add_eth_token");
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
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
        }

        #[weight = 0]
        pub fn transfer_to_sidechain(
            origin,
            asset_id: AssetIdOf<T>,
            to: EthereumAddress,
            amount: Balance
        ) {
            debug::debug!("called transfer_to_sidechain");
            let from = ensure_signed(origin)?;
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer {
                from: from.clone(),
                to,
                asset_id: asset_id.clone(),
                amount,
                nonce,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
        }

        #[weight = 0]
        pub fn request_from_sidechain(origin, eth_tx_hash: H256, kind: IncomingRequestKind) {
            debug::debug!("called request_from_sidechain");
            let from = ensure_signed(origin)?;
            let timepoint = bridge_multisig::Module::<T>::timepoint();
            Self::add_request(OffchainRequest::Incoming(from, eth_tx_hash, timepoint, kind))?;
        }

        #[weight = (0, Pays::No)]
        pub fn finalize_incoming_request(origin, result: Result<IncomingRequest<T>, (H256, DispatchError)>) {
            debug::debug!("called finalize_incoming_request");

            let from = ensure_signed(origin)?;
            let bridge_account_id = Self::bridge_account();
            ensure!(from == bridge_account_id, Error::<T>::Forbidden);

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
                    RequestStatuses::insert(hash, RequestStatus::Done);
                    Self::deposit_event(RawEvent::IncomingRequestFinalized(hash));
                    hash
                }
                Err((hash, e)) => {
                    debug::error!("Incoming request finalization failed {:?} {:?}", hash, e);
                    RequestStatuses::insert(hash, RequestStatus::Failed);
                    Self::deposit_event(RawEvent::IncomingRequestFinalizationFailed(hash));
                    hash
                }
            };
            PendingIncomingRequests::mutate(|set| {
                set.remove(&hash)
            });
            Self::remove_request_from_queue(&hash);
        }

        #[weight = (0, Pays::No)]
        pub fn add_peer(origin, account_id: T::AccountId, address: EthereumAddress) {
            debug::debug!("called change_peers_out");
            let from = ensure_signed(origin.clone())?;
            ensure!(from == Self::authority_account(), Error::<T>::Forbidden);
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::AddPeer(OutgoingAddPeer {
                author: account_id.clone(),
                peer_account_id: account_id,
                peer_address: address,
                nonce,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
        }

        #[weight = (0, Pays::No)]
        pub fn remove_peer(origin, account_id: T::AccountId) {
            debug::debug!("called change_peers_out");
            let from = ensure_signed(origin.clone())?;
            ensure!(from == Self::authority_account(), Error::<T>::Forbidden);
            let peer_address = Self::peer_address(&account_id);
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::RemovePeer(OutgoingRemovePeer {
                author: account_id.clone(),
                peer_account_id: account_id,
                peer_address,
                nonce,
            })))?;
            frame_system::Module::<T>::inc_account_nonce(&from);
        }

        #[weight = (0, Pays::No)]
        pub fn register_incoming_request(origin, incoming_request: IncomingRequest<T>) {
            debug::debug!("called register_incoming_request");
            let author = ensure_signed(origin)?;
            let bridge_account_id = Self::bridge_account();
            ensure!(author == bridge_account_id, Error::<T>::Forbidden);
            let tx_hash = incoming_request.hash();
            ensure!(
                !PendingIncomingRequests::get().contains(&tx_hash),
                Error::<T>::TransferIsAlreadyRegistered
            );
            incoming_request.prepare()?;
            PendingIncomingRequests::mutate(|transfers| transfers.insert(tx_hash.clone()));
            Self::remove_request_from_queue(&tx_hash);
            IncomingRequests::insert(&tx_hash, incoming_request);
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
            Self::ensure_peer(&author)?;
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
            let mut approvals = RequestApprovals::get(&hash);
            let pending_peers_len = if PendingPeer::<T>::get().is_some() {
                1
            } else {
                0
            };
            let need_sigs = majority(Self::peers().len()) + pending_peers_len;
            approvals.insert(signature_params);
            RequestApprovals::insert(&hash, &approvals);
            let current_status = RequestStatuses::get(&hash).unwrap_or(RequestStatus::Pending);
            if current_status == RequestStatus::Pending && approvals.len() == need_sigs {
                if let Err(err) = request.finalize() {
                    debug::error!("Outgoing request finalization failed: {:?}", err);
                    RequestStatuses::insert(hash, RequestStatus::Failed);
                    Self::deposit_event(RawEvent::RequestFinalizationFailed(hash));
                    if let Err(e) = request.cancel() {
                        debug::error!("Request cancellation failed: {:?}, {:?}", e, request)
                    }
                } else {
                    debug::debug!("Outgoing request finalized {:?}", hash);
                    RequestStatuses::insert(hash, RequestStatus::ApprovesReady);
                    Self::deposit_event(RawEvent::ApprovalsCollected(
                        request_encoded,
                        approvals.clone(),
                    ));
                }
                Self::remove_request_from_queue(&hash);
            }
        }

        #[weight = (0, Pays::No)]
        pub fn finalize_mark_as_done(origin, request_hash: H256) {
            debug::debug!("called finalize_mark_as_done");
            let author = ensure_signed(origin)?;
            let bridge_account_id = Self::bridge_account();
            ensure!(author == bridge_account_id, Error::<T>::Forbidden);
            let request_status = RequestStatuses::get(request_hash).ok_or(Error::<T>::UnknownRequest)?;
            ensure!(request_status == RequestStatus::ApprovesReady, Error::<T>::RequestIsNotReady);
            RequestStatuses::insert(request_hash, RequestStatus::Done);
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

        #[weight = 0]
        pub fn force_add_peer(origin, who: T::AccountId) {
            let _ = ensure_root(origin)?;
            if !Self::is_peer(&who) {
                <Peers<T>>::mutate(|l| l.insert(who));
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
            .ok_or(Error::<T>::InvalidString.into())
    }

    pub fn next_bool(&mut self) -> Result<bool, DispatchError> {
        self.tokens
            .pop()
            .and_then(|x| x.into_bool())
            .ok_or(Error::<T>::InvalidBool.into())
    }

    pub fn next_u8(&mut self) -> Result<u8, DispatchError> {
        self.tokens
            .pop()
            .and_then(|x| x.into_uint())
            .filter(|x| x.as_u32() <= u8::MAX as u32)
            .map(|x| x.as_u32() as u8)
            .ok_or(Error::<T>::InvalidByte.into())
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
            .map_err(|_| Error::<T>::InvalidBalance)?, // amount should be of size u128
        ))
    }

    pub fn next_amount(&mut self) -> Result<Balance, DispatchError> {
        Ok(Balance::from(Fixed::from_bits(
            i128::try_from(
                self.tokens
                    .pop()
                    .and_then(|x| x.into_uint())
                    .ok_or(Error::<T>::InvalidUint)?,
            )
            .map_err(|_| Error::<T>::InvalidAmount)?,
        )))
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
            .ok_or(Error::<T>::InvalidH256.into())
    }

    pub fn next_array(&mut self) -> Result<Vec<Token>, DispatchError> {
        self.tokens
            .pop()
            .and_then(|x| x.into_array())
            .ok_or(Error::<T>::Other.into())
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
        let hash = request.hash();

        let can_resubmit = RequestStatuses::get(&hash)
            .map(|status| status == RequestStatus::Failed)
            .unwrap_or(false);
        if !can_resubmit {
            ensure!(
                Request::<T>::get(&hash).is_none(),
                Error::<T>::DuplicatedRequest
            );
        }
        request.validate()?;
        request.prepare()?;
        AccountRequests::<T>::mutate(&request.author(), |vec| vec.push(hash));
        Request::<T>::insert(&hash, request.clone());
        RequestsQueue::<T>::mutate(|v| v.push(request));
        RequestStatuses::insert(&hash, RequestStatus::Pending);
        let block_number = frame_system::Module::<T>::current_block_number();
        RequestSubmissionHeight::<T>::insert(&hash, block_number);
        Self::deposit_event(RawEvent::RequestRegistered(hash));
        Ok(())
    }

    fn remove_request_from_queue(hash: &H256) {
        RequestsQueue::<T>::mutate(|queue| {
            if let Some(pos) = queue.iter().position(|x| x.hash() == *hash) {
                queue.remove(pos);
            }
        });
    }

    fn parse_main_event(
        logs: &[Log],
    ) -> Result<ContractEvent<T::AssetId, Address, T::AccountId, Balance>, DispatchError> {
        for log in logs {
            if log.removed.unwrap_or(false) {
                continue;
            }
            let topic = match log.topics.get(0) {
                Some(x) => &x.0,
                None => continue,
            };
            match topic {
                // Deposit(bytes32,uint256,address,bytes32)
                &hex!("85c0fa492ded927d3acca961da52b0dda1debb06d8c27fe189315f06bb6e26c8") => {
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
                &hex!("a9fac23eb012e72fbd1f453498e7069c380385436763ee2c1c057b170d88d9f9") => {
                    let types = [ParamType::Address, ParamType::Bool];
                    let decoded = ethabi::decode(&types, &log.data.0)
                        .map_err(|_| Error::<T>::EthAbiDecodingError)?;
                    let mut decoder = Decoder::<T>::new(decoded);
                    let added = decoder.next_bool()?;
                    let peer_address = decoder.next_address()?;
                    return Ok(ContractEvent::ChangePeers(H160(peer_address.0), added));
                }
                &hex!("4eb3aea69bf61684354f60a43d355c3026751ddd0ea4e1f5afc1274b96c65505") => {
                    let types = [ParamType::FixedBytes(32)];
                    let decoded =
                        ethabi::decode(&types, &log.data.0).map_err(|_| Error::<T>::Other)?;
                    let mut decoder = Decoder::<T>::new(decoded);
                    let account_id = decoder.next_account_id()?;
                    return Ok(ContractEvent::ClaimPswap(account_id));
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

    fn handle_pending_incoming_requests(current_eth_height: u64) {
        let s_approved_pending_incoming_requests =
            StorageValueRef::persistent(b"eth-bridge-ocw::approved-pending-incoming-request");
        let mut approved = s_approved_pending_incoming_requests
            .get::<BTreeMap<H256, T::BlockNumber>>()
            .flatten()
            .unwrap_or_default();
        for hash in <Self as Store>::PendingIncomingRequests::get() {
            let request: IncomingRequest<T> = <Self as Store>::IncomingRequests::get(&hash)
                .expect("request are never removed; qed");
            let request_submission_height: T::BlockNumber = Self::request_submission_height(&hash);
            let need_to_approve = match approved.get(&hash) {
                Some(height) => &request_submission_height > height,
                None => true,
            };
            let confirmed = current_eth_height >= request.at_height()
                && current_eth_height - request.at_height() >= CONFIRMATION_INTERVAL;
            if need_to_approve && confirmed {
                if Self::send_incoming_request_result(Ok(request)).is_ok() {
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
    ) -> Result<(), Error<T>> {
        match result {
            Ok(incoming_request) => {
                let register_call = Call::<T>::register_incoming_request(incoming_request);
                let call = bridge_multisig::Call::as_multi(
                    Self::bridge_account(),
                    Some(timepoint),
                    <<T as Trait>::Call>::from(register_call).encode(),
                    false,
                    Weight::from(10_000_000_000_000u64),
                );
                Self::send_signed_transaction::<bridge_multisig::Call<T>>(call)
            }
            Err(e) if e == Error::<T>::HttpFetchingError.into() => {
                Err(Error::<T>::HttpFetchingError)
            }
            Err(e) => Self::send_incoming_request_result(Err((hash, timepoint, e.into()))),
        }
    }

    fn parse_cancel_incoming_request(
        tx_receipt: TransactionReceipt,
        author: &T::AccountId,
        request_hash: H256,
        tx_hash: H256,
        timepoint: Timepoint<T>,
    ) -> Result<IncomingRequest<T>, DispatchError> {
        let tx_approved = tx_receipt.is_approved();
        ensure!(!tx_approved, Error::<T>::EthTransactionIsSucceeded);
        let at_height = tx_receipt
            .block_number
            .expect("'block_number' is null only when the log/transaction is pending; qed")
            .as_u64();
        let tx = Self::load_tx(H256(tx_receipt.transaction_hash.0))?;
        let mut method_id = [0u8; 4];
        method_id.clone_from_slice(&tx.input.0[..4]);
        let funs = &*FUNCTIONS;
        let fun_meta = funs.get(&method_id).ok_or(Error::<T>::UnknownMethodId)?;
        let fun = &fun_meta.function;
        let tokens = fun
            .decode_input(&tx.input.0)
            .map_err(|_| Error::<T>::InvalidFunctionInput)?;
        let hash = parse_hash_from_call::<T>(tokens, fun_meta.tx_hash_arg_pos)?;
        let oc_request: OffchainRequest<T> =
            crate::Request::<T>::get(hash).ok_or(Error::<T>::Other)?;
        let request = match oc_request {
            OffchainRequest::Outgoing(request, _) => request,
            OffchainRequest::Incoming(..) => fail!(Error::<T>::Other),
        };
        ensure!(
            request.author() == author,
            Error::<T>::RequestIsNotOwnedByTheAuthor
        );
        Ok(IncomingRequest::CancelOutgoingRequest(
            IncomingCancelOutgoingRequest {
                request,
                initial_request_hash: request_hash,
                tx_input: tx.input.0,
                tx_hash,
                at_height,
                timepoint,
            },
        ))
    }

    fn handle_mark_as_done_incoming_request(
        tx_hash: H256,
        timepoint: Timepoint<T>,
    ) -> Result<(), Error<T>> {
        Self::load_is_used(tx_hash).and_then(|is_used| {
            ensure!(is_used, Error::<T>::RequestNotFinalizedOnSidechain);
            let finalize_mark_as_done = Call::<T>::finalize_mark_as_done(tx_hash);
            let call = bridge_multisig::Call::as_multi(
                Self::bridge_account(),
                Some(timepoint),
                <<T as Trait>::Call>::from(finalize_mark_as_done).encode(),
                false,
                Weight::from(10_000_000_000_000u64),
            );
            Self::send_signed_transaction::<bridge_multisig::Call<T>>(call)
        })
    }

    fn handle_offchain_request(
        request: OffchainRequest<T>,
        request_hash: H256,
    ) -> Result<(), Error<T>> {
        match request {
            OffchainRequest::Incoming(author, tx_hash, timepoint, kind) => match kind {
                IncomingRequestKind::MarkAsDone => {
                    Self::handle_mark_as_done_incoming_request(tx_hash, timepoint)
                }
                IncomingRequestKind::CancelOutgoingRequest => {
                    let result = Self::load_tx_receipt(tx_hash).and_then(|tx| {
                        Self::parse_cancel_incoming_request(
                            tx,
                            &author,
                            request_hash,
                            tx_hash,
                            timepoint,
                        )
                    });
                    Self::handle_parsed_incoming_request_result(result, timepoint, request_hash)
                }
                _ => {
                    debug::debug!("Loading approved tx {}", tx_hash);
                    Self::handle_parsed_incoming_request_result(
                        Self::load_tx_receipt(tx_hash)
                            .and_then(|tx| Self::parse_incoming_request(tx, timepoint)),
                        timepoint,
                        request_hash,
                    )
                }
            },
            OffchainRequest::Outgoing(request, _) => Self::handle_outgoing_request(request),
        }
    }

    fn offchain() {
        let s_eth_height = StorageValueRef::persistent(b"eth-bridge-ocw::eth-height");
        let current_eth_height = match Self::load_current_height() {
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

        let finalized_height = match Self::load_substrate_finalized_height() {
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
        for request in <Self as Store>::RequestsQueue::get() {
            let request_hash = request.hash();
            let request_submission_height: T::BlockNumber =
                Self::request_submission_height(&request_hash);
            if finalized_height < request_submission_height {
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

        Self::handle_pending_incoming_requests(current_eth_height);
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
    ) -> Option<Vec<O>> {
        let s_node_url = StorageValueRef::persistent(STORAGE_ETH_NODE_URL_KEY);
        let node_url = s_node_url
            .get::<String>()
            .flatten()
            .unwrap_or(ETH_NODE_URL.into());
        let mut headers: Vec<(_, String)> = vec![("content-type", "application/json".into())];

        let s_node_credentials = StorageValueRef::persistent(STORAGE_ETH_NODE_CREDENTIALS_KEY);
        let option = s_node_credentials.get::<String>();
        if let Some(node_credentials) = option.flatten() {
            headers.push(("Authorization", node_credentials));
        }
        Self::json_rpc_request(&node_url, 1, method, params, &headers)
    }

    fn substrate_json_rpc_request<I: Serialize, O: for<'de> Deserialize<'de>>(
        method: &str,
        params: &I,
    ) -> Option<Vec<O>> {
        let s_node_url = StorageValueRef::persistent(STORAGE_SUB_NODE_URL_KEY);
        let node_url = s_node_url
            .get::<String>()
            .flatten()
            .unwrap_or(SUB_NODE_URL.into());
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

    fn load_is_used(hash: H256) -> Result<bool, Error<T>> {
        // `used(bytes32)`
        let mut data: Vec<_> = hex!("b07c411f").to_vec();
        data.extend(&hash.0);
        let result = Self::eth_json_rpc_request::<_, bool>(
            "eth_call",
            &vec![
                serialize(&CallRequest {
                    to: Some(CONTRACT_ADDRESS),
                    data: Some(Bytes(data)),
                    ..Default::default()
                }),
                Value::String("latest".into()),
            ],
        )
        .ok_or(Error::<T>::HttpFetchingError)?;
        Ok(result.first().cloned().unwrap_or(false))
    }

    fn register_sidechain_asset(
        token_address: Address,
        precision: BalancePrecision,
        symbol: AssetSymbol,
    ) -> Result<T::AssetId, DispatchError> {
        ensure!(
            RegisteredSidechainAsset::<T>::get(&token_address).is_none(),
            Error::<T>::TokenIsAlreadyAdded
        );
        let asset_id = assets::Module::<T>::register_from(
            &Self::bridge_account(),
            symbol,
            precision,
            Balance::from(0u32),
            true,
        )?;
        RegisteredAsset::<T>::insert(&asset_id, AssetKind::Sidechain);
        RegisteredSidechainAsset::<T>::insert(&token_address, asset_id);
        RegisteredSidechainToken::<T>::insert(&asset_id, token_address);
        Ok(asset_id)
    }

    fn get_asset_by_raw_asset_id(
        raw_asset_id: H256,
        token_address: &Address,
    ) -> Result<Option<(T::AssetId, AssetKind)>, DispatchError> {
        let is_sidechain_token = raw_asset_id == H256::zero();
        if is_sidechain_token {
            let asset_id = match Self::registered_sidechain_asset(&token_address) {
                Some(asset_id) => asset_id,
                _ => {
                    return Ok(None);
                }
            };
            Ok(Some((
                asset_id,
                Self::registered_asset(&asset_id).unwrap_or(AssetKind::Sidechain),
            )))
        } else {
            let asset_id = T::AssetId::from(H256(raw_asset_id.0));
            let asset_kind = Self::registered_asset(&asset_id);
            if asset_kind.is_none() || asset_kind.unwrap() == AssetKind::Sidechain {
                fail!(Error::<T>::Other);
            }
            Ok(Some((asset_id, AssetKind::Thischain)))
        }
    }

    fn parse_incoming_request(
        tx_receipt: TransactionReceipt,
        timepoint: Timepoint<T>,
    ) -> Result<IncomingRequest<T>, DispatchError> {
        let tx_approved = tx_receipt.is_approved();
        ensure!(tx_approved, Error::<T>::EthTransactionIsFailed);
        let at_height = tx_receipt
            .block_number
            .expect("'block_number' is null only when the log/transaction is pending; qed")
            .as_u64();
        let tx_hash = H256(tx_receipt.transaction_hash.0);

        let call = Self::parse_main_event(&tx_receipt.logs)?;

        Ok(match call {
            ContractEvent::Deposit(to, amount, token_address, raw_asset_id) => {
                let (asset_id, asset_kind) =
                    Module::<T>::get_asset_by_raw_asset_id(raw_asset_id, &token_address)?
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
                })
            }
            ContractEvent::ChangePeers(peer_address, added) => {
                let peer_account_id = Self::peer_account_id(&peer_address);
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
                })
            }
            ContractEvent::ClaimPswap(account_id) => {
                let at_height = tx_receipt
                    .block_number
                    .expect("'block_number' is null only when the log is pending; qed")
                    .as_u64();
                let tx_hash = H256(tx_receipt.transaction_hash.0);
                IncomingRequest::ClaimPswap(IncomingClaimPswap {
                    account_id,
                    eth_address: H160(tx_receipt.from.0),
                    tx_hash,
                    at_height,
                    timepoint,
                })
            }
            _ => fail!(Error::<T>::UnknownMethodId),
        })
    }

    fn send_incoming_request_result(
        incoming_request_result: Result<IncomingRequest<T>, (H256, Timepoint<T>, DispatchError)>,
    ) -> Result<(), Error<T>> {
        debug::debug!(
            "send_incoming_request_result: {:?}",
            incoming_request_result
        );
        let transfer_call = Call::<T>::finalize_incoming_request(
            incoming_request_result.clone().map_err(|(h, _, e)| (h, e)),
        );
        let timepoint = match &incoming_request_result {
            Ok(r) => r.timepoint(),
            Err((_, t, ..)) => *t,
        };
        let call = bridge_multisig::Call::as_multi(
            Self::bridge_account(),
            Some(timepoint),
            <<T as Trait>::Call>::from(transfer_call).encode(),
            false,
            Weight::from(10_000_000_000_000_000u64),
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

    fn load_current_height() -> Option<u64> {
        Self::eth_json_rpc_request::<_, types::U64>("eth_blockNumber", &())?
            .first()
            .map(|x| x.as_u64())
    }

    fn load_tx(hash: H256) -> Result<Transaction, DispatchError> {
        let hash = types::H256(hash.0);
        let tx_receipt =
            Self::eth_json_rpc_request::<_, Transaction>("eth_getTransactionByHash", &vec![hash])
                .ok_or(Error::<T>::HttpFetchingError)?
                .pop()
                .ok_or(Error::<T>::FailedToLoadTransaction)?;
        ensure!(
            tx_receipt.to == Some(CONTRACT_ADDRESS),
            Error::<T>::UnknownContractAddress
        );
        Ok(tx_receipt)
    }

    fn load_tx_receipt(hash: H256) -> Result<TransactionReceipt, DispatchError> {
        let hash = types::H256(hash.0);
        let tx_receipt = Self::eth_json_rpc_request::<_, TransactionReceipt>(
            "eth_getTransactionReceipt",
            &vec![hash],
        )
        .ok_or(Error::<T>::HttpFetchingError)?
        .pop()
        .ok_or(Error::<T>::FailedToLoadTransaction)?;
        ensure!(
            tx_receipt.to == Some(CONTRACT_ADDRESS),
            Error::<T>::UnknownContractAddress
        );
        Ok(tx_receipt)
    }

    fn is_peer(who: &T::AccountId) -> bool {
        Self::peers().into_iter().find(|i| i == who).is_some()
    }

    fn ensure_peer(who: &T::AccountId) -> DispatchResult {
        ensure!(Self::is_peer(who), Error::<T>::Forbidden);
        Ok(())
    }
}

impl<T: Trait> Module<T> {
    const ITEMS_LIMIT: usize = 50;

    /// Get requests data and their statuses by hash.
    pub fn get_requests(
        hashes: &[H256],
    ) -> Result<Vec<(OffchainRequest<T>, RequestStatus)>, DispatchError> {
        Ok(hashes
            .iter()
            .take(Self::ITEMS_LIMIT)
            .filter_map(|hash| Request::get(hash).zip(Self::request_status(hash)))
            .collect())
    }

    /// Get approved outgoing requests data and proofs.
    pub fn get_approved_requests(
        hashes: &[H256],
    ) -> Result<Vec<(OutgoingRequestEncoded, Vec<SignatureParams>)>, DispatchError> {
        let items = hashes
            .iter()
            .take(Self::ITEMS_LIMIT)
            .filter_map(|hash| {
                if Self::request_status(hash)? == RequestStatus::ApprovesReady {
                    let request: OffchainRequest<T> = Request::get(hash)?;
                    match request {
                        OffchainRequest::Outgoing(request, hash) => {
                            let encoded = request
                                .to_eth_abi(hash)
                                .expect("this conversion was already tested; qed");
                            Self::get_approvals(&[hash.clone()])
                                .ok()?
                                .pop()
                                .map(|approvals| (encoded, approvals))
                        }
                        OffchainRequest::Incoming(..) => None,
                    }
                } else {
                    None
                }
            })
            .collect();
        Ok(items)
    }

    /// Get requests approvals.
    pub fn get_approvals(hashes: &[H256]) -> Result<Vec<Vec<SignatureParams>>, DispatchError> {
        Ok(hashes
            .iter()
            .take(Self::ITEMS_LIMIT)
            .map(|hash| RequestApprovals::get(hash).into_iter().collect())
            .collect())
    }

    /// Get account requests list.
    pub fn get_account_requests(
        account: &T::AccountId,
        status_filter: Option<RequestStatus>,
    ) -> Result<Vec<H256>, DispatchError> {
        let mut requests: Vec<H256> = Self::account_requests(account);
        if let Some(filter) = status_filter {
            requests.retain(|x| Self::request_status(x).unwrap() == filter)
        }
        Ok(requests)
    }

    /// Get registered assets and tokens.
    pub fn get_registered_assets(
    ) -> Result<Vec<(AssetKind, AssetIdOf<T>, Option<H160>)>, DispatchError> {
        Ok(RegisteredAsset::<T>::iter()
            .map(|(asset_id, kind)| {
                let token_addr = RegisteredSidechainToken::<T>::get(&asset_id).map(|x| H160(x.0));
                (kind, asset_id, token_addr)
            })
            .collect())
    }
}
