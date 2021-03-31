use crate::mock::*;
use crate::Error;
use common::prelude::fixnum::ops::CheckedSub;
use common::prelude::{Balance, SwapAmount};
use common::{
    balance, fixed, FilterMode, Fixed, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType, DOT, KSM, VAL,
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
        let quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, 0),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
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
        let quotes = LiquidityProxy::quote_single(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(amount, 0),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(amount, 0),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

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
        let quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(amount, balance!(10000)),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
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
        let quotes = LiquidityProxy::quote_single(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(amount, balance!(10000)),
            mcbc_excluding_filter(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
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
        let quotes = LiquidityProxy::quote(
            &KSM,
            &DOT,
            SwapAmount::with_desired_input(balance!(100), 0),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
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
        let quotes = LiquidityProxy::quote(
            &KSM,
            &DOT,
            SwapAmount::with_desired_output(balance!(934.572151021276260545), balance!(501)),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
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
        let quotes = LiquidityProxy::quote(
            &DOT,
            &KSM,
            SwapAmount::with_desired_input(balance!(500), 0),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
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
        let quotes = LiquidityProxy::quote(
            &DOT,
            &KSM,
            SwapAmount::with_desired_output(balance!(555.083861089846196673), balance!(501)),
            LiquiditySourceFilter::empty(DEX_A_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
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
        let quotes = LiquidityProxy::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(amount, balance!(10000)),
            filter,
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
        let amount: Balance = balance!(500);
        let quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, 0),
            LiquiditySourceFilter::with_allowed(DEX_C_ID, [LiquiditySourceType::MockPool].into()),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
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
        let mut quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            SwapAmount::with_desired_input(balance!(100), 0),
            filter.clone(),
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(17530.059712310552788491));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.67765313719130581),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.32234686280869419),
                ),
            ]
        );

        // Buying KSM for XOR
        quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &KSM,
            SwapAmount::with_desired_input(balance!(200), 0),
            filter.clone(),
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
        quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(balance!(200), 0),
            filter.clone(),
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(1562.994117765899819763));
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
fn test_quote_fast_split_exact_ouput_target_should_pass() {
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
        let mut quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            SwapAmount::with_desired_output(balance!(20000), balance!(1000)),
            filter.clone(),
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(117.197946263858078312));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.687719404227631117),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.312280595772368883),
                ),
            ]
        );

        // Buying KSM for XOR
        quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &KSM,
            SwapAmount::with_desired_output(balance!(200), balance!(1000)),
            filter.clone(),
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
        quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(balance!(1000), balance!(1000)),
            filter.clone(),
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(124.256775151618382704));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.220302501954229723),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.779697498045770277),
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
        let mut quotes = LiquidityProxy::quote_single(
            &VAL,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(100), Balance::MAX),
            filter.clone(),
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

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
        quotes = LiquidityProxy::quote_single(
            &KSM,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(200), Balance::MAX),
            filter.clone(),
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
        quotes = LiquidityProxy::quote_single(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(balance!(100), Balance::MAX),
            filter.clone(),
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
        let mut quotes = LiquidityProxy::quote_single(
            &VAL,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(balance!(20000), 0),
            filter.clone(),
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

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
        quotes = LiquidityProxy::quote_single(
            &KSM,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(balance!(200), 0),
            filter.clone(),
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
        quotes = LiquidityProxy::quote_single(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(balance!(500), 0),
            filter.clone(),
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
fn test_quote_fast_split_exact_ouput_target_undercollateralized_should_pass() {
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
        let mut quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &VAL,
            SwapAmount::with_desired_output(balance!(20000), balance!(1000)),
            filter.clone(),
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(323.750240809708188590));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.225),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.775),
                ),
            ]
        );

        // Buying KSM for XOR
        quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &KSM,
            SwapAmount::with_desired_output(balance!(200), balance!(1000)),
            filter.clone(),
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(1202.422808773859499438));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.45),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.55),
                ),
            ]
        );

        // Buying DOT for XOR
        quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(balance!(1000), balance!(1000)),
            filter.clone(),
        )
        .expect("Failed to get a quote");

        dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, balance!(339.983899478813217049));
        assert_eq!(quotes.fee, balance!(0));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(
                        DEX_D_ID,
                        LiquiditySourceType::MulticollateralBondingCurvePool
                    ),
                    fixed!(0.18),
                ),
                (
                    LiquiditySourceId::new(DEX_D_ID, LiquiditySourceType::MockPool),
                    fixed!(0.82),
                ),
            ]
        );
    });
}
