use crate::{mock::*, Error};
use common::{
    balance, fixed,
    prelude::{fixnum::ops::CheckedSub, Balance, SwapAmount},
    FilterMode, Fixed, LiquiditySourceFilter, LiquiditySourceId, LiquiditySourceType, DOT, KSM,
};
use core::convert::TryInto;
use frame_support::assert_noop;
use sp_runtime::DispatchError;

#[test]
fn test_quote_exact_input_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount: Balance = balance!(500);
        let quotes = LiquidityProxy::quote_single(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, 0),
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, 0),
            LiquiditySourceFilter::empty(DEX_C_ID),
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
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(amount, 0),
            LiquiditySourceFilter::empty(DEX_C_ID),
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
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(amount, balance!(10000)),
            LiquiditySourceFilter::empty(DEX_C_ID),
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
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(amount, balance!(10000)),
            LiquiditySourceFilter::empty(DEX_C_ID),
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
        let filter = LiquiditySourceFilter::empty(DEX_C_ID);
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
        let filter = LiquiditySourceFilter::empty(DEX_C_ID);
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
            LiquiditySourceFilter::empty(DEX_C_ID),
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
            LiquiditySourceFilter::empty(DEX_C_ID),
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
            LiquiditySourceFilter::empty(DEX_C_ID),
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
            LiquiditySourceFilter::empty(DEX_C_ID),
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
