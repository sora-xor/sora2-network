//! Ceres governance platform module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{balance, FromGenericPair, CERES_ASSET_ID};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::Pallet as CeresGovernancePlatform;
use assets::Pallet as Assets;
use technical::Pallet as Technical;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
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
        let assets_and_permissions_tech_account_id =
            T::TechAccountId::from_generic_pair(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
        let assets_and_permissions_account_id =
            Technical::<T>::tech_account_id_to_account_id(
                &assets_and_permissions_tech_account_id,
            ).unwrap();

        let owner: T::AccountId = assets::AssetOwners::<T>::get::<T::AssetId>(CERES_ASSET_ID.clone().into()).unwrap();

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
        assert_last_event::<T>(Event::Voted(caller, poll_id, voting_option, number_of_votes).into());
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
        assert_last_event::<T>(Event::Created(caller, voting_option, poll_start_timestamp, poll_end_timestamp).into());
    }

   withdraw {
        let caller = alice::<T>();
        let poll_id = Vec::from([1, 2, 3, 4]);
        let voting_option = 3;
        let number_of_votes = balance!(300);
        let poll_start_timestamp = Timestamp::<T>::get();
        let poll_end_timestamp = poll_start_timestamp + 10u32.into();

        frame_system::Pallet::<T>::inc_providers(&caller);
        let assets_and_permissions_tech_account_id =
            T::TechAccountId::from_generic_pair(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
        let assets_and_permissions_account_id =
            Technical::<T>::tech_account_id_to_account_id(
                &assets_and_permissions_tech_account_id,
            ).unwrap();

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            number_of_votes
        );

        // Create poll
        let _ = CeresGovernancePlatform::<T>::create_poll(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            poll_start_timestamp,
            poll_end_timestamp
        );

        // Vote
        let _ = CeresGovernancePlatform::<T>::vote(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            number_of_votes
        );

        pallet_timestamp::Now::<T>::put(poll_start_timestamp + 14440u32.into());
    }: _(RawOrigin::Signed(caller.clone()), poll_id.clone())
    verify {
        assert_last_event::<T>(Event::Withdrawn(caller, 0).into());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
