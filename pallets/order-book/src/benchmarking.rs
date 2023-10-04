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
#![cfg(feature = "wip")]
// now it works only as benchmarks, not as unit tests
// TODO fix when new approach be developed
#![cfg(not(test))]

#[cfg(not(test))]
use crate::{
    Config, Event, LimitOrder, MarketRole, MomentOf, OrderAmount, OrderBook, OrderBookId,
    OrderBookStatus, OrderVolume, Pallet,
};
#[cfg(test)]
use framenode_runtime::order_book::{
    Config, Event, LimitOrder, MarketRole, MomentOf, OrderAmount, OrderBook, OrderBookId,
    OrderBookStatus, OrderVolume, Pallet,
};

use crate::{CacheDataLayer, ExpirationScheduler};
use assets::AssetIdOf;
use codec::Decode;
use common::prelude::{QuoteAmount, SwapAmount};
use common::{
    balance, AssetInfoProvider, AssetName, AssetSymbol, DEXId, LiquiditySource, PriceVariant, VAL,
    XOR,
};
use frame_benchmarking::benchmarks;
use frame_support::traits::Time;
use frame_support::weights::WeightMeter;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_runtime::traits::UniqueSaturatedInto;

use assets::Pallet as Assets;
use frame_system::Pallet as FrameSystem;
use trading_pair::Pallet as TradingPair;
use Pallet as OrderBookPallet;

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
fn create_and_fill_order_book<T: Config>(order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>) {
    OrderBookPallet::<T>::create_orderbook(RawOrigin::Signed(bob::<T>()).into(), order_book_id)
        .unwrap();

    Assets::<T>::update_balance(
        RawOrigin::Root.into(),
        bob::<T>(),
        order_book_id.quote,
        balance!(1000000).try_into().unwrap(),
    )
    .unwrap();

    Assets::<T>::update_balance(
        RawOrigin::Root.into(),
        bob::<T>(),
        order_book_id.base,
        balance!(1000000).try_into().unwrap(),
    )
    .unwrap();

    let lifespan: Option<MomentOf<T>> = Some(10000u32.into());

    // prices
    let bp1 = balance!(10);
    let bp2 = balance!(9.8);
    let bp3 = balance!(9.5);
    let sp1 = balance!(11);
    let sp2 = balance!(11.2);
    let sp3 = balance!(11.5);

    // amounts
    let amount1 = balance!(168.5);
    let amount2 = balance!(95.2);
    let amount3 = balance!(44.7);
    let amount4 = balance!(56.4);
    let amount5 = balance!(89.9);
    let amount6 = balance!(115);
    let amount7 = balance!(176.3);
    let amount8 = balance!(85.4);
    let amount9 = balance!(93.2);
    let amount10 = balance!(36.6);
    let amount11 = balance!(205.5);
    let amount12 = balance!(13.7);

    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp1,
        amount1,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp2,
        amount2,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp2,
        amount3,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp3,
        amount4,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp3,
        amount5,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        bp3,
        amount6,
        PriceVariant::Buy,
        lifespan,
    )
    .unwrap();

    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp1,
        amount7,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp2,
        amount8,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp2,
        amount9,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp3,
        amount10,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp3,
        amount11,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
    OrderBookPallet::<T>::place_limit_order(
        RawOrigin::Signed(bob::<T>()).into(),
        order_book_id,
        sp3,
        amount12,
        PriceVariant::Sell,
        lifespan,
    )
    .unwrap();
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

    delete_orderbook {
        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book::<T>(order_book_id);
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
                count_of_canceled_orders: 12,
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

        create_and_fill_order_book::<T>(order_book_id);

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
        let caller = alice::<T>();

        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        Assets::<T>::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            order_book_id.quote,
            balance!(1000000).try_into().unwrap()
        ).unwrap();

        let balance_before = <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap();

        let price = balance!(10);
        let amount = balance!(100);
        let lifespan: MomentOf<T> = 10000u32.into();
        let now = <<T as Config>::Time as Time>::now();

        create_and_fill_order_book::<T>(order_book_id);
    }: {
        OrderBookPallet::<T>::place_limit_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            price,
            amount,
            PriceVariant::Buy,
            Some(lifespan)
        ).unwrap();
    }
    verify {
        let order_id = get_last_order_id::<T>(order_book_id).unwrap();

        assert_last_event::<T>(
            Event::<T>::LimitOrderPlaced {
                order_book_id,
                order_id,
                owner_id: caller.clone(),
            }
            .into(),
        );

        let current_block = frame_system::Pallet::<T>::block_number();

        let expected_limit_order = LimitOrder::<T>::new(
            order_id,
            caller.clone(),
            PriceVariant::Buy,
            price.into(),
            amount.into(),
            now,
            lifespan,
            current_block
        );

        assert_eq!(
            OrderBookPallet::<T>::limit_orders(order_book_id, order_id).unwrap(),
            expected_limit_order
        );

        let deal_amount = *expected_limit_order.deal_amount(MarketRole::Taker, None).unwrap().value();
        let balance =
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap();
        let expected_balance = balance_before - deal_amount.balance();
        assert_eq!(balance, expected_balance);
    }

    cancel_limit_order {
        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book::<T>(order_book_id);

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
        let expected_balance = balance_before + deal_amount.balance();
        assert_eq!(balance, expected_balance);
    }

    execute_market_order {
        let caller = alice::<T>();
        let creator = bob::<T>();

        FrameSystem::<T>::inc_providers(&creator);

        let nft = Assets::<T>::register_from(
            &creator,
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            100000,
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

        Assets::<T>::update_balance(
            RawOrigin::Root.into(),
            creator.clone(),
            order_book_id.quote,
            balance!(1000000).try_into().unwrap()
        ).unwrap();

        Assets::<T>::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            order_book_id.base,
            1000000
        ).unwrap();
        Assets::<T>::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            order_book_id.quote,
            balance!(1000000).try_into().unwrap()
        ).unwrap();

        TradingPair::<T>::register(
            RawOrigin::Signed(creator.clone()).into(),
            DEX.into(),
            order_book_id.quote,
            order_book_id.base
        ).unwrap();

        OrderBookPallet::<T>::create_orderbook(
            RawOrigin::Signed(creator.clone()).into(),
            order_book_id
        ).unwrap();

        OrderBookPallet::<T>::place_limit_order(
            RawOrigin::Signed(creator.clone()).into(),
            order_book_id,
            balance!(10),
            100,
            PriceVariant::Buy,
            None
        ).unwrap();

        OrderBookPallet::<T>::place_limit_order(
            RawOrigin::Signed(creator.clone()).into(),
            order_book_id,
            balance!(11),
            100,
            PriceVariant::Sell,
            None
        ).unwrap();

        let amount = 20;

        let caller_base_balance = <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap();
        let caller_quote_balance = <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap();
    }: {
        OrderBookPallet::<T>::execute_market_order(
            RawOrigin::Signed(caller.clone()).into(),
            order_book_id,
            PriceVariant::Buy,
            amount
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(
            Event::<T>::MarketOrderExecuted {
                order_book_id,
                owner_id: caller.clone(),
                direction: PriceVariant::Buy,
                amount: OrderAmount::Base(OrderVolume::indivisible(amount)),
                average_price: balance!(11).into(),
                to: None,
            }
            .into(),
        );

        assert_eq!(
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap(),
            caller_base_balance + amount
        );
        assert_eq!(
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap(),
            caller_quote_balance - balance!(220)
        );
    }

    quote {
        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book::<T>(order_book_id);
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

    exchange_single_order {
        let caller = alice::<T>();

        let order_book_id = OrderBookId::<AssetIdOf<T>, T::DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book::<T>(order_book_id);

        Assets::<T>::update_balance(
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
            SwapAmount::with_desired_output(balance!(1685), balance!(170)), // this amount executes only one limit order
        )
        .unwrap();
    }
    verify {
        assert_last_event::<T>(
            Event::<T>::MarketOrderExecuted {
                order_book_id,
                owner_id: caller.clone(),
                direction: PriceVariant::Sell,
                amount: OrderAmount::Base(balance!(168.5).into()),
                average_price: balance!(10).into(),
                to: None,
            }
            .into(),
        );

        assert_eq!(
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &caller).unwrap(),
            caller_base_balance - balance!(168.5)
        );
        assert_eq!(
            <T as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &caller).unwrap(),
            caller_quote_balance + balance!(1685)
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

        create_and_fill_order_book::<T>(order_book_id);

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
        let expected_balance = balance_before + deal_amount.balance();
        assert_eq!(balance, expected_balance);
    }


    impl_benchmark_test_suite!(Pallet, framenode_chain_spec::ext(), framenode_runtime::Runtime);
}
