//! Ceres launchpad module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::fixnum::ops::CheckedAdd;
use common::prelude::FixedWrapper;
use common::{
    balance, AccountIdOf, AssetId32, AssetIdOf, AssetInfoProvider, AssetManager, PredefinedAssetId,
    CERES_ASSET_ID, PSWAP, XOR,
};
use frame_benchmarking::benchmarks;
use frame_support::traits::Get;
use frame_support::PalletId;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use pswap_distribution::{ClaimableShares, ShareholderAccounts};
use sp_runtime::traits::{AccountIdConversion, Saturating};
use sp_std::prelude::*;

use crate::Pallet as CeresLaunchpad;

// Support Functions
fn alice<T: frame_system::Config>() -> T::AccountId {
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
    where_clause {
        where AssetIdOf::<T>: From<AssetId32<PredefinedAssetId>>
    }

    create_ilo {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_timestamp = Timestamp::<T>::get();

        let asset_id = AssetIdOf::<T>::from(CERES_ASSET_ID);
        let asset_owner = <T as Config>::AssetInfoProvider::get_asset_owner(&asset_id).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(20000)
        ).unwrap();

        CeresLaunchpad::<T>::add_whitelisted_ilo_organizer(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();
    }: _(RawOrigin::Signed(caller.clone()),
        XOR.into(),
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
        current_timestamp + 5u32.into(),
        current_timestamp + 10u32.into(),
        balance!(1000),
        balance!(0.2),
        current_timestamp + 3u32.into(),
        balance!(0.2),
        balance!(0.2),
        current_timestamp + 3u32.into(),
        balance!(0.2)
    )
    verify {
        assert_last_event::<T>(Event::<T>::ILOCreated(caller, CERES_ASSET_ID.into()).into());
    }

    contribute {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_timestamp = Timestamp::<T>::get();
        let funds_to_contribute = balance!(800);

        let asset_id = AssetIdOf::<T>::from(CERES_ASSET_ID);
        let asset_owner = <T as Config>::AssetInfoProvider::get_asset_owner(&asset_id).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(20000)
        ).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner).into(),
            XOR.into(),
            caller.clone(),
            balance!(20000)
        ).unwrap();

        CeresLaunchpad::<T>::add_whitelisted_ilo_organizer(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        // Create ILO
        CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
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
            current_timestamp + 5u32.into(),
            current_timestamp + 10u32.into(),
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2)
        ).unwrap();

        CeresLaunchpad::<T>::add_whitelisted_contributor(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        pallet_timestamp::Now::<T>::put(current_timestamp + 7u32.into());
    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into(), funds_to_contribute)
    verify {
        assert_last_event::<T>(Event::<T>::Contributed(caller, CERES_ASSET_ID.into(), funds_to_contribute).into());
    }

    emergency_withdraw {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_timestamp = Timestamp::<T>::get();
        let funds_to_contribute = balance!(800);

        let asset_id = AssetIdOf::<T>::from(CERES_ASSET_ID);
        let asset_owner = <T as Config>::AssetInfoProvider::get_asset_owner(&asset_id).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(20000)
        ).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner).into(),
            XOR.into(),
            caller.clone(),
            balance!(20000)
        ).unwrap();

        CeresLaunchpad::<T>::add_whitelisted_ilo_organizer(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        // Create ILO
        CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
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
            current_timestamp + 5u32.into(),
            current_timestamp + 10u32.into(),
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2)
        ).unwrap();

        pallet_timestamp::Now::<T>::put(current_timestamp + 7u32.into());

        CeresLaunchpad::<T>::add_whitelisted_contributor(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        // Contribute
        CeresLaunchpad::<T>::contribute(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            funds_to_contribute,
        ).unwrap();
    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into())
    verify {
        assert_last_event::<T>(Event::<T>::EmergencyWithdrawn(caller, CERES_ASSET_ID.into(), funds_to_contribute).into());
    }

    finish_ilo {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_timestamp = Timestamp::<T>::get();

        let asset_id = AssetIdOf::<T>::from(CERES_ASSET_ID);
        let asset_owner = <T as Config>::AssetInfoProvider::get_asset_owner(&asset_id).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(20000)
        ).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner).into(),
            XOR.into(),
            caller.clone(),
            balance!(10000)
        ).unwrap();

        CeresLaunchpad::<T>::add_whitelisted_ilo_organizer(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
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
            current_timestamp + 5u32.into(),
            current_timestamp + 10u32.into(),
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2)
        ).unwrap();


        pallet_timestamp::Now::<T>::put(current_timestamp + 7u32.into());

        let funds_to_contribute = balance!(800);

        CeresLaunchpad::<T>::add_whitelisted_contributor(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        CeresLaunchpad::<T>::contribute(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ).unwrap();

        pallet_timestamp::Now::<T>::put(current_timestamp + 11u32.into());

    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into())
    verify {
        assert_last_event::<T>(Event::<T>::ILOFinished(caller, CERES_ASSET_ID.into()).into());
    }

    claim_lp_tokens {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_timestamp = Timestamp::<T>::get();
        let finish_timestamp = current_timestamp + 11u32.into();
        let funds_to_contribute = balance!(800);

        let asset_id = AssetIdOf::<T>::from(CERES_ASSET_ID);
        let asset_owner = <T as Config>::AssetInfoProvider::get_asset_owner(&asset_id).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(20000)
        ).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner).into(),
            XOR.into(),
            caller.clone(),
            balance!(20000)
        ).unwrap();

        CeresLaunchpad::<T>::add_whitelisted_ilo_organizer(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        // Create ILO
        CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
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
            current_timestamp + 5u32.into(),
            current_timestamp + 10u32.into(),
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2)
        ).unwrap();

        pallet_timestamp::Now::<T>::put(current_timestamp + 7u32.into());

        CeresLaunchpad::<T>::add_whitelisted_contributor(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        // Contribute
        CeresLaunchpad::<T>::contribute(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            funds_to_contribute,
        ).unwrap();

        pallet_timestamp::Now::<T>::put(finish_timestamp);

        // Finish ILO
        CeresLaunchpad::<T>::finish_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into()
        ).unwrap();

        let unlocking_timestamp = finish_timestamp.saturating_add((86_400_000u32.saturating_mul(31u32)).into());
        pallet_timestamp::Now::<T>::put(unlocking_timestamp + 1u32.into());
    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into())
    verify {
        assert_last_event::<T>(Event::<T>::ClaimedLP(caller, CERES_ASSET_ID.into()).into());
    }

    claim {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_timestamp = Timestamp::<T>::get();

        let asset_id = AssetIdOf::<T>::from(CERES_ASSET_ID);
        let asset_owner = <T as Config>::AssetInfoProvider::get_asset_owner(&asset_id).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(20000)
        ).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner).into(),
            XOR.into(),
            caller.clone(),
            balance!(10000)
        ).unwrap();

        CeresLaunchpad::<T>::add_whitelisted_ilo_organizer(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
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
            current_timestamp + 5u32.into(),
            current_timestamp + 10u32.into(),
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2),
            balance!(0.1),
            current_timestamp + 30u32.into(),
            balance!(0.18)
        ).unwrap();

        pallet_timestamp::Now::<T>::put(current_timestamp + 7u32.into());

        let funds_to_contribute = balance!(800);

        CeresLaunchpad::<T>::add_whitelisted_contributor(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        CeresLaunchpad::<T>::contribute(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ).unwrap();

        pallet_timestamp::Now::<T>::put(current_timestamp + 11u32.into());

        CeresLaunchpad::<T>::finish_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into()
        ).unwrap();

        pallet_timestamp::Now::<T>::put(current_timestamp + 44u32.into());
    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into())
    verify {
        assert_last_event::<T>(Event::<T>::Claimed(caller, CERES_ASSET_ID.into()).into());
    }

    change_ceres_burn_fee {
        let caller = AuthorityAccount::<T>::get();
        let fee = balance!(69);
    }: _(RawOrigin::Signed(caller.clone()), fee)
    verify {
        assert_last_event::<T>(Event::<T>::FeeChanged(fee).into());
    }

    change_ceres_contribution_fee {
        let caller = AuthorityAccount::<T>::get();
        let fee = balance!(69);
    }: _(RawOrigin::Signed(caller.clone()), fee)
    verify {
        assert_last_event::<T>(Event::<T>::FeeChanged(fee).into());
    }

    claim_pswap_rewards {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let current_timestamp = Timestamp::<T>::get();

        let asset_id = AssetIdOf::<T>::from(CERES_ASSET_ID);
        let asset_owner = <T as Config>::AssetInfoProvider::get_asset_owner(&asset_id).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner.clone()).into(),
            CERES_ASSET_ID.into(),
            caller.clone(),
            balance!(20000)
        ).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner.clone()).into(),
            XOR.into(),
            caller.clone(),
            balance!(10000)
        ).unwrap();

        T::AssetManager::mint(
            RawOrigin::Signed(asset_owner).into(),
            PSWAP.into(),
            T::GetTechnicalAccountId::get(),
            balance!(10000)
        ).unwrap();

        CeresLaunchpad::<T>::add_whitelisted_ilo_organizer(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        CeresLaunchpad::<T>::create_ilo(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
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
            current_timestamp + 5u32.into(),
            current_timestamp + 10u32.into(),
            balance!(1000),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2),
            balance!(0.2),
            current_timestamp + 3u32.into(),
            balance!(0.2)
        ).unwrap();


        pallet_timestamp::Now::<T>::put(current_timestamp + 7u32.into());

        let funds_to_contribute = balance!(800);

        CeresLaunchpad::<T>::add_whitelisted_contributor(
            RawOrigin::Signed(pallet::AuthorityAccount::<T>::get()).into(),
            caller.clone()
        ).unwrap();

        CeresLaunchpad::<T>::contribute(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            funds_to_contribute
        ).unwrap();

        pallet_timestamp::Now::<T>::put(current_timestamp + 11u32.into());

        CeresLaunchpad::<T>::finish_ilo(
            RawOrigin::Signed(caller).into(),
            CERES_ASSET_ID.into()
        ).unwrap();

        pallet_timestamp::Now::<T>::put(current_timestamp + 20000u32.into());

        let share = FixedWrapper::from(1.00).get().unwrap();
        let pallet_account: AccountIdOf<T> = PalletId(*b"crslaunc").into_account_truncating();
        ShareholderAccounts::<T>::mutate(&pallet_account, |current| {
            *current = current.saturating_add(share)
        });
        ClaimableShares::<T>::mutate(|current| *current = current.saturating_add(share));
    }: _(RawOrigin::Signed(AuthorityAccount::<T>::get()))
    verify {
        assert_last_event::<T>(Event::<T>::ClaimedPSWAP().into());
    }

    add_whitelisted_contributor {
        let caller = AuthorityAccount::<T>::get();
        let contributor = alice::<T>();
    }: _(RawOrigin::Signed(caller.clone()), contributor.clone())
    verify {
        assert_last_event::<T>(Event::<T>::WhitelistedContributor(contributor).into());
    }

    remove_whitelisted_contributor {
        let caller = AuthorityAccount::<T>::get();
        let contributor = alice::<T>();
    }: _(RawOrigin::Signed(caller.clone()), contributor.clone())
    verify {
        assert_last_event::<T>(Event::<T>::RemovedWhitelistedContributor(contributor).into());
    }

    add_whitelisted_ilo_organizer {
        let caller = AuthorityAccount::<T>::get();
        let ilo_organizer = alice::<T>();
    }: _(RawOrigin::Signed(caller.clone()), ilo_organizer.clone())
    verify {
        assert_last_event::<T>(Event::<T>::WhitelistedIloOrganizer(ilo_organizer).into());
    }

    remove_whitelisted_ilo_organizer {
        let caller = AuthorityAccount::<T>::get();
        let ilo_organizer = alice::<T>();
    }: _(RawOrigin::Signed(caller.clone()), ilo_organizer.clone())
    verify {
        assert_last_event::<T>(Event::<T>::RemovedWhitelistedIloOrganizer(ilo_organizer).into());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::benchmarking().build(),
        crate::mock::Runtime
    );
}
