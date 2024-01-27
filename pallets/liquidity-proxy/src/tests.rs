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
use crate::test_utils::calculate_swap_batch_input_amount_with_adar_commission;
use crate::weights::WeightInfo;
use crate::{test_utils, BatchReceiverInfo, Error, QuoteInfo, SwapBatchInfo};
use common::prelude::fixnum::ops::CheckedSub;
use common::prelude::{
    AssetName, AssetSymbol, Balance, FixedWrapper, QuoteAmount, SwapAmount, SwapVariant,
};
use common::test_utils::assert_event;
use common::{
    assert_approx_eq, balance, fixed, fixed_wrapper, AssetInfoProvider, BuyBackHandler, FilterMode,
    Fixed, LiquidityProxyTrait, LiquiditySource, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType, ReferencePriceProvider, RewardReason, TradingPairSourceManager, DAI, DOT,
    ETH, KSM, PSWAP, USDT, VAL, XOR, XST, XSTUSD,
};
use core::convert::TryInto;
use frame_support::weights::Weight;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError;
use test_utils::mcbc_excluding_filter;

#[test]
#[ignore] // dependency on sampling which is removed
fn test_quote_exact_input_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount: Balance = balance!(500);
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_input(amount),
            mcbc_excluding_filter(DEX_C_ID),
            false,
            true,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            DEX_C_ID,
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_input(amount),
            mcbc_excluding_filter(DEX_C_ID),
            true,
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
                    QuoteAmount::with_desired_input(balance!(0.1)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    QuoteAmount::with_desired_input(balance!(0.225)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    QuoteAmount::with_desired_input(balance!(0.025)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    QuoteAmount::with_desired_input(balance!(0.65)),
                ),
            ]
        );
    });
}

#[test]
#[ignore] // dependency on sampling which is removed
fn test_quote_exact_input_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    let amount = balance!(500);
    ext.execute_with(|| {
        let (quotes, rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_input(amount),
            mcbc_excluding_filter(DEX_C_ID),
            false,
            true,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            DEX_C_ID,
            &DOT,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_input(amount),
            mcbc_excluding_filter(DEX_C_ID),
            true,
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
                    QuoteAmount::with_desired_input(balance!(0.275)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    QuoteAmount::with_desired_input(balance!(0.2)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    QuoteAmount::with_desired_input(balance!(0.225)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    QuoteAmount::with_desired_input(balance!(0.3)),
                ),
            ]
        );
    });
}

#[test]
#[ignore] // dependency on sampling which is removed
fn test_quote_exact_output_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount: Balance = balance!(250);
        let (quotes, rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_output(amount),
            mcbc_excluding_filter(DEX_C_ID),
            false,
            true,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            DEX_C_ID,
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_output(amount),
            mcbc_excluding_filter(DEX_C_ID),
            true,
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
                    QuoteAmount::with_desired_input(balance!(0)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    QuoteAmount::with_desired_input(balance!(0.2)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    QuoteAmount::with_desired_input(balance!(0)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    QuoteAmount::with_desired_input(balance!(0.8)),
                ),
            ]
        );
    });
}

#[test]
#[ignore] // dependency on sampling which is removed
fn test_quote_exact_output_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount = balance!(250);
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(amount),
            mcbc_excluding_filter(DEX_C_ID),
            false,
            true,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            DEX_C_ID,
            &DOT,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(amount),
            mcbc_excluding_filter(DEX_C_ID),
            true,
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
                    QuoteAmount::with_desired_input(balance!(0.325)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    QuoteAmount::with_desired_input(balance!(0.175)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    QuoteAmount::with_desired_input(balance!(0.325)),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    QuoteAmount::with_desired_input(balance!(0.175)),
                ),
            ]
        );
    });
}

#[test]
#[ignore] // dependency on sampling which is removed
fn test_poly_quote_exact_input_1_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let QuoteInfo {
            outcome: quotes, ..
        } = LiquidityProxy::inner_quote(
            DEX_A_ID,
            &KSM,
            &DOT,
            QuoteAmount::with_desired_input(balance!(100)),
            LiquiditySourceFilter::empty(DEX_A_ID),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            DEX_A_ID,
            &KSM,
            &DOT,
            QuoteAmount::with_desired_input(balance!(100)),
            LiquiditySourceFilter::empty(DEX_A_ID),
            true,
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let ls_swap = LiquidityProxy::exchange(
            DEX_A_ID,
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
#[ignore] // dependency on sampling which is removed
fn test_poly_quote_exact_output_1_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let QuoteInfo {
            outcome: quotes, ..
        } = LiquidityProxy::inner_quote(
            DEX_A_ID,
            &KSM,
            &DOT,
            QuoteAmount::with_desired_output(balance!(934.572151021276260545)),
            LiquiditySourceFilter::empty(DEX_A_ID),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            DEX_A_ID,
            &KSM,
            &DOT,
            SwapAmount::with_desired_output(balance!(934.572151021276260545), balance!(101)).into(),
            LiquiditySourceFilter::empty(DEX_A_ID),
            true,
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let ls_swap = LiquidityProxy::exchange(
            DEX_A_ID,
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
#[ignore] // dependency on sampling which is removed
fn test_poly_quote_exact_input_2_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let QuoteInfo {
            outcome: quotes, ..
        } = LiquidityProxy::inner_quote(
            DEX_A_ID,
            &DOT,
            &KSM,
            QuoteAmount::with_desired_input(balance!(500)),
            LiquiditySourceFilter::empty(DEX_A_ID),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            DEX_A_ID,
            &DOT,
            &KSM,
            QuoteAmount::with_desired_input(balance!(500)),
            LiquiditySourceFilter::empty(DEX_A_ID),
            true,
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let ls_swap = LiquidityProxy::exchange(
            DEX_A_ID,
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
#[ignore] // dependency on sampling which is removed
fn test_poly_quote_exact_output_2_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let QuoteInfo {
            outcome: quotes, ..
        } = LiquidityProxy::inner_quote(
            DEX_A_ID,
            &DOT,
            &KSM,
            QuoteAmount::with_desired_output(balance!(555.083861089846196673)),
            LiquiditySourceFilter::empty(DEX_A_ID),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            DEX_A_ID,
            &DOT,
            &KSM,
            QuoteAmount::with_desired_output(balance!(555.083861089846196673)),
            LiquiditySourceFilter::empty(DEX_A_ID),
            true,
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let ls_swap = LiquidityProxy::exchange(
            DEX_A_ID,
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
#[ignore] // dependency on sampling which is removed
fn test_sell_token_for_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let filter = mcbc_excluding_filter(DEX_C_ID);
        let (outcome, _, _) = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(balance!(500), balance!(345)),
            filter,
        )
        .expect("Failed to swap assets");
        assert_eq!(outcome.amount, balance!(363.569067258883248731));
    });
}

#[test]
#[ignore] // dependency on sampling which is removed
fn test_sell_base_for_token_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let filter = mcbc_excluding_filter(DEX_C_ID);
        let (outcome, _, _) = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(500), balance!(510)),
            filter,
        )
        .expect("Failed to swap assets");
        assert_eq!(outcome.amount, balance!(537.643138033120596095));
    });
}

#[test]
#[ignore] // dependency on sampling which is removed
fn test_sell_token_for_base_with_liquidity_source_trait_should_pass() {
    let mut ext = ExtBuilder::default().build();
    let amount = balance!(500);
    ext.execute_with(|| {
        let alice = alice();
        let result = LiquidityProxy::exchange(
            DEX_C_ID,
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
#[ignore] // dependency on sampling which is removed
fn test_sell_base_for_token_with_liquidity_source_trait_should_pass() {
    let mut ext = ExtBuilder::default().build();
    let amount: Balance = balance!(500);
    ext.execute_with(|| {
        let alice = alice();
        let result = LiquidityProxy::exchange(
            DEX_C_ID,
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
#[ignore] // dependency on sampling which is removed
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
        let (outcome, _, _) = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(200), balance!(298)),
            filter,
        )
        .expect("Failed to swap assets");
        let tolerance = fixed!(0.0000000001);
        let approx_expected_target_amount = fixed!(284.281354954553);
        assert!(
            Fixed::from_bits(outcome.amount.try_into().unwrap())
                .csub(approx_expected_target_amount)
                .unwrap()
                < tolerance
        );
        assert!(
            approx_expected_target_amount
                .csub(Fixed::from_bits(outcome.amount.try_into().unwrap()))
                .unwrap()
                < tolerance
        );
    });
}

#[test]
#[ignore] // dependency on sampling which is removed
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
        let (outcome, _, _) = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(200), balance!(291)),
            filter,
        )
        .expect("Failed to swap assets");
        let tolerance = fixed!(0.0000000001);
        let approx_expected_target_amount: Fixed = fixed!(277.348779693090);
        assert!(
            Fixed::from_bits(outcome.amount.try_into().unwrap())
                .csub(approx_expected_target_amount)
                .unwrap()
                < tolerance
        );
        assert!(
            approx_expected_target_amount
                .csub(Fixed::from_bits(outcome.amount.try_into().unwrap()))
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
            &GetBaseAssetId::get(),
            &KSM,
            QuoteAmount::with_desired_output(balance!(300)),
            mcbc_excluding_filter(DEX_C_ID),
            false,
            true,
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
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_output(balance!(300)),
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
            true,
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
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_output(balance!(5000)),
            mcbc_excluding_filter(DEX_C_ID),
            false,
            true,
        );
        assert_noop!(result, Error::<Runtime>::UnavailableExchangePath);
    });
}

#[test]
#[ignore] // dependency on sampling which is removed
fn test_sell_however_big_amount_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let (outcome, _, _) = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(2000), 0),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(outcome.amount > 0 && outcome.amount < balance!(180));

        let (outcome, _, _) = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(4000), 0),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(outcome.amount > 0 && outcome.amount < balance!(180));

        let (outcome, _, _) = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(10000), 0),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(outcome.amount > 0 && outcome.amount < balance!(180));

        let (outcome, _, _) = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(100000), 0),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(outcome.amount > 0 && outcome.amount < balance!(180));

        let (outcome, _, _) = LiquidityProxy::exchange_single(
            &alice,
            &alice,
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(1000000), 0),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(outcome.amount > 0 && outcome.amount < balance!(180));
    });
}

#[test]
fn test_swap_weight_considers_available_sources() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let swap_base_weight = <Runtime as crate::Config>::WeightInfo::check_indivisible_assets()
            .saturating_add(<Runtime as crate::Config>::WeightInfo::is_forbidden_filter());

        let quote_single_weight = <Runtime as crate::Config>::WeightInfo::list_liquidity_sources()
            .saturating_add(
                <Runtime as crate::Config>::LiquidityRegistry::quote_weight().saturating_mul(4),
            )
            .saturating_add(
                <Runtime as crate::Config>::LiquidityRegistry::check_rewards_weight()
                    .saturating_mul(2),
            );
        let exchange_base_weight = <Runtime as crate::Config>::WeightInfo::new_trivial()
            .saturating_add(quote_single_weight); // once within a path
        let multicollateral_weight =
            <Runtime as dex_api::Config>::MulticollateralBondingCurvePool::exchange_weight();
        let xst_weight = <Runtime as dex_api::Config>::XSTPool::exchange_weight();

        // ETH -1-> XOR -2-> XST (DEX 0)
        // 1) Multicollateral
        // 2) MockPool
        let swap_weight_without_path = swap_base_weight
            .saturating_add(exchange_base_weight)
            .saturating_add(quote_single_weight.saturating_mul(1)) // for each available path
            .saturating_add(quote_single_weight); // WithDesiredOutput
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_D_ID,
                &ETH,
                &XST,
                SwapVariant::WithDesiredOutput,
                &Vec::new(),
                &FilterMode::Disabled,
            ),
            swap_weight_without_path
                .saturating_add(multicollateral_weight)
                .saturating_add(Weight::zero()) // `MockSource`s are not counted
        );

        // DOT -1-> XOR (DEX ID 1)
        // 1) Multicollateral + MockPool(1-3)
        let swap_weight_without_path = swap_base_weight
            .saturating_add(exchange_base_weight)
            .saturating_add(quote_single_weight.saturating_mul(1)) // for each available path
            .saturating_add(quote_single_weight); // WithDesiredOutput
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &DOT,
                &XOR,
                SwapVariant::WithDesiredOutput,
                &Vec::new(),
                &FilterMode::Disabled,
            ),
            swap_weight_without_path.saturating_add(multicollateral_weight)
        );

        // DOT -1-> XOR (DEX ID 1)
        // 1) Multicollateral + MockPool(1-3)
        // (WithDesiredInput)
        let swap_weight_without_path = swap_base_weight
            .saturating_add(exchange_base_weight)
            .saturating_add(quote_single_weight.saturating_mul(1)) // for each available path
            .saturating_add(Weight::zero()); // WithDesiredInput
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &DOT,
                &XOR,
                SwapVariant::WithDesiredInput,
                &Vec::new(),
                &FilterMode::Disabled,
            ),
            swap_weight_without_path.saturating_add(multicollateral_weight)
        );

        // Two paths (DEX ID 1):
        //
        // XSTUSD -1-> XST -2-> XOR
        // 1) XSTPool
        // 2) Multicollateral
        //
        // XSTUSD -1-> XOR
        // 1) Multicollateral

        // The first path is obviously more expensive (multicollateral + xst > multicollateral)

        let swap_weight_without_path = swap_base_weight
            .saturating_add(exchange_base_weight)
            .saturating_add(quote_single_weight.saturating_mul(2)) // for each available path
            .saturating_add(quote_single_weight); // WithDesiredOutput
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &XSTUSD,
                &XOR,
                SwapVariant::WithDesiredOutput,
                &Vec::new(),
                &FilterMode::Disabled,
            ),
            swap_weight_without_path
                .saturating_add(xst_weight)
                .saturating_add(multicollateral_weight)
        );
    });
}

#[test]
fn test_swap_weight_filters_sources() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let swap_base_weight = <Runtime as crate::Config>::WeightInfo::check_indivisible_assets()
            .saturating_add(<Runtime as crate::Config>::WeightInfo::is_forbidden_filter());

        let quote_single_weight = <Runtime as crate::Config>::WeightInfo::list_liquidity_sources()
            .saturating_add(
                <Runtime as crate::Config>::LiquidityRegistry::quote_weight().saturating_mul(4),
            )
            .saturating_add(
                <Runtime as crate::Config>::LiquidityRegistry::check_rewards_weight()
                    .saturating_mul(2),
            );
        let exchange_base_weight = <Runtime as crate::Config>::WeightInfo::new_trivial()
            .saturating_add(quote_single_weight); // once within a path
        let multicollateral_weight =
            <Runtime as dex_api::Config>::MulticollateralBondingCurvePool::exchange_weight();
        let xst_weight = <Runtime as dex_api::Config>::XSTPool::exchange_weight();

        // ETH -1-> XOR -2-> XST (DEX 0)
        // 1) Multicollateral
        // 2) MockPool
        let swap_weight_without_path = swap_base_weight
            .saturating_add(exchange_base_weight)
            .saturating_add(quote_single_weight.saturating_mul(1)) // for each available path
            .saturating_add(quote_single_weight); // WithDesiredOutput
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_D_ID,
                &ETH,
                &XST,
                SwapVariant::WithDesiredOutput,
                &Vec::from([
                    LiquiditySourceType::MockPool,
                    LiquiditySourceType::MulticollateralBondingCurvePool
                ]),
                &FilterMode::AllowSelected,
            ),
            swap_weight_without_path
                .saturating_add(multicollateral_weight)
                .saturating_add(Weight::zero()) // `MockSource`s are not counted
        );
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_D_ID,
                &ETH,
                &XST,
                SwapVariant::WithDesiredOutput,
                &Vec::from([LiquiditySourceType::MockPool]),
                &FilterMode::AllowSelected,
            ),
            swap_weight_without_path
                // Multicollateral is filtered out
                .saturating_add(Weight::zero()) // `MockSource`s are not counted
        );
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_D_ID,
                &ETH,
                &XST,
                SwapVariant::WithDesiredOutput,
                &Vec::from([LiquiditySourceType::MulticollateralBondingCurvePool]),
                &FilterMode::ForbidSelected,
            ),
            swap_weight_without_path
                // Multicollateral is filtered out
                .saturating_add(Weight::zero()) // `MockSource`s are not counted
        );
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_D_ID,
                &ETH,
                &XST,
                SwapVariant::WithDesiredOutput,
                &Vec::new(),
                &FilterMode::AllowSelected,
            ),
            swap_weight_without_path
        );

        // DOT -1-> XOR (DEX ID 1)
        // 1) Multicollateral + MockPool(1-3)
        let swap_weight_without_path = swap_base_weight
            .saturating_add(exchange_base_weight)
            .saturating_add(quote_single_weight.saturating_mul(1)) // for each available path
            .saturating_add(quote_single_weight); // WithDesiredOutput
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &DOT,
                &XOR,
                SwapVariant::WithDesiredOutput,
                &Vec::new(),
                &FilterMode::ForbidSelected,
            ),
            // Multicollateral is the heaviest
            swap_weight_without_path.saturating_add(multicollateral_weight)
        );
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &DOT,
                &XOR,
                SwapVariant::WithDesiredOutput,
                &Vec::from([LiquiditySourceType::MulticollateralBondingCurvePool]),
                &FilterMode::ForbidSelected,
            ),
            swap_weight_without_path.saturating_add(Weight::zero()) // `MockSource`s are not counted
        );

        // Two paths (DEX ID 1):
        //
        // XSTUSD -1-> XST -2-> XOR
        // 1) XSTPool
        // 2) Multicollateral
        //
        // XSTUSD -1-> XOR
        // 1) Multicollateral
        let swap_weight_without_path = swap_base_weight
            .saturating_add(exchange_base_weight)
            .saturating_add(quote_single_weight.saturating_mul(2)) // for each available path
            .saturating_add(quote_single_weight); // WithDesiredOutput
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &XSTUSD,
                &XOR,
                SwapVariant::WithDesiredOutput,
                &Vec::new(),
                &FilterMode::ForbidSelected,
            ),
            swap_weight_without_path
                .saturating_add(xst_weight)
                .saturating_add(multicollateral_weight)
        );
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &XSTUSD,
                &XOR,
                SwapVariant::WithDesiredOutput,
                &Vec::from([LiquiditySourceType::XSTPool]),
                &FilterMode::AllowSelected,
            ),
            swap_weight_without_path.saturating_add(xst_weight)
        );
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &XSTUSD,
                &XOR,
                SwapVariant::WithDesiredOutput,
                &Vec::from([LiquiditySourceType::MulticollateralBondingCurvePool]),
                &FilterMode::AllowSelected,
            ),
            swap_weight_without_path.saturating_add(multicollateral_weight)
        );
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &XSTUSD,
                &XOR,
                SwapVariant::WithDesiredOutput,
                &Vec::new(),
                &FilterMode::AllowSelected,
            ),
            swap_weight_without_path
        );
    });
}

#[test]
fn test_swap_should_fail_with_bad_origin() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::swap(
            RuntimeOrigin::root(),
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
fn test_swap_shoild_fail_with_non_divisible_assets() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // Register ETH as non-divisible asset
        assert_ok!(Assets::register_asset_id(
            alice(),
            ETH,
            AssetSymbol(b"ETH".to_vec()),
            AssetName(b"Ethereum".to_vec()),
            0,
            Balance::from(10u32),
            true,
            None,
            None,
        ));

        // Register DOT as non-divisible asset
        assert_ok!(Assets::register_asset_id(
            alice(),
            DOT,
            AssetSymbol(b"DOT".to_vec()),
            AssetName(b"Polkadot".to_vec()),
            0,
            Balance::from(10u32),
            true,
            None,
            None,
        ));

        assert_noop!(
            LiquidityProxy::swap(
                RuntimeOrigin::signed(alice()),
                DEX_C_ID,
                ETH,
                GetBaseAssetId::get(),
                SwapAmount::with_desired_input(balance!(500), balance!(300)),
                Vec::new(),
                FilterMode::Disabled,
            ),
            Error::<Runtime>::UnableToSwapIndivisibleAssets
        );

        assert_noop!(
            LiquidityProxy::swap(
                RuntimeOrigin::signed(alice()),
                DEX_C_ID,
                GetBaseAssetId::get(),
                DOT,
                SwapAmount::with_desired_input(balance!(500), balance!(300)),
                Vec::new(),
                FilterMode::Disabled,
            ),
            Error::<Runtime>::UnableToSwapIndivisibleAssets
        );

        assert_noop!(
            LiquidityProxy::swap(
                RuntimeOrigin::signed(alice()),
                DEX_C_ID,
                ETH,
                DOT,
                SwapAmount::with_desired_input(balance!(500), balance!(300)),
                Vec::new(),
                FilterMode::Disabled,
            ),
            Error::<Runtime>::UnableToSwapIndivisibleAssets
        );
    });
}

#[test]
#[ignore] // dependency on sampling which is removed
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
        let QuoteInfo {
            outcome: quotes, ..
        } = LiquidityProxy::inner_quote(
            DEX_C_ID,
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_output(amount),
            filter,
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;
        assert_eq!(quotes.fee, balance!(0.630925033164008153));
    });
}

#[test]
fn test_quote_single_source_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockMCBCPool::init(get_mcbc_reserves_normal()).unwrap();
        let amount = balance!(500);
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_input(amount),
            LiquiditySourceFilter::with_allowed(DEX_C_ID, [LiquiditySourceType::MockPool].into()),
            false,
            true,
        )
        .expect("Failed to get a quote");

        let ls_quote = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            DEX_C_ID,
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_input(amount),
            LiquiditySourceFilter::with_allowed(DEX_C_ID, [LiquiditySourceType::MockPool].into()),
            true,
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
                QuoteAmount::with_desired_input(balance!(500)),
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
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &VAL,
            QuoteAmount::with_desired_input(balance!(100)),
            filter.clone(),
            false,
            true,
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
            &[(
                LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                QuoteAmount::with_desired_input(balance!(100)),
            ),]
        );

        // Buying KSM for XOR
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &KSM,
            QuoteAmount::with_desired_input(balance!(200)),
            filter.clone(),
            false,
            true,
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
                        LiquiditySourceType::MulticollateralBondingCurvePool,
                    ),
                    QuoteAmount::with_desired_input(balance!(105.149780332243106453)),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    QuoteAmount::with_desired_input(balance!(94.850219667756893547)),
                ),
            ]
        );

        // Buying DOT for XOR
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_input(balance!(200)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(1516.342527519604340858));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool,
                    ),
                    QuoteAmount::with_desired_input(balance!(105.149780332243106818)),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    QuoteAmount::with_desired_input(balance!(94.850219667756893182)),
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
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &VAL,
            QuoteAmount::with_desired_output(balance!(20000)),
            filter.clone(),
            false,
            true,
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
            &[(
                LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                QuoteAmount::with_desired_output(balance!(20000)),
            ),]
        );

        // Buying KSM for XOR
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &KSM,
            QuoteAmount::with_desired_output(balance!(200)),
            filter.clone(),
            false,
            true,
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
                        LiquiditySourceType::MulticollateralBondingCurvePool,
                    ),
                    QuoteAmount::with_desired_output(balance!(113.366944661581080036)),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    QuoteAmount::with_desired_output(balance!(86.633055338418919964)),
                ),
            ]
        );

        // Buying DOT for XOR
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_output(balance!(1000)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(125.000000000000000000));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[(
                LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                QuoteAmount::with_desired_output(balance!(1000)),
            ),]
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
        let (quotes, rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(balance!(100)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            rewards,
            vec![(
                balance!(23.258770902877438466),
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
                        LiquiditySourceType::MulticollateralBondingCurvePool,
                    ),
                    QuoteAmount::with_desired_output(balance!(23.258770902877438466)),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    QuoteAmount::with_desired_output(balance!(76.741229097122561534)),
                ),
            ]
        );

        // Buying XOR for KSM
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &KSM,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(balance!(200)),
            filter.clone(),
            false,
            true,
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
                        LiquiditySourceType::MulticollateralBondingCurvePool,
                    ),
                    QuoteAmount::with_desired_output(balance!(179.263806543072651075)),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    QuoteAmount::with_desired_output(balance!(20.736193456927348925)),
                ),
            ]
        );

        // Buying XOR for DOT
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(balance!(100)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(934.530528433224671738));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool,
                    ),
                    QuoteAmount::with_desired_output(balance!(79.263806543072650867)),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    QuoteAmount::with_desired_output(balance!(20.736193456927349133)),
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
        let (quotes, rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_input(balance!(20000)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            rewards,
            vec![(
                balance!(14.388332979612791988),
                XOR.into(),
                RewardReason::BuyOnBondingCurve
            )]
        );
        assert_eq!(quotes.amount, balance!(91.129562076735353496));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool,
                    ),
                    QuoteAmount::with_desired_input(balance!(3376.008652032533006925)),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    QuoteAmount::with_desired_input(balance!(16623.991347967466993075)),
                ),
            ]
        );

        // Buying XOR for KSM
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &KSM,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_input(balance!(200)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(182.802146328804827615));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool,
                    ),
                    QuoteAmount::with_desired_input(balance!(178.824711667708029000)),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    QuoteAmount::with_desired_input(balance!(21.175288332291971000)),
                ),
            ]
        );

        // Buying XOR for DOT
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_input(balance!(500)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(53.662213070708617870));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool,
                    ),
                    QuoteAmount::with_desired_input(balance!(309.422405009372255000)),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    QuoteAmount::with_desired_input(balance!(190.577594990627745000)),
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
        let (quotes, rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &VAL,
            QuoteAmount::with_desired_output(balance!(20000)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(rewards, Vec::new());
        assert_eq!(quotes.amount, balance!(111.111111111111111112));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[(
                LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                QuoteAmount::with_desired_output(balance!(20000)),
            ),]
        );

        // Buying KSM for XOR
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &KSM,
            QuoteAmount::with_desired_output(balance!(200)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(250.0));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[(
                LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                QuoteAmount::with_desired_output(balance!(200)),
            ),]
        );

        // Buying DOT for XOR
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_output(balance!(1000)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(125.0));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[(
                LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                QuoteAmount::with_desired_output(balance!(1000)),
            ),]
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

        let (_, rewards_forward, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(balance!(100)),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");

        let (_, rewards_backward, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &VAL,
            QuoteAmount::with_desired_output(balance!(100)),
            filter.clone(),
            false,
            true,
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
#[ignore] // dependency on sampling which is removed
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
        let QuoteInfo { rewards, .. } = LiquidityProxy::inner_quote(
            DEX_C_ID,
            &GetBaseAssetId::get(),
            &DOT,
            QuoteAmount::with_desired_input(amount),
            mcbc_excluding_filter(DEX_C_ID),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;

        assert_eq!(
            rewards,
            vec![
                (balance!(101), PSWAP.into(), RewardReason::Unspecified),
                (balance!(201), VAL.into(), RewardReason::Unspecified),
                (balance!(202), XOR.into(), RewardReason::Unspecified),
                (balance!(301), DOT.into(), RewardReason::Unspecified),
            ]
        );
    });
}

#[test]
fn test_quote_should_work_for_synthetics() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let pairs = [
            (XOR, XST),
            (VAL, XST),
            (XST, XSTUSD),
            (XOR, XSTUSD),
            (VAL, XSTUSD),
        ];

        for (from, to) in pairs
            .into_iter()
            .flat_map(|(from, to)| [(from, to), (to, from)].into_iter())
        {
            let amount: Balance = balance!(1);
            LiquidityProxy::inner_quote(
                0,
                &from,
                &to,
                QuoteAmount::with_desired_input(amount),
                mcbc_excluding_filter(0),
                false,
                true,
            )
            .expect(&format!("Failed to get a quote for {}-{} pair", from, to))
            .0;
        }
    });
}

#[test]
#[rustfmt::skip]
fn test_list_enabled_sources_for_path_query_should_pass_1() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
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
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
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
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
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
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
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
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
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
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
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
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
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
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
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
#[rustfmt::skip]
fn test_is_path_available_should_pass_5() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        assets::Pallet::<Runtime>::register_asset_id(
            alice(),
            XST.into(),
            AssetSymbol(b"XST".to_vec()),
            AssetName(b"SORA Synthetics".to_vec()),
            0,
            Balance::from(0u32),
            true,
            None,
            None,
        ).expect("failed to register XST asset");
        assets::Pallet::<Runtime>::register_asset_id(
            alice(),
            XSTUSD.into(),
            AssetSymbol(b"XSTUSD".to_vec()),
            AssetName(b"SORA Synthetic USD".to_vec()),
            0,
            Balance::from(0u32),
            true,
            None,
            None,
        ).expect("failed to register XSTUSD asset");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, XST).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XST, XSTUSD).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &XST, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XST, &XSTUSD, XSTPool).expect("failed to enable source");
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, PSWAP).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, XOR).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, XST).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, VAL, XSTUSD).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, VAL).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, XST).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XOR, XSTUSD).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XST, XSTUSD).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XST, XOR).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XST, VAL).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XSTUSD, XST).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XSTUSD, XOR).unwrap(), true);
        assert_eq!(LiquidityProxy::is_path_available(0, XSTUSD, VAL).unwrap(), true);
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
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_input(amount_base.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_input(amount_base.clone()),
                filter_xyk.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_xyk.0.amount);

            // collateral - > base, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_output(amount_base.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_output(amount_base.clone()),
                filter_xyk.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount <= quotes_xyk.0.amount);

            // collateral - > base, fixed input
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_input(amount_collateral.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_input(amount_collateral.clone()),
                filter_xyk.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_xyk.0.amount);

            // base -> collateral, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_output(amount_collateral.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_output(amount_collateral.clone()),
                filter_xyk.clone(),
                false,
                true,
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
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_input(amount_base.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_input(amount_base.clone()),
                filter_xyk.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_xyk.0.amount);

            // collateral - > base, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_output(amount_base.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_output(amount_base.clone()),
                filter_xyk.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount <= quotes_xyk.0.amount);

            // collateral - > base, fixed input
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_input(amount_collateral.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_input(amount_collateral.clone()),
                filter_xyk.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_xyk.0.amount);

            // base -> collateral, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_output(amount_collateral.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_xyk = LiquidityProxy::quote_single(
                &base_asset,
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_output(amount_collateral.clone()),
                filter_xyk.clone(),
                false,
                true,
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
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_input(amount_base.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_mcbc = LiquidityProxy::quote_single(
                &base_asset,
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_input(amount_base.clone()),
                filter_mcbc.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_mcbc.0.amount);

            // collateral - > base, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_output(amount_base.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_mcbc = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_output(amount_base.clone()),
                filter_mcbc.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount <= quotes_mcbc.0.amount);

            // collateral - > base, fixed input
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_input(amount_collateral.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_mcbc = LiquidityProxy::quote_single(
                &base_asset,
                &collateral_asset_id,
                &base_asset,
                QuoteAmount::with_desired_input(amount_collateral.clone()),
                filter_mcbc.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            assert!(quotes_smart.0.amount >= quotes_mcbc.0.amount);

            // base -> collateral, fixed output
            let quotes_smart = LiquidityProxy::quote_single(
                &base_asset,
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_output(amount_collateral.clone()),
                filter_both.clone(),
                false,
                true,
            )
            .expect("Failed to get a quote");
            let quotes_mcbc = LiquidityProxy::quote_single(
                &base_asset,
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_output(amount_collateral.clone()),
                filter_mcbc.clone(),
                false,
                true,
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
        _amount_base: Balance,
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
                &base_asset,
                &collateral_asset_id,
                QuoteAmount::with_desired_output(amount_collateral.clone()),
                filter_both.clone(),
                false,
                true,
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
        amount: QuoteAmount<Balance>,
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
                &GetBaseAssetId::get(),
                &collateral_asset_id,
                amount,
                LiquiditySourceFilter::empty(0),
                false,
                true,
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
        QuoteAmount::with_desired_output(balance!(5000)),
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
        QuoteAmount::with_desired_output(balance!(5000)),
        mock_liquidity_source::Error::<Runtime, mock_liquidity_source::Instance1>::InsufficientLiquidity.into(),
    );
}

#[test]
#[rustfmt::skip]
fn selecting_xyk_only_filter_is_forbidden() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        use FilterMode::*;

        // xyk only selection, base case
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &VAL, &vec![XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &PSWAP, &vec![XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &DAI, &vec![XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &ETH, &vec![XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&VAL, &XOR, &vec![XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&PSWAP, &XOR, &vec![XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&DAI, &XOR, &vec![XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&ETH, &XOR, &vec![XYKPool], &AllowSelected), true);

        // xyk only selection, indirect swaps
        assert_eq!(LiquidityProxy::is_forbidden_filter(&DAI, &PSWAP, &vec![XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&PSWAP, &VAL, &vec![XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&USDT, &VAL, &vec![XYKPool], &AllowSelected), true);

        // xyk only selection, non-reserve assets
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &USDT, &vec![XYKPool], &AllowSelected), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&USDT, &XOR, &vec![XYKPool], &AllowSelected), false);

        let mut sources_except_xyk = vec![MulticollateralBondingCurvePool, XSTPool, OrderBook];
        
        // xyk only selection, base case
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &VAL, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &PSWAP, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &DAI, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &ETH, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&VAL, &XOR, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&PSWAP, &XOR, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&DAI, &XOR, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&ETH, &XOR, &sources_except_xyk, &ForbidSelected), true);

        // xyk only selection, indirect swaps
        assert_eq!(LiquidityProxy::is_forbidden_filter(&DAI, &PSWAP, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&PSWAP, &VAL, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&USDT, &VAL, &sources_except_xyk, &ForbidSelected), true);

        // xyk only selection, non-reserve assets
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &USDT, &sources_except_xyk, &ForbidSelected), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&USDT, &XOR, &sources_except_xyk, &ForbidSelected), false);

        // smart selection, base case
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &VAL, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &PSWAP, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &DAI, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &ETH, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&VAL, &XOR, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&PSWAP, &XOR, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&DAI, &XOR, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &DAI, &vec![], &Disabled), false);

        // smart selection, indirect swaps
        assert_eq!(LiquidityProxy::is_forbidden_filter(&DAI, &PSWAP, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&PSWAP, &VAL, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&USDT, &VAL, &vec![], &Disabled), false);

        // smart selection, non-reserve assets
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &USDT, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&USDT, &XOR, &vec![], &Disabled), false);

        // tbc only selection, base case
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &VAL, &vec![MulticollateralBondingCurvePool], &AllowSelected), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &PSWAP, &vec![MulticollateralBondingCurvePool], &AllowSelected), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &DAI, &vec![MulticollateralBondingCurvePool], &AllowSelected), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &ETH, &vec![MulticollateralBondingCurvePool], &AllowSelected), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&VAL, &XOR, &vec![MulticollateralBondingCurvePool], &AllowSelected), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&PSWAP, &XOR, &vec![MulticollateralBondingCurvePool], &AllowSelected), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&DAI, &XOR, &vec![MulticollateralBondingCurvePool], &AllowSelected), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&ETH, &XOR, &vec![MulticollateralBondingCurvePool], &AllowSelected), false);

        // tbc only selection, indirect swaps
        assert_eq!(LiquidityProxy::is_forbidden_filter(&DAI, &PSWAP, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&PSWAP, &VAL, &vec![], &Disabled), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&USDT, &VAL, &vec![], &Disabled), false);

        // tbc only selection, non-reserve assets
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &USDT, &vec![MulticollateralBondingCurvePool], &AllowSelected), false);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&USDT, &XOR, &vec![MulticollateralBondingCurvePool], &AllowSelected), false);

        // hack cases with unavailable sources
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &VAL, &vec![MockPool, XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&VAL, &PSWAP, &vec![MockPool, XYKPool], &AllowSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &USDT, &vec![MockPool, XYKPool], &AllowSelected), false);

        sources_except_xyk.push(MockPool);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &VAL, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&VAL, &PSWAP, &sources_except_xyk, &ForbidSelected), true);
        assert_eq!(LiquidityProxy::is_forbidden_filter(&XOR, &USDT, &sources_except_xyk, &ForbidSelected), false);
    });
}

#[test]
#[rustfmt::skip]
fn test_list_enabled_sources_for_path_with_xyk_forbidden_1() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, USDT).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &USDT, XYKPool).expect("failed to enable source");
        let query_a = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, XOR, VAL);
        let query_b = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, VAL, XOR);
        let query_c = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, XOR, PSWAP);
        let query_d = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, PSWAP, XOR);
        let query_e = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, XOR, USDT);
        let query_f = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, USDT, XOR);
        let query_g = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, PSWAP, USDT);
        let query_h = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, USDT, VAL);

        assert_eq!(query_a.unwrap_err(), Error::<Runtime>::UnavailableExchangePath.into());
        assert_eq!(query_b.unwrap_err(), Error::<Runtime>::UnavailableExchangePath.into());
        assert_eq!(query_c.unwrap(), vec![]);
        assert_eq!(query_d.unwrap(), vec![]);
        assert_eq!(query_e.unwrap(), vec![XYKPool]);
        assert_eq!(query_f.unwrap(), vec![XYKPool]);
        assert_eq!(query_g.unwrap(), vec![]);
        assert_eq!(query_h.unwrap_err(), Error::<Runtime>::UnavailableExchangePath.into());
    });
}

#[test]
#[rustfmt::skip]
fn test_list_enabled_sources_for_path_with_xyk_forbidden_2() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, MulticollateralBondingCurvePool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, XYKPool).expect("failed to enable source");
        let query_a = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, XOR, VAL);
        let query_b = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, VAL, XOR);
        let query_c = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, XOR, PSWAP);
        let query_d = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, PSWAP, XOR);
        let query_e = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, VAL, PSWAP);
        let query_f = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, PSWAP, VAL);
        assert_eq!(query_a.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_b.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_c.unwrap(), vec![]);
        assert_eq!(query_d.unwrap(), vec![]);
        assert_eq!(query_e.unwrap(), vec![]);
        assert_eq!(query_f.unwrap(), vec![]);
    });
}

#[test]
#[rustfmt::skip]
fn test_list_enabled_sources_for_path_with_xyk_forbidden_3() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, MulticollateralBondingCurvePool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, MulticollateralBondingCurvePool).expect("failed to enable source");
        let query_a = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, XOR, VAL);
        let query_b = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, VAL, XOR);
        let query_c = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, XOR, PSWAP);
        let query_d = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, PSWAP, XOR);
        let query_e = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, VAL, PSWAP);
        let query_f = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, PSWAP, VAL);
        assert_eq!(query_a.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_b.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_c.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_d.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_e.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_f.unwrap(), vec![MulticollateralBondingCurvePool]);
    });
}

#[test]
#[rustfmt::skip]
fn test_list_enabled_sources_for_path_with_xyk_forbidden_4() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        use LiquiditySourceType::*;
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, VAL).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, PSWAP).expect("failed to register pair");
        TradingPair::register(RuntimeOrigin::signed(alice()), 0, XOR, USDT).expect("failed to register pair");

        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, MulticollateralBondingCurvePool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &VAL, MockPool2).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, XYKPool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, MulticollateralBondingCurvePool).expect("failed to enable source");
        TradingPair::enable_source_for_trading_pair(&0, &XOR, &PSWAP, MockPool3).expect("failed to enable source");
        let query_a = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, XOR, VAL);
        let query_b = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, VAL, XOR);
        let query_c = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, XOR, PSWAP);
        let query_d = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, PSWAP, XOR);
        let query_e = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, VAL, PSWAP);
        let query_f = LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(0, PSWAP, VAL);
        assert_eq!(query_a.unwrap(), vec![MulticollateralBondingCurvePool, MockPool2]);
        assert_eq!(query_b.unwrap(), vec![MulticollateralBondingCurvePool, MockPool2]);
        assert_eq!(query_c.unwrap(), vec![MulticollateralBondingCurvePool, MockPool3]);
        assert_eq!(query_d.unwrap(), vec![MulticollateralBondingCurvePool, MockPool3]);
        assert_eq!(query_e.unwrap(), vec![MulticollateralBondingCurvePool]);
        assert_eq!(query_f.unwrap(), vec![MulticollateralBondingCurvePool]);
    });
}

#[test]
fn test_quote_with_no_price_impact_with_desired_input() {
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
        let amount_val_in = balance!(45700);
        let amount_xor_intermediate = balance!(200);
        let amount_ksm_out = balance!(174);

        // Buying XOR for VAL
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_input(amount_val_in),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");
        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));
        assert_approx_eq!(quotes.amount, amount_xor_intermediate, balance!(1));
        assert_eq!(quotes.fee, balance!(0));
        assert!(matches!(
            dist.as_slice(),
            [
                (
                    LiquiditySourceId {
                        dex_id: _,
                        liquidity_source_index:
                            LiquiditySourceType::MulticollateralBondingCurvePool
                    },
                    _
                ),
                (
                    LiquiditySourceId {
                        dex_id: _,
                        liquidity_source_index: LiquiditySourceType::MockPool
                    },
                    _
                ),
            ]
        ));
        // without impact
        let QuoteInfo {
            amount_without_impact,
            ..
        } = LiquidityProxy::inner_quote(
            DEX_D_ID,
            &VAL,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_input(amount_val_in),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;
        assert_approx_eq!(quotes.amount, amount_without_impact.unwrap(), balance!(20));
        assert!(amount_without_impact.unwrap() > quotes.amount);

        // Buying KSM for XOR
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &KSM,
            QuoteAmount::with_desired_input(amount_xor_intermediate),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");
        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));
        assert_approx_eq!(quotes.amount, amount_ksm_out, balance!(1));
        assert_eq!(quotes.fee, balance!(0));
        assert!(matches!(
            dist.as_slice(),
            [
                (
                    LiquiditySourceId {
                        dex_id: _,
                        liquidity_source_index:
                            LiquiditySourceType::MulticollateralBondingCurvePool
                    },
                    _
                ),
                (
                    LiquiditySourceId {
                        dex_id: _,
                        liquidity_source_index: LiquiditySourceType::MockPool
                    },
                    _
                ),
            ]
        ));
        // without impact
        let QuoteInfo {
            amount_without_impact,
            ..
        } = LiquidityProxy::inner_quote(
            DEX_D_ID,
            &GetBaseAssetId::get(),
            &KSM,
            QuoteAmount::with_desired_input(amount_xor_intermediate),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;
        assert_approx_eq!(quotes.amount, amount_without_impact.unwrap(), balance!(20));
        assert!(amount_without_impact.unwrap() > quotes.amount);

        // Buying KSM for VAL
        let QuoteInfo {
            outcome: quotes,
            amount_without_impact,
            ..
        } = LiquidityProxy::inner_quote(
            DEX_D_ID,
            &VAL,
            &KSM,
            QuoteAmount::with_desired_input(amount_val_in),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;
        assert_approx_eq!(quotes.amount, amount_ksm_out, balance!(1));
        assert_approx_eq!(amount_without_impact.unwrap(), amount_ksm_out, balance!(20));
        assert!(amount_without_impact.unwrap() > quotes.amount);
    });
}

#[test]
fn test_quote_with_no_price_impact_with_desired_output() {
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
        let amount_val_in = balance!(45547);
        let amount_xor_intermediate = balance!(200);
        let amount_ksm_out = balance!(174);

        // Buying XOR for VAL
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(amount_xor_intermediate),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");
        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));
        assert_approx_eq!(quotes.amount, amount_val_in, balance!(1));
        assert_eq!(quotes.fee, balance!(0));
        assert!(matches!(
            dist.as_slice(),
            [
                (
                    LiquiditySourceId {
                        dex_id: _,
                        liquidity_source_index:
                            LiquiditySourceType::MulticollateralBondingCurvePool
                    },
                    _
                ),
                (
                    LiquiditySourceId {
                        dex_id: _,
                        liquidity_source_index: LiquiditySourceType::MockPool
                    },
                    _
                ),
            ]
        ));
        // without impact
        let QuoteInfo {
            amount_without_impact,
            ..
        } = LiquidityProxy::inner_quote(
            DEX_D_ID,
            &VAL,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(amount_xor_intermediate),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;
        assert_approx_eq!(
            quotes.amount,
            amount_without_impact.unwrap(),
            balance!(5000)
        );
        assert!(amount_without_impact.unwrap() < quotes.amount);

        // Buying KSM for XOR
        let (quotes, _rewards, _, _) = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &GetBaseAssetId::get(),
            &KSM,
            QuoteAmount::with_desired_output(amount_ksm_out),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote");
        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));
        assert_approx_eq!(quotes.amount, amount_xor_intermediate, balance!(1));
        assert_eq!(quotes.fee, balance!(0));
        assert!(matches!(
            dist.as_slice(),
            [
                (
                    LiquiditySourceId {
                        dex_id: _,
                        liquidity_source_index:
                            LiquiditySourceType::MulticollateralBondingCurvePool
                    },
                    _
                ),
                (
                    LiquiditySourceId {
                        dex_id: _,
                        liquidity_source_index: LiquiditySourceType::MockPool
                    },
                    _
                ),
            ]
        ));
        // without impact
        let QuoteInfo {
            amount_without_impact,
            ..
        } = LiquidityProxy::inner_quote(
            DEX_D_ID,
            &GetBaseAssetId::get(),
            &KSM,
            QuoteAmount::with_desired_output(amount_ksm_out),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;
        assert_approx_eq!(
            quotes.amount,
            amount_without_impact.unwrap(),
            balance!(5000)
        );
        assert!(amount_without_impact.unwrap() < quotes.amount);

        // Buying KSM for VAL
        let QuoteInfo {
            outcome: quotes,
            amount_without_impact,
            ..
        } = LiquidityProxy::inner_quote(
            DEX_D_ID,
            &VAL,
            &KSM,
            QuoteAmount::with_desired_output(amount_ksm_out),
            filter.clone(),
            false,
            true,
        )
        .expect("Failed to get a quote")
        .0;
        assert_approx_eq!(quotes.amount, amount_val_in, balance!(100));
        assert_approx_eq!(
            amount_without_impact.unwrap(),
            amount_val_in,
            balance!(5000)
        );
        assert!(amount_without_impact.unwrap() < quotes.amount);
    });
}

#[test]
fn test_quote_does_not_overflow_with_desired_input() {
    let collateral_asset_id = VAL;
    let mut ext = ExtBuilder::with_total_supply_and_reserves(
        balance!(200000000000),
        vec![(0, collateral_asset_id, (fixed!(3000000), fixed!(1100000)))],
    )
    .build();
    ext.execute_with(|| {
        MockMCBCPool::init(vec![(collateral_asset_id, balance!(1100000))]).unwrap();

        let base_asset = GetBaseAssetId::get();

        LiquidityProxy::quote_single(
            &base_asset,
            &collateral_asset_id,
            &base_asset,
            QuoteAmount::with_desired_input(balance!(1)),
            LiquiditySourceFilter::empty(0),
            false,
            true,
        )
        .expect("Failed to get a quote");
    });
}

#[test]
fn test_inner_exchange_returns_correct_sources() {
    use LiquiditySourceType::*;
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockMCBCPool::init(vec![(VAL, balance!(1100000)), (KSM, balance!(1100000))]).unwrap();

        let base_asset = GetBaseAssetId::get();
        let result_base = LiquidityProxy::inner_exchange(
            DEX_D_ID,
            &alice(),
            &common::mock::bob(),
            &VAL,
            &base_asset,
            SwapAmount::with_desired_input(balance!(100), 0),
            LiquiditySourceFilter::empty(0),
        );

        let selected_source_types: Vec<LiquiditySourceType> =
            vec![XYKPool, MulticollateralBondingCurvePool, MockPool];
        let filter_mode = FilterMode::AllowSelected;
        let filter = LiquiditySourceFilter::with_mode(0, filter_mode, selected_source_types);
        let result_val_ksm = LiquidityProxy::inner_exchange(
            DEX_D_ID,
            &alice(),
            &common::mock::bob(),
            &VAL,
            &KSM,
            SwapAmount::with_desired_input(balance!(100), 0),
            filter,
        );

        let (_, sources_base, _) = result_base.expect("inner_exchange: result is not ok!");
        let (_, sources_val_ksm, _) = result_val_ksm.expect("inner_exchange: result is not ok!");
        let multicoll_source = LiquiditySourceId {
            dex_id: 0,
            liquidity_source_index: LiquiditySourceType::MulticollateralBondingCurvePool,
        };

        let mock_source = LiquiditySourceId {
            dex_id: 0,
            liquidity_source_index: LiquiditySourceType::MockPool,
        };

        let check_vec = vec![multicoll_source, mock_source];
        assert_eq!(check_vec, sources_base);
        assert_eq!(check_vec, sources_val_ksm);
    });
}

#[test]
fn test_enable_correct_liquidity_source() {
    let mut ext = ExtBuilder::with_enabled_sources(vec![
        LiquiditySourceType::XYKPool,
        LiquiditySourceType::MulticollateralBondingCurvePool,
        LiquiditySourceType::XSTPool,
    ])
    .build();
    ext.execute_with(|| {
        // Only XST & TBC sources could be enabled/disabled
        assert_noop!(
            LiquidityProxy::enable_liquidity_source(
                RuntimeOrigin::root(),
                LiquiditySourceType::XYKPool
            ),
            Error::<Runtime>::UnableToEnableLiquiditySource
        );

        // User cannot enable liquidity source if it was not disabled
        assert_noop!(
            LiquidityProxy::enable_liquidity_source(
                RuntimeOrigin::root(),
                LiquiditySourceType::XSTPool
            ),
            Error::<Runtime>::LiquiditySourceAlreadyEnabled
        );

        // Disable XST & TBC that allows us to enable them
        assert_ok!(LiquidityProxy::disable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::XSTPool
        ));
        assert_ok!(LiquidityProxy::disable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::MulticollateralBondingCurvePool
        ));

        // Enable success
        assert_ok!(LiquidityProxy::enable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::XSTPool
        ));
        assert_ok!(LiquidityProxy::enable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::MulticollateralBondingCurvePool
        ));
    });
}

#[test]
fn test_double_enable_liquidity_source() {
    let mut ext = ExtBuilder::with_enabled_sources(vec![
        LiquiditySourceType::XYKPool,
        LiquiditySourceType::MulticollateralBondingCurvePool,
        LiquiditySourceType::XSTPool,
    ])
    .build();
    ext.execute_with(|| {
        // Disable TBC that allows us to enable it
        assert_ok!(LiquidityProxy::disable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::MulticollateralBondingCurvePool
        ));

        // Enable success
        assert_ok!(LiquidityProxy::enable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::MulticollateralBondingCurvePool
        ));

        // Second enabling failed
        assert_noop!(
            LiquidityProxy::enable_liquidity_source(
                RuntimeOrigin::root(),
                LiquiditySourceType::MulticollateralBondingCurvePool
            ),
            Error::<Runtime>::LiquiditySourceAlreadyEnabled
        );
    });
}

#[test]
fn test_disable_correct_liquidity_source() {
    let mut ext = ExtBuilder::with_enabled_sources(vec![
        LiquiditySourceType::XYKPool,
        LiquiditySourceType::MulticollateralBondingCurvePool,
        LiquiditySourceType::XSTPool,
    ])
    .build();
    ext.execute_with(|| {
        // Only XST & TBC sources could be enabled/disabled
        assert_noop!(
            LiquidityProxy::disable_liquidity_source(
                RuntimeOrigin::root(),
                LiquiditySourceType::XYKPool
            ),
            Error::<Runtime>::UnableToDisableLiquiditySource
        );

        // Disable success
        assert_ok!(LiquidityProxy::disable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::XSTPool
        ));
        assert_ok!(LiquidityProxy::disable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::MulticollateralBondingCurvePool
        ));
    });
}

#[test]
fn test_double_disable_liquidity_source() {
    let mut ext = ExtBuilder::with_enabled_sources(vec![
        LiquiditySourceType::XYKPool,
        LiquiditySourceType::MulticollateralBondingCurvePool,
        LiquiditySourceType::XSTPool,
    ])
    .build();
    ext.execute_with(|| {
        // Disable success
        assert_ok!(LiquidityProxy::disable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::MulticollateralBondingCurvePool
        ));

        // Second disabling failed
        assert_noop!(
            LiquidityProxy::disable_liquidity_source(
                RuntimeOrigin::root(),
                LiquiditySourceType::MulticollateralBondingCurvePool
            ),
            Error::<Runtime>::LiquiditySourceAlreadyDisabled
        );
    });
}

#[test]
fn test_disable_enable_liquidity_source() {
    let mut ext = ExtBuilder::with_enabled_sources(vec![
        LiquiditySourceType::XYKPool,
        LiquiditySourceType::MulticollateralBondingCurvePool,
        LiquiditySourceType::XSTPool,
    ])
    .build();
    ext.execute_with(|| {
        MockMCBCPool::init(get_mcbc_reserves_normal()).unwrap();

        // Check that TBC is enabled
        assert_ok!(LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(balance!(300)),
            LiquiditySourceFilter::with_allowed(
                DEX_C_ID,
                [LiquiditySourceType::MulticollateralBondingCurvePool].into()
            ),
            false,
            true,
        ));

        // Disable TBC
        assert_ok!(LiquidityProxy::disable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::MulticollateralBondingCurvePool
        ));

        // Check that TBC is disabled
        assert_noop!(
            LiquidityProxy::quote_single(
                &GetBaseAssetId::get(),
                &DOT,
                &GetBaseAssetId::get(),
                QuoteAmount::with_desired_output(balance!(300)),
                LiquiditySourceFilter::with_allowed(
                    DEX_C_ID,
                    [LiquiditySourceType::MulticollateralBondingCurvePool].into()
                ),
                false,
                true,
            ),
            Error::<Runtime>::UnavailableExchangePath
        );

        // Enable TBC
        assert_ok!(LiquidityProxy::enable_liquidity_source(
            RuntimeOrigin::root(),
            LiquiditySourceType::MulticollateralBondingCurvePool
        ));

        // Check that TBC is enabled again
        assert_ok!(LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            &GetBaseAssetId::get(),
            QuoteAmount::with_desired_output(balance!(300)),
            LiquiditySourceFilter::with_allowed(
                DEX_C_ID,
                [LiquiditySourceType::MulticollateralBondingCurvePool].into()
            ),
            false,
            true,
        ));
    });
}

#[test]
fn test_batch_swap_desired_input_successful() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        assert_eq!(Assets::free_balance(&XOR, &adar()).unwrap(), balance!(0));

        assert_eq!(Assets::free_balance(&USDT, &bob()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&KSM, &charlie()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&KSM, &dave()).unwrap(), balance!(0));

        let swap_batches = Vec::from([
            SwapBatchInfo {
                outcome_asset_id: USDT,
                dex_id: DEX_C_ID,
                receivers: vec![BatchReceiverInfo::new(bob(), balance!(10))],
                outcome_asset_reuse: 0,
            },
            SwapBatchInfo {
                outcome_asset_id: KSM,
                dex_id: DEX_A_ID,
                receivers: vec![
                    BatchReceiverInfo::new(charlie(), balance!(10)),
                    BatchReceiverInfo::new(dave(), balance!(10)),
                ],
                outcome_asset_reuse: 0,
            },
        ]);

        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let max_input_amount =
            calculate_swap_batch_input_amount_with_adar_commission(&swap_batches, sources.clone())
                + balance!(1);

        assert_ok!(LiquidityProxy::swap_transfer_batch(
            RuntimeOrigin::signed(alice()),
            swap_batches.clone(),
            XOR,
            max_input_amount,
            sources.clone(),
            filter_mode,
        ));

        test_utils::check_adar_commission(&swap_batches, sources);
        test_utils::check_swap_batch_executed_amount(swap_batches);
    });
}

#[test]
fn test_batch_swap_emits_event() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        frame_system::Pallet::<Runtime>::set_block_number(1);
        assert_eq!(Assets::free_balance(&XOR, &adar()).unwrap(), balance!(0));

        let swap_batches = Vec::from([SwapBatchInfo {
            outcome_asset_id: XOR,
            dex_id: DEX_C_ID,
            receivers: vec![
                BatchReceiverInfo::new(charlie(), balance!(10)),
                BatchReceiverInfo::new(dave(), balance!(10)),
            ],
            outcome_asset_reuse: 0,
        }]);

        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();

        let amount_in = balance!(20);
        let adar_fee = (FixedWrapper::from(amount_in) * fixed_wrapper!(0.0025)).into_balance();

        let max_input_amount = amount_in + adar_fee;

        assert_ok!(LiquidityProxy::swap_transfer_batch(
            RuntimeOrigin::signed(alice()),
            swap_batches.clone(),
            XOR,
            max_input_amount,
            sources.clone(),
            filter_mode,
        ));

        common::test_utils::assert_last_event::<Runtime>(
            crate::Event::BatchSwapExecuted(adar_fee, amount_in).into(),
        );
    });
}

#[test]
fn test_batch_swap_duplicate_receivers_successful() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        assert_eq!(Assets::free_balance(&XOR, &adar()).unwrap(), balance!(0));

        assert_eq!(Assets::free_balance(&USDT, &bob()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&KSM, &charlie()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&KSM, &dave()).unwrap(), balance!(0));

        let swap_batches = Vec::from([
            SwapBatchInfo {
                outcome_asset_id: USDT,
                dex_id: DEX_C_ID,
                receivers: vec![BatchReceiverInfo::new(bob(), balance!(10))],
                outcome_asset_reuse: 0,
            },
            SwapBatchInfo {
                outcome_asset_id: KSM,
                dex_id: DEX_A_ID,
                receivers: vec![
                    BatchReceiverInfo::new(charlie(), balance!(10)),
                    BatchReceiverInfo::new(dave(), balance!(10)),
                    BatchReceiverInfo::new(dave(), balance!(10)),
                ],
                outcome_asset_reuse: 0,
            },
        ]);

        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let max_input_amount =
            calculate_swap_batch_input_amount_with_adar_commission(&swap_batches, sources.clone())
                + balance!(1);

        assert_ok!(LiquidityProxy::swap_transfer_batch(
            RuntimeOrigin::signed(alice()),
            swap_batches.clone(),
            XOR,
            max_input_amount,
            sources.clone(),
            filter_mode,
        ));

        test_utils::check_adar_commission(&swap_batches, sources);
        test_utils::check_swap_batch_executed_amount(swap_batches);
    })
}

#[test]
fn test_batch_swap_desired_input_too_low() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        assert_eq!(Assets::free_balance(&USDT, &bob()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&KSM, &charlie()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&KSM, &dave()).unwrap(), balance!(0));

        let swap_batches = Vec::from([
            SwapBatchInfo {
                outcome_asset_id: USDT,
                dex_id: DEX_C_ID,
                receivers: vec![BatchReceiverInfo::new(bob(), balance!(10))],
                outcome_asset_reuse: 0,
            },
            SwapBatchInfo {
                outcome_asset_id: KSM,
                dex_id: DEX_A_ID,
                receivers: vec![
                    BatchReceiverInfo::new(charlie(), balance!(10)),
                    BatchReceiverInfo::new(dave(), balance!(10)),
                ],
                outcome_asset_reuse: 0,
            },
        ]);
        let sources = [LiquiditySourceType::XYKPool].to_vec();

        let max_input_amount =
            calculate_swap_batch_input_amount_with_adar_commission(&swap_batches, sources.clone())
                - balance!(1);

        assert_noop!(
            LiquidityProxy::swap_transfer_batch(
                RuntimeOrigin::signed(alice()),
                swap_batches,
                XOR,
                max_input_amount,
                sources,
                FilterMode::AllowSelected,
            ),
            Error::<Runtime>::SlippageNotTolerated
        );
    });
}

#[test]
fn test_batch_swap_fail_with_duplicate_asset_ids() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        assert_eq!(Assets::free_balance(&USDT, &bob()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&KSM, &charlie()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&KSM, &dave()).unwrap(), balance!(0));

        let swap_batches = Vec::from([
            SwapBatchInfo {
                outcome_asset_id: USDT,
                dex_id: DEX_C_ID,
                receivers: vec![BatchReceiverInfo::new(bob(), balance!(10))],
                outcome_asset_reuse: 0,
            },
            SwapBatchInfo {
                outcome_asset_id: USDT,
                dex_id: DEX_A_ID,
                receivers: vec![BatchReceiverInfo::new(bob(), balance!(10))],
                outcome_asset_reuse: 0,
            },
            SwapBatchInfo {
                outcome_asset_id: KSM,
                dex_id: DEX_A_ID,
                receivers: vec![
                    BatchReceiverInfo::new(charlie(), balance!(10)),
                    BatchReceiverInfo::new(dave(), balance!(10)),
                ],
                outcome_asset_reuse: 0,
            },
        ]);

        assert_noop!(
            LiquidityProxy::swap_transfer_batch(
                RuntimeOrigin::signed(alice()),
                swap_batches,
                XOR,
                balance!(100),
                [LiquiditySourceType::XYKPool].to_vec(),
                FilterMode::AllowSelected,
            ),
            Error::<Runtime>::AggregationError
        );
    });
}

#[test]
fn test_mint_buy_back_and_burn() {
    let mut ext = ExtBuilder::with_enabled_sources(vec![
        LiquiditySourceType::MulticollateralBondingCurvePool,
        LiquiditySourceType::XSTPool,
    ])
    .with_xyk_pool()
    .build();
    ext.execute_with(|| {
        let transit = <Runtime as crate::Config>::GetTechnicalAccountId::get();
        assert_eq!(Assets::free_balance(&KSM, &transit).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&USDT, &transit).unwrap(), balance!(0));
        assert_eq!(Assets::total_issuance(&USDT).unwrap(), balance!(24000));
        assert_eq!(Assets::total_issuance(&KSM).unwrap(), balance!(4000));

        assert_eq!(crate::LiquidityProxyBuyBackHandler::<
            Runtime,
            GetBuyBackDexId,
        >::mint_buy_back_and_burn(&USDT, &KSM, balance!(1)).unwrap(), balance!(1.984061762988045965));

        assert_eq!(Assets::total_issuance(&USDT).unwrap(), balance!(24001));
        assert_eq!(
            Assets::total_issuance(&KSM).unwrap(),
            balance!(3998.015938237011954035)
        );
        assert_eq!(Assets::free_balance(&KSM, &transit).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&USDT, &transit).unwrap(), balance!(0));
    });
}

#[test]
fn test_buy_back_handler() {
    let mut ext = ExtBuilder::with_enabled_sources(vec![
        LiquiditySourceType::MulticollateralBondingCurvePool,
        LiquiditySourceType::XSTPool,
    ])
    .with_xyk_pool()
    .build();
    ext.execute_with(|| {
        let transit = <Runtime as crate::Config>::GetTechnicalAccountId::get();
        assert_eq!(Assets::free_balance(&KSM, &transit).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&USDT, &transit).unwrap(), balance!(0));
        assert_eq!(Assets::total_issuance(&USDT).unwrap(), balance!(24000));
        assert_eq!(Assets::total_issuance(&KSM).unwrap(), balance!(4000));
        assert_eq!(
            Assets::free_balance(&KSM, &alice()).unwrap(),
            balance!(2000)
        );
        assert_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            balance!(12000)
        );

        assert_eq!(
            crate::LiquidityProxyBuyBackHandler::<Runtime, GetBuyBackDexId>::buy_back_and_burn(
                &alice(),
                &USDT,
                &KSM,
                balance!(1)
            )
            .unwrap(),
            balance!(1.984061762988045965)
        );

        assert_eq!(Assets::total_issuance(&USDT).unwrap(), balance!(24000));
        assert_eq!(
            Assets::total_issuance(&KSM).unwrap(),
            balance!(3998.015938237011954035)
        );
        assert_eq!(Assets::free_balance(&KSM, &transit).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&USDT, &transit).unwrap(), balance!(0));

        assert_eq!(
            Assets::free_balance(&KSM, &alice()).unwrap(),
            balance!(2000)
        );
        assert_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            balance!(11999)
        );
    });
}

#[test]
fn test_set_adar_commission_ratio() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert!(LiquidityProxy::adar_commission_ratio() == balance!(0.0025));
        assert_noop!(
            LiquidityProxy::set_adar_commission_ratio(
                RuntimeOrigin::signed(alice()),
                balance!(0.5)
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            LiquidityProxy::set_adar_commission_ratio(RuntimeOrigin::root(), balance!(1)),
            Error::<Runtime>::InvalidADARCommissionRatio
        );
        assert_ok!(LiquidityProxy::set_adar_commission_ratio(
            RuntimeOrigin::root(),
            balance!(0.5)
        ));
        assert!(LiquidityProxy::adar_commission_ratio() == balance!(0.5));
    })
}

#[test]
fn test_reference_price_provider() {
    let mut ext = ExtBuilder::with_enabled_sources(vec![
        LiquiditySourceType::MulticollateralBondingCurvePool,
        LiquiditySourceType::XSTPool,
    ])
    .with_xyk_pool()
    .build();
    ext.execute_with(|| {
        frame_support::parameter_types! {
            pub const GetReferenceDexId: DEXId = DEX_A_ID;
            pub const GetReferenceAssetId: AssetId = USDT;
        }

        assert_eq!(
            crate::ReferencePriceProvider::<Runtime, GetReferenceDexId, GetReferenceAssetId>::get_reference_price(
                &KSM,
            )
            .unwrap(),
            balance!(0.499500499500499500)
        );
    });
}

#[test]
fn test_batch_swap_asset_reuse_works() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        assert_eq!(Assets::free_balance(&XOR, &adar()).unwrap(), balance!(0));

        assert_eq!(Assets::free_balance(&USDT, &bob()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&KSM, &charlie()).unwrap(), balance!(0));
        assert_eq!(Assets::free_balance(&KSM, &dave()).unwrap(), balance!(0));
        assert_eq!(
            Assets::free_balance(&KSM, &alice()).unwrap(),
            balance!(2000)
        );
        assert_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            balance!(12000)
        );
        assert_approx_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            balance!(356400),
            balance!(0.00001)
        );

        let swap_batches = Vec::from([
            SwapBatchInfo {
                outcome_asset_id: USDT,
                dex_id: DEX_C_ID,
                receivers: vec![BatchReceiverInfo::new(bob(), balance!(10))],
                outcome_asset_reuse: balance!(20),
            },
            SwapBatchInfo {
                outcome_asset_id: KSM,
                dex_id: DEX_A_ID,
                receivers: vec![
                    BatchReceiverInfo::new(charlie(), balance!(10)),
                    BatchReceiverInfo::new(dave(), balance!(10)),
                ],
                outcome_asset_reuse: balance!(10),
            },
        ]);

        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let max_input_amount =
            calculate_swap_batch_input_amount_with_adar_commission(&swap_batches, sources.clone())
                + balance!(1);

        assert_ok!(LiquidityProxy::swap_transfer_batch(
            RuntimeOrigin::signed(alice()),
            swap_batches.clone(),
            XOR,
            max_input_amount,
            sources.clone(),
            filter_mode,
        ));

        test_utils::check_adar_commission(&swap_batches, sources);
        test_utils::check_swap_batch_executed_amount(swap_batches);
        assert_event::<Runtime>(
            crate::Event::<Runtime>::ADARFeeWithdrawn(KSM, balance!(0.025)).into(),
        );
        assert_event::<Runtime>(
            crate::Event::<Runtime>::ADARFeeWithdrawn(USDT, balance!(0.025)).into(),
        );
        assert_approx_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            balance!(356394.934457262),
            balance!(0.00001)
        );
        assert_approx_eq!(
            Assets::free_balance(&KSM, &alice()).unwrap(),
            balance!(1990),
            balance!(0.00001)
        );
        assert_approx_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            balance!(11989.975),
            balance!(0.00001)
        );
    });
}

#[test]
fn test_batch_swap_asset_reuse_fails() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        assert_eq!(Assets::free_balance(&XOR, &adar()).unwrap(), balance!(0));

        assert_eq!(Assets::free_balance(&USDT, &bob()).unwrap(), balance!(0));
        assert_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            balance!(12000)
        );

        let swap_batches = Vec::from([SwapBatchInfo {
            outcome_asset_id: USDT,
            dex_id: DEX_C_ID,
            receivers: vec![BatchReceiverInfo::new(bob(), balance!(10))],
            outcome_asset_reuse: balance!(1000000),
        }]);

        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let max_input_amount =
            calculate_swap_batch_input_amount_with_adar_commission(&swap_batches, sources.clone())
                + balance!(1);

        assert_noop!(
            LiquidityProxy::swap_transfer_batch(
                RuntimeOrigin::signed(alice()),
                swap_batches.clone(),
                XOR,
                max_input_amount,
                sources.clone(),
                filter_mode,
            ),
            Error::<Runtime>::InsufficientBalance
        );
    });
}

#[test]
fn test_xorless_transfer_works() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        assert_eq!(Assets::free_balance(&USDT, &bob()).unwrap(), balance!(0));
        assert_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            balance!(12000)
        );
        assert_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            balance!(356400)
        );

        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();

        assert_ok!(LiquidityProxy::xorless_transfer(
            RuntimeOrigin::signed(alice()),
            0,
            USDT,
            bob(),
            balance!(1),
            balance!(1),
            balance!(10),
            sources,
            filter_mode,
            Default::default(),
        ));

        assert_approx_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            // 12000 USDT - 1 USDT for swap - 1 USDT for transfer
            balance!(11998),
            balance!(0.01)
        );
        assert_approx_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            balance!(356401),
            balance!(0.01)
        );
        assert_approx_eq!(
            Assets::free_balance(&USDT, &bob()).unwrap(),
            balance!(1),
            balance!(0.01)
        );
    });
}

#[test]
fn test_xorless_transfer_without_swap_works() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        assert_eq!(Assets::free_balance(&USDT, &bob()).unwrap(), balance!(0));
        assert_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            balance!(12000)
        );
        assert_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            balance!(356400)
        );

        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();

        assert_ok!(LiquidityProxy::xorless_transfer(
            RuntimeOrigin::signed(alice()),
            0,
            USDT,
            bob(),
            balance!(1),
            balance!(0),
            balance!(0),
            sources,
            filter_mode,
            Default::default(),
        ));

        assert_approx_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            // 12000 USDT - 1 USDT for swap - 1 USDT for transfer
            balance!(11999),
            balance!(0.01)
        );
        assert_approx_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            balance!(356400),
            balance!(0.01)
        );
        assert_approx_eq!(
            Assets::free_balance(&USDT, &bob()).unwrap(),
            balance!(1),
            balance!(0.01)
        );
    });
}

#[test]
fn test_xorless_transfer_fails_on_swap() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        assert_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            balance!(12000)
        );

        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();

        assert_noop!(
            LiquidityProxy::xorless_transfer(
                RuntimeOrigin::signed(alice()),
                0,
                USDT,
                bob(),
                balance!(1),
                balance!(1),
                balance!(0.5),
                sources,
                filter_mode,
                Default::default(),
            ),
            Error::<Runtime>::SlippageNotTolerated
        );
    });
}

#[test]
fn test_xorless_transfer_fails_on_transfer() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        assert_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            balance!(12000)
        );

        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();

        assert_noop!(
            LiquidityProxy::xorless_transfer(
                RuntimeOrigin::signed(alice()),
                0,
                USDT,
                bob(),
                balance!(12000),
                balance!(1),
                balance!(2),
                sources,
                filter_mode,
                Default::default(),
            ),
            tokens::Error::<Runtime>::BalanceTooLow
        );
    });
}
