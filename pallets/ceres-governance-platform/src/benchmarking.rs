//! Ceres governance platform module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{balance, CERES_ASSET_ID};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::Pallet as CeresGovernancePlatform;
use assets::Pallet as Assets;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    vote {
        let caller = alice::<T>();
        let poll_id = Vec::from([1, 2, 3, 4]);
        let voting_option = 3;
        let number_of_votes = balance!(300);
        let poll_start_timestamp = Timestamp::<T>::get();
        let poll_end_timestamp = poll_start_timestamp + 10u32.into();

        frame_system::Pallet::<T>::inc_providers(&caller);

        let owner: T::AccountId = assets::AssetOwners::<T>::get::<T::AssetId>(CERES_ASSET_ID.into()).unwrap();
        Assets::<T>::mint(
            RawOrigin::Signed(owner).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            number_of_votes
        ).unwrap();

        CeresGovernancePlatform::<T>::create_poll(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            poll_start_timestamp,
            poll_end_timestamp
        ).unwrap();
    }: {
        CeresGovernancePlatform::<T>::vote(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            number_of_votes
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::Voted(caller, poll_id, voting_option, number_of_votes).into());
    }

    create_poll {
        let caller = alice::<T>();
        let poll_id = Vec::from([1, 2, 3, 4]);
        let voting_option = 3;
        let poll_start_timestamp = Timestamp::<T>::get() + 5u32.into();
        let poll_end_timestamp = poll_start_timestamp + 10u32.into();
        frame_system::Pallet::<T>::inc_providers(&caller);
    }: {
       let _ = CeresGovernancePlatform::<T>::create_poll(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            poll_start_timestamp,
            poll_end_timestamp
        );
    }
    verify {
        assert_last_event::<T>(Event::<T>::Created(caller, voting_option, poll_start_timestamp, poll_end_timestamp).into());
    }

   withdraw {
        let caller = alice::<T>();
        let poll_id = Vec::from([1, 2, 3, 4]);
        let voting_option = 3;
        let number_of_votes = balance!(300);
        let poll_start_timestamp = Timestamp::<T>::get();
        let poll_end_timestamp = poll_start_timestamp + 10u32.into();

        frame_system::Pallet::<T>::inc_providers(&caller);

        let owner: T::AccountId = assets::AssetOwners::<T>::get::<T::AssetId>(CERES_ASSET_ID.into()).unwrap();
        let _ = Assets::<T>::mint(
            RawOrigin::Signed(owner).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            number_of_votes
        ).unwrap();

        // Create poll
        let _ = CeresGovernancePlatform::<T>::create_poll(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            poll_start_timestamp,
            poll_end_timestamp
        ).unwrap();

        // Vote
        let _ = CeresGovernancePlatform::<T>::vote(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            number_of_votes
        ).unwrap();

        pallet_timestamp::Now::<T>::put(poll_start_timestamp + 14440u32.into());
    }: _(RawOrigin::Signed(caller.clone()), poll_id)
    verify {
        assert_last_event::<T>(Event::<T>::Withdrawn(caller, number_of_votes).into());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
