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
        pub settings: FillSettings<T>,
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
            place_limit_order_without_cross_spread::<T>(settings.clone(), caller.clone());
        let next_order_id = OrderBookPallet::<T>::order_books(order_book_id)
            .unwrap()
            .last_order_id
            + T::OrderId::one();
        Context {
            settings,
            caller,
            order_book_id,
            price,
            amount,
            side,
            lifespan,
            expected_order_id: next_order_id,
        }
    }

    pub fn verify<T: Config + core::fmt::Debug>(init_values: Context<T>) {
        let Context {
            settings,
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
        pub settings: FillSettings<T>,
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
            cancel_limit_order(settings.clone(), caller.clone(), first_expiration);
        let order =
            OrderBookPallet::<T>::limit_orders::<_, T::OrderId>(order_book_id, order_id).unwrap();
        let balance_before = <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
            &order_book_id.quote,
            &order.owner,
        )
        .unwrap();
        Context {
            settings,
            caller,
            order_book_id,
            order_id,
            order,
            balance_before,
        }
    }

    pub fn verify<T: Config + core::fmt::Debug>(context: Context<T>) {
        let Context {
            settings: _,
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
    use common::Balance;

    pub struct Context<T: Config> {
        pub settings: FillSettings<T>,
        pub caller: T::AccountId,
        pub order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        pub amount: BalanceUnit,
        pub direction: PriceVariant,
        pub caller_base_balance: Balance,
        pub caller_quote_balance: Balance,
        pub average_price: OrderPrice,
        pub expected_executed_orders: usize,
    }

    pub(crate) fn init_inner<T: Config + trading_pair::Config>(
        settings: FillSettings<T>,
    ) -> Context<T> {
        let caller = accounts::alice::<T>();
        let is_divisible = false;
        let (order_book_id, info, expected_executed_orders) =
            market_order_execution(settings.clone(), caller.clone(), is_divisible);
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
        Context {
            settings,
            caller,
            order_book_id,
            amount: info.base_amount(),
            direction: info.direction,
            caller_base_balance,
            caller_quote_balance,
            average_price: info.average_price,
            expected_executed_orders,
        }
    }

    pub fn init<T: Config + trading_pair::Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        init_inner(settings)
    }

    pub fn verify<T: Config + core::fmt::Debug>(context: Context<T>) {
        let Context {
            settings,
            caller,
            order_book_id,
            amount,
            direction,
            caller_base_balance,
            caller_quote_balance,
            average_price,
            expected_executed_orders,
        } = context;
        assert_last_event::<T>(
            Event::<T>::MarketOrderExecuted {
                order_book_id,
                owner_id: caller.clone(),
                direction,
                amount: OrderAmount::Base(OrderVolume::indivisible(*amount.balance())),
                average_price,
                to: None,
            }
            .into(),
        );
        assert_orders_numbers::<T>(
            order_book_id,
            Some(settings.max_side_orders() as usize - expected_executed_orders),
            Some(0),
            None,
            None,
        );
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

pub(crate) mod quote {
    use super::*;
    use common::prelude::QuoteAmount;
    use common::Balance;

    pub struct Context<T: Config> {
        pub settings: FillSettings<T>,
        pub dex_id: T::DEXId,
        pub input_asset_id: AssetIdOf<T>,
        pub output_asset_id: AssetIdOf<T>,
        pub amount: QuoteAmount<Balance>,
        pub deduce_fee: bool,
    }

    pub fn init<T: Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let (dex_id, input_asset_id, output_asset_id, amount, deduce_fee) =
            quote::<T>(settings.clone());
        Context {
            settings,
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
    use common::prelude::SwapAmount;
    use common::Balance;

    pub struct Context<T: Config> {
        pub settings: FillSettings<T>,
        pub caller: T::AccountId,
        pub order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        pub input_amount: OrderAmount,
        pub output_amount: OrderAmount,
        pub caller_base_balance: Balance,
        pub caller_quote_balance: Balance,
        pub average_price: OrderPrice,
        pub direction: PriceVariant,
        pub swap_amount: SwapAmount<Balance>,
    }

    pub(crate) fn init_inner<T: Config + trading_pair::Config>(
        settings: FillSettings<T>,
    ) -> Context<T> {
        let caller = accounts::alice::<T>();
        let is_divisible = true;
        let (order_book_id, info, _) =
            market_order_execution(settings.clone(), caller.clone(), is_divisible);
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
        let expected_orders = settings.max_side_orders() as usize;
        let (expected_bids, expected_asks, swap_amount) = match info.direction {
            PriceVariant::Buy => (
                0,
                expected_orders,
                SwapAmount::with_desired_output(
                    *info.base_amount().balance(),
                    *info.quote_amount().balance(),
                ),
            ),
            PriceVariant::Sell => (
                expected_orders,
                0,
                SwapAmount::with_desired_input(
                    *info.base_amount().balance(),
                    *info.quote_amount().balance(),
                ),
            ),
        };
        assert_orders_numbers::<T>(
            order_book_id,
            Some(expected_bids),
            Some(expected_asks),
            None,
            None,
        );
        Context {
            settings,
            caller,
            order_book_id,
            input_amount: info.input_amount,
            output_amount: info.output_amount,
            caller_base_balance,
            caller_quote_balance,
            average_price: info.average_price,
            direction: info.direction,
            swap_amount,
        }
    }

    pub fn init<T: Config + trading_pair::Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        init_inner(settings)
    }

    pub fn verify<T: Config + core::fmt::Debug>(context: Context<T>) {
        let Context {
            settings: _,
            caller,
            order_book_id,
            input_amount,
            output_amount,
            caller_base_balance,
            caller_quote_balance,
            average_price,
            direction,
            swap_amount: _,
        } = context;
        assert_last_event::<T>(
            Event::<T>::MarketOrderExecuted {
                order_book_id,
                owner_id: caller.clone(),
                direction,
                amount: input_amount,
                average_price,
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
            caller_base_balance - input_amount.value().balance()
        );
        assert_eq!(
            <T as order_book_imported::Config>::AssetInfoProvider::free_balance(
                &order_book_id.quote,
                &caller
            )
            .unwrap(),
            caller_quote_balance + output_amount.value().balance()
        );
    }
}

pub(crate) mod align_single_order {
    use super::*;
    use common::balance;

    pub struct Context<T: Config> {
        pub settings: FillSettings<T>,
        pub order_book: OrderBook<T>,
        pub order_to_align: LimitOrder<T>,
    }

    pub fn init<T: Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let (mut order_book, mut order_to_align) =
            align_single_order::<T>(settings.clone(), PriceVariant::Buy);

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
            settings,
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
