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
use common::{balance, AssetInfoProvider, AssetName, AssetSymbol, PriceVariant, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::order_book::cache_data_layer::CacheDataLayer;
use framenode_runtime::order_book::storage_data_layer::StorageDataLayer;
use framenode_runtime::order_book::{
    Config, DataLayer, LimitOrder, OrderBook, OrderBookId, OrderBookStatus,
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
        let balance_before =
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &owner)
                .unwrap();

        // new order
        let order = LimitOrder::<Runtime>::new(
            order_id,
            owner.clone(),
            PriceVariant::Buy,
            price,
            amount,
            10,
            10000,
        );

        let appropriate_amount = order.appropriate_amount().unwrap();

        // place new order
        assert_ok!(order_book.place_limit_order::<OrderBookPallet>(order, &mut data));

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

        let balance =
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &owner)
                .unwrap();
        let expected_balance = balance_before - appropriate_amount;
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
        );

        // place new order
        assert_ok!(order_book.place_limit_order::<OrderBookPallet>(order, &mut data));

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

        let balance =
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &owner)
                .unwrap();
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
        );

        order_book.status = OrderBookStatus::Stop;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(order.clone(), &mut data),
            E::PlacementOfLimitOrdersIsForbidden
        );

        order_book.status = OrderBookStatus::OnlyCancel;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(order.clone(), &mut data),
            E::PlacementOfLimitOrdersIsForbidden
        );

        order_book.status = OrderBookStatus::PlaceAndCancel;
        assert_ok!(order_book.place_limit_order::<OrderBookPallet>(order.clone(), &mut data));

        order_book.status = OrderBookStatus::Trade;
        order.id = 2;
        assert_ok!(order_book.place_limit_order::<OrderBookPallet>(order.clone(), &mut data));
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
        );

        let mut wrong_price_order = order.clone();
        wrong_price_order.price = balance!(10) + order_book.tick_size / 100;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(wrong_price_order, &mut data),
            E::InvalidLimitOrderPrice
        );

        let mut too_small_amount_order = order.clone();
        too_small_amount_order.amount = order_book.min_lot_size / 2;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(too_small_amount_order, &mut data),
            E::InvalidOrderAmount
        );

        let mut too_big_amount_order = order.clone();
        too_big_amount_order.amount = order_book.max_lot_size + 1;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(too_big_amount_order, &mut data),
            E::InvalidOrderAmount
        );

        let mut wrong_amount_order = order.clone();
        wrong_amount_order.amount = balance!(100) + order_book.step_lot_size / 100;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(wrong_amount_order, &mut data),
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
        );

        let mut wrong_price_order = order.clone();
        wrong_price_order.price = balance!(10) + order_book.tick_size / 100;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(wrong_price_order, &mut data),
            E::InvalidLimitOrderPrice
        );

        let mut too_small_amount_order = order.clone();
        too_small_amount_order.amount = balance!(0.5);
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(too_small_amount_order, &mut data),
            E::InvalidOrderAmount
        );

        let mut too_big_amount_order = order.clone();
        too_big_amount_order.amount = order_book.max_lot_size + 1;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(too_big_amount_order, &mut data),
            E::InvalidOrderAmount
        );

        let mut wrong_amount_order = order.clone();
        wrong_amount_order.amount = balance!(1) - order_book.step_lot_size / 100;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(wrong_amount_order, &mut data),
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
        );

        let max_orders_per_user: u32 = <Runtime as Config>::MaxOpenedLimitOrdersPerUser::get();

        for _ in 0..max_orders_per_user {
            order.id += 1;
            order.price += balance!(0.001);
            assert_ok!(order_book.place_limit_order::<OrderBookPallet>(order.clone(), &mut data));
        }

        order.id += 1;
        order.price += balance!(0.001);
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(order, &mut data),
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
        );

        let mut sell_order = LimitOrder::<Runtime>::new(
            max_orders_for_price as u128 + 1000,
            alice(),
            PriceVariant::Sell,
            balance!(11),
            balance!(100),
            10,
            10000,
        );

        for i in 0..max_orders_for_price {
            // get new owner for each order to not get UserHasMaxCountOfOpenedOrders error
            let account = generate_account(i);

            fill_balance(account.clone(), order_book_id);

            buy_order.id += 1;
            buy_order.owner = account.clone();
            sell_order.id += 1;
            sell_order.owner = account;

            assert_ok!(
                order_book.place_limit_order::<OrderBookPallet>(buy_order.clone(), &mut data)
            );
            assert_ok!(
                order_book.place_limit_order::<OrderBookPallet>(sell_order.clone(), &mut data)
            );
        }

        buy_order.id += 1;
        sell_order.id += 1;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(buy_order, &mut data),
            E::PriceReachedMaxCountOfLimitOrders
        );
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(sell_order, &mut data),
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
        );

        let mut sell_order = LimitOrder::<Runtime>::new(
            max_prices_for_side as u128 + 1000,
            alice(),
            PriceVariant::Sell,
            balance!(1001),
            balance!(100),
            10,
            10000,
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
                order_book.place_limit_order::<OrderBookPallet>(buy_order.clone(), &mut data)
            );
            assert_ok!(
                order_book.place_limit_order::<OrderBookPallet>(sell_order.clone(), &mut data)
            );
        }

        buy_order.id += 1;
        sell_order.id += 1;
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(buy_order, &mut data),
            E::OrderBookReachedMaxCountOfPricesForSide
        );
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(sell_order, &mut data),
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
        );

        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(buy_order.clone(), &mut data),
            E::InvalidLimitOrderPrice
        );
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(sell_order.clone(), &mut data),
            E::InvalidLimitOrderPrice
        );

        // fix prices, now they are on the max distance from the spread
        buy_order.price = bes_bid_price - max_price_shift * bes_bid_price;
        sell_order.price = bes_ask_price + max_price_shift * bes_ask_price;

        assert_ok!(order_book.place_limit_order::<OrderBookPallet>(buy_order.clone(), &mut data));
        assert_ok!(order_book.place_limit_order::<OrderBookPallet>(sell_order.clone(), &mut data));
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
        );

        // Stop & OnlyCancel statuses don't allow to place limit orders
        // Trade status should proceed another market mechanism
        // This test case is reachable only for PlaceAndCancel status
        order_book.status = OrderBookStatus::PlaceAndCancel;

        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(buy_order, &mut data),
            E::InvalidLimitOrderPrice
        );
        assert_err!(
            order_book.place_limit_order::<OrderBookPallet>(sell_order, &mut data),
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
        let balance_before = <Runtime as Config>::AssetInfoProvider::free_balance(
            &order_book_id.quote,
            &order.owner,
        )
        .unwrap();

        // cancel the limit order
        assert_ok!(order_book.cancel_limit_order::<OrderBookPallet>(order.clone(), &mut data));

        let appropriate_amount = order.appropriate_amount().unwrap();

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

        let balance = <Runtime as Config>::AssetInfoProvider::free_balance(
            &order_book_id.quote,
            &order.owner,
        )
        .unwrap();
        let expected_balance = balance_before + appropriate_amount;
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
        );

        assert_err!(
            order_book.cancel_limit_order::<OrderBookPallet>(unknown_order, &mut data),
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
            order_book.cancel_limit_order::<OrderBookPallet>(order1.clone(), &mut data),
            E::CancellationOfLimitOrdersIsForbidden
        );

        order_book.status = OrderBookStatus::Trade;
        assert_ok!(order_book.cancel_limit_order::<OrderBookPallet>(order1, &mut data));

        order_book.status = OrderBookStatus::PlaceAndCancel;
        assert_ok!(order_book.cancel_limit_order::<OrderBookPallet>(order2, &mut data));

        order_book.status = OrderBookStatus::OnlyCancel;
        assert_ok!(order_book.cancel_limit_order::<OrderBookPallet>(order3, &mut data));
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

        // not empty at the beginning
        assert!(!data.get_all_limit_orders(&order_book_id).is_empty());
        assert!(!data.get_aggregated_bids(&order_book_id).is_empty());
        assert!(!data.get_aggregated_asks(&order_book_id).is_empty());
        assert!(!data
            .get_user_limit_orders(&owner, &order_book_id)
            .unwrap()
            .is_empty());

        // some balance is locked in limit orders
        assert_ne!(
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &owner)
                .unwrap(),
            INIT_BALANCE
        );
        assert_ne!(
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &owner)
                .unwrap(),
            INIT_BALANCE
        );

        // cancel all orders
        assert_ok!(order_book.cancel_all_limit_orders::<OrderBookPallet>(&mut data));

        // empty after canceling of all limit orders
        assert!(data.get_all_limit_orders(&order_book_id).is_empty());
        assert!(data.get_aggregated_bids(&order_book_id).is_empty());
        assert!(data.get_aggregated_asks(&order_book_id).is_empty());
        assert_eq!(data.get_user_limit_orders(&owner, &order_book_id), None);

        // locked balance is unlocked
        assert_eq!(
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.base, &owner)
                .unwrap(),
            INIT_BALANCE
        );
        assert_eq!(
            <Runtime as Config>::AssetInfoProvider::free_balance(&order_book_id.quote, &owner)
                .unwrap(),
            INIT_BALANCE
        );
    });
}
