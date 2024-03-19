#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{balance, DOT, XOR};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::Pallet as ApolloPlatform;

// Support functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

fn bob<T: Config>() -> T::AccountId {
    let bytes = hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48");
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
    add_pool {
        let caller = pallet::AuthorityAccount::<T>::get();
        let asset_id = XOR;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);
    }: {
         ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id.into(),
            loan_to_value,
            liquidation_threshold,
            optimal_utilization_rate,
            base_rate,
            slope_rate_1,
            slope_rate_2,
            reserve_factor,
        ).unwrap()
    }
    verify {
        assert_last_event::<T>(Event::PoolAdded(caller, asset_id.into()).into());
    }

    lend {
        let caller = pallet::AuthorityAccount::<T>::get();
        let alice = alice::<T>();
        let asset_id = XOR;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);

        let lending_amount = balance!(100);

        let mint = assets::Pallet::<T>::mint_to(
            &XOR.into(),
            &alice,
            &alice,
            balance!(300000)
        );

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id.into(),
            loan_to_value,
            liquidation_threshold,
            optimal_utilization_rate,
            base_rate,
            slope_rate_1,
            slope_rate_2,
            reserve_factor,
        ).unwrap();
    }: {
        ApolloPlatform::<T>::lend(
            RawOrigin::Signed(alice.clone()).into(),
            XOR.into(),
            lending_amount
        ).unwrap()
    } verify {
        assert_last_event::<T>(Event::Lended(alice, asset_id.into(), lending_amount).into());
    }

    borrow {
        let caller = pallet::AuthorityAccount::<T>::get();
        let alice = alice::<T>();
        let bob = bob::<T>();
        let asset_id_xor = XOR;
        let asset_id_dot = DOT;
        let loan_to_value = balance!(1);
        let liquidation_threshold = balance!(1);
        let optimal_utilization_rate = balance!(1);
        let base_rate = balance!(1);
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);

        let lending_amount_alice = balance!(100);
        let lending_amount_bob = balance!(200000);

        let collateral_amount = balance!(100);
        let borrow_amount = balance!(100);

        let mint_alice = assets::Pallet::<T>::mint_to(
            &DOT.into(),
            &alice,
            &alice,
            balance!(1000)
        );

        let mint_bob = assets::Pallet::<T>::mint_to(
            &XOR.into(),
            &alice,
            &bob,
            balance!(300000)
        );

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id_xor.into(),
            loan_to_value,
            liquidation_threshold,
            optimal_utilization_rate,
            base_rate,
            slope_rate_1,
            slope_rate_2,
            reserve_factor,
        ).unwrap();

        ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id_dot.into(),
            loan_to_value,
            liquidation_threshold,
            optimal_utilization_rate,
            base_rate,
            slope_rate_1,
            slope_rate_2,
            reserve_factor,
        ).unwrap();

        ApolloPlatform::<T>::lend(
            RawOrigin::Signed(bob.clone()).into(),
            XOR.into(),
            lending_amount_bob
        ).unwrap();

        ApolloPlatform::<T>::lend(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            lending_amount_alice
        ).unwrap();

    }: {
        ApolloPlatform::<T>::borrow(
            RawOrigin::Signed(alice.clone()).into(),
            DOT.into(),
            XOR.into(),
            borrow_amount
        ).unwrap()
    } verify {
        assert_last_event::<T>(Event::Borrowed(alice, DOT.into(), collateral_amount, XOR.into(), borrow_amount).into());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
