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
// #![cfg(feature = "ready-to-test")]

#[allow(unused)]
#[cfg(not(test))]
use crate::{
    self as order_book, cache_data_layer::CacheDataLayer, traits::DataLayer, Config, Event,
    ExpirationScheduler, LimitOrder, MarketRole, MomentOf, OrderAmount, OrderBook, OrderBookId,
    OrderBookStatus, OrderBooks, OrderVolume, Pallet,
};
#[allow(unused)]
#[cfg(test)]
use framenode_runtime::order_book::{
    self as order_book, cache_data_layer::CacheDataLayer, traits::DataLayer, Config, Event,
    ExpirationScheduler, LimitOrder, MarketRole, MomentOf, OrderAmount, OrderBook, OrderBookId,
    OrderBookStatus, OrderBooks, OrderVolume, Pallet,
};

use assets::AssetIdOf;
use codec::Decode;
use common::{DEXId, VAL, XOR};
use frame_system::EventRecord;
use hex_literal::hex;

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
            order_book::Bids::<T>::iter_prefix(order_book_id)
                .flat_map(|(_price, orders)| orders.into_iter())
                .count(),
            expected_bids
        );
    }
    if let Some(expected_asks) = asks {
        assert_eq!(
            order_book::Asks::<T>::iter_prefix(order_book_id)
                .flat_map(|(_price, orders)| orders.into_iter())
                .count(),
            expected_asks
        );
    }
    if let Some((user, count)) = user_orders {
        // user orders of `caller` should be almost full
        assert_eq!(
            order_book::UserLimitOrders::<T>::get(user.clone(), order_book_id)
                .unwrap()
                .len(),
            count
        );
    }
    if let Some((lifespan, count)) = expirations {
        // expiration schedule for the block should be almost full
        assert_eq!(
            order_book::ExpirationsAgenda::<T>::get(LimitOrder::<T>::resolve_lifespan(
                frame_system::Pallet::<T>::block_number(),
                lifespan
            ))
            .len(),
            count
        );
    }
}

#[cfg(not(test))]
pub use benchmarks_inner::*;
#[cfg(not(test))]
mod benchmarks_inner {
    use common::prelude::SwapAmount;
    use common::{
        balance, AssetInfoProvider, AssetName, AssetSymbol, LiquiditySource, PriceVariant,
    };
    use frame_benchmarking::benchmarks;
    use frame_support::traits::{Get, Time};
    use frame_support::weights::WeightMeter;
    use frame_system::RawOrigin;
    use sp_runtime::traits::UniqueSaturatedInto;

    use super::*;
    use crate::{
        self as order_book, cache_data_layer::CacheDataLayer, Config, Event, ExpirationScheduler,
        LimitOrder, MarketRole, OrderAmount, OrderBook, OrderBookId, OrderBookStatus, Pallet,
    };
    use preparation::{
        create_and_populate_order_book, prepare_cancel_orderbook_benchmark,
        prepare_delete_orderbook_benchmark, prepare_market_order_benchmark,
        prepare_place_orderbook_benchmark, prepare_quote_benchmark, presets::*, FillSettings,
    };

    use assets::Pallet as Assets;
    use frame_system::Pallet as FrameSystem;
    use trading_pair::Pallet as TradingPair;

    benchmarks! {
        where_clause {
            where T: trading_pair::Config + core::fmt::Debug
        }

        create_orderbook {
            let caller = alice::<T>();
            FrameSystem::<T>::inc_providers(&caller);

            let nft = Assets::<T>::register_from(
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

        #[extra]
        delete_orderbook {
            let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_1::<T>());
        }: {
            OrderBookPallet::<T>::delete_orderbook(
                RawOrigin::Root.into(),
                order_book_id
            ).unwrap();
        }
        verify {
            assert_last_event::<T>(
                Event::<T>::OrderBookDeleted {
                    order_book_id,
                    count_of_canceled_orders:
                        <T as Config>::MaxSidePriceCount::get()
                        * <T as Config>::MaxLimitOrdersForPrice::get() * 2,
                }
                .into(),
            );
            assert_eq!(OrderBookPallet::<T>::order_books(order_book_id), None);
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
            let min_lot_size = balance!(1);
            let max_lot_size = balance!(10000);
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

        #[extra]
        place_limit_order {
            let caller = alice::<T>();
            let settings = FillSettings::new(
                <T as Config>::MaxSidePriceCount::get(),
                <T as Config>::MaxLimitOrdersForPrice::get(),
                <T as Config>::MaxOpenedLimitOrdersPerUser::get(),
                <T as Config>::MaxExpiringOrdersPerBlock::get()
            );
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(settings.clone(), caller.clone());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id,
                price,
                amount,
                side,
                Some(lifespan)
            ).unwrap();
        }
        verify {
            let order_id = get_last_order_id::<T>(order_book_id).unwrap();

            assert_orders_numbers::<T>(
                order_book_id,
                Some(0),
                None,
                Some((caller.clone(), settings.max_orders_per_user as usize)),
                Some((lifespan, settings.max_expiring_orders_per_block as usize)),
            );

            assert_last_event::<T>(
                Event::<T>::LimitOrderPlaced {
                    order_book_id,
                    order_id,
                    owner_id: caller.clone(),
                }
                .into(),
            );

            let current_block = frame_system::Pallet::<T>::block_number();

            let order_book = order_book::OrderBooks::<T>::get(order_book_id).unwrap();
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

        #[extra]
        cancel_limit_order_first_expiration {
            let caller = alice::<T>();
            let settings = FillSettings::<T>::new(
                <T as Config>::MaxSidePriceCount::get(),
                <T as Config>::MaxLimitOrdersForPrice::get(),
                <T as Config>::MaxOpenedLimitOrdersPerUser::get(),
                <T as Config>::MaxExpiringOrdersPerBlock::get()
            );
            let (order_book_id, order_id) = prepare_cancel_orderbook_benchmark(settings, caller.clone(), true);
            let order = OrderBookPallet::<T>::limit_orders::<_, T::OrderId>(order_book_id, order_id).unwrap();
            let balance_before =
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &order.owner).unwrap();
        }: {
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id.clone(),
                order_id.clone()
            ).unwrap();
        }
        verify {
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
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &order.owner).unwrap();
            let expected_balance = balance_before + deal_amount.balance();
            assert_eq!(balance, expected_balance);
        }

        #[extra]
        cancel_limit_order_last_expiration {
            let caller = alice::<T>();
            let settings = FillSettings::<T>::new(
                <T as Config>::MaxSidePriceCount::get(),
                <T as Config>::MaxLimitOrdersForPrice::get(),
                <T as Config>::MaxOpenedLimitOrdersPerUser::get(),
                <T as Config>::MaxExpiringOrdersPerBlock::get()
            );
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(settings, caller.clone(), false);
            let order =
                OrderBookPallet::<T>::limit_orders::<_, T::OrderId>(order_book_id, order_id)
                    .unwrap();
            let balance_before =
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &order.owner)
                    .unwrap();
        }: {
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id.clone(),
                order_id.clone()
            ).unwrap();
        }
        verify {
            assert_last_event::<T>(
                Event::<T>::LimitOrderCanceled {
                    order_book_id,
                    order_id,
                    owner_id: order.owner.clone(),
                }
                .into(),
            );

            let deal_amount =
                *order.deal_amount(MarketRole::Taker, None).unwrap().value();
            let balance =
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &order.owner)
                    .unwrap();
            let expected_balance = balance_before + deal_amount.balance();
            assert_eq!(balance, expected_balance);
        }

        #[extra]
        execute_market_order {
            let caller = alice::<T>();
            let settings = FillSettings::<T>::new(
                <T as Config>::MaxSidePriceCount::get(),
                <T as Config>::MaxLimitOrdersForPrice::get(),
                <T as Config>::MaxOpenedLimitOrdersPerUser::get(),
                <T as Config>::MaxExpiringOrdersPerBlock::get()
            );
            let (order_book_id, amount) =
                prepare_market_order_benchmark(settings.clone(), caller.clone(), false);
            let caller_base_balance =
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller)
                    .unwrap();
            let caller_quote_balance =
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller)
                    .unwrap();
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id,
                PriceVariant::Sell,
                *amount.balance()
            ).unwrap();
        }
        verify {
            let average_price = balance!(11).into();
            assert_last_event::<T>(
                Event::<T>::MarketOrderExecuted {
                    order_book_id,
                    owner_id: caller.clone(),
                    direction: PriceVariant::Buy,
                    amount: OrderAmount::Base(OrderVolume::indivisible(*amount.balance())),
                    average_price,
                    to: None,
                }
                .into(),
            );
            assert_orders_numbers::<T>(order_book_id, Some(1), Some(0), None, None);
            assert_eq!(
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap(),
                caller_base_balance + *amount.balance()
            );
            assert_eq!(
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap(),
                caller_quote_balance - *(amount * average_price).balance()
            );
        }

        #[extra]
        quote {
            let settings = FillSettings::<T>::new(
                <T as Config>::MaxSidePriceCount::get(),
                <T as Config>::MaxLimitOrdersForPrice::get(),
                <T as Config>::MaxOpenedLimitOrdersPerUser::get(),
                <T as Config>::MaxExpiringOrdersPerBlock::get()
            );
            let (dex_id, input_asset_id, output_asset_id, amount, deduce_fee) =
                prepare_quote_benchmark::<T>(settings);
        }: {
            OrderBookPallet::<T>::quote(
                &dex_id,
                &input_asset_id,
                &output_asset_id,
                amount,
                deduce_fee,
            )
            .unwrap();
        }
        verify {
            // nothing changed
        }

        #[extra]
        exchange {
            let caller = alice::<T>();
            let settings = FillSettings::<T>::new(
                <T as Config>::MaxSidePriceCount::get(),
                <T as Config>::MaxLimitOrdersForPrice::get(),
                <T as Config>::MaxOpenedLimitOrdersPerUser::get(),
                <T as Config>::MaxExpiringOrdersPerBlock::get()
            );
            let (order_book_id, amount) =
                prepare_market_order_benchmark(settings.clone(), caller.clone(), true);
            let caller_base_balance =
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller)
                    .unwrap();
            let caller_quote_balance =
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller)
                    .unwrap();
        }: {
            OrderBookPallet::<T>::exchange(
                &caller,
                &caller,
                &order_book_id.dex_id,
                &order_book_id.base,
                &order_book_id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            )
            .unwrap();
        }
        verify {
            let order_amount = OrderAmount::Base(balance!(355.13473).into());
            let average_price = balance!(9.855414408497867837).into();
            assert_last_event::<T>(
                Event::<T>::MarketOrderExecuted {
                    order_book_id,
                    owner_id: caller.clone(),
                    direction: PriceVariant::Sell,
                    amount: order_amount,
                    average_price,
                    to: None,
                }
                .into(),
            );

            assert_orders_numbers::<T>(order_book_id, Some(1), Some(0), None, None);


            assert_eq!(
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap(),
                caller_base_balance + *order_amount.value().balance()
            );
            assert_eq!(
                <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap(),
                caller_quote_balance - *(*order_amount.value() * average_price).balance()
            );
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
        // modifying `benchmarks!` macro from substrate (which is quite challenging)

        delete_orderbook_1 {
            let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_1());
        } : { OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }

        delete_orderbook_2 {
            let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_2());
        } : { OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }

        delete_orderbook_3 {
            let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_3());
        } : { OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }

        delete_orderbook_4 {
            let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_4());
        } : { OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }

        #[extra]
        delete_orderbook_5 {
            let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_5());
        } : { OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }

        #[extra]
        delete_orderbook_6 {
            let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_6());
        } : { OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }

        #[extra]
        delete_orderbook_7 {
            let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_7());
        } : { OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }


        place_limit_order_1 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_1(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, price, amount, side, Some(lifespan),
            ).unwrap();
        }

        place_limit_order_2 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_2(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, price, amount, side, Some(lifespan),
            ).unwrap();
        }

        place_limit_order_3 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_3(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, price, amount, side, Some(lifespan),
            ).unwrap();
        }

        place_limit_order_4 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_4(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, price, amount, side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_5 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_5(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, price, amount, side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_6 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_6(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, price, amount, side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_7 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark::<T>(preset_7(), alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, price, amount, side, Some(lifespan),
            ).unwrap();
        }


        cancel_limit_order_first_1 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_1::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        cancel_limit_order_first_2 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_2::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        cancel_limit_order_first_3 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_3::<T>(), alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

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


        cancel_limit_order_last_1 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_1::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        cancel_limit_order_last_2 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_2::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        cancel_limit_order_last_3 {
            let signer = RawOrigin::Signed(alice::<T>()).into();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(preset_3::<T>(), alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

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


        execute_market_order_1 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_1(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, PriceVariant::Sell, *amount.balance()
            ).unwrap();
        }

        execute_market_order_2 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_2(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, PriceVariant::Sell, *amount.balance()
            ).unwrap();
        }

        execute_market_order_3 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_3(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, PriceVariant::Sell, *amount.balance()
            ).unwrap();
        }

        execute_market_order_4 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_4(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, PriceVariant::Sell, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_5 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_5(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, PriceVariant::Sell, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_6 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_6(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, PriceVariant::Sell, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_7 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_7(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, PriceVariant::Sell, *amount.balance()
            ).unwrap();
        }


        quote_1 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_1());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        quote_2 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_2());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        quote_3 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            prepare_quote_benchmark::<T>(preset_3());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

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


        exchange_1 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_1(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        exchange_2 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_2(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        exchange_3 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_3(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        exchange_4 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_4(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_5 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_5(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_6 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_6(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_7 {
            let caller = alice::<T>();
            let (id, amount) = prepare_market_order_benchmark::<T>(preset_7(), caller.clone(), true);
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
    #[allow(unused)]
    use crate::test_utils::{
        create_empty_order_book, pretty_print_expirations, pretty_print_order_book, run_to_block,
    };
    use frame_support::assert_ok;

    use crate::benchmarking::preparation::prepare_market_order_benchmark;
    use common::prelude::SwapAmount;
    use common::{balance, PriceVariant};
    use frame_support::traits::Time;
    use frame_system::RawOrigin;
    use framenode_chain_spec::ext;
    use framenode_runtime::Runtime;
    use preparation::{
        fill_order_book_worst_case, prepare_cancel_orderbook_benchmark,
        prepare_delete_orderbook_benchmark, prepare_place_orderbook_benchmark,
        prepare_quote_benchmark, presets::*,
    };

    #[test]
    #[ignore] // slow
    fn test_benchmark_fill() {
        ext().execute_with(|| {
            let order_book_id = OrderBookId::<AssetIdOf<Runtime>, u32> {
                dex_id: DEX.into(),
                base: VAL.into(),
                quote: XOR.into(),
            };

            let mut order_book = create_empty_order_book(order_book_id);
            let mut data_layer =
                framenode_runtime::order_book::cache_data_layer::CacheDataLayer::<Runtime>::new();
            // let settings = FillSettings::new(
            //     <Runtime as Config>::MaxSidePriceCount::get(),
            //     <Runtime as Config>::MaxLimitOrdersForPrice::get(),
            //     <Runtime as Config>::MaxOpenedLimitOrdersPerUser::get(),
            //     <Runtime as Config>::MaxExpiringOrdersPerBlock::get(),
            // );
            let settings = preset_3::<Runtime>();
            let _ =
                fill_order_book_worst_case(settings, &mut order_book, &mut data_layer, true, true);
            <OrderBooks<Runtime>>::insert(order_book_id, order_book);
        })
    }

    #[test]
    #[ignore] // slow
    fn test_benchmark_delete_orderbook() {
        ext().execute_with(|| {
            let settings = preset_7::<Runtime>();
            let order_book_id = prepare_delete_orderbook_benchmark::<Runtime>(settings.clone());
            let mut data_layer =
                framenode_runtime::order_book::storage_data_layer::StorageDataLayer::<Runtime>::new(
                );
            let total_orders = data_layer.get_all_limit_orders(&order_book_id).len() as u32;
            assert_eq!(
                (settings.max_side_price_count * settings.max_orders_per_price * 2),
                total_orders
            );
            run_to_block(1);
            OrderBookPallet::<Runtime>::delete_orderbook(RawOrigin::Root.into(), order_book_id)
                .unwrap();
            assert_last_event::<Runtime>(
                Event::<Runtime>::OrderBookDeleted {
                    order_book_id,
                    count_of_canceled_orders: settings.max_side_price_count
                        * settings.max_orders_per_price
                        * 2,
                }
                .into(),
            );
            assert_eq!(OrderBookPallet::<Runtime>::order_books(order_book_id), None);
            assert_eq!(
                <framenode_runtime::order_book::LimitOrders<Runtime>>::iter_prefix_values(
                    order_book_id
                )
                .next(),
                None,
            );
        })
    }

    #[test]
    fn test_benchmark_place() {
        ext().execute_with(|| {
            let settings = preset_7::<Runtime>();
            let caller = alice::<Runtime>();
            let (order_book_id, price, amount, side, lifespan) =
                prepare_place_orderbook_benchmark(settings.clone(), caller.clone());

            OrderBookPallet::<Runtime>::place_limit_order(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id,
                price,
                amount,
                side,
                Some(lifespan),
            )
            .unwrap();

            let order_id = get_last_order_id::<Runtime>(order_book_id).unwrap();

            assert_orders_numbers::<Runtime>(
                order_book_id,
                Some(0),
                None,
                Some((caller.clone(), settings.max_orders_per_user as usize)),
                Some((lifespan, settings.max_expiring_orders_per_block as usize)),
            );

            let current_block = frame_system::Pallet::<Runtime>::block_number();

            let order_book = order_book::OrderBooks::<Runtime>::get(order_book_id).unwrap();
            let now = <<Runtime as Config>::Time as Time>::now();
            let expected_limit_order = LimitOrder::<Runtime>::new(
                order_id,
                caller.clone(),
                PriceVariant::Sell,
                price.into(),
                order_book.min_lot_size.into(),
                now,
                lifespan,
                current_block,
            );
            let mut order =
                OrderBookPallet::<Runtime>::limit_orders(order_book_id, order_id).unwrap();
            order.original_amount = order.amount;
            assert_eq!(order, expected_limit_order);
        })
    }

    #[test]
    fn test_benchmark_cancel() {
        ext().execute_with(|| {
            // let settings = FillSettings::<Runtime>::new(2, 2, 3, 2);
            let settings = preset_3::<Runtime>();
            let caller = alice::<Runtime>();
            let (order_book_id, order_id) =
                prepare_cancel_orderbook_benchmark(settings, caller.clone(), false);

            // println!("1;");
            // pretty_print_order_book::<Runtime>(order_book_id.clone(), Some(9));
            // pretty_print_expirations::<Runtime>(0..10);
            OrderBookPallet::<Runtime>::cancel_limit_order(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id.clone(),
                order_id.clone(),
            )
            .unwrap();
            // println!("2;");
            // pretty_print_order_book::<Runtime>(order_book_id.clone(), Some(9));
            // pretty_print_expirations::<Runtime>(0..10);
        })
    }

    #[test]
    fn test_benchmark_quote() {
        ext().execute_with(|| {
            use common::LiquiditySource;

            // let settings = FillSettings::<Runtime>::new(2, 2, 3, 2);
            let settings = preset_3::<Runtime>();
            let (dex_id, input_asset_id, output_asset_id, amount, deduce_fee) =
                prepare_quote_benchmark::<Runtime>(settings);
            let _order_book_id = OrderBookId {
                dex_id,
                base: input_asset_id,
                quote: output_asset_id,
            };
            // pretty_print_order_book::<Runtime>(_order_book_id.clone(), None);
            let _ = OrderBookPallet::<Runtime>::quote(
                &dex_id,
                &input_asset_id,
                &output_asset_id,
                amount,
                deduce_fee,
            )
            .unwrap();
        })
    }

    #[test]
    fn test_benchmark_exchange() {
        ext().execute_with(|| {
            use common::LiquiditySource;

            // let settings = FillSettings::<Runtime>::new(2, 2, 3, 2);
            let settings = preset_3::<Runtime>();
            let caller = alice::<Runtime>();
            let (order_book_id, amount) =
                prepare_market_order_benchmark(settings.clone(), caller.clone(), true);
            // pretty_print_order_book::<Runtime>(order_book_id.clone(), None);
            let (_outcome, _) = OrderBookPallet::<Runtime>::exchange(
                &caller,
                &caller,
                &order_book_id.dex_id,
                &order_book_id.base,
                &order_book_id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            )
            .unwrap();
            // pretty_print_order_book::<Runtime>(order_book_id.clone(), None);
            assert_orders_numbers::<Runtime>(order_book_id, Some(1), Some(0), None, None);
        })
    }

    #[test]
    fn test_benchmark_execute_market_order() {
        ext().execute_with(|| {
            // let settings = FillSettings::<Runtime>::new(2, 2, 3, 2);
            let settings = preset_3::<Runtime>();
            let caller = alice::<Runtime>();
            let (order_book_id, amount) =
                prepare_market_order_benchmark(settings, caller.clone(), false);
            // pretty_print_order_book::<Runtime>(order_book_id.clone(), Some(20));
            assert_ok!(OrderBookPallet::<Runtime>::execute_market_order(
                RawOrigin::Signed(caller.clone()).into(),
                order_book_id,
                PriceVariant::Sell,
                *amount.balance(),
            ));
            // pretty_print_order_book::<Runtime>(order_book_id.clone(), Some(20));
            assert_orders_numbers::<Runtime>(order_book_id, Some(1), Some(0), None, None);
        })
    }
}
