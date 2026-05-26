use crate::requests::RequestStatus;
use codec::Decode;
use codec::Encode;
use frame_support::sp_runtime::legacy::byte_sized_error::DispatchError as OldDispatchError;
use frame_support::sp_runtime::DispatchError;
use frame_support::sp_runtime::ModuleError;
use frame_support::traits::Get;
use frame_support::traits::GetStorageVersion;
use frame_support::traits::StorageVersion;
use frame_support::weights::Weight;
use sp_runtime::RuntimeDebug;

use crate::Config;
use crate::Pallet;
use crate::RequestApprovals;
use crate::RequestApprovers;
use crate::RequestStatuses;
use crate::Requests;
use crate::{
    BridgeAccount, DeprecatedSidechainTokens, LegacyEthereumXorDecommissioned, RegisteredAsset,
    RegisteredSidechainAsset, RegisteredSidechainToken, SidechainAssetPrecision,
    LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
};
use crate::{Error, RequestsQueue};
use crate::{IncomingRequest, OffchainRequest, OutgoingRequest};
use common::prelude::Balance;
use common::AssetInfoProvider;
use sp_std::vec::Vec;

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

pub fn migrate<T: Config>() -> Weight {
    let mut reads = 0u64;
    let mut writes = 0u64;

    if Pallet::<T>::on_chain_storage_version() < StorageVersion::new(2) {
        reads = reads.saturating_add(1);
        let mut translated = 0u64;
        RequestStatuses::<T>::translate::<OldRequestStatus, _>(|_, _, status| {
            translated = translated.saturating_add(1);
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
        reads = reads.saturating_add(translated);
        writes = writes.saturating_add(translated);
        StorageVersion::new(2).put::<Pallet<T>>();
        writes = writes.saturating_add(1);
    }

    T::DbWeight::get().reads_writes(reads, writes)
}

pub fn decommission_legacy_ethereum_xor<T: Config>() -> Weight {
    match common::with_transaction(decommission_legacy_ethereum_xor_inner::<T>) {
        Ok(weight) => weight,
        Err(error) => {
            frame_support::__private::log::error!(
                "Legacy Ethereum XOR decommission failed and was rolled back: {:?}",
                error
            );
            panic!(
                "Legacy Ethereum XOR decommission failed and was rolled back: {:?}",
                error
            );
        }
    }
}

fn decommission_legacy_ethereum_xor_inner<T: Config>() -> Result<Weight, DispatchError> {
    let mut reads = 0u64;
    let mut writes = 0u64;

    reads = reads.saturating_add(1);
    if LegacyEthereumXorDecommissioned::<T>::get() {
        return Ok(T::DbWeight::get().reads(reads));
    }

    let network_id = T::GetEthNetworkId::get();
    let xor_asset_id = common::XOR.into();
    let reason: DispatchError = Error::<T>::DeprecatedLegacyXor.into();
    reads = reads.saturating_add(1);
    let queue = RequestsQueue::<T>::get(network_id);
    let (blockers, blocker_reads) =
        legacy_ethereum_xor_decommission_blocker_count_with_queue::<T>(network_id, &queue);
    reads = reads.saturating_add(blocker_reads);
    if blockers != 0 {
        frame_support::__private::log::error!(
            "Refusing to decommission legacy Ethereum XOR: {blockers} unsafe outgoing transfer requests remain"
        );
        return Err(DispatchError::Other(
            "legacy Ethereum XOR decommission blocked",
        ));
    }

    DeprecatedSidechainTokens::<T>::insert(network_id, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS, true);
    writes = writes.saturating_add(1);

    let mut retained_queue = Vec::with_capacity(queue.len());
    let mut queue_changed = false;
    for hash in queue {
        reads = reads.saturating_add(1);
        let Some(request) = Requests::<T>::get(network_id, hash) else {
            retained_queue.push(hash);
            continue;
        };
        if !is_legacy_xor_request::<T>(network_id, &request) {
            retained_queue.push(hash);
            continue;
        }

        queue_changed = true;
        reads = reads.saturating_add(1);
        match RequestStatuses::<T>::get(network_id, hash) {
            Some(RequestStatus::Pending) => {
                let new_status = match request.cancel() {
                    Ok(()) => RequestStatus::Failed(reason),
                    Err(cancel_error) => RequestStatus::Broken(reason, cancel_error),
                };
                RequestStatuses::<T>::insert(network_id, hash, new_status);
                writes = writes.saturating_add(1);
            }
            Some(
                RequestStatus::ApprovalsReady | RequestStatus::Frozen | RequestStatus::Broken(_, _),
            ) => {
                RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Failed(reason));
                writes = writes.saturating_add(1);
            }
            Some(RequestStatus::Failed(_) | RequestStatus::Done) => {}
            None => {
                RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Failed(reason));
                writes = writes.saturating_add(1);
            }
        }

        RequestApprovals::<T>::remove(network_id, hash);
        RequestApprovers::<T>::remove(network_id, hash);
        writes = writes.saturating_add(2);
    }
    if queue_changed {
        RequestsQueue::<T>::insert(network_id, retained_queue);
        writes = writes.saturating_add(1);
    }

    RegisteredAsset::<T>::remove(network_id, &xor_asset_id);
    SidechainAssetPrecision::<T>::remove(network_id, &xor_asset_id);
    if let Some(token_address) = RegisteredSidechainToken::<T>::get(network_id, &xor_asset_id) {
        RegisteredSidechainAsset::<T>::remove(network_id, token_address);
        writes = writes.saturating_add(1);
    }
    RegisteredSidechainAsset::<T>::remove(network_id, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS);
    RegisteredSidechainToken::<T>::remove(network_id, &xor_asset_id);
    writes = writes.saturating_add(4);

    reads = reads.saturating_add(1);
    let bridge_account = BridgeAccount::<T>::get(network_id).ok_or(Error::<T>::UnknownNetwork)?;
    let total_balance = assets::Pallet::<T>::total_balance(&xor_asset_id, &bridge_account)?;
    reads = reads.saturating_add(1);
    if total_balance > Balance::from(0u32) {
        let _ = assets::Pallet::<T>::unreserve(&xor_asset_id, &bridge_account, total_balance)?;
        assets::Pallet::<T>::burn_from(
            &xor_asset_id,
            &bridge_account,
            &bridge_account,
            total_balance,
        )?;
        writes = writes.saturating_add(2);

        let remaining_balance = assets::Pallet::<T>::total_balance(&xor_asset_id, &bridge_account)?;
        reads = reads.saturating_add(1);
        if remaining_balance != Balance::from(0u32) {
            return Err(DispatchError::Other(
                "legacy Ethereum XOR bridge balance remains",
            ));
        }
    }

    LegacyEthereumXorDecommissioned::<T>::put(true);
    writes = writes.saturating_add(1);

    Ok(T::DbWeight::get().reads_writes(reads, writes))
}

pub fn legacy_ethereum_xor_decommission_blockers<T: Config>() -> u32 {
    legacy_ethereum_xor_decommission_blocker_count::<T>().0
}

pub fn is_legacy_ethereum_xor_decommissioned<T: Config>() -> bool {
    LegacyEthereumXorDecommissioned::<T>::get()
}

fn legacy_ethereum_xor_decommission_blocker_count<T: Config>() -> (u32, u64) {
    let network_id = T::GetEthNetworkId::get();
    let queue = RequestsQueue::<T>::get(network_id);
    let (blockers, reads) =
        legacy_ethereum_xor_decommission_blocker_count_with_queue::<T>(network_id, &queue);
    (blockers, reads.saturating_add(1))
}

fn legacy_ethereum_xor_decommission_blocker_count_with_queue<T: Config>(
    network_id: T::NetworkId,
    queue: &[sp_core::H256],
) -> (u32, u64) {
    let mut blockers = 0u32;
    let mut reads = 0u64;

    for hash in queue {
        reads = reads.saturating_add(1);
        let Some(request) = Requests::<T>::get(network_id, hash) else {
            continue;
        };
        if !is_legacy_xor_outgoing_transfer::<T>(network_id, &request) {
            continue;
        }
        reads = reads.saturating_add(1);
        if is_unsafe_legacy_xor_outgoing_transfer_status::<T>(network_id, *hash) {
            blockers = blockers.saturating_add(1);
        }
    }

    (blockers, reads)
}

fn is_unsafe_legacy_xor_outgoing_transfer_status<T: Config>(
    network_id: T::NetworkId,
    hash: sp_core::H256,
) -> bool {
    matches!(
        RequestStatuses::<T>::get(network_id, hash),
        Some(RequestStatus::ApprovalsReady | RequestStatus::Frozen | RequestStatus::Broken(_, _))
            | None
    )
}

fn is_legacy_xor_outgoing_transfer<T: Config>(
    network_id: T::NetworkId,
    request: &OffchainRequest<T>,
) -> bool {
    if network_id != T::GetEthNetworkId::get() {
        return false;
    }
    matches!(
        request,
        OffchainRequest::Outgoing(OutgoingRequest::Transfer(request), _)
            if request.asset_id == common::XOR.into()
    )
}

fn is_legacy_xor_request<T: Config>(
    network_id: T::NetworkId,
    request: &OffchainRequest<T>,
) -> bool {
    if network_id != T::GetEthNetworkId::get() {
        return false;
    }
    let xor_asset_id = common::XOR.into();
    match request {
        OffchainRequest::Outgoing(OutgoingRequest::Transfer(request), _) => {
            request.asset_id == xor_asset_id
        }
        OffchainRequest::Incoming(IncomingRequest::Transfer(request), _) => {
            request.asset_id == xor_asset_id
        }
        OffchainRequest::Outgoing(OutgoingRequest::AddAsset(request), _) => {
            request.asset_id == xor_asset_id
        }
        OffchainRequest::Outgoing(OutgoingRequest::AddToken(request), _) => {
            request.token_address == LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS
        }
        OffchainRequest::Incoming(IncomingRequest::AddToken(request), _) => {
            request.token_address == LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS
        }
        _ => false,
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
    use bridge_types::H256;
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
