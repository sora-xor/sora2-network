//! Durable nonce coordination for funded `keep` offchain workers.

use alloc::vec::Vec;
use codec::{Decode, Encode};
use frame_support::traits::Get;
use frame_system::offchain::{AppCrypto, CreateSignedTransaction, CreateTransactionBase};
use sp_runtime::offchain::{
    storage::StorageValueRef,
    storage_lock::{StorageLock, Time},
    Duration,
};
use sp_runtime::traits::{CheckedAdd, IdentifyAccount, One, UniqueSaturatedInto};
use sp_runtime::RuntimeAppPublic;

const MAX_PENDING: usize = 32;

#[derive(Encode, Decode)]
struct Pending<Nonce> {
    nonce: Nonce,
    operation: [u8; 32],
    call: Vec<u8>,
    encoded: Vec<u8>,
    expires: u64,
    rebroadcast_at: u64,
    spec_version: u32,
    transaction_version: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SubmitStatus {
    Submitted,
    AlreadyPending,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SubmitError {
    NoKey,
    Busy,
    InvalidStorage,
    QueueFull,
    NonceOverflow,
    Signing,
    Pool,
}

/// Share the nonce queue across both keeper pallets and across offchain invocations.
/// The account must be dedicated to these workers, with a funded native balance.
pub fn submit<T, LocalCall, C>(call: LocalCall) -> Result<SubmitStatus, SubmitError>
where
    T: CreateSignedTransaction<LocalCall>
        + CreateTransactionBase<LocalCall, RuntimeCall = <T as frame_system::Config>::RuntimeCall>,
    C: AppCrypto<T::Public, T::Signature>,
    <T as frame_system::Config>::RuntimeCall: From<LocalCall>,
{
    let key = C::RuntimeAppPublic::all()
        .into_iter()
        .next()
        .ok_or(SubmitError::NoKey)?;
    let public: T::Public = C::GenericPublic::from(key).into();
    let account = public.clone().into_account();
    let pending_key = (b"sora/keep/pending/v1", &account).encode();
    let lock_key = (b"sora/keep/lock/v1", &account).encode();
    let mut lock = StorageLock::<Time>::with_deadline(&lock_key, Duration::from_millis(5000));
    let _guard = lock.try_lock().map_err(|_| SubmitError::Busy)?;
    let storage = StorageValueRef::persistent(&pending_key);
    let mut pending = storage
        .get::<Vec<Pending<T::Nonce>>>()
        .map_err(|_| SubmitError::InvalidStorage)?
        .unwrap_or_default();
    if pending.len() > MAX_PENDING {
        return Err(SubmitError::InvalidStorage);
    }

    let now: u64 = frame_system::Pallet::<T>::block_number().unique_saturated_into();
    let chain_nonce = frame_system::Account::<T>::get(&account).nonce;
    pending.retain(|entry| entry.nonce >= chain_nonce);
    pending.sort_by_key(|entry| entry.nonce);
    let current = now.saturating_sub(1);
    let period: u64 = T::BlockHashCount::get().unique_saturated_into();
    let expires = sp_runtime::generic::Era::mortal(period, current).death(current);
    let version = T::Version::get();
    // Reuse exact signed bytes instead of replacing a pending operation with a new nonce.
    for entry in &mut pending {
        if entry.expires <= now
            || entry.spec_version != version.spec_version
            || entry.transaction_version != version.transaction_version
        {
            // Renew the same operation at the same nonce: never leave a nonce gap
            // behind still-live future transactions. Stale operations may fail and
            // pay a fee, but cannot silently replace another keeper operation.
            let call = <T as frame_system::Config>::RuntimeCall::decode(&mut &entry.call[..])
                .map_err(|_| SubmitError::InvalidStorage)?;
            entry.encoded = T::create_signed_transaction::<C>(
                call,
                public.clone(),
                account.clone(),
                entry.nonce,
            )
            .ok_or(SubmitError::Signing)?
            .encode();
            entry.expires = expires;
            entry.rebroadcast_at = now.saturating_sub(1);
            entry.spec_version = version.spec_version;
            entry.transaction_version = version.transaction_version;
        }
        if entry.rebroadcast_at != now {
            let _ = sp_io::offchain::submit_transaction(entry.encoded.clone());
            entry.rebroadcast_at = now;
        }
    }
    storage.set(&pending);

    let call: <T as frame_system::Config>::RuntimeCall = call.into();
    let encoded_call = call.encode();
    let operation = sp_io::hashing::blake2_256(&encoded_call);
    if pending.iter().any(|entry| entry.operation == operation) {
        storage.set(&pending);
        return Ok(SubmitStatus::AlreadyPending);
    }
    if pending.len() >= MAX_PENDING {
        storage.set(&pending);
        return Err(SubmitError::QueueFull);
    }
    let mut nonce = chain_nonce;
    for entry in &pending {
        if entry.nonce == nonce {
            nonce = nonce
                .checked_add(&One::one())
                .ok_or(SubmitError::NonceOverflow)?;
        }
    }
    let transaction = T::create_signed_transaction::<C>(call, public, account, nonce)
        .ok_or(SubmitError::Signing)?;
    let encoded = transaction.encode();
    sp_io::offchain::submit_transaction(encoded.clone()).map_err(|_| SubmitError::Pool)?;
    // This matches the runtime's standard CreateSignedTransaction mortal era.
    pending.push(Pending {
        nonce,
        operation,
        call: encoded_call,
        encoded,
        expires,
        rebroadcast_at: now,
        spec_version: version.spec_version,
        transaction_version: version.transaction_version,
    });
    storage.set(&pending);
    Ok(SubmitStatus::Submitted)
}
