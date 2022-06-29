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

//! Liquidity Proxy benchmarking module.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use liquidity_proxy::*;

use codec::Decode;
use common::prelude::{Balance, SwapAmount};
use common::{
    balance, AssetName, AssetSymbol, DEXId, FilterMode, LiquiditySourceType, DAI,
    DEFAULT_BALANCE_PRECISION, DOT, PSWAP, USDT, VAL, XOR, XSTUSD,
};
use frame_benchmarking::{benchmarks, Zero};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;

use assets::Pallet as Assets;
use multicollateral_bonding_curve_pool::Pallet as MBCPool;
use permissions::Pallet as Permissions;
use pool_xyk::Pallet as XYKPool;
use trading_pair::Pallet as TradingPair;

pub const DEX: DEXId = DEXId::Polkaswap;

#[cfg(test)]
mod mock;

pub struct Module<T: Config>(liquidity_proxy::Module<T>);
pub trait Config:
    liquidity_proxy::Config
    + pool_xyk::Config
    + multicollateral_bonding_curve_pool::Config
    + price_tools::Config
{
}

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

// Prepare Runtime for running benchmarks
fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Module::<T>::inc_providers(&owner);
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();
    let dex_id: T::DEXId = DEX.into();

    // Grant permissions to self in case they haven't been explicitly given in genesis config
    Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::MANAGE_DEX,
        permissions::Scope::Limited(common::hash(&dex_id)),
    )
    .unwrap();
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
        USDT.into(),
        AssetSymbol(b"TESTUSD".to_vec()),
        AssetName(b"USD".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::zero(),
        true,
        None,
        None,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"TESTDOT".to_vec()),
        AssetName(b"DOT".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::zero(),
        true,
        None,
        None,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        DAI.into(),
        AssetSymbol(b"DAI".to_vec()),
        AssetName(b"DAI".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::zero(),
        true,
        None,
        None,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        XSTUSD.into(),
        AssetSymbol(b"XSTUSD".to_vec()),
        AssetName(b"SORA Synthetic USD".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::zero(),
        true,
        None,
        None,
    );
    Assets::<T>::mint_to(&XOR.into(), &owner.clone(), &owner.clone(), balance!(50000)).unwrap();
    Assets::<T>::mint_to(
        &DOT.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &USDT.into(),
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
    Assets::<T>::mint_to(
        &DAI.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &PSWAP.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();

    let _ = TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into());
    let _ = TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), USDT.into());
    let _ = TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DAI.into());

    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())
        .unwrap();
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), VAL.into())
        .unwrap();
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), DAI.into())
        .unwrap();
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), PSWAP.into())
        .unwrap();
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), USDT.into())
        .unwrap();

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        DOT.into(),
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
        PSWAP.into(),
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
        USDT.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )
    .unwrap();

    MBCPool::<T>::initialize_pool(owner_origin.clone(), USDT.into()).unwrap();

    for _ in 0..price_tools::AVG_BLOCK_SPAN {
        price_tools::Module::<T>::average_prices_calculation_routine();
    }

    Ok(())
}

benchmarks! {
    swap_exact_input_primary_only {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let from_asset: T::AssetId = VAL.into();
        let to_asset: T::AssetId = XOR.into();
        let initial_from_balance = Assets::<T>::free_balance(&from_asset, &caller).unwrap();
    }: {
        liquidity_proxy::Module::<T>::swap(
            RawOrigin::Signed(caller.clone()).into(),
            DEX.into(),
            from_asset.clone(),
            to_asset.clone(),
            SwapAmount::with_desired_input(balance!(100), 0),
            [LiquiditySourceType::MulticollateralBondingCurvePool].into(),
            FilterMode::AllowSelected
        ).unwrap()
    }
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&from_asset, &caller).unwrap()),
            Into::<u128>::into(initial_from_balance) - balance!(100)
        );
    }

    swap_exact_output_primary_only {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let from_asset: T::AssetId = VAL.into();
        let to_asset: T::AssetId = XOR.into();
        let initial_to_balance = Assets::<T>::free_balance(&to_asset, &caller).unwrap();
    }: {
        liquidity_proxy::Module::<T>::swap(
            RawOrigin::Signed(caller.clone()).into(),
            DEX.into(),
            from_asset.clone(),
            to_asset.clone(),
            SwapAmount::with_desired_output(balance!(1), balance!(10000000)),
            [LiquiditySourceType::MulticollateralBondingCurvePool].into(),
            FilterMode::AllowSelected
        ).unwrap();
    }
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&to_asset, &caller).unwrap()),
            Into::<u128>::into(initial_to_balance) + balance!(1)
        );
    }

    swap_exact_input_secondary_only {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let base_asset: T::AssetId = <T as assets::Config>::GetBaseAssetId::get();
        let target_asset: T::AssetId = DOT.into();
        let initial_base_balance = Assets::<T>::free_balance(&base_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        base_asset.clone(),
        target_asset.clone(),
        SwapAmount::with_desired_input(balance!(100), 0),
        [LiquiditySourceType::XYKPool].into(),
        FilterMode::AllowSelected
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&base_asset, &caller).unwrap()),
            Into::<u128>::into(initial_base_balance) - balance!(100)
        );
    }

    swap_exact_output_secondary_only {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let base_asset: T::AssetId = <T as assets::Config>::GetBaseAssetId::get();
        let target_asset: T::AssetId = DOT.into();
        let initial_target_balance = Assets::<T>::free_balance(&target_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        base_asset.clone(),
        target_asset.clone(),
        SwapAmount::with_desired_output(balance!(100), balance!(100)),
        [LiquiditySourceType::XYKPool].into(),
        FilterMode::AllowSelected
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&target_asset, &caller).unwrap()),
            Into::<u128>::into(initial_target_balance) + balance!(100)
        );
    }

    swap_exact_input_multiple {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let from_asset: T::AssetId = VAL.into();
        let to_asset: T::AssetId = DOT.into();
        let initial_from_balance = Assets::<T>::free_balance(&from_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        from_asset.clone(),
        to_asset.clone(),
        SwapAmount::with_desired_input(balance!(1), 0),
        Vec::new(),
        FilterMode::Disabled
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&from_asset, &caller).unwrap()),
            Into::<u128>::into(initial_from_balance) - balance!(1)
        );
    }

    swap_exact_output_multiple {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let from_asset: T::AssetId = VAL.into();
        let to_asset: T::AssetId = DOT.into();
        let initial_to_balance = Assets::<T>::free_balance(&to_asset, &caller).unwrap();
    }: swap(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        from_asset.clone(),
        to_asset.clone(),
        SwapAmount::with_desired_output(balance!(1), balance!(10000000)),
        Vec::new(),
        FilterMode::Disabled
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&to_asset, &caller).unwrap()),
            Into::<u128>::into(initial_to_balance) + balance!(0.999999999999999996) // FIXME: this happens because routing via two pools can't guarantee exact amount
        );
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
            assert_ok!(test_benchmark_swap_exact_input_primary_only::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_output_primary_only::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_input_secondary_only::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_output_secondary_only::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_input_multiple::<Runtime>());
            assert_ok!(test_benchmark_swap_exact_output_multiple::<Runtime>());
        });
    }
}
