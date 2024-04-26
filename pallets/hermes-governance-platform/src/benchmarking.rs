//! Hermes governance platform module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::{Decode, Encode};
use common::{balance, AssetInfoProvider, HERMES_ASSET_ID};
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_support::BoundedVec;
use frame_support::PalletId;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_runtime::traits::AccountIdConversion;
use sp_std::prelude::*;

use crate::Pallet as HermesGovernancePlatform;

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
        let title = "Title";
        let description = "Description";
        let voting_option = "Yes";
        let hermes_amount = balance!(1000);
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<T>::get();
        let poll_start_timestamp = Timestamp::<T>::get();
        let poll_end_timestamp = Timestamp::<T>::get() + (14400*1000u32).into();
        let nonce = frame_system::Pallet::<T>::account_nonce(&caller);
        let encoded: [u8; 32] = (&caller, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let owner: T::AccountId = <T as Config>::AssetInfoProvider::get_asset_owner(&HERMES_ASSET_ID.clone().into()).unwrap();
        let mut options = BoundedVec::default();
        options.try_push("Yes".try_into().unwrap()).unwrap();
        options.try_push("No".try_into().unwrap()).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(owner).into(),
            HERMES_ASSET_ID.into(),
            caller.clone(),
            hermes_amount
        ).unwrap();

        let hermes_poll_info = HermesPollInfo {
            creator: caller.clone(),
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: title.try_into().unwrap(),
            description: description.try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options
        };

        pallet::HermesPollData::<T>::insert(&poll_id, &hermes_poll_info);
    }: {
        let _ = HermesGovernancePlatform::<T>::vote(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option.try_into().unwrap(),
        ).unwrap();
    }
    verify{
        assert_last_event::<T>(Event::Voted(caller, poll_id, voting_option.try_into().unwrap()).into())
    }

    create_poll {
        let caller = alice::<T>();
        let title = "Title";
        let descripton = "Description";
        let poll_start_timestamp = Timestamp::<T>::get();
        let poll_end_timestamp = Timestamp::<T>::get() + (14400*1000u32).into();
        let hermes_amount = balance!(100000);
        let owner: T::AccountId = <T as Config>::AssetInfoProvider::get_asset_owner(&HERMES_ASSET_ID.clone().into()).unwrap();
        let mut options = BoundedVec::default();
        options.try_push("Yes".try_into().unwrap()).unwrap();
        options.try_push("No".try_into().unwrap()).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(owner).into(),
            HERMES_ASSET_ID.into(),
            caller.clone(),
            hermes_amount
        ).unwrap();

    }: {
        let _ = HermesGovernancePlatform::<T>::create_poll(
            RawOrigin::Signed(caller.clone()).into(),
            poll_start_timestamp,
            poll_end_timestamp,
            title.try_into().unwrap(),
            descripton.try_into().unwrap(),
            options
        );
    }
    verify{
        assert_last_event::<T>(Event::Created(caller, title.try_into().unwrap(), poll_start_timestamp, poll_end_timestamp).into())
    }

    withdraw_funds_voter {
        let caller = alice::<T>();
        let title = "Title";
        let description = "Description";
        let voting_option = "Yes";
        let number_of_hermes = balance!(1000);
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<T>::get();
        let poll_start_timestamp = Timestamp::<T>::get();
        let poll_end_timestamp = Timestamp::<T>::get() + (14400*1000u32).into();
        let current_timestamp = Timestamp::<T>::get();
        let owner: T::AccountId = <T as Config>::AssetInfoProvider::get_asset_owner(&HERMES_ASSET_ID.clone().into()).unwrap();
        let nonce = frame_system::Pallet::<T>::account_nonce(&caller);
        let encoded: [u8; 32] = (&caller, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let mut options = BoundedVec::default();
        options.try_push("Yes".try_into().unwrap()).unwrap();
        options.try_push("No".try_into().unwrap()).unwrap();

        let _ = T::AssetManager::mint(
            RawOrigin::Signed(owner.clone()).into(),
            HERMES_ASSET_ID.into(),
            caller.clone(),
            number_of_hermes
        );

        let hermes_poll_info = HermesPollInfo {
            creator: caller.clone(),
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: title.try_into().unwrap(),
            description: description.try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options
        };

        pallet::HermesPollData::<T>::insert(&poll_id, &hermes_poll_info);

        let _ = HermesGovernancePlatform::<T>::vote(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option.try_into().unwrap(),
        );

        let hermes_voting_info = pallet::HermesVotings::<T>::get(&poll_id, &caller).unwrap();
        pallet_timestamp::Now::<T>::put(current_timestamp + (14401*1000u32).into());
    }: {
        let _ = HermesGovernancePlatform::<T>::withdraw_funds_voter(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone()
        );
    }
    verify {
        assert_last_event::<T>(Event::VoterFundsWithdrawn(caller, hermes_voting_info.number_of_hermes).into())
    }

    withdraw_funds_creator {
        let caller = alice::<T>();
        let title = "Title";
        let description = "Description";
        let voting_option = "Yes";
        let number_of_hermes = balance!(200000);
        let hermes_locked = pallet::MinimumHermesAmountForCreatingPoll::<T>::get();
        let poll_start_timestamp = Timestamp::<T>::get();
        let poll_end_timestamp = Timestamp::<T>::get() + (14400*1000u32).into();
        let owner: T::AccountId = <T as Config>::AssetInfoProvider::get_asset_owner(&HERMES_ASSET_ID.clone().into()).unwrap();
        let nonce = frame_system::Pallet::<T>::account_nonce(&caller);
        let encoded: [u8; 32] = (&caller, nonce).using_encoded(blake2_256);
        let poll_id = H256::from(encoded);
        let mut options = BoundedVec::default();
        options.try_push("Yes".try_into().unwrap()).unwrap();
        options.try_push("No".try_into().unwrap()).unwrap();

        let _ = T::AssetManager::mint(
            RawOrigin::Signed(owner).into(),
            HERMES_ASSET_ID.into(),
            caller.clone(),
            number_of_hermes
        );

        let hermes_poll_info = HermesPollInfo {
            creator: caller.clone(),
            hermes_locked,
            poll_start_timestamp,
            poll_end_timestamp,
            title: title.try_into().unwrap(),
            description: description.try_into().unwrap(),
            creator_hermes_withdrawn: false,
            options
        };

        pallet::HermesPollData::<T>::insert(&poll_id, &hermes_poll_info);

        let pallet_account: AccountIdOf<T> = PalletId(*b"hermsgov").into_account_truncating();
        assert_ok!(T::AssetManager::transfer_from(
            &HERMES_ASSET_ID.into(),
            &caller,
            &pallet_account,
            hermes_locked,
        ));

        pallet_timestamp::Now::<T>::put(poll_start_timestamp + (14401*1000u32).into());

    }: {
        let _ = HermesGovernancePlatform::<T>::withdraw_funds_creator(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone()
        );
    }
    verify {
        assert_last_event::<T>(Event::CreatorFundsWithdrawn(caller, hermes_locked).into())
    }

    change_min_hermes_for_voting {
        let caller = pallet::AuthorityAccount::<T>::get();
        let hermes_amount = balance!(20);
    }: {
        let _ = HermesGovernancePlatform::<T>::change_min_hermes_for_voting(
            RawOrigin::Signed(caller.clone()).into(),
            hermes_amount
        );
    }
    verify {
        assert_last_event::<T>(Event::MinimumHermesForVotingChanged(hermes_amount).into())
    }

    change_min_hermes_for_creating_poll {
        let caller = pallet::AuthorityAccount::<T>::get();
        let hermes_amount = balance!(20);
    }: {
        let _ = HermesGovernancePlatform::<T>::change_min_hermes_for_creating_poll(
            RawOrigin::Signed(caller.clone()).into(),
            hermes_amount
        );
    }
    verify {
        assert_last_event::<T>(Event::MinimumHermesForCreatingPollChanged(hermes_amount).into())
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
