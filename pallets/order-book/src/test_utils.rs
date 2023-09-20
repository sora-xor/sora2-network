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

#[allow(unused)]
#[cfg(not(test))]
use crate::{
    self as order_book, cache_data_layer::CacheDataLayer, traits::DataLayer, Asks, Bids, Config,
    Event, ExpirationScheduler, ExpirationsAgenda, LimitOrder, LimitOrders, MarketRole, MomentOf,
    OrderAmount, OrderBook, OrderBookId, OrderBookStatus, OrderBooks, OrderPrice, OrderVolume,
    Pallet, Payment, PriceOrders,
};
#[allow(unused)]
#[cfg(test)]
use framenode_runtime::order_book::{
    self as order_book, cache_data_layer::CacheDataLayer, traits::DataLayer, Asks, Bids, Config,
    Event, ExpirationScheduler, ExpirationsAgenda, LimitOrder, LimitOrders, MarketRole, MomentOf,
    OrderAmount, OrderBook, OrderBookId, OrderBookStatus, OrderBooks, OrderPrice, OrderVolume,
    Pallet, Payment, PriceOrders,
};

use assets::AssetIdOf;
use codec::Decode;
use common::prelude::{BalanceUnit, FixedWrapper, Scalar};
use common::{balance, AssetInfoProvider, Balance, PriceVariant};
use sp_runtime::traits::{CheckedAdd, CheckedMul, SaturatedConversion, Zero};
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
        .map(crate::test_utils::generate_account::<T>)
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
    use common::PriceVariant;
    use frame_support::assert_ok;
    use frame_support::traits::Hooks;
    use frame_support::weights::Weight;
    use frame_system::RawOrigin;
    use framenode_runtime::order_book::{self, Config, OrderBook, OrderBookId, OrderPrice, Pallet};
    use framenode_runtime::{Runtime, RuntimeOrigin};
    use sp_std::collections::btree_map::BTreeMap;

    pub type E = order_book::Error<Runtime>;
    pub type OrderBookPallet = Pallet<Runtime>;
    pub type DEXId = <Runtime as common::Config>::DEXId;

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
    pub fn create_and_fill_order_book(
        order_book_id: OrderBookId<AssetIdOf<Runtime>, DEXId>,
    ) -> OrderBook<Runtime> {
        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(bob::<Runtime>()).into(),
            order_book_id
        ));

        fill_balance(bob::<Runtime>(), order_book_id);
        fill_balance(charlie::<Runtime>(), order_book_id);

        let lifespan = Some(100000);

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

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(bob::<Runtime>()).into(),
            order_book_id,
            bp1,
            amount1,
            PriceVariant::Buy,
            lifespan
        ));
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(charlie::<Runtime>()).into(),
            order_book_id,
            bp2,
            amount2,
            PriceVariant::Buy,
            lifespan
        ));
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(bob::<Runtime>()).into(),
            order_book_id,
            bp2,
            amount3,
            PriceVariant::Buy,
            lifespan
        ));
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(charlie::<Runtime>()).into(),
            order_book_id,
            bp3,
            amount4,
            PriceVariant::Buy,
            lifespan
        ));
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(bob::<Runtime>()).into(),
            order_book_id,
            bp3,
            amount5,
            PriceVariant::Buy,
            lifespan
        ));
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(charlie::<Runtime>()).into(),
            order_book_id,
            bp3,
            amount6,
            PriceVariant::Buy,
            lifespan
        ));

        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(bob::<Runtime>()).into(),
            order_book_id,
            sp1,
            amount7,
            PriceVariant::Sell,
            lifespan
        ));
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(charlie::<Runtime>()).into(),
            order_book_id,
            sp2,
            amount8,
            PriceVariant::Sell,
            lifespan
        ));
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(bob::<Runtime>()).into(),
            order_book_id,
            sp2,
            amount9,
            PriceVariant::Sell,
            lifespan
        ));
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(charlie::<Runtime>()).into(),
            order_book_id,
            sp3,
            amount10,
            PriceVariant::Sell,
            lifespan
        ));
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(bob::<Runtime>()).into(),
            order_book_id,
            sp3,
            amount11,
            PriceVariant::Sell,
            lifespan
        ));
        assert_ok!(OrderBookPallet::place_limit_order(
            RawOrigin::Signed(charlie::<Runtime>()).into(),
            order_book_id,
            sp3,
            amount12,
            PriceVariant::Sell,
            lifespan
        ));

        // check
        assert_eq!(
            OrderBookPallet::bids(order_book_id, OrderPrice::divisible(bp1)).unwrap(),
            vec![1]
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, OrderPrice::divisible(bp2)).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, OrderPrice::divisible(bp3)).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            OrderBookPallet::asks(order_book_id, OrderPrice::divisible(sp1)).unwrap(),
            vec![7]
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, OrderPrice::divisible(sp2)).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, OrderPrice::divisible(sp3)).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            OrderBookPallet::aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bp1.into(), amount1.into()),
                (bp2.into(), (amount2 + amount3).into()),
                (bp3.into(), (amount4 + amount5 + amount6).into())
            ])
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(&order_book_id),
            BTreeMap::from([
                (sp1.into(), amount7.into()),
                (sp2.into(), (amount8 + amount9).into()),
                (sp3.into(), (amount10 + amount11 + amount12).into())
            ])
        );

        OrderBookPallet::order_books(order_book_id).unwrap()
    }

    pub fn create_empty_order_book(
        order_book_id: OrderBookId<AssetIdOf<Runtime>, DEXId>,
    ) -> OrderBook<Runtime> {
        assert_ok!(OrderBookPallet::create_orderbook(
            RawOrigin::Signed(bob::<Runtime>()).into(),
            order_book_id
        ));

        OrderBookPallet::order_books(order_book_id).unwrap()
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
}
