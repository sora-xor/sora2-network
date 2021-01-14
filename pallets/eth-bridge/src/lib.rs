#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;
extern crate jsonrpc_core as rpc;

use crate::types::{Address, Bytes, CallRequest, Log, TransactionReceipt, U64};
use alloc::string::String;
use alt_serde::{Deserialize, Serialize};
use codec::{Decode, Encode};
use common::{prelude::Balance, AssetSymbol, BalancePrecision};
use core::{convert::TryFrom, fmt, line, stringify};
use ethabi::{ParamType, Token, Uint};
use ethereum_types::H256;
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
    weights::Weight,
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
use serde_json::Value;
use sp_io::hashing::{blake2_256, keccak_256};
use sp_std::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    convert::TryInto,
    fmt::Formatter,
    prelude::*,
};

#[cfg(test)]
mod mock;
mod requests;
#[cfg(test)]
mod tests;
pub mod types;

const URL: &str = "https://parity-testnet-open.s0.dev.soranet.soramitsu.co.jp";

pub fn serialize<T: alt_serde::Serialize>(t: &T) -> rpc::Value {
    serde_json::to_value(t).expect("Types never fail to serialize.")
}

pub fn to_string<T: alt_serde::Serialize>(request: &T) -> String {
    serde_json::to_string(&request).expect("String serialization never fails.")
}

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"bridge";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ethb");
pub const CONFIRMATION_INTERVAL: u64 = 30;

type AssetIdOf<T> = <T as assets::Trait>::AssetId;

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

/// The type of requests we can send to the offchain worker
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum OutgoingRequest<T: Trait> {
    /// Incoming transfer from Substrate to Ethereum request.
    OutgoingTransfer(OutgoingTransfer<T>),
}

impl<T: Trait> OutgoingRequest<T> {
    fn author(&self) -> &T::AccountId {
        match self {
            OutgoingRequest::OutgoingTransfer(transfer) => &transfer.from,
        }
    }

    fn to_eth_abi(&self, tx_hash: sp_core::H256) -> Result<OutgoingRequestEncoded, Error<T>> {
        match self {
            OutgoingRequest::OutgoingTransfer(transfer) => transfer
                .to_eth_abi(tx_hash)
                .map(OutgoingRequestEncoded::OutgoingTransfer),
        }
    }

    fn hash(&self) -> sp_core::H256 {
        let hash = self.using_encoded(blake2_256);
        sp_core::H256(hash)
    }

    fn validate(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::OutgoingTransfer(request) => request.validate(),
        }
    }

    fn prepare(&mut self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::OutgoingTransfer(request) => request.prepare(),
        }
    }

    fn finalize(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::OutgoingTransfer(request) => request.finalize(),
        }
    }

    fn cancel(&self) -> Result<(), DispatchError> {
        match self {
            OutgoingRequest::OutgoingTransfer(request) => request.cancel(),
        }
    }
}

#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, Eq)]
pub enum IncomingRequestKind {
    Transfer,
    AddToken,
    AddPeer,
    RemovePeer,
    ClaimPswap,
}

/// The type of request we can send to the offchain worker
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum IncomingRequest<T: Trait> {
    Transfer(IncomingTransfer<T>),
    ClaimPswap(IncomingClaimPswap<T>),
}

impl<T: Trait> IncomingRequest<T> {
    fn tx_hash(&self) -> &sp_core::H256 {
        match self {
            IncomingRequest::Transfer(request) => &request.tx_hash,
            IncomingRequest::ClaimPswap(request) => &request.tx_hash,
        }
    }

    fn at_height(&self) -> u64 {
        match self {
            IncomingRequest::Transfer(request) => request.at_height,
            IncomingRequest::ClaimPswap(request) => request.at_height,
        }
    }

    pub fn finalize(self) -> Result<sp_core::H256, DispatchError> {
        match self {
            IncomingRequest::Transfer(request) => request.finalize(),
            IncomingRequest::ClaimPswap(request) => request.finalize(),
        }
    }
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum OffchainRequest<T: Trait> {
    Outgoing(OutgoingRequest<T>, sp_core::H256),
    Incoming(T::AccountId, H256, IncomingRequestKind),
}

impl<T: Trait> OffchainRequest<T> {
    pub fn outgoing(request: OutgoingRequest<T>) -> Self {
        let hash = request.hash();
        Self::Outgoing(request, hash)
    }

    fn hash(&self) -> sp_core::H256 {
        match self {
            OffchainRequest::Outgoing(_request, hash) => *hash,
            OffchainRequest::Incoming(_, hash, _) => sp_core::H256(hash.0.clone()),
        }
    }

    fn author(&self) -> &T::AccountId {
        match self {
            OffchainRequest::Outgoing(request, _) => request.author(),
            OffchainRequest::Incoming(author, _, _) => author,
        }
    }

    fn prepare(&mut self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.prepare(),
            OffchainRequest::Incoming(_, _, _) => Ok(()),
        }
    }

    fn validate(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.validate(),
            OffchainRequest::Incoming(_, _, _) => Ok(()),
        }
    }
}

#[derive(Clone, Encode, Decode, RuntimeDebug, PartialEq, Eq)]
pub enum OutgoingRequestEncoded {
    /// ETH-encoded incoming transfer from Substrate to Ethereum request.
    OutgoingTransfer(OutgoingTransferEthEncoded),
}

impl OutgoingRequestEncoded {
    #[allow(unused)]
    fn hash(&self) -> sp_core::H256 {
        let hash = match self {
            OutgoingRequestEncoded::OutgoingTransfer(transfer) => transfer.tx_hash,
        };
        sp_core::H256(hash.0)
    }

    fn as_raw(&self) -> &[u8] {
        match self {
            OutgoingRequestEncoded::OutgoingTransfer(transfer) => &transfer.raw,
        }
    }
}

#[derive(PartialEq, Eq, Encode, Decode, RuntimeDebug)]
pub enum RequestStatus {
    Pending,
    Ready,
    Failed,
}

pub trait Trait:
    frame_system::Trait
    + CreateSignedTransaction<Call<Self>>
    + CreateSignedTransaction<multisig::Call<Self>>
    + assets::Trait
    + multisig::Trait
    + fmt::Debug
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    /// The identifier type for an offchain worker.
    type PeerId: AppCrypto<Self::Public, Self::Signature>;
    /// The overarching dispatch call type.
    type Call: From<Call<Self>>;
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum AssetKind {
    Thischain,
    Sidechain,
    SidechainOwned,
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum IncomingAsset<T: Trait> {
    Loaded(T::AssetId, AssetKind),
    ToRegister(Address, BalancePrecision, AssetSymbol),
}

decl_storage! {
    trait Store for Module<T: Trait> as EthBridge {
        pub RequestsQueue get(fn requests_queue): Vec<OffchainRequest<T>>;

        pub IncomingRequests get(fn incoming_requests): map hasher(identity) sp_core::H256 => Option<IncomingRequest<T>>;
        pub PendingIncomingRequests get(fn pending_incoming_requests): BTreeSet<sp_core::H256>;

        pub Request get(fn request): map hasher(identity) sp_core::H256 => Option<OffchainRequest<T>>;
        pub RequestStatuses get(fn request_status): map hasher(identity) sp_core::H256 => Option<RequestStatus>;
        pub RequestSubmissionHeight get(fn request_submission_height): map hasher(identity) sp_core::H256 => T::BlockNumber;
        RequestApproves get(fn approves): map hasher(identity) sp_core::H256 => BTreeSet<SignatureParams>;
        AccountRequests get(fn account_requests): map hasher(identity) T::AccountId => Vec<sp_core::H256>; // TODO: non-set

        RegisteredAsset get(fn registered_asset): map hasher(identity) T::AssetId => Option<AssetKind>;
        RegisteredSidechainAsset get(fn registered_sidechain_asset): map hasher(identity) Address => Option<T::AssetId>;
        RegisteredSidechainToken get(fn registered_sidechain_token): map hasher(identity) T::AssetId => Option<Address>;

        Peers get(fn peers) config(): BTreeSet<T::AccountId>;
        PendingPeer get(fn pending_peer): Option<T::AccountId>;

        BridgeAccount get(fn bridge_account) config(): T::AccountId;

        // None means the address owns no pswap.
        // 0 means the address claimed them.
        PswapOwners: map hasher(identity) Address => Option<Balance>;
    }
    add_extra_genesis {
        config(tokens): Vec<(T::AssetId, Option<sp_core::H160>, AssetKind)>;
        config(pswap_owners): Vec<(sp_core::H160, Balance)>;
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
        ApprovesCollected(OutgoingRequestEncoded, BTreeSet<SignatureParams>),
        RequestFinalizationFailed(sp_core::H256),
        IncomingRequestFinalizationFailed(sp_core::H256),
        IncomingRequestFinalized(sp_core::H256),
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
        InvalidAmount,
        InvalidAddress,
        InvalidAssetId,
        InvalidAccountId,
        InvalidBool,
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
        EthAbiDecodingError,
        EthTransactionIsPending,
        NoPendingPeer,
        WrongPendingPeer,
        TooManyPendingPeers,
        FailedToGetAssetById,
        CantAddMorePeers,
        CantRemoveMorePeers,
        UnknownPeerId,
        AlreadyClaimed,
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
        pub fn transfer_to_sidechain(
            origin,
            asset_id: AssetIdOf<T>,
            to: Address,
            amount: Balance
        ) {
            debug::debug!("called transfer_to_sidechain");
            let from = ensure_signed(origin)?;
            let nonce = frame_system::Module::<T>::account_nonce(&from);
            Self::add_request(OffchainRequest::outgoing(OutgoingRequest::OutgoingTransfer(OutgoingTransfer {
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
            Self::add_request(OffchainRequest::Incoming(from, eth_tx_hash, kind))?;
        }

        #[weight = 0]
        pub fn finalize_incoming_request(origin, result: Result<IncomingRequest<T>, (sp_core::H256, DispatchError)>) {
            debug::debug!("called finalize_incoming_request");
            let from = ensure_signed(origin)?;
            let bridge_account_id = Self::bridge_account();
            ensure!(from == bridge_account_id, Error::<T>::Forbidden);
            // TODO: emit event
            let result = result.and_then(|req| {
                let hash = sp_core::H256(req.tx_hash().0);
                req.finalize().map_err(|e| (hash, e))
            });
            let hash = match result {
                Ok(hash) => {
                    debug::warn!("Incoming request finalized failed {:?}", hash);
                    RequestStatuses::insert(hash, RequestStatus::Ready);
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

        #[weight = 0]
        pub fn register_incoming_request(origin, incoming_request: IncomingRequest<T>) {
            debug::debug!("called register_incoming_request");
            let author = ensure_signed(origin)?;
            let bridge_account_id = Self::bridge_account();
            ensure!(author == bridge_account_id, Error::<T>::Forbidden);
            let tx_hash = incoming_request.tx_hash();
            ensure!(
                !PendingIncomingRequests::get().contains(&tx_hash),
                Error::<T>::TransferIsAlreadyRegistered
            );
            PendingIncomingRequests::mutate(|transfers| transfers.insert(tx_hash.clone()));
            Self::remove_request_from_queue(&tx_hash);
            IncomingRequests::insert(tx_hash.clone(), incoming_request);
        }

        #[weight = 0]
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
            if !Self::verify_message(request_encoded.as_raw(), &signature_params, &ocw_public, &author) {
                // TODO: punish the off-chain worker
                return Err(Error::<T>::InvalidSignature.into());
            }
            debug::info!("Verified request approve {:?}", request_encoded);
            let hash = request.hash();
            RequestApproves::mutate(&hash, |approves| {
                let pending_peers_len = if PendingPeer::<T>::get().is_some() { 1 } else { 0 };
                let need_sigs = majority(Self::peers().len()) + pending_peers_len;
                approves.insert(signature_params);
                let current_status = RequestStatuses::get(&hash).unwrap_or(RequestStatus::Pending);
                if current_status == RequestStatus::Pending && approves.len() == need_sigs {
                    if let Err(err) = request.finalize() {
                        debug::error!("Outgoing request finalization failed: {:?}", err);
                        RequestStatuses::insert(hash, RequestStatus::Failed);
                        Self::deposit_event(RawEvent::RequestFinalizationFailed(hash));
                        let _res = request.cancel();
                    } else {
                        debug::debug!("Outgoing request finalized {:?}", hash);
                        RequestStatuses::insert(hash, RequestStatus::Ready);
                        Self::deposit_event(RawEvent::ApprovesCollected(request_encoded, approves.clone()));
                    }
                    Self::remove_request_from_queue(&hash);
                }
            });
        }

        fn offchain_worker(block_number: T::BlockNumber) {
            debug::info!("Entering off-chain workers {:?}", block_number);
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
    ClaimPswap(AccountId),
}

fn parse_eth_string(bytes: &[u8]) -> Option<String> {
    Token::to_string(ethabi::decode(&[ParamType::String], bytes).ok()?.pop()?)
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
        Ok(())
    }

    fn remove_request_from_queue(hash: &sp_core::H256) {
        RequestsQueue::<T>::mutate(|queue| {
            if let Some(pos) = queue.iter().position(|x| x.hash() == *hash) {
                queue.remove(pos);
            }
        });
    }

    fn parse_main_event(
        logs: &[Log],
    ) -> Result<ContractEvent<T::AssetId, Address, T::AccountId, Balance>, Error<T>> {
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
                    let mut decoded = ethabi::decode(&types, &log.data.0)
                        .map_err(|_| Error::<T>::EthAbiDecodingError)?;
                    let asset_id = decoded
                        .pop()
                        .and_then(|x| <[u8; 32]>::try_from(x.to_fixed_bytes()?).ok())
                        .map(H256)
                        .ok_or(Error::<T>::InvalidAssetId)?;
                    let token = decoded
                        .pop()
                        .and_then(|x| x.to_address())
                        .ok_or(Error::<T>::InvalidAddress)?;
                    let amount = Balance::from(
                        u128::try_from(
                            decoded
                                .pop()
                                .and_then(|x| x.to_uint())
                                .ok_or(Error::<T>::InvalidAmount)?,
                        )
                        .map_err(|_| Error::<T>::InvalidAmount)?,
                    );
                    let to = T::AccountId::decode(
                        &mut &decoded
                            .pop()
                            .and_then(|x| x.to_fixed_bytes())
                            .ok_or(Error::<T>::InvalidAccountId)?[..],
                    )
                    .map_err(|_| Error::<T>::InvalidAccountId)?;
                    return Ok(ContractEvent::Deposit(to, amount, token, asset_id));
                }
                &hex!("4eb3aea69bf61684354f60a43d355c3026751ddd0ea4e1f5afc1274b96c65505") => {
                    let types = [ParamType::FixedBytes(32)];
                    let mut decoded =
                        ethabi::decode(&types, &log.data.0).map_err(|_| Error::<T>::Other)?;
                    let account_id = T::AccountId::decode(
                        &mut &decoded
                            .pop()
                            .and_then(|x| x.to_fixed_bytes())
                            .ok_or(Error::<T>::InvalidAccountId)?[..],
                    )
                    .map_err(|_| Error::<T>::InvalidAccountId)?;
                    return Ok(ContractEvent::ClaimPswap(account_id));
                }
                _ => (),
            }
        }
        Err(Error::<T>::UnknownEvent)
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

    fn sign_message(msg: &[u8]) -> SignatureParams {
        // TODO: encrypt the key
        let secret_s = StorageValueRef::local(b"key");
        let sk = secp256k1::SecretKey::parse_slice(
            &secret_s
                .get::<Vec<u8>>()
                .flatten()
                .expect("Off-chain worker secret key is not specified."),
        )
        .expect("Invalid off-chain worker secret key.");
        let message = Self::prepare_message(msg);
        let (sig, v) = secp256k1::sign(&message, &sk);
        let v = v.serialize();
        let sig_ser = sig.serialize();
        SignatureParams {
            r: sig_ser[..32].try_into().unwrap(),
            s: sig_ser[32..].try_into().unwrap(),
            v,
        }
    }

    fn handle_pending_incoming_requests(current_eth_height: u64) {
        let s_approved_pending_incoming_requests =
            StorageValueRef::persistent(b"eth-bridge-ocw::approved-pending-incoming-request");
        let mut approved = s_approved_pending_incoming_requests
            .get::<BTreeMap<sp_core::H256, T::BlockNumber>>()
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

    fn offchain() {
        let s_eth_height = StorageValueRef::persistent(b"eth-bridge-ocw::eth-height");
        let current_height = match Self::load_current_height() {
            Some(v) => v,
            None => {
                debug::info!("Failed to load current height. Skipping off-chain procedure.");
                return;
            }
        };
        s_eth_height.set(&current_height);

        let s_handled_requests = StorageValueRef::persistent(b"eth-bridge-ocw::handled-requests");
        let mut handled = s_handled_requests
            .get::<BTreeMap<sp_core::H256, T::BlockNumber>>()
            .flatten()
            .unwrap_or_default();

        for request in <Self as Store>::RequestsQueue::get() {
            let request_hash = request.hash();
            let request_submission_height: T::BlockNumber =
                Self::request_submission_height(&request_hash);
            let need_to_handle = match handled.get(&request_hash) {
                Some(height) => &request_submission_height > height,
                None => true,
            };
            if need_to_handle {
                let error = match request {
                    OffchainRequest::Incoming(_author, tx_hash, _request) => {
                        match Self::load_approved_tx_receipt(tx_hash)
                            .and_then(Self::parse_incoming_request)
                        {
                            Ok(incoming_request) => {
                                let register_call =
                                    Call::<T>::register_incoming_request(incoming_request);
                                let call = multisig::Call::as_multi(
                                    Self::bridge_account(),
                                    None,
                                    register_call.encode(),
                                    false,
                                    Weight::from(1000000u32),
                                );
                                Self::send_signed_transaction::<multisig::Call<T>>(call).err()
                            }
                            Err(e) if e == Error::<T>::HttpFetchingError.into() => {
                                Some(Error::<T>::HttpFetchingError)
                            }
                            Err(e) => Self::send_incoming_request_result(Err((
                                sp_core::H256(tx_hash.0),
                                e.into(),
                            )))
                            .err(),
                        }
                    }
                    OffchainRequest::Outgoing(request, _) => {
                        Self::handle_outgoing_request(request).err()
                    }
                };
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

        Self::handle_pending_incoming_requests(current_height);
    }

    fn http_request(
        url: &str,
        body: Vec<u8>,
        headers: &[(&'static str, &'static str)],
    ) -> Result<Vec<u8>, Error<T>> {
        debug::trace!("Sending request to: {}", url);
        let mut request = rt_offchain::http::Request::post(url, vec![body]);
        let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(10000));
        for (key, value) in headers {
            request = request.add_header(*key, *value);
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
        id: u64,
        method: &str,
        params: &I,
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
            URL,
            serde_json::to_vec(&rpc::Call::MethodCall(rpc::MethodCall {
                jsonrpc: Some(rpc::Version::V2),
                method: method.into(),
                params,
                id: rpc::Id::Num(id as u64),
            }))
            .ok()?,
            &[("content-type", "application/json")],
        )
        .and_then(|x| String::from_utf8(x).map_err(|_| Error::<T>::HttpFetchingError))
        .ok()?;
        let response = rpc::Response::from_json(&raw_response).ok()?;
        let results = match response {
            rpc::Response::Batch(xs) => xs,
            rpc::Response::Single(x) => vec![x],
        };
        results
            .into_iter()
            .map(|x| match x {
                rpc::Output::Success(s) => serde_json::from_value(s.result).ok(),
                _ => {
                    debug::error!("json_rpc_request: request failed");
                    None
                }
            })
            .collect()
    }

    fn send_signed_transaction<LocalCall: Clone>(call: LocalCall) -> Result<(), Error<T>>
    where
        T: CreateSignedTransaction<LocalCall>,
    {
        let signer = Signer::<T, T::PeerId>::any_account();
        if !signer.can_sign() {
            debug::error!("No local account available");
            return Err(<Error<T>>::NoLocalAccountForSigning);
        }
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

    fn load_token_decimals(erc20_address: Address) -> Result<BalancePrecision, Error<T>> {
        let result = Self::json_rpc_request::<_, Uint>(
            1,
            "eth_call",
            &vec![
                serialize(&CallRequest {
                    to: Some(erc20_address),
                    // decimals()
                    data: Some(Bytes(hex!("313ce567").to_vec())),
                    ..Default::default()
                }),
                Value::String("latest".into()),
            ],
        )
        .ok_or(Error::<T>::HttpFetchingError)?;
        Ok(result.first().map(|x| x.byte(0)).unwrap_or(18))
    }

    fn load_token_symbol(erc20_address: Address) -> Result<AssetSymbol, Error<T>> {
        let result = Self::json_rpc_request::<_, Bytes>(
            1,
            "eth_call",
            &vec![
                serialize(&CallRequest {
                    to: Some(erc20_address),
                    // symbol()
                    data: Some(Bytes(hex!("95d89b41").to_vec())),
                    ..Default::default()
                }),
                Value::String("latest".into()),
            ],
        )
        .ok_or(Error::<T>::HttpFetchingError)?;

        Ok(result
            .first()
            .and_then(|x| {
                let symbol = AssetSymbol(parse_eth_string(&x.0)?.into_bytes());
                if !assets::is_symbol_valid(&symbol) {
                    return None;
                }
                Some(symbol)
            })
            .unwrap_or(AssetSymbol(b"BRDGERC".to_vec())))
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
        let asset_id =
            assets::Module::<T>::register_from(&Self::bridge_account(), symbol, precision)?;
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
            let asset_id = T::AssetId::from(sp_core::H256(raw_asset_id.0));
            let asset_kind = Self::registered_asset(&asset_id);
            if asset_kind.is_none() || asset_kind.unwrap() == AssetKind::Sidechain {
                fail!(Error::<T>::Other);
            }
            Ok(Some((asset_id, AssetKind::Thischain)))
        }
    }

    fn load_approved_tx_receipt(tx_hash: H256) -> Result<TransactionReceipt, DispatchError> {
        let tx_receipt = Self::load_tx_receipt(tx_hash)?;
        // TODO: handle `root` field?
        if tx_receipt.status.unwrap_or(0.into()) == 0.into() {
            fail!(Error::<T>::EthTransactionIsPending);
        }
        Ok(tx_receipt)
    }

    fn parse_incoming_request(
        tx_receipt: TransactionReceipt,
    ) -> Result<IncomingRequest<T>, DispatchError> {
        let call = Self::parse_main_event(&tx_receipt.logs)?;
        let at_height = tx_receipt
            .block_number
            .expect("'block_number' is null only when the log/transaction is pending; qed")
            .as_u64();
        let tx_hash = sp_core::H256(tx_receipt.transaction_hash.0);

        Ok(match call {
            ContractEvent::Deposit(to, amount, token_address, raw_asset_id) => {
                let incoming_asset = if let Some((asset_id, asset_kind)) =
                    Module::<T>::get_asset_by_raw_asset_id(raw_asset_id, &token_address)?
                {
                    IncomingAsset::Loaded(asset_id, asset_kind)
                } else {
                    let precision = Self::load_token_decimals(token_address)?;
                    let symbol = Self::load_token_symbol(token_address)?;
                    IncomingAsset::ToRegister(token_address, precision, symbol)
                };
                IncomingRequest::Transfer(IncomingTransfer {
                    from: Default::default(),
                    to,
                    incoming_asset,
                    amount,
                    tx_hash,
                    at_height,
                })
            }
            ContractEvent::ClaimPswap(account_id) => {
                let at_height = tx_receipt
                    .block_number
                    .expect("'block_number' is null only when the log is pending; qed")
                    .as_u64();
                let tx_hash = sp_core::H256(tx_receipt.transaction_hash.0);
                IncomingRequest::ClaimPswap(IncomingClaimPswap {
                    account_id,
                    eth_address: tx_receipt.from,
                    tx_hash,
                    at_height,
                })
            }
            _ => fail!(Error::<T>::UnknownMethodId),
        })
    }

    fn send_incoming_request_result(
        incoming_request_result: Result<IncomingRequest<T>, (sp_core::H256, DispatchError)>,
    ) -> Result<(), Error<T>> {
        let transfer_call = Call::<T>::finalize_incoming_request(incoming_request_result);
        let call = multisig::Call::as_multi(
            Self::bridge_account(),
            None,
            transfer_call.encode(),
            false,
            Weight::from(1000000u32),
        );
        Self::send_signed_transaction::<multisig::Call<T>>(call)?;
        Ok(())
    }

    fn handle_outgoing_request(request: OutgoingRequest<T>) -> Result<(), Error<T>> {
        let signer = Signer::<T, T::PeerId>::any_account();
        if !signer.can_sign() {
            debug::error!("No local account available");
            return Err(<Error<T>>::NoLocalAccountForSigning);
        }
        let encoded_request = request.to_eth_abi(request.hash())?;

        // Signs `abi.encodePacked(tokenAddress, amount, to, txHash, from)`.
        let result = signer.send_signed_transaction(|acc| {
            let signature = Self::sign_message(encoded_request.as_raw());
            Call::approve_request(
                ecdsa::Public::decode(&mut &acc.clone().public.encode()[..]).unwrap(),
                request.clone(),
                encoded_request.clone(),
                signature,
            )
        });

        match result {
            Some((_acc, Ok(_))) => {}
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
        Self::json_rpc_request::<_, U64>(1, "eth_blockNumber", &())?
            .first()
            .map(|x| x.as_u64())
    }

    fn load_tx_receipt(hash: H256) -> Result<TransactionReceipt, Error<T>> {
        Self::json_rpc_request::<_, TransactionReceipt>(2, "eth_getTransactionReceipt", &vec![hash])
            .ok_or(Error::<T>::HttpFetchingError)?
            .pop()
            .ok_or(Error::<T>::FailedToLoadTransaction)
    }

    fn is_peer(who: &T::AccountId) -> bool {
        Self::peers().into_iter().find(|i| i == who).is_some()
    }

    fn ensure_peer(who: &T::AccountId) -> DispatchResult {
        ensure!(Self::is_peer(who), Error::<T>::Forbidden);
        Ok(())
    }
}
