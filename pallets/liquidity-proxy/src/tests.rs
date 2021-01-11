use common::prelude::fixnum::ops::{CheckedSub, Numeric};
use common::prelude::{FixedWrapper, SwapAmount};
use common::{
    fixed, FilterMode, Fixed, LiquiditySource, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType, DOT, KSM,
};
use frame_support::assert_noop;
use sp_runtime::DispatchError;

use crate::{mock::*, Error};

#[test]
#[ignore]
fn test_quote_exact_input_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount: Fixed = fixed!(500);
        let quotes = LiquidityProxy::quote_with_filter(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, fixed!(0)),
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
            &DEX_C_ID,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, fixed!(0)).into(),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let mut dist = quotes.distribution;

        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, fixed!(537.643138033120596204));
        assert_eq!(ls_quote.amount, quotes.amount.into());
        assert_eq!(ls_quote.fee, quotes.fee.into());
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
#[ignore]
fn test_quote_exact_input_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    let amount: Fixed = fixed!(500);
    ext.execute_with(|| {
        let quotes = LiquidityProxy::quote_with_filter(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(amount, fixed!(0)),
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
            &DEX_C_ID,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(amount, fixed!(0)).into(),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, fixed!(363.569067258883248761));
        assert_eq!(ls_quote.amount, quotes.amount.into());
        assert_eq!(ls_quote.fee, quotes.fee.into());
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
#[ignore]
fn test_quote_exact_output_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount: Fixed = fixed!(250);
        let quotes = LiquidityProxy::quote_with_filter(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(amount, fixed!(10000)),
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
            &DEX_C_ID,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(amount, fixed!(10000)).into(),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        let tolerance = fixed!(0.0000000001);
        let approx_expected_base_amount = fixed!(205.339009250744456360);
        assert!(
            (quotes.amount.csub(approx_expected_base_amount).unwrap() < tolerance)
                && (approx_expected_base_amount.csub(quotes.amount).unwrap() < tolerance)
        );
        assert_eq!(ls_quote.amount, quotes.amount.into());
        assert_eq!(ls_quote.fee, quotes.fee.into());
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
#[ignore]
fn test_quote_exact_output_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let amount: Fixed = fixed!(250);
        let quotes = LiquidityProxy::quote_with_filter(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(amount, fixed!(10000)),
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let ls_quote = LiquidityProxy::quote(
            &DEX_C_ID,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(amount, fixed!(10000)).into(),
        )
        .expect("Failed to get a quote via LiquiditySource trait");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        let tolerance = fixed!(0.0000000001);
        let approx_expected_target_amount = fixed!(322.399717709871);
        assert!(
            (quotes.amount.csub(approx_expected_target_amount).unwrap() < tolerance)
                && (approx_expected_target_amount.csub(quotes.amount).unwrap() < tolerance)
        );
        assert_eq!(ls_quote.amount, quotes.amount.into());
        assert_eq!(ls_quote.fee, quotes.fee.into());
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
#[ignore]
fn test_sell_token_for_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let filter = LiquiditySourceFilter::empty(DEX_C_ID);
        let result = LiquidityProxy::perform_swap(
            &alice,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(fixed!(500), fixed!(345)),
            filter,
        )
        .expect("Failed to swap assets");
        assert_eq!(result.amount, fixed!(363.569067258883248731));
    });
}

#[test]
#[ignore]
fn test_sell_base_for_token_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let filter = LiquiditySourceFilter::empty(DEX_C_ID);
        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(500), fixed!(510)),
            filter,
        )
        .expect("Failed to swap assets");
        assert_eq!(result.amount, fixed!(537.643138033120596096));
    });
}

#[test]
#[ignore]
fn test_sell_token_for_base_with_liquidity_source_trait_should_pass() {
    let mut ext = ExtBuilder::default().build();
    let amount: Fixed = fixed!(500);
    ext.execute_with(|| {
        let alice = alice();
        let result = LiquidityProxy::exchange(
            &alice,
            &alice,
            &DEX_C_ID,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(amount, fixed!(345)).into(),
        )
        .expect("Failed to swap assets");
        assert_eq!(result.amount, fixed!(363.569067258883248731));
    });
}

#[test]
#[ignore]
fn test_sell_base_for_token_with_liquidity_source_trait_should_pass() {
    let mut ext = ExtBuilder::default().build();
    let amount: Fixed = fixed!(500);
    ext.execute_with(|| {
        let alice = alice();
        let result = LiquidityProxy::exchange(
            &alice,
            &alice,
            &DEX_C_ID,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(amount, fixed!(510)).into(),
        )
        .expect("Failed to swap assets");
        assert_eq!(result.amount, fixed!(537.643138033120596096));
    });
}

#[test]
#[ignore]
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
        let result = LiquidityProxy::perform_swap(
            &alice,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(fixed!(200), fixed!(298)),
            filter,
        )
        .expect("Failed to swap assets");
        let tolerance = fixed!(0.0000000001);
        let approx_expected_target_amount = fixed!(284.281354954553);
        assert!(result.amount.csub(approx_expected_target_amount).unwrap() < tolerance);
        assert!(approx_expected_target_amount.csub(result.amount).unwrap() < tolerance);
    });
}

#[test]
#[ignore]
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
        let result = LiquidityProxy::perform_swap(
            &alice,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(fixed!(200), fixed!(291)),
            filter,
        )
        .expect("Failed to swap assets");
        let tolerance = fixed!(0.0000000001);
        let approx_expected_target_amount: Fixed = fixed!(277.348779693090);
        assert!(result.amount.csub(approx_expected_target_amount).unwrap() < tolerance);
        assert!(approx_expected_target_amount.csub(result.amount).unwrap() < tolerance);
    });
}

#[test]
fn test_quote_should_fail_with_unavailable_exchange_path() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::quote_with_filter(
            &GetBaseAssetId::get(),
            &KSM,
            SwapAmount::with_desired_output(fixed!(300), Fixed::MAX),
            LiquiditySourceFilter::empty(DEX_C_ID),
        );
        assert_noop!(result, <Error<Runtime>>::UnavailableExchangePath);
    });
}

#[test]
fn test_quote_should_fail_with_unavailable_exchange_path_2() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::quote_with_filter(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(fixed!(300), Fixed::MAX),
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
#[ignore]
fn test_quote_should_fail_with_aggregation_error() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::quote_with_filter(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(fixed!(5000), Fixed::MAX),
            LiquiditySourceFilter::empty(DEX_C_ID),
        );
        assert_noop!(result, <Error<Runtime>>::AggregationError);
    });
}

#[test]
#[ignore]
fn test_sell_however_big_amount_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(2000), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > fixed!(0) && result.amount < fixed!(180));

        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(4000), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > fixed!(0) && result.amount < fixed!(180));

        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(10000), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > fixed!(0) && result.amount < fixed!(180));

        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(100000), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > fixed!(0) && result.amount < fixed!(180));

        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(1000000), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > fixed!(0) && result.amount < fixed!(180));
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
            SwapAmount::with_desired_input(fixed!(500), fixed!(300)),
            Vec::new(),
            FilterMode::Disabled,
        );
        assert_noop!(result, DispatchError::BadOrigin);
    });
}

#[test]
fn test_can_exchange_via_liquidity_proxy_should_pass() {
    let mut ext = ExtBuilder {
        source_types: vec![LiquiditySourceType::MockPool],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| assert!(LiquidityProxy::can_exchange(&DEX_A_ID, &KSM, &DOT)));
}

#[test]
fn test_can_exchange_with_uninitialized_source_should_pass() {
    let mut ext = ExtBuilder {
        source_types: vec![LiquiditySourceType::XYKPool],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| assert!(!LiquidityProxy::can_exchange(&DEX_A_ID, &KSM, &DOT)));
}

#[test]
fn test_can_exchange_with_no_sources_should_pass() {
    let mut ext = ExtBuilder {
        source_types: vec![],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| assert!(!LiquidityProxy::can_exchange(&DEX_A_ID, &KSM, &DOT)));
}
