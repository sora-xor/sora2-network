use crate::{Config, Error, RequestStatus};
use codec::{Decode, Encode};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::RuntimeDebug;
use frame_system::RawOrigin;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
pub enum StorageVersion {
    V1,
    V2RemovePendingTransfers,
}

impl Default for StorageVersion {
    fn default() -> Self {
        Self::V2RemovePendingTransfers
    }
}

pub fn migrate_broken_pending_outgoing_transfers<T: Config>(to_height: T::BlockNumber) -> Weight {
    let net_id = T::GetEthNetworkId::get();
    let mut queue = crate::RequestsQueue::<T>::get(net_id);
    let mut count = 0;
    let queue_len = queue.len();
    queue.retain(|hash| {
        let status = crate::RequestStatuses::<T>::get(net_id, hash);
        if status != Some(RequestStatus::Pending) {
            return true;
        }
        let request_height = crate::RequestSubmissionHeight::<T>::get(net_id, hash);
        let should_remove = request_height <= to_height;
        if should_remove {
            crate::RequestStatuses::<T>::insert(
                net_id,
                hash,
                RequestStatus::Broken(
                    Error::<T>::RemovedAndRefunded.into(),
                    Error::<T>::RemovedAndRefunded.into(),
                ),
            );
            let _ = crate::RequestApprovals::<T>::take(net_id, hash);
            count += 1;
        }
        !should_remove
    });
    crate::RequestsQueue::<T>::insert(net_id, queue);
    frame_support::debug::info!(
        "eth-bridge: {} requests migrated to V2RemovePendingTransfers.",
        count,
    );
    <T as frame_system::Config>::DbWeight::get()
        .reads_writes((queue_len * 2 + 1) as Weight, (count * 2) as Weight)
}

pub fn remove_signatory<T: Config>(account_id: T::AccountId) {
    let bridge_multisig =
        crate::BridgeAccount::<T>::get(T::GetEthNetworkId::get()).unwrap_or_default();
    let result = bridge_multisig::Pallet::<T>::remove_signatory(
        RawOrigin::Signed(bridge_multisig).into(),
        account_id,
    );
    frame_support::debug::info!("eth-bridge: remove_signatory result {:?}", result);
}
