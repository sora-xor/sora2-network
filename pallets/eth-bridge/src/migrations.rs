use crate::{Address, Call, Config, Error, RequestStatus};
use bridge_multisig::MultiChainHeight;
use frame_support::debug;
use frame_support::sp_runtime::offchain::storage_lock::BlockNumberProvider;
use frame_support::sp_runtime::traits::{One, Saturating};
use frame_support::traits::schedule::{Anon, DispatchTime};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_system::RawOrigin;
use sp_core::H256;

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

pub fn remove_peers<T: Config>(peer_ids: &[(T::AccountId, Address)]) {
    let eth_network_id = T::GetEthNetworkId::get();
    let mut peers = crate::Peers::<T>::get(eth_network_id);
    for (account_id, address) in peer_ids {
        let bridge_multisig = crate::BridgeAccount::<T>::get(eth_network_id).unwrap_or_default();
        let result = bridge_multisig::Pallet::<T>::remove_signatory(
            RawOrigin::Signed(bridge_multisig).into(),
            account_id.clone(),
        );
        frame_support::debug::info!("eth-bridge: remove_signatory result {:?}", result);
        crate::PeerAddress::<T>::remove(eth_network_id, account_id);
        crate::PeerAccountId::<T>::remove(eth_network_id, &address);
        peers.remove(account_id);
    }
    crate::Peers::<T>::insert(eth_network_id, peers);
}

pub(crate) fn migrate_to_0_2_0<T: Config>() -> Weight {
    let block_number = frame_system::Pallet::<T>::current_block_number();
    let weight = migrate_broken_pending_outgoing_transfers::<T>(
        block_number.saturating_sub(T::RemovePendingOutgoingRequestsAfter::get()),
    );
    if !T::RemovePeerAccountIds::get().is_empty() {
        let eth_network_id = T::GetEthNetworkId::get();
        let bridge_multisig = crate::BridgeAccount::<T>::get(eth_network_id).unwrap_or_default();
        let (from_thischain_height, from_sidechain_height) =
            T::TrackPendingIncomingRequestsAfter::get();
        // The migration involves removing some peers and since there may be requests
        // with, for example, 2/3 approvals, after removing a peer, the operation
        // will never be executed. To prevent this, we store the pending requests and wait
        // for all of them to finish first, before executing the next migration step.
        let pending_multisigs = bridge_multisig::Multisigs::<T>::iter_prefix(&bridge_multisig)
            .filter(|(_, multisig)| match multisig.when.height {
                MultiChainHeight::Thischain(n) => n >= from_thischain_height,
                MultiChainHeight::Sidechain(n) => n >= from_sidechain_height,
            })
            .map(|(hash, _)| H256(hash))
            .collect();
        crate::MigratingRequests::<T>::set(pending_multisigs);

        if T::Scheduler::schedule(
            DispatchTime::At(block_number + T::BlockNumber::one()),
            None,
            1,
            RawOrigin::Root.into(),
            Call::migrate_to_0_3_0().into(),
        )
        .is_err()
        {
            debug::warn!("eth bridge migration to v0.2.0 failed to schedule");
        }
    }

    weight
}
