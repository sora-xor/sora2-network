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

use crate::tests::test_utils::*;
use assets::AssetIdOf;
use common::prelude::QuoteAmount;
use common::{balance, AssetName, AssetSymbol, PriceVariant, DOT, KSM, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::order_book::cache_data_layer::CacheDataLayer;
use framenode_runtime::order_book::storage_data_layer::StorageDataLayer;
use framenode_runtime::order_book::{
    Config, DataLayer, DealInfo, LimitOrder, MarketChange, MarketOrder, MarketRole, OrderAmount,
    OrderBook, OrderBookId, OrderBookStatus,
};
use framenode_runtime::{Runtime, RuntimeOrigin};
use sp_core::Get;
use sp_std::collections::btree_map::BTreeMap;

#[test]
fn should_create_new() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    let expected = OrderBook::<Runtime> {
        order_book_id: order_book_id,
        dex_id: DEX.into(),
        status: OrderBookStatus::Trade,
        last_order_id: 0,
        tick_size: balance!(0.001),
        step_lot_size: balance!(0.1),
        min_lot_size: balance!(1),
        max_lot_size: balance!(10000),
    };

    assert_eq!(
        OrderBook::<Runtime>::new(
            order_book_id,
            DEX.into(),
            balance!(0.001),
            balance!(0.1),
            balance!(1),
            balance!(10000)
        ),
        expected
    );
}

#[test]
fn should_create_default() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    let expected = OrderBook::<Runtime> {
        order_book_id: order_book_id,
        dex_id: DEX.into(),
        status: OrderBookStatus::Trade,
        last_order_id: 0,
        tick_size: balance!(0.00001),
        step_lot_size: balance!(0.00001),
        min_lot_size: balance!(1),
        max_lot_size: balance!(100000),
    };

    assert_eq!(
        OrderBook::<Runtime>::default(order_book_id, DEX.into()),
        expected
    );
}

#[test]
fn should_create_default_nft() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    let expected = OrderBook::<Runtime> {
        order_book_id: order_book_id,
        dex_id: DEX.into(),
        status: OrderBookStatus::Trade,
        last_order_id: 0,
        tick_size: balance!(0.00001),
        step_lot_size: balance!(1),
        min_lot_size: balance!(1),
        max_lot_size: balance!(100000),
    };

    assert_eq!(
        OrderBook::<Runtime>::default_nft(order_book_id, DEX.into()),
        expected
    );
}

#[test]
fn should_increment_order_id() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    let mut order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());
    assert_eq!(order_book.last_order_id, 0);

    assert_eq!(order_book.next_order_id(), 1);
    assert_eq!(order_book.last_order_id, 1);

    order_book.last_order_id = 8;

    assert_eq!(order_book.next_order_id(), 9);
    assert_eq!(order_book.last_order_id, 9);
}

#[test]
fn should_place_limit_order() {
    ext().execute_with(|| {
        let owner = alice();
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);
        fill_balance(owner.clone(), order_book_id);

        let order_id = 100;
        let price = balance!(10);
        let amount = balance!(100);

        // fix state before
        let bids_before = data.get_bids(&order_book_id, &price).unwrap_or_default();
        let agg_bids_before = data.get_aggregated_bids(&order_book_id);
        let price_volume_before = agg_bids_before.get(&price).cloned().unwrap_or_default();
        let user_orders_before = data
            .get_user_limit_orders(&owner, &order_book_id)
            .unwrap_or_default();
        let balance_before = free_balance(&order_book_id.quote, &owner);

        // new order
        let order = LimitOrder::<Runtime>::new(
            order_id,
            owner.clone(),
            PriceVariant::Buy,
            price,
            amount,
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let deal_amount = *order.deal_amount(MarketRole::Taker, None).unwrap().value();

        // place new order
        assert_ok!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(order, &mut data)
        );

        // check
        let mut expected_bids = bids_before.clone();
        assert_ok!(expected_bids.try_push(order_id));
        assert_eq!(
            data.get_bids(&order_book_id, &price).unwrap(),
            expected_bids
        );

        let expected_price_volume = price_volume_before + amount;
        let mut expected_agg_bids = agg_bids_before.clone();
        assert_ok!(expected_agg_bids.try_insert(price, expected_price_volume));
        assert_eq!(data.get_aggregated_bids(&order_book_id), expected_agg_bids);

        let mut expected_user_orders = user_orders_before.clone();
        assert_ok!(expected_user_orders.try_push(order_id));
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            expected_user_orders
        );

        let balance = free_balance(&order_book_id.quote, &owner);
        let expected_balance = balance_before - deal_amount;
        assert_eq!(balance, expected_balance);
    });
}

#[test]
fn should_place_nft_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let owner = alice();
        frame_system::Pallet::<Runtime>::inc_providers(&owner);

        let nft = assets::Pallet::<Runtime>::register_from(
            &owner,
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            balance!(1),
            false,
            None,
            None,
        )
        .unwrap();

        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            XOR,
            INIT_BALANCE.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: nft,
            quote: XOR.into(),
        };

        let order_book = OrderBook::<Runtime>::default_nft(order_book_id, DEX.into());
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        let order_id = 11;
        let price = balance!(10);
        let amount = balance!(1);

        // new order
        let order = LimitOrder::<Runtime>::new(
            order_id,
            owner.clone(),
            PriceVariant::Sell,
            price,
            amount,
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // place new order
        assert_ok!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(order, &mut data)
        );

        // check
        assert_eq!(
            data.get_asks(&order_book_id, &price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_id]
        );

        let balance = free_balance(&order_book_id.base, &owner);
        assert_eq!(balance, balance!(0));
    })
}

#[test]
fn should_not_place_limit_order_when_status_doesnt_allow() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let mut order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        fill_balance(alice(), order_book_id);

        let mut order = LimitOrder::<Runtime>::new(
            1,
            alice(),
            PriceVariant::Buy,
            balance!(10),
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        order_book.status = OrderBookStatus::Stop;
        assert_err!(
            order_book
                .place_limit_order::<OrderBookPallet, OrderBookPallet>(order.clone(), &mut data),
            E::PlacementOfLimitOrdersIsForbidden
        );

        order_book.status = OrderBookStatus::OnlyCancel;
        assert_err!(
            order_book
                .place_limit_order::<OrderBookPallet, OrderBookPallet>(order.clone(), &mut data),
            E::PlacementOfLimitOrdersIsForbidden
        );

        order_book.status = OrderBookStatus::PlaceAndCancel;
        assert_ok!(order_book
            .place_limit_order::<OrderBookPallet, OrderBookPallet>(order.clone(), &mut data));

        order_book.status = OrderBookStatus::Trade;
        order.id = 2;
        assert_ok!(order_book
            .place_limit_order::<OrderBookPallet, OrderBookPallet>(order.clone(), &mut data));
    });
}

#[test]
fn should_not_place_invalid_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());

        let order = LimitOrder::<Runtime>::new(
            1,
            alice(),
            PriceVariant::Buy,
            balance!(10),
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let mut wrong_price_order = order.clone();
        wrong_price_order.price = balance!(10) + order_book.tick_size / 100;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                wrong_price_order,
                &mut data
            ),
            E::InvalidLimitOrderPrice
        );

        let mut too_small_amount_order = order.clone();
        too_small_amount_order.amount = order_book.min_lot_size / 2;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                too_small_amount_order,
                &mut data
            ),
            E::InvalidOrderAmount
        );

        let mut too_big_amount_order = order.clone();
        too_big_amount_order.amount = order_book.max_lot_size + 1;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                too_big_amount_order,
                &mut data
            ),
            E::InvalidOrderAmount
        );

        let mut wrong_amount_order = order.clone();
        wrong_amount_order.amount = balance!(100) + order_book.step_lot_size / 100;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                wrong_amount_order,
                &mut data
            ),
            E::InvalidOrderAmount
        );
    })
}

#[test]
fn should_not_place_invalid_nft_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();
        frame_system::Pallet::<Runtime>::inc_providers(&alice());

        let nft = assets::Pallet::<Runtime>::register_from(
            &alice(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            balance!(1),
            false,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: nft,
            quote: XOR.into(),
        };

        let order_book = OrderBook::<Runtime>::default_nft(order_book_id, DEX.into());

        let order = LimitOrder::<Runtime>::new(
            1,
            alice(),
            PriceVariant::Buy,
            balance!(10),
            balance!(1),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let mut wrong_price_order = order.clone();
        wrong_price_order.price = balance!(10) + order_book.tick_size / 100;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                wrong_price_order,
                &mut data
            ),
            E::InvalidLimitOrderPrice
        );

        let mut too_small_amount_order = order.clone();
        too_small_amount_order.amount = balance!(0.5);
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                too_small_amount_order,
                &mut data
            ),
            E::InvalidOrderAmount
        );

        let mut too_big_amount_order = order.clone();
        too_big_amount_order.amount = order_book.max_lot_size + 1;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                too_big_amount_order,
                &mut data
            ),
            E::InvalidOrderAmount
        );

        let mut wrong_amount_order = order.clone();
        wrong_amount_order.amount = balance!(1) - order_book.step_lot_size / 100;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                wrong_amount_order,
                &mut data
            ),
            E::InvalidOrderAmount
        );
    })
}

#[test]
fn should_not_place_limit_order_that_doesnt_meet_restrictions_for_user() {
    ext().execute_with(|| {
        let mut data = CacheDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();

        fill_balance(alice(), order_book_id);

        let mut order = LimitOrder::<Runtime>::new(
            0,
            alice(),
            PriceVariant::Buy,
            balance!(10),
            balance!(1),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let max_orders_per_user: u32 = <Runtime as Config>::MaxOpenedLimitOrdersPerUser::get();

        for _ in 0..max_orders_per_user {
            order.id += 1;
            order.price += balance!(0.001);
            assert_ok!(order_book
                .place_limit_order::<OrderBookPallet, OrderBookPallet>(order.clone(), &mut data));
        }

        order.id += 1;
        order.price += balance!(0.001);
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(order, &mut data),
            E::UserHasMaxCountOfOpenedOrders
        );
    })
}

#[test]
fn should_not_place_limit_order_that_doesnt_meet_restrictions_for_orders_in_price() {
    ext().execute_with(|| {
        let mut data = CacheDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();
        let max_orders_for_price: u32 = <Runtime as Config>::MaxLimitOrdersForPrice::get();

        let mut buy_order = LimitOrder::<Runtime>::new(
            0,
            alice(),
            PriceVariant::Buy,
            balance!(10),
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let mut sell_order = LimitOrder::<Runtime>::new(
            max_orders_for_price as u128 + 1000,
            alice(),
            PriceVariant::Sell,
            balance!(11),
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        for i in 0..max_orders_for_price {
            // get new owner for each order to not get UserHasMaxCountOfOpenedOrders error
            let account = generate_account(i);

            fill_balance(account.clone(), order_book_id);

            buy_order.id += 1;
            buy_order.owner = account.clone();
            // should ideally be set through `LimitOrder::new`
            // but let's do it in a hacky way for simplicity
            buy_order.expires_at += 1;
            sell_order.id += 1;
            sell_order.owner = account;
            // should ideally be set through `LimitOrder::new`
            // but let's do it in a hacky way for simplicity
            sell_order.expires_at += 1;

            assert_ok!(
                order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                    buy_order.clone(),
                    &mut data
                )
            );
            assert_ok!(
                order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                    sell_order.clone(),
                    &mut data
                )
            );
        }

        buy_order.id += 1;
        sell_order.id += 1;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(buy_order, &mut data),
            E::PriceReachedMaxCountOfLimitOrders
        );
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(sell_order, &mut data),
            E::PriceReachedMaxCountOfLimitOrders
        );
    })
}

#[test]
#[ignore] // it works, but takes a lot of time
fn should_not_place_limit_order_that_doesnt_meet_restrictions_for_side() {
    ext().execute_with(|| {
        let mut data = CacheDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());
        OrderBookPallet::register_tech_account(DEX.into(), order_book_id).unwrap();
        let max_prices_for_side: u32 = <Runtime as Config>::MaxSidePriceCount::get();

        let mut buy_order = LimitOrder::<Runtime>::new(
            0,
            alice(),
            PriceVariant::Buy,
            balance!(1000),
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let mut sell_order = LimitOrder::<Runtime>::new(
            max_prices_for_side as u128 + 1000,
            alice(),
            PriceVariant::Sell,
            balance!(1001),
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        for i in 0..max_prices_for_side {
            // get new owner for each order to not get UserHasMaxCountOfOpenedOrders error
            let account = generate_account(i);

            fill_balance(account.clone(), order_book_id);

            buy_order.id += 1;
            buy_order.owner = account.clone();
            buy_order.price -= order_book.tick_size;

            sell_order.id += 1;
            sell_order.owner = account;
            sell_order.price += order_book.tick_size;

            assert_ok!(
                order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                    buy_order.clone(),
                    &mut data
                )
            );
            assert_ok!(
                order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                    sell_order.clone(),
                    &mut data
                )
            );
        }

        buy_order.id += 1;
        sell_order.id += 1;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(buy_order, &mut data),
            E::OrderBookReachedMaxCountOfPricesForSide
        );
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(sell_order, &mut data),
            E::OrderBookReachedMaxCountOfPricesForSide
        );
    })
}

#[test]
fn should_not_place_limit_order_that_doesnt_meet_restrictions_for_price() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        fill_balance(alice(), order_book_id);

        let max_price_shift = <Runtime as Config>::MAX_PRICE_SHIFT;

        // values from create_and_fill_order_book()
        let bes_bid_price = balance!(10);
        let bes_ask_price = balance!(11);

        let wrong_buy_price =
            bes_bid_price - max_price_shift * bes_bid_price - order_book.tick_size;
        let mut buy_order = LimitOrder::<Runtime>::new(
            101,
            alice(),
            PriceVariant::Buy,
            wrong_buy_price,
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let wrong_sell_price =
            bes_ask_price + max_price_shift * bes_ask_price + order_book.tick_size;
        let mut sell_order = LimitOrder::<Runtime>::new(
            102,
            alice(),
            PriceVariant::Sell,
            wrong_sell_price,
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                buy_order.clone(),
                &mut data
            ),
            E::InvalidLimitOrderPrice
        );
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(
                sell_order.clone(),
                &mut data
            ),
            E::InvalidLimitOrderPrice
        );

        // fix prices, now they are on the max distance from the spread
        buy_order.price = bes_bid_price - max_price_shift * bes_bid_price;
        sell_order.price = bes_ask_price + max_price_shift * bes_ask_price;

        assert_ok!(order_book
            .place_limit_order::<OrderBookPallet, OrderBookPallet>(buy_order.clone(), &mut data));
        assert_ok!(order_book
            .place_limit_order::<OrderBookPallet, OrderBookPallet>(sell_order.clone(), &mut data));
    })
}

#[test]
fn should_not_place_limit_order_in_spread() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let mut order_book = create_and_fill_order_book(order_book_id);

        let buy_price = balance!(11.1); // above the spread, in the asks zone
        let buy_order = LimitOrder::<Runtime>::new(
            101,
            alice(),
            PriceVariant::Buy,
            buy_price,
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let sell_price = balance!(9.9); // below the spread, in the bids zone
        let sell_order = LimitOrder::<Runtime>::new(
            102,
            alice(),
            PriceVariant::Sell,
            sell_price,
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // Stop & OnlyCancel statuses don't allow to place limit orders
        // Trade status should proceed another market mechanism
        // This test case is reachable only for PlaceAndCancel status
        order_book.status = OrderBookStatus::PlaceAndCancel;

        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(buy_order, &mut data),
            E::InvalidLimitOrderPrice
        );
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet, OrderBookPallet>(sell_order, &mut data),
            E::InvalidLimitOrderPrice
        );
    });
}

#[test]
fn should_cancel_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        let order = data.get_limit_order(&order_book_id, 5).unwrap();

        // fix state before
        let bids_before = data
            .get_bids(&order_book_id, &order.price)
            .unwrap_or_default();
        let agg_bids_before = data.get_aggregated_bids(&order_book_id);
        let price_volume_before = agg_bids_before
            .get(&order.price)
            .cloned()
            .unwrap_or_default();
        let user_orders_before = data
            .get_user_limit_orders(&order.owner, &order_book_id)
            .unwrap_or_default();
        let balance_before = free_balance(&order_book_id.quote, &order.owner);

        // cancel the limit order
        assert_ok!(order_book
            .cancel_limit_order::<OrderBookPallet, OrderBookPallet>(order.clone(), &mut data));

        let deal_amount = *order.deal_amount(MarketRole::Taker, None).unwrap().value();

        // check
        let mut expected_bids = bids_before.clone();
        expected_bids.retain(|&id| id != order.id);
        assert_eq!(
            data.get_bids(&order_book_id, &order.price).unwrap(),
            expected_bids
        );

        let expected_price_volume = price_volume_before - order.amount;
        let mut expected_agg_bids = agg_bids_before.clone();
        assert_ok!(expected_agg_bids.try_insert(order.price, expected_price_volume));
        assert_eq!(data.get_aggregated_bids(&order_book_id), expected_agg_bids);

        let mut expected_user_orders = user_orders_before.clone();
        expected_user_orders.retain(|&id| id != order.id);
        assert_eq!(
            data.get_user_limit_orders(&order.owner, &order_book_id)
                .unwrap(),
            expected_user_orders
        );

        let balance = free_balance(&order_book_id.quote, &order.owner);
        let expected_balance = balance_before + deal_amount;
        assert_eq!(balance, expected_balance);
    });
}

#[test]
fn should_not_cancel_unknown_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        let unknown_order = LimitOrder::<Runtime>::new(
            1234,
            alice(),
            PriceVariant::Sell,
            balance!(10),
            balance!(100),
            10,
            10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_err!(
            order_book
                .cancel_limit_order::<OrderBookPallet, OrderBookPallet>(unknown_order, &mut data),
            E::UnknownLimitOrder
        );
    });
}

#[test]
fn should_not_cancel_limit_order_when_status_doesnt_allow() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let mut order_book = create_and_fill_order_book(order_book_id);

        let order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        let order2 = data.get_limit_order(&order_book_id, 2).unwrap();
        let order3 = data.get_limit_order(&order_book_id, 3).unwrap();

        order_book.status = OrderBookStatus::Stop;
        assert_err!(
            order_book
                .cancel_limit_order::<OrderBookPallet, OrderBookPallet>(order1.clone(), &mut data),
            E::CancellationOfLimitOrdersIsForbidden
        );

        order_book.status = OrderBookStatus::Trade;
        assert_ok!(
            order_book.cancel_limit_order::<OrderBookPallet, OrderBookPallet>(order1, &mut data)
        );

        order_book.status = OrderBookStatus::PlaceAndCancel;
        assert_ok!(
            order_book.cancel_limit_order::<OrderBookPallet, OrderBookPallet>(order2, &mut data)
        );

        order_book.status = OrderBookStatus::OnlyCancel;
        assert_ok!(
            order_book.cancel_limit_order::<OrderBookPallet, OrderBookPallet>(order3, &mut data)
        );
    });
}

#[test]
fn should_cancel_all_limit_orders() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();
        let owner = bob();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        let tech_account = technical::Pallet::<Runtime>::tech_account_id_to_account_id(
            &OrderBookPallet::tech_account_for_order_book(DEX.into(), order_book_id.clone()),
        )
        .unwrap();

        // not empty at the beginning
        assert!(!data.get_all_limit_orders(&order_book_id).is_empty());
        assert!(!data.get_aggregated_bids(&order_book_id).is_empty());
        assert!(!data.get_aggregated_asks(&order_book_id).is_empty());
        assert!(!data
            .get_user_limit_orders(&owner, &order_book_id)
            .unwrap()
            .is_empty());

        // some balance is locked in limit orders
        assert_ne!(free_balance(&order_book_id.base, &owner), INIT_BALANCE);
        assert_ne!(free_balance(&order_book_id.quote, &owner), INIT_BALANCE);

        // tech account keeps the locked assets
        assert!(free_balance(&order_book_id.base, &tech_account) > balance!(0));
        assert!(free_balance(&order_book_id.quote, &tech_account) > balance!(0));

        // cancel all orders
        assert_ok!(
            order_book.cancel_all_limit_orders::<OrderBookPallet, OrderBookPallet>(&mut data)
        );

        // empty after canceling of all limit orders
        assert!(data.get_all_limit_orders(&order_book_id).is_empty());
        assert!(data.get_aggregated_bids(&order_book_id).is_empty());
        assert!(data.get_aggregated_asks(&order_book_id).is_empty());
        assert_eq!(data.get_user_limit_orders(&owner, &order_book_id), None);

        // locked balance is unlocked
        assert_eq!(free_balance(&order_book_id.base, &owner), INIT_BALANCE);
        assert_eq!(free_balance(&order_book_id.quote, &owner), INIT_BALANCE);

        // tech account balance is empty after canceling of all limit orders
        assert_eq!(
            free_balance(&order_book_id.base, &tech_account),
            balance!(0)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &tech_account),
            balance!(0)
        );
    });
}

#[test]
fn should_not_get_best_bid_from_empty_order_book() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_empty_order_book(order_book_id);

        assert_eq!(order_book.best_bid(&mut data), None);
    });
}

#[test]
fn should_get_best_bid() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        assert_eq!(
            order_book.best_bid(&mut data).unwrap(),
            (balance!(10), balance!(168.5))
        );
    });
}

#[test]
fn should_not_get_best_ask_from_empty_order_book() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_empty_order_book(order_book_id);

        assert_eq!(order_book.best_ask(&mut data), None);
    });
}

#[test]
fn should_get_best_ask() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        assert_eq!(
            order_book.best_ask(&mut data).unwrap(),
            (balance!(11), balance!(176.3))
        );
    });
}

#[test]
fn should_not_get_side_if_any_asset_is_not_in_order_book_id() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        assert_err!(order_book.get_side(&DOT, &KSM), E::InvalidAsset);
        assert_err!(order_book.get_side(&XOR, &KSM), E::InvalidAsset);
        assert_err!(order_book.get_side(&DOT, &VAL), E::InvalidAsset);
        assert_err!(order_book.get_side(&VAL, &VAL), E::InvalidAsset);
        assert_err!(order_book.get_side(&XOR, &XOR), E::InvalidAsset);
    });
}

#[test]
fn should_get_side() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        assert_eq!(order_book.get_side(&XOR, &VAL).unwrap(), PriceVariant::Buy);
        assert_eq!(order_book.get_side(&VAL, &XOR).unwrap(), PriceVariant::Sell);
    });
}

#[test]
fn should_align_amount() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_empty_order_book(order_book_id);

        // default step = 0.00001
        assert_eq!(order_book.align_amount(balance!(10.01)), balance!(10.01));
        assert_eq!(
            order_book.align_amount(balance!(10.00001)),
            balance!(10.00001)
        );
        assert_eq!(
            order_book.align_amount(balance!(10.000011)),
            balance!(10.00001)
        );
        assert_eq!(order_book.align_amount(balance!(10.000001)), balance!(10));
        assert_eq!(order_book.align_amount(balance!(10)), balance!(10));
        assert_eq!(
            order_book.align_amount(balance!(0.00001)),
            balance!(0.00001)
        );
        assert_eq!(order_book.align_amount(balance!(0.00000123)), balance!(0));
        assert_eq!(
            order_book.align_amount(balance!(9.999999999999)),
            balance!(9.99999)
        );
        assert_eq!(order_book.align_amount(balance!(0)), balance!(0));
    });
}

#[test]
fn should_align_nft_amount() {
    ext().execute_with(|| {
        let owner = alice();
        frame_system::Pallet::<Runtime>::inc_providers(&owner);

        let nft = assets::Pallet::<Runtime>::register_from(
            &owner,
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            balance!(1),
            false,
            None,
            None,
        )
        .unwrap();

        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            XOR,
            INIT_BALANCE.try_into().unwrap()
        ));

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: nft,
            quote: XOR.into(),
        };

        let order_book = OrderBook::<Runtime>::default_nft(order_book_id, DEX.into());

        // default nft step = 1
        assert_eq!(order_book.align_amount(balance!(10.01)), balance!(10));
        assert_eq!(order_book.align_amount(balance!(0.123456789)), balance!(0));
        assert_eq!(order_book.align_amount(balance!(1)), balance!(1));
        assert_eq!(order_book.align_amount(balance!(10)), balance!(10));
        assert_eq!(order_book.align_amount(balance!(0)), balance!(0));
    });
}

#[test]
fn should_not_sum_market_if_limit_is_greater_than_liquidity() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        let asks = data.get_aggregated_asks(&order_book_id);
        let bids = data.get_aggregated_bids(&order_book_id);

        assert_err!(
            order_book.sum_market(asks.iter(), Some(OrderAmount::Base(balance!(1000)))),
            E::NotEnoughLiquidity
        );
        assert_err!(
            order_book.sum_market(asks.iter(), Some(OrderAmount::Quote(balance!(10000)))),
            E::NotEnoughLiquidity
        );
        assert_err!(
            order_book.sum_market(bids.iter().rev(), Some(OrderAmount::Base(balance!(1000)))),
            E::NotEnoughLiquidity
        );
        assert_err!(
            order_book.sum_market(bids.iter().rev(), Some(OrderAmount::Quote(balance!(10000)))),
            E::NotEnoughLiquidity
        );
    });
}

#[test]
fn should_sum_market_with_zero_limit() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        let asks = data.get_aggregated_asks(&order_book_id);
        let bids = data.get_aggregated_bids(&order_book_id);

        assert_eq!(
            order_book
                .sum_market(asks.iter(), Some(OrderAmount::Base(balance!(0))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(0)),
                OrderAmount::Quote(balance!(0))
            )
        );
        assert_eq!(
            order_book
                .sum_market(asks.iter(), Some(OrderAmount::Quote(balance!(0))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(0)),
                OrderAmount::Quote(balance!(0))
            )
        );
        assert_eq!(
            order_book
                .sum_market(bids.iter().rev(), Some(OrderAmount::Base(balance!(0))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(0)),
                OrderAmount::Quote(balance!(0))
            )
        );
        assert_eq!(
            order_book
                .sum_market(bids.iter().rev(), Some(OrderAmount::Quote(balance!(0))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(0)),
                OrderAmount::Quote(balance!(0))
            )
        );
    });
}

#[test]
fn should_sum_market() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        let asks = data.get_aggregated_asks(&order_book_id);
        let bids = data.get_aggregated_bids(&order_book_id);

        // impacts 1 price
        assert_eq!(
            order_book
                .sum_market(asks.iter(), Some(OrderAmount::Base(balance!(100))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(100)),
                OrderAmount::Quote(balance!(1100))
            )
        );
        // impacts 2 prices
        assert_eq!(
            order_book
                .sum_market(asks.iter(), Some(OrderAmount::Base(balance!(200))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(200)),
                OrderAmount::Quote(balance!(2204.74))
            )
        );
        // impacts 3 prices
        assert_eq!(
            order_book
                .sum_market(asks.iter(), Some(OrderAmount::Base(balance!(400))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(400)),
                OrderAmount::Quote(balance!(4458.27))
            )
        );

        // impacts 1 price
        assert_eq!(
            order_book
                .sum_market(asks.iter(), Some(OrderAmount::Quote(balance!(1000))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(90.90909)),
                OrderAmount::Quote(balance!(999.99999))
            )
        );
        // impacts 2 prices
        assert_eq!(
            order_book
                .sum_market(asks.iter(), Some(OrderAmount::Quote(balance!(3000))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(271.00535)),
                OrderAmount::Quote(balance!(2999.99992))
            )
        );
        // impacts 3 prices
        assert_eq!(
            order_book
                .sum_market(asks.iter(), Some(OrderAmount::Quote(balance!(5000))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(447.10695)),
                OrderAmount::Quote(balance!(4999.999925))
            )
        );

        // impacts 1 price
        assert_eq!(
            order_book
                .sum_market(bids.iter().rev(), Some(OrderAmount::Base(balance!(100))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(100)),
                OrderAmount::Quote(balance!(1000))
            )
        );
        // impacts 2 prices
        assert_eq!(
            order_book
                .sum_market(bids.iter().rev(), Some(OrderAmount::Base(balance!(200))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(200)),
                OrderAmount::Quote(balance!(1993.7))
            )
        );
        // impacts 3 prices
        assert_eq!(
            order_book
                .sum_market(bids.iter().rev(), Some(OrderAmount::Base(balance!(400))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(400)),
                OrderAmount::Quote(balance!(3926.22))
            )
        );

        // impacts 1 price
        assert_eq!(
            order_book
                .sum_market(bids.iter().rev(), Some(OrderAmount::Quote(balance!(1000))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(100)),
                OrderAmount::Quote(balance!(1000))
            )
        );
        // impacts 2 prices
        assert_eq!(
            order_book
                .sum_market(bids.iter().rev(), Some(OrderAmount::Quote(balance!(2500))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(251.66326)),
                OrderAmount::Quote(balance!(2499.999948))
            )
        );
        // impacts 3 prices
        assert_eq!(
            order_book
                .sum_market(bids.iter().rev(), Some(OrderAmount::Quote(balance!(4500))))
                .unwrap(),
            (
                OrderAmount::Base(balance!(460.39789)),
                OrderAmount::Quote(balance!(4499.999955))
            )
        );

        // without depth limit
        assert_eq!(
            order_book.sum_market(asks.iter(), None).unwrap(),
            (
                OrderAmount::Base(balance!(610.7)),
                OrderAmount::Quote(balance!(6881.32))
            )
        );
        assert_eq!(
            order_book.sum_market(bids.iter().rev(), None).unwrap(),
            (
                OrderAmount::Base(balance!(569.7)),
                OrderAmount::Quote(balance!(5538.37))
            )
        );

        // base is aligned
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Base(balance!(200.123456789)))
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(200.12345)),
                OrderAmount::Quote(balance!(2206.12264))
            )
        );

        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Base(balance!(200.00000123)))
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(200)),
                OrderAmount::Quote(balance!(1993.7))
            )
        );
    });
}

#[test]
fn should_not_calculate_deal_with_small_amount() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        assert_err!(
            order_book.calculate_deal(
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(balance!(0.000001)),
                &mut data
            ),
            E::InvalidOrderAmount
        );
        assert_err!(
            order_book.calculate_deal(
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(0.000001)),
                &mut data
            ),
            E::InvalidOrderAmount
        );
        assert_err!(
            order_book.calculate_deal(
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(balance!(0.000001)),
                &mut data
            ),
            E::InvalidOrderAmount
        );
        assert_err!(
            order_book.calculate_deal(
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(balance!(0.000001)),
                &mut data
            ),
            E::InvalidOrderAmount
        );
    });
}

#[test]
fn should_calculate_deal() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        assert_eq!(
            order_book
                .calculate_deal(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_input(balance!(3000)),
                    &mut data
                )
                .unwrap(),
            DealInfo::<AssetIdOf<Runtime>> {
                input_asset_id: XOR,
                input_amount: OrderAmount::Quote(balance!(2999.99992)),
                output_asset_id: VAL,
                output_amount: OrderAmount::Base(balance!(271.00535)),
                average_price: balance!(11.069891867448373251),
                side: PriceVariant::Buy
            }
        );
        assert_eq!(
            order_book
                .calculate_deal(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_output(balance!(200)),
                    &mut data
                )
                .unwrap(),
            DealInfo::<AssetIdOf<Runtime>> {
                input_asset_id: XOR,
                input_amount: OrderAmount::Quote(balance!(2204.74)),
                output_asset_id: VAL,
                output_amount: OrderAmount::Base(balance!(200)),
                average_price: balance!(11.0237),
                side: PriceVariant::Buy
            }
        );
        assert_eq!(
            order_book
                .calculate_deal(
                    &VAL,
                    &XOR,
                    QuoteAmount::with_desired_input(balance!(200)),
                    &mut data
                )
                .unwrap(),
            DealInfo::<AssetIdOf<Runtime>> {
                input_asset_id: VAL,
                input_amount: OrderAmount::Base(balance!(200)),
                output_asset_id: XOR,
                output_amount: OrderAmount::Quote(balance!(1993.7)),
                average_price: balance!(9.9685),
                side: PriceVariant::Sell
            }
        );
        assert_eq!(
            order_book
                .calculate_deal(
                    &VAL,
                    &XOR,
                    QuoteAmount::with_desired_output(balance!(2500)),
                    &mut data
                )
                .unwrap(),
            DealInfo::<AssetIdOf<Runtime>> {
                input_asset_id: VAL,
                input_amount: OrderAmount::Base(balance!(251.66326)),
                output_asset_id: XOR,
                output_amount: OrderAmount::Quote(balance!(2499.999948)),
                average_price: balance!(9.933909097418510751),
                side: PriceVariant::Sell
            }
        );
    });
}

#[test]
fn should_not_execute_market_order_with_non_trade_status() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let mut order_book = create_and_fill_order_book(order_book_id);

        let order = MarketOrder::<Runtime>::new(
            alice(),
            PriceVariant::Buy,
            order_book_id,
            balance!(10),
            None,
        );

        order_book.status = OrderBookStatus::PlaceAndCancel;
        assert_err!(
            order_book.execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                order.clone(),
                &mut data
            ),
            E::TradingIsForbidden
        );

        order_book.status = OrderBookStatus::OnlyCancel;
        assert_err!(
            order_book.execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                order.clone(),
                &mut data
            ),
            E::TradingIsForbidden
        );

        order_book.status = OrderBookStatus::Stop;
        assert_err!(
            order_book.execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                order.clone(),
                &mut data
            ),
            E::TradingIsForbidden
        );
    });
}

#[test]
fn should_not_execute_market_order_with_empty_amount() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        let wrong_amount = balance!(0);
        let order = MarketOrder::<Runtime>::new(
            alice(),
            PriceVariant::Buy,
            order_book_id,
            wrong_amount,
            None,
        );

        assert_err!(
            order_book.execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                order, &mut data
            ),
            E::InvalidOrderAmount
        );
    });
}

#[test]
fn should_not_execute_market_order_with_invalid_order_book_id() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let wrong_order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: DOT.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        let order = MarketOrder::<Runtime>::new(
            alice(),
            PriceVariant::Buy,
            wrong_order_book_id,
            balance!(100),
            None,
        );

        assert_err!(
            order_book.execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                order, &mut data
            ),
            E::InvalidOrderBookId
        );
    });
}

#[test]
fn should_not_execute_market_order_with_invalid_amount() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        let wrong_amount = balance!(1.123456);
        let order = MarketOrder::<Runtime>::new(
            alice(),
            PriceVariant::Buy,
            order_book_id,
            wrong_amount,
            None,
        );

        assert_err!(
            order_book.execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                order, &mut data
            ),
            E::InvalidOrderAmount
        );
    });
}

#[test]
fn should_execute_market_order_and_transfer_to_owner() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        fill_balance(alice(), order_book_id);

        let mut alice_base_balance = free_balance(&order_book_id.base, &alice());
        let mut alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        let mut bob_base_balance = free_balance(&order_book_id.base, &bob());
        let mut bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        let mut charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        let mut charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        let mut buy_order = MarketOrder::<Runtime>::new(
            alice(),
            PriceVariant::Buy,
            order_book_id,
            balance!(150),
            None,
        );
        let mut sell_order = MarketOrder::<Runtime>::new(
            alice(),
            PriceVariant::Sell,
            order_book_id,
            balance!(150),
            None,
        );

        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11), balance!(176.3)),
                (balance!(11.2), balance!(178.6)),
                (balance!(11.5), balance!(255.8))
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(139.9)),
                (balance!(10), balance!(168.5))
            ])
        );

        // 1st buy order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                    buy_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1650)),
                OrderAmount::Base(balance!(150))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(1650)
        );
        assert_eq!(free_balance(&order_book_id.base, &bob()), bob_base_balance);
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance + balance!(1650)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11), balance!(26.3)),
                (balance!(11.2), balance!(178.6)),
                (balance!(11.5), balance!(255.8))
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(139.9)),
                (balance!(10), balance!(168.5))
            ])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        // 2nd buy order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                    buy_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1674.74)),
                OrderAmount::Base(balance!(150))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(1674.74)
        );
        assert_eq!(free_balance(&order_book_id.base, &bob()), bob_base_balance);
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance + balance!(718.26)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance + balance!(956.48)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11.2), balance!(54.9)),
                (balance!(11.5), balance!(255.8))
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(139.9)),
                (balance!(10), balance!(168.5))
            ])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        // 3rd buy order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                    buy_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1708.53)),
                OrderAmount::Base(balance!(150))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(1708.53)
        );
        assert_eq!(free_balance(&order_book_id.base, &bob()), bob_base_balance);
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance + balance!(1287.63)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance + balance!(420.9)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5), balance!(160.7))])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(139.9)),
                (balance!(10), balance!(168.5))
            ])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        // 1st sell order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                    sell_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(150)),
                OrderAmount::Quote(balance!(1500))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance + balance!(1500)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &bob()),
            bob_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5), balance!(160.7))])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(139.9)),
                (balance!(10), balance!(18.5))
            ])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        // 2nd sell order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                    sell_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(150)),
                OrderAmount::Quote(balance!(1473.7))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance + balance!(1473.7)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &bob()),
            bob_base_balance + balance!(54.8)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance + balance!(95.2)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5), balance!(160.7))])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(8.4)),
            ])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        // 3rd sell order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                    sell_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(150)),
                OrderAmount::Quote(balance!(1427.52))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance + balance!(1427.52)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &bob()),
            bob_base_balance + balance!(93.6)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance + balance!(56.4)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5), balance!(160.7))])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(balance!(9.5), balance!(119.7)),])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        // buy & sell remaining amounts
        buy_order.amount = balance!(160.7);
        sell_order.amount = balance!(119.7);

        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                    buy_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1848.05)),
                OrderAmount::Base(balance!(160.7))
            )
        );
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet, OrderBookPallet>(
                    sell_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(119.7)),
                OrderAmount::Quote(balance!(1137.15))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance + balance!(160.7) - balance!(119.7)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(1848.05) + balance!(1137.15)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &bob()),
            bob_base_balance + balance!(4.7)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance + balance!(1690.5)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance + balance!(115)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance + balance!(157.55)
        );
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_bids(&order_book_id), BTreeMap::from([]));
    });
}

#[test]
// In this test `Alice` spends assets on market orders, but `Dave` receives the deal result amounts
fn should_execute_market_order_and_transfer_to_another_account() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        fill_balance(alice(), order_book_id);

        let mut alice_base_balance = free_balance(&order_book_id.base, &alice());
        let mut alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        let mut bob_base_balance = free_balance(&order_book_id.base, &bob());
        let mut bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        let mut charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        let mut charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        let mut dave_base_balance = free_balance(&order_book_id.base, &dave());
        let mut dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        let mut buy_order = MarketOrder::<Runtime>::new(
            alice(),
            PriceVariant::Buy,
            order_book_id,
            balance!(150),
            Some(dave()),
        );
        let mut sell_order = MarketOrder::<Runtime>::new(
            alice(),
            PriceVariant::Sell,
            order_book_id,
            balance!(150),
            Some(dave()),
        );

        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11), balance!(176.3)),
                (balance!(11.2), balance!(178.6)),
                (balance!(11.5), balance!(255.8))
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(139.9)),
                (balance!(10), balance!(168.5))
            ])
        );

        // 1st buy order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet>(
                    buy_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1650)),
                OrderAmount::Base(balance!(150))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(1650)
        );
        assert_eq!(free_balance(&order_book_id.base, &bob()), bob_base_balance);
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance + balance!(1650)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11), balance!(26.3)),
                (balance!(11.2), balance!(178.6)),
                (balance!(11.5), balance!(255.8))
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(139.9)),
                (balance!(10), balance!(168.5))
            ])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        dave_base_balance = free_balance(&order_book_id.base, &dave());
        dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        // 2nd buy order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet>(
                    buy_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1674.74)),
                OrderAmount::Base(balance!(150))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(1674.74)
        );
        assert_eq!(free_balance(&order_book_id.base, &bob()), bob_base_balance);
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance + balance!(718.26)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance + balance!(956.48)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11.2), balance!(54.9)),
                (balance!(11.5), balance!(255.8))
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(139.9)),
                (balance!(10), balance!(168.5))
            ])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        dave_base_balance = free_balance(&order_book_id.base, &dave());
        dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        // 3rd buy order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet>(
                    buy_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1708.53)),
                OrderAmount::Base(balance!(150))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(1708.53)
        );
        assert_eq!(free_balance(&order_book_id.base, &bob()), bob_base_balance);
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance + balance!(1287.63)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance + balance!(420.9)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5), balance!(160.7))])
        );
        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(139.9)),
                (balance!(10), balance!(168.5))
            ])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        dave_base_balance = free_balance(&order_book_id.base, &dave());
        dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        // 1st sell order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet>(
                    sell_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(150)),
                OrderAmount::Quote(balance!(1500))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &bob()),
            bob_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance + balance!(1500)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5), balance!(160.7))])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(139.9)),
                (balance!(10), balance!(18.5))
            ])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        dave_base_balance = free_balance(&order_book_id.base, &dave());
        dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        // 2nd sell order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet>(
                    sell_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(150)),
                OrderAmount::Quote(balance!(1473.7))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &bob()),
            bob_base_balance + balance!(54.8)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance + balance!(95.2)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance + balance!(1473.7)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5), balance!(160.7))])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5), balance!(261.3)),
                (balance!(9.8), balance!(8.4)),
            ])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        dave_base_balance = free_balance(&order_book_id.base, &dave());
        dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        // 3rd sell order
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet>(
                    sell_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(150)),
                OrderAmount::Quote(balance!(1427.52))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &bob()),
            bob_base_balance + balance!(93.6)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance + balance!(56.4)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance
        );
        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance + balance!(1427.52)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5), balance!(160.7))])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(balance!(9.5), balance!(119.7)),])
        );

        alice_base_balance = free_balance(&order_book_id.base, &alice());
        alice_quote_balance = free_balance(&order_book_id.quote, &alice());

        bob_base_balance = free_balance(&order_book_id.base, &bob());
        bob_quote_balance = free_balance(&order_book_id.quote, &bob());

        charlie_base_balance = free_balance(&order_book_id.base, &charlie());
        charlie_quote_balance = free_balance(&order_book_id.quote, &charlie());

        dave_base_balance = free_balance(&order_book_id.base, &dave());
        dave_quote_balance = free_balance(&order_book_id.quote, &dave());

        // buy & sell remaining amounts
        buy_order.amount = balance!(160.7);
        sell_order.amount = balance!(119.7);

        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet>(
                    buy_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1848.05)),
                OrderAmount::Base(balance!(160.7))
            )
        );
        assert_eq!(
            order_book
                .execute_market_order::<OrderBookPallet, OrderBookPallet>(
                    sell_order.clone(),
                    &mut data
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(119.7)),
                OrderAmount::Quote(balance!(1137.15))
            )
        );
        assert_eq!(
            free_balance(&order_book_id.base, &alice()),
            alice_base_balance - balance!(119.7)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &alice()),
            alice_quote_balance - balance!(1848.05)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &bob()),
            bob_base_balance + balance!(4.7)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &bob()),
            bob_quote_balance + balance!(1690.5)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &charlie()),
            charlie_base_balance + balance!(115)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &charlie()),
            charlie_quote_balance + balance!(157.55)
        );
        assert_eq!(
            free_balance(&order_book_id.base, &dave()),
            dave_base_balance + balance!(160.7)
        );
        assert_eq!(
            free_balance(&order_book_id.quote, &dave()),
            dave_quote_balance + balance!(1137.15)
        );
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_bids(&order_book_id), BTreeMap::from([]));
    });
}

#[test]
fn should_not_calculate_market_impact_with_empty_side() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_empty_order_book(order_book_id);

        assert_err!(
            order_book.calculate_market_impact(
                PriceVariant::Buy,
                balance!(1),
                data.get_aggregated_asks(&order_book_id).iter(),
                &mut data
            ),
            E::NotEnoughLiquidity
        );
        assert_err!(
            order_book.calculate_market_impact(
                PriceVariant::Sell,
                balance!(1),
                data.get_aggregated_bids(&order_book_id).iter().rev(),
                &mut data
            ),
            E::NotEnoughLiquidity
        );
    });
}

#[test]
fn should_not_calculate_market_impact_if_liquidity_is_not_enough() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);

        assert_err!(
            order_book.calculate_market_impact(
                PriceVariant::Buy,
                balance!(1000),
                data.get_aggregated_asks(&order_book_id).iter(),
                &mut data
            ),
            E::NotEnoughLiquidity
        );
        assert_err!(
            order_book.calculate_market_impact(
                PriceVariant::Sell,
                balance!(1000),
                data.get_aggregated_bids(&order_book_id).iter().rev(),
                &mut data
            ),
            E::NotEnoughLiquidity
        );
    });
}

#[test]
fn should_calculate_market_impact() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_book = create_and_fill_order_book(order_book_id);
        let expiration_block = 3;

        let market_for_buy = data.get_aggregated_asks(&order_book_id);
        let market_for_sell = data.get_aggregated_bids(&order_book_id);

        let buy_amount1 = balance!(100);
        let buy_amount2 = balance!(300);
        let buy_amount3 = balance!(600);
        let buy_amount4 = balance!(391.5);
        let buy_amount5 = balance!(610.7);

        let mut limit_order7 = data.get_limit_order(&order_book_id, 7).unwrap();
        limit_order7.amount -= buy_amount1;
        assert_eq!(
            order_book
                .calculate_market_impact(
                    PriceVariant::Buy,
                    buy_amount1,
                    market_for_buy.iter(),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                market_input: OrderAmount::Quote(balance!(1100)),
                market_output: OrderAmount::Base(buy_amount1),
                to_delete: vec![],
                to_update: vec![limit_order7],
                makers_output: BTreeMap::from([(bob(), balance!(1100))])
            }
        );

        let mut limit_order9 = data.get_limit_order(&order_book_id, 9).unwrap();
        limit_order9.amount -= balance!(38.3);
        assert_eq!(
            order_book
                .calculate_market_impact(
                    PriceVariant::Buy,
                    buy_amount2,
                    market_for_buy.iter(),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                market_input: OrderAmount::Quote(balance!(3324.74)),
                market_output: OrderAmount::Base(buy_amount2),
                to_delete: vec![(7, expiration_block), (8, expiration_block)],
                to_update: vec![limit_order9],
                makers_output: BTreeMap::from([
                    (bob(), balance!(2368.26)),
                    (charlie(), balance!(956.48))
                ])
            }
        );

        let mut limit_order12 = data.get_limit_order(&order_book_id, 12).unwrap();
        limit_order12.amount -= balance!(3);
        assert_eq!(
            order_book
                .calculate_market_impact(
                    PriceVariant::Buy,
                    buy_amount3,
                    market_for_buy.iter(),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                market_input: OrderAmount::Quote(balance!(6758.27)),
                market_output: OrderAmount::Base(buy_amount3),
                to_delete: vec![
                    (7, expiration_block),
                    (8, expiration_block),
                    (9, expiration_block),
                    (10, expiration_block),
                    (11, expiration_block)
                ],
                to_update: vec![limit_order12],
                makers_output: BTreeMap::from([
                    (bob(), balance!(5346.39)),
                    (charlie(), balance!(1411.88))
                ])
            }
        );

        assert_eq!(
            order_book
                .calculate_market_impact(
                    PriceVariant::Buy,
                    buy_amount4,
                    market_for_buy.iter(),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                market_input: OrderAmount::Quote(balance!(4360.52)),
                market_output: OrderAmount::Base(buy_amount4),
                to_delete: vec![
                    (7, expiration_block),
                    (8, expiration_block),
                    (9, expiration_block),
                    (10, expiration_block)
                ],
                to_update: vec![],
                makers_output: BTreeMap::from([
                    (bob(), balance!(2983.14)),
                    (charlie(), balance!(1377.38))
                ])
            }
        );

        assert_eq!(
            order_book
                .calculate_market_impact(
                    PriceVariant::Buy,
                    buy_amount5,
                    market_for_buy.iter(),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                market_input: OrderAmount::Quote(balance!(6881.32)),
                market_output: OrderAmount::Base(buy_amount5),
                to_delete: vec![
                    (7, expiration_block),
                    (8, expiration_block),
                    (9, expiration_block),
                    (10, expiration_block),
                    (11, expiration_block),
                    (12, expiration_block)
                ],
                to_update: vec![],
                makers_output: BTreeMap::from([
                    (bob(), balance!(5346.39)),
                    (charlie(), balance!(1534.93))
                ])
            }
        );

        let sell_amount1 = balance!(100);
        let sell_amount2 = balance!(270);
        let sell_amount3 = balance!(400);
        let sell_amount4 = balance!(364.8);
        let sell_amount5 = balance!(569.7);

        let mut limit_order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        limit_order1.amount -= buy_amount1;
        assert_eq!(
            order_book
                .calculate_market_impact(
                    PriceVariant::Sell,
                    sell_amount1,
                    market_for_sell.iter().rev(),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                market_input: OrderAmount::Base(sell_amount1),
                market_output: OrderAmount::Quote(balance!(1000)),
                to_delete: vec![],
                to_update: vec![limit_order1],
                makers_output: BTreeMap::from([(bob(), sell_amount1)])
            }
        );

        let mut limit_order3 = data.get_limit_order(&order_book_id, 3).unwrap();
        limit_order3.amount -= balance!(6.3);
        assert_eq!(
            order_book
                .calculate_market_impact(
                    PriceVariant::Sell,
                    sell_amount2,
                    market_for_sell.iter().rev(),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                market_input: OrderAmount::Base(sell_amount2),
                market_output: OrderAmount::Quote(balance!(2679.7)),
                to_delete: vec![(1, expiration_block), (2, expiration_block)],
                to_update: vec![limit_order3],
                makers_output: BTreeMap::from([
                    (bob(), balance!(174.8)),
                    (charlie(), balance!(95.2))
                ])
            }
        );

        let mut limit_order5 = data.get_limit_order(&order_book_id, 5).unwrap();
        limit_order5.amount -= balance!(35.2);
        assert_eq!(
            order_book
                .calculate_market_impact(
                    PriceVariant::Sell,
                    sell_amount3,
                    market_for_sell.iter().rev(),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                market_input: OrderAmount::Base(sell_amount3),
                market_output: OrderAmount::Quote(balance!(3926.22)),
                to_delete: vec![
                    (1, expiration_block),
                    (2, expiration_block),
                    (3, expiration_block),
                    (4, expiration_block)
                ],
                to_update: vec![limit_order5],
                makers_output: BTreeMap::from([
                    (bob(), balance!(248.4)),
                    (charlie(), balance!(151.6))
                ])
            }
        );

        assert_eq!(
            order_book
                .calculate_market_impact(
                    PriceVariant::Sell,
                    sell_amount4,
                    market_for_sell.iter().rev(),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                market_input: OrderAmount::Base(sell_amount4),
                market_output: OrderAmount::Quote(balance!(3591.82)),
                to_delete: vec![
                    (1, expiration_block),
                    (2, expiration_block),
                    (3, expiration_block),
                    (4, expiration_block)
                ],
                to_update: vec![],
                makers_output: BTreeMap::from([
                    (bob(), balance!(213.2)),
                    (charlie(), balance!(151.6))
                ])
            }
        );

        assert_eq!(
            order_book
                .calculate_market_impact(
                    PriceVariant::Sell,
                    sell_amount5,
                    market_for_sell.iter().rev(),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                market_input: OrderAmount::Base(sell_amount5),
                market_output: OrderAmount::Quote(balance!(5538.37)),
                to_delete: vec![
                    (1, expiration_block),
                    (2, expiration_block),
                    (3, expiration_block),
                    (4, expiration_block),
                    (5, expiration_block),
                    (6, expiration_block)
                ],
                to_update: vec![],
                makers_output: BTreeMap::from([
                    (bob(), balance!(303.1)),
                    (charlie(), balance!(266.6))
                ])
            }
        );
    });
}
