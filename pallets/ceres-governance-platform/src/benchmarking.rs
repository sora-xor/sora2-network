//! Ceres governance platform module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{Balance, balance, AssetId32, AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION};
use frame_benchmarking::{benchmarks, Zero};
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;
use frame_support::assert_ok;

use crate::Pallet as CeresGovernancePlatform;
use frame_support::traits::Hooks;

pub type AssetId = AssetId32<common::PredefinedAssetId>;
pub const CERES_ASSET_ID: AssetId = common::AssetId32::from_bytes(hex!(
    "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
));

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

fn run_to_block<T: Config>(n: u32) {
    while frame_system::Pallet::<T>::block_number() < n.into() {
        frame_system::Pallet::<T>::on_finalize(frame_system::Pallet::<T>::block_number());
        frame_system::Pallet::<T>::set_block_number(
            frame_system::Pallet::<T>::block_number() + 1u32.into(),
        );
        frame_system::Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number());
    }
}

benchmarks! {
    /*vote {
        let caller = alice::<T>();
        let poll_id = Vec::from([1, 2, 3, 4]);
        let voting_option = 3;
        let poll_start_block = frame_system::Pallet::<T>::block_number();
        let number_of_votes = balance!(300);
        let poll_end_block = poll_start_block + 10u32.into();

        frame_system::Pallet::<T>::inc_providers(&caller);
        let _ = assets::Pallet::<T>::register_asset_id(
            caller.clone(),
            CERES_ASSET_ID.into(),
            AssetSymbol(b"CERES".to_vec()),
            AssetName(b"Ceres".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        );
        let _ = assets::Pallet::<T>::mint(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            number_of_votes,
        );

        let _ = CeresGovernancePlatform::<T>::create_poll(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            poll_start_block,
            poll_end_block
        );
    }: {
        let _ = CeresGovernancePlatform::<T>::vote(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            number_of_votes
        );
    }
    verify {
        assert_last_event::<T>(Event::Voted(caller, voting_option, number_of_votes).into());
    }*/

    create_poll {
        let caller = alice::<T>();
        let poll_id = Vec::from([1, 2, 3, 4]);
        let voting_option = 3;
        let poll_start_block = frame_system::Pallet::<T>::block_number();
        let poll_end_block = poll_start_block + 10u32.into();

        CeresGovernancePlatform::<T>::create_poll(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            poll_start_block,
            poll_end_block
            );
    }: {
       CeresGovernancePlatform::<T>::create_poll(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            poll_start_block,
            poll_end_block
        );
    }
    verify {
        assert_last_event::<T>(Event::Created(caller, voting_option, poll_start_block, poll_end_block).into());
    }

   /* withdraw {
        let caller = alice::<T>();
        let poll_id = Vec::from([1, 2, 3, 4]);
        let voting_option = 3;
        let number_of_votes = balance!(300);
        let poll_start_block = frame_system::Pallet::<T>::block_number();
        let poll_end_block = poll_start_block + 10u32.into();

        frame_system::Pallet::<T>::inc_providers(&caller);
        let _ = assets::Pallet::<T>::register_asset_id(
            caller.clone(),
            CERES_ASSET_ID.into(),
            AssetSymbol(b"CERES".to_vec()),
            AssetName(b"Ceres".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        );
        let _ = assets::Pallet::<T>::mint(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            number_of_votes,
        );

        // Create poll
        let _ = CeresGovernancePlatform::<T>::create_poll(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            poll_start_block,
            poll_end_block
        );

        // Vote
        let _ = CeresGovernancePlatform::<T>::vote(
            RawOrigin::Signed(caller.clone()).into(),
            poll_id.clone(),
            voting_option,
            number_of_votes
        );

        run_to_block::<T>(20);
    }: _(RawOrigin::Signed(caller.clone()), poll_id.clone())
    verify {
        assert_last_event::<T>(Event::Withdrawn(caller, number_of_votes).into());
    }*/
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            //assert_ok!(test_benchmark_vote::<Runtime>());
            assert_ok!(test_benchmark_create_poll::<Runtime>());
            //assert_ok!(test_benchmark_withdraw::<Runtime>());
        });
    }
}