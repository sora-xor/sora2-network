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
use crate::{test_utils, BatchReceiverInfo, Error, ExchangePath, QuoteInfo, SwapBatchInfo};
use common::prelude::{
    AssetName, AssetSymbol, Balance, FixedWrapper, OutcomeFee, QuoteAmount, SwapAmount,
};
use common::{
    assert_approx_eq_abs, balance, fixed, fixed_wrapper, AssetInfoProvider, BuyBackHandler,
    DEXInfo, FilterMode, Fixed, LiquidityProxyTrait, LiquiditySource, LiquiditySourceFilter,
    LiquiditySourceId, LiquiditySourceType, ReferencePriceProvider, RewardReason,
    TradingPairSourceManager, DAI, DOT, ETH, KSM, KXOR, PSWAP, USDT, VAL, XOR, XST, XSTUSD,
};
use core::convert::TryInto;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{assert_noop, assert_ok};
use sp_core::bounded::BoundedVec;
use sp_runtime::DispatchError;
use test_utils::mcbc_excluding_filter;

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

        assert_noop!(result, Error::<Runtime>::InsufficientLiquidity);
    });
}

#[test]
fn test_swap_weight_considers_available_sources() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let swap_base_weight = <Runtime as crate::Config>::WeightInfo::check_indivisible_assets();

        let quote_single_weight = <Runtime as crate::Config>::WeightInfo::list_liquidity_sources()
            .saturating_add(<Runtime as crate::Config>::LiquidityRegistry::check_rewards_weight())
            .saturating_add(
                <Runtime as crate::Config>::LiquidityRegistry::step_quote_weight(
                    <Runtime as crate::Config>::GetNumSamples::get(),
                )
                .saturating_mul(4),
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
            .saturating_add(quote_single_weight.saturating_mul(1)); // for each available path
        assert_eq!(
            LiquidityProxy::swap_weight(&DEX_D_ID, &DAI, &XST, &Vec::new(), &FilterMode::Disabled,),
            swap_weight_without_path
                .saturating_add(multicollateral_weight)
                .saturating_add(Weight::zero()) // `MockSource`s are not counted
        );

        // DOT -1-> XOR (DEX ID 1)
        // 1) Multicollateral + MockPool(1-3)
        let swap_weight_without_path = swap_base_weight
            .saturating_add(exchange_base_weight)
            .saturating_add(quote_single_weight.saturating_mul(1)); // for each available path
        assert_eq!(
            LiquidityProxy::swap_weight(&DEX_A_ID, &DOT, &XOR, &Vec::new(), &FilterMode::Disabled,),
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
            LiquidityProxy::swap_weight(&DEX_A_ID, &DOT, &XOR, &Vec::new(), &FilterMode::Disabled,),
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
            .saturating_add(quote_single_weight.saturating_mul(2)); // for each available path
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &XSTUSD,
                &XOR,
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
        let swap_base_weight = <Runtime as crate::Config>::WeightInfo::check_indivisible_assets();

        let quote_single_weight = <Runtime as crate::Config>::WeightInfo::list_liquidity_sources()
            .saturating_add(<Runtime as crate::Config>::LiquidityRegistry::check_rewards_weight())
            .saturating_add(
                <Runtime as crate::Config>::LiquidityRegistry::step_quote_weight(
                    <Runtime as crate::Config>::GetNumSamples::get(),
                )
                .saturating_mul(4),
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
            .saturating_add(quote_single_weight.saturating_mul(1)); // for each available path
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_D_ID,
                &DAI,
                &XST,
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
                &DAI,
                &XST,
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
                &DAI,
                &XST,
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
                &DAI,
                &XST,
                &Vec::new(),
                &FilterMode::AllowSelected,
            ),
            swap_weight_without_path
        );

        // DOT -1-> XOR (DEX ID 1)
        // 1) Multicollateral + MockPool(1-3)
        let swap_weight_without_path = swap_base_weight
            .saturating_add(exchange_base_weight)
            .saturating_add(quote_single_weight.saturating_mul(1)); // for each available path
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &DOT,
                &XOR,
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
            .saturating_add(quote_single_weight.saturating_mul(2)); // for each available path
        assert_eq!(
            LiquidityProxy::swap_weight(
                &DEX_A_ID,
                &XSTUSD,
                &XOR,
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
            common::AssetType::Regular,
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
            common::AssetType::Regular,
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
fn test_swap_with_desired_output_returns_precise_amount() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let initial_balance = Assets::free_balance(&XOR, &alice()).unwrap();
        let desired_amount_out = balance!(52.789948793749670063);

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            USDT,
            XOR,
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in: balance!(10000.0)
            },
            sources.clone(),
            filter_mode,
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            initial_balance + desired_amount_out
        );
    });
}

#[test]
fn test_swap_for_permissioned_pool_with_desired_output_returns_precise_amount() {
    let mut ext = ExtBuilder::default().with_permissioned_xyk_pool().build();
    ext.execute_with(|| {
        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let initial_balance = Assets::free_balance(&XOR, &alice()).unwrap();
        let desired_amount_out = balance!(52.789948793749670063);

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            USDT,
            XOR,
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in: balance!(10000.0)
            },
            sources.clone(),
            filter_mode,
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            initial_balance + desired_amount_out
        );
    });
}

#[test]
fn test_swap_with_multi_steps_desired_output_return_precise_amount() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let initial_balance = Assets::free_balance(&KSM, &alice()).unwrap();
        let desired_amount_out = balance!(100.0);

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            USDT,
            KSM,
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in: balance!(10000.0)
            },
            sources.clone(),
            filter_mode,
        ));

        assert_eq!(
            Assets::free_balance(&KSM, &alice()).unwrap(),
            initial_balance + desired_amount_out
        );
    });
}

#[test]
fn test_swap_for_permissioned_pool_with_multi_steps_desired_output_return_precise_amount() {
    let mut ext = ExtBuilder::default().with_permissioned_xyk_pool().build();
    ext.execute_with(|| {
        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let initial_balance = Assets::free_balance(&KSM, &alice()).unwrap();
        let desired_amount_out = balance!(100.0);

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            USDT,
            KSM,
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in: balance!(10000.0)
            },
            sources.clone(),
            filter_mode,
        ));

        assert_eq!(
            Assets::free_balance(&KSM, &alice()).unwrap(),
            initial_balance + desired_amount_out
        );
    });
}

#[test]
fn test_swap_with_desired_input_return_precise_amount() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let initial_balance = Assets::free_balance(&USDT, &alice()).unwrap();
        let desired_amount_in = balance!(100.0);

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            USDT,
            XOR,
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out: balance!(0)
            },
            sources.clone(),
            filter_mode,
        ));
        assert_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            initial_balance - desired_amount_in
        );
    });
}

#[test]
fn test_swap_for_permissioned_pool_with_desired_input_return_precise_amount() {
    let mut ext = ExtBuilder::default().with_permissioned_xyk_pool().build();
    ext.execute_with(|| {
        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let initial_balance = Assets::free_balance(&USDT, &alice()).unwrap();
        let desired_amount_in = balance!(100.0);

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            USDT,
            XOR,
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out: balance!(0)
            },
            sources.clone(),
            filter_mode,
        ));
        assert_eq!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            initial_balance - desired_amount_in
        );
    });
}

#[test]
fn test_swap_with_multi_steps_desired_input_return_precise_amount() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let initial_balance = Assets::free_balance(&KSM, &alice()).unwrap();
        let desired_amount_in = balance!(100.0);

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            KSM,
            USDT,
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out: balance!(0)
            },
            sources.clone(),
            filter_mode,
        ));
        assert_eq!(
            Assets::free_balance(&KSM, &alice()).unwrap(),
            initial_balance - desired_amount_in
        );
    });
}

#[test]
fn test_swap_for_permissioned_pool_with_multi_steps_desired_input_return_precise_amount() {
    let mut ext = ExtBuilder::default().with_permissioned_xyk_pool().build();
    ext.execute_with(|| {
        let filter_mode = FilterMode::AllowSelected;
        let sources = [LiquiditySourceType::XYKPool].to_vec();
        let initial_balance = Assets::free_balance(&KSM, &alice()).unwrap();
        let desired_amount_in = balance!(100.0);

        assert_ok!(LiquidityProxy::swap(
            RuntimeOrigin::signed(alice()),
            DEX_A_ID,
            KSM,
            USDT,
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out: balance!(0)
            },
            sources.clone(),
            filter_mode,
        ));
        assert_eq!(
            Assets::free_balance(&KSM, &alice()).unwrap(),
            initial_balance - desired_amount_in
        );
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
        assert_eq!(quotes.fee, OutcomeFee::new());
        assert_eq!(ls_quote.amount, quotes.amount);
        assert_eq!(ls_quote.fee, quotes.fee);
        assert_eq!(
            &dist,
            &[(
                LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool),
                SwapAmount::with_desired_input(balance!(500), balance!(269.607843137254901960)),
            ),]
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
        assert_eq!(quotes.fee, OutcomeFee::new());
        assert_eq!(
            &dist,
            &[(
                LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                SwapAmount::with_desired_output(balance!(20000), balance!(111.111111111111111112)),
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
        assert_eq!(quotes.fee, OutcomeFee::new());
        assert_eq!(
            &dist,
            &[(
                LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                SwapAmount::with_desired_output(balance!(200), balance!(250)),
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
        assert_eq!(quotes.fee, OutcomeFee::new());
        assert_eq!(
            &dist,
            &[(
                LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                SwapAmount::with_desired_output(balance!(1000), balance!(125)),
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
        assert_approx_eq_abs!(quotes.amount, amount_xor_intermediate, balance!(1));
        assert_eq!(quotes.fee, OutcomeFee::new());
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
        assert_approx_eq_abs!(quotes.amount, amount_without_impact.unwrap(), balance!(20));
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
        assert_approx_eq_abs!(quotes.amount, amount_ksm_out, balance!(1));
        assert_eq!(quotes.fee, OutcomeFee::new());
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
        assert_approx_eq_abs!(quotes.amount, amount_without_impact.unwrap(), balance!(20));
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
        assert_approx_eq_abs!(quotes.amount, amount_ksm_out, balance!(1));
        assert_approx_eq_abs!(amount_without_impact.unwrap(), amount_ksm_out, balance!(20));
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
        assert_approx_eq_abs!(quotes.amount, amount_val_in, balance!(1));
        assert_eq!(quotes.fee, OutcomeFee::new());
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
        assert_approx_eq_abs!(
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
        assert_approx_eq_abs!(quotes.amount, amount_xor_intermediate, balance!(1));
        assert_eq!(quotes.fee, OutcomeFee::new());
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
        assert_approx_eq_abs!(
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
        assert_approx_eq_abs!(quotes.amount, amount_val_in, balance!(100));
        assert_approx_eq_abs!(
            amount_without_impact.unwrap(),
            amount_val_in,
            balance!(5000)
        );
        assert!(amount_without_impact.unwrap() < quotes.amount);
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
            None,
        ));

        test_utils::check_adar_commission(&swap_batches, sources);
        test_utils::check_swap_batch_executed_amount(swap_batches);
    });
}

fn test_batch_swap_event(
    event_data: BoundedVec<
        u8,
        <Runtime as crate::Config>::MaxAdditionalDataLengthSwapTransferBatch,
    >,
) {
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
            Some(event_data.clone()),
        ));

        frame_system::Pallet::<Runtime>::assert_last_event(
            crate::Event::BatchSwapExecuted(adar_fee, amount_in, Some(event_data)).into(),
        );
    });
}

#[test]
fn test_batch_swap_emits_event() {
    test_batch_swap_event(BoundedVec::try_from(vec![1, 2, 3, 32, 2, 13, 37]).unwrap());
}

#[test]
fn test_batch_swap_max_additional_data() {
    let max_data_len: u32 =
        <Runtime as crate::Config>::MaxAdditionalDataLengthSwapTransferBatch::get();
    let max_additional_data =
        BoundedVec::try_from(vec![255; max_data_len.try_into().unwrap()]).unwrap();
    test_batch_swap_event(max_additional_data)
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
            None,
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
                None,
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
                None,
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
        >::mint_buy_back_and_burn(&USDT, &KSM, balance!(1)).unwrap(), balance!(1.972151292233593092));

        assert_eq!(Assets::total_issuance(&USDT).unwrap(), balance!(24001));
        assert_eq!(
            Assets::total_issuance(&KSM).unwrap(),
            balance!(3998.027848707766406908)
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
            balance!(1.972151292233593092)
        );

        assert_eq!(Assets::total_issuance(&USDT).unwrap(), balance!(24000));
        assert_eq!(
            Assets::total_issuance(&KSM).unwrap(),
            balance!(3998.027848707766406908)
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
        assert_approx_eq_abs!(
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
            None,
        ));

        test_utils::check_adar_commission(&swap_batches, sources);
        test_utils::check_swap_batch_executed_amount(swap_batches);
        frame_system::Pallet::<Runtime>::assert_has_event(
            crate::Event::<Runtime>::ADARFeeWithdrawn(KSM, balance!(0.025)).into(),
        );
        frame_system::Pallet::<Runtime>::assert_has_event(
            crate::Event::<Runtime>::ADARFeeWithdrawn(USDT, balance!(0.025)).into(),
        );
        assert_approx_eq_abs!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            balance!(356394.919168903),
            balance!(0.00001)
        );
        assert_approx_eq_abs!(
            Assets::free_balance(&KSM, &alice()).unwrap(),
            balance!(1990),
            balance!(0.00001)
        );
        assert_approx_eq_abs!(
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
                None,
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

        assert_approx_eq_abs!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            // 12000 USDT - 1 USDT for swap - 1 USDT for transfer
            balance!(11998),
            balance!(0.01)
        );
        assert_approx_eq_abs!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            balance!(356401),
            balance!(0.01)
        );
        assert_approx_eq_abs!(
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

        assert_approx_eq_abs!(
            Assets::free_balance(&USDT, &alice()).unwrap(),
            // 12000 USDT - 1 USDT for swap - 1 USDT for transfer
            balance!(11999),
            balance!(0.01)
        );
        assert_approx_eq_abs!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            balance!(356400),
            balance!(0.01)
        );
        assert_approx_eq_abs!(
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

fn test_path_build(
    dex_info: &DEXInfo<AssetId>,
    input_asset_id: AssetId,
    output_asset_id: AssetId,
    expected_paths: Vec<Vec<AssetId>>,
) {
    let paths =
        crate::ExchangePath::<Runtime>::new_trivial(dex_info, input_asset_id, output_asset_id);
    let Some(paths) = paths else {
        assert!(expected_paths.is_empty());
        return;
    };
    let paths = paths.into_iter().map(|x| x.0).collect::<Vec<_>>();
    let expected_paths = expected_paths
        .into_iter()
        .map(|mut x| {
            x.insert(0, input_asset_id);
            x.push(output_asset_id);
            x
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paths, expected_paths,
        "{} -> {}",
        input_asset_id, output_asset_id
    );
}

#[test]
fn test_all_possible_asset_paths() {
    let mut ext = ExtBuilder::default().with_xyk_pool().build();
    ext.execute_with(|| {
        let dex_info = DEXInfo {
            base_asset_id: common::XOR,
            synthetic_base_asset_id: common::XST,
            is_public: true,
        };
        let cases = vec![
            (XOR, XOR, vec![]),
            (XOR, DAI, vec![vec![]]),
            (XOR, XST, vec![vec![]]),
            (XOR, XSTUSD, vec![vec![], vec![XST]]),
            (XOR, KXOR, vec![vec![]]),
            (XOR, ETH, vec![vec![], vec![KXOR]]),
            (DAI, XOR, vec![vec![]]),
            (DAI, DAI, vec![]),
            (DAI, XST, vec![vec![XOR]]),
            (DAI, XSTUSD, vec![vec![XOR], vec![XOR, XST]]),
            (DAI, KXOR, vec![vec![XOR]]),
            (DAI, ETH, vec![vec![XOR], vec![XOR, KXOR]]),
            (XST, XOR, vec![vec![]]),
            (XST, DAI, vec![vec![XOR]]),
            (XST, XST, vec![]),
            (XST, XSTUSD, vec![vec![], vec![XOR]]),
            (XST, KXOR, vec![vec![XOR]]),
            (XST, ETH, vec![vec![XOR], vec![XOR, KXOR]]),
            (XSTUSD, XOR, vec![vec![], vec![XST]]),
            (XSTUSD, DAI, vec![vec![XOR], vec![XST, XOR]]),
            (XSTUSD, XST, vec![vec![], vec![XOR]]),
            (XSTUSD, XSTUSD, vec![]),
            (XSTUSD, KXOR, vec![vec![XOR], vec![XST, XOR]]),
            (
                XSTUSD,
                ETH,
                vec![
                    vec![XOR],
                    vec![XST, XOR],
                    vec![XOR, KXOR],
                    vec![XST, XOR, KXOR],
                ],
            ),
            (KXOR, XOR, vec![vec![]]),
            (KXOR, DAI, vec![vec![XOR]]),
            (KXOR, XST, vec![vec![XOR]]),
            (KXOR, XSTUSD, vec![vec![XOR], vec![XOR, XST]]),
            (KXOR, KXOR, vec![]),
            (KXOR, ETH, vec![vec![], vec![XOR]]),
            (ETH, XOR, vec![vec![], vec![KXOR]]),
            (ETH, DAI, vec![vec![XOR], vec![KXOR, XOR]]),
            (ETH, XST, vec![vec![XOR], vec![KXOR, XOR]]),
            (
                ETH,
                XSTUSD,
                vec![
                    vec![XOR],
                    vec![XOR, XST],
                    vec![KXOR, XOR],
                    vec![KXOR, XOR, XST],
                ],
            ),
            (ETH, KXOR, vec![vec![], vec![XOR]]),
            (ETH, ETH, vec![]),
        ];
        for (input, output, expected_paths) in cases {
            test_path_build(&dex_info, input, output, expected_paths);
        }
    });
}

#[test]
fn test_select_best_path() {
    let mut ext = ExtBuilder::default()
        .with_xyk_pool()
        .with_xyk_pool_xstusd()
        .build();
    ext.execute_with(|| {
        let dex_info = DexManager::dex_id(DEX_D_ID).unwrap();
        let asset_paths = ExchangePath::<Runtime>::new_trivial(&dex_info, XSTUSD, XST).unwrap();
        let reversed_paths = asset_paths.iter().cloned().rev().collect();
        let result = LiquidityProxy::select_best_path(
            &dex_info,
            asset_paths,
            common::prelude::SwapVariant::WithDesiredInput,
            balance!(1),
            &mcbc_excluding_filter(DEX_D_ID),
            false,
            true,
        )
        .unwrap();
        let reversed_result = LiquidityProxy::select_best_path(
            &dex_info,
            reversed_paths,
            common::prelude::SwapVariant::WithDesiredInput,
            balance!(1),
            &mcbc_excluding_filter(DEX_D_ID),
            false,
            true,
        )
        .unwrap();
        assert_eq!(result, reversed_result);
    });
}
