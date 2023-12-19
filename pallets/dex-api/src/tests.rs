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

use crate::mock::*;
use crate::{EnabledSourceTypes, Error, Pallet};
use assets::AssetIdOf;
use common::prelude::QuoteAmount;
use common::{
    balance, AccountIdOf, Balance, DexIdOf, LiquidityRegistry, LiquiditySource,
    LiquiditySourceFilter, LiquiditySourceId, LiquiditySourceType, DOT, XOR,
};
use frame_support::error::BadOrigin;
use frame_support::weights::Weight;
use frame_support::{assert_err, assert_ok};
use sp_runtime::DispatchError;
use strum::IntoEnumIterator;

type DexApi = Pallet<Runtime>;

#[test]
fn test_filter_empty_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list =
            DexApi::list_liquidity_sources(&XOR, &DOT, &LiquiditySourceFilter::empty(DEX_A_ID))
                .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool2),
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool3),
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool4),
            ]
        );
    })
}

#[test]
fn test_filter_with_forbidden_existing_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DexApi::list_liquidity_sources(
            &XOR,
            &DOT,
            &LiquiditySourceFilter::with_forbidden(
                DEX_A_ID,
                [
                    LiquiditySourceType::MockPool,
                    LiquiditySourceType::MockPool3,
                ]
                .into(),
            ),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool2),
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool4),
            ]
        );
    })
}

#[test]
fn test_filter_with_allowed_existing_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DexApi::list_liquidity_sources(
            &XOR,
            &DOT,
            &LiquiditySourceFilter::with_allowed(
                DEX_A_ID,
                [
                    LiquiditySourceType::MockPool,
                    LiquiditySourceType::MockPool2,
                ]
                .into(),
            ),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool2),
            ]
        );
    })
}

#[test]
fn test_different_reserves_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let res1 = crate::Pallet::<Runtime>::quote(
            &LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
            &XOR,
            &DOT,
            QuoteAmount::with_desired_input(balance!(100)),
            true,
        );
        assert_eq!(
            res1.unwrap().0.amount,
            balance!(136.851187324744592819) // for reserves: 5000 XOR, 7000 DOT, 30bp fee
        );
        let res2 = crate::Pallet::<Runtime>::quote(
            &LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool2),
            &XOR,
            &DOT,
            QuoteAmount::with_desired_input(balance!(100)),
            true,
        );
        assert_eq!(
            res2.unwrap().0.amount,
            balance!(114.415463055560109513) // for reserves: 6000 XOR, 7000 DOT, 30bp fee
        );
    })
}

#[test]
fn test_exchange_weight_correct() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let expected_weight = <<Runtime as crate::Config>::XSTPool as LiquiditySource<
            DexIdOf<Runtime>,
            AccountIdOf<Runtime>,
            AssetIdOf<Runtime>,
            Balance,
            DispatchError,
        >>::exchange_weight()
        .max(<<Runtime as crate::Config>::XYKPool as LiquiditySource<
            DexIdOf<Runtime>,
            AccountIdOf<Runtime>,
            AssetIdOf<Runtime>,
            Balance,
            DispatchError,
        >>::exchange_weight())
        .max(
            <<Runtime as crate::Config>::MulticollateralBondingCurvePool as LiquiditySource<
                DexIdOf<Runtime>,
                AccountIdOf<Runtime>,
                AssetIdOf<Runtime>,
                Balance,
                DispatchError,
            >>::exchange_weight(),
        )
        .max(<<Runtime as crate::Config>::OrderBook as LiquiditySource<
            DexIdOf<Runtime>,
            AccountIdOf<Runtime>,
            AssetIdOf<Runtime>,
            Balance,
            DispatchError,
        >>::exchange_weight());
        let got_weight = DexApi::exchange_weight();
        assert_eq!(expected_weight, got_weight);
    })
}

#[test]
fn test_exchange_weight_filtered_calculates() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let xyk_weight = <<Runtime as crate::Config>::XYKPool as LiquiditySource<
            DexIdOf<Runtime>,
            AccountIdOf<Runtime>,
            AssetIdOf<Runtime>,
            Balance,
            DispatchError,
        >>::exchange_weight();
        let multicollateral_weight =
            <<Runtime as crate::Config>::MulticollateralBondingCurvePool as LiquiditySource<
                DexIdOf<Runtime>,
                AccountIdOf<Runtime>,
                AssetIdOf<Runtime>,
                Balance,
                DispatchError,
            >>::exchange_weight();
        let xst_weight = <<Runtime as crate::Config>::XSTPool as LiquiditySource<
            DexIdOf<Runtime>,
            AccountIdOf<Runtime>,
            AssetIdOf<Runtime>,
            Balance,
            DispatchError,
        >>::exchange_weight();
        let order_book_weight = <<Runtime as crate::Config>::OrderBook as LiquiditySource<
            DexIdOf<Runtime>,
            AccountIdOf<Runtime>,
            AssetIdOf<Runtime>,
            Balance,
            DispatchError,
        >>::exchange_weight();

        assert_eq!(
            DexApi::exchange_weight_filtered([].into_iter()),
            Weight::zero()
        );
        assert_eq!(
            DexApi::exchange_weight_filtered([LiquiditySourceType::XYKPool].into_iter()),
            xyk_weight
        );
        assert_eq!(
            DexApi::exchange_weight_filtered(
                [LiquiditySourceType::MulticollateralBondingCurvePool].into_iter()
            ),
            multicollateral_weight
        );
        assert_eq!(
            DexApi::exchange_weight_filtered([LiquiditySourceType::XSTPool].into_iter()),
            xst_weight
        );
        assert_eq!(
            DexApi::exchange_weight_filtered([LiquiditySourceType::OrderBook].into_iter()),
            order_book_weight
        );
        assert_eq!(
            DexApi::exchange_weight_filtered(
                [LiquiditySourceType::XYKPool, LiquiditySourceType::XSTPool].into_iter()
            ),
            xyk_weight.max(xst_weight)
        );
        assert_eq!(
            DexApi::exchange_weight_filtered(
                [
                    LiquiditySourceType::XYKPool,
                    LiquiditySourceType::XSTPool,
                    LiquiditySourceType::MulticollateralBondingCurvePool
                ]
                .into_iter()
            ),
            xyk_weight.max(xst_weight).max(multicollateral_weight)
        );
        assert_eq!(
            DexApi::exchange_weight_filtered(
                [
                    LiquiditySourceType::XYKPool,
                    LiquiditySourceType::XSTPool,
                    LiquiditySourceType::MulticollateralBondingCurvePool,
                    LiquiditySourceType::OrderBook
                ]
                .into_iter()
            ),
            xyk_weight
                .max(xst_weight)
                .max(multicollateral_weight)
                .max(order_book_weight)
        );
    })
}

#[test]
fn test_exchange_weight_filtered_matches_exchange_weight() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let all_sources: Vec<_> = LiquiditySourceType::iter().collect();
        assert_eq!(
            DexApi::exchange_weight_filtered(all_sources.into_iter()),
            DexApi::exchange_weight(),
        )
    })
}

#[test]
fn test_enable_disable_liquidity_source_unauthorized() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_err!(
            DexApi::enable_liquidity_source(
                RuntimeOrigin::signed(alice()),
                LiquiditySourceType::XYKPool
            ),
            BadOrigin
        );

        assert_err!(
            DexApi::disable_liquidity_source(
                RuntimeOrigin::signed(bob()),
                LiquiditySourceType::XYKPool
            ),
            BadOrigin
        );
    })
}

#[test]
fn test_liquidity_source_should_enable() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // check before
        assert_eq!(
            EnabledSourceTypes::<Runtime>::get(),
            [
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4
            ]
        );

        // enable source
        assert_ok!(DexApi::enable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::XYKPool
        ));

        // check after
        assert_eq!(
            EnabledSourceTypes::<Runtime>::get(),
            [
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4,
                LiquiditySourceType::XYKPool
            ]
        );
    })
}

#[test]
fn test_liquidity_source_should_not_enable_twice() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // check before
        assert_eq!(
            EnabledSourceTypes::<Runtime>::get(),
            [
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4
            ]
        );

        // try to enable already enabled source
        assert_err!(
            DexApi::enable_liquidity_source(RuntimeOrigin::root(), LiquiditySourceType::MockPool2),
            Error::<Runtime>::LiquiditySourceAlreadyEnabled
        );

        // check after
        assert_eq!(
            EnabledSourceTypes::<Runtime>::get(),
            [
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4,
            ]
        );
    })
}

#[test]
fn test_liquidity_source_should_disable() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // check before
        assert_eq!(
            EnabledSourceTypes::<Runtime>::get(),
            [
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4
            ]
        );

        // disable source
        assert_ok!(DexApi::disable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::MockPool3
        ));

        // check after
        assert_eq!(
            EnabledSourceTypes::<Runtime>::get(),
            [
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool4,
            ]
        );
    })
}

#[test]
fn test_liquidity_source_should_not_disable_twice() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // check before
        assert_eq!(
            EnabledSourceTypes::<Runtime>::get(),
            [
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4
            ]
        );

        // try to disable already disabled source
        assert_err!(
            DexApi::disable_liquidity_source(RuntimeOrigin::root(), LiquiditySourceType::XYKPool),
            Error::<Runtime>::LiquiditySourceAlreadyDisabled
        );

        // check after
        assert_eq!(
            EnabledSourceTypes::<Runtime>::get(),
            [
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4,
            ]
        );
    })
}
