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

//! Benchmarking setup for order-book
//!
//! Includes both regular benchmarks for extrinsics, as well as extra ones designed for figuring
//! out appropriate parameters for the order book pallet.
//!
//! The normal benches are run as intended.
//!
//! The extra benchmarks can be conveniently run through script in this directory. Also they are
//! generated with `./generate_benchmarks.py`

#![cfg(feature = "runtime-benchmarks")]
// order-book
#![cfg(feature = "wip")]

// TODO: rename by `order_book` after upgrading to nightly-2023-07-01+
#[cfg(not(test))]
use crate as order_book_imported;
#[cfg(test)]
use framenode_runtime::order_book as order_book_imported;

use assets::AssetIdOf;
use codec::Decode;
use common::{AssetInfoProvider, DEXId, PriceVariant, VAL, XOR};
use frame_support::traits::Time;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use order_book_imported::{
    Config, Event, LimitOrder, MarketRole, MomentOf, OrderAmount, OrderBook, OrderBookId,
    OrderBookStatus, OrderPrice, OrderVolume, Pallet,
};

use crate::test_utils::FillSettings;
use preparation::{
    prepare_cancel_orderbook_benchmark, prepare_market_order_benchmark,
    prepare_place_orderbook_benchmark, prepare_quote_benchmark,
};

use Pallet as OrderBookPallet;

mod preparation;

pub const DEX: DEXId = DEXId::Polkaswap;

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn bob<T: Config>() -> T::AccountId {
    let bytes = hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

fn get_last_order_id<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
) -> Option<<T as Config>::OrderId> {
    if let Some(order_book) = OrderBookPallet::<T>::order_books(order_book_id) {
        Some(order_book.last_order_id)
    } else {
        None
    }
}

/// if `None` then don't compare the value
pub fn assert_orders_numbers<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    bids: Option<usize>,
    asks: Option<usize>,
    user_orders: Option<(T::AccountId, usize)>,
    expirations: Option<(MomentOf<T>, usize)>,
) {
    // # of bids should be max to execute max # of orders and payments
    if let Some(expected_bids) = bids {
        assert_eq!(
            order_book_imported::Bids::<T>::iter_prefix(order_book_id)
                .flat_map(|(_price, orders)| orders.into_iter())
                .count(),
            expected_bids
        );
    }
    if let Some(expected_asks) = asks {
        assert_eq!(
            order_book_imported::Asks::<T>::iter_prefix(order_book_id)
                .flat_map(|(_price, orders)| orders.into_iter())
                .count(),
            expected_asks
        );
    }
    if let Some((user, count)) = user_orders {
        // user orders of `caller` should be almost full
        assert_eq!(
            order_book_imported::UserLimitOrders::<T>::get(user.clone(), order_book_id)
                .unwrap()
                .len(),
            count
        );
    }
    if let Some((lifespan, count)) = expirations {
        // expiration schedule for the block should be almost full
        assert_eq!(
            order_book_imported::ExpirationsAgenda::<T>::get(LimitOrder::<T>::resolve_lifespan(
                frame_system::Pallet::<T>::block_number(),
                lifespan
            ))
            .len(),
            count
        );
    }
}

pub(crate) mod delete_orderbook_benchmark {
    use super::*;

    pub fn init<T: Config>(_settings: FillSettings<T>) -> OrderBookId<AssetIdOf<T>, T::DEXId> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        OrderBookPallet::<T>::create_orderbook(RawOrigin::Signed(bob::<T>()).into(), order_book_id)
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

pub(crate) mod place_limit_order_benchmark {
    use super::*;
    use crate::OrderPrice;
    use common::balance;

    pub struct Context<T: Config> {
        pub caller: T::AccountId,
        pub order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        pub price: OrderPrice,
        pub amount: OrderVolume,
        pub side: PriceVariant,
        pub lifespan: MomentOf<T>,
    }

    pub fn init<T: Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let caller = alice::<T>();
        let (order_book_id, price, amount, side, lifespan) =
            prepare_place_orderbook_benchmark::<T>(settings.clone(), caller.clone());
        Context {
            caller,
            order_book_id,
            price,
            amount,
            side,
            lifespan,
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
            amount: _,
            side,
            lifespan,
        } = init_values;
        let order_id = get_last_order_id::<T>(order_book_id).unwrap();
        assert_orders_numbers::<T>(
            order_book_id,
            Some(0),
            None,
            // 1 order was placed
            Some((
                caller.clone(),
                sp_std::cmp::min(
                    settings.max_orders_per_user,
                    (settings.max_side_price_count - 1) * settings.max_orders_per_price + 1,
                ) as usize,
            )),
            Some((lifespan, settings.max_expiring_orders_per_block as usize)),
        );

        assert_last_event::<T>(
            Event::<T>::LimitOrderIsSplitIntoMarketOrderAndLimitOrder {
                order_book_id,
                owner_id: caller.clone(),
                market_order_direction: side,
                market_order_amount: OrderAmount::Base(balance!(4096).into()),
                market_order_average_price: balance!(0.000325).into(),
                limit_order_id: order_id,
            }
            .into(),
        );

        let current_block = frame_system::Pallet::<T>::block_number();

        let order_book = order_book_imported::OrderBooks::<T>::get(order_book_id).unwrap();
        let now = <<T as Config>::Time as Time>::now();
        let expected_limit_order = LimitOrder::<T>::new(
            order_id,
            caller.clone(),
            PriceVariant::Sell,
            price.into(),
            order_book.min_lot_size.into(),
            now,
            lifespan,
            current_block,
        );
        let mut order = OrderBookPallet::<T>::limit_orders(order_book_id, order_id).unwrap();
        order.original_amount = order.amount;
        assert_eq!(order, expected_limit_order);
    }
}

pub(crate) mod cancel_limit_order_benchmark {
    use common::Balance;
    // use crate::LimitOrder;
    use super::*;

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
        let caller = alice::<T>();
        let (order_book_id, order_id) =
            prepare_cancel_orderbook_benchmark(settings, caller.clone(), first_expiration);
        let order =
            OrderBookPallet::<T>::limit_orders::<_, T::OrderId>(order_book_id, order_id).unwrap();
        let balance_before =
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &order.owner)
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
            }
            .into(),
        );

        let deal_amount = *order.deal_amount(MarketRole::Taker, None).unwrap().value();
        let balance =
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &order.owner)
                .unwrap();
        let expected_balance = balance_before + deal_amount.balance();
        assert_eq!(balance, expected_balance);
    }
}

pub(crate) mod execute_market_order_benchmark {
    use super::*;
    use common::prelude::BalanceUnit;
    use common::Balance;
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

    /// `pairs` consists of pairs `(value, weight)`
    fn weighted_average(pairs: impl IntoIterator<Item = (OrderPrice, OrderVolume)>) -> OrderPrice {
        let mut weight_sum = BalanceUnit::zero();
        let mut weight_times_value_sum = BalanceUnit::zero();
        for (value, weight) in pairs {
            weight_sum += weight;
            weight_times_value_sum += weight * value;
        }
        weight_times_value_sum / weight_sum
    }

    fn expected_average_price<T: Config>(
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        side: PriceVariant,
        is_divisible: bool,
    ) -> OrderPrice {
        let aggregated_side = match side {
            PriceVariant::Buy => OrderBookPallet::<T>::aggregated_asks(order_book_id.clone()),
            PriceVariant::Sell => OrderBookPallet::<T>::aggregated_bids(order_book_id.clone()),
        };
        let mut aggregated_side = aggregated_side.into_iter();
        // account for partial execution
        let (worst_price, worst_price_sum) = match side {
            // for asks it's max price
            PriceVariant::Buy => aggregated_side.next_back().unwrap(),
            // for bids - min price
            PriceVariant::Sell => aggregated_side.next().unwrap(),
        };
        let default_order_book = if is_divisible {
            OrderBook::<T>::default(order_book_id)
        } else {
            OrderBook::<T>::default_indivisible(order_book_id)
        };
        let worst_price_sum = worst_price_sum - default_order_book.min_lot_size;

        weighted_average(aggregated_side.chain(sp_std::iter::once((worst_price, worst_price_sum))))
    }

    pub fn init<T: Config + trading_pair::Config>(settings: FillSettings<T>) -> Context<T> {
        // https://github.com/paritytech/polkadot-sdk/issues/383
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        let caller = alice::<T>();
        let is_divisible = false;
        let (order_book_id, amount, side) =
            prepare_market_order_benchmark(settings, caller.clone(), is_divisible);
        let caller_base_balance =
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap();
        let caller_quote_balance =
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap();
        Context {
            caller,
            order_book_id,
            amount,
            side,
            caller_base_balance,
            caller_quote_balance,
            expected_average_price: expected_average_price::<T>(
                order_book_id.clone(),
                side,
                is_divisible,
            ),
        }
    }

    pub fn verify<T: Config + core::fmt::Debug>(_settings: FillSettings<T>, context: Context<T>) {
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
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap(),
            caller_base_balance - *amount.balance()
        );
        assert_eq!(
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap(),
            caller_quote_balance + *(amount * average_price).balance()
        );
    }
}

pub(crate) mod quote_benchmark {
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
        let (dex_id, input_asset_id, output_asset_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(settings);
        Context {
            dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            deduce_fee,
        }
    }
}

pub(crate) mod exchange_single_order_benchmark {
    use super::*;
    use crate::test_utils::create_and_fill_order_book;
    use common::{balance, Balance, VAL, XOR};

    pub struct Context<T: Config> {
        pub caller: T::AccountId,
        pub order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        pub expected_in: Balance,
        pub expected_out: Balance,
        pub caller_base_balance: Balance,
        pub caller_quote_balance: Balance,
    }

    pub fn init<T: Config + trading_pair::Config>(_settings: FillSettings<T>) -> Context<T> {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::set_block_number(1u32.into());

        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book::<T>(order_book_id);

        assets::Pallet::<T>::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            order_book_id.base,
            balance!(1000000).try_into().unwrap(),
        )
        .unwrap();

        let caller_base_balance =
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap();
        let caller_quote_balance =
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap();
        Context {
            caller,
            order_book_id,
            expected_in: balance!(168.5),
            expected_out: balance!(1685), // this amount executes only one limit order
            caller_base_balance,
            caller_quote_balance,
        }
    }

    pub fn verify<T: Config + core::fmt::Debug>(_settings: FillSettings<T>, context: Context<T>) {
        let Context {
            caller,
            order_book_id,
            expected_in,
            expected_out,
            caller_base_balance,
            caller_quote_balance,
        } = context;

        assert_last_event::<T>(
            Event::<T>::MarketOrderExecuted {
                order_book_id,
                owner_id: caller.clone(),
                direction: PriceVariant::Sell,
                amount: OrderAmount::Base(expected_in.into()),
                average_price: balance!(10).into(),
                to: None,
            }
            .into(),
        );
        assert_eq!(
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap(),
            caller_base_balance - expected_in
        );
        assert_eq!(
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap(),
            caller_quote_balance + expected_out
        );
    }
}

#[cfg(not(test))]
pub use benchmarks_inner::*;

#[cfg(not(test))]
mod benchmarks_inner {
    use common::prelude::SwapAmount;
    use common::{balance, AssetInfoProvider, AssetName, AssetSymbol, LiquiditySource, VAL, XOR};
    use frame_benchmarking::benchmarks;
    use frame_support::weights::WeightMeter;
    use frame_system::RawOrigin;
    use sp_runtime::traits::UniqueSaturatedInto;

    use super::*;
    use crate::cache_data_layer::CacheDataLayer;
    use crate::{
        Config, Event, ExpirationScheduler, MarketRole, OrderBook, OrderBookId, OrderBookStatus,
        Pallet,
    };
    use preparation::{
        create_and_populate_order_book, prepare_cancel_orderbook_benchmark,
        prepare_market_order_benchmark, prepare_place_orderbook_benchmark, prepare_quote_benchmark,
        presets::*,
    };

    use frame_system::Pallet as FrameSystem;
    use trading_pair::Pallet as TradingPair;

    benchmarks! {
        where_clause {
            where T: trading_pair::Config + core::fmt::Debug
        }

        create_orderbook {
            let caller = alice::<T>();
            FrameSystem::<T>::inc_providers(&caller);

            let nft = assets::Pallet::<T>::register_from(
                &caller,
                AssetSymbol(b"NFT".to_vec()),
                AssetName(b"Nft".to_vec()),
                0,
                balance!(1),
                false,
                None,
                None,
            )
            .unwrap();

            let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
                dex_id: DEX.into(),
                base: nft,
                quote: XOR.into(),
            };

            TradingPair::<T>::register(
                RawOrigin::Signed(caller.clone()).into(),
                DEX.into(),
                order_book_id.quote,
                order_book_id.base
            ).unwrap();
        }: {
            OrderBookPallet::<T>::create_orderbook(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id
            ).unwrap();
        }
        verify {
            assert_last_event::<T>(
                Event::<T>::OrderBookCreated {
                    order_book_id,
                    creator: caller,
                }
                .into(),
            );

            assert_eq!(
                OrderBookPallet::<T>::order_books(order_book_id).unwrap(),
                OrderBook::<T>::default_indivisible(order_book_id)
            );
        }

        delete_orderbook {
            let settings = preset_14::<T>();
            let order_book_id = delete_orderbook_benchmark::init(settings.clone());
        }: {
            OrderBookPallet::<T>::delete_orderbook(
                RawOrigin::Root.into(),
                order_book_id
            ).unwrap();
        }
        verify {
            delete_orderbook_benchmark::verify(settings, order_book_id);
        }

        update_orderbook {
            let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
                dex_id: DEX.into(),
                base: VAL.into(),
                quote: XOR.into(),
            };

            create_and_populate_order_book::<T>(order_book_id);

            let tick_size = balance!(0.01);
            let step_lot_size = balance!(1); // limit orders should be aligned according to new step_lot_size
            let min_lot_size = balance!(10);
            let max_lot_size = balance!(2000);

            OrderBookPallet::<T>::change_orderbook_status(RawOrigin::Root.into(), order_book_id, OrderBookStatus::Stop).unwrap();
        }: {
            OrderBookPallet::<T>::update_orderbook(
                RawOrigin::Root.into(),
                order_book_id,
                tick_size,
                step_lot_size,
                min_lot_size,
                max_lot_size
            ).unwrap();
        }
        verify {
            assert_last_event::<T>(
                Event::<T>::OrderBookUpdated {
                    order_book_id,
                }
                .into(),
            );

            let order_book = OrderBookPallet::<T>::order_books(order_book_id).unwrap();
            assert_eq!(order_book.tick_size, tick_size.into());
            assert_eq!(order_book.step_lot_size, step_lot_size.into());
            assert_eq!(order_book.min_lot_size, min_lot_size.into());
            assert_eq!(order_book.max_lot_size, max_lot_size.into());
        }

        change_orderbook_status {
            let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
                dex_id: DEX.into(),
                base: VAL.into(),
                quote: XOR.into(),
            };

            create_and_populate_order_book::<T>(order_book_id);
        }: {
            OrderBookPallet::<T>::change_orderbook_status(
                RawOrigin::Root.into(),
                order_book_id,
                OrderBookStatus::Stop
            ).unwrap();
        }
        verify {
            assert_last_event::<T>(
                Event::<T>::OrderBookStatusChanged {
                    order_book_id,
                    new_status: OrderBookStatus::Stop,
                }
                .into(),
            );

            assert_eq!(OrderBookPallet::<T>::order_books(order_book_id).unwrap().status, OrderBookStatus::Stop);
        }

        place_limit_order {
            let settings = preset_14::<T>();
            let context = place_limit_order_benchmark::init(settings.clone());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id,
                *context.price.balance(),
                *context.amount.balance(),
                context.side,
                Some(context.lifespan)
            ).unwrap();
        }
        verify {
            place_limit_order_benchmark::verify(settings, context);
        }

        cancel_limit_order_first_expiration {
            let settings = preset_14::<T>();
            let context = cancel_limit_order_benchmark::init(settings.clone(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id.clone(),
                context.order_id.clone()
            ).unwrap();
        }
        verify {
            cancel_limit_order_benchmark::verify(settings, context);
        }

        cancel_limit_order_last_expiration {
            let settings = preset_14::<T>();
            let context = cancel_limit_order_benchmark::init(settings.clone(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id.clone(),
                context.order_id.clone()
            ).unwrap();
        }
        verify {
            cancel_limit_order_benchmark::verify(settings, context);
        }

        execute_market_order {
            let settings = preset_14::<T>();
            let context = execute_market_order_benchmark::init(settings.clone());
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id,
                context.side,
                *context.amount.balance()
            ).unwrap();
        }
        verify {
            execute_market_order_benchmark::verify(settings, context);
        }

        quote {
            let settings = preset_14::<T>();
            let context = quote_benchmark::init(settings.clone());
        }: {
            OrderBookPallet::<T>::quote(
                &context.dex_id,
                &context.input_asset_id,
                &context.output_asset_id,
                context.amount,
                context.deduce_fee,
            )
            .unwrap();
        }
        verify {
            // nothing changed
        }

        exchange_single_order {
            let settings = preset_14::<T>();
            let context = exchange_single_order_benchmark::init(settings.clone());
        }: {
            OrderBookPallet::<T>::exchange(
                &context.caller,
                &context.caller,
                &context.order_book_id.dex_id,
                &context.order_book_id.base,
                &context.order_book_id.quote,
                SwapAmount::with_desired_output(
                    context.expected_out, context.expected_in + balance!(1.5)
                ),
            )
            .unwrap();
        }
        verify {
            exchange_single_order_benchmark::verify(settings, context);
        }

        service_base {
            let mut weight = WeightMeter::max_limit();
            let block_number = 0u32.unique_saturated_into();
        }: {
            OrderBookPallet::<T>::service(block_number, &mut weight);
        }
        verify {}

        service_block_base {
            let mut weight = WeightMeter::max_limit();
            let block_number = 0u32.unique_saturated_into();
            // should be the slower layer because cache is not
            // warmed up
            let mut data_layer = CacheDataLayer::<T>::new();
        }: {
            OrderBookPallet::<T>::service_block(&mut data_layer, block_number, &mut weight);
        }
        verify {}

        // TODO: benchmark worst case
        service_single_expiration {
            // very similar to cancel_limit_order
            let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
                dex_id: DEX.into(),
                base: VAL.into(),
                quote: XOR.into(),
            };

            create_and_populate_order_book::<T>(order_book_id);

            let order_id = 5u128.unique_saturated_into();

            let order = OrderBookPallet::<T>::limit_orders(order_book_id, order_id).unwrap();

            let balance_before =
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &order.owner).unwrap();

            // should be the slower layer because cache is not warmed up
            let mut data_layer = CacheDataLayer::<T>::new();
        }: {
            OrderBookPallet::<T>::service_single_expiration(&mut data_layer, &order_book_id, order_id);
        }
        verify {
            assert_last_event::<T>(
                Event::<T>::LimitOrderExpired {
                    order_book_id,
                    order_id,
                    owner_id: order.owner.clone(),
                }
                .into(),
            );

            let deal_amount = *order.deal_amount(MarketRole::Taker, None).unwrap().value();
            let balance =
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &order.owner).unwrap();
            let expected_balance = balance_before + deal_amount.balance();
            assert_eq!(balance, expected_balance);
        }


        // now it works only as benchmarks, not as unit tests
        // TODO fix when new approach be developed
        // impl_benchmark_test_suite!(Pallet, framenode_chain_spec::ext(), framenode_runtime::Runtime);

        // attributes benchmarks

        // macros are executed outside-in, therefore implementing such codegen within Rust requires
        // modifying `benchmarks!` macro from substrate (which is quite challenging); so
        // python-codegen approach is chosen (:

        // the workflow is the following:
        // 1. edit presets in ./preparation.rs (with names "preset_*" where * is 1,2,3,4,5,...)
        // 2. in ./generate_benchmarks.py set `max_preset` to the highest preset number
        // 3. run ./generate_benchmarks.py
        // 4. paste output here (instead of existing benches)
        // 5. build as usual (with `--release` flag)
        // 6. run with ./benchmark_attributes.sh

        #[extra]
        place_limit_order_1 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_1(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_2 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_2(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_3 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_3(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_4 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_4(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_5 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_5(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_6 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_6(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_7 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_7(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_8 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_8(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_9 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_9(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_10 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_10(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_11 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_11(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_12 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_12(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_13 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_13(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_14 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_14(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_15 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_15(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_16 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_16(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }


        #[extra]
        cancel_limit_order_first_1 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_1::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_2 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_2::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_3 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_3::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_4 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_4::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_5 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_5::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_6 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_6::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_7 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_7::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_8 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_8::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_9 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_9::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_10 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_10::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_11 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_11::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_12 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_12::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_13 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_13::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_14 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_14::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_15 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_15::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_16 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_16::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }


        #[extra]
        cancel_limit_order_last_1 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_1::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_2 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_2::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_3 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_3::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_4 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_4::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_5 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_5::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_6 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_6::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_7 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_7::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_8 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_8::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_9 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_9::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_10 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_10::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_11 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_11::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_12 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_12::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_13 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_13::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_14 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_14::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_15 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_15::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_16 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_16::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }


        #[extra]
        execute_market_order_1 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_1(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_2 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_2(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_3 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_3(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_4 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_4(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_5 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_5(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_6 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_6(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_7 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_7(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_8 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_8(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_9 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_9(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_10 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_10(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_11 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_11(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_12 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_12(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_13 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_13(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_14 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_14(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_15 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_15(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_16 {
            let caller = alice::<T>();
            let (id, amount, side) = prepare_market_order_benchmark::<T>(preset_16(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }


        #[extra]
        quote_1 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_1());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_2 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_2());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_3 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_3());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_4 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_4());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_5 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_5());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_6 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_6());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_7 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_7());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_8 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_8());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_9 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_9());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_10 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_10());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_11 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_11());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_12 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_12());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_13 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_13());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_14 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_14());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_15 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_15());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_16 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_16());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }


        #[extra]
        exchange_1 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_1(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_2 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_2(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_3 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_3(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_4 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_4(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_5 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_5(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_6 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_6(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_7 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_7(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_8 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_8(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_9 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_9(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_10 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_10(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_11 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_11(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_12 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_12(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_13 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_13(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_14 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_14(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_15 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_15(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_16 {
            let caller = alice::<T>();
            let (id, amount, _) = prepare_market_order_benchmark::<T>(preset_16(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
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
            let order_book_id = delete_orderbook_benchmark::init(settings.clone());

            OrderBookPallet::<Runtime>::delete_orderbook(RawOrigin::Root.into(), order_book_id)
                .unwrap();

            delete_orderbook_benchmark::verify(settings, order_book_id);
        })
    }

    #[test]
    fn test_benchmark_place() {
        ext().execute_with(|| {
            let settings = preset_14::<Runtime>();
            let context = place_limit_order_benchmark::init(settings.clone());

            OrderBookPallet::<Runtime>::place_limit_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id,
                *context.price.balance(),
                *context.amount.balance(),
                context.side,
                Some(context.lifespan),
            )
            .unwrap();

            place_limit_order_benchmark::verify(settings, context);
        })
    }

    #[test]
    fn test_benchmark_cancel() {
        ext().execute_with(|| {
            let settings = preset_14::<Runtime>();
            let context = cancel_limit_order_benchmark::init(settings.clone(), false);

            OrderBookPallet::<Runtime>::cancel_limit_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id.clone(),
                context.order_id.clone(),
            )
            .unwrap();

            cancel_limit_order_benchmark::verify(settings, context);
        })
    }

    #[test]
    fn test_benchmark_execute_market_order() {
        ext().execute_with(|| {
            let settings = preset_14::<Runtime>();
            let context = execute_market_order_benchmark::init(settings.clone());
            OrderBookPallet::<Runtime>::execute_market_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id,
                context.side,
                *context.amount.balance(),
            )
            .unwrap();
            execute_market_order_benchmark::verify(settings, context);
        })
    }

    #[test]
    fn test_benchmark_quote() {
        ext().execute_with(|| {
            use common::LiquiditySource;

            let settings = preset_14::<Runtime>();
            let context = quote_benchmark::init(settings.clone());
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
            let context = exchange_single_order_benchmark::init(settings.clone());
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
            exchange_single_order_benchmark::verify(settings, context);
        })
    }
}
