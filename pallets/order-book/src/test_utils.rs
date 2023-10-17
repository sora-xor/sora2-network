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

#![cfg(feature = "wip")] // order-book

// TODO: rename by `order_book` after upgrading to nightly-2023-07-01+
#[cfg(not(test))]
use crate as order_book_imported;
#[cfg(test)]
use framenode_runtime::order_book as order_book_imported;

use order_book_imported::{
    traits::DataLayer, Config, ExpirationsAgenda, LimitOrder, MarketRole, OrderBook, OrderBookId,
    OrderPrice, OrderVolume, Pallet, Payment, PriceOrders,
};
#[cfg(feature = "std")]
use order_book_imported::{Asks, Bids, LimitOrders};

use assets::AssetIdOf;
use codec::Decode;
#[cfg(feature = "std")]
use common::prelude::FixedWrapper;
use common::prelude::{BalanceUnit, Scalar};
use common::{balance, AssetInfoProvider, Balance, DexIdOf, PriceVariant};
use frame_support::assert_ok;
use frame_support::log::{debug, trace};
use frame_support::traits::{Get, Time};
use frame_system::RawOrigin;
#[cfg(feature = "std")]
use sp_runtime::traits::{CheckedAdd, Zero};
use sp_runtime::traits::{CheckedMul, SaturatedConversion};
#[cfg(feature = "std")]
use sp_runtime::BoundedVec;
use sp_std::{collections::btree_map::BTreeMap, iter::repeat, vec::Vec};

pub const DEX: common::DEXId = common::DEXId::Polkaswap;
pub const INIT_BALANCE: Balance = balance!(1000000);

pub fn alice<T: frame_system::Config>() -> <T as frame_system::Config>::AccountId {
    <T as frame_system::Config>::AccountId::decode(&mut &[1u8; 32][..]).unwrap()
}

pub fn bob<T: frame_system::Config>() -> <T as frame_system::Config>::AccountId {
    <T as frame_system::Config>::AccountId::decode(&mut &[2u8; 32][..]).unwrap()
}

pub fn charlie<T: frame_system::Config>() -> <T as frame_system::Config>::AccountId {
    <T as frame_system::Config>::AccountId::decode(&mut &[3u8; 32][..]).unwrap()
}

pub fn dave<T: frame_system::Config>() -> <T as frame_system::Config>::AccountId {
    <T as frame_system::Config>::AccountId::decode(&mut &[4u8; 32][..]).unwrap()
}

pub fn generate_account<T: frame_system::Config>(
    seed: u32,
) -> <T as frame_system::Config>::AccountId {
    let mut adr = [0u8; 32];

    let mut value = seed;
    let mut id = 0;
    while value != 0 {
        adr[31 - id] = (value % 256) as u8;
        value = value / 256;
        id += 1;
    }

    <T as frame_system::Config>::AccountId::decode(&mut &adr[..]).unwrap()
}

pub fn free_balance<T: assets::Config + frame_system::Config>(
    asset: &AssetIdOf<T>,
    account: &<T as frame_system::Config>::AccountId,
) -> Balance {
    assets::Pallet::<T>::free_balance(asset, account).expect("Asset must exist")
}

pub fn fill_balance<T: assets::Config + frame_system::Config>(
    account: <T as frame_system::Config>::AccountId,
    order_book_id: OrderBookId<AssetIdOf<T>, DexIdOf<T>>,
) {
    assert_ok!(assets::Pallet::<T>::update_balance(
        RawOrigin::Root.into(),
        account.clone(),
        order_book_id.base,
        INIT_BALANCE.try_into().unwrap()
    ));

    assert_ok!(assets::Pallet::<T>::update_balance(
        RawOrigin::Root.into(),
        account,
        order_book_id.quote,
        INIT_BALANCE.try_into().unwrap()
    ));
}

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
        .map(generate_account::<T>)
        // each user receives assets that should be enough for placing their orders
        .inspect(move |user| {
            assets::Pallet::<T>::mint_unchecked(
                &order_book_id.base,
                &user,
                *mint_per_user.balance(),
            )
            .unwrap();
            assets::Pallet::<T>::mint_unchecked(
                &order_book_id.quote,
                &user,
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

pub fn create_empty_order_book<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, DexIdOf<T>>,
) -> OrderBook<T> {
    assert_ok!(Pallet::<T>::create_orderbook(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id
    ));

    Pallet::<T>::order_books(order_book_id).unwrap()
}

// Creates and fills the order book
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
pub fn create_and_fill_order_book<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, DexIdOf<T>>,
) -> OrderBook<T> {
    assert_ok!(Pallet::<T>::create_orderbook(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id
    ));

    fill_balance::<T>(bob::<T>(), order_book_id);
    fill_balance::<T>(charlie::<T>(), order_book_id);

    let lifespan = Some(100000u32.into());

    // prices
    let bp1 = balance!(10);
    let bp2 = balance!(9.8);
    let bp3 = balance!(9.5);
    let sp1 = balance!(11);
    let sp2 = balance!(11.2);
    let sp3 = balance!(11.5);

    // buy amounts
    let amount1 = balance!(168.5);
    let amount2 = balance!(95.2);
    let amount3 = balance!(44.7);
    let amount4 = balance!(56.4);
    let amount5 = balance!(89.9);
    let amount6 = balance!(115);

    // sell amounts
    let amount7 = balance!(176.3);
    let amount8 = balance!(85.4);
    let amount9 = balance!(93.2);
    let amount10 = balance!(36.6);
    let amount11 = balance!(205.5);
    let amount12 = balance!(13.7);

    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp1,
        amount1,
        PriceVariant::Buy,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(charlie::<T>()).into(),
        order_book_id,
        bp2,
        amount2,
        PriceVariant::Buy,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp2,
        amount3,
        PriceVariant::Buy,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(charlie::<T>()).into(),
        order_book_id,
        bp3,
        amount4,
        PriceVariant::Buy,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp3,
        amount5,
        PriceVariant::Buy,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(charlie::<T>()).into(),
        order_book_id,
        bp3,
        amount6,
        PriceVariant::Buy,
        lifespan
    ));

    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp1,
        amount7,
        PriceVariant::Sell,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(charlie::<T>()).into(),
        order_book_id,
        sp2,
        amount8,
        PriceVariant::Sell,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp2,
        amount9,
        PriceVariant::Sell,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(charlie::<T>()).into(),
        order_book_id,
        sp3,
        amount10,
        PriceVariant::Sell,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp3,
        amount11,
        PriceVariant::Sell,
        lifespan
    ));
    assert_ok!(Pallet::<T>::place_limit_order(
        RawOrigin::Signed(charlie::<T>()).into(),
        order_book_id,
        sp3,
        amount12,
        PriceVariant::Sell,
        lifespan
    ));

    fn slice_to_price_orders<T: Config>(
        v: &[u32],
    ) -> PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice> {
        v.into_iter()
            .map(|id| T::OrderId::from(*id))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    // check
    assert_eq!(
        Pallet::<T>::bids(order_book_id, OrderPrice::divisible(bp1)).unwrap(),
        slice_to_price_orders::<T>(&[1])
    );
    assert_eq!(
        Pallet::<T>::bids(order_book_id, OrderPrice::divisible(bp2)).unwrap(),
        slice_to_price_orders::<T>(&[2, 3])
    );
    assert_eq!(
        Pallet::<T>::bids(order_book_id, OrderPrice::divisible(bp3)).unwrap(),
        slice_to_price_orders::<T>(&[4, 5, 6])
    );

    assert_eq!(
        Pallet::<T>::asks(order_book_id, OrderPrice::divisible(sp1)).unwrap(),
        slice_to_price_orders::<T>(&[7])
    );
    assert_eq!(
        Pallet::<T>::asks(order_book_id, OrderPrice::divisible(sp2)).unwrap(),
        slice_to_price_orders::<T>(&[8, 9])
    );
    assert_eq!(
        Pallet::<T>::asks(order_book_id, OrderPrice::divisible(sp3)).unwrap(),
        slice_to_price_orders::<T>(&[10, 11, 12])
    );

    assert_eq!(
        Pallet::<T>::aggregated_bids(&order_book_id),
        BTreeMap::from([
            (bp1.into(), amount1.into()),
            (bp2.into(), (amount2 + amount3).into()),
            (bp3.into(), (amount4 + amount5 + amount6).into())
        ])
    );
    assert_eq!(
        Pallet::<T>::aggregated_asks(&order_book_id),
        BTreeMap::from([
            (sp1.into(), amount7.into()),
            (sp2.into(), (amount8 + amount9).into()),
            (sp3.into(), (amount10 + amount11 + amount12).into())
        ])
    );

    Pallet::<T>::order_books(order_book_id).unwrap()
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
        let Some(user) = users.next() else { debug!("`users` iterator exhausted, stopping placement"); break };
        let Some(lifespan) = lifespans.next() else { debug!("`users` iterator exhausted, stopping placement"); break };
        let order = LimitOrder::<T>::new(
            order_book.next_order_id(),
            user.clone(),
            side,
            price,
            orders_amount,
            settings.now.clone(),
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
                settings.now.clone(),
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

#[cfg(feature = "std")]
fn print_side<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    side: PriceVariant,
    column_width: usize,
) {
    let side_orders: Vec<(
        OrderPrice,
        crate::PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>,
    )> = match side {
        PriceVariant::Buy => {
            let mut side_orders: Vec<_> = Bids::<T>::iter_prefix(order_book_id).collect();
            side_orders.sort_by_key(|value| value.0);
            side_orders.reverse();
            side_orders
        }
        PriceVariant::Sell => {
            let mut side_orders: Vec<_> = Asks::<T>::iter_prefix(order_book_id).collect();
            side_orders.sort_by_key(|value| value.0);
            side_orders
        }
    };
    let order_data: BTreeMap<T::OrderId, LimitOrder<T>> =
        LimitOrders::<T>::iter_prefix(order_book_id).collect();
    for (price, price_order_ids) in side_orders {
        let price_orders: Vec<_> = price_order_ids
            .iter()
            .map(|id| order_data.get(id).unwrap())
            .collect();
        let volume: OrderVolume = price_orders
            .iter()
            .map(|order| order.amount)
            .fold(OrderVolume::zero(), |acc, item| {
                acc.checked_add(&item).unwrap()
            });
        print!(
            "{:>1$} |",
            FixedWrapper::from(*price.balance())
                .get()
                .unwrap()
                .to_string(),
            column_width - 1
        );
        print!(
            "{:>1$} |",
            FixedWrapper::from(*volume.balance())
                .get()
                .unwrap()
                .to_string(),
            column_width - 1
        );
        println!(
            " {}",
            price_order_ids
                .iter()
                .fold("".to_owned(), |s, id| s + &id.to_string() + ", ")
        );
    }
}

/// Print in the following form:
/// ```text
/// price | volume | orders
///          Asks
///  11.5 |  255.8 | sell4, sell5, sell6
///  11.2 |  178.6 | sell2, sell3
///  11.0 |  176.3 | sell1
///  spread
///  10.0 |  168.5 | buy1
///   9.8 |  139.9 | buy2, buy3
///   9.5 |  261.3 | buy4, buy5, buy6
///          Bids
/// ```
#[cfg(feature = "std")]
pub fn pretty_print_order_book<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    column_width: Option<usize>,
) {
    let column_width = column_width.unwrap_or(8);
    println!(
        "{0:>3$} |{1:>3$} |{2:>3$} ",
        "price",
        "volume",
        "orders",
        column_width - 1
    );
    println!("\tAsks");
    print_side::<T>(order_book_id, PriceVariant::Sell, column_width);
    println!(") spread");
    print_side::<T>(order_book_id, PriceVariant::Buy, column_width);
    println!("\tBids\n");
}

#[cfg(feature = "std")]
fn print_block_expirations<T: Config>(block: u32)
where
    T::BlockNumber: From<u32>,
{
    let block = T::BlockNumber::from(block);
    let expirations: BoundedVec<
        (OrderBookId<AssetIdOf<T>, T::DEXId>, T::OrderId),
        T::MaxExpiringOrdersPerBlock,
    > = ExpirationsAgenda::<T>::get(block);
    for (order_book_id, order_id) in expirations {
        println!(
            "{:>5} | base: {:?}; quote: {:?} |{:>4} ",
            block, order_book_id.base, order_book_id.quote, order_id
        );
    }
}

/// Print expirations agenda in the form:
///
/// ```text
/// block number | order book id | order id
/// ```
#[cfg(feature = "std")]
pub fn pretty_print_expirations<T: Config>(blocks: sp_std::ops::Range<u32>)
where
    T::BlockNumber: TryFrom<u32>,
{
    println!("block |{:>148} | order id", "order book id");
    for block in blocks {
        print_block_expirations::<T>(block)
    }
}

#[cfg(test)]
pub use test_only::*;

#[cfg(test)]
mod test_only {
    use super::*;
    use frame_support::assert_ok;
    use frame_support::traits::Hooks;
    use frame_support::weights::Weight;
    use framenode_runtime::order_book::{self, Config, OrderBookId, Pallet};
    use framenode_runtime::{Runtime, RuntimeOrigin};

    pub type E = order_book::Error<Runtime>;
    pub type OrderBookPallet = Pallet<Runtime>;
    pub type DEXId = DexIdOf<Runtime>;

    pub fn fill_balance(
        account: <Runtime as frame_system::Config>::AccountId,
        order_book_id: OrderBookId<AssetIdOf<Runtime>, DEXId>,
    ) {
        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            account.clone(),
            order_book_id.base,
            INIT_BALANCE.try_into().unwrap()
        ));

        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            account,
            order_book_id.quote,
            INIT_BALANCE.try_into().unwrap()
        ));
    }

    pub fn get_last_order_id(
        order_book_id: OrderBookId<AssetIdOf<Runtime>, DEXId>,
    ) -> Option<<Runtime as Config>::OrderId> {
        if let Some(order_book) = OrderBookPallet::order_books(order_book_id) {
            Some(order_book.last_order_id)
        } else {
            None
        }
    }

    /// Returns weight spent on initializations
    pub fn run_to_block(n: u32) -> Weight {
        type System = frame_system::Pallet<Runtime>;
        let mut total_init_weight = 0.into();
        while System::block_number() < n {
            OrderBookPallet::on_finalize(System::block_number());
            System::set_block_number(System::block_number() + 1);
            total_init_weight += OrderBookPallet::on_initialize(System::block_number());
        }
        total_init_weight
    }

    pub fn update_orderbook_unchecked(
        order_book_id: OrderBookId<AssetIdOf<Runtime>, DEXId>,
        tick_size: Balance,
        step_lot_size: Balance,
        min_lot_size: Balance,
        max_lot_size: Balance,
    ) -> OrderBook<Runtime> {
        let mut order_book = OrderBookPallet::order_books(order_book_id).unwrap();
        order_book.tick_size.set(tick_size);
        order_book.step_lot_size.set(step_lot_size);
        order_book.min_lot_size.set(min_lot_size);
        order_book.max_lot_size.set(max_lot_size);
        framenode_runtime::order_book::OrderBooks::<Runtime>::set(order_book_id, Some(order_book));

        OrderBookPallet::order_books(order_book_id).unwrap()
    }
}
