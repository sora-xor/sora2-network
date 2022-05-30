//! Demeter farming platform module benchmarking.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Decode;
use common::{
    balance, AssetName, AssetSymbol, Balance, CERES_ASSET_ID, DEFAULT_BALANCE_PRECISION, XOR,
};
use demeter_farming_platform::{AccountIdOf, AuthorityAccount, UserInfos};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_runtime::traits::AccountIdConversion;
use sp_std::prelude::*;

use assets::Pallet as Assets;
use demeter_farming_platform::Pallet as DemeterFarmingPlatform;
use frame_support::traits::Hooks;
use frame_support::PalletId;
use permissions::Pallet as Permissions;

#[cfg(test)]
mod mock;

pub use demeter_farming_platform::Config;
pub struct Pallet<T: Config>(demeter_farming_platform::Pallet<T>);

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

fn setup_benchmark_assets_only<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);

    let _ = Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::MINT,
        permissions::Scope::Unlimited,
    );
    let _ = Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::BURN,
        permissions::Scope::Unlimited,
    );

    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        XOR.into(),
        AssetSymbol(b"XOR".to_vec()),
        AssetName(b"SORA".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        CERES_ASSET_ID.into(),
        AssetSymbol(b"CERES".to_vec()),
        AssetName(b"Ceres".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    );

    Ok(())
}

fn run_to_block<T: Config>(n: u32) {
    while frame_system::Pallet::<T>::block_number() < n.into() {
        frame_system::Pallet::<T>::on_finalize(frame_system::Pallet::<T>::block_number().into());
        frame_system::Pallet::<T>::set_block_number(
            frame_system::Pallet::<T>::block_number() + 1u32.into(),
        );
        frame_system::Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number().into());
        DemeterFarmingPlatform::<T>::on_initialize(
            frame_system::Pallet::<T>::block_number().into(),
        );
    }
}

benchmarks! {
    register_token {
        let authority = AuthorityAccount::<T>::get();
        let team_account = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&authority);
        let reward_asset = XOR;
        let token_per_block = balance!(1);
        let farms_allocation = balance!(0.6);
        let staking_allocation = balance!(0.2);
        let team_allocation = balance!(0.2);
    }: {
        let _ = DemeterFarmingPlatform::<T>::register_token(
            RawOrigin::Signed(authority.clone()).into(),
            reward_asset.into(),
            token_per_block,
            farms_allocation,
            staking_allocation,
            team_allocation,
            team_account
        );
    }
    verify {
        assert_last_event::<T>(demeter_farming_platform::Event::<T>::TokenRegistered(authority, reward_asset.into()).into());
    }

    add_pool {
        let authority = AuthorityAccount::<T>::get();
        let team_account = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&authority);
        let pool_asset = XOR;
        let reward_asset = CERES_ASSET_ID;
        let is_farm = true;
        let multiplier = 1;
        let deposit_fee = balance!(0.4);
        let is_core = true;
        let token_per_block = balance!(1);
        let farms_allocation = balance!(0.6);
        let staking_allocation = balance!(0.2);
        let team_allocation = balance!(0.2);

        let _ = DemeterFarmingPlatform::<T>::register_token(
            RawOrigin::Signed(authority.clone()).into(),
            reward_asset.into(),
            token_per_block,
            farms_allocation,
            staking_allocation,
            team_allocation,
            team_account,
        );
    }: {
        let _ = DemeterFarmingPlatform::<T>::add_pool(
            RawOrigin::Signed(authority.clone()).into(),
            pool_asset.into(),
            reward_asset.into(),
            is_farm,
            multiplier,
            deposit_fee,
            is_core
        );
    }
    verify {
        assert_last_event::<T>(demeter_farming_platform::Event::<T>::PoolAdded(authority, pool_asset.into(), reward_asset.into(), is_farm).into());
    }

    deposit {
        let authority = AuthorityAccount::<T>::get();
        let team_account = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&authority);
        let reward_asset = CERES_ASSET_ID;
        let is_farm = false;
        let multiplier = 1;
        let deposit_fee = balance!(0.04);
        let is_core = true;
        let token_per_block = balance!(1);
        let farms_allocation = balance!(0.6);
        let staking_allocation = balance!(0.2);
        let team_allocation = balance!(0.2);
        let pooled_tokens = balance!(10);

        setup_benchmark_assets_only::<T>()?;

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(team_account.clone()).into(),
            reward_asset.into(),
            authority.clone(),
            balance!(20000)
        );

        let _ = DemeterFarmingPlatform::<T>::register_token(
            RawOrigin::Signed(authority.clone()).into(),
            reward_asset.into(),
            token_per_block,
            farms_allocation,
            staking_allocation,
            team_allocation,
            team_account,
        );

        let _ = DemeterFarmingPlatform::<T>::add_pool(
            RawOrigin::Signed(authority.clone()).into(),
            reward_asset.into(),
            reward_asset.into(),
            is_farm,
            multiplier,
            deposit_fee,
            is_core,
        );
    }: {
        let _ = DemeterFarmingPlatform::<T>::deposit(
            RawOrigin::Signed(authority.clone()).into(),
            reward_asset.into(),
            reward_asset.into(),
            is_farm,
            pooled_tokens
        );
    }
    verify {
        assert_last_event::<T>(demeter_farming_platform::Event::<T>::Deposited(authority, reward_asset.into(), reward_asset.into(), is_farm, balance!(9.6)).into());
    }

    get_rewards {
        let caller = alice::<T>();
        let authority = AuthorityAccount::<T>::get();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let reward_asset = CERES_ASSET_ID;
        let is_farm = false;
        let pallet_account: AccountIdOf<T> = PalletId(*b"deofarms").into_account();

        setup_benchmark_assets_only::<T>()?;

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(caller.clone()).into(),
            reward_asset.into(),
            pallet_account.clone(),
            balance!(20000)
        );

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(caller.clone()).into(),
            reward_asset.into(),
            caller.clone(),
            balance!(1000)
        );

        // Register token
        let _ = DemeterFarmingPlatform::<T>::register_token(
            RawOrigin::Signed(authority.clone()).into(),
            reward_asset.into(),
            balance!(1),
            balance!(0.6),
            balance!(0.2),
            balance!(0.2),
            caller.clone()
        );

        // Add pool
        let _ = DemeterFarmingPlatform::<T>::add_pool(
            RawOrigin::Signed(authority.clone()).into(),
            reward_asset.into(),
            reward_asset.into(),
            is_farm,
            2,
            balance!(0),
            true
        );

        let _ = DemeterFarmingPlatform::<T>::deposit(
            RawOrigin::Signed(caller.clone()).into(),
            reward_asset.into(),
            reward_asset.into(),
            is_farm,
            balance!(10)
        );

        run_to_block::<T>(16201);

        let user_infos = UserInfos::<T>::get(&caller);
        let mut rewards = balance!(0);
        for user_info in user_infos {
            rewards = user_info.rewards;
        }

    }: {
        let _ = DemeterFarmingPlatform::<T>::get_rewards(
            RawOrigin::Signed(caller.clone()).into(),
            reward_asset.into(),
            reward_asset.into(),
            is_farm
        );
    }
    verify {
        assert_last_event::<T>(demeter_farming_platform::Event::<T>::RewardWithdrawn(caller, rewards, reward_asset.into(), reward_asset.into(), is_farm).into());
    }

    withdraw {
        let caller = alice::<T>();
        let authority = AuthorityAccount::<T>::get();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let is_farm = false;
        let reward_asset = CERES_ASSET_ID;
        let pallet_account: AccountIdOf<T> = PalletId(*b"deofarms").into_account();

        setup_benchmark_assets_only::<T>()?;

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(caller.clone()).into(),
            reward_asset.into(),
            caller.clone(),
            balance!(20000)
        );

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(caller.clone()).into(),
            reward_asset.into(),
            pallet_account.clone(),
            balance!(20000)
        );

        // Register token
        let _ = DemeterFarmingPlatform::<T>::register_token(
            RawOrigin::Signed(authority.clone()).into(),
            reward_asset.into(),
            balance!(1),
            balance!(0.6),
            balance!(0.2),
            balance!(0.2),
            caller.clone()
        );

        // Add pool
        let _ = DemeterFarmingPlatform::<T>::add_pool(
            RawOrigin::Signed(authority.clone()).into(),
            reward_asset.into(),
            reward_asset.into(),
            is_farm,
            2,
            balance!(0),
            true
        );

        let pooled_tokens = balance!(30);

        // Deposit
        let _ = DemeterFarmingPlatform::<T>::deposit(
            RawOrigin::Signed(caller.clone()).into(),
            reward_asset.into(),
            reward_asset.into(),
            is_farm,
            pooled_tokens
        );
    }: {
        let _ = DemeterFarmingPlatform::<T>::withdraw(
            RawOrigin::Signed(caller.clone()).into(),
            reward_asset.into(),
            reward_asset.into(),
            pooled_tokens,
            is_farm
        );
    }
    verify {
        assert_last_event::<T>(demeter_farming_platform::Event::<T>::Withdrawn(caller, pooled_tokens, reward_asset.into(), reward_asset.into(), is_farm).into());
    }

    remove_pool {
        let caller = AuthorityAccount::<T>::get();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let is_farm = true;
        let team_account = alice::<T>();

        // Register token
        let _ = DemeterFarmingPlatform::<T>::register_token(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            balance!(1),
            balance!(0.6),
            balance!(0.2),
            balance!(0.2),
            team_account
        );

        // Add pool
        let _ = DemeterFarmingPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            CERES_ASSET_ID.into(),
            is_farm,
            2,
            balance!(0.2),
            true
        );
    }: {
        let _ = DemeterFarmingPlatform::<T>::remove_pool(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            CERES_ASSET_ID.into(),
            is_farm
        );
    }
    verify {
        assert_last_event::<T>(demeter_farming_platform::Event::<T>::PoolRemoved(caller, XOR.into(), CERES_ASSET_ID.into(), is_farm).into());
    }

    change_pool_multiplier {
        let caller = AuthorityAccount::<T>::get();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let is_farm = true;
        let new_multiplier = 2;
        let team_account = alice::<T>();

        // Register token
        let _ = DemeterFarmingPlatform::<T>::register_token(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            balance!(1),
            balance!(0.6),
            balance!(0.2),
            balance!(0.2),
            team_account
        );

        // Add pool
        let _ = DemeterFarmingPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            CERES_ASSET_ID.into(),
            is_farm,
            1,
            balance!(0.2),
            true
        );

    }: {
        let _ = DemeterFarmingPlatform::<T>::change_pool_multiplier(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            CERES_ASSET_ID.into(),
            is_farm,
            new_multiplier
        );
    }
    verify {
        assert_last_event::<T>(demeter_farming_platform::Event::<T>::MultiplierChanged(caller, XOR.into(), CERES_ASSET_ID.into(), is_farm, new_multiplier).into());
    }

    change_pool_deposit_fee {
        let caller = AuthorityAccount::<T>::get();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let is_farm = true;
        let deposit_fee = balance!(0.6);
        let team_account = alice::<T>();

        // Register token
        let _ = DemeterFarmingPlatform::<T>::register_token(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            balance!(1),
            balance!(0.6),
            balance!(0.2),
            balance!(0.2),
            team_account
        );

        // Add pool
        let _ = DemeterFarmingPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            CERES_ASSET_ID.into(),
            true,
            2,
            balance!(0.4),
            true
        );
    }: {
        let _ = DemeterFarmingPlatform::<T>::change_pool_deposit_fee(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            CERES_ASSET_ID.into(),
            is_farm,
            deposit_fee
        );
    }
    verify {
        assert_last_event::<T>(demeter_farming_platform::Event::<T>::DepositFeeChanged(caller, XOR.into(), CERES_ASSET_ID.into(), is_farm, deposit_fee).into());
    }

    change_token_info {
        let caller = AuthorityAccount::<T>::get();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let token_per_block = balance!(1);
        let farms_allocation = balance!(0.2);
        let staking_allocation = balance!(0.4);
        let team_allocation = balance!(0.4);
        let deposit_fee = balance!(1);
        let team_account = alice::<T>();

        // Register token
        let _ = DemeterFarmingPlatform::<T>::register_token(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            token_per_block,
            farms_allocation,
            staking_allocation,
            team_allocation,
            team_account.clone()
        );
    }: {
        let _ = DemeterFarmingPlatform::<T>::change_token_info(
            RawOrigin::Signed(caller.clone()).into(),
            CERES_ASSET_ID.into(),
            token_per_block,
            farms_allocation,
            staking_allocation,
            team_allocation,
            team_account
        );
    }
    verify {
        assert_last_event::<T>(demeter_farming_platform::Event::<T>::TokenInfoChanged(caller, CERES_ASSET_ID.into()).into());
    }
}

frame_benchmarking::impl_benchmark_test_suite!(
    Pallet,
    crate::mock::ExtBuilder::default().build(),
    crate::mock::Runtime,
);
