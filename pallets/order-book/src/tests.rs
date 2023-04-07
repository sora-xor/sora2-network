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

use common::{balance, PriceVariant, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::order_book::{LimitOrder, OrderBookId, Pallet};
use framenode_runtime::{order_book, Runtime};

type OrderBook = Pallet<Runtime>;

fn alice() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([1u8; 32])
}

type E = order_book::Error<Runtime>;

#[test]
fn insert_limit_order_success() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: VAL.into(),
        };

        let order_buy_id = 1;
        let order_sell_id = 2;
        let owner = alice();
        let price = balance!(12);

        let order_buy = LimitOrder::<Runtime> {
            id: order_buy_id,
            owner: owner.clone(),
            side: PriceVariant::Buy,
            price: price,
            original_amount: balance!(10),
            executed_amount: balance!(0),
            time: 10,
            lifespan: 1000,
        };

        let order_sell = LimitOrder::<Runtime> {
            id: order_sell_id,
            owner: owner.clone(),
            side: PriceVariant::Sell,
            price: price,
            original_amount: balance!(10),
            executed_amount: balance!(0),
            time: 10,
            lifespan: 1000,
        };

        assert_ok!(OrderBook::insert_limit_order(&order_book_id, &order_buy));
        assert_eq!(
            OrderBook::limit_orders(order_book_id, order_buy_id).unwrap(),
            order_buy
        );
        assert_eq!(
            OrderBook::bids(order_book_id, price).unwrap(),
            vec![order_buy_id]
        );
        assert_eq!(OrderBook::asks(order_book_id, price), None);
        assert_eq!(
            OrderBook::user_limit_orders(&owner, order_book_id).unwrap(),
            vec![order_buy_id]
        );

        assert_ok!(OrderBook::insert_limit_order(&order_book_id, &order_sell));
        assert_eq!(
            OrderBook::limit_orders(order_book_id, order_sell_id).unwrap(),
            order_sell
        );
        assert_eq!(
            OrderBook::bids(order_book_id, price).unwrap(),
            vec![order_buy_id]
        );
        assert_eq!(
            OrderBook::asks(order_book_id, price).unwrap(),
            vec![order_sell_id]
        );
        assert_eq!(
            OrderBook::user_limit_orders(&owner, order_book_id).unwrap(),
            vec![order_buy_id, order_sell_id]
        );
    });
}

#[test]
fn insert_limit_order_fail() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: VAL.into(),
        };

        let order_id = 1;
        let owner = alice();
        let price = balance!(12);

        let order = LimitOrder::<Runtime> {
            id: order_id,
            owner: owner.clone(),
            side: PriceVariant::Sell,
            price: price,
            original_amount: balance!(10),
            executed_amount: balance!(0),
            time: 10,
            lifespan: 1000,
        };

        // Take actual values from `impl order_book::Config for Runtime`
        // =min(MaxOpenedLimitOrdersForAllOrderBooksPerUser, MaxLimitOrdersForPrice)
        let max = 10000;

        for _ in 0..max {
            assert_ok!(OrderBook::insert_limit_order(&order_book_id, &order));
        }

        // Error if storage overflow
        assert_err!(
            OrderBook::insert_limit_order(&order_book_id, &order),
            E::InsertLimitOrderError
        );
    });
}

#[test]
fn delete_limit_order_success() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: VAL.into(),
        };

        let order_buy_id1 = 1;
        let order_buy_id2 = 2;
        let order_sell_id1 = 3;
        let order_sell_id2 = 4;
        let owner = alice();
        let price = balance!(12);

        let order_buy1 = LimitOrder::<Runtime> {
            id: order_buy_id1,
            owner: owner.clone(),
            side: PriceVariant::Buy,
            price: price,
            original_amount: balance!(10),
            executed_amount: balance!(0),
            time: 10,
            lifespan: 1000,
        };

        let order_buy2 = LimitOrder::<Runtime> {
            id: order_buy_id2,
            owner: owner.clone(),
            side: PriceVariant::Buy,
            price: price,
            original_amount: balance!(10),
            executed_amount: balance!(0),
            time: 10,
            lifespan: 1000,
        };

        let order_sell1 = LimitOrder::<Runtime> {
            id: order_sell_id1,
            owner: owner.clone(),
            side: PriceVariant::Sell,
            price: price,
            original_amount: balance!(10),
            executed_amount: balance!(0),
            time: 10,
            lifespan: 1000,
        };

        let order_sell2 = LimitOrder::<Runtime> {
            id: order_sell_id2,
            owner: owner.clone(),
            side: PriceVariant::Sell,
            price: price,
            original_amount: balance!(10),
            executed_amount: balance!(0),
            time: 10,
            lifespan: 1000,
        };

        // add orders
        assert_ok!(OrderBook::insert_limit_order(&order_book_id, &order_buy1));
        assert_ok!(OrderBook::insert_limit_order(&order_book_id, &order_buy2));
        assert_ok!(OrderBook::insert_limit_order(&order_book_id, &order_sell1));
        assert_ok!(OrderBook::insert_limit_order(&order_book_id, &order_sell2));

        // check they added
        assert_eq!(
            OrderBook::limit_orders(order_book_id, order_buy_id1).unwrap(),
            order_buy1
        );
        assert_eq!(
            OrderBook::limit_orders(order_book_id, order_buy_id2).unwrap(),
            order_buy2
        );
        assert_eq!(
            OrderBook::limit_orders(order_book_id, order_sell_id1).unwrap(),
            order_sell1
        );
        assert_eq!(
            OrderBook::limit_orders(order_book_id, order_sell_id2).unwrap(),
            order_sell2
        );
        assert_eq!(
            OrderBook::bids(order_book_id, price).unwrap(),
            vec![order_buy_id1, order_buy_id2]
        );
        assert_eq!(
            OrderBook::asks(order_book_id, price).unwrap(),
            vec![order_sell_id1, order_sell_id2]
        );
        assert_eq!(
            OrderBook::user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_buy_id1, order_buy_id2, order_sell_id1, order_sell_id2]
        );

        // delete order sell 1
        assert_ok!(OrderBook::delete_limit_order(
            &order_book_id,
            order_sell_id1
        ));
        assert_eq!(OrderBook::limit_orders(order_book_id, order_sell_id1), None);
        assert_eq!(
            OrderBook::bids(order_book_id, price).unwrap(),
            vec![order_buy_id1, order_buy_id2]
        );
        assert_eq!(
            OrderBook::asks(order_book_id, price).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(
            OrderBook::user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_buy_id1, order_buy_id2, order_sell_id2]
        );

        // delete order buy 1
        assert_ok!(OrderBook::delete_limit_order(&order_book_id, order_buy_id1));
        assert_eq!(OrderBook::limit_orders(order_book_id, order_buy_id1), None);
        assert_eq!(
            OrderBook::bids(order_book_id, price).unwrap(),
            vec![order_buy_id2]
        );
        assert_eq!(
            OrderBook::asks(order_book_id, price).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(
            OrderBook::user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_buy_id2, order_sell_id2]
        );

        // delete order buy 2
        assert_ok!(OrderBook::delete_limit_order(&order_book_id, order_buy_id2));
        assert_eq!(OrderBook::limit_orders(order_book_id, order_buy_id2), None);
        assert_eq!(OrderBook::bids(order_book_id, price), None);
        assert_eq!(
            OrderBook::asks(order_book_id, price).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(
            OrderBook::user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_sell_id2]
        );

        // delete order sell 2
        assert_ok!(OrderBook::delete_limit_order(
            &order_book_id,
            order_sell_id2
        ));
        assert_eq!(OrderBook::limit_orders(order_book_id, order_sell_id2), None);
        assert_eq!(OrderBook::bids(order_book_id, price), None);
        assert_eq!(OrderBook::asks(order_book_id, price), None);
        assert_eq!(OrderBook::user_limit_orders(&owner, &order_book_id), None);
    });
}

#[test]
fn delete_limit_order_fail() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<Runtime> {
            base_asset_id: XOR.into(),
            target_asset_id: VAL.into(),
        };

        let order_id = 1;

        assert_err!(
            OrderBook::delete_limit_order(&order_book_id, order_id),
            E::DeleteLimitOrderError
        );
    });
}
