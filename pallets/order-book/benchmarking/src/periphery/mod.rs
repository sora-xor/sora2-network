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

//! Benchmark periphery: initialization code (run before), post-conditions checks, and running
//! context.
//!
//! Separated for each benchmark.

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
use common::{AssetInfoProvider, PriceVariant, VAL, XOR};
use frame_system::RawOrigin;
use order_book_benchmarking_imported::Config;
use order_book_imported::Pallet as OrderBookPallet;
use order_book_imported::{
    test_utils::{accounts, fill_tools::FillSettings},
    CancelReason, Event, LimitOrder, LimitOrders, MarketRole, MomentOf, OrderAmount, OrderBook,
    OrderBookId, OrderBookStatus, OrderBooks, OrderPrice, OrderVolume,
};

use crate::{assert_last_event, assert_orders_numbers, DEX};
use preparation::{
    align_single_order, cancel_limit_order, market_order_execution,
    place_limit_order_without_cross_spread, quote,
};

mod preparation;

pub use preparation::presets;

pub(crate) mod delete_orderbook {
    use super::*;
    use common::balance;

    pub fn init<T: Config>(_settings: FillSettings<T>) -> OrderBookId<AssetIdOf<T>, T::DEXId> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
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
        .unwrap();
        OrderBookPallet::<T>::change_orderbook_status(
            RawOrigin::Root.into(),
            order_book_id,
            OrderBookStatus::Stop,
        )
        .unwrap();
        order_book_id
    }

    pub fn verify<T: Config + core::fmt::Debug>(
        _settings: FillSettings<T>,
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    ) {
        assert_last_event::<T>(Event::<T>::OrderBookDeleted { order_book_id }.into());
        assert_eq!(OrderBookPallet::<T>::order_books(order_book_id), None);
    }
}

pub(crate) mod place_limit_order {
    use super::*;
    use order_book_imported::OrderPrice;
    use sp_runtime::traits::One;

    pub struct Context<T: Config> {
        pub caller: T::AccountId,
        pub order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        pub price: OrderPrice,
        pub amount: OrderVolume,
        pub side: PriceVariant,
        pub lifespan: MomentOf<T>,
        pub expected_order_id: T::OrderId,
    }

    pub fn init<T: Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let caller = accounts::alice::<T>();
        let (order_book_id, price, amount, side, lifespan) =
            place_limit_order_without_cross_spread::<T>(settings, caller.clone());
        let next_order_id = OrderBookPallet::<T>::order_books(order_book_id)
            .unwrap()
            .last_order_id
            + T::OrderId::one();
        Context {
            caller,
            order_book_id,
            price,
            amount,
            side,
            lifespan,
            expected_order_id: next_order_id,
        }
    }

    pub fn verify<T: Config + core::fmt::Debug>(
        settings: FillSettings<T>,
        init_values: Context<T>,
    ) {
        let Context {
            caller,
            order_book_id,
            price,
            amount,
            side,
            lifespan,
            expected_order_id,
        } = init_values;
        let expected_bids = sp_std::cmp::min(
            settings.max_orders_per_user - 1,
            settings.max_side_price_count * settings.max_orders_per_price,
        ) as usize;
        let expected_user_orders = sp_std::cmp::min(
            settings.max_orders_per_user,
            settings.max_side_price_count * settings.max_orders_per_price,
        ) as usize;
        assert_orders_numbers::<T>(
            order_book_id,
            Some(expected_bids),
            Some(settings.max_orders_per_price as usize),
            Some((caller.clone(), expected_user_orders)),
            Some((lifespan, settings.max_expiring_orders_per_block as usize)),
        );

        assert_last_event::<T>(
            Event::<T>::LimitOrderPlaced {
                order_book_id,
                order_id: expected_order_id,
                owner_id: caller,
                side,
                price,
                amount,
                lifetime: lifespan,
            }
            .into(),
        );
    }
}

pub(crate) mod cancel_limit_order {
    use super::*;
    use common::Balance;

    pub struct Context<T: Config> {
        pub caller: T::AccountId,
        pub order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        pub order_id: T::OrderId,
        pub order: LimitOrder<T>,
        pub balance_before: Balance,
    }

    pub fn init<T: Config>(settings: FillSettings<T>, first_expiration: bool) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let caller = accounts::alice::<T>();
        let (order_book_id, order_id) =
            cancel_limit_order(settings, caller.clone(), first_expiration);
        let order =
            OrderBookPallet::<T>::limit_orders::<_, T::OrderId>(order_book_id, order_id).unwrap();
        let balance_before = <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
            &order_book_id.quote,
            &order.owner,
        )
        .unwrap();
        Context {
            caller,
            order_book_id,
            order_id,
            order,
            balance_before,
        }
    }

    pub fn verify<T: Config + core::fmt::Debug>(_settings: FillSettings<T>, context: Context<T>) {
        let Context {
            caller: _,
            order_book_id,
            order_id,
            order,
            balance_before,
        } = context;
        assert_last_event::<T>(
            Event::<T>::LimitOrderCanceled {
                order_book_id,
                order_id,
                owner_id: order.owner.clone(),
                reason: CancelReason::Manual,
            }
            .into(),
        );

        let deal_amount = *order.deal_amount(MarketRole::Taker, None).unwrap().value();
        let balance = <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
            &order_book_id.quote,
            &order.owner,
        )
        .unwrap();
        let expected_balance = balance_before + deal_amount.balance();
        assert_eq!(balance, expected_balance);
    }
}

pub(crate) mod execute_market_order {
    use super::*;
    use common::prelude::BalanceUnit;
    use common::{balance, Balance};
    use sp_runtime::traits::Zero;

    pub struct Context<T: Config> {
        pub caller: T::AccountId,
        pub order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        pub amount: BalanceUnit,
        pub side: PriceVariant,
        pub caller_base_balance: Balance,
        pub caller_quote_balance: Balance,
        pub expected_average_price: OrderPrice,
    }

    /// returns `(expected_base, expected_quote)` if executing all orders in `side`
    pub(crate) fn expected_side_total<T: Config>(
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        side: PriceVariant,
        is_divisible: bool,
    ) -> (OrderVolume, OrderVolume) {
        let aggregated_side = match side {
            PriceVariant::Buy => OrderBookPallet::<T>::aggregated_asks(order_book_id),
            PriceVariant::Sell => OrderBookPallet::<T>::aggregated_bids(order_book_id),
        };
        let mut aggregated_side = aggregated_side.into_iter();
        // account for partial execution
        let (worst_price, worst_price_sum) = match side {
            // for asks it's max price
            PriceVariant::Buy => aggregated_side.next_back().unwrap(),
            // for bids - min price
            PriceVariant::Sell => aggregated_side.next().unwrap(),
        };
        let min_lot_size = if is_divisible {
            OrderVolume::divisible(balance!(1))
        } else {
            OrderVolume::indivisible(1)
        };
        let worst_price_sum = worst_price_sum - min_lot_size;
        let aggregated_side =
            aggregated_side.chain(sp_std::iter::once((worst_price, worst_price_sum)));

        let bases_quotes = aggregated_side.map(|(price, volume)| (volume, volume * price));
        bases_quotes.fold((OrderVolume::zero(), OrderVolume::zero()), |acc, next| {
            (acc.0 + next.0, acc.1 + next.1)
        })
    }

    pub(crate) fn init_inner<T: Config + trading_pair::Config>(
        settings: FillSettings<T>,
        scatter: bool,
    ) -> Context<T> {
        let caller = accounts::alice::<T>();
        let is_divisible = false;
        let (order_book_id, amount, side) =
            market_order_execution(settings, caller.clone(), is_divisible, scatter);
        let caller_base_balance =
            <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
                &order_book_id.base,
                &caller,
            )
            .unwrap();
        let caller_quote_balance =
            <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
                &order_book_id.quote,
                &caller,
            )
            .unwrap();
        let (expected_base, expected_quote) =
            expected_side_total::<T>(order_book_id, side, is_divisible);
        assert_eq!(amount, expected_base);
        Context {
            caller,
            order_book_id,
            amount,
            side,
            caller_base_balance,
            caller_quote_balance,
            expected_average_price: expected_quote / expected_base,
        }
    }

    pub fn init<T: Config + trading_pair::Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        init_inner(settings, false)
    }

    pub fn verify<T: Config + core::fmt::Debug>(context: Context<T>) {
        let Context {
            caller,
            order_book_id,
            amount,
            side,
            caller_base_balance,
            caller_quote_balance,
            expected_average_price,
        } = context;
        let average_price = expected_average_price;
        assert_last_event::<T>(
            Event::<T>::MarketOrderExecuted {
                order_book_id,
                owner_id: caller.clone(),
                direction: side,
                amount: OrderAmount::Base(OrderVolume::indivisible(*amount.balance())),
                average_price,
                to: None,
            }
            .into(),
        );
        assert_orders_numbers::<T>(order_book_id, Some(1), Some(0), None, None);
        assert_eq!(
            <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
                &order_book_id.base,
                &caller
            )
            .unwrap(),
            caller_base_balance - *amount.balance()
        );
        assert_eq!(
            <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
                &order_book_id.quote,
                &caller
            )
            .unwrap(),
            caller_quote_balance + *(amount * average_price).balance()
        );
    }
}

pub(crate) mod execute_market_order_scattered {
    //! Same as `execute_market_order` benchmark but with orders evenly spread across
    //! the order book.
    //!
    //! This might be slower because of working with storages aggregated by price.
    //!
    //! Update: it is indeed worse than just `mod execute_market_order` benchmark

    use super::*;

    pub fn init<T: Config + trading_pair::Config>(
        settings: FillSettings<T>,
    ) -> execute_market_order::Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());

        let context = execute_market_order::init_inner(settings.clone(), true);
        let aggregated_side_executed = match context.side.switched() {
            PriceVariant::Buy => {
                order_book_imported::Pallet::<T>::aggregated_bids(context.order_book_id)
            }
            PriceVariant::Sell => {
                order_book_imported::Pallet::<T>::aggregated_asks(context.order_book_id)
            }
        };
        assert_eq!(
            aggregated_side_executed.len(),
            sp_std::cmp::min(
                settings.max_side_price_count,
                settings.executed_orders_limit
            ) as usize
        );
        context
    }

    pub fn verify<T: Config + core::fmt::Debug>(context: execute_market_order::Context<T>) {
        execute_market_order::verify(context)
    }
}

pub(crate) mod quote {
    use super::*;
    use common::prelude::QuoteAmount;
    use common::Balance;

    pub struct Context<T: Config> {
        pub dex_id: T::DEXId,
        pub input_asset_id: AssetIdOf<T>,
        pub output_asset_id: AssetIdOf<T>,
        pub amount: QuoteAmount<Balance>,
        pub deduce_fee: bool,
    }

    pub fn init<T: Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let (dex_id, input_asset_id, output_asset_id, amount, deduce_fee) = quote::<T>(settings);
        Context {
            dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            deduce_fee,
        }
    }
}

pub(crate) mod exchange {

    use super::*;
    use common::Balance;

    pub struct Context<T: Config> {
        pub caller: T::AccountId,
        pub order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        pub expected_in: Balance,
        pub expected_out: Balance,
        pub caller_base_balance: Balance,
        pub caller_quote_balance: Balance,
        pub expected_average_price: OrderPrice,
        pub side: PriceVariant,
    }

    pub(crate) fn init_inner<T: Config + trading_pair::Config>(
        settings: FillSettings<T>,
        scatter: bool,
    ) -> Context<T> {
        let caller = accounts::alice::<T>();
        let is_divisible = true;
        let (order_book_id, amount, side) =
            market_order_execution(settings.clone(), caller.clone(), is_divisible, scatter);
        let caller_base_balance =
            <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
                &order_book_id.base,
                &caller,
            )
            .unwrap();
        let caller_quote_balance =
            <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
                &order_book_id.quote,
                &caller,
            )
            .unwrap();
        let (expected_base, expected_quote) =
            execute_market_order::expected_side_total::<T>(order_book_id, side, is_divisible);
        assert_eq!(amount, expected_base);
        let expected_orders = sp_std::cmp::min(
            settings.executed_orders_limit,
            settings.max_side_price_count * settings.max_orders_per_price,
        ) as usize;
        let (expected_bids, expected_asks) = match side {
            PriceVariant::Buy => (0, expected_orders),
            PriceVariant::Sell => (expected_orders, 0),
        };
        assert_orders_numbers::<T>(
            order_book_id,
            Some(expected_bids),
            Some(expected_asks),
            None,
            None,
        );
        let expected_average_price = expected_quote / expected_base;
        let (expected_in, expected_out) = match side {
            PriceVariant::Buy => (expected_quote, expected_base),
            PriceVariant::Sell => (expected_base, expected_quote),
        };
        let (expected_in, expected_out) = (*expected_in.balance(), *expected_out.balance());
        Context {
            caller,
            order_book_id,
            expected_in,
            expected_out,
            caller_base_balance,
            caller_quote_balance,
            expected_average_price,
            side,
        }
    }

    #[allow(unused)]
    pub fn init<T: Config + trading_pair::Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        init_inner(settings, false)
    }

    pub fn verify<T: Config + core::fmt::Debug>(context: Context<T>) {
        let Context {
            caller,
            order_book_id,
            expected_in,
            expected_out,
            caller_base_balance,
            caller_quote_balance,
            expected_average_price,
            side,
        } = context;
        assert_last_event::<T>(
            Event::<T>::MarketOrderExecuted {
                order_book_id,
                owner_id: caller.clone(),
                direction: side,
                amount: OrderAmount::Base(expected_in.into()),
                average_price: expected_average_price,
                to: None,
            }
            .into(),
        );
        assert_eq!(
            <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
                &order_book_id.base,
                &caller
            )
            .unwrap(),
            caller_base_balance - expected_in
        );
        assert_eq!(
            <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
                &order_book_id.quote,
                &caller
            )
            .unwrap(),
            caller_quote_balance + expected_out
        );
    }
}

pub(crate) mod exchange_scattered {
    //! Same as `exchange` benchmark but with orders (more or less) evenly scattered across
    //! the order book.
    //!
    //! This might be slower because of working with storages aggregated by price.
    //!
    //! Update: it is indeed worse than just `mod exchange` benchmark

    use super::*;
    pub use exchange::Context;

    pub fn init<T: Config + trading_pair::Config>(
        settings: FillSettings<T>,
    ) -> exchange::Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let context = exchange::init_inner(settings.clone(), true);
        let aggregated_side_executed = match context.side.switched() {
            PriceVariant::Buy => {
                order_book_imported::Pallet::<T>::aggregated_bids(context.order_book_id)
            }
            PriceVariant::Sell => {
                order_book_imported::Pallet::<T>::aggregated_asks(context.order_book_id)
            }
        };
        assert_eq!(
            aggregated_side_executed.len(),
            sp_std::cmp::min(
                settings.max_side_price_count,
                settings.executed_orders_limit
            ) as usize
        );
        context
    }

    pub fn verify<T: Config + core::fmt::Debug>(context: exchange::Context<T>) {
        exchange::verify(context)
    }
}

pub(crate) mod align_single_order {
    use super::*;
    use common::balance;

    pub struct Context<T: Config> {
        pub order_book: OrderBook<T>,
        pub order_to_align: LimitOrder<T>,
    }

    pub fn init<T: Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let (mut order_book, mut order_to_align) =
            align_single_order::<T>(settings, PriceVariant::Buy);

        let old_step_lot_size = *order_book.step_lot_size.balance();

        // update step lot size
        order_book.step_lot_size.set(balance!(1));
        <OrderBooks<T>>::insert(order_book.order_book_id, order_book.clone());

        // update order amount to be aligned
        order_to_align
            .amount
            .set(*order_to_align.amount.balance() + old_step_lot_size);
        <LimitOrders<T>>::set(
            order_book.order_book_id,
            order_to_align.id,
            Some(order_to_align.clone()),
        );

        Context {
            order_book,
            order_to_align,
        }
    }

    pub fn verify<T: Config + core::fmt::Debug>(context: Context<T>) {
        let aligned_order =
            <LimitOrders<T>>::get(context.order_book.order_book_id, context.order_to_align.id)
                .unwrap();

        assert_last_event::<T>(
            Event::<T>::LimitOrderUpdated {
                order_book_id: context.order_book.order_book_id,
                order_id: aligned_order.id,
                owner_id: aligned_order.owner,
                new_amount: aligned_order.amount,
            }
            .into(),
        );

        assert!(
            *context.order_to_align.amount.balance() % *context.order_book.step_lot_size.balance()
                != 0
        );
        assert!(*aligned_order.amount.balance() % *context.order_book.step_lot_size.balance() == 0);
    }
}
