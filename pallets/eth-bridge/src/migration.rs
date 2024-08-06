use crate::requests::RequestStatus;
use codec::Decode;
use codec::Encode;
use frame_support::pallet_prelude::GetStorageVersion;
use frame_support::sp_runtime::legacy::byte_sized_error::DispatchError as OldDispatchError;
use frame_support::sp_runtime::DispatchError;
use frame_support::sp_runtime::ModuleError;
use frame_support::traits::StorageVersion;
use sp_core::RuntimeDebug;

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
        RequestStatuses::<T>::translate::<OldRequestStatus, _>(|_, _, status| {
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

#[cfg(test)]
mod tests {
    use crate::migration::OldRequestStatus;
    use crate::requests::RequestStatus;
    use crate::tests::mock::ExtBuilder;
    use crate::tests::mock::Runtime;
    use crate::Pallet;
    use crate::RequestStatuses;
    use ethereum_types::H256;
    use frame_support::sp_runtime::legacy::byte_sized_error::DispatchError as OldDispatchError;
    use frame_support::sp_runtime::legacy::byte_sized_error::ModuleError as OldModuleError;
    use frame_support::sp_runtime::DispatchError;
    use frame_support::sp_runtime::ModuleError;
    use frame_support::traits::GetStorageVersion;

    #[test]
    fn request_statuses_migration_works() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 0);
            let key = RequestStatuses::<Runtime>::hashed_key_for(0, H256::from_low_u64_be(1));
            frame_support::storage::unhashed::put(&key, &OldRequestStatus::Done);
            let key = RequestStatuses::<Runtime>::hashed_key_for(0, H256::from_low_u64_be(2));
            frame_support::storage::unhashed::put(&key, &OldRequestStatus::ApprovalsReady);
            let key = RequestStatuses::<Runtime>::hashed_key_for(0, H256::from_low_u64_be(3));
            frame_support::storage::unhashed::put(
                &key,
                &OldRequestStatus::Failed(OldDispatchError::Module(OldModuleError {
                    index: 1,
                    error: 2,
                    message: Some("test"),
                })),
            );
            let key = RequestStatuses::<Runtime>::hashed_key_for(0, H256::from_low_u64_be(4));
            frame_support::storage::unhashed::put(
                &key,
                &OldRequestStatus::Broken(
                    OldDispatchError::Module(OldModuleError {
                        index: 3,
                        error: 4,
                        message: Some("test2"),
                    }),
                    OldDispatchError::Module(OldModuleError {
                        index: 5,
                        error: 6,
                        message: Some("test3"),
                    }),
                ),
            );
            super::migrate::<Runtime>();
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 2);
            assert_eq!(
                RequestStatuses::<Runtime>::get(0, H256::from_low_u64_be(1)),
                Some(RequestStatus::Done)
            );
            assert_eq!(
                RequestStatuses::<Runtime>::get(0, H256::from_low_u64_be(2)),
                Some(RequestStatus::ApprovalsReady)
            );
            assert_eq!(
                RequestStatuses::<Runtime>::get(0, H256::from_low_u64_be(3)),
                Some(RequestStatus::Failed(DispatchError::Module(ModuleError {
                    index: 1,
                    error: [2, 0, 0, 0],
                    message: Some("test"),
                })))
            );
            assert_eq!(
                RequestStatuses::<Runtime>::get(0, H256::from_low_u64_be(4)),
                Some(RequestStatus::Broken(
                    DispatchError::Module(ModuleError {
                        index: 3,
                        error: [4, 0, 0, 0],
                        message: Some("test2"),
                    }),
                    DispatchError::Module(ModuleError {
                        index: 5,
                        error: [6, 0, 0, 0],
                        message: Some("test3"),
                    }),
                )),
            );
        });
    }
}
