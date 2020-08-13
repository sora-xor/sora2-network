//! A demonstration of an offchain worker that sends onchain callbacks

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

#[cfg(test)]
pub mod mock;

use core::{convert::TryInto, fmt};
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, traits::Get,
};
use parity_scale_codec::{Decode, Encode};

use alt_serde::{Deserialize, Deserializer};
use frame_support::dispatch::Weight;
use frame_support::traits::Currency;
use frame_support::traits::ExistenceRequirement;
use frame_system::offchain::{Account, SignMessage, SigningTypes};
use frame_system::{
    self as system, ensure_none, ensure_signed, ensure_root,
    offchain::{
        AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer, SubmitTransaction,
    },
};
use iroha_client_no_std::bridge;
use iroha_client_no_std::account;
use iroha_client_no_std::account::isi::AccountInstruction;
use iroha_client_no_std::asset::isi::AssetInstruction;
use iroha_client_no_std::asset::query::GetAccountAssets;
use iroha_client_no_std::block::{BlockHeader, Message as BlockMessage, ValidBlock};
use iroha_client_no_std::crypto::{PublicKey, Signature, Signatures};
use iroha_client_no_std::isi::prelude::PeerInstruction;
use iroha_client_no_std::peer::PeerId;
use iroha_client_no_std::prelude::*;
use iroha_client_no_std::tx::{Payload, RequestedTransaction};
use sp_core::crypto::KeyTypeId;
use sp_core::ed25519::Signature as SpSignature;
use sp_runtime::offchain::http::Request;
use sp_runtime::traits::{Hash, StaticLookup};
use sp_runtime::{
    offchain as rt_offchain,
    offchain::storage::StorageValueRef,
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
        ValidTransaction,
    },
    MultiSignature,
};
use sp_std::convert::TryFrom;
use sp_std::prelude::*;
use sp_std::str;
use frame_system::RawOrigin;
use sp_runtime::DispatchError;
use sp_core::{crypto::AccountId32, ed25519, sr25519};
use sp_runtime::traits::{IdentifyAccount, Verify};
use iroha_client_no_std::account::query::GetAccount;
use core::{line, stringify};
use iroha_client_no_std::bridge::{BridgeDefinitionId, ExternalTransaction};
use iroha_client_no_std::bridge::asset::ExternalAsset;
use treasury::AssetKind;

/// Defines application identifier for crypto keys of this module.
///
/// Every module that deals with signatures needs to declare its unique identifier for
/// its crypto keys.
/// When offchain worker is signing transactions it's going to request keys of type
/// `KeyTypeId` from the keystore and use the ones it finds to sign the transaction.
/// The keys can be inserted manually via RPC (see `author_insertKey`).
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"demo");
pub const KEY_TYPE_2: KeyTypeId = KeyTypeId(*b"dem0");
pub const NUM_VEC_LEN: usize = 10;

pub const INSTRUCTION_ENDPOINT: &[u8] = b"http://127.0.0.1:7878/instruction";
pub const BLOCK_ENDPOINT: &[u8] = b"http://127.0.0.1:7878/block";
pub const QUERY_ENDPOINT: &[u8] = b"http://127.0.0.1:7878/query";

macro_rules! dbg {
    () => {
        debug::info!("[{}]", $crate::line!());
    };
    ($val:expr) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                debug::info!("[{}] {} = {:#?}",
                    $crate::line!(), $crate::stringify!($val), &tmp);
                tmp
            }
        }
    };
    // Trailing comma with single argument is ignored
    ($val:expr,) => { debug::info!($val) };
    ($($val:expr),+ $(,)?) => {
        ($(debug::info!($val)),+,)
    };
}

/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrappers.
/// We can use from supported crypto kinds (`sr25519`, `ed25519` and `ecdsa`) and augment
/// the types with this pallet-specific identifier.
pub mod crypto {
    use crate::KEY_TYPE;
    use sp_core::ecdsa::Signature as EcdsaSignature;
    use sp_core::ed25519::{Public as EdPublic, Signature as Ed25519Signature};
    use sp_core::sr25519::Signature as Sr25519Signature;

    use sp_runtime::{
        app_crypto::{app_crypto, ecdsa, ed25519, sr25519},
        traits::Verify,
        MultiSignature, MultiSigner,
    };

    app_crypto!(sr25519, KEY_TYPE);

    pub struct TestAuthId;

    // implemented for ocw-runtime
    impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
        type RuntimeAppPublic = Public;
        type GenericSignature = sp_core::sr25519::Signature;
        type GenericPublic = sp_core::sr25519::Public;
    }

    // implemented for mock runtime in test
    // impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
    // for TestAuthId
    // {
    //     type RuntimeAppPublic = Public;
    //     type GenericSignature = sp_core::sr25519::Signature;
    //     type GenericPublic = sp_core::sr25519::Public;
    // }
}

pub mod crypto_ed {
    use crate::KEY_TYPE_2 as KEY_TYPE;
    use sp_core::ed25519::{Public as EdPublic, Signature as Ed25519Signature};

    use sp_runtime::{
        app_crypto::{app_crypto, ed25519},
        traits::Verify,
        MultiSignature, MultiSigner,
    };

    app_crypto!(ed25519, KEY_TYPE);

    pub struct TestAuthId;
    impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
        type RuntimeAppPublic = Public;
        type GenericSignature = sp_core::ed25519::Signature;
        type GenericPublic = sp_core::ed25519::Public;
    }
}

/// This is the pallet's configuration trait
pub trait Trait: system::Trait + treasury::Trait + CreateSignedTransaction<Call<Self>> {
    /// The identifier type for an offchain worker.
    type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
    /// The identifier type for an offchain worker Ed25519 keys.
    type AuthorityIdEd: AppCrypto<Self::Public, Self::Signature>;
    /// The overarching dispatch call type.
    type Call: From<Call<Self>>;
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    /// The type to sign and send transactions.
    type UnsignedPriority: Get<TransactionPriority>;
}

#[derive(Debug)]
enum TransactionType {
    SignedSubmitNumber,
    UnsignedSubmitNumber,
    HttpFetching,
    None,
}

/// The type of requests we can send to the offchain worker
#[cfg_attr(feature = "std", derive(PartialEq, Eq, Debug))]
#[derive(Encode, Decode)]
pub enum OffchainRequest<T: system::Trait + treasury::Trait> {
    OutgoingTransfer(
        treasury::AssetKind,
        u128,
        T::AccountId,
        AccountId,
        u8,
    ),
}

decl_storage! {
    trait Store for Module<T: Trait> as Example {
        /// Requests for off-chain workers made within this block execution
        OcRequests get(fn oc_requests): Vec<OffchainRequest<T>>;
        Authorities get(fn authorities) config(): Vec<T::AccountId>;
        Accounts: map hasher(twox_64_concat) AccountId => T::AccountId;
    }
}

decl_event!(
    /// Events generated by the module.
    pub enum Event<T>
    where
        AccId = <T as system::Trait>::AccountId,
    {
        Ack(u8, AccId),
        IncomingTransfer(AccountId, AccId, AssetKind, u128),
        OutgoingTransfer(AccId, AccountId, AssetKind, u128),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        // Error returned when making signed transactions in off-chain worker
        SignedSubmitNumberError,
        // Error returned when making unsigned transactions in off-chain worker
        UnsignedSubmitNumberError,
        // Error returned when making remote http fetching
        HttpFetchingError,
        // Error returned when gh-info has already been fetched
        AlreadyFetched,
        ReserveCollateralError,
        AccountNotFound,
        InvalidBalanceType,
    }
}

//type BalanceOf<T: Trait> = treasury::BalanceOf::<T>;

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        /// Clean the state on initialisation of a block
        fn on_initialize(_now: T::BlockNumber) -> Weight {
            // At the beginning of each block execution, system triggers all
            // `on_initialize` functions, which allows us to set up some temporary state or - like
            // in this case - clean up other states
            <Self as Store>::OcRequests::kill();
            0
        }

        #[weight = 0]
        pub fn outgoing_transfer(origin, sender: T::AccountId, receiver: AccountId, asset_kind: AssetKind, amount: u128, nonce: u8) -> DispatchResult {
            debug::info!("called outgoing_transfer");
            let author = ensure_signed(origin)?;

            if Self::is_authority(&author) {
                <treasury::Module<T>>::burn(sender.clone(), asset_kind, amount);
                Self::deposit_event(RawEvent::OutgoingTransfer(sender, receiver, asset_kind, amount));
            }

            Ok(())
        }

        #[weight = 0]
        pub fn fetch_blocks_signed(origin) -> DispatchResult {
            debug::info!("called fetch_blocks");
            let who = ensure_signed(origin)?;
            Ok(())
        }

        #[weight = 0]
        pub fn request_transfer(origin, receiver: AccountId, asset_kind: AssetKind, amount: u128, nonce: u8) -> DispatchResult {
            debug::debug!("called request_transfer");
            let sender = ensure_signed(origin)?;

            let mut from = <Self as Store>::Accounts::get(&receiver);
            if from == T::AccountId::default() {
                debug::error!("Account was not found for: {:?}", receiver);
                return Err(<Error<T>>::AccountNotFound.into());
            }
            <treasury::Module<T>>::lock(from.clone(), asset_kind, amount);

            <Self as Store>::OcRequests::mutate(|v| v.push(OffchainRequest::OutgoingTransfer(asset_kind, amount, from, receiver, nonce)));
            Ok(())
        }

        #[weight = 0]
        pub fn incoming_transfer(origin, sender: AccountId, receiver: T::AccountId, asset_kind: treasury::AssetKind, amount: u128) -> DispatchResult {
            debug::debug!("called force_transfer");
            let _ = ensure_signed(origin)?;
            debug::info!("Incoming transfer from {:?} to {:?} with {:?} XOR", sender, receiver, amount);
            if <Accounts<T>>::get(&sender) == T::AccountId::default() {
                <Accounts<T>>::insert(sender.clone(), receiver.clone());
            }
            <treasury::Module<T>>::mint(receiver.clone(), asset_kind, amount);
            Self::deposit_event(RawEvent::IncomingTransfer(sender, receiver, asset_kind, amount));
            Ok(())
        }

        #[weight = 0]
        pub fn add_authority(origin, who: T::AccountId) -> DispatchResult {
            let _ = ensure_root(origin)?;

            if !Self::is_authority(&who) {
                <Authorities<T>>::mutate(|l| l.push(who));
            }

            Ok(())
        }

        fn offchain_worker(block_number: T::BlockNumber) {
            for e in <Self as Store>::OcRequests::get() {
                match e {
                    OffchainRequest::OutgoingTransfer(asset_kind, amount, from, to, nonce) => {
                        let _ = Self::handle_outgoing_transfer(asset_kind, amount, from, to, nonce);
                    }
                }
            }

            // TODO: save parsed height
            if block_number < T::BlockNumber::from(1) {
                debug::info!("Entering off-chain workers");
                match Self::fetch_blocks() {
                    Ok(blocks) => {
                        for block in blocks {
                            Self::handle_block(block);
                        }
                    }
                    Err(e) => { debug::error!("Error: {:?}", e); }
                }
            }
        }
    }
}

fn parity_sig_to_iroha_sig<T: Trait>(
    (pk, sig): (T::Public, <T as SigningTypes>::Signature),
) -> Signature {
    let public_key = PublicKey::try_from(pk.encode()[1..].to_vec()).unwrap();
    let sig_bytes = sig.encode();
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&sig_bytes[1..]);
    Signature {
        public_key,
        signature,
    }
}

fn iroha_sig_to_parity_sig<T: Trait>(
    Signature {
        public_key,
        mut signature,
    }: Signature,
) -> (T::Public, <T as SigningTypes>::Signature) {
    (
        <T::Public>::decode(&mut &(*public_key)[..]).unwrap(),
        <T as SigningTypes>::Signature::decode(&mut &signature[..]).unwrap(),
    )
}

fn substrate_account_id_from_iroha_pk<T: Trait>(
    public_key: &PublicKey
) -> T::AccountId {
    <T::AccountId>::decode(&mut &(*public_key)[..]).unwrap()
}

impl<T: Trait> Module<T> {
    /*
    /// Check if we have fetched github info before. if yes, we use the cached version that is
    ///   stored in off-chain worker storage `storage`. if no, we fetch the remote info and then
    ///   write the info into the storage for future retrieval.
    fn fetch_if_needed() -> Result<(), Error<T>> {
        // Start off by creating a reference to Local Storage value.
        // Since the local storage is common for all offchain workers, it's a good practice
        // to prepend our entry with the pallet name.
        let s_info = StorageValueRef::persistent(b"offchain-demo::gh-info");
        let s_lock = StorageValueRef::persistent(b"offchain-demo::lock");

        // The local storage is persisted and shared between runs of the offchain workers,
        // and offchain workers may run concurrently. We can use the `mutate` function, to
        // write a storage entry in an atomic fashion.
        //
        // It has a similar API as `StorageValue` that offer `get`, `set`, `mutate`.
        // If we are using a get-check-set access pattern, we likely want to use `mutate` to access
        // the storage in one go.
        //
        // Ref: https://substrate.dev/rustdocs/v2.0.0-rc3/sp_runtime/offchain/storage/struct.StorageValueRef.html
        if let Some(Some(gh_info)) = s_info.get::<GithubInfo>() {
            // gh-info has already been fetched. Return early.
            debug::info!("cached gh-info: {:?}", gh_info);
            return Ok(());
        }

        // We are implementing a mutex lock here with `s_lock`
        let res: Result<Result<bool, bool>, Error<T>> = s_lock.mutate(|s: Option<Option<bool>>| {
            match s {
                // `s` can be one of the following:
                //   `None`: the lock has never been set. Treated as the lock is free
                //   `Some(None)`: unexpected case, treated it as AlreadyFetch
                //   `Some(Some(false))`: the lock is free
                //   `Some(Some(true))`: the lock is held

                // If the lock has never been set or is free (false), return true to execute `fetch_n_parse`
                None | Some(Some(false)) => Ok(true),

                // Otherwise, someone already hold the lock (true), we want to skip `fetch_n_parse`.
                // Covering cases: `Some(None)` and `Some(Some(true))`
                _ => Err(<Error<T>>::AlreadyFetched),
            }
        });
        // Cases of `res` returned result:
        //   `Err(<Error<T>>)` - lock is held, so we want to skip `fetch_n_parse` function.
        //   `Ok(Err(true))` - Another ocw is writing to the storage while we set it,
        //                     we also skip `fetch_n_parse` in this case.
        //   `Ok(Ok(true))` - successfully acquire the lock, so we run `fetch_n_parse`
        if let Ok(Ok(true)) = res {
            match Self::fetch_n_parse() {
                Ok(gh_info) => {
                    // set gh-info into the storage and release the lock
                    s_info.set(&gh_info);
                    s_lock.set(&false);

                    debug::info!("fetched gh-info: {:?}", gh_info);
                }
                Err(err) => {
                    // release the lock
                    s_lock.set(&false);
                    return Err(err);
                }
            }
        }
        Ok(())
    }
    */

    fn handle_block(block: ValidBlock) -> Result<(), Error<T>> {
        for tx in block.transactions {
            let author_id = tx.payload.account_id;
            let bridge_account_id = AccountId::new("bridge", "polkadot");
            let root_account_id = AccountId::new("root", "global");
            let xor_asset_def = AssetDefinitionId::new("XOR", "global");
            let xor_asset_id = AssetId::new(xor_asset_def.clone(), root_account_id.clone());
            let dot_asset_def = AssetDefinitionId::new("DOT", "polkadot");
            let dot_asset_id = AssetId::new(dot_asset_def.clone(), bridge_account_id.clone());
            for isi in tx.payload.instructions {
                match isi {
                    Instruction::Account(AccountInstruction::TransferAsset(
                        from,
                        to,
                        mut asset,
                    )) => {
                        debug::info!(
                            "Outgoing {} transfer from {}",
                            asset.id.definition_id.name,
                            from
                        );
                        if to == bridge_account_id {
                            if asset.id.definition_id != xor_asset_def {
                                continue;
                            }
                            use sp_core::crypto::AccountId32;
                            // TODO: create mapping or do a query for the user public key
                            if from == root_account_id {
                                let quantity = asset.quantity;
                                let amount = quantity as u128;
                                    //treasury::BalanceOf::<T>::from(quantity);

                                let signer = Signer::<T, T::AuthorityId>::any_account();
                                if !signer.can_sign() {
                                    debug::error!("No local account available");
                                    return Err(<Error<T>>::SignedSubmitNumberError);
                                }

                                let recipient_account = {
                                    let mut recipient_account = <Self as Store>::Accounts::get(&from);
                                    if recipient_account == T::AccountId::default() {
                                        let account_query = GetAccount::build_request(from.clone());
                                        let query_result = Self::send_query(account_query)?;
                                        debug::trace!("query result: {:?}", query_result);
                                        let queried_acc = match query_result {
                                            QueryResult::GetAccount(res) => res.account,
                                            _ => return Err(<Error<T>>::SignedSubmitNumberError),
                                        };
                                        let account_pk = queried_acc.signatories
                                            .first()
                                            .ok_or(<Error<T>>::SignedSubmitNumberError)?;
                                        let account_id = substrate_account_id_from_iroha_pk::<T>(account_pk);
                                        <Accounts<T>>::insert(from.clone(), account_id.clone());
                                        recipient_account = account_id;
                                    }
                                    recipient_account
                                };

                                let result = signer.send_signed_transaction(|acc| {
                                    debug::info!("signer {:?}", acc.id);
                                    debug::info!("receiver {:?}", recipient_account);

                                    let sender = <<T as frame_system::Trait>::AccountId>::decode(
                                        &mut &([
                                            212u8, 53, 147, 199, 21, 253, 211, 28, 97, 20, 26, 189,
                                            4, 169, 159, 214, 130, 44, 133, 88, 133, 76, 205, 227,
                                            154, 86, 132, 231, 165, 109, 162, 125,
                                        ])[..],
                                    )
                                    .unwrap();
                                    debug::info!("sender {:?}", sender);

                                    debug::info!("recipient_account {:?}", &recipient_account);
                                    let asset_kind = AssetKind::XOR;
                                    // TODO
                                    Call::incoming_transfer(from.clone(), recipient_account.clone(), asset_kind, amount)
                                });

                                match result {
                                    Some((acc, Ok(_))) => {
                                        debug::native::info!(
                                            "off-chain send_signed: acc: {:?}",
                                            acc.id
                                        );

                                        let bridge_def_id = BridgeDefinitionId::new("polkadot");
                                        let asset_definition_id = AssetDefinitionId::new("XOR".into(), "global");
                                        let tx = ExternalTransaction {
                                            hash: "".into(),
                                            payload: vec![]
                                        };
                                        let instructions = vec![
                                            bridge::isi::handle_outgoing_transfer(&bridge_def_id, &asset_definition_id, quantity, 0, &tx),
                                        ];
                                        let resp = Self::send_instructions(instructions)?;
                                        if !resp.is_empty() {
                                            debug::error!("error while processing handle_outgoing_transfer ISI");
                                            // TODO: return err
                                        } else {
                                            debug::error!("ok processing handle_outgoing_transfer ISI"); 
                                        }
                                    }
                                    Some((acc, Err(e))) => {
                                        debug::error!(
                                            "[{:?}] Failed in signed_submit_number: {:?}",
                                            acc.id,
                                            e
                                        );
                                        return Err(<Error<T>>::SignedSubmitNumberError);
                                    }
                                    _ => {
                                        debug::error!("Failed in signed_submit_number");
                                        return Err(<Error<T>>::SignedSubmitNumberError);
                                    }
                                };
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    }

    fn fetch_blocks() -> Result<Vec<ValidBlock>, Error<T>> {
        let remote_url_bytes = BLOCK_ENDPOINT.to_vec();
        let remote_url =
            str::from_utf8(&remote_url_bytes).map_err(|_| <Error<T>>::HttpFetchingError)?;
        let latest_hash = [0; 32];
        let null_pk = PublicKey::try_from(vec![0u8; 32]).unwrap();
        let mut get_blocks = BlockMessage::GetBlocksAfter(latest_hash, PeerId::new("", &null_pk));
        debug::info!("Sending request to: {}", remote_url);
        let request = rt_offchain::http::Request::post(remote_url, vec![get_blocks.encode()]);
        let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(3000));
        let pending = request
            .deadline(timeout)
            .send()
            .map_err(|e| {
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
        let msg = BlockMessage::decode(&mut resp.as_slice()).map_err(|e| {
            debug::error!("Failed to decode BlockMessage: {:?}", e);
            <Error<T>>::HttpFetchingError
        })?;

        let blocks = match msg {
            BlockMessage::LatestBlock(_, _) => {
                debug::error!("Received wrong message: BlockMessage::LatestBlock");
                return Err(<Error<T>>::HttpFetchingError);
            }
            BlockMessage::GetBlocksAfter(_, _) => {
                debug::error!("Received wrong message: BlockMessage::GetBlocksAfter");
                return Err(<Error<T>>::HttpFetchingError);
            }
            BlockMessage::ShareBlocks(blocks, _) => blocks,
        };
        debug::info!("Sending request to: {}", remote_url);
        for block in blocks.clone() {
            for (pk, sig) in block
                .signatures
                .values()
                .iter()
                .cloned()
                .map(iroha_sig_to_parity_sig::<T>)
            {
                let block_hash = T::Hashing::hash(&block.header.encode());
                if !T::AuthorityId::verify(block_hash.as_ref(), pk, sig) {
                    debug::error!("Invalid signature of block: {:?}", block_hash);
                    return Err(<Error<T>>::HttpFetchingError);
                }
            }
        }
        debug::info!("Blocks verified");
        Ok(blocks)
    }

    fn send_instructions(instructions: Vec<Instruction>) -> Result<Vec<u8>, Error<T>> {
        let signer = Signer::<T, T::AuthorityIdEd>::all_accounts();
        if !signer.can_sign() {
            debug::error!("No local account available");
            return Err(<Error<T>>::SignedSubmitNumberError);
        }
        let remote_url_bytes = INSTRUCTION_ENDPOINT.to_vec();
        let remote_url =
            str::from_utf8(&remote_url_bytes).map_err(|_| <Error<T>>::HttpFetchingError)?;
        let mut requested_tx = RequestedTransaction::new(
            instructions,
            account::Id::new("root", "global"),
            10000,
            sp_io::offchain::timestamp().unix_millis(),
        );
        let payload_encoded = requested_tx.payload.encode();
        let sigs = signer.sign_message(&payload_encoded);
        for (acc, sig) in sigs {
            debug::info!("send_instructions acc [{}]: {:?}", acc.index, acc.public);
            if acc.index == 0 {
                let sig = parity_sig_to_iroha_sig::<T>((acc.public, sig));
                requested_tx.signatures.push(sig);
            }
        }
        let tx_encoded = requested_tx.encode();
        let request = rt_offchain::http::Request::post(remote_url, vec![tx_encoded]);
        let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(10000));
        let pending = request
            .deadline(timeout)
            .send()
            .map_err(|e| {
                debug::error!("e1: {:?}", e);
                <Error<T>>::HttpFetchingError
            })?;
        let response = pending
            .try_wait(timeout)
            .map_err(|e| {
                debug::error!("e2: {:?}", e);
                <Error<T>>::HttpFetchingError
            })?
            .map_err(|e| {
                debug::error!("e3: {:?}", e);
                <Error<T>>::HttpFetchingError
            })?;

        if response.code != 200 {
            debug::error!("Unexpected http request status code: {}", response.code);
            return Err(<Error<T>>::HttpFetchingError);
        }

        Ok(response.body().collect::<Vec<u8>>())
    }

    fn send_query(query: QueryRequest) -> Result<QueryResult, Error<T>> {
        let signer = Signer::<T, T::AuthorityId>::all_accounts();
        if !signer.can_sign() {
            debug::error!("No local account available");
            return Err(<Error<T>>::SignedSubmitNumberError);
        }

        let remote_url_bytes = QUERY_ENDPOINT.to_vec();
        let remote_url =
            str::from_utf8(&remote_url_bytes).map_err(|_| <Error<T>>::HttpFetchingError)?;
        debug::info!("Sending query to: {}", remote_url);

        let query_encoded = query.encode();
        let request = rt_offchain::http::Request::post(remote_url, vec![query_encoded]);

        let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(10000));

        let pending = request.deadline(timeout).send().map_err(|e| {
            debug::error!("e1: {:?}", e);
            <Error<T>>::HttpFetchingError
        })?;

        let response = pending
            .try_wait(timeout)
            .map_err(|e| {
                debug::error!("e2: {:?}", e);
                <Error<T>>::HttpFetchingError
            })?
            .map_err(|e| {
                debug::error!("e3: {:?}", e);
                <Error<T>>::HttpFetchingError
            })?;

        if response.code != 200 {
            debug::error!("Unexpected http request status code: {}", response.code);
            return Err(<Error<T>>::HttpFetchingError);
        }

        let bytes = response.body().collect::<Vec<u8>>();
        let query_result = QueryResult::decode(&mut bytes.as_slice()).map_err(|_| <Error<T>>::HttpFetchingError)?;
        Ok(query_result)
    }

    fn handle_outgoing_transfer(
        asset_kind: AssetKind,
        amount: u128,
        from_account_id: T::AccountId,
        to_account_id: AccountId,
        nonce: u8,
    ) -> Result<(), Error<T>> {
        debug::info!("Received transfer request");

        let signer = Signer::<T, T::AuthorityId>::all_accounts();
        if !signer.can_sign() {
            debug::error!("No local account available");
            return Ok(());
        }

        let bridge_def_id = BridgeDefinitionId::new("polkadot");
        let asset_definition_id = AssetDefinitionId::new("XOR".into(), "global");
        let bridge_account_id = AccountId::new("bridge", &bridge_def_id.name);
        let quantity = u32::try_from(amount).map_err(|_| <Error<T>>::InvalidBalanceType)?;

        let instructions = vec![
            bridge::isi::handle_incoming_transfer(&bridge_def_id, &asset_definition_id, quantity, 0, to_account_id.clone(), &ExternalTransaction {
                hash: "".into(),
                payload: vec![]
            }),
        ];
        let resp = Self::send_instructions(instructions)?;
        if !resp.is_empty() {
            debug::error!("error while processing transaction");
            // TODO: return err
        }
        let results = signer.send_signed_transaction(|_acc| {
            Call::outgoing_transfer(from_account_id.clone(), to_account_id.clone(), asset_kind, amount, nonce)
        });

        for (acc, res) in &results {
            match res {
                Ok(()) => {
                    debug::native::info!(
                        "off-chain respond: acc: {:?}| nonce: {}",
                        acc.id,
                        nonce
                    );
                }
                Err(e) => {
                    debug::error!("[{:?}] Failed in respond: {:?}", acc.id, e);
                    return Err(<Error<T>>::SignedSubmitNumberError);
                }
            };
        }
        Ok(())
    }

    fn is_authority(who: &T::AccountId) -> bool {
        Self::authorities().into_iter().find(|i| i == who).is_some()
    }
}
