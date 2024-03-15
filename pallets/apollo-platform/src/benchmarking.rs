#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{Balance, XOR, DAI, DOT, APOLLO_ASSET_ID}
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use sp_std::prelude::*;

use crate::Pallet as ApolloPlatform;

// Support functions

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

// fn bob<T: Config>() -> T::AccountId {
//     // Not Bobs hex!
//     let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
//     T::AccountId::decode(&mut &bytes[..]).unwrap()
// }

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
        let slope_rate_1 = balance!(1);
        let slope_rate_2 = balance!(1);
        let reserve_factor = balance!(1);
    }: {
         ApolloPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id,
            loan_to_value,
            liquidation_threshold,
            optimal_utilization_rate,
            slope_rate_1,
            slope_rate_2,
            reserve_factor,
        ).unwrap()
    }
    verify {
        assert_last_event::<T>(Event::PoolAdded(caller, asset_id).into());
    }

    // lend {
    //     let caller = pallet::AuthorityAccount::<T>::get();
    //     let alice = alice::<T>();
    //     let asset_id = XOR;
    //     let loan_to_value = balance!(1);
    //     let liquidation_threshold = balance!(1);
    //     let optimal_utilization_rate = balance!(1);
    //     let slope_rate_1 = balance!(1);
    //     let slope_rate_2 = balance!(1);
    //     let reserve_factor = balance!(1);

    //     let lending_amount = balance!(100);

    //     let mint = assets::Pallet::<Runtime>::mint_to(
    //         &XOR,
    //         &alice,
    //         &alice,
    //         balance!(300000)
    //     );

    //     let add_pool = ApolloPlatform::<T>::add_pool(
    //         RawOrigin::Signed(caller.clone()).into(),
    //         asset_id,
    //         loan_to_value,
    //         liquidation_threshold,
    //         optimal_utilization_rate,
    //         slope_rate_1,
    //         slope_rate_2,
    //         reserve_factor,
    //     )
    // }: {
    //     let _ = ApolloPlatform::<T>::lend(
    //         &alice,
    //         &XOR,
    //         lending_amount
    //     )
    // } verify {
    //     assert_last_event::<T>(Event::Lended(alice, asset_id, lending_amount).into());
    // }


    // borrow {
    //         let caller = pallet::AuthorityAccount::<T>::get();
    //         let alice = alice::<T>();
    //         let asset_id = XOR;
    //         let loan_to_value = balance!(1);
    //         let liquidation_threshold = balance!(1);
    //         let optimal_utilization_rate = balance!(1);
    //         let slope_rate_1 = balance!(1);
    //         let slope_rate_2 = balance!(1);
    //         let reserve_factor = balance!(1);
    
    //         let lending_amount = balance!(100);

    //         let mintAlice = assets::Pallet::<Runtime>::mint_to(
    //             &DOT,
    //             &alice
    //             balance!(300000)
    //         )

    //         let mintBob = assets::Pallet::<Runtime>::mint_to(
    //             &XOR,
    //             &alice
    //             &bob,
    //             balance!(300000)
    //         )

    //         let add_pool = ApolloPlatform::<T>::add_pool(
    //             RawOrigin::Signed(caller.clone()).into(),
    //             asset_id,
    //             loan_to_value,
    //             liquidation_threshold,
    //             optimal_utilization_rate,
    //             slope_rate_1,
    //             slope_rate_2,
    //             reserve_factor,
    //         )

    //         let lend_alice = ApolloPlatform::lend(
    //             RuntimeOrigin::signed(alice()),
    //             DOT,
    //             balance!(100),
    //         );

    //         let lend_bob = ApolloPlatform::lend(
    //             RuntimeOrigin::signed(bob()),
    //             XOR,
    //             balance!(300000),
    //         );

    // }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
