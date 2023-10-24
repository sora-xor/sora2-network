use super::*;

use common::balance;
use common::prelude::SwapAmount;
use frame_system::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::Runtime;
use order_book::test_utils::fill_tools::FillSettings;
#[allow(unused)]
use preparation::presets::*;

#[test]
fn test_benchmark_delete_orderbook() {
    ext().execute_with(|| {
        let settings = FillSettings::<Runtime>::max();
        let order_book_id = periphery::delete_orderbook::init(settings.clone());

        OrderBookPallet::<Runtime>::delete_orderbook(RawOrigin::Root.into(), order_book_id)
            .unwrap();

        periphery::delete_orderbook::verify(settings, order_book_id);
    })
}

#[test]
fn test_benchmark_place() {
    ext().execute_with(|| {
        let settings = FillSettings::<Runtime>::max();
        let context = periphery::place_limit_order::init(settings.clone());

        OrderBookPallet::<Runtime>::place_limit_order(
            RawOrigin::Signed(context.caller.clone()).into(),
            context.order_book_id,
            *context.price.balance(),
            *context.amount.balance(),
            context.side,
            Some(context.lifespan),
        )
        .unwrap();

        periphery::place_limit_order::verify(settings, context);
    })
}

#[test]
fn test_benchmark_cancel() {
    ext().execute_with(|| {
        let settings = FillSettings::<Runtime>::max();
        let context = periphery::cancel_limit_order::init(settings.clone(), false);

        OrderBookPallet::<Runtime>::cancel_limit_order(
            RawOrigin::Signed(context.caller.clone()).into(),
            context.order_book_id.clone(),
            context.order_id.clone(),
        )
        .unwrap();

        periphery::cancel_limit_order::verify(settings, context);
    })
}

#[test]
fn test_benchmark_execute_market_order() {
    ext().execute_with(|| {
        let settings = FillSettings::<Runtime>::max();
        let context = periphery::execute_market_order::init(settings.clone());

        OrderBookPallet::<Runtime>::execute_market_order(
            RawOrigin::Signed(context.caller.clone()).into(),
            context.order_book_id,
            context.side,
            *context.amount.balance(),
        )
        .unwrap();

        periphery::execute_market_order::verify(settings, context);
    })
}

#[test]
fn test_benchmark_quote() {
    ext().execute_with(|| {
        use common::LiquiditySource;

        let settings = FillSettings::<Runtime>::max();
        let context = periphery::quote::init(settings.clone());

        let _ = OrderBookPallet::<Runtime>::quote(
            &context.dex_id,
            &context.input_asset_id,
            &context.output_asset_id,
            context.amount,
            context.deduce_fee,
        )
        .unwrap();
    })
}

#[test]
fn test_benchmark_exchange_single_order() {
    ext().execute_with(|| {
        use common::LiquiditySource;

        let settings = FillSettings::<Runtime>::max();
        let context = periphery::exchange_single_order::init(settings.clone());

        let (_outcome, _) = OrderBookPallet::<Runtime>::exchange(
            &context.caller,
            &context.caller,
            &context.order_book_id.dex_id,
            &context.order_book_id.base,
            &context.order_book_id.quote,
            SwapAmount::with_desired_output(
                context.expected_out,
                context.expected_in + balance!(5),
            ),
        )
        .unwrap();

        periphery::exchange_single_order::verify(settings, context);
    })
}
