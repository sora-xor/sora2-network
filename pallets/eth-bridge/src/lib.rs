#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;
extern crate jsonrpc_core as rpc;

use crate::types::{Address, U64};
use alloc::string::String;
use alt_serde::{Deserialize, Serialize};
use codec::{Decode, Encode};
use common::prelude::Balance;
use core::{fmt, line, stringify};
use ethereum_types::H256;
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult},
    ensure, sp_io,
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
    RuntimeDebug,
};
use frame_system::{
    ensure_root, ensure_signed,
    offchain::{AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer},
};
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

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum OffchainRequest<T: Trait> {
    Outgoing(OutgoingRequest<T>, sp_core::H256),
}

impl<T: Trait> OffchainRequest<T> {
    pub fn outgoing(request: OutgoingRequest<T>) -> Self {
        let hash = request.hash();
        Self::Outgoing(request, hash)
    }

    fn hash(&self) -> sp_core::H256 {
        match self {
            OffchainRequest::Outgoing(_request, hash) => *hash,
        }
    }

    fn author(&self) -> &T::AccountId {
        match self {
            OffchainRequest::Outgoing(request, _) => request.author(),
        }
    }

    fn prepare(&mut self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.prepare(),
        }
    }

    fn validate(&self) -> Result<(), DispatchError> {
        match self {
            OffchainRequest::Outgoing(request, _) => request.validate(),
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
    type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
    /// The overarching dispatch call type.
    type Call: From<Call<Self>>;
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub enum AssetKind {
    Thischain,
    Sidechain,
}

decl_storage! {
    trait Store for Module<T: Trait> as EthBridge {
        pub RequestsQueue get(fn oc_requests): Vec<OffchainRequest<T>>;

        pub Requests get(fn requests): map hasher(identity) sp_core::H256 => Option<OffchainRequest<T>>;
        pub RequestStatuses get(fn request_statuses): map hasher(identity) sp_core::H256 => Option<RequestStatus>;
        pub RequestSubmissionHeight get(fn request_submission_height): map hasher(identity) sp_core::H256 => T::BlockNumber;
        RequestApproves get(fn approves): map hasher(identity) sp_core::H256 => BTreeSet<SignatureParams>;
        AccountRequests get(fn account_transfers): map hasher(identity) T::AccountId => Vec<sp_core::H256>; // TODO: maybe should be a 'linked-set'

        RegisteredAssets get(fn registered_asset): map hasher(identity) T::AssetId => Option<AssetKind>;
        RegisteredSidechainAssets get(fn registered_sidechain_asset): map hasher(identity) Address => Option<T::AssetId>;
        RegisteredSidechainTokens get(fn registered_sidechain_token): map hasher(identity) T::AssetId => Option<Address>;

        Authorities get(fn authorities) config(): BTreeSet<T::AccountId>;
        PendingAuthority get(fn pending_authority): Option<T::AccountId>;

        BridgeAccount get(fn bridge_account) config(): T::AccountId;
    }
    add_extra_genesis {
        config(tokens): Vec<(T::AssetId, sp_core::H160)>;
        build(|config| {
            for (asset_id, _token_address) in &config.tokens {
                RegisteredAssets::<T>::insert(asset_id, AssetKind::Thischain);
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
        pub fn approve_request(
            origin,
            ocw_public: ecdsa::Public,
            request: OutgoingRequest<T>,
            request_encoded: OutgoingRequestEncoded,
            signature_params: SignatureParams
        ) {
            debug::debug!("called approve_request");
            let author = ensure_signed(origin)?;
            Self::ensure_authority(&author)?;
            if !Self::verify_message(request_encoded.as_raw(), &signature_params, &ocw_public, &author) {
                // TODO: punish the off-chain worker
                return Err(Error::<T>::InvalidSignature.into());
            }
            debug::info!("Verified request approve {:?}", request_encoded);
            let hash = request.hash();
            RequestApproves::mutate(&hash, |approves| {
                let pending_peers_len = if PendingAuthority::<T>::get().is_some() { 1 } else { 0 };
                let need_sigs = majority(Self::authorities().len()) + pending_peers_len;
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
        pub fn add_authority(origin, who: T::AccountId) {
            let _ = ensure_root(origin)?;
            if !Self::is_authority(&who) {
                <Authorities<T>>::mutate(|l| l.insert(who));
            }
        }
    }
}

#[cfg_attr(feature = "std", derive(PartialEq, Eq, RuntimeDebug))]
pub enum ContractEvent<AssetId, Address, AccountId, Balance> {
    Withdraw(AssetId, Balance, Address, AccountId),
    Deposit(AccountId, Balance, Address, H256),
}

impl<T: Trait> Module<T> {
    fn add_request(mut request: OffchainRequest<T>) -> Result<(), DispatchError> {
        let hash = request.hash();

        let can_resubmit = RequestStatuses::get(&hash)
            .map(|status| status == RequestStatus::Failed)
            .unwrap_or(false);
        if !can_resubmit {
            ensure!(
                Requests::<T>::get(&hash).is_none(),
                Error::<T>::DuplicatedRequest
            );
        }
        request.validate()?;
        request.prepare()?;
        AccountRequests::<T>::mutate(&request.author(), |vec| vec.push(hash));
        Requests::<T>::insert(&hash, request.clone());
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

    fn handle_outgoing_request(request: OutgoingRequest<T>) -> Result<(), Error<T>> {
        let signer = Signer::<T, T::AuthorityId>::any_account();
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

    fn is_authority(who: &T::AccountId) -> bool {
        Self::authorities().into_iter().find(|i| i == who).is_some()
    }

    fn ensure_authority(who: &T::AccountId) -> DispatchResult {
        ensure!(Self::is_authority(who), Error::<T>::Forbidden);
        Ok(())
    }
}
