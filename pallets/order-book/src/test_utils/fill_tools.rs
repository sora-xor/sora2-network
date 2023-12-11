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

//! Stuff that helps to fill up order book

#![cfg(feature = "ready-to-test")] // order-book

use super::{accounts, order_book_imported};
use order_book_imported::{
    traits::DataLayer, Config, ExpirationsAgenda, LimitOrder, MarketRole, OrderBook, OrderBookId,
    OrderPrice, OrderVolume, Pallet, Payment,
};

use assets::AssetIdOf;
use common::prelude::{BalanceUnit, Scalar};
use common::PriceVariant;
use frame_support::log::{debug, trace};
use frame_support::traits::{Get, Time};
use sp_runtime::traits::{CheckedMul, SaturatedConversion};
use sp_std::{collections::btree_map::BTreeMap, iter::repeat, vec::Vec};

/// iterator over the smallest possible bid prices (ascending)
pub fn bid_prices_iterator(
    tick_size: OrderPrice,
    max_side_price_count: u32,
) -> impl Iterator<Item = BalanceUnit> {
    (1..=max_side_price_count).map(move |i| tick_size * Scalar(i))
}

/// descending iterator over ask prices to have the smallest spread with `bid_prices_iterator`
pub fn ask_prices_iterator(
    tick_size: OrderPrice,
    max_side_price_count: u32,
) -> impl Iterator<Item = BalanceUnit> {
    (max_side_price_count + 1..=2 * max_side_price_count)
        .rev()
        .map(move |i| tick_size * Scalar(i))
}

/// iterator of authors for each order. gives out `max_orders_per_user` times of each user while
/// also minting assets for order placement
pub fn users_iterator<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    max_order_amount: OrderVolume,
    max_price: OrderPrice,
    max_orders_per_user: u32,
) -> impl Iterator<Item = T::AccountId> {
    let mint_per_user = max_order_amount * Scalar(max_orders_per_user);
    (1..)
        .map(accounts::generate_account::<T>)
        // each user receives assets that should be enough for placing their orders
        .inspect(move |user| {
            assets::Pallet::<T>::mint_unchecked(
                &order_book_id.base,
                user,
                *mint_per_user.balance(),
            )
            .unwrap();
            assets::Pallet::<T>::mint_unchecked(
                &order_book_id.quote,
                user,
                *max_price.checked_mul(&mint_per_user).unwrap().balance(),
            )
            .unwrap();
        })
        // yield same user for `max_orders_per_user` orders.
        // `inspect` is still called only once for each user.
        .flat_map(move |user| repeat(user).take(max_orders_per_user.try_into().unwrap()))
}

/// produces lifespans for new orders to successively fill each block expiration schedule
pub fn lifespans_iterator<T: Config>(
    max_expiring_orders_per_block: u32,
    start_from_block: u64,
) -> impl Iterator<Item = u64> {
    (start_from_block..)
        .map(|i| {
            i * T::MILLISECS_PER_BLOCK.saturated_into::<u64>()
                + T::MIN_ORDER_LIFESPAN.saturated_into::<u64>()
        })
        // same lifespan should be yielded for `max_expiring_orders_per_block` orders
        .flat_map(move |lifespan| {
            repeat(lifespan).take(max_expiring_orders_per_block.try_into().unwrap())
        })
}

#[derive(Clone, Debug)]
pub struct FillSettings<T: Config> {
    pub now: <<T as Config>::Time as Time>::Moment,
    pub max_side_price_count: u32,
    pub max_orders_per_price: u32,
    pub max_orders_per_user: u32,
    pub max_expiring_orders_per_block: u32,
}

impl<T: Config> FillSettings<T> {
    pub fn new(
        max_side_price_count: u32,
        max_orders_per_price: u32,
        max_orders_per_user: u32,
        max_expiring_orders_per_block: u32,
    ) -> Self {
        Self {
            now: T::Time::now(),
            max_side_price_count,
            max_orders_per_price,
            max_orders_per_user,
            max_expiring_orders_per_block,
        }
    }

    pub fn max() -> Self {
        Self::new(
            <T as Config>::MaxSidePriceCount::get(),
            <T as Config>::MaxLimitOrdersForPrice::get(),
            <T as Config>::MaxOpenedLimitOrdersPerUser::get(),
            <T as Config>::MaxExpiringOrdersPerBlock::get(),
        )
    }
}

pub fn fill_expiration_schedule<T: Config>(
    data: &mut impl DataLayer<T>,
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    order_amount: OrderVolume,
    users: &mut impl Iterator<Item = T::AccountId>,
    lifespan: u64,
) {
    let mut lifespans = repeat(lifespan).take(settings.max_expiring_orders_per_block as usize);
    match side {
        PriceVariant::Buy => fill_order_book_side(
            data,
            settings.clone(),
            order_book,
            side,
            order_amount,
            &mut bid_prices_iterator(order_book.tick_size, settings.max_side_price_count),
            users,
            &mut lifespans,
        ),
        PriceVariant::Sell => fill_order_book_side(
            data,
            settings.clone(),
            order_book,
            side,
            order_amount,
            &mut ask_prices_iterator(order_book.tick_size, settings.max_side_price_count),
            users,
            &mut lifespans,
        ),
    }
}

pub fn fill_user_orders<T: Config>(
    data: &mut impl DataLayer<T>,
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    order_amount: OrderVolume,
    author: T::AccountId,
    lifespans: &mut impl Iterator<Item = u64>,
) {
    let FillSettings {
        now: _,
        max_side_price_count,
        max_orders_per_price: _,
        max_orders_per_user,
        max_expiring_orders_per_block: _,
    } = settings;
    // Since we fill orders of the user, it is the only author
    match side {
        PriceVariant::Buy => {
            let max_price = order_book
                .tick_size
                .checked_mul_by_scalar(Scalar(max_side_price_count))
                .unwrap();
            assets::Pallet::<T>::mint_unchecked(
                &order_book.order_book_id.quote,
                &author,
                *max_price
                    .checked_mul(&order_amount)
                    .unwrap()
                    .checked_mul_by_scalar(Scalar(max_orders_per_user))
                    .unwrap()
                    .balance(),
            )
            .unwrap()
        }
        PriceVariant::Sell => assets::Pallet::<T>::mint_unchecked(
            &order_book.order_book_id.base,
            &author,
            *order_amount
                .checked_mul_by_scalar(Scalar(max_orders_per_user))
                .unwrap()
                .balance(),
        )
        .unwrap(),
    }
    let mut users = repeat(author).take(max_orders_per_user.try_into().unwrap());
    match side {
        PriceVariant::Buy => fill_order_book_side(
            data,
            settings,
            order_book,
            side,
            order_amount,
            &mut bid_prices_iterator(order_book.tick_size, max_side_price_count),
            &mut users,
            lifespans,
        ),
        PriceVariant::Sell => fill_order_book_side(
            data,
            settings,
            order_book,
            side,
            order_amount,
            &mut ask_prices_iterator(order_book.tick_size, max_side_price_count),
            &mut users,
            lifespans,
        ),
    };
}

/// Fill `side` of an `order_book` according to `settings`.
/// Places `settings.max_orders_per_price * prices.len()` orders at max.
///
/// Each order is for `orders_amount`. `users`, and `lifespans` specify
/// corresponding fields for each order. If at least one of the iterators finishes, it stops.
pub fn fill_order_book_side<T: Config>(
    data: &mut impl DataLayer<T>,
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    orders_amount: OrderVolume,
    prices: &mut impl Iterator<Item = OrderPrice>,
    users: &mut impl Iterator<Item = T::AccountId>,
    lifespans: &mut impl Iterator<Item = u64>,
) {
    let current_block = frame_system::Pallet::<T>::block_number();
    let mut total_payment = Payment::new(order_book.order_book_id);
    let mut to_expire = BTreeMap::<_, Vec<_>>::new();
    for price in prices {
        debug!("Fill price {:?}", price);
        fill_price_inner(
            data,
            settings.clone(),
            order_book,
            side,
            orders_amount,
            price,
            users,
            lifespans,
            current_block,
            &mut total_payment,
            &mut to_expire,
        );
    }
    total_payment.execute_all::<Pallet<T>, Pallet<T>>().unwrap();
    for (expires_at, orders) in to_expire.into_iter() {
        <ExpirationsAgenda<T>>::try_mutate(expires_at, |block_expirations| {
            block_expirations.try_extend(
                orders
                    .into_iter()
                    .map(|order_id| (order_book.order_book_id, order_id)),
            )
        })
        .expect("Failed to schedule orders for expiration");
    }
}

pub fn fill_price<T: Config>(
    data: &mut impl DataLayer<T>,
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    orders_amount: OrderVolume,
    price: OrderPrice,
    users: &mut impl Iterator<Item = T::AccountId>,
    lifespans: &mut impl Iterator<Item = u64>,
) {
    let current_block = frame_system::Pallet::<T>::block_number();
    let mut total_payment = Payment::new(order_book.order_book_id);
    let mut to_expire = BTreeMap::<_, Vec<_>>::new();
    fill_price_inner(
        data,
        settings,
        order_book,
        side,
        orders_amount,
        price,
        users,
        lifespans,
        current_block,
        &mut total_payment,
        &mut to_expire,
    );
    total_payment.execute_all::<Pallet<T>, Pallet<T>>().unwrap();
    // should avoid duplicating with `fill_order_book_worst_case` somehow
    for (expires_at, orders) in to_expire.into_iter() {
        <ExpirationsAgenda<T>>::try_mutate(expires_at, |block_expirations| {
            block_expirations.try_extend(
                orders
                    .into_iter()
                    .map(|order_id| (order_book.order_book_id, order_id)),
            )
        })
        .expect("Failed to schedule orders for expiration");
    }
}

#[inline]
/// Version of `fill_price` for optimized execution that aggregates payments/expirations (and
/// avoids multiple calls to retrieve block #)
fn fill_price_inner<T: Config>(
    data: &mut impl DataLayer<T>,
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    side: PriceVariant,
    orders_amount: OrderVolume,
    price: OrderPrice,
    users: &mut impl Iterator<Item = T::AccountId>,
    lifespans: &mut impl Iterator<Item = u64>,
    current_block: T::BlockNumber,
    total_payment: &mut Payment<T::AssetId, T::AccountId, T::DEXId>,
    to_expire: &mut BTreeMap<T::BlockNumber, Vec<T::OrderId>>,
) {
    for _ in 0..settings.max_orders_per_price {
        let Some(user) = users.next() else {
                debug!("`users` iterator exhausted, stopping placement");
                break
            };
        let Some(lifespan) = lifespans.next() else {
                debug!("`users` iterator exhausted, stopping placement");
                break
            };
        let order = LimitOrder::<T>::new(
            order_book.next_order_id(),
            user.clone(),
            side,
            price,
            orders_amount,
            settings.now,
            lifespan.saturated_into(),
            current_block,
        );
        // Instead of `order_book.place_limit_order(order, data)` we do the same steps manually
        // in order to avoid overhead on checking various restrictions and other unnecessary
        // stuff

        let order_id = order.id;
        let expires_at = order.expires_at;
        // lock corresponding currency
        let lock_amount = order.deal_amount(MarketRole::Taker, None).unwrap();
        let lock_asset = lock_amount.associated_asset(&order_book.order_book_id);
        total_payment
            .to_lock
            .entry(*lock_asset)
            .or_default()
            .entry(order.owner.clone())
            .and_modify(|amount| *amount += *lock_amount.value())
            .or_insert(*lock_amount.value());
        // insert the order in storages
        trace!(
            "placing next order {:?}",
            (
                order_book.next_order_id(),
                user.clone(),
                side,
                price,
                orders_amount,
                settings.now,
                lifespan,
                current_block,
            )
        );
        data.insert_limit_order(&order_book.order_book_id, order)
            .unwrap();
        // schedule its expiration
        to_expire.entry(expires_at).or_default().push(order_id);
    }
}

/// Returns per-order iterators for users and lifespans. They can be used for proceeding with
/// filling respective storages (user orders and expiration schedules respectively). Can be seen
/// as cursors.
pub fn fill_order_book_worst_case<T: Config + assets::Config>(
    settings: FillSettings<T>,
    order_book: &mut OrderBook<T>,
    data: &mut impl DataLayer<T>,
    place_buy: bool,
    place_sell: bool,
) -> (
    impl Iterator<Item = T::AccountId>,
    impl Iterator<Item = u64>,
) {
    let order_amount = sp_std::cmp::max(order_book.step_lot_size, order_book.min_lot_size);
    let max_price = order_book.tick_size * Scalar(2 * settings.max_side_price_count);

    // Owners for each placed order
    let mut users = users_iterator::<T>(
        order_book.order_book_id,
        order_amount,
        max_price,
        settings.max_orders_per_user,
    );
    // Lifespans for each placed order
    let mut lifespans = lifespans_iterator::<T>(settings.max_expiring_orders_per_block, 1);

    if place_buy {
        let mut bid_prices =
            bid_prices_iterator(order_book.tick_size, settings.max_side_price_count);
        fill_order_book_side(
            data,
            settings.clone(),
            order_book,
            PriceVariant::Buy,
            order_amount,
            &mut bid_prices,
            &mut users,
            &mut lifespans,
        );
    }

    if place_sell {
        let mut ask_prices =
            ask_prices_iterator(order_book.tick_size, settings.max_side_price_count);
        fill_order_book_side(
            data,
            settings,
            order_book,
            PriceVariant::Sell,
            order_amount,
            &mut ask_prices,
            &mut users,
            &mut lifespans,
        );
    }
    (users, lifespans)
}
