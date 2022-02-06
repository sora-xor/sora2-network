//! Ceres launchpad module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::fixnum::ops::CheckedAdd;
use common::prelude::FixedWrapper;
use common::{balance, AccountIdOf, AssetId32, FromGenericPair, PSWAP, XOR};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use pswap_distribution::{ClaimableShares, ShareholderAccounts};
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::ModuleId;
use sp_std::prelude::*;

use crate::Pallet as CeresLaunchpad;
use assets::Module as Assets;
use frame_support::traits::{Get, Hooks};
use technical::Module as Technical;

pub type AssetId = AssetId32<common::PredefinedAssetId>;
pub const CERES_ASSET_ID: AssetId = common::AssetId32::from_bytes(hex!(
    "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
));

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn authority<T: Config>() -> T::AccountId {
    let bytes = hex!("34a5b78f5fbcdc92a28767d63b579690a4b2f6a179931b3ecc87f09fc9366d47");
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
    create_ilo {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_block = frame_system::Pallet::<T>::block_number();

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
            balance!(20000)
        );
    }: _(RawOrigin::Signed(caller.clone()),
        CERES_ASSET_ID.into(),
        balance!(7693),
        balance!(3000),
        balance!(0.13),
        balance!(600),
        balance!(1000),
        balance!(0.2),
        balance!(0.25),
        true,
        balance!(0.75),
        balance!(0.25),
        31,
        current_block + 5u32.into(),
        current_block + 10u32.into(),
        balance!(0.2),
        current_block + 3u32.into(),
        balance!(0.2)
    )
    verify {
        assert_last_event::<T>(Event::ILOCreated(caller.clone(), CERES_ASSET_ID.into()).into());
    }

    contribute {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_block = frame_system::Pallet::<T>::block_number();
        let funds_to_contribute = balance!(800);

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
            balance!(20000)
        );

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
            XOR.into(),
            caller.clone(),
            balance!(20000)
        );
        // Create ILO
        let _ = CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(500),
            balance!(900),
            true,
            balance!(0.75),
            balance!(0.25),
            31,
            current_block + 5u32.into(),
            current_block + 10u32.into(),
            balance!(0.2),
            current_block + 3u32.into(),
            balance!(0.2)
        );
        run_to_block::<T>(6);
    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into(), funds_to_contribute)
    verify {
        assert_last_event::<T>(Event::Contributed(caller, CERES_ASSET_ID.into(), funds_to_contribute).into());
    }

    emergency_withdraw {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_block = frame_system::Pallet::<T>::block_number();
        let funds_to_contribute = balance!(800);

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
            balance!(20000)
        );

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
            XOR.into(),
            caller.clone(),
            balance!(20000)
        );

        // Create ILO
        let _ = CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(500),
            balance!(900),
            true,
            balance!(0.75),
            balance!(0.25),
            31,
            current_block + 5u32.into(),
            current_block + 10u32.into(),
            balance!(0.2),
            current_block + 3u32.into(),
            balance!(0.2)
        );

        run_to_block::<T>(6);

        // Contribute
        let _ = CeresLaunchpad::<T>::contribute(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            funds_to_contribute,
        );
    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into())
    verify {
        assert_last_event::<T>(Event::EmergencyWithdrawn(caller, CERES_ASSET_ID.into(), funds_to_contribute).into());
    }

    finish_ilo {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_block = frame_system::Pallet::<T>::block_number();

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
            balance!(20000)
        );

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
            XOR.into(),
            caller.clone(),
            balance!(10000)
        );

        let _ = CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(500),
            balance!(900),
            false,
            balance!(0.75),
            balance!(0.25),
            31,
            current_block + 5u32.into(),
            current_block + 10u32.into(),
            balance!(0.2),
            current_block + 3u32.into(),
            balance!(0.2)
        );

        run_to_block::<T>(6);

        let funds_to_contribute = balance!(800);

        let _ = CeresLaunchpad::<T>::contribute(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        );

        run_to_block::<T>(11);
    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into())
    verify {
        assert_last_event::<T>(Event::ILOFinished(caller.clone(), CERES_ASSET_ID.into()).into());
    }

    claim_lp_tokens {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_block = frame_system::Pallet::<T>::block_number();
        let funds_to_contribute = balance!(800);

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
            balance!(20000)
        );

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
            XOR.into(),
            caller.clone(),
            balance!(20000)
        );
        // Create ILO
        let _ = CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(500),
            balance!(900),
            true,
            balance!(0.75),
            balance!(0.25),
            31,
            current_block + 5u32.into(),
            current_block + 10u32.into(),
            balance!(0.2),
            current_block + 3u32.into(),
            balance!(0.2)
        );

        run_to_block::<T>(6);
        // Contribute
        let _ = CeresLaunchpad::<T>::contribute(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            funds_to_contribute,
        );
        run_to_block::<T>(11);
        // Finish ILO
        let _ = CeresLaunchpad::<T>::finish_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into()
        );
        run_to_block::<T>(500000);
    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into())
    verify {
        assert_last_event::<T>(Event::ClaimedLP(caller, CERES_ASSET_ID.into()).into());
    }

    claim {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_block = frame_system::Pallet::<T>::block_number();

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
            balance!(20000)
        );

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
            XOR.into(),
            caller.clone(),
            balance!(10000)
        );

        let _ = CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(500),
            balance!(900),
            false,
            balance!(0.75),
            balance!(0.25),
            31,
            current_block + 5u32.into(),
            current_block + 10u32.into(),
            balance!(0.1),
            30u32.into(),
            balance!(0.18)
        );

        run_to_block::<T>(6);

        let funds_to_contribute = balance!(800);

        let _ = CeresLaunchpad::<T>::contribute(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        );

        run_to_block::<T>(11);

        let _ = CeresLaunchpad::<T>::finish_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into()
        );

        let _ = CeresLaunchpad::<T>::claim(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
        );

        run_to_block::<T>(43);
    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into())
    verify {
        assert_last_event::<T>(Event::Claimed(caller.clone(), CERES_ASSET_ID.into()).into());
    }

    change_ceres_burn_fee {
        let caller = authority::<T>();
        let fee = balance!(69);
    }: _(RawOrigin::Signed(caller.clone()), fee)
    verify {
        assert_last_event::<T>(Event::FeeChanged(fee).into());
    }

    change_ceres_contribution_fee {
        let caller = authority::<T>();
        let fee = balance!(69);
    }: _(RawOrigin::Signed(caller.clone()), fee)
    verify {
        assert_last_event::<T>(Event::FeeChanged(fee).into());
    }

    claim_pswap_rewards {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_block = frame_system::Pallet::<T>::block_number();

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
            balance!(20000)
        );

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
            XOR.into(),
            caller.clone(),
            balance!(10000)
        );

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
            PSWAP.into(),
            T::GetTechnicalAccountId::get(),
            balance!(10000)
        );

        let _ = CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            balance!(7693),
            balance!(3000),
            balance!(0.13),
            balance!(600),
            balance!(1000),
            balance!(500),
            balance!(900),
            false,
            balance!(0.75),
            balance!(0.25),
            31,
            current_block + 5u32.into(),
            current_block + 10u32.into(),
            balance!(0.2),
            current_block + 3u32.into(),
            balance!(0.2)
        );

        run_to_block::<T>(6);

        let funds_to_contribute = balance!(800);

        let _ = CeresLaunchpad::<T>::contribute(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        );

        run_to_block::<T>(11);

        let _ = CeresLaunchpad::<T>::finish_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into()
        );

        run_to_block::<T>(20000);

        let share = FixedWrapper::from(1.00).get().unwrap();
        let pallet_account: AccountIdOf<T> = ModuleId(*b"crslaunc").into_account();
        ShareholderAccounts::<T>::mutate(&pallet_account, |current| {
            *current = current.saturating_add(share)
        });
        ClaimableShares::<T>::mutate(|current| *current = current.saturating_add(share));
    }: _(RawOrigin::Signed(authority::<T>()))
    verify {
        assert_last_event::<T>(Event::ClaimedPSWAP().into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::empty().build().execute_with(|| {
            assert_ok!(test_benchmark_create_ilo::<Runtime>());
            assert_ok!(test_benchmark_contribute::<Runtime>());
            assert_ok!(test_benchmark_emergency_withdraw::<Runtime>());
            assert_ok!(test_benchmark_finish_ilo::<Runtime>());
            assert_ok!(test_benchmark_claim_lp_tokens::<Runtime>());
            assert_ok!(test_benchmark_claim::<Runtime>());
            assert_ok!(test_benchmark_change_ceres_burn_fee::<Runtime>());
            assert_ok!(test_benchmark_change_ceres_contribution_fee::<Runtime>());
            assert_ok!(test_benchmark_claim_pswap_rewards::<Runtime>());
        });
    }
}
