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

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "runtime-benchmarks")]
// order-book
#![cfg(feature = "wip")]
// too many benchmarks, doesn't compile otherwise
#![recursion_limit = "512"]
#![feature(int_roundings)]

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
use common::DEXId;
use frame_system::EventRecord;
use order_book_imported::Pallet as OrderBookPallet;
use order_book_imported::{LimitOrder, MomentOf, OrderBookId};

mod periphery;
mod preparation;
#[cfg(test)]
mod tests;

pub const DEX: DEXId = DEXId::Polkaswap;

pub struct Pallet<T: Config>(order_book_imported::Pallet<T>);
pub trait Config: order_book_imported::Config {}

fn assert_last_event<T: order_book_benchmarking_imported::Config>(
    generic_event: <T as order_book_imported::Config>::RuntimeEvent,
) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

/// if `None` then don't compare the value
pub fn assert_orders_numbers<T: order_book_benchmarking_imported::Config>(
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

#[cfg(not(test))]
pub use benchmarks_inner::*;

// Separate module in order to disable tests (they do not work with current approach: using
// runtime)
#[cfg(not(test))]
mod benchmarks_inner {
    use common::prelude::SwapAmount;
    use common::{balance, AssetInfoProvider, AssetName, AssetSymbol, LiquiditySource, VAL, XOR};
    use frame_benchmarking::benchmarks;
    use frame_support::weights::WeightMeter;
    use frame_system::RawOrigin;
    use sp_runtime::traits::UniqueSaturatedInto;

    use super::*;
    use order_book_imported::cache_data_layer::CacheDataLayer;
    use order_book_imported::test_utils::fill_tools::FillSettings;
    use order_book_imported::test_utils::{accounts, create_and_fill_order_book};
    use order_book_imported::{
        Event, ExpirationScheduler, MarketRole, OrderBook, OrderBookId, OrderBookStatus,
    };
    use preparation::presets::*;

    use frame_system::Pallet as FrameSystem;
    use trading_pair::Pallet as TradingPair;

    benchmarks! {
        where_clause {
            where T: trading_pair::Config + core::fmt::Debug
        }

        create_orderbook {
            let caller = accounts::alice::<T>();
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
            let settings = FillSettings::<T>::max();
            let order_book_id = periphery::delete_orderbook::init(settings.clone());
        }: {
            OrderBookPallet::<T>::delete_orderbook(
                RawOrigin::Root.into(),
                order_book_id
            ).unwrap();
        }
        verify {
            periphery::delete_orderbook::verify(settings, order_book_id);
        }

        update_orderbook {
            let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
                dex_id: DEX.into(),
                base: VAL.into(),
                quote: XOR.into(),
            };

            create_and_fill_order_book::<T>(order_book_id);

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

            create_and_fill_order_book::<T>(order_book_id);
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
            let settings = FillSettings::<T>::max();
            let context = periphery::place_limit_order::init(settings.clone());
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
            periphery::place_limit_order::verify(settings, context);
        }

        cancel_limit_order_first_expiration {
            let settings = FillSettings::<T>::max();
            let context = periphery::cancel_limit_order::init(settings.clone(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id.clone(),
                context.order_id.clone()
            ).unwrap();
        }
        verify {
            periphery::cancel_limit_order::verify(settings, context);
        }

        cancel_limit_order_last_expiration {
            let settings = FillSettings::<T>::max();
            let context = periphery::cancel_limit_order::init(settings.clone(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id.clone(),
                context.order_id.clone()
            ).unwrap();
        }
        verify {
            periphery::cancel_limit_order::verify(settings, context);
        }

        execute_market_order {
            let settings = FillSettings::<T>::max();
            let context = periphery::execute_market_order::init(settings.clone());
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id,
                context.side,
                *context.amount.balance()
            ).unwrap();
        }
        verify {
            periphery::execute_market_order::verify(settings, context);
        }

        quote {
            let settings = FillSettings::<T>::max();
            let context = periphery::quote::init(settings.clone());
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
            let settings = FillSettings::<T>::max();
            let context = periphery::exchange_single_order::init(settings.clone());
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
            periphery::exchange_single_order::verify(settings, context);
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

            create_and_fill_order_book::<T>(order_book_id);

            let order_id = 5u128.unique_saturated_into();

            let order = OrderBookPallet::<T>::limit_orders(order_book_id, order_id).unwrap();

            let balance_before =
                <T as order_book_imported::Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &order.owner).unwrap();

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
                <T as order_book_imported::Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &order.owner).unwrap();
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
            let signer = RawOrigin::Signed(accounts::alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                preparation::place_limit_order::<T>(preset_1(), accounts::alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_2 {
            let signer = RawOrigin::Signed(accounts::alice::<T>()).into();
            let (order_book_id, price, amount, side, lifespan) =
                preparation::place_limit_order::<T>(preset_2(), accounts::alice::<T>());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                signer, order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }


        #[extra]
        cancel_limit_order_first_1 {
            let signer = RawOrigin::Signed(accounts::alice::<T>()).into();
            let (order_book_id, order_id) =
                preparation::cancel_limit_order(preset_1::<T>(), accounts::alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_first_2 {
            let signer = RawOrigin::Signed(accounts::alice::<T>()).into();
            let (order_book_id, order_id) =
                preparation::cancel_limit_order(preset_2::<T>(), accounts::alice::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }


        #[extra]
        cancel_limit_order_last_1 {
            let signer = RawOrigin::Signed(accounts::alice::<T>()).into();
            let (order_book_id, order_id) =
                preparation::cancel_limit_order(preset_1::<T>(), accounts::alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }

        #[extra]
        cancel_limit_order_last_2 {
            let signer = RawOrigin::Signed(accounts::alice::<T>()).into();
            let (order_book_id, order_id) =
                preparation::cancel_limit_order(preset_2::<T>(), accounts::alice::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(signer, order_book_id, order_id).unwrap();
        }


        #[extra]
        execute_market_order_1 {
            let caller = accounts::alice::<T>();
            let (id, amount, side) = preparation::market_order_execution::<T>(preset_1(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_2 {
            let caller = accounts::alice::<T>();
            let (id, amount, side) = preparation::market_order_execution::<T>(preset_2(), caller.clone(), false);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }


        #[extra]
        quote_1 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            preparation::quote::<T>(preset_1());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_2 {
            let (dex_id, input_id, output_id, amount, deduce_fee) =
            preparation::quote::<T>(preset_2());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_id, &output_id, amount, deduce_fee)
                .unwrap();
        }


        #[extra]
        exchange_1 {
            let caller = accounts::alice::<T>();
            let (id, amount, _) = preparation::market_order_execution::<T>(preset_1(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }

        #[extra]
        exchange_2 {
            let caller = accounts::alice::<T>();
            let (id, amount, _) = preparation::market_order_execution::<T>(preset_2(), caller.clone(), true);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_input(*amount.balance(), balance!(0)),
            ).unwrap();
        }
    }
}
