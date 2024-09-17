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

//! General preparation logic for the benchmarking.

// TODO: rename to `order_book` after upgrading to nightly-2023-07-01+
#[cfg(test)]
use framenode_runtime::order_book as order_book_imported;
#[cfg(not(test))]
use order_book as order_book_imported;

// TODO: rename to `order_book_benchmarking` after upgrading to nightly-2023-07-01+
#[cfg(not(test))]
use crate as order_book_benchmarking_imported;
#[cfg(test)]
use framenode_runtime::order_book_benchmarking as order_book_benchmarking_imported;

use common::prelude::{QuoteAmount, Scalar};
use common::{balance, AssetIdOf, AssetManager, Balance, PriceVariant, ETH, VAL, XOR};
use frame_support::traits::Time;
use frame_system::RawOrigin;
use log::debug;
use order_book_imported::test_utils::accounts;
use order_book_imported::test_utils::fill_tools::{
    bid_prices_iterator, fill_expiration_schedule, fill_order_book_side,
    fill_order_book_worst_case, fill_price, fill_user_orders, lifespans_iterator, users_iterator,
    AmountVariant, FillSettings,
};
use order_book_imported::{
    cache_data_layer::CacheDataLayer, traits::DataLayer, DealInfo, LimitOrder, MomentOf, OrderBook,
    OrderBookId, OrderBooks, OrderPrice, OrderVolume,
};
use sp_runtime::traits::{CheckedMul, SaturatedConversion};
use sp_std::iter::Peekable;

use order_book_benchmarking_imported::{assert_orders_numbers, Config, DEX};

use order_book_imported::Pallet as OrderBookPallet;

/// Places buy orders for worst-case execution. Fill up max amount allowed by the settings
///
/// Price of the order to execute is minimal possible in the order book.
///
/// Returns:
/// - iterators in the same way as `fill_order_book_worst_case`
/// - price variant of the order
fn prepare_order_execute_worst_case<T: Config>(
    data: &mut impl DataLayer<T>,
    order_book: &mut OrderBook<T>,
    fill_settings: FillSettings<T>,
) -> (
    Peekable<impl Iterator<Item = T::AccountId>>,
    Peekable<impl Iterator<Item = u64>>,
    PriceVariant,
) {
    let orders_amount = fill_settings.amount_variant.calculate_amount(order_book);
    let orders_side = PriceVariant::Buy;
    let max_price = order_book.tick_size * Scalar(2 * fill_settings.max_side_price_count);

    // Owners for each placed order
    let mut users = users_iterator::<T>(
        order_book.order_book_id,
        orders_amount,
        max_price, // still mint max to reuse the iter later
        fill_settings.max_orders_per_user,
    )
    .peekable();
    // Lifespans for each placed order
    let mut lifespans =
        lifespans_iterator::<T>(fill_settings.max_expiring_orders_per_block, 1).peekable();
    let max_side_orders = fill_settings.max_side_orders();
    let mut bid_prices =
        bid_prices_iterator(order_book.tick_size, fill_settings.max_side_price_count);

    // any of the iterators can limit number of placed orders; `users` is one of them
    let mut limited_users = users.by_ref().take(max_side_orders as usize).peekable(); // would be great to reuse the peekable inside; not sure how

    debug!(
        "Filling a side of the order book for worst-case execution with settings ({:?} orders)",
        fill_settings.max_side_price_count * fill_settings.max_orders_per_price
    );
    fill_order_book_side(
        data,
        fill_settings,
        order_book,
        orders_side,
        orders_amount,
        &mut bid_prices,
        &mut limited_users,
        &mut lifespans,
    );

    assert!(limited_users.next().is_none(), "did not place all orders");
    (users, lifespans, orders_side.switched())
}

/// Prepare benchmark for `place_limit_order` extrinsic. Benchmark only considers placing limit
/// order without conversion to market (even partially).
///
/// Returns parameters for placing a limit order;
/// `author` should not be from `test_utils::generate_account`
pub fn place_limit_order_without_cross_spread<T: Config>(
    fill_settings: FillSettings<T>,
    author: T::AccountId,
) -> (
    OrderBookId<AssetIdOf<T>, T::DEXId>,
    OrderPrice,
    OrderVolume,
    PriceVariant,
    MomentOf<T>,
) {
    let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };
    OrderBookPallet::<T>::create_orderbook(
        RawOrigin::Root.into(),
        order_book_id,
        balance!(0.00001),
        balance!(0.00001),
        balance!(1),
        balance!(1000),
    )
    .expect("failed to create an order book");
    let mut order_book = <OrderBooks<T>>::get(order_book_id).unwrap();
    let mut data_layer = CacheDataLayer::<T>::new();

    let side_to_place = PriceVariant::Sell;
    let min_price = order_book.tick_size;
    let price_to_place = min_price
        .checked_mul_by_scalar(Scalar(T::SOFT_MIN_MAX_RATIO as u128))
        .unwrap();
    let order_amount = sp_std::cmp::max(order_book.step_lot_size, order_book.min_lot_size);

    // Owners for each placed order
    let mut users = users_iterator::<T>(
        order_book.order_book_id,
        order_amount,
        order_book.tick_size,
        fill_settings.max_orders_per_user,
    )
    .peekable();
    // Lifespans for each placed order
    let mut lifespans =
        lifespans_iterator::<T>(fill_settings.max_expiring_orders_per_block, 1).peekable();

    // The price where the order is going to be placed should not be filled
    let mut fill_price_settings = fill_settings.clone();
    fill_price_settings.max_orders_per_price -= 1;
    fill_price(
        &mut data_layer,
        fill_price_settings,
        &mut order_book,
        side_to_place,
        order_amount,
        price_to_place,
        &mut users,
        &mut lifespans,
    );

    let mut fill_user_settings = fill_settings.clone();
    // leave a room for one more order
    fill_user_settings.max_orders_per_user -= 1;
    fill_user_orders(
        &mut data_layer,
        fill_user_settings,
        &mut order_book,
        side_to_place.switched(),
        order_amount,
        author.clone(),
        &mut lifespans,
    );

    // fill expiration schedule for a block:
    // skip to an empty block
    let filled_block = lifespans.next().unwrap();
    let mut lifespans = lifespans.skip_while(|b| *b == filled_block);
    let to_fill = lifespans.next().unwrap();
    // we are going to fill this lifespan, so skipping it for possible future use of the iter
    let _lifespans = lifespans.skip_while(|b| *b == to_fill);
    // different order book because we just want to fill expirations
    let order_book_id_2 = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: ETH.into(),
        quote: XOR.into(),
    };

    T::AssetManager::mint_unchecked(&ETH.into(), &accounts::bob::<T>(), balance!(1000)).unwrap();

    OrderBookPallet::<T>::create_orderbook(
        RawOrigin::Root.into(),
        order_book_id_2,
        balance!(0.00001),
        balance!(0.00001),
        balance!(1),
        balance!(1000),
    )
    .expect("failed to create an order book");
    let mut order_book_2 = <OrderBooks<T>>::get(order_book_id_2).unwrap();
    let order_amount_2 = sp_std::cmp::max(order_book_2.step_lot_size, order_book_2.min_lot_size);
    let mut fill_expiration_settings = fill_settings.clone();
    // leave a room for 1
    fill_expiration_settings.max_expiring_orders_per_block -= 1;
    // mint other base asset as well
    let mut users = users
        .inspect(move |user| {
            T::AssetManager::mint_unchecked(&order_book_id_2.base, user, *order_amount_2.balance())
                .unwrap();
        })
        .peekable();

    fill_expiration_schedule(
        &mut data_layer,
        fill_expiration_settings,
        &mut order_book_2,
        PriceVariant::Sell,
        order_amount_2,
        &mut users,
        to_fill,
    );

    debug!("Committing data...");
    <OrderBooks<T>>::insert(order_book_id, order_book.clone());
    <OrderBooks<T>>::insert(order_book_id_2, order_book_2);
    data_layer.commit();
    debug!("Data committed!");

    let lifespan = to_fill.saturated_into::<MomentOf<T>>();
    T::AssetManager::mint_unchecked(&order_book_id.base, &author, *order_amount.balance()).unwrap();

    let expected_user_orders = sp_std::cmp::min(
        fill_settings.max_orders_per_user - 1,
        fill_settings.max_side_price_count * fill_settings.max_orders_per_price,
    ) as usize;

    assert_orders_numbers::<T>(
        order_book_id,
        Some(expected_user_orders),
        Some((fill_settings.max_orders_per_price - 1) as usize),
        Some((author, expected_user_orders)),
        Some((
            lifespan,
            (fill_settings.max_expiring_orders_per_block - 1) as usize,
        )),
    );

    (
        order_book_id,
        price_to_place,
        order_amount,
        side_to_place,
        lifespan,
    )
}

/// Prepare benchmark for `cancel_limit_order` extrinsic.
///
/// Returns parameters for cancelling a limit order.
/// `expirations_first` switches between two cases; it's not clear which one is heavier.
/// `author` should not be from `test_utils::generate_account`.
pub fn cancel_limit_order<T: Config>(
    fill_settings: FillSettings<T>,
    author: T::AccountId,
    place_first_expiring: bool,
) -> (OrderBookId<AssetIdOf<T>, T::DEXId>, T::OrderId) {
    let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };
    OrderBookPallet::<T>::create_orderbook(
        RawOrigin::Root.into(),
        order_book_id,
        balance!(0.00001),
        balance!(0.00001),
        balance!(1),
        balance!(1000),
    )
    .expect("failed to create an order book");
    let mut order_book = <OrderBooks<T>>::get(order_book_id).unwrap();
    let mut data_layer = CacheDataLayer::<T>::new();

    debug!("Filling aggregated bids");
    let mut buy_settings = fill_settings.clone();
    buy_settings.max_orders_per_price = 1;
    let (mut users, mut lifespans) = fill_order_book_worst_case::<T>(
        buy_settings,
        &mut order_book,
        &mut data_layer,
        true,
        false,
    );

    debug!("Filling the price of the cancelled order");
    let mut fill_price_settings = fill_settings.clone();
    // account for previous fill + room for order to cancel
    fill_price_settings.max_orders_per_price -= 2;
    let target_price = data_layer.best_bid(&order_book_id).unwrap().0;
    let order_amount = sp_std::cmp::max(order_book.step_lot_size, order_book.min_lot_size);
    fill_price(
        &mut data_layer,
        fill_price_settings,
        &mut order_book,
        PriceVariant::Buy,
        order_amount,
        target_price,
        &mut users,
        &mut lifespans,
    );

    debug!("Fill user orders (leave a room for cancelled which will be inserted later)");
    let mut fill_user_settings = fill_settings.clone();
    fill_user_settings.max_orders_per_user -= 1;
    fill_user_orders(
        &mut data_layer,
        fill_user_settings,
        &mut order_book,
        PriceVariant::Sell,
        order_amount,
        author.clone(),
        &mut lifespans,
    );

    // skip to an empty block
    let filled_block = lifespans.next().unwrap();
    let mut lifespans = lifespans.skip_while(|b| *b == filled_block);
    let to_fill = lifespans.next().unwrap();
    // we are going to fill this lifespan, so skipping it for possible future use of the iter
    let mut _lifespans = lifespans.skip_while(|b| *b == to_fill);

    T::AssetManager::mint_unchecked(
        &order_book.order_book_id.quote,
        &author,
        *target_price.checked_mul(&order_amount).unwrap().balance(),
    )
    .unwrap();
    // don't repeat this code for both `place_first_expiring` cases
    let place_to_cancel = |order_book: &mut OrderBook<T>, data_layer: &mut CacheDataLayer<T>| {
        debug!("Inserting order to cancel");
        let id = order_book.next_order_id();
        let order = LimitOrder::<T>::new(
            id,
            author.clone(),
            PriceVariant::Buy,
            target_price,
            order_amount,
            T::Time::now(),
            to_fill.saturated_into(),
            frame_system::Pallet::<T>::block_number(),
        );
        order_book.place_limit_order(order, data_layer).unwrap();
        id
    };

    let mut to_cancel_id = T::OrderId::from(0u8);
    if place_first_expiring {
        to_cancel_id = place_to_cancel(&mut order_book, &mut data_layer);
    }

    // different order book because we just want to fill expirations and don't face restrictions
    // from the first one
    let order_book_id_2 = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: ETH.into(),
        quote: XOR.into(),
    };

    T::AssetManager::mint_unchecked(&ETH.into(), &accounts::bob::<T>(), balance!(1000)).unwrap();

    OrderBookPallet::<T>::create_orderbook(
        RawOrigin::Root.into(),
        order_book_id_2,
        balance!(0.00001),
        balance!(0.00001),
        balance!(1),
        balance!(1000),
    )
    .expect("failed to create an order book");
    let mut order_book_2 = <OrderBooks<T>>::get(order_book_id_2).unwrap();
    let order_amount_2 = sp_std::cmp::max(order_book_2.step_lot_size, order_book_2.min_lot_size);
    let mut fill_expiration_settings = fill_settings;
    // we add one more separately
    fill_expiration_settings.max_expiring_orders_per_block -= 1;
    // mint other base asset as well
    let mut users = users
        .inspect(move |user| {
            T::AssetManager::mint_unchecked(&order_book_id_2.base, user, *order_amount_2.balance())
                .unwrap();
        })
        .peekable();
    fill_expiration_schedule(
        &mut data_layer,
        fill_expiration_settings,
        &mut order_book_2,
        PriceVariant::Sell,
        order_amount_2,
        &mut users,
        to_fill,
    );

    if !place_first_expiring {
        to_cancel_id = place_to_cancel(&mut order_book, &mut data_layer);
    }

    debug!("Committing data...");
    <OrderBooks<T>>::insert(order_book_id, order_book.clone());
    data_layer.commit();
    debug!("Data committed!");

    (order_book_id, to_cancel_id)
}

/// Prepare benchmark for `quote` extrinsic.
pub fn quote<T: Config>(
    fill_settings: FillSettings<T>,
) -> (
    T::DEXId,
    AssetIdOf<T>,
    AssetIdOf<T>,
    QuoteAmount<Balance>,
    bool,
) {
    let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };

    let max_lot_size = balance!(1000);

    OrderBookPallet::<T>::create_orderbook(
        RawOrigin::Root.into(),
        order_book_id,
        balance!(0.00001),
        balance!(0.00001),
        balance!(1),
        max_lot_size,
    )
    .expect("failed to create an order book");
    let mut order_book = <OrderBooks<T>>::get(order_book_id).unwrap();
    let mut data_layer = CacheDataLayer::<T>::new();

    // fill aggregated bids
    let _ = fill_order_book_worst_case::<T>(
        fill_settings,
        &mut order_book,
        &mut data_layer,
        true,
        false,
    );

    debug!("Committing data...");
    <OrderBooks<T>>::insert(order_book_id, order_book);
    data_layer.commit();
    debug!("Data committed!");

    let dex_id = order_book_id.dex_id;
    let input_asset_id = order_book_id.base;
    let output_asset_id = order_book_id.quote;
    let amount = QuoteAmount::with_desired_input(max_lot_size);
    let deduce_fee = true;
    (dex_id, input_asset_id, output_asset_id, amount, deduce_fee)
}

/// Prepare worst-case scenario for market order execution (`swap`/`exchange`). In particular, execution of
/// max possible orders # with partial execution of an order at the end.
///
/// - `amount_variant` - setting for the amount of the market order.
/// - `fill_settings` - settings for the benchmark; should be within storage constraints.
/// - `author` - the account from which the order is going to be executed. It should not be from
///   `test_utils::generate_account`.
/// - `is_divisible` - controls the divisibility of order book base asset.
///
/// Returns parameters necessary for the order execution. `OrderVolume` is in base asset.
pub fn market_order_execution<T: Config + trading_pair::Config>(
    amount_variant: AmountVariant,
    fill_settings: FillSettings<T>,
    author: T::AccountId,
    is_divisible: bool,
) -> (
    OrderBookId<AssetIdOf<T>, T::DEXId>,
    DealInfo<AssetIdOf<T>>,
    usize,
) {
    let order_book_id = if is_divisible {
        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        OrderBookPallet::<T>::create_orderbook(
            RawOrigin::Root.into(),
            order_book_id,
            balance!(0.00001),
            balance!(0.00001),
            balance!(1),
            balance!(1000),
        )
        .expect("failed to create an order book");

        order_book_id
    } else {
        let creator = accounts::bob::<T>();
        frame_system::Pallet::<T>::inc_providers(&creator);

        let nft = T::AssetManager::register_from(
            &accounts::bob::<T>(),
            common::AssetSymbol(b"NFT".to_vec()),
            common::AssetName(b"Nft".to_vec()),
            0,
            1000,
            false,
            common::AssetType::NFT,
            None,
            None,
        )
        .unwrap();

        let id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: nft,
            quote: XOR.into(),
        };
        trading_pair::Pallet::<T>::register(
            RawOrigin::Signed(creator).into(),
            DEX.into(),
            id.quote,
            id.base,
        )
        .unwrap();
        OrderBookPallet::<T>::create_orderbook(
            RawOrigin::Root.into(),
            id,
            balance!(0.00001),
            1,
            1,
            1000,
        )
        .expect("failed to create an order book");
        id
    };

    let mut order_book = <OrderBooks<T>>::get(order_book_id).unwrap();
    let mut data_layer = CacheDataLayer::<T>::new();
    let max_side_orders = fill_settings.max_side_orders();

    let limit_order_amount = fill_settings.amount_variant.calculate_amount(&order_book);
    let market_order_amount = amount_variant.calculate_amount(&order_book);
    let expected_executed_orders =
        (market_order_amount.balance() / limit_order_amount.balance()) as usize;

    let (_, _, direction) =
        prepare_order_execute_worst_case::<T>(&mut data_layer, &mut order_book, fill_settings);

    let (input, output, quote_amount) = match direction {
        PriceVariant::Buy => (
            &order_book_id.quote,
            &order_book_id.base,
            QuoteAmount::with_desired_output(*market_order_amount.balance()),
        ),
        PriceVariant::Sell => (
            &order_book_id.base,
            &order_book_id.quote,
            QuoteAmount::with_desired_input(*market_order_amount.balance()),
        ),
    };
    let info = order_book
        .calculate_deal(input, output, quote_amount, &mut data_layer)
        .unwrap();

    T::AssetManager::mint_unchecked(&order_book_id.base, &author, *market_order_amount.balance())
        .unwrap();

    debug!("Committing data...");
    data_layer.commit();
    debug!("Data committed!");

    assert_orders_numbers::<T>(
        order_book_id,
        Some(max_side_orders as usize),
        None,
        None,
        None,
    );

    (order_book_id, info, expected_executed_orders)
}

pub fn align_single_order<T: Config>(
    fill_settings: FillSettings<T>,
    side: PriceVariant,
) -> (OrderBook<T>, LimitOrder<T>) {
    let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };

    let (place_buy, place_sell) = match side {
        PriceVariant::Buy => (true, false),
        PriceVariant::Sell => (false, true),
    };

    OrderBookPallet::<T>::create_orderbook(
        RawOrigin::Root.into(),
        order_book_id,
        balance!(0.00001),
        balance!(0.00001),
        balance!(1),
        balance!(1000),
    )
    .expect("failed to create an order book");

    let mut order_book = <OrderBooks<T>>::get(order_book_id).unwrap();
    let mut data_layer = CacheDataLayer::<T>::new();

    debug!("Filling aggregated side");
    let mut prices_settings = fill_settings.clone();
    prices_settings.max_orders_per_price = 1;
    let (mut users, mut lifespans) = fill_order_book_worst_case::<T>(
        prices_settings,
        &mut order_book,
        &mut data_layer,
        place_buy,
        place_sell,
    );

    let prices_count = match side {
        PriceVariant::Buy => data_layer.get_aggregated_bids_len(&order_book_id).unwrap(),
        PriceVariant::Sell => data_layer.get_aggregated_asks_len(&order_book_id).unwrap(),
    };

    let aggregated_side = match side {
        PriceVariant::Buy => data_layer.get_aggregated_bids(&order_book_id),
        PriceVariant::Sell => data_layer.get_aggregated_asks(&order_book_id),
    };

    let (price, _) = aggregated_side.iter().nth(prices_count / 2).unwrap();

    debug!("Filling the price of the aligned order");
    let mut price_settings = fill_settings;
    price_settings.max_orders_per_price -= 2;
    let amount = order_book.min_lot_size;
    fill_price(
        &mut data_layer,
        price_settings,
        &mut order_book,
        side,
        amount,
        *price,
        &mut users,
        &mut lifespans,
    );

    let orders = data_layer.get_bids(&order_book_id, price).unwrap();
    let order_id_to_align = orders[orders.len() / 2];
    let order_to_align = data_layer
        .get_limit_order(&order_book_id, order_id_to_align)
        .unwrap();

    debug!("Committing data...");
    <OrderBooks<T>>::insert(order_book_id, order_book);
    data_layer.commit();
    debug!("Data committed!");

    (<OrderBooks<T>>::get(order_book_id).unwrap(), order_to_align)
}

pub mod presets {
    // TODO: rename to `order_book` after upgrading to nightly-2023-07-01+
    #[cfg(test)]
    use framenode_runtime::order_book as order_book_imported;
    #[cfg(not(test))]
    use order_book as order_book_imported;

    use order_book_imported::test_utils::fill_tools::{AmountVariant, FillSettings};
    use order_book_imported::Config;

    macro_rules! generate_presets {
        ($($name:ident: $($params:expr),+ $(,)? );+ $(;)? ) => {
            $(
            #[allow(unused)]
            pub fn $name<T: Config>() -> FillSettings<T> {
                FillSettings::<T>::new($($params),+)
            }
            )+
        };
    }

    // the preset values must not exceed hard limits set in pallet parameters
    generate_presets!(
        preset_1: 1024, 1024, 1024, 512, AmountVariant::Min;
        preset_2: 64, 64, 1024, 512, AmountVariant::Min;
    );
}
