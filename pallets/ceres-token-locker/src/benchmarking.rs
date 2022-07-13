//! Ceres token locker module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{balance, FromGenericPair, CERES_ASSET_ID};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::Pallet as CeresTokenLocker;
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
    lock_tokens {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let timestamp = Timestamp::<T>::get() + 10u32.into();
        let locked_tokens = balance!(2000);
        let token_balance = locked_tokens + balance!(100);

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
            token_balance
        );
    }: {
        let _ = CeresTokenLocker::<T>::lock_tokens(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            timestamp,
            locked_tokens
        );
    }
    verify {
        assert_last_event::<T>(Event::Locked(caller, locked_tokens, CERES_ASSET_ID.into()).into());
    }

    withdraw_tokens {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let timestamp = Timestamp::<T>::get() + 10u32.into();
        let locked_tokens = balance!(2000);
        let token_balance = locked_tokens + balance!(100);

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
            token_balance
        );

        // Lock tokens
        let _ = CeresTokenLocker::<T>::lock_tokens(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            timestamp,
            locked_tokens
        );

        pallet_timestamp::Now::<T>::put(Timestamp::<T>::get() + 14440u32.into());


    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into(), timestamp, locked_tokens)
    verify {
        assert_last_event::<T>(Event::Withdrawn(caller, locked_tokens, CERES_ASSET_ID.into()).into());
    }

    change_fee {
        let caller = crate::AuthorityAccount::<T>::get();
        let new_fee = balance!(100);
    }: _(RawOrigin::Signed(caller.clone()), new_fee)
    verify {
        assert_last_event::<T>(Event::FeeChanged(caller, new_fee).into());
    }

    // impl_benchmark_test_suite!(
    //     Pallet,
    //     crate::mock::ExtBuilder::default().build(),
    //     crate::mock::Runtime
    // );
}
