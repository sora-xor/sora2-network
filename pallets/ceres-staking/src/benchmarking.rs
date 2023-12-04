//! Ceres staking module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{balance, AssetId32, PredefinedAssetId, CERES_ASSET_ID};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::Pallet as CeresStaking;
use assets::Pallet as Assets;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    where_clause {
        where T::AssetId: From<AssetId32<PredefinedAssetId>>
    }

    deposit {
        let caller = alice::<T>();
        let amount = balance!(100);
        let asset_id = T::AssetId::from(CERES_ASSET_ID);
        let asset_owner = Assets::<T>::asset_owner(asset_id).unwrap();
        frame_system::Pallet::<T>::inc_providers(&caller);

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(asset_owner).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(101)
        );
    }: _(RawOrigin::Signed(caller.clone()), amount)
    verify {
        assert_last_event::<T>(Event::<T>::Deposited(caller.clone(), amount).into());
    }

    withdraw {
        let caller = alice::<T>();
        let amount = balance!(100);
        let asset_id = T::AssetId::from(CERES_ASSET_ID);
        let asset_owner = Assets::<T>::asset_owner(asset_id).unwrap();
        frame_system::Pallet::<T>::inc_providers(&caller);

        Assets::<T>::mint(
            RawOrigin::Signed(asset_owner).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(101)
        ).unwrap();

        CeresStaking::<T>::deposit(
            RawOrigin::Signed(caller.clone()).into(),
            amount
        ).unwrap();
    }: _(RawOrigin::Signed(caller.clone()))
    verify {
        assert_last_event::<T>(Event::<T>::Withdrawn(caller, amount, balance!(0)).into());
    }

    change_rewards_remaining {
        let caller = AuthorityAccount::<T>::get();
        let rewards = balance!(69);
    }: _(RawOrigin::Signed(caller.clone()), rewards)
    verify {
        assert_last_event::<T>(Event::<T>::RewardsChanged(rewards).into());
    }

    impl_benchmark_test_suite!(
        CeresStaking,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
