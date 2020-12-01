use crate::{mock::*, Error};
use common::{
    fixed, prelude::SwapAmount, FilterMode, Fixed, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType,
};
use frame_support::assert_noop;
use sp_arithmetic::traits::{Bounded, Saturating};
use sp_runtime::DispatchError;

#[test]
fn test_quote_exact_input_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let quotes = LiquidityProxy::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(500), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;

        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, fixed!(537, 658984414492410269));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::XYKPool),
                    fixed!(0, 0),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool),
                    fixed!(0, 1),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    fixed!(0, 22),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    fixed!(0, 03),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    fixed!(0, 65),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_exact_input_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let quotes = LiquidityProxy::quote(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(fixed!(500), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(quotes.amount, fixed!(363, 647298994628839043));
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::XYKPool),
                    fixed!(0, 0),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool),
                    fixed!(0, 27),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    fixed!(0, 19),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    fixed!(0, 24),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    fixed!(0, 3),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_exact_output_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let quotes = LiquidityProxy::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(fixed!(250), fixed!(10000)),
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        let tolerance = fixed!(1 e-10);
        let expected_base_amount = fixed!(205, 339009250744456360);
        assert!(
            (quotes.amount.saturating_sub(expected_base_amount) < tolerance)
                && (expected_base_amount.saturating_sub(quotes.amount) < tolerance)
        );
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::XYKPool),
                    fixed!(0),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool),
                    fixed!(0),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    fixed!(0, 2),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    fixed!(0),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    fixed!(0, 8),
                ),
            ]
        );
    });
}

#[test]
fn test_quote_exact_output_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let quotes = LiquidityProxy::quote(
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(fixed!(250), fixed!(10000)),
            LiquiditySourceFilter::empty(DEX_C_ID),
        )
        .expect("Failed to get a quote");

        let mut dist = quotes.distribution;
        dist.sort_by(|a, b| a.0.cmp(&b.0));

        let tolerance = fixed!(1 e-10);
        let expected_target_amount = fixed!(322, 379695480740487555);
        assert!(
            (quotes.amount.saturating_sub(expected_target_amount) < tolerance)
                && (expected_target_amount.saturating_sub(quotes.amount) < tolerance)
        );
        assert_eq!(
            &dist,
            &[
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::XYKPool),
                    fixed!(0, 0),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool),
                    fixed!(0, 33),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool2),
                    fixed!(0, 17),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool3),
                    fixed!(0, 32),
                ),
                (
                    LiquiditySourceId::new(DEX_C_ID, LiquiditySourceType::MockPool4),
                    fixed!(0, 18),
                ),
            ]
        );
    });
}

#[test]
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
        assert_eq!(result.amount, fixed!(363, 647298994628839003));
    });
}

#[test]
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
        assert_eq!(result.amount, fixed!(537, 658984414492410156));
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
        let result = LiquidityProxy::perform_swap(
            &alice,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(fixed!(200), fixed!(298)),
            filter,
        )
        .expect("Failed to swap assets");
        let tolerance = fixed!(1 e-10);
        let expected_target_amount = fixed!(284, 236667768229000000);
        assert!(
            (result.amount.saturating_sub(expected_target_amount) < tolerance)
                && (expected_target_amount.saturating_sub(result.amount) < tolerance)
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
        let result = LiquidityProxy::perform_swap(
            &alice,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(fixed!(200), fixed!(291)),
            filter,
        )
        .expect("Failed to swap assets");
        let tolerance = fixed!(1 e-10);
        let expected_target_amount = fixed!(277, 350428580061000000);
        assert!(
            (result.amount.saturating_sub(expected_target_amount) < tolerance)
                && (expected_target_amount.saturating_sub(result.amount) < tolerance)
        );
    });
}

#[test]
fn test_quote_should_fail_with_unavailable_exchange_path() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::quote(
            &GetBaseAssetId::get(),
            &KSM,
            SwapAmount::with_desired_output(fixed!(300), Fixed::max_value()),
            LiquiditySourceFilter::empty(DEX_C_ID),
        );
        assert_noop!(result, <Error<Runtime>>::UnavailableExchangePath);
    });
}

#[test]
fn test_quote_should_fail_with_unavailable_exchange_path_2() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(fixed!(300), Fixed::max_value()),
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
fn test_quote_should_fail_with_unavailable_exchange_path_3() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::quote(
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(fixed!(5_000), Fixed::max_value()),
            LiquiditySourceFilter::empty(DEX_C_ID),
        );
        assert_noop!(result, <Error<Runtime>>::UnavailableExchangePath);
    });
}

#[test]
fn test_sell_however_big_amount_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = alice();
        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(2_000), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > fixed!(0) && result.amount < fixed!(180));

        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(4_000), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > fixed!(0) && result.amount < fixed!(180));

        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(10_000), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > fixed!(0) && result.amount < fixed!(180));

        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(100_000), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > fixed!(0) && result.amount < fixed!(180));

        let result = LiquidityProxy::perform_swap(
            &alice,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(1_000_000), fixed!(0)),
            LiquiditySourceFilter::empty(DEX_B_ID),
        )
        .expect("Failed to swap assets");
        assert!(result.amount > fixed!(0) && result.amount < fixed!(180));
    });
}

#[test]
fn test_swap_should_fail_with_unavailable_exchange_path() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let result = LiquidityProxy::swap(
            Origin::signed(alice()),
            DEX_C_ID,
            DOT,
            GetBaseAssetId::get(),
            SwapAmount::with_desired_output(fixed!(500), fixed!(400)), // expectation too high
            Vec::new(),
            FilterMode::Disabled,
        );
        assert_noop!(result, <Error<Runtime>>::UnavailableExchangePath);
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
