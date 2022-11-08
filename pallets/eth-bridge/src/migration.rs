use crate::requests::RequestStatus;
use codec::Decode;
use codec::Encode;
use frame_support::dispatch::GetStorageVersion;
use frame_support::sp_runtime::legacy::byte_sized_error::DispatchError as OldDispatchError;
use frame_support::sp_runtime::DispatchError;
use frame_support::sp_runtime::ModuleError;
use frame_support::traits::StorageVersion;
use frame_support::RuntimeDebug;

use crate::Config;
use crate::Pallet;
use crate::RequestStatuses;

#[derive(PartialEq, Eq, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
pub enum OldRequestStatus {
    Pending,
    Frozen,
    ApprovalsReady,
    Failed(OldDispatchError),
    Done,
    /// Request is broken. Tried to abort with the first error but got another one when cancelling.
    Broken(OldDispatchError, OldDispatchError),
}

pub fn migrate<T: Config>() {
    if Pallet::<T>::on_chain_storage_version() < StorageVersion::new(2) {
        RequestStatuses::<T>::translate(|_, _, status| {
            let status = match status {
                OldRequestStatus::Pending => RequestStatus::Pending,
                OldRequestStatus::Frozen => RequestStatus::Frozen,
                OldRequestStatus::ApprovalsReady => RequestStatus::ApprovalsReady,
                OldRequestStatus::Failed(err) => RequestStatus::Failed(migrate_error(err)),
                OldRequestStatus::Done => RequestStatus::Done,
                OldRequestStatus::Broken(err1, err2) => {
                    RequestStatus::Broken(migrate_error(err1), migrate_error(err2))
                }
            };
            Some(status)
        });
        StorageVersion::new(2).put::<Pallet<T>>()
    }
}

pub fn migrate_error(err: OldDispatchError) -> DispatchError {
    match err {
        OldDispatchError::Other(s) => DispatchError::Other(s),
        OldDispatchError::CannotLookup => DispatchError::CannotLookup,
        OldDispatchError::BadOrigin => DispatchError::BadOrigin,
        OldDispatchError::Module(err) => DispatchError::Module(ModuleError {
            index: err.index,
            error: [err.error, 0, 0, 0],
            message: err.message,
        }),
        OldDispatchError::ConsumerRemaining => DispatchError::ConsumerRemaining,
        OldDispatchError::NoProviders => DispatchError::NoProviders,
        OldDispatchError::TooManyConsumers => DispatchError::TooManyConsumers,
        OldDispatchError::Token(err) => DispatchError::Token(err),
        OldDispatchError::Arithmetic(err) => DispatchError::Arithmetic(err),
    }
}
