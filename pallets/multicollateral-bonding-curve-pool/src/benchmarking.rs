// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! Multicollateral bonding curve pool module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_support::traits::OnInitialize;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

#[cfg(not(test))]
use price_tools::AVG_BLOCK_SPAN;

use common::prelude::SwapAmount;
use common::{fixed, AssetName, AssetSymbol, DAI, DEFAULT_BALANCE_PRECISION, USDT, XOR};

use crate::Pallet as MBCPool;
use permissions::Pallet as Permissions;
use pool_xyk::Pallet as XYKPool;

pub const DEX: DEXId = DEXId::Polkaswap;

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

fn setup_benchmark<T>() -> Result<(), &'static str>
where
    T: Config + pool_xyk::Config,
{
    let owner = alice::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);
    #[cfg(test)]
    crate::mock::MockDEXApi::init_without_reserves().unwrap();

    // Grant permissions to self in case they haven't been explicitly given in genesis config
    Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::MINT,
        permissions::Scope::Unlimited,
    )
    .unwrap();
    Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::BURN,
        permissions::Scope::Unlimited,
    )
    .unwrap();
    T::AssetManager::mint_to(&XOR.into(), &owner, &owner, balance!(3000)).unwrap();
    register_tbc_for_asset::<T>(DAI.into());
    register_tbc_for_asset::<T>(VAL.into());
    Ok(())
}

fn gen_asset_id<T: Config>(n: u32) -> AssetIdOf<T> {
    let asset_id = T::AssetManager::gen_asset_id_from_any(&("mbc_bench", n));
    let owner = alice::<T>();
    T::AssetManager::register_asset_id(
        owner,
        asset_id,
        common::AssetSymbol(b"TEST".to_vec()),
        common::AssetName(b"TEST".to_vec()),
        common::DEFAULT_BALANCE_PRECISION,
        0,
        true,
        common::AssetType::Regular,
        None,
        None,
    )
    .unwrap();
    <T as Config>::TradingPairSourceManager::register_pair(DEX.into(), XOR.into(), asset_id)
        .unwrap();
    asset_id
}

fn register_tbc_for_asset<T>(asset_id: AssetIdOf<T>)
where
    T: Config + pool_xyk::Config,
{
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::RuntimeOrigin =
        RawOrigin::Signed(owner.clone()).into();

    T::AssetManager::mint_to(&XOR.into(), &owner, &owner, balance!(1000)).unwrap();
    T::AssetManager::mint_to(&asset_id, &owner, &owner, balance!(50000000)).unwrap();

    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), asset_id).unwrap();

    XYKPool::<T>::deposit_liquidity(
        owner_origin,
        DEX.into(),
        XOR.into(),
        asset_id,
        balance!(1000),
        balance!(2000),
        balance!(1000),
        balance!(2000),
    )
    .unwrap();
}

fn add_pending<T>(n: u32)
where
    T: Config + pool_xyk::Config,
{
    let mut pending = sp_std::collections::btree_map::BTreeMap::<AssetIdOf<T>, Balance>::new();
    for i in 0..n {
        let asset_id = gen_asset_id::<T>(i);
        register_tbc_for_asset::<T>(asset_id);
        pending.insert(DAI.into(), balance!(1));
    }
    PendingFreeReserves::<T>::insert(BlockNumberFor::<T>::from(0u32), pending);
}

#[cfg(not(test))]
fn incoming_spot_price<T: price_tools::Config>(asset_id: AssetIdOf<T>, price: Balance) {
    price_tools::PriceInfos::<T>::mutate(&asset_id, |opt_val| {
        let val = opt_val.as_mut().unwrap();
        val.price_mut_of(PriceVariant::Buy)
            .incoming_spot_price(price, PriceVariant::Buy, &price_tools::DEFAULT_PARAMETERS)
            .unwrap();
        val.price_mut_of(PriceVariant::Sell)
            .incoming_spot_price(price, PriceVariant::Sell, &price_tools::DEFAULT_PARAMETERS)
            .unwrap();
    })
}

benchmarks! {
    where_clause {
        where T: price_tools::Config
    }

    initialize_pool {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
        T::AssetManager::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            common::AssetType::Regular,
            None,
            None
        ).unwrap();
        <T as Config>::TradingPairSourceManager::register_pair(
            dex_id,
            XOR.into(),
            USDT.into()
        ).unwrap();
    }: {
        Pallet::<T>::initialize_pool(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into()
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::PoolInitialized(dex_id, USDT.into()).into())
    }

    set_reference_asset {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
        T::AssetManager::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            common::AssetType::Regular,
            None,
            None
        ).unwrap();
    }: {
        Pallet::<T>::set_reference_asset(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into()
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::ReferenceAssetChanged(USDT.into()).into())
    }

    set_optional_reward_multiplier {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
        T::AssetManager::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            common::AssetType::Regular,
            None,
            None
        ).unwrap();
        <T as Config>::TradingPairSourceManager::register_pair(dex_id, XOR.into(), USDT.into()).unwrap();
        MBCPool::<T>::initialize_pool(RawOrigin::Signed(caller.clone()).into(), USDT.into()).unwrap();
    }: {
        Pallet::<T>::set_optional_reward_multiplier(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into(),
            Some(fixed!(123))
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::OptionalRewardMultiplierUpdated(USDT.into(), Some(fixed!(123))).into())
    }

    on_initialize {
        let n in 0 .. 10;
        setup_benchmark::<T>().unwrap();
        add_pending::<T>(n);
    }: {
        Pallet::<T>::on_initialize(0u32.into());
    }
    verify {}

    set_price_change_config {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
    }: {
        Pallet::<T>::set_price_change_config(
            RawOrigin::Root.into(),
            balance!(12),
            balance!(2600)
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::PriceChangeConfigChanged(balance!(12), balance!(2600)).into());
        assert_eq!(PriceChangeRate::<T>::get(), FixedWrapper::from(balance!(12)).get().unwrap());
        assert_eq!(PriceChangeStep::<T>::get(), FixedWrapper::from(balance!(2600)).get().unwrap());
    }

    set_price_bias {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
    }: {
        Pallet::<T>::set_price_bias(
            RawOrigin::Root.into(),
            balance!(253)
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::PriceBiasChanged(balance!(253)).into());
        assert_eq!(InitialPrice::<T>::get(), FixedWrapper::from(balance!(253)).get().unwrap());
    }

    quote {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();

        T::AssetManager::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            balance!(50000000),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        <T as Config>::TradingPairSourceManager::register_pair(
            dex_id,
            XOR.into(),
            USDT.into(),
        )
        .unwrap();
        Pallet::<T>::initialize_pool(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into()
        ).unwrap();

        #[cfg(not(test))]
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price::<T>(DAI.into(), balance!(1));
            incoming_spot_price::<T>(USDT.into(), balance!(1));
        }
        let amount = SwapAmount::WithDesiredInput {
            desired_amount_in: balance!(1),
            min_amount_out: balance!(0),
        };
    }: {
        Pallet::<T>::quote(&dex_id, &USDT.into(), &XOR.into(), amount.into(), true).unwrap();
    }
    verify {
        // can't check, nothing is changed
    }

    step_quote {
        let a in 10..1000;

        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();

        T::AssetManager::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            balance!(50000000),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        <T as Config>::TradingPairSourceManager::register_pair(
            dex_id,
            XOR.into(),
            USDT.into(),
        )
        .unwrap();
        Pallet::<T>::initialize_pool(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into()
        ).unwrap();

        #[cfg(not(test))]
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price::<T>(DAI.into(), balance!(1));
            incoming_spot_price::<T>(USDT.into(), balance!(1));
        }
        let amount = SwapAmount::WithDesiredInput {
            desired_amount_in: balance!(1000),
            min_amount_out: balance!(0),
        };
    }: {
        Pallet::<T>::step_quote(&dex_id, &USDT.into(), &XOR.into(), amount.into(), a as usize, true).unwrap();
    }
    verify {
        // can't check, nothing is changed
    }

    exchange {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();

        T::AssetManager::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            balance!(50000000),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        T::AssetManager::mint_to(
            &USDT.into(),
            &caller.clone(),
            &caller.clone(),
            balance!(50000000),
        )
        .unwrap();
        <T as Config>::TradingPairSourceManager::register_pair(
            dex_id,
            XOR.into(),
            USDT.into(),
        )
        .unwrap();
        Pallet::<T>::initialize_pool(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into()
        ).unwrap();

        #[cfg(not(test))]
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price::<T>(DAI.into(), balance!(1));
            incoming_spot_price::<T>(USDT.into(), balance!(1));
        }
        let amount = SwapAmount::WithDesiredInput {
            desired_amount_in: balance!(100),
            min_amount_out: balance!(0),
        };
        let initial_base_balance = <T as Config>::AssetInfoProvider::free_balance(&USDT.into(), &caller).unwrap();
    }: {
        // run only for benchmarks, not for tests
        // TODO: remake when unit tests use chainspec
        #[cfg(not(test))]
        Pallet::<T>::exchange(&caller, &caller, &dex_id, &USDT.into(), &XOR.into(), amount.into()).unwrap();
    }
    verify {
        #[cfg(not(test))]
        assert_eq!(
            Into::<u128>::into(<T as Config>::AssetInfoProvider::free_balance(&USDT.into(), &caller).unwrap()),
            Into::<u128>::into(initial_base_balance) - balance!(100)
        );
    }

    can_exchange {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
        T::AssetManager::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            common::AssetType::Regular,
            None,
            None
        ).unwrap();
        <T as Config>::TradingPairSourceManager::register_pair(
            dex_id,
            XOR.into(),
            USDT.into()
        ).unwrap();
        Pallet::<T>::initialize_pool(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into()
        ).unwrap();
    }: {
        assert!(MBCPool::<T>::can_exchange(
            &dex_id,
            &XOR.into(),
            &USDT.into(),
        ));
    }
    verify {
    }

    check_rewards {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();

        T::AssetManager::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        T::AssetManager::mint_to(
            &USDT.into(),
            &caller.clone(),
            &caller.clone(),
            balance!(50000000),
        )
        .unwrap();
        <T as Config>::TradingPairSourceManager::register_pair(
            dex_id,
            XOR.into(),
            USDT.into(),
        )
        .unwrap();
        Pallet::<T>::initialize_pool(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into()
        ).unwrap();

        #[cfg(not(test))]
        for _ in 1..=AVG_BLOCK_SPAN {
            incoming_spot_price::<T>(DAI.into(), balance!(1));
            incoming_spot_price::<T>(USDT.into(), balance!(1));
        }
    }: {
        let (rewards, _) = MBCPool::<T>::check_rewards(&dex_id, &USDT.into(), &XOR.into(), balance!(1000), balance!(10)).unwrap();
        assert!(!rewards.is_empty());
    }
    verify {
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::bench_init().build(),
        crate::mock::Runtime
    );
}
