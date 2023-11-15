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

// TODO #167: fix clippy warnings
#![allow(clippy::all)]

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

use assets::AssetIdOf;
use common::prelude::{QuoteAmount, Scalar};
use common::{Balance, PriceVariant, ETH, VAL, XOR};
use frame_benchmarking::log::debug;
use frame_support::traits::Time;
use frame_system::RawOrigin;
use order_book_imported::test_utils::fill_tools::{
    bid_prices_iterator, fill_expiration_schedule, fill_order_book_side,
    fill_order_book_worst_case, fill_price, fill_user_orders, lifespans_iterator, users_iterator,
    FillSettings,
};
use order_book_imported::test_utils::{accounts, update_order_book_with_set_status};
use order_book_imported::{
    cache_data_layer::CacheDataLayer, traits::DataLayer, LimitOrder, MomentOf, OrderBook,
    OrderBookId, OrderBooks, OrderPrice, OrderVolume,
};
use sp_runtime::traits::{CheckedMul, SaturatedConversion, Scale};
use sp_std::iter::Peekable;

use order_book_benchmarking_imported::{assert_orders_numbers, Config, DEX};

use order_book_imported::Pallet as OrderBookPallet;

/// In order to scatter, the orders must be placed in the following way:
///
/// ```text
/// 1st part                    2nd part
/// ==========================|
/// ==========================|========================
///
/// worst price                              best_price
/// ```
///
/// Returns `(settings_for_1st_part, settings_for_2nd_part)`
///
/// If not `scattered`, 2nd part is empty
///
fn worst_case_settings_parts<T: Config>(
    fill_settings: FillSettings<T>,
    scattered: bool,
) -> (FillSettings<T>, FillSettings<T>) {
    if scattered {
        let max_side_orders = sp_std::cmp::min(
            fill_settings.max_orders_per_price as u128 * fill_settings.max_side_price_count as u128,
            fill_settings.executed_orders_limit as u128,
        );
        // the orders are to be placed as even as possible, having either
        // `ceil(max_side_orders/max_side_price_count)` or `floor(max_side_orders/max_side_price_count)`
        // orders in each price

        // first part: `ceil(..)`
        let settings_1st = FillSettings {
            max_orders_per_price: max_side_orders
                .div_ceil(fill_settings.max_side_price_count as u128)
                .try_into()
                .unwrap(),
            max_side_price_count: max_side_orders
                .rem(fill_settings.max_side_price_count as u128)
                .try_into()
                .unwrap(),
            ..fill_settings
        };
        // second part: `floor(..)`
        let settings_2nd = FillSettings {
            max_orders_per_price: max_side_orders
                .div_floor(fill_settings.max_side_price_count as u128)
                .try_into()
                .unwrap(),
            max_side_price_count: fill_settings.max_side_price_count
                - settings_1st.max_side_price_count,
            ..fill_settings
        };
        (settings_1st, settings_2nd)
    } else {
        let empty_settings = FillSettings {
            now: fill_settings.now,
            max_side_price_count: 0,
            max_orders_per_price: 0,
            max_orders_per_user: 0,
            max_expiring_orders_per_block: 0,
            executed_orders_limit: 0,
        };
        (fill_settings, empty_settings)
    }
}

/// Places buy orders for worst-case execution. Fill up max amount allowed by the storages or by
/// `fill_settings.executed_orders_limit`.
///
/// If `double_worst_order_amount` is true, one order in the worst price is set for twice of the
/// amount; it allows partial execution.
///
/// Price of the order to execute is minimal possible in the order book.
///
/// Returns:
/// - iterators in the same way as `fill_order_book_worst_case`
/// - volume of an order for worst-case execution (for partial execution of the last one, in case
/// `double_worst_order_amount` is true)
/// - price variant of the order
fn prepare_order_execute_worst_case<T: Config>(
    data: &mut impl DataLayer<T>,
    order_book: &mut OrderBook<T>,
    fill_settings: FillSettings<T>,
    double_worst_order_amount: bool,
    scatter: bool,
) -> (
    Peekable<impl Iterator<Item = T::AccountId>>,
    Peekable<impl Iterator<Item = u64>>,
    OrderVolume,
    PriceVariant,
) {
    let orders_amount = sp_std::cmp::max(order_book.step_lot_size, order_book.min_lot_size);
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
    // maximum number of executed orders is either max the storages allow or the value from
    // the settings, whichever restricts the most
    let max_side_orders = sp_std::cmp::min(
        fill_settings.max_orders_per_price as u128 * fill_settings.max_side_price_count as u128,
        fill_settings.executed_orders_limit as u128,
    );
    let mut orders_to_place = max_side_orders;
    let (mut settings_1st, settings_2nd) = worst_case_settings_parts(fill_settings, scatter);
    let mut bid_prices = bid_prices_iterator(
        order_book.tick_size,
        settings_1st.max_side_price_count + settings_2nd.max_side_price_count,
    );

    if double_worst_order_amount {
        // The worst price is a special case:
        // the last order in the price has double of the amount to allow partial execution
        debug!("Filling worst price to allow partial execution of one order");
        // it always falls into `settings_1st`
        let min_price = bid_prices.next().unwrap();

        let mut fill_price_settings = settings_1st.clone();
        fill_price_settings.max_orders_per_price = sp_std::cmp::min(
            orders_to_place.try_into().unwrap_or(u32::MAX),
            fill_price_settings.max_orders_per_price,
        );
        fill_price_settings.max_orders_per_price -= 1;
        orders_to_place -= fill_price_settings.max_orders_per_price as u128;
        fill_price(
            data,
            fill_price_settings,
            order_book,
            orders_side,
            orders_amount,
            min_price,
            &mut users,
            &mut lifespans,
        );

        // place double amount order
        let id = order_book.next_order_id();
        let order = LimitOrder::<T>::new(
            id,
            users.next().unwrap(),
            orders_side,
            min_price,
            orders_amount * Scalar(2u64),
            T::Time::now(),
            lifespans.next().unwrap().saturated_into(),
            frame_system::Pallet::<T>::block_number(),
        );
        // just in case
        assets::Pallet::<T>::mint_unchecked(
            &order_book.order_book_id.quote,
            &order.owner,
            *order.price.checked_mul(&order.amount).unwrap().balance(),
        )
        .unwrap();
        order_book.place_limit_order(order, data).unwrap();
        orders_to_place -= 1;
        settings_1st.max_side_price_count = settings_1st
            .max_side_price_count
            .checked_sub(1)
            .expect("Must have at least one price in the first part");
    }
    // any of the iterators can limit number of placed orders; `users` is one of them
    let mut limited_users = users
        .by_ref()
        .take(orders_to_place.try_into().unwrap())
        .peekable(); // would be great to reuse the peekable inside; not sure how

    debug!("Filling a side of the order book for worst-case execution");
    debug!(
        "Filling 1st part with settings ({:?} orders)",
        settings_1st.max_side_price_count * settings_1st.max_orders_per_price
    );
    let mut bid_prices_1st = bid_prices
        .by_ref()
        .take(settings_1st.max_side_price_count as usize);
    fill_order_book_side(
        data,
        settings_1st.clone(),
        order_book,
        orders_side,
        orders_amount,
        &mut bid_prices_1st,
        &mut limited_users,
        &mut lifespans,
    );
    debug!(
        "Filling 2nd part with settings ({:?} orders)",
        settings_2nd.max_side_price_count * settings_2nd.max_orders_per_price
    );
    fill_order_book_side(
        data,
        settings_2nd.clone(),
        order_book,
        orders_side,
        orders_amount,
        &mut bid_prices,
        &mut limited_users,
        &mut lifespans,
    );

    // all orders were placed
    #[allow(unused_variables)]
    let orders_to_place = 0;
    assert!(limited_users.next().is_none(), "did not place all orders");
    debug!("Update order book to allow execution of max # of orders");
    let to_execute_volume = order_book
        .min_lot_size
        .checked_mul_by_scalar(Scalar(max_side_orders))
        .unwrap();
    // new values for min/max lot size to check maximum possible number of executed orders at once
    let new_max_lot_size = sp_std::cmp::max(to_execute_volume, order_book.max_lot_size);
    let new_min_lot_size = new_max_lot_size.copy_divisibility(
        new_max_lot_size
            .balance()
            .div_ceil(T::SOFT_MIN_MAX_RATIO.try_into().unwrap()),
    );
    update_order_book_with_set_status::<T>(
        order_book,
        None,
        None,
        Some(new_min_lot_size),
        Some(new_max_lot_size),
    )
    .unwrap();
    (users, lifespans, to_execute_volume, orders_side.switched())
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
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id,
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

    // The price where the order is going to be placed should be filled
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
    OrderBookPallet::<T>::create_orderbook(
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id_2,
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
            assets::Pallet::<T>::mint_unchecked(
                &order_book_id_2.base,
                &user,
                *order_amount_2.balance(),
            )
            .unwrap();
        })
        .peekable();

    fill_expiration_schedule(
        &mut data_layer,
        fill_expiration_settings.clone(),
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
    assets::Pallet::<T>::mint_unchecked(&order_book_id.base, &author, *order_amount.balance())
        .unwrap();

    let expected_user_orders = sp_std::cmp::min(
        fill_settings.max_orders_per_user - 1,
        fill_settings.max_side_price_count * fill_settings.max_orders_per_price,
    ) as usize;

    assert_orders_numbers::<T>(
        order_book_id,
        Some(expected_user_orders),
        Some((fill_settings.max_orders_per_price - 1) as usize),
        Some((author.clone(), expected_user_orders)),
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
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id,
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
    // we don't want to face `MAX_PRICE_SHIFT`
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

    assets::Pallet::<T>::mint_unchecked(
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
    OrderBookPallet::<T>::create_orderbook(
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id_2,
    )
    .expect("failed to create an order book");
    let mut order_book_2 = <OrderBooks<T>>::get(order_book_id_2).unwrap();
    let order_amount_2 = sp_std::cmp::max(order_book_2.step_lot_size, order_book_2.min_lot_size);
    let mut fill_expiration_settings = fill_settings.clone();
    // we add one more separately
    fill_expiration_settings.max_expiring_orders_per_block -= 1;
    // mint other base asset as well
    let mut users = users
        .inspect(move |user| {
            assets::Pallet::<T>::mint_unchecked(
                &order_book_id_2.base,
                &user,
                *order_amount_2.balance(),
            )
            .unwrap();
        })
        .peekable();
    fill_expiration_schedule(
        &mut data_layer,
        fill_expiration_settings.clone(),
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
) -> (T::DEXId, T::AssetId, T::AssetId, QuoteAmount<Balance>, bool) {
    let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };
    OrderBookPallet::<T>::create_orderbook(
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id,
    )
    .expect("failed to create an order book");
    let mut order_book = <OrderBooks<T>>::get(order_book_id).unwrap();
    let mut data_layer = CacheDataLayer::<T>::new();

    // fill aggregated bids
    let mut buy_settings = fill_settings.clone();
    buy_settings.max_orders_per_price = 1;
    let _ = fill_order_book_worst_case::<T>(
        buy_settings,
        &mut order_book,
        &mut data_layer,
        true,
        false,
    );

    let (total_bids_amount, _) = order_book
        .sum_market(
            data_layer
                .get_aggregated_bids(&order_book.order_book_id)
                .iter(),
            None,
        )
        .unwrap();
    assert!(total_bids_amount.is_base());
    let total_bids_base_amount = total_bids_amount.value();

    debug!("Committing data...");
    <OrderBooks<T>>::insert(order_book_id, order_book);
    data_layer.commit();
    debug!("Data committed!");

    let dex_id = order_book_id.dex_id;
    let input_asset_id = order_book_id.base;
    let output_asset_id = order_book_id.quote;
    let amount = QuoteAmount::with_desired_input(*total_bids_base_amount.balance());
    let deduce_fee = true;
    (dex_id, input_asset_id, output_asset_id, amount, deduce_fee)
}

/// Prepare worst-case scenario for market order execution (`swap`/`exchange`). In particular, execution of
/// max possible orders # with partial execution of an order at the end.
///
/// - `fill_settings` - settings for the benchmark; should be within storage constraints.
/// - `author` - the account from which the order is going to be executed. It should not be from
/// `test_utils::generate_account`.
/// - `is_divisible` - controls the divisibility of order book base asset.
/// - `scatter` - if true, spreads orders across all prices (as even as possible).
///
/// Returns parameters necessary for the order execution. `OrderVolume` is in base asset.
pub fn market_order_execution<T: Config + trading_pair::Config>(
    fill_settings: FillSettings<T>,
    author: T::AccountId,
    is_divisible: bool,
    scatter: bool,
) -> (
    OrderBookId<AssetIdOf<T>, T::DEXId>,
    OrderVolume,
    PriceVariant,
) {
    let order_book_id = if is_divisible {
        OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        }
    } else {
        let creator = accounts::bob::<T>();
        frame_system::Pallet::<T>::inc_providers(&creator);

        let nft = assets::Pallet::<T>::register_from(
            &accounts::bob::<T>(),
            common::AssetSymbol(b"NFT".to_vec()),
            common::AssetName(b"Nft".to_vec()),
            0,
            1,
            false,
            None,
            None,
        )
        .unwrap();

        let id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: nft.clone(),
            quote: XOR.into(),
        };
        trading_pair::Pallet::<T>::register(
            RawOrigin::Signed(creator.clone()).into(),
            DEX.into(),
            id.quote,
            id.base,
        )
        .unwrap();
        id
    };
    OrderBookPallet::<T>::create_orderbook(
        RawOrigin::Signed(accounts::bob::<T>()).into(),
        order_book_id,
    )
    .expect("failed to create an order book");
    let mut order_book = <OrderBooks<T>>::get(order_book_id).unwrap();
    let mut data_layer = CacheDataLayer::<T>::new();
    let max_side_orders = sp_std::cmp::min(
        fill_settings.max_orders_per_price as u128 * fill_settings.max_side_price_count as u128,
        fill_settings.executed_orders_limit as u128,
    );

    if !is_divisible {
        // to allow order book update
        let needed_supply = *sp_std::cmp::max(
            order_book
                .min_lot_size
                .checked_mul_by_scalar(Scalar(max_side_orders + 1))
                .unwrap(),
            order_book.max_lot_size,
        )
        .balance();
        assets::Pallet::<T>::mint_unchecked(
            &order_book_id.base,
            &accounts::bob::<T>(),
            needed_supply,
        )
        .unwrap();
    }

    let (_, _, amount, side) = prepare_order_execute_worst_case::<T>(
        &mut data_layer,
        &mut order_book,
        fill_settings.clone(),
        true,
        scatter,
    );

    assets::Pallet::<T>::mint_unchecked(&order_book_id.base, &author, *amount.balance()).unwrap();

    debug!("Committing data...");
    <OrderBooks<T>>::insert(order_book_id, order_book);
    data_layer.commit();
    debug!("Data committed!");

    assert_orders_numbers::<T>(
        order_book_id,
        Some(max_side_orders as usize),
        None,
        None,
        None,
    );

    (order_book_id, amount, side)
}

pub mod presets {
    // TODO: rename to `order_book` after upgrading to nightly-2023-07-01+
    #[cfg(test)]
    use framenode_runtime::order_book as order_book_imported;
    #[cfg(not(test))]
    use order_book as order_book_imported;

    use order_book_imported::test_utils::fill_tools::FillSettings;
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
        preset_1: 1024, 1024, 1024, 512, <T as Config>::HARD_MIN_MAX_RATIO.try_into().unwrap();
        preset_2: 64, 64, 1024, 512, <T as Config>::HARD_MIN_MAX_RATIO.try_into().unwrap();
    );
}
