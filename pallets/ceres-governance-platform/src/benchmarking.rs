//! Ceres governance platform module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use common::{balance, CERES_ASSET_ID};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_std::prelude::*;

use crate::Pallet as CeresGovernancePlatform;
use assets::Pallet as Assets;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    vote {
        let title = "Title";
        let description = "Description";
        let voting_option = "Yes";
        let asset_id = CERES_ASSET_ID;
        let number_of_votes = balance!(300);
        let poll_start_timestamp = Timestamp::<T>::get();
        let poll_end_timestamp = poll_start_timestamp + 10u32.into();
        let caller = pallet::AuthorityAccount::<T>::get();
        let nonce = frame_system::Pallet::<T>::account_nonce(&caller);
        let encoded: [u8; 32] = (&caller, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let mut options = BoundedVec::default();
        options.try_push("Yes".try_into().unwrap()).unwrap();
        options.try_push("No".try_into().unwrap()).unwrap();

        frame_system::Pallet::<T>::inc_providers(&caller);

        let owner: T::AccountId = assets::AssetOwners::<T>::get::<T::AssetId>(asset_id.into()).unwrap();
        Assets::<T>::mint(
            RawOrigin::Signed(owner).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            number_of_votes
        ).unwrap();

        // Create poll
        let poll_info = PollInfo {
            poll_asset: asset_id.into(),
            poll_start_timestamp,
            poll_end_timestamp,
            title: title.try_into().unwrap(),
            description: description.try_into().unwrap(),
            options,
        };
        pallet::PollData::<T>::insert(poll_id, poll_info);
    }: {
        CeresGovernancePlatform::<T>::vote(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id,
            voting_option.try_into().unwrap(),
            number_of_votes
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::Voted(caller, poll_id, voting_option.try_into().unwrap(), asset_id.into(), number_of_votes).into());
    }

    create_poll {
        let title = "Title";
        let description = "Description";
        let voting_option = "Yes";
        let asset_id = CERES_ASSET_ID;
        let number_of_votes = balance!(300);
        let poll_start_timestamp = Timestamp::<T>::get();
        let poll_end_timestamp = poll_start_timestamp + 10u32.into();
        let caller = pallet::AuthorityAccount::<T>::get();
        let nonce = frame_system::Pallet::<T>::account_nonce(&caller);
        let encoded: [u8; 32] = (&caller, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let mut options = BoundedVec::default();
        options.try_push("Yes".try_into().unwrap()).unwrap();
        options.try_push("No".try_into().unwrap()).unwrap();

        frame_system::Pallet::<T>::inc_providers(&caller);

        let owner: T::AccountId = assets::AssetOwners::<T>::get::<T::AssetId>(asset_id.into()).unwrap();
        Assets::<T>::mint(
            RawOrigin::Signed(owner).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            number_of_votes
        ).unwrap();
    }: {
       let _ = CeresGovernancePlatform::<T>::create_poll(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id.into(),
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            description.try_into().unwrap(),
            options
        );
    }
    verify {
        assert_last_event::<T>(Event::<T>::Created(caller, title.try_into().unwrap(), asset_id.into(), poll_start_timestamp, poll_end_timestamp).into());
    }

   withdraw {
    let title = "Title";
    let description = "Description";
    let voting_option = "Yes";
    let asset_id = CERES_ASSET_ID;
    let number_of_votes = balance!(300);
    let poll_start_timestamp = Timestamp::<T>::get();
    let poll_end_timestamp = poll_start_timestamp + 100u32.into();
    let caller = pallet::AuthorityAccount::<T>::get();
    let nonce = frame_system::Pallet::<T>::account_nonce(&caller);
    let encoded: [u8; 32] = (&caller, nonce).using_encoded(blake2_256);
    let poll_id = H256::from(encoded);
    let mut options = BoundedVec::default();
    options.try_push("Yes".try_into().unwrap()).unwrap();
    options.try_push("No".try_into().unwrap()).unwrap();

    frame_system::Pallet::<T>::inc_providers(&caller);

    let owner: T::AccountId = assets::AssetOwners::<T>::get::<T::AssetId>(asset_id.into()).unwrap();
    Assets::<T>::mint(
        RawOrigin::Signed(owner).into(),
        CERES_ASSET_ID.into(),
        caller.clone(),
        number_of_votes
    ).unwrap();

        // Create poll
        let poll_info = PollInfo {
            poll_asset: asset_id.into(),
            poll_start_timestamp,
            poll_end_timestamp,
            title: title.try_into().unwrap(),
            description: description.try_into().unwrap(),
            options,
        };
        pallet::PollData::<T>::insert(poll_id, poll_info);

        // Vote
        let _ = CeresGovernancePlatform::<T>::vote(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id,
            voting_option.try_into().unwrap(),
            number_of_votes
        ).unwrap();

        pallet_timestamp::Now::<T>::put(poll_start_timestamp + 150u32.into());
    }: _(RawOrigin::Signed(caller.clone()), poll_id)
    verify {
        assert_last_event::<T>(Event::<T>::Withdrawn(caller, poll_id, asset_id.into(), number_of_votes).into());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
