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
#![cfg(feature = "ready-to-test")]
#![allow(clippy::type_complexity)]
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
            order_book_imported::UserLimitOrders::<T>::get(user, order_book_id)
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
    use sp_std::vec::Vec;

    use super::*;
    use order_book_imported::cache_data_layer::CacheDataLayer;
    use order_book_imported::test_utils::fill_tools::FillSettings;
    use order_book_imported::test_utils::{accounts, create_and_fill_order_book};
    use order_book_imported::{
        CancelReason, Event, ExpirationScheduler, MarketRole, OrderBook, OrderBookId,
        OrderBookStatus, OrderPrice, OrderVolume,
    };
    use periphery::presets::*;

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
                1000,
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
                order_book_id,
                balance!(0.00001),
                1,
                1,
                1000
            ).unwrap();
        }
        verify {
            assert_last_event::<T>(
                Event::<T>::OrderBookCreated {
                    order_book_id,
                    creator: Some(caller),
                }
                .into(),
            );

            assert_eq!(
                OrderBookPallet::<T>::order_books(order_book_id).unwrap(),
                OrderBook::<T>::new(
                    order_book_id,
                    OrderPrice::divisible(balance!(0.00001)),
                    OrderVolume::indivisible(1),
                    OrderVolume::indivisible(1),
                    OrderVolume::indivisible(1000),
                )
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

        place_limit_order_without_cross_spread {
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
                context.order_book_id,
                context.order_id
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
                context.order_book_id,
                context.order_id
            ).unwrap();
        }
        verify {
            periphery::cancel_limit_order::verify(settings, context);
        }

        execute_market_order {
            let settings = FillSettings::<T>::max();
            let context = periphery::execute_market_order_scattered::init(settings);
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(context.caller.clone()).into(),
                context.order_book_id,
                context.side,
                *context.amount.balance()
            ).unwrap();
        }
        verify {
            periphery::execute_market_order_scattered::verify(context);
        }

        quote {
            let settings = FillSettings::<T>::max();
            let context = periphery::quote::init(settings);
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

        exchange {
            let e in 1u32 .. <T as order_book_imported::Config>::HARD_MIN_MAX_RATIO.try_into().unwrap();
            let mut settings = FillSettings::<T>::max();
            settings.executed_orders_limit = e;
            let context = periphery::exchange_scattered::init(settings);
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
            periphery::exchange_scattered::verify(context);
        }

        align_single_order {
            let settings = FillSettings::<T>::max();
            let context = periphery::align_single_order::init(settings);

            let mut data = order_book_imported::storage_data_layer::StorageDataLayer::<T>::new();
        }: {
            context
            .order_book
            .align_limit_orders(Vec::from([context.order_to_align.clone()]), &mut data)
            .unwrap();
        }
        verify {
            periphery::align_single_order::verify(context);
        }

        service_expiration_base {
            let mut weight = WeightMeter::max_limit();
            let block_number = 0u32.unique_saturated_into();
        }: {
            OrderBookPallet::<T>::service_expiration(block_number, &mut weight);
        }
        verify {}

        service_expiration_block_base {
            let mut weight = WeightMeter::max_limit();
            let block_number = 0u32.unique_saturated_into();
            // should be the slower layer because cache is not
            // warmed up
            let mut data_layer = CacheDataLayer::<T>::new();
        }: {
            OrderBookPallet::<T>::service_expiration_block(&mut data_layer, block_number, &mut weight);
        }
        verify {}

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
                Event::<T>::LimitOrderCanceled {
                    order_book_id,
                    order_id,
                    owner_id: order.owner.clone(),
                    reason: CancelReason::Expired,
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
        // 1. edit presets in ./periphery/preparation.rs (with names "preset_*" where * is 1,2,3,4,5,...)
        // 2. in ./generate_benchmarks.py set `max_preset` to the highest preset number
        // 3. run ./generate_benchmarks.py
        // 4. paste output here (instead of existing benches)
        // 5. build as usual (with `--release` flag)
        // 6. run with ./benchmark_attributes.sh

        #[extra]
        place_limit_order_without_cross_spread_1 {
            use periphery::place_limit_order::{init, Context};
            let Context { caller, order_book_id, price, amount, side, lifespan, .. } =
                init::<T>(preset_1());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                RawOrigin::Signed(caller).into(), order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }

        #[extra]
        place_limit_order_without_cross_spread_2 {
            use periphery::place_limit_order::{init, Context};
            let Context { caller, order_book_id, price, amount, side, lifespan, .. } =
                init::<T>(preset_2());
        }: {
            OrderBookPallet::<T>::place_limit_order(
                RawOrigin::Signed(caller).into(), order_book_id, *price.balance(), *amount.balance(), side, Some(lifespan),
            ).unwrap();
        }


        #[extra]
        cancel_limit_order_first_1 {
            use periphery::cancel_limit_order::{init, Context};
            let Context { caller, order_book_id, order_id, .. } =
                init::<T>(preset_1::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(caller).into(), order_book_id, order_id
            ).unwrap();
        }

        #[extra]
        cancel_limit_order_first_2 {
            use periphery::cancel_limit_order::{init, Context};
            let Context { caller, order_book_id, order_id, .. } =
                init::<T>(preset_2::<T>(), true);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(caller).into(), order_book_id, order_id
            ).unwrap();
        }


        #[extra]
        cancel_limit_order_last_1 {
            use periphery::cancel_limit_order::{init, Context};
            let Context { caller, order_book_id, order_id, .. } =
                init::<T>(preset_1::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(caller).into(), order_book_id, order_id
            ).unwrap();
        }

        #[extra]
        cancel_limit_order_last_2 {
            use periphery::cancel_limit_order::{init, Context};
            let Context { caller, order_book_id, order_id, .. } =
                init::<T>(preset_2::<T>(), false);
        }: {
            OrderBookPallet::<T>::cancel_limit_order(
                RawOrigin::Signed(caller).into(), order_book_id, order_id
            ).unwrap();
        }


        #[extra]
        execute_market_order_1 {
            use periphery::execute_market_order::{init, Context};
            let Context { caller, order_book_id: id, amount, side, .. } =
                init::<T>(preset_1::<T>());
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }

        #[extra]
        execute_market_order_2 {
            use periphery::execute_market_order::{init, Context};
            let Context { caller, order_book_id: id, amount, side, .. } =
                init::<T>(preset_2::<T>());
        }: {
            OrderBookPallet::<T>::execute_market_order(
                RawOrigin::Signed(caller).into(), id, side, *amount.balance()
            ).unwrap();
        }


        #[extra]
        quote_1 {
            use periphery::quote::{init, Context};
            let Context { dex_id, input_asset_id, output_asset_id, amount, deduce_fee } =
                init::<T>(preset_1::<T>());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_asset_id, &output_asset_id, amount, deduce_fee)
                .unwrap();
        }

        #[extra]
        quote_2 {
            use periphery::quote::{init, Context};
            let Context { dex_id, input_asset_id, output_asset_id, amount, deduce_fee } =
                init::<T>(preset_2::<T>());
        }: {
            OrderBookPallet::<T>::quote(&dex_id, &input_asset_id, &output_asset_id, amount, deduce_fee)
                .unwrap();
        }


        #[extra]
        exchange_1 {
            let e in 1u32 .. <T as order_book_imported::Config>::HARD_MIN_MAX_RATIO.try_into().unwrap();
            use periphery::exchange_scattered::{init, Context};
            let mut settings = preset_1::<T>();
            settings.executed_orders_limit = e;
            let Context { caller, order_book_id: id, expected_in, expected_out, .. } = init(settings);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_output(expected_out, expected_in + balance!(1.5)),
            ).unwrap();
        }

        #[extra]
        exchange_2 {
            let e in 1u32 .. <T as order_book_imported::Config>::HARD_MIN_MAX_RATIO.try_into().unwrap();
            use periphery::exchange_scattered::{init, Context};
            let mut settings = preset_1::<T>();
            settings.executed_orders_limit = e;
            let Context { caller, order_book_id: id, expected_in, expected_out, .. } = init(settings);
        } : {
            OrderBookPallet::<T>::exchange(
                &caller, &caller, &id.dex_id, &id.base, &id.quote,
                SwapAmount::with_desired_output(expected_out, expected_in + balance!(1.5)),
            ).unwrap();
        }
    }
}
