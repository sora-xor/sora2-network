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

use codec::Decode;
use common::prelude::{Balance, SwapAmount};
use common::{
    balance, AssetId32, AssetName, AssetSymbol, DEXId, FilterMode, LiquiditySourceType,
    PriceVariant, DAI, DEFAULT_BALANCE_PRECISION, DOT, PSWAP, USDT, VAL, XOR, XSTUSD,
};
use frame_benchmarking::{benchmarks, Zero};
use frame_support::traits::Get;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use liquidity_proxy::{BatchReceiverInfo, Call};
use sp_std::prelude::*;

use assets::Pallet as Assets;
use multicollateral_bonding_curve_pool::Pallet as MBCPool;
use permissions::Pallet as Permissions;
use pool_xyk::Pallet as XYKPool;
use scale_info::prelude::string::ToString;
use trading_pair::Pallet as TradingPair;

pub const DEX: DEXId = DEXId::Polkaswap;

#[cfg(test)]
mod mock;

fn assert_last_event<T: liquidity_proxy::Config>(
    generic_event: <T as liquidity_proxy::Config>::Event,
) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

pub struct Pallet<T: Config>(liquidity_proxy::Pallet<T>);
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
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

fn generic_account<T: Config>(seed_1: u32, seed_2: u32) -> T::AccountId {
    let raw_account_id: [u8; 32] = [
        seed_1.to_be_bytes().to_vec(),
        seed_2.to_be_bytes().to_vec(),
        [0u8; 24].to_vec(),
    ]
    .concat()
    .try_into()
    .expect("Failed to generate account id byte array");
    T::AccountId::decode(&mut &raw_account_id[..]).expect("Failed to create a new account id")
}

// Prepare Runtime for running benchmarks
fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);
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
        balance!(1000),
        balance!(2000),
    )
    .unwrap();
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        VAL.into(),
        balance!(1000),
        balance!(2000),
        balance!(1000),
        balance!(2000),
    )
    .unwrap();
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        DAI.into(),
        balance!(1000),
        balance!(2000),
        balance!(1000),
        balance!(2000),
    )
    .unwrap();
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        PSWAP.into(),
        balance!(1000),
        balance!(2000),
        balance!(1000),
        balance!(2000),
    )
    .unwrap();
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        USDT.into(),
        balance!(1000),
        balance!(2000),
        balance!(1000),
        balance!(2000),
    )
    .unwrap();

    MBCPool::<T>::initialize_pool(owner_origin.clone(), USDT.into()).unwrap();

    for _ in 0..price_tools::AVG_BLOCK_SPAN {
        price_tools::Pallet::<T>::average_prices_calculation_routine(PriceVariant::Buy);
        price_tools::Pallet::<T>::average_prices_calculation_routine(PriceVariant::Sell);
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
        liquidity_proxy::Pallet::<T>::swap(
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
        liquidity_proxy::Pallet::<T>::swap(
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
            Into::<u128>::into(initial_to_balance) + balance!(0.999999999999977496)
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
            Into::<u128>::into(initial_target_balance) + balance!(99.999999999999999998)
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

    enable_liquidity_source {
        setup_benchmark::<T>()?;
        liquidity_proxy::Pallet::<T>::disable_liquidity_source(
            RawOrigin::Root.into(),
            LiquiditySourceType::XSTPool
        )?;
    }: {
        liquidity_proxy::Pallet::<T>::enable_liquidity_source(
            RawOrigin::Root.into(),
            LiquiditySourceType::XSTPool
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(
            liquidity_proxy::Event::<T>::LiquiditySourceEnabled(
                LiquiditySourceType::XSTPool
            ).into()
        );
    }

    disable_liquidity_source {
        setup_benchmark::<T>()?;
    }: {
        liquidity_proxy::Pallet::<T>::disable_liquidity_source(
            RawOrigin::Root.into(),
            LiquiditySourceType::XSTPool
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(
            liquidity_proxy::Event::<T>::LiquiditySourceDisabled(
                LiquiditySourceType::XSTPool
            ).into()
        );
    }

    swap_transfer_batch {
        let n in 1..10; // number of output assets
        let m in 10..100; // full number of receivers

        let k = m/n;

        let caller = alice::<T>();
        let caller_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(caller.clone()).into();

        let mut receivers: Vec<(T::AssetId, Vec<BatchReceiverInfo<T>>)> = Vec::new();
        setup_benchmark::<T>()?;
        for i in 0..n {
            let raw_asset_id = [[3u8; 28].to_vec(), i.to_be_bytes().to_vec()]
                .concat()
                .try_into()
                .expect("Failed to cast vector to [u8; 32]");
            let new_asset_id = AssetId32::from_bytes(raw_asset_id);
            let asset_symbol = {
                let mut asset_symbol_prefix: Vec<u8> = "TEST".into();
                let asset_symbol_remainder: Vec<u8> = i.to_string().into();
                asset_symbol_prefix.extend_from_slice(&asset_symbol_remainder);
                asset_symbol_prefix
            };
            let asset_name = {
                let mut asset_name_prefix: Vec<u8> = "Test".into();
                let asset_name_remainder: Vec<u8> = i.to_string().into();
                asset_name_prefix.extend_from_slice(&asset_name_remainder);
                asset_name_prefix
            };

            Assets::<T>::register_asset_id(
                caller.clone(),
                new_asset_id.into(),
                AssetSymbol(asset_symbol),
                AssetName(asset_name),
                DEFAULT_BALANCE_PRECISION,
                Balance::zero(),
                true,
                None,
                None,
            ).expect("Failed to register a new asset id");

            Assets::<T>::mint_to(
                &new_asset_id.into(),
                &caller.clone(),
                &caller.clone(),
                balance!(500000),
            ).expect("Failed to mint a new asset");

            TradingPair::<T>::register(
                caller_origin.clone(),
                DEX.into(),
                XOR.into(),
                new_asset_id.into()
            ).expect("Failed to register a trading pair");

            XYKPool::<T>::initialize_pool(
                caller_origin.clone(),
                DEX.into(),
                XOR.into(),
                new_asset_id.into()
            ).expect("Failed to initialize pool");

            XYKPool::<T>::deposit_liquidity(
                caller_origin.clone(),
                DEX.into(),
                XOR.into(),
                new_asset_id.into(),
                balance!(10000),
                balance!(10000),
                balance!(10000),
                balance!(10000),
            ).expect("Failed to deposit liquidity");
            let recv_batch: Vec<BatchReceiverInfo<T>> = (0..k).into_iter().map(|recv_num| {
                let account_id = generic_account::<T>(i, recv_num);
                let target_amount = balance!(0.1);
                BatchReceiverInfo {account_id, target_amount}
            }).collect();
            receivers.push((new_asset_id.into(), recv_batch));
        }
        let max_input_amount = balance!(k*n + 100);
    }: {
        liquidity_proxy::Pallet::<T>::swap_transfer_batch(
            caller_origin,
            receivers.clone(),
            DEX.into(),
            XOR.into(),
            max_input_amount,
            [LiquiditySourceType::XYKPool].to_vec(),
            FilterMode::AllowSelected,
        ).unwrap();
    } verify {
        receivers.into_iter().for_each(|(asset_id, recv_batch)| {
            recv_batch.into_iter().for_each(|batch| {
                let BatchReceiverInfo {account_id, target_amount} = batch;
                assert_eq!(Assets::<T>::free_balance(&asset_id, &account_id).unwrap(), target_amount);
            })
        });
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
            assert_ok!(Pallet::<Runtime>::test_benchmark_swap_exact_input_primary_only());
            assert_ok!(Pallet::<Runtime>::test_benchmark_swap_exact_output_primary_only());
            assert_ok!(Pallet::<Runtime>::test_benchmark_swap_exact_input_secondary_only());
            assert_ok!(Pallet::<Runtime>::test_benchmark_swap_exact_output_secondary_only());
            assert_ok!(Pallet::<Runtime>::test_benchmark_swap_exact_input_multiple());
            assert_ok!(Pallet::<Runtime>::test_benchmark_swap_exact_output_multiple());
            assert_ok!(Pallet::<Runtime>::test_benchmark_enable_liquidity_source());
            assert_ok!(Pallet::<Runtime>::test_benchmark_disable_liquidity_source());
            assert_ok!(Pallet::<Runtime>::test_benchmark_swap_transfer_batch());
        });
    }
}
