//! Demeter farming platform module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{balance, FromGenericPair, CERES_ASSET_ID, XOR};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_runtime::traits::AccountIdConversion;
use sp_std::prelude::*;

use crate::Pallet as DemeterFarmingPlatform;
use assets::Module as Assets;
use sp_runtime::ModuleId;
use technical::Module as Technical;

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

benchmarks! {
    register_token {
        let authority = pallet::AuthorityAccount::<T>::get();
        let team_account = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&authority);
        let reward_asset = XOR;
        let token_per_block = balance!(1);
        let farms_allocation = balance!(0.6);
        let staking_allocation = balance!(0.2);
        let team_allocation = balance!(0.2);
    }: _(
        RawOrigin::Signed(authority.clone()),
        reward_asset.into(),
        token_per_block,
        farms_allocation,
        staking_allocation,
        team_allocation,
        team_account
    )
    verify {
        assert_last_event::<T>(Event::TokenRegistered(authority, reward_asset.into()).into());
    }

    add_pool {
        let authority = pallet::AuthorityAccount::<T>::get();
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
    }: _(
        RawOrigin::Signed(authority.clone()),
        pool_asset.into(),
        reward_asset.into(),
        is_farm,
        multiplier,
        deposit_fee,
        is_core
    )
    verify {
        assert_last_event::<T>(Event::PoolAdded(authority, pool_asset.into(), reward_asset.into(), is_farm).into());
    }

    deposit {
        let authority = pallet::AuthorityAccount::<T>::get();
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

        let assets_and_permissions_tech_account_id =
            T::TechAccountId::from_generic_pair(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
        let assets_and_permissions_account_id =
            Technical::<T>::tech_account_id_to_account_id(
                &assets_and_permissions_tech_account_id,
            ).unwrap();

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
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
    }: _(
        RawOrigin::Signed(authority.clone()),
        reward_asset.into(),
        reward_asset.into(),
        is_farm,
        pooled_tokens
    )
    verify {
        assert_last_event::<T>(Event::Deposited(authority, reward_asset.into(), reward_asset.into(), is_farm, balance!(9.6)).into());
    }

    get_rewards {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let rewards = balance!(100);
        let is_farm = true;
        let pallet_account: AccountIdOf<T> = ModuleId(*b"deofarms").into_account();
        let team_account = alice::<T>();

        let assets_and_permissions_tech_account_id =
            T::TechAccountId::from_generic_pair(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
        let assets_and_permissions_account_id =
            Technical::<T>::tech_account_id_to_account_id(
                &assets_and_permissions_tech_account_id,
            ).unwrap();

        let _ = Assets::<T>::mint(
            RawOrigin::Signed(assets_and_permissions_account_id.clone()).into(),
            CERES_ASSET_ID.into(),
            pallet_account.clone(),
            balance!(20000)
        );

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
            balance!(0),
            true
        );

        let user_info = UserInfo {
            pool_asset: XOR.into(),
            reward_asset: CERES_ASSET_ID.into(),
            is_farm,
            pooled_tokens: balance!(1000),
            rewards,
        };

        UserInfos::<T>::append(&caller, user_info);

    }: _(RawOrigin::Signed(caller.clone()), XOR.into(), CERES_ASSET_ID.into(), is_farm)
    verify {
        assert_last_event::<T>(Event::RewardWithdrawn(caller, rewards, XOR.into(), CERES_ASSET_ID.into(), is_farm).into());
    }

    withdraw {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let is_farm = true;
        let pallet_account: AccountIdOf<T> = ModuleId(*b"deofarms").into_account();
        let team_account = alice::<T>();

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
            CERES_ASSET_ID.into(),
            pallet_account.clone(),
            balance!(20000)
        );

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
            balance!(0),
            true
        );

        let pooled_tokens = balance!(30);

        // Deposit
        let _ = DemeterFarmingPlatform::<T>::deposit(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            CERES_ASSET_ID.into(),
            is_farm,
            pooled_tokens
        );
    }: _(RawOrigin::Signed(caller.clone()), XOR.into(), CERES_ASSET_ID.into(), pooled_tokens, is_farm)
    verify {
        assert_last_event::<T>(Event::Withdrawn(caller, pooled_tokens, XOR.into(), CERES_ASSET_ID.into(), is_farm).into());
    }

    remove_pool{
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

        //Add pool
        let _ = DemeterFarmingPlatform::<T>::add_pool(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            CERES_ASSET_ID.into(),
            true,
            2,
            balance!(0.2),
            true
        );
    }: _(RawOrigin::Signed(caller.clone()), XOR.into(), CERES_ASSET_ID.into(), is_farm)
    verify {
        assert_last_event::<T>(Event::PoolRemoved(caller, XOR.into(), CERES_ASSET_ID.into(), is_farm).into());
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
            XOR.into(),
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
            balance!(0.2),
            true
        );

    }: _(RawOrigin::Signed(caller.clone()), XOR.into(), CERES_ASSET_ID.into(), is_farm, new_multiplier)
    verify {
        assert_last_event::<T>(Event::MultiplierChanged(caller, XOR.into(), CERES_ASSET_ID.into(), is_farm, new_multiplier).into());
    }

    change_pool_deposit_fee {
        let caller = AuthorityAccount::<T>::get();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let is_farm = true;
        let deposit_fee = balance!(1);
        let team_account = alice::<T>();

        // Register token
        let _ = DemeterFarmingPlatform::<T>::register_token(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
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
    }: _(RawOrigin::Signed(caller.clone()), XOR.into(), CERES_ASSET_ID.into(), is_farm, deposit_fee)
    verify {
        assert_last_event::<T>(Event::DepositFeeChanged(caller, XOR.into(), CERES_ASSET_ID.into(), is_farm, deposit_fee).into());
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
    }: _(RawOrigin::Signed(caller.clone()), CERES_ASSET_ID.into(), token_per_block, farms_allocation, staking_allocation, team_allocation, team_account)
    verify {
        assert_last_event::<T>(Event::TokenInfoChanged(caller, CERES_ASSET_ID.into()).into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(test_benchmark_register_token::<Runtime>());
            assert_ok!(test_benchmark_add_pool::<Runtime>());
            assert_ok!(test_benchmark_deposit::<Runtime>());
            assert_ok!(test_benchmark_get_rewards::<Runtime>());
            assert_ok!(test_benchmark_withdraw::<Runtime>());
            assert_ok!(test_benchmark_remove_pool::<Runtime>());
            assert_ok!(test_benchmark_change_pool_multiplier::<Runtime>());
            assert_ok!(test_benchmark_change_pool_deposit_fee::<Runtime>());
            assert_ok!(test_benchmark_change_token_info::<Runtime>());
        });
    }
}
