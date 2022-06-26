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

use common::{fixed, AssetName, AssetSymbol, DAI, DEFAULT_BALANCE_PRECISION, USDT, XOR};

use crate::Pallet as MBCPool;
use assets::Pallet as Assets;
use permissions::Pallet as Permissions;
use pool_xyk::Pallet as XYKPool;
use trading_pair::Pallet as TradingPair;

pub const DEX: DEXId = DEXId::Polkaswap;

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

fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Module::<T>::inc_providers(&owner);
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();

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
    Assets::<T>::mint_to(&XOR.into(), &owner.clone(), &owner.clone(), balance!(5000)).unwrap();
    Assets::<T>::mint_to(
        &DAI.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &VAL.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();

    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), DAI.into())
        .unwrap();
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), VAL.into())
        .unwrap();

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        DAI.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )
    .unwrap();
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        VAL.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )
    .unwrap();

    Ok(())
}

fn add_pending<T: Config>(n: u32) {
    let mut pending = Vec::new();
    for _i in 0..n {
        pending.push((DAI.into(), balance!(1)))
    }
    PendingFreeReserves::<T>::set(pending);
}

benchmarks! {
    initialize_pool {
        let caller = alice::<T>();
        frame_system::Module::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
        Assets::<T>::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None
        ).unwrap();
        TradingPair::<T>::register(
            RawOrigin::Signed(caller.clone()).into(),
            common::DEXId::Polkaswap.into(),
            XOR.into(),
            USDT.into()
        ).unwrap();
    }: {
        Module::<T>::initialize_pool(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into()
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::PoolInitialized(common::DEXId::Polkaswap.into(), USDT.into()).into())
    }

    set_reference_asset {
        let caller = alice::<T>();
        frame_system::Module::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
        Assets::<T>::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None
        ).unwrap();
    }: {
        Module::<T>::set_reference_asset(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into()
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::ReferenceAssetChanged(USDT.into()).into())
    }

    set_optional_reward_multiplier {
        let caller = alice::<T>();
        frame_system::Module::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
        Assets::<T>::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"TESTUSD".to_vec()),
            AssetName(b"USD".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None
        ).unwrap();
        TradingPair::<T>::register(RawOrigin::Signed(caller.clone()).into(), common::DEXId::Polkaswap.into(), XOR.into(), USDT.into()).unwrap();
        MBCPool::<T>::initialize_pool(RawOrigin::Signed(caller.clone()).into(), USDT.into()).unwrap();
    }: {
        Module::<T>::set_optional_reward_multiplier(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into(),
            Some(fixed!(123))
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::OptionalRewardMultiplierUpdated(USDT.into(), Some(fixed!(123))).into())
    }

    on_initialize {
        let n in 0 .. 10;
        setup_benchmark::<T>().unwrap();
        add_pending::<T>(n);
    }: {
        Pallet::<T>::on_initialize(crate::RETRY_DISTRIBUTION_FREQUENCY.into());
    }
    verify {}

    set_price_change_config {
        let caller = alice::<T>();
        frame_system::Module::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
    }: {
        Module::<T>::set_price_change_config(
            RawOrigin::Root.into(),
            balance!(12),
            balance!(2600)
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::PriceChangeConfigChanged(balance!(12), balance!(2600)).into());
        assert_eq!(PriceChangeRate::<T>::get(), FixedWrapper::from(balance!(12)).get().unwrap());
        assert_eq!(PriceChangeStep::<T>::get(), FixedWrapper::from(balance!(2600)).get().unwrap());
    }

    set_price_bias {
        let caller = alice::<T>();
        frame_system::Module::<T>::inc_providers(&caller);
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        Permissions::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
    }: {
        Module::<T>::set_price_bias(
            RawOrigin::Root.into(),
            balance!(253)
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::PriceBiasChanged(balance!(253)).into());
        assert_eq!(InitialPrice::<T>::get(), FixedWrapper::from(balance!(253)).get().unwrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    #[ignore]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(test_benchmark_initialize_pool::<Runtime>());
            assert_ok!(test_benchmark_set_reference_asset::<Runtime>());
            assert_ok!(test_benchmark_set_optional_reward_multiplier::<Runtime>());
            assert_ok!(test_benchmark_set_price_bias::<Runtime>());
            assert_ok!(test_benchmark_set_price_change_config::<Runtime>());
            assert_ok!(test_benchmark_on_initialize::<Runtime>());
        });
    }
}
