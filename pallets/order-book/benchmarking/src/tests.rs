use super::*;

use common::balance;
use common::prelude::SwapAmount;
use frame_system::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::Runtime;
#[allow(unused)]
use preparation::presets::*;

#[test]
fn test_benchmark_delete_orderbook() {
    ext().execute_with(|| {
        let settings = preset_14::<Runtime>();
        let order_book_id = lifecycle::delete_orderbook_benchmark::init(settings.clone());

        OrderBookPallet::<Runtime>::delete_orderbook(RawOrigin::Root.into(), order_book_id)
            .unwrap();

        lifecycle::delete_orderbook_benchmark::verify(settings, order_book_id);
    })
}

#[test]
fn test_benchmark_place() {
    ext().execute_with(|| {
        let settings = preset_14::<Runtime>();
        let context = lifecycle::place_limit_order_benchmark::init(settings.clone());

        OrderBookPallet::<Runtime>::place_limit_order(
            RawOrigin::Signed(context.caller.clone()).into(),
            context.order_book_id,
            *context.price.balance(),
            *context.amount.balance(),
            context.side,
            Some(context.lifespan),
        )
        .unwrap();

        lifecycle::place_limit_order_benchmark::verify(settings, context);
    })
}

#[test]
fn test_benchmark_cancel() {
    ext().execute_with(|| {
        let settings = preset_14::<Runtime>();
        let context = lifecycle::cancel_limit_order_benchmark::init(settings.clone(), false);

        OrderBookPallet::<Runtime>::cancel_limit_order(
            RawOrigin::Signed(context.caller.clone()).into(),
            context.order_book_id.clone(),
            context.order_id.clone(),
        )
        .unwrap();

        lifecycle::cancel_limit_order_benchmark::verify(settings, context);
    })
}

#[test]
fn test_benchmark_execute_market_order() {
    ext().execute_with(|| {
        let settings = preset_14::<Runtime>();
        let context = lifecycle::execute_market_order_benchmark::init(settings.clone());

        OrderBookPallet::<Runtime>::execute_market_order(
            RawOrigin::Signed(context.caller.clone()).into(),
            context.order_book_id,
            context.side,
            *context.amount.balance(),
        )
        .unwrap();

        lifecycle::execute_market_order_benchmark::verify(settings, context);
    })
}

#[test]
fn test_benchmark_quote() {
    ext().execute_with(|| {
        use common::LiquiditySource;

        let settings = preset_14::<Runtime>();
        let context = lifecycle::quote_benchmark::init(settings.clone());

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
fn test_benchmark_exchange() {
    ext().execute_with(|| {
        use common::LiquiditySource;

        let settings = preset_14::<Runtime>();
        let context = lifecycle::exchange_single_order_benchmark::init(settings.clone());

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

        lifecycle::exchange_single_order_benchmark::verify(settings, context);
    })
}
