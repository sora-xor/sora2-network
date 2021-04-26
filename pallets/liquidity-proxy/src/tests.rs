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
use crate::{Error, LiquidityProxyTrait};
use common::prelude::fixnum::ops::CheckedSub;
use common::prelude::{Balance, SwapAmount};
use common::{
    balance, fixed, fixed_wrapper, FilterMode, Fixed, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType, RewardReason, DOT, KSM, PSWAP, VAL, XOR,
};
use core::convert::TryInto;
use frame_support::assert_noop;
use sp_runtime::DispatchError;

#[inline]
fn mcbc_excluding_filter(dex: DEXId) -> LiquiditySourceFilter<DEXId, LiquiditySourceType> {
    LiquiditySourceFilter::with_forbidden(
        dex,
        [LiquiditySourceType::MulticollateralBondingCurvePool].into(),
    )
}

#[test]
fn test_quote_exact_input_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount: Balance = balance!(500);
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, 0),
            mcbc_excluding_filter(DEX_C_ID),
            false,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, 0),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let mut dist = quotes.distribution;

        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(537.643138033120596204));
        assert_eq!(quotes.fee, balance!(1.1125));
        assert_eq!(ls_quote.amount, quotes.amount);
        assert_eq!(ls_quote.fee, quotes.fee);
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool),
                    fixed!(0.1),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    fixed!(0.225),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    fixed!(0.025),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    fixed!(0.65),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_exact_input_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    let amount = balance!(500);
    ext.execute_with(|| {
        let (quotes, rewards) = LiquidityProxy::quote_single(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(amount, 0),
            mcbc_excluding_filter(DEX_C_ID),
            false,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(amount, 0),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(rewards, Vec::new());
        assert_eq!(quotes.amount, balance!(363.569067258883248761));
        assert_eq!(quotes.fee, balance!(0.551491116751269035));
        assert_eq!(ls_quote.amount, quotes.amount);
        assert_eq!(ls_quote.fee, quotes.fee);
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool),
                    fixed!(0.275),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    fixed!(0.2),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    fixed!(0.225),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    fixed!(0.3),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_exact_output_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount: Balance = balance!(250);
        let (quotes, rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(amount, balance!(10000)),
            mcbc_excluding_filter(DEX_C_ID),
            false,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(amount, balance!(10000)),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        let tolerance = fixed!(0.0000000001);
        let approx_expected_base_amount = fixed!(205.339009250744456360);
        assert_eq!(rewards, Vec::new());
        assert!(
            (Fixed::from_bits(quotes.amount.try_into().unwrap())
                .csub(approx_expected_base_amount)
                .unwrap()
                < tolerance)
                && (approx_expected_base_amount
                    .csub(Fixed::from_bits(quotes.amount.try_into().unwrap()))
                    .unwrap()
                    < tolerance)
        );
        assert_eq!(quotes.fee, balance!(0.531316943052148668));
        assert_eq!(ls_quote.amount, quotes.amount);
        assert_eq!(ls_quote.fee, quotes.fee);
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool),
                    fixed!(0),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    fixed!(0.2),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    fixed!(0),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    fixed!(0.8),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_exact_output_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount = balance!(250);
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(amount, balance!(10000)),
            mcbc_excluding_filter(DEX_C_ID),
            false,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(amount, balance!(10000)),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        let tolerance = fixed!(0.0000000001);
        let approx_expected_target_amount = fixed!(322.399717709871);
        assert!(
            (Fixed::from_bits(quotes.amount.try_into().unwrap())
                .csub(approx_expected_target_amount)
                .unwrap()
                < tolerance)
                && (approx_expected_target_amount
                    .csub(Fixed::from_bits(quotes.amount.try_into().unwrap()))
                    .unwrap()
                    < tolerance)
        );
        assert_eq!(quotes.fee, balance!(0.338264379900812242));
        assert_eq!(ls_quote.amount, quotes.amount);
        assert_eq!(ls_quote.fee, quotes.fee);
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool),
                    fixed!(0.325),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    fixed!(0.175),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    fixed!(0.325),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    fixed!(0.175),
                ),
            ]
        );
    });
}

#[test]
fn test_poly_quote_exact_input_1_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let (quotes, _rewards) = LiquidityProxy::quote(
            &KSM,
            &DOT,
            SwapAmount::with_desired_input(balance!(100), 0),
            LiquiditySourceFilter::empty(DEX_A_ID),
            false,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            &KSM,
            &DOT,
            SwapAmount::with_desired_input(balance!(100), 0),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let ls_swap = LiquidityProxy::exchange(
            &alice(),
            &alice(),
            &KSM,
            &DOT,
            SwapAmount::with_desired_input(balance!(100), 0),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to swap via LiquiditySource trait");

        assert_eq!(quotes.amount, balance!(934.572151021276260545));
        assert_eq!(quotes.fee, balance!(2.318181818181818181));
        assert_eq!(ls_quote.amount, quotes.amount);
        assert_eq!(ls_quote.fee, quotes.fee);
        assert_eq!(ls_swap.amount, quotes.amount);
        assert_eq!(ls_swap.fee, quotes.fee);
    });
}

#[test]
fn test_poly_quote_exact_output_1_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let (quotes, _rewards) = LiquidityProxy::quote(
            &KSM,
            &DOT,
            SwapAmount::with_desired_output(balance!(934.572151021276260545), balance!(501)),
            LiquiditySourceFilter::empty(DEX_A_ID),
            false,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            &KSM,
            &DOT,
            SwapAmount::with_desired_output(balance!(934.572151021276260545), balance!(101)).into(),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let ls_swap = LiquidityProxy::exchange(
            &alice(),
            &alice(),
            &KSM,
            &DOT,
            SwapAmount::with_desired_output(balance!(934.572151021276260545), balance!(101)).into(),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to swap via LiquiditySource trait");

        assert_eq!(quotes.amount, balance!(100.0));
        assert_eq!(quotes.fee, balance!(2.318181818181818181));
        assert_eq!(ls_quote.amount, quotes.amount);
        assert_eq!(ls_quote.fee, quotes.fee);
        assert_eq!(ls_swap.amount, quotes.amount);
        assert_eq!(ls_swap.fee, quotes.fee);
    });
}

#[test]
fn test_poly_quote_exact_input_2_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let (quotes, _rewards) = LiquidityProxy::quote(
            &DOT,
            &KSM,
            SwapAmount::with_desired_input(balance!(500), 0),
            LiquiditySourceFilter::empty(DEX_A_ID),
            false,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            &DOT,
            &KSM,
            SwapAmount::with_desired_input(balance!(500), 0),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let ls_swap = LiquidityProxy::exchange(
            &alice(),
            &alice(),
            &DOT,
            &KSM,
            SwapAmount::with_desired_input(balance!(500), 0),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to swap via LiquiditySource trait");

        assert_eq!(quotes.amount, balance!(555.083861089846196673));
        assert_eq!(quotes.fee, balance!(2.666666666666666666));
        assert_eq!(ls_quote.amount, quotes.amount);
        assert_eq!(ls_quote.fee, quotes.fee);
        assert_eq!(ls_swap.amount, quotes.amount);
        assert_eq!(ls_swap.fee, quotes.fee);
    });
}

#[test]
fn test_poly_quote_exact_output_2_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let (quotes, _rewards) = LiquidityProxy::quote(
            &DOT,
            &KSM,
            SwapAmount::with_desired_output(balance!(555.083861089846196673), balance!(501)),
            LiquiditySourceFilter::empty(DEX_A_ID),
            false,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            &DOT,
            &KSM,
            SwapAmount::with_desired_output(balance!(555.083861089846196673), balance!(501)).into(),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let ls_swap = LiquidityProxy::exchange(
            &alice(),
            &alice(),
            &DOT,
            &KSM,
            SwapAmount::with_desired_output(balance!(555.083861089846196673), balance!(501)).into(),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to swap via LiquiditySource trait");

        assert_eq!(quotes.amount, balance!(500.000000000000000000));
        assert_eq!(quotes.fee, balance!(2.666666666666666666));
        assert_eq!(ls_quote.amount, quotes.amount);
        assert_eq!(ls_quote.fee, quotes.fee);
        assert_eq!(ls_swap.amount, quotes.amount);
        assert_eq!(ls_swap.fee, quotes.fee);
    });
}

#[test]
fn test_sell_token_for_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let filter = mcbc_excluding_filter(DEX_C_ID);
        let result = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(balance!(500), balance!(345)),
            filter,
        )
        .expect("Failed to swap assets");
        assert_eq!(result.amount, balance!(363.569067258883248731));
    });
}

#[test]
fn test_sell_base_for_token_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let filter = mcbc_excluding_filter(DEX_C_ID);
        let result = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(500), balance!(510)),
            filter,
        )
        .expect("Failed to swap assets");
        assert_eq!(result.amount, balance!(537.643138033120596095));
    });
}

#[test]
fn test_sell_token_for_base_with_liquidity_source_trait_should_pass() {
    let mut ext = ExtBuilder::default().build();
    let amount = balance!(500);
    ext.execute_with(|| {
        let alice = alice();
        let result = LiquidityProxy::exchange(
            &alice,
            &alice,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(amount, balance!(345)),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to swap assets");
        assert_eq!(result.amount, balance!(363.569067258883248731));
    });
}

#[test]
fn test_sell_base_for_token_with_liquidity_source_trait_should_pass() {
    let mut ext = ExtBuilder::default().build();
    let amount: Balance = balance!(500);
    ext.execute_with(|| {
        let alice = alice();
        let result = LiquidityProxy::exchange(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, balance!(510)),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to swap assets");
        assert_eq!(result.amount, balance!(537.643138033120596095));
    });
}

#[test]
fn test_buy_base_with_allowed_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let filter = LiquiditySourceFilter::with_allowed(
            DEX_C_ID,
            [
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
            ]
            .into(),
        );
        let result = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(200), balance!(298)),
            filter,
        )
        .expect("Failed to swap assets");
        let tolerance = fixed!(0.0000000001);
        let approx_expected_target_amount = fixed!(284.281354954553);
        assert!(
            Fixed::from_bits(result.amount.try_into().unwrap())
                .csub(approx_expected_target_amount)
                .unwrap()
                < tolerance
        );
        assert!(
            approx_expected_target_amount
                .csub(Fixed::from_bits(result.amount.try_into().unwrap()))
                .unwrap()
                < tolerance
        );
    });
}

#[test]
fn test_buy_base_with_forbidden_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let filter = LiquiditySourceFilter::with_forbidden(
            DEX_C_ID,
            [
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MulticollateralBondingCurvePool,
            ]
            .into(),
        );
        let result = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(200), balance!(291)),
            filter,
        )
        .expect("Failed to swap assets");
        let tolerance = fixed!(0.0000000001);
        let approx_expected_target_amount: Fixed = fixed!(277.348779693090);
        assert!(
            Fixed::from_bits(result.amount.try_into().unwrap())
                .csub(approx_expected_target_amount)
                .unwrap()
                < tolerance
        );
        assert!(
            approx_expected_target_amount
                .csub(Fixed::from_bits(result.amount.try_into().unwrap()))
                .unwrap()
                < tolerance
        );
    });
}

#[test]
fn test_quote_should_fail_with_unavailable_exchange_path() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &KSM,
            SwapAmount::with_desired_output(balance!(300), Balance::MAX),
            mcbc_excluding_filter(DEX_C_ID),
            false,
        );
        assert_noop!(result, <Error<Runtime>>::UnavailableExchangePath);
    });
}

#[test]
fn test_quote_should_fail_with_unavailable_exchange_path_2() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(balance!(300), Balance::MAX),
            LiquiditySourceFilter::with_forbidden(
                DEX_C_ID,
                [
                    LiquiditySourceType::MockPool,
                    LiquiditySourceType::MockPool2,
                    LiquiditySourceType::MockPool3,
                    LiquiditySourceType::MockPool4,
                    LiquiditySourceType::MulticollateralBondingCurvePool,
                ]
                .into(),
            ),
            false,
        );
        assert_noop!(result, <Error<Runtime>>::UnavailableExchangePath);
    });
}

#[test]
fn test_quote_should_fail_with_aggregation_error() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(balance!(5000), i128::MAX as u128),
            mcbc_excluding_filter(DEX_C_ID),
            false,
        );
        assert_noop!(result, <Error<Runtime>>::AggregationError);
    });
}

#[test]
fn test_sell_however_big_amount_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let result = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(2000), 0),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > 0 && result.amount < balance!(180));

        let result = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(4000), 0),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > 0 && result.amount < balance!(180));

        let result = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(10000), 0),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > 0 && result.amount < balance!(180));

        let result = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(100000), 0),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > 0 && result.amount < balance!(180));

        let result = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(1000000), 0),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > 0 && result.amount < balance!(180));
    });
}

#[test]
fn test_swap_should_fail_with_bad_origin() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::swap(
            Origin::root(),
            DEX_C_ID,
            DOT,
            GetBaseAssetId::get(),
            SwapAmount::with_desired_input(balance!(500), balance!(300)),
            Vec::new(),
            FilterMode::Disabled,
        );
        assert_noop!(result, DispatchError::BadOrigin);
    });
}

#[test]
fn test_fee_when_exchange_on_one_source_of_many_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount: Balance = balance!(250);
        let filter = LiquiditySourceFilter::with_allowed(
            DEX_C_ID,
            [
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4,
            ]
            .into(),
        );
        let (quotes, _rewards) = LiquidityProxy::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(amount, balance!(10000)),
            filter,
            false,
        )
        .expect("Failed to get a quote");
        assert_eq!(quotes.fee, balance!(0.630925033164008153));
    });
}

#[test]
fn test_quote_single_source_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockMCBCPool::init(get_mcbc_reserves_normal()).unwrap();
        let amount = balance!(500);
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, 0),
            LiquiditySourceFilter::with_allowed(DEX_C_ID, [LiquiditySourceType::MockPool].into()),
            false,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, 0),
            LiquiditySourceFilter::with_allowed(DEX_C_ID, [LiquiditySourceType::MockPool].into()),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let dist = quotes.distribution;

        assert_eq!(quotes.amount, balance!(269.607843137254901960));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(ls_quote.amount, quotes.amount);
        assert_eq!(ls_quote.fee, quotes.fee);
        assert_eq!(
            &dist,
            &[(
                LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool),
                fixed!(1),
            ),]
        );
    });
}

#[test]
fn test_quote_fast_split_exact_input_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockMCBCPool::init(get_mcbc_reserves_normal()).unwrap();
        let filter = LiquiditySourceFilter::with_allowed(
            DEX_D_ID,
            [
                LiquiditySourceType::MulticollateralBondingCurvePool,
                LiquiditySourceType::MockPool,
            ]
            .to_vec(),
        );

        // Buying VAL for XOR
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            SwapAmount::with_desired_input(balance!(100), 0),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        // The "smart" split produces the outcome which is worse than purely XYK pool swap
        // Hence the latter result use used resulting in the dist == [0.0, 1.0]
        assert_eq!(quotes.amount, balance!(18181.818181818181818181));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.0),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(1.0),
                ),
            ]
        );

        // Buying KSM for XOR
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &KSM,
            SwapAmount::with_desired_input(balance!(200), 0),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(174.276240737227906075));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.525748901661215533),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.474251098338784467),
                ),
            ]
        );

        // Buying DOT for XOR
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(200), 0),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(1516.342527519604340764));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.525748901661215535),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.474251098338784465),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_fast_split_exact_output_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockMCBCPool::init(get_mcbc_reserves_normal()).unwrap();
        let filter = LiquiditySourceFilter::with_allowed(
            DEX_D_ID,
            [
                LiquiditySourceType::MulticollateralBondingCurvePool,
                LiquiditySourceType::MockPool,
            ]
            .to_vec(),
        );

        // Buying VAL for XOR
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            SwapAmount::with_desired_output(balance!(20000), balance!(1000)),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        // The "smart" split produces the outcome which is worse than purely XYK pool swap
        // Hence the latter result use used resulting in the dist == [0.0, 1.0]
        assert_eq!(quotes.amount, balance!(111.111111111111111112));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.0),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(1.0),
                ),
            ]
        );

        // Buying KSM for XOR
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &KSM,
            SwapAmount::with_desired_output(balance!(200), balance!(1000)),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(230.897068686326074201));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.5668347233079054),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.4331652766920946),
                ),
            ]
        );

        // Buying DOT for XOR
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(balance!(1000), balance!(1000)),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(125.000000000000000000));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.0),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(1.0),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_fast_split_exact_output_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockMCBCPool::init(get_mcbc_reserves_normal()).unwrap();
        let filter = LiquiditySourceFilter::with_allowed(
            DEX_D_ID,
            [
                LiquiditySourceType::MulticollateralBondingCurvePool,
                LiquiditySourceType::MockPool,
            ]
            .to_vec(),
        );

        // Buying XOR for VAL
        let (quotes, rewards) = LiquidityProxy::quote_single(
            &VAL,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(100), Balance::MAX),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            rewards,
            vec![(
                balance!(23.258770902877438500),
                XOR.into(),
                RewardReason::BuyOnBondingCurve
            )]
        );
        assert_eq!(quotes.amount, balance!(22081.292525857240241897));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.232587709028774385),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.767412290971225615),
                ),
            ]
        );

        // Buying XOR for KSM
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &KSM,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(200), Balance::MAX),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(208.138107215848656553));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.896319032715363259),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.103680967284636741),
                ),
            ]
        );

        // Buying XOR for DOT
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(100), Balance::MAX),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(934.530528433224671739));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.792638065430726512),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.207361934569273488),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_fast_split_exact_input_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockMCBCPool::init(get_mcbc_reserves_normal()).unwrap();
        let filter = LiquiditySourceFilter::with_allowed(
            DEX_D_ID,
            [
                LiquiditySourceType::MulticollateralBondingCurvePool,
                LiquiditySourceType::MockPool,
            ]
            .to_vec(),
        );

        // Buying XOR for VAL
        let (quotes, rewards) = LiquidityProxy::quote_single(
            &VAL,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(balance!(20000), 0),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            rewards,
            vec![(
                balance!(14.388332979612792044),
                XOR.into(),
                RewardReason::BuyOnBondingCurve
            )]
        );
        assert_eq!(quotes.amount, balance!(91.129562076735353497));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.168800432601626651),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.831199567398373349),
                ),
            ]
        );

        // Buying XOR for KSM
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &KSM,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(balance!(200), 0),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(182.802146328804827595));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.894123558338540146),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.105876441661459854),
                ),
            ]
        );

        // Buying XOR for DOT
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(balance!(500), 0),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(53.662213070708617869));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.618844810018744511),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.381155189981255489),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_fast_split_exact_output_target_undercollateralized_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockMCBCPool::init(get_mcbc_reserves_undercollateralized()).unwrap();
        let filter = LiquiditySourceFilter::with_allowed(
            DEX_D_ID,
            [
                LiquiditySourceType::MulticollateralBondingCurvePool,
                LiquiditySourceType::MockPool,
            ]
            .to_vec(),
        );

        // Buying VAL for XOR
        // Will overflow if the requested amount of collateral exceeds this
        // collateral reserves in MCBC unless specifically guarded
        // - VAL reserves in MCBC: 5,000
        // - the default requested VAL (after split at the price equillibrium): ~13,755
        // As a result, the price at TBC becomes too high so that the "Smart" algo is dropped
        // so that the entire amount ends up being exchanged at the XYK pool
        let (quotes, rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            SwapAmount::with_desired_output(balance!(20000), balance!(1000)),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(rewards, Vec::new());
        assert_eq!(quotes.amount, balance!(111.111111111111111112));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.0),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(1.0),
                ),
            ]
        );

        // Buying KSM for XOR
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &KSM,
            SwapAmount::with_desired_output(balance!(200), balance!(1000)),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(250.0));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.0),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(1.0),
                ),
            ]
        );

        // Buying DOT for XOR
        let (quotes, _rewards) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(balance!(1000), balance!(1000)),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(125.0));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.0),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(1.0),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_should_return_rewards_for_single_source() {
    let mut ext = ExtBuilder::with_enabled_sources(vec![
        LiquiditySourceType::MulticollateralBondingCurvePool,
    ])
    .build();
    ext.execute_with(|| {
        MockMCBCPool::init(get_mcbc_reserves_normal()).unwrap();
        let filter = LiquiditySourceFilter::empty(DEX_D_ID);

        let (_, rewards_forward) = LiquidityProxy::quote_single(
            &VAL,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(100), balance!(1000)),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        let (_, rewards_backward) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            SwapAmount::with_desired_output(balance!(100), balance!(1000)),
            filter.clone(),
            false,
        )
        .expect("Failed to get a quote");

        // Mock tbc defined reward as output token amount.
        assert_eq!(
            rewards_forward,
            vec![(balance!(100), XOR.into(), RewardReason::BuyOnBondingCurve)]
        );
        assert_eq!(rewards_backward, vec![]);
    });
}

#[test]
fn test_quote_should_return_rewards_for_multiple_sources() {
    let mut ext = ExtBuilder::with_enabled_sources(vec![
        LiquiditySourceType::MockPool,
        LiquiditySourceType::MockPool2,
        LiquiditySourceType::MockPool3,
        LiquiditySourceType::MockPool4,
    ])
    .build();
    ext.execute_with(|| {
        MockLiquiditySource::add_reward((balance!(101), PSWAP.into(), RewardReason::Unspecified));
        MockLiquiditySource2::add_reward((balance!(201), VAL.into(), RewardReason::Unspecified));
        MockLiquiditySource2::add_reward((balance!(202), XOR.into(), RewardReason::Unspecified));
        MockLiquiditySource3::add_reward((balance!(301), DOT.into(), RewardReason::Unspecified));

        let amount: Balance = balance!(500);
        let (_, rewards) = LiquidityProxy::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, 0),
            mcbc_excluding_filter(DEX_C_ID),
            false,
        )
        .expect("Failed to get a quote");

        assert_eq!(
            rewards,
            vec![
                (balance!(101), PSWAP.into(), RewardReason::Unspecified),
                (balance!(201), VAL.into(), RewardReason::Unspecified),
                (balance!(202), XOR.into(), RewardReason::Unspecified),
                (balance!(301), DOT.into(), RewardReason::Unspecified)
            ]
        );
    });
}

#[test]
#[rustfmt::skip]
fn test_list_enabled_sources_for_path_query_should_pass_1() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(Origin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(Origin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, XYKPool).expect("failed to enable source");
        let query_a = LiquidityProxy::list_enabled_sources_for_path(0, XOR, VAL);
        let query_b = LiquidityProxy::list_enabled_sources_for_path(0, VAL, XOR);
        let query_c = LiquidityProxy::list_enabled_sources_for_path(0, XOR, PSWAP);
        let query_d = LiquidityProxy::list_enabled_sources_for_path(0, PSWAP, XOR);
        let query_e = LiquidityProxy::list_enabled_sources_for_path(0, VAL, PSWAP);
        let query_f = LiquidityProxy::list_enabled_sources_for_path(0, PSWAP, VAL);
        assert_eq!(query_a.unwrap_err(), Error::<Runtime>::UnavailableExchangePath.into());
        assert_eq!(query_b.unwrap_err(), Error::<Runtime>::UnavailableExchangePath.into());
        assert_eq!(query_c.unwrap(), vec![XYKPool]);
        assert_eq!(query_d.unwrap(), vec![XYKPool]);
        assert_eq!(query_e.unwrap_err(), Error::<Runtime>::UnavailableExchangePath.into());
        assert_eq!(query_f.unwrap_err(), Error::<Runtime>::UnavailableExchangePath.into());
    });
}

#[test]
#[rustfmt::skip]
fn test_list_enabled_sources_for_path_query_should_pass_2() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(Origin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(Origin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, MulticollateralBondingCurvePool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, XYKPool).expect("failed to enable source");
        let query_a = LiquidityProxy::list_enabled_sources_for_path(0, XOR, VAL);
        let query_b = LiquidityProxy::list_enabled_sources_for_path(0, VAL, XOR);
        let query_c = LiquidityProxy::list_enabled_sources_for_path(0, XOR, PSWAP);
        let query_d = LiquidityProxy::list_enabled_sources_for_path(0, PSWAP, XOR);
        let query_e = LiquidityProxy::list_enabled_sources_for_path(0, VAL, PSWAP);
        let query_f = LiquidityProxy::list_enabled_sources_for_path(0, PSWAP, VAL);
        assert_eq!(query_a.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_b.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_c.unwrap(), vec![XYKPool]);
        assert_eq!(query_d.unwrap(), vec![XYKPool]);
        assert_eq!(query_e.unwrap(), vec![]);
        assert_eq!(query_f.unwrap(), vec![]);
    });
}

#[test]
#[rustfmt::skip]
fn test_list_enabled_sources_for_path_query_should_pass_3() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(Origin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(Origin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, MulticollateralBondingCurvePool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, MulticollateralBondingCurvePool).expect("failed to enable source");
        let query_a = LiquidityProxy::list_enabled_sources_for_path(0, XOR, VAL);
        let query_b = LiquidityProxy::list_enabled_sources_for_path(0, VAL, XOR);
        let query_c = LiquidityProxy::list_enabled_sources_for_path(0, XOR, PSWAP);
        let query_d = LiquidityProxy::list_enabled_sources_for_path(0, PSWAP, XOR);
        let query_e = LiquidityProxy::list_enabled_sources_for_path(0, VAL, PSWAP);
        let query_f = LiquidityProxy::list_enabled_sources_for_path(0, PSWAP, VAL);
        assert_eq!(query_a.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_b.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_c.unwrap(), vec![XYKPool, MulticollateralBondingCurvePool]);
        assert_eq!(query_d.unwrap(), vec![XYKPool, MulticollateralBondingCurvePool]);
        assert_eq!(query_e.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_f.unwrap(), vec![MulticollateralBondingCurvePool]);
    });
}

#[test]
#[rustfmt::skip]
fn test_list_enabled_sources_for_path_query_should_pass_4() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(Origin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(Origin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, MulticollateralBondingCurvePool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, MockPool2).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, MulticollateralBondingCurvePool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, MockPool3).expect("failed to enable source");
        let query_a = LiquidityProxy::list_enabled_sources_for_path(0, XOR, VAL);
        let query_b = LiquidityProxy::list_enabled_sources_for_path(0, VAL, XOR);
        let query_c = LiquidityProxy::list_enabled_sources_for_path(0, XOR, PSWAP);
        let query_d = LiquidityProxy::list_enabled_sources_for_path(0, PSWAP, XOR);
        let query_e = LiquidityProxy::list_enabled_sources_for_path(0, VAL, PSWAP);
        let query_f = LiquidityProxy::list_enabled_sources_for_path(0, PSWAP, VAL);
        assert_eq!(query_a.unwrap(), vec![XYKPool, MulticollateralBondingCurvePool, MockPool2]);
        assert_eq!(query_b.unwrap(), vec![XYKPool, MulticollateralBondingCurvePool, MockPool2]);
        assert_eq!(query_c.unwrap(), vec![XYKPool, MulticollateralBondingCurvePool, MockPool3]);
        assert_eq!(query_d.unwrap(), vec![XYKPool, MulticollateralBondingCurvePool, MockPool3]);
        assert_eq!(query_e.unwrap(), vec![XYKPool, MulticollateralBondingCurvePool]);
        assert_eq!(query_f.unwrap(), vec![XYKPool, MulticollateralBondingCurvePool]);
    });
}

#[test]
#[rustfmt::skip]
fn test_is_path_available_should_pass_1() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(Origin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(Origin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, MulticollateralBondingCurvePool).expect("failed to enable source");
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, VAL).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, XOR).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, PSWAP).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, PSWAP, XOR).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, PSWAP).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, PSWAP, VAL).unwrap(), true);
    });
}

#[test]
#[rustfmt::skip]
fn test_is_path_available_should_pass_2() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(Origin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(Origin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, MulticollateralBondingCurvePool).expect("failed to enable source");
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, VAL).unwrap(), false);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, XOR).unwrap(), false);
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, PSWAP).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, PSWAP, XOR).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, PSWAP).unwrap(), false);
        assert_eq!(LiquidityProxy::is_path_available(0, PSWAP, VAL).unwrap(), false);
    });
}

#[test]
#[rustfmt::skip]
fn test_is_path_available_should_pass_3() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(Origin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(Origin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, MulticollateralBondingCurvePool).expect("failed to enable source");
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, VAL).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, XOR).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, PSWAP).unwrap(), false);
        assert_eq!(LiquidityProxy::is_path_available(0, PSWAP, XOR).unwrap(), false);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, PSWAP).unwrap(), false);
        assert_eq!(LiquidityProxy::is_path_available(0, PSWAP, VAL).unwrap(), false);
    });
}

#[test]
#[rustfmt::skip]
fn test_is_path_available_should_pass_4() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(Origin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(Origin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, MulticollateralBondingCurvePool).expect("failed to enable source");
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, VAL).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, XOR).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, PSWAP).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, PSWAP, XOR).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, PSWAP).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, PSWAP, VAL).unwrap(), true);
    });
}

#[test]
fn test_smart_split_with_extreme_total_supply_works() {
    fn run_test(
        collateral_asset_id: AssetId,
        xyk_pool_reserves: (Fixed, Fixed),
        tbc_reserves: Balance,
        total_supply: Balance,
    ) {
        let mut ext = ExtBuilder::with_total_supply_and_reserves(
            total_supply,
            vec![(0, collateral_asset_id, xyk_pool_reserves.clone())],
        )
        .build();
        ext.execute_with(|| {
            MockMCBCPool::init(vec![(collateral_asset_id, tbc_reserves)]).unwrap();

            let amount_base: Balance = (xyk_pool_reserves.0 / fixed_wrapper!(10))
                .try_into_balance()
                .unwrap();
            let amount_collateral: Balance = (xyk_pool_reserves.1 / fixed_wrapper!(10))
                .try_into_balance()
                .unwrap();
            let base_asset = GetBaseAssetId::get();
            let filter_both = LiquiditySourceFilter::with_allowed(
                0,
                [
                    LiquiditySourceType::MulticollateralBondingCurvePool,
                    LiquiditySourceType::MockPool,
                ]
                .to_vec(),
            );
            let filter_xyk =
                LiquiditySourceFilter::with_allowed(0, [LiquiditySourceType::MockPool].to_vec());

            // base -> collateral, fixed input
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_input(amount_base.clone(), 0),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_input(amount_base.clone(), 0),
                filter_xyk.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_xyk.0.amount);

            // collateral - > base, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_output(
                    amount_base.clone(),
                    xyk_pool_reserves.1.into_bits().try_into().unwrap(),
                ),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_output(
                    amount_base.clone(),
                    xyk_pool_reserves.1.into_bits().try_into().unwrap(),
                ),
                filter_xyk.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount <= quotes_xyk.0.amount);

            // collateral - > base, fixed input
            let quotes_smart = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_input(amount_collateral.clone(), 0),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_input(amount_collateral.clone(), 0),
                filter_xyk.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_xyk.0.amount);

            // base -> collateral, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_output(
                    amount_collateral.clone(),
                    xyk_pool_reserves.0.into_bits().try_into().unwrap(),
                ),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_output(
                    amount_collateral.clone(),
                    xyk_pool_reserves.0.into_bits().try_into().unwrap(),
                ),
                filter_xyk.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount <= quotes_xyk.0.amount);
        });
    }

    let total_supplies = vec![
        balance!(0),
        balance!(100),
        balance!(1000),
        balance!(10000),
        balance!(500000),
        balance!(1000000),
        balance!(10000000),
    ];

    for t in &total_supplies {
        // XOR <-> VAL
        run_test(VAL, (fixed!(1000), fixed!(200000)), balance!(100000), *t);
        // XOR <-> KSM
        run_test(KSM, (fixed!(1000), fixed!(1000)), balance!(100000), *t);
        // XOR <-> DOT
        run_test(DOT, (fixed!(1000), fixed!(9000)), balance!(100000), *t);
    }
}

#[test]
fn test_smart_split_with_low_collateral_reserves_works() {
    fn run_test(
        collateral_asset_id: AssetId,
        xyk_pool_reserves: (Fixed, Fixed),
        tbc_reserves: Balance,
        total_supply: Balance,
    ) {
        let mut ext = ExtBuilder::with_total_supply_and_reserves(
            total_supply,
            vec![(0, collateral_asset_id, xyk_pool_reserves.clone())],
        )
        .build();
        ext.execute_with(|| {
            MockMCBCPool::init(vec![(collateral_asset_id, tbc_reserves)]).unwrap();

            let amount_base: Balance = (xyk_pool_reserves.0 / fixed_wrapper!(10))
                .try_into_balance()
                .unwrap();
            let amount_collateral: Balance = (xyk_pool_reserves.1 / fixed_wrapper!(10))
                .try_into_balance()
                .unwrap();
            let base_asset = GetBaseAssetId::get();
            let filter_both = LiquiditySourceFilter::with_allowed(
                0,
                [
                    LiquiditySourceType::MulticollateralBondingCurvePool,
                    LiquiditySourceType::MockPool,
                ]
                .to_vec(),
            );
            let filter_xyk =
                LiquiditySourceFilter::with_allowed(0, [LiquiditySourceType::MockPool].to_vec());

            // base -> collateral, fixed input
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_input(amount_base.clone(), 0),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_input(amount_base.clone(), 0),
                filter_xyk.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_xyk.0.amount);

            // collateral - > base, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_output(
                    amount_base.clone(),
                    xyk_pool_reserves.1.into_bits().try_into().unwrap(),
                ),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_output(
                    amount_base.clone(),
                    xyk_pool_reserves.1.into_bits().try_into().unwrap(),
                ),
                filter_xyk.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount <= quotes_xyk.0.amount);

            // collateral - > base, fixed input
            let quotes_smart = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_input(amount_collateral.clone(), 0),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_input(amount_collateral.clone(), 0),
                filter_xyk.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_xyk.0.amount);

            // base -> collateral, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_output(
                    amount_collateral.clone(),
                    xyk_pool_reserves.0.into_bits().try_into().unwrap(),
                ),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_output(
                    amount_collateral.clone(),
                    xyk_pool_reserves.0.into_bits().try_into().unwrap(),
                ),
                filter_xyk.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount <= quotes_xyk.0.amount);
        });
    }

    let tbc_reserves_amounts = vec![
        balance!(0),
        balance!(100),
        balance!(200),
        balance!(500),
        balance!(1000),
        balance!(10000),
    ];

    for r in &tbc_reserves_amounts {
        // XOR <-> VAL
        run_test(VAL, (fixed!(1000), fixed!(200000)), *r, balance!(350000));
        // XOR <-> KSM
        run_test(KSM, (fixed!(1000), fixed!(1000)), *r, balance!(350000));
        // XOR <-> DOT
        run_test(DOT, (fixed!(1000), fixed!(9000)), *r, balance!(350000));
    }
}

#[test]
fn test_smart_split_with_low_xykpool_reserves_works() {
    fn run_test(
        collateral_asset_id: AssetId,
        xyk_pool_reserves: (Fixed, Fixed),
        tbc_reserves: Balance,
        total_supply: Balance,
        amount_base: Balance,
        amount_collateral: Balance,
    ) {
        let mut ext = ExtBuilder::with_total_supply_and_reserves(
            total_supply,
            vec![(0, collateral_asset_id, xyk_pool_reserves.clone())],
        )
        .build();
        ext.execute_with(|| {
            MockMCBCPool::init(vec![(collateral_asset_id, tbc_reserves)]).unwrap();

            let base_asset = GetBaseAssetId::get();
            let filter_both = LiquiditySourceFilter::with_allowed(
                0,
                [
                    LiquiditySourceType::MulticollateralBondingCurvePool,
                    LiquiditySourceType::MockPool,
                ]
                .to_vec(),
            );
            let filter_mcbc = LiquiditySourceFilter::with_allowed(
                0,
                [LiquiditySourceType::MulticollateralBondingCurvePool].to_vec(),
            );

            // base -> collateral, fixed input
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_input(amount_base.clone(), 0),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_mcbc = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_input(amount_base.clone(), 0),
                filter_mcbc.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_mcbc.0.amount);

            // collateral - > base, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_output(
                    amount_base.clone(),
                    amount_collateral.saturating_mul(10),
                ),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_mcbc = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_output(
                    amount_base.clone(),
                    amount_collateral.saturating_mul(10),
                ),
                filter_mcbc.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount <= quotes_mcbc.0.amount);

            // collateral - > base, fixed input
            let quotes_smart = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_input(amount_collateral.clone(), 0),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_mcbc = LiquidityProxy::quote_single(
                &collateral_asset_id,
                &base_asset,
                SwapAmount::with_desired_input(amount_collateral.clone(), 0),
                filter_mcbc.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_mcbc.0.amount);

            // base -> collateral, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_output(
                    amount_collateral.clone(),
                    amount_base.saturating_mul(10),
                ),
                filter_both.clone(),
                false,
            )
            .expect("Failed to get a quote");
            let quotes_mcbc = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_output(
                    amount_collateral.clone(),
                    amount_base.saturating_mul(10),
                ),
                filter_mcbc.clone(),
                false,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount <= quotes_mcbc.0.amount);
        });
    }

    let xor_val_pool_reserves = vec![
        (fixed!(0), fixed!(0)),
        (fixed!(5), fixed!(1000)),
        (fixed!(10), fixed!(2000)),
        (fixed!(50), fixed!(10000)),
        (fixed!(100), fixed!(20000)),
        (fixed!(500), fixed!(100000)),
    ];

    let xor_ksm_pool_reserves = vec![
        (fixed!(0), fixed!(0)),
        (fixed!(5), fixed!(5)),
        (fixed!(10), fixed!(10)),
        (fixed!(50), fixed!(50)),
        (fixed!(100), fixed!(100)),
        (fixed!(500), fixed!(500)),
    ];

    let xor_dot_pool_reserves = vec![
        (fixed!(0), fixed!(0)),
        (fixed!(5), fixed!(45)),
        (fixed!(10), fixed!(90)),
        (fixed!(50), fixed!(450)),
        (fixed!(100), fixed!(900)),
        (fixed!(500), fixed!(4500)),
    ];

    for r in &xor_val_pool_reserves {
        // XOR <-> VAL
        run_test(
            VAL,
            *r,
            balance!(100000),
            balance!(350000),
            balance!(100),
            balance!(20000),
        );
    }
    for r in &xor_ksm_pool_reserves {
        // XOR <-> KSM
        run_test(
            KSM,
            *r,
            balance!(100000),
            balance!(350000),
            balance!(200),
            balance!(200),
        );
    }
    for r in &xor_dot_pool_reserves {
        // XOR <-> DOT
        run_test(
            DOT,
            *r,
            balance!(100000),
            balance!(350000),
            balance!(200),
            balance!(1000),
        );
    }
}

#[test]
fn test_smart_split_selling_xor_should_fail() {
    fn run_test(
        collateral_asset_id: AssetId,
        xyk_pool_reserves: (Fixed, Fixed),
        tbc_reserves: Balance,
        total_supply: Balance,
        amount_base: Balance,
        amount_collateral: Balance,
    ) {
        let mut ext = ExtBuilder::with_total_supply_and_reserves(
            total_supply,
            vec![(0, collateral_asset_id, xyk_pool_reserves.clone())],
        )
        .build();
        ext.execute_with(|| {
            MockMCBCPool::init(vec![(collateral_asset_id, tbc_reserves)]).unwrap();

            let base_asset = GetBaseAssetId::get();
            let filter_both = LiquiditySourceFilter::with_allowed(
                0,
                [
                    LiquiditySourceType::MulticollateralBondingCurvePool,
                    LiquiditySourceType::MockPool,
                ]
                .to_vec(),
            );

            // base -> collateral, fixed output
            let result = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                SwapAmount::with_desired_output(
                    amount_collateral.clone(),
                    amount_base.saturating_mul(10),
                ),
                filter_both.clone(),
                false,
            );
            assert_noop!(result, crate::Error::<Runtime>::InsufficientLiquidity);
        });
    }

    let xor_val_pool_reserves = vec![
        (fixed!(5), fixed!(1000)),
        (fixed!(10), fixed!(2000)),
        (fixed!(50), fixed!(10000)),
    ];
    let val_tbc_reserves = vec![balance!(1000), balance!(2000), balance!(5000)];

    let xor_ksm_pool_reserves = vec![
        (fixed!(5), fixed!(5)),
        (fixed!(10), fixed!(10)),
        (fixed!(50), fixed!(50)),
    ];
    let ksm_tbc_reserves = vec![balance!(20), balance!(50), balance!(100)];

    let xor_dot_pool_reserves = vec![
        (fixed!(5), fixed!(45)),
        (fixed!(10), fixed!(90)),
        (fixed!(50), fixed!(450)),
    ];
    let dot_tbc_reserves = vec![balance!(20), balance!(50), balance!(100)];

    for r in &xor_val_pool_reserves {
        for t in &val_tbc_reserves {
            // XOR <-> VAL
            run_test(
                VAL,
                *r,
                *t,
                balance!(350000),
                balance!(100),
                balance!(20000),
            );
        }
    }
    for r in &xor_ksm_pool_reserves {
        for t in &ksm_tbc_reserves {
            // XOR <-> KSM
            run_test(KSM, *r, *t, balance!(350000), balance!(200), balance!(200));
        }
    }
    for r in &xor_dot_pool_reserves {
        for t in &dot_tbc_reserves {
            // XOR <-> DOT
            run_test(DOT, *r, *t, balance!(350000), balance!(200), balance!(1000));
        }
    }
}

#[test]
fn test_smart_split_error_handling_works() {
    fn run_test(
        collateral_asset_id: AssetId,
        xyk_pool_reserves: (Fixed, Fixed),
        tbc_reserves: Balance,
        amount: SwapAmount<Balance>,
        expected_error: DispatchError,
    ) {
        let mut ext = ExtBuilder::with_total_supply_and_reserves(
            balance!(350000),
            vec![(0, collateral_asset_id, xyk_pool_reserves.clone())],
        )
        .build();
        ext.execute_with(|| {
            MockMCBCPool::init(vec![(collateral_asset_id, tbc_reserves)]).unwrap();

            let result = LiquidityProxy::quote_single(
                &GetBaseAssetId::get(),
                &collateral_asset_id,
                amount,
                LiquiditySourceFilter::empty(0),
                false,
            );

            assert_noop!(result, expected_error);
        });
    }

    // XYK pool has zero reserves, the whole trade will be directed to the MCBC pool.
    // Quote at the MCBC pool fails due to insufficient collateral reserves.
    // Subsequent quote from the XYK pool also fails since it doesn't have any reserves.
    // Error from the MCBC pool quote should be returned as the outcome.
    run_test(
        VAL,
        (fixed!(0), fixed!(0)),
        balance!(1000),
        SwapAmount::with_desired_output(balance!(5000), balance!(1000000)),
        crate::Error::<Runtime>::InsufficientLiquidity.into(),
    );

    // MCBC will fail trying to get the sell price for the `special_asset`.
    // The entire trade will be directed to the XYK pool.
    // Quote at the MCBC pool will never be attempted.
    // Quote from the XYK pool should fail due to insufficient reserves.
    // Error from the XYK pool quote should be returned as the outcome.
    run_test(
        special_asset(),
        (fixed!(500), fixed!(500)),
        balance!(1000),
        SwapAmount::with_desired_output(balance!(5000), balance!(1000000)),
        mock_liquidity_source::Error::<Runtime, mock_liquidity_source::Instance1>::InsufficientLiquidity.into(),
    );
}
