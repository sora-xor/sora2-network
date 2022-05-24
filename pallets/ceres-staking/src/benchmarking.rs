//! Ceres staking module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{balance, FromGenericPair, CERES_ASSET_ID};
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::Pallet as CeresStaking;
use assets::Pallet as Assets;
use technical::Pallet as Technical;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    deposit {
        let caller = alice::<T>();
        let amount = balance!(100);
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
            balance!(101)
        );
    }: _(RawOrigin::Signed(caller.clone()), amount)
    verify {
        assert_last_event::<T>(Event::Deposited(caller.clone(), amount).into());
    }

    withdraw {
        let caller = alice::<T>();
        let amount = balance!(100);
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
            balance!(101)
        );

        let _ = CeresStaking::<T>::deposit(
            RawOrigin::Signed(caller.clone()).into(),
            amount
        ).unwrap();
    }: _(RawOrigin::Signed(caller.clone()))
    verify {
        assert_last_event::<T>(Event::Withdrawn(caller, amount, balance!(0)).into());
    }

    change_rewards_remaining {
        let caller = AuthorityAccount::<T>::get();
        let rewards = balance!(69);
    }: _(RawOrigin::Signed(caller.clone()), rewards)
    verify {
        assert_last_event::<T>(Event::RewardsChanged(rewards).into());
    }
}

impl_benchmark_test_suite!(
    CeresStaking,
    crate::mock::ExtBuilder {
        endowed_assets: vec![],
        endowed_accounts: vec![]
    }
    .build(),
    crate::mock::Runtime
);
