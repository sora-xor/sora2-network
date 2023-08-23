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

#![cfg(feature = "runtime-benchmarks")]
// order-book
#![cfg(feature = "ready-to-test")]

#[cfg(not(test))]
use crate as order_book;
#[allow(unused)]
#[cfg(not(test))]
use crate::{
    cache_data_layer::CacheDataLayer, traits::DataLayer, Config, Event, ExpirationScheduler,
    LimitOrder, MarketRole, MomentOf, OrderAmount, OrderBook, OrderBookId, OrderBookStatus,
    OrderBooks, OrderVolume, Pallet,
};
#[cfg(test)]
use framenode_runtime::order_book;
#[allow(unused)]
#[cfg(test)]
use framenode_runtime::order_book::{
    cache_data_layer::CacheDataLayer, traits::DataLayer, Config, Event, ExpirationScheduler,
    LimitOrder, MarketRole, MomentOf, OrderAmount, OrderBook, OrderBookId, OrderBookStatus,
    OrderBooks, OrderVolume, Pallet,
};

// `#[cfg(not(test))]`'s are so that `unused_imports` warnings are not emitted
use assets::AssetIdOf;
use codec::Decode;
#[cfg(not(test))]
use common::prelude::{QuoteAmount, SwapAmount};
#[cfg(not(test))]
use common::{balance, AssetInfoProvider, LiquiditySource, PriceVariant};
use common::{DEXId, VAL, XOR};
#[cfg(not(test))]
use frame_benchmarking::benchmarks;
#[cfg(not(test))]
use frame_support::traits::{Get, Time};
#[cfg(not(test))]
use frame_support::weights::WeightMeter;
use frame_system::EventRecord;
#[cfg(not(test))]
use frame_system::RawOrigin;
use hex_literal::hex;
#[cfg(not(test))]
use preparation::create_and_populate_order_book;
#[cfg(not(test))]
use sp_runtime::traits::UniqueSaturatedInto;

#[allow(unused)]
use crate::benchmarking::preparation::{
    prepare_delete_orderbook_benchmark, prepare_place_orderbook_benchmark, presets::*, FillSettings,
};

#[cfg(not(test))]
use assets::Pallet as AssetsPallet;
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
    expected_bids: Option<usize>,
    expected_asks: Option<usize>,
    author: T::AccountId,
    expected_user_limit_orders: usize,
    lifespan: MomentOf<T>,
    expected_expirations: usize,
) {
    // # of bids should be max to execute max # of orders and payments
    if let Some(expected_bids) = expected_bids {
        assert_eq!(
            order_book::Bids::<T>::iter_prefix(order_book_id)
                .flat_map(|(_price, orders)| orders.into_iter())
                .count(),
            expected_bids
        );
    }
    if let Some(expected_asks) = expected_asks {
        assert_eq!(
            order_book::Asks::<T>::iter_prefix(order_book_id)
                .flat_map(|(_price, orders)| orders.into_iter())
                .count(),
            expected_asks
        );
    }
    // user orders of `caller` should be almost full
    assert_eq!(
        order_book::UserLimitOrders::<T>::get(author.clone(), order_book_id)
            .unwrap()
            .len(),
        expected_user_limit_orders
    );
    // expiration schedule for the block should be almost full
    assert_eq!(
        order_book::ExpirationsAgenda::<T>::get(LimitOrder::<T>::resolve_lifespan(
            frame_system::Pallet::<T>::block_number(),
            lifespan
        ))
        .len(),
        expected_expirations
    );
}

#[cfg(not(test))]
benchmarks! {
    where_clause {
        where T: trading_pair::Config + core::fmt::Debug
    }

    create_orderbook {
        let caller = alice::<T>();

        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
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
            OrderBook::<T>::default(order_book_id)
        );
    }

    delete_orderbook {
        let order_book_id = prepare_delete_orderbook_benchmark::<T>(FillSettings::new(
            <T as Config>::MaxSidePriceCount::get(),
            <T as Config>::MaxLimitOrdersForPrice::get(),
            <T as Config>::MaxOpenedLimitOrdersPerUser::get(),
            <T as Config>::MaxExpiringOrdersPerBlock::get()
        ));
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
        let step_lot_size = balance!(0.001);
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
        assert_eq!(order_book.tick_size, tick_size);
        assert_eq!(order_book.step_lot_size, step_lot_size);
        assert_eq!(order_book.min_lot_size, min_lot_size);
        assert_eq!(order_book.max_lot_size, max_lot_size);
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
            caller.clone(),
            settings.max_orders_per_user as usize,
            lifespan,
            settings.max_expiring_orders_per_block as usize,
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
            PriceVariant::Buy,
            price,
            order_book.min_lot_size,
            now,
            lifespan,
            current_block
        );

        assert_eq!(
            OrderBookPallet::<T>::limit_orders(order_book_id, order_id).unwrap(),
            expected_limit_order
        );
    }

    cancel_limit_order {
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
    }: {
        OrderBookPallet::<T>::cancel_limit_order(
            RawOrigin::Signed(order.owner.clone()).into(),
            order_book_id,
            order_id
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
        let expected_balance = balance_before + deal_amount;
        assert_eq!(balance, expected_balance);
    }

    quote {
        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_populate_order_book::<T>(order_book_id);
    }: {
        OrderBookPallet::<T>::quote(
            &DEX.into(),
            &VAL.into(),
            &XOR.into(),
            QuoteAmount::with_desired_output(balance!(2500)),
            true
        )
        .unwrap();
    }
    verify {
        // nothing changed
    }

    exchange {
        let caller = alice::<T>();

        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_populate_order_book::<T>(order_book_id);

        AssetsPallet::<T>::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            order_book_id.base,
            balance!(1000000).try_into().unwrap()
        ).unwrap();

        let caller_base_balance = <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap();
        let caller_quote_balance = <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap();
    }: {
        OrderBookPallet::<T>::exchange(
            &caller,
            &caller,
            &DEX.into(),
            &VAL.into(),
            &XOR.into(),
            SwapAmount::with_desired_output(balance!(3500), balance!(360)),
        )
        .unwrap();
    }
    verify {
        assert_last_event::<T>(
            Event::<T>::MarketOrderExecuted {
                order_book_id,
                owner_id: caller.clone(),
                direction: PriceVariant::Sell,
                amount: OrderAmount::Base(balance!(355.13473)),
                average_price: balance!(9.855414408497867837),
                to: None,
            }
            .into(),
        );

        assert_eq!(
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap(),
            caller_base_balance - balance!(355.13473)
        );
        assert_eq!(
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap(),
            caller_quote_balance + balance!(3499.999935)
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

        // should be the slower layer because cache is not
        // warmed up
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
        let expected_balance = balance_before + deal_amount;
        assert_eq!(balance, expected_balance);
    }


    // now it works only as benchmarks, not as unit tests
    // TODO fix when new approach be developed
    // impl_benchmark_test_suite!(Pallet, framenode_chain_spec::ext(), framenode_runtime::Runtime);

    // attributes benchmarks

    // macros are executed outside-in, therefore implementing such codegen within Rust requires
    // modifying `benchmarks!` macro from substrate (which is quite challenging)

    #[extra]
    delete_orderbook_1 {
        let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_1());
    } : { OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }

    #[extra]
    delete_orderbook_2 {
        let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_2());
    } : { OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }

    #[extra]
    delete_orderbook_3 {
        let order_book_id = prepare_delete_orderbook_benchmark::<T>(preset_3());
    } : { OrderBookPallet::<T>::delete_orderbook(RawOrigin::Root.into(), order_book_id).unwrap() }

    #[extra]
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

    #[extra]
    place_limit_order_1 {
        let signer = RawOrigin::Signed(alice::<T>()).into();
        let (order_book_id, price, amount, side, lifespan) =
            prepare_place_orderbook_benchmark::<T>(preset_1(), alice::<T>());
    }: {
        OrderBookPallet::<T>::place_limit_order(
            signer, order_book_id, price, amount, side, Some(lifespan),
        ).unwrap();
    }

    #[extra]
    place_limit_order_2 {
        let signer = RawOrigin::Signed(alice::<T>()).into();
        let (order_book_id, price, amount, side, lifespan) =
            prepare_place_orderbook_benchmark::<T>(preset_2(), alice::<T>());
    }: {
        OrderBookPallet::<T>::place_limit_order(
            signer, order_book_id, price, amount, side, Some(lifespan),
        ).unwrap();
    }

    #[extra]
    place_limit_order_3 {
        let signer = RawOrigin::Signed(alice::<T>()).into();
        let (order_book_id, price, amount, side, lifespan) =
            prepare_place_orderbook_benchmark::<T>(preset_3(), alice::<T>());
    }: {
        OrderBookPallet::<T>::place_limit_order(
            signer, order_book_id, price, amount, side, Some(lifespan),
        ).unwrap();
    }

    #[extra]
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::benchmarking::preparation::{
        fill_order_book_worst_case, prepare_place_orderbook_benchmark, FillSettings,
    };
    use crate::test_utils::create_empty_order_book;
    use frame_support::traits::Get;
    use frame_system::RawOrigin;
    use framenode_chain_spec::ext;
    use framenode_runtime::Runtime;

    #[test]
    #[ignore] // slow
    fn test_benchmark_fill() {
        ext().execute_with(|| {
            let order_book_id = OrderBookId::<AssetIdOf<Runtime>, u32> {
                dex_id: DEX.into(),
                base: VAL.into(),
                quote: XOR.into(),
            };

            create_empty_order_book(order_book_id);
            let mut data_layer =
                framenode_runtime::order_book::cache_data_layer::CacheDataLayer::<Runtime>::new();
            let settings = FillSettings::new(
                <Runtime as Config>::MaxSidePriceCount::get(),
                <Runtime as Config>::MaxLimitOrdersForPrice::get(),
                <Runtime as Config>::MaxOpenedLimitOrdersPerUser::get(),
                <Runtime as Config>::MaxExpiringOrdersPerBlock::get(),
            );
            fill_order_book_worst_case(settings, &order_book_id, &mut data_layer, true, true);
        })
    }

    #[ignore] // slow
    fn test_benchmark_delete_orderbook() {
        ext().execute_with(|| {
            let settings = preset_3::<Runtime>();
            let order_book_id = prepare_delete_orderbook_benchmark::<Runtime>(settings.clone());
            let mut data_layer =
                framenode_runtime::order_book::storage_data_layer::StorageDataLayer::<Runtime>::new(
                );
            let total_orders = data_layer.get_all_limit_orders(&order_book_id).len() as u32;
            assert_eq!(
                (settings.max_side_price_count * settings.max_orders_per_price * 2),
                total_orders
            );
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

    fn print_order_book<T: Config>(
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        author: T::AccountId,
    ) {
        println!(
            "bids:\n{:#?}",
            framenode_runtime::order_book::Bids::<T>::iter_prefix(order_book_id)
                .collect::<Vec<_>>()
        );
        println!(
            "asks:\n{:#?}",
            framenode_runtime::order_book::Asks::<T>::iter_prefix(order_book_id)
                .collect::<Vec<_>>()
        );
        println!(
            "alice orders:\n{:#?}",
            framenode_runtime::order_book::UserLimitOrders::<T>::iter_prefix(author)
                .collect::<Vec<_>>()
        );
        println!(
            "expirations:\n{:#?}",
            framenode_runtime::order_book::ExpirationsAgenda::<T>::iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_benchmark_place() {
        ext().execute_with(|| {
            let settings = preset_3::<Runtime>();
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
        })
    }
}
