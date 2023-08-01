#[cfg(not(test))]
use crate::{
    traits::DataLayer, Config, Event, LimitOrder, MarketRole, MomentOf, OrderAmount, OrderBook,
    OrderBookId, OrderBookStatus, OrderBooks, OrderVolume, Pallet,
};
use crate::{CacheDataLayer, ExpirationScheduler};
use assets::AssetIdOf;
use codec::Decode;
use common::prelude::{FixedWrapper, QuoteAmount, SwapAmount};
use common::{balance, AssetInfoProvider, Balance, DEXId, LiquiditySource, PriceVariant, VAL, XOR};
use frame_benchmarking::benchmarks;
use frame_support::traits::{Get, Time};
use frame_support::weights::WeightMeter;
use frame_support::{assert_err, assert_ok};
use frame_system::{EventRecord, RawOrigin};
#[cfg(test)]
use framenode_runtime::order_book::{
    test_utils::generate_account, traits::DataLayer, Config, Event, LimitOrder, MarketRole,
    MomentOf, OrderAmount, OrderBook, OrderBookId, OrderBookStatus, OrderBooks, OrderVolume,
    Pallet,
};
use hex_literal::hex;
use sp_runtime::traits::{SaturatedConversion, Saturating, UniqueSaturatedInto};

use crate::benchmarking::bob;
use assets::Pallet as Assets;
use Pallet as OrderBookPallet;

// Creates and populates the order book with the following orders:
// price | volume | orders
//          Asks
//  11.5 |  255.8 | sell4, sell5, sell6
//  11.2 |  178.6 | sell2, sell3
//  11.0 |  176.3 | sell1
//  spread
//  10.0 |  168.5 | buy1
//   9.8 |  139.9 | buy2, buy3
//   9.5 |  261.3 | buy4, buy5, buy6
//          Bids
pub fn create_and_populate_order_book<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
) {
    OrderBookPallet::<T>::create_orderbook(RawOrigin::Signed(bob::<T>()).into(), order_book_id)
        .unwrap();

    Assets::<T>::update_balance(
        RawOrigin::Root.into(),
        bob::<T>(),
        order_book_id.quote,
        balance!(1000000).try_into().unwrap(),
    )
    .unwrap();

    Assets::<T>::update_balance(
        RawOrigin::Root.into(),
        bob::<T>(),
        order_book_id.base,
        balance!(1000000).try_into().unwrap(),
    )
    .unwrap();

    let lifespan: Option<MomentOf<T>> = Some(10000u32.into());

    // prices
    let bp1 = balance!(10);
    let bp2 = balance!(9.8);
    let bp3 = balance!(9.5);
    let sp1 = balance!(11);
    let sp2 = balance!(11.2);
    let sp3 = balance!(11.5);

    // amounts
    let amount1 = balance!(168.5);
    let amount2 = balance!(95.2);
    let amount3 = balance!(44.7);
    let amount4 = balance!(56.4);
    let amount5 = balance!(89.9);
    let amount6 = balance!(115);
    let amount7 = balance!(176.3);
    let amount8 = balance!(85.4);
    let amount9 = balance!(93.2);
    let amount10 = balance!(36.6);
    let amount11 = balance!(205.5);
    let amount12 = balance!(13.7);

    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp1,
        amount1,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp2,
        amount2,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp2,
        amount3,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp3,
        amount4,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp3,
        amount5,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp3,
        amount6,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();

    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp1,
        amount7,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp2,
        amount8,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp2,
        amount9,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp3,
        amount10,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp3,
        amount11,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp3,
        amount12,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
}

fn fill_order_book_side<T: Config>(
    data: &mut impl DataLayer<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    amount: OrderVolume,
    now: <<T as Config>::Time as Time>::Moment,
    prices: &mut impl Iterator<Item = Balance>,
    max_side_price_count: u32,
    max_orders_per_price: u32,
    users: &mut impl Iterator<Item = T::AccountId>,
    current_user: &mut T::AccountId,
    user_orders: &mut u64,
    max_orders_per_user: u128,
    lifespans: &mut impl Iterator<Item = u64>,
    current_lifespan: &mut u64,
    lifespan_orders: &mut u64,
    max_expiring_orders_per_block: u128,
) {
    use std::io::Write;

    let current_block = frame_system::Pallet::<T>::block_number();
    let mut i = 0;
    for price in prices {
        print!(
            "\r{}/{} ({}%)",
            i,
            max_side_price_count,
            100.0 * (i as f32) / (max_side_price_count as f32)
        );
        std::io::stdout().flush().unwrap();
        i += 1;
        for _ in 0..max_orders_per_price {
            let buy_order = LimitOrder::<T>::new(
                order_book.next_order_id(),
                current_user.clone(),
                side,
                price,
                amount,
                now.clone(),
                (*current_lifespan).saturated_into(),
                current_block,
            );
            data.insert_limit_order(&order_book.order_book_id, buy_order)
                .unwrap();
            // order_book.place_limit_order(buy_order, data).unwrap();
            // todo: make an object for this
            *user_orders += 1;
            if *user_orders as u128 >= max_orders_per_user {
                *current_user = users.next().expect("infinite iterator");
                *user_orders = 0
            }
            *lifespan_orders += 1;
            if *lifespan_orders as u128 >= max_expiring_orders_per_block {
                *current_lifespan = lifespans.next().expect("infinite iterator");
                *lifespan_orders = 0
            }
        }
    }
}

pub fn fill_order_book_worst_case<T: Config + assets::Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    data: &mut impl DataLayer<T>,
) {
    use std::io::Write;

    let max_side_price_count = T::MaxSidePriceCount::get();
    let max_orders_per_price = T::MaxLimitOrdersForPrice::get();
    let max_orders_per_user = T::MaxOpenedLimitOrdersPerUser::get() as u128;
    let max_expiring_orders_per_block = T::MaxExpiringOrdersPerBlock::get();

    let mut order_book = <OrderBooks<T>>::get(order_book_id).unwrap();
    let amount = std::cmp::max(order_book.step_lot_size, order_book.min_lot_size);
    let now = T::Time::now();
    // to allow mutating with `order_book.next_order_id()` later
    let tick_size = order_book.tick_size;
    let amount_per_user: OrderVolume = max_orders_per_user * amount;

    let mut bid_prices = (1..=max_side_price_count).map(|i| (i as u128) * tick_size);
    let mut ask_prices = (max_side_price_count + 1..=2 * max_side_price_count)
        .rev()
        .map(|i| (i as u128) * tick_size);
    let max_price = (2 * max_side_price_count) as u128 * tick_size;

    // user generator with minted add quote & base assets
    let mut users = (1..)
        .map(crate::test_utils::generate_account::<T>)
        .inspect(|user| {
            assets::Pallet::<T>::mint_unchecked(&order_book_id.base, &user, amount_per_user)
                .unwrap();
            assets::Pallet::<T>::mint_unchecked(
                &order_book_id.quote,
                &user,
                (FixedWrapper::from(max_price) * FixedWrapper::from(amount_per_user))
                    .try_into_balance()
                    .unwrap(),
            )
            .unwrap();
        });
    let mut current_user = users.next().expect("infinite iterator");

    // # of orders placed by `current_user`
    let mut user_orders = 0;
    let mut lifespans = (1..).map(|i| {
        i * T::MILLISECS_PER_BLOCK.saturated_into::<u64>()
            + T::MIN_ORDER_LIFETIME.saturated_into::<u64>()
    });
    let mut current_lifespan = lifespans.next().expect("infinite iterator");
    let mut block_orders = 0;
    let mut i = 0;
    let mut start_time = std::time::SystemTime::now();
    println!(
        "Starting placement of bid orders, {} orders per price",
        max_orders_per_price
    );
    fill_order_book_side(
        data,
        &mut order_book,
        PriceVariant::Buy,
        amount,
        now,
        &mut bid_prices,
        max_side_price_count,
        max_orders_per_price,
        &mut users,
        &mut current_user,
        &mut user_orders,
        max_orders_per_user,
        &mut lifespans,
        &mut current_lifespan,
        &mut block_orders,
        max_expiring_orders_per_block as u128,
    );
    println!(
        "\nprocessed all bid prices in {:?}",
        start_time.elapsed().unwrap()
    );

    let mut start_time = std::time::SystemTime::now();
    println!(
        "Starting placement of ask orders, {} orders per price",
        max_orders_per_price
    );
    fill_order_book_side(
        data,
        &mut order_book,
        PriceVariant::Sell,
        amount,
        now,
        &mut ask_prices,
        max_side_price_count,
        max_orders_per_price,
        &mut users,
        &mut current_user,
        &mut user_orders,
        max_orders_per_user,
        &mut lifespans,
        &mut current_lifespan,
        &mut block_orders,
        max_expiring_orders_per_block as u128,
    );
    println!(
        "\nprocessed all ask prices in {:?}",
        start_time.elapsed().unwrap()
    );
}
