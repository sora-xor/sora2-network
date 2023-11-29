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

#![cfg(feature = "ready-to-test")] // order-book

use crate::test_utils::*;
use assets::AssetIdOf;
use common::prelude::BalanceUnit;
use common::{balance, PriceVariant, ETH, PSWAP, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::order_book::cache_data_layer::CacheDataLayer;
use framenode_runtime::order_book::storage_data_layer::StorageDataLayer;
use framenode_runtime::order_book::{
    Config, DataLayer, LimitOrder, OrderBook, OrderBookId, OrderBooks, OrderPrice,
};
use framenode_runtime::Runtime;
use sp_core::Get;
use sp_runtime::BoundedVec;
use sp_std::collections::btree_map::BTreeMap;

trait StoragePush {
    fn push_to_storage(&mut self);
    fn reset(&mut self);
}

impl<T: Config> StoragePush for CacheDataLayer<T> {
    fn push_to_storage(&mut self) {
        self.commit();
    }

    fn reset(&mut self) {
        self.reset();
    }
}

impl<T: Config> StoragePush for StorageDataLayer<T> {
    fn push_to_storage(&mut self) {}
    fn reset(&mut self) {}
}

#[test]
fn should_work_as_cache() {
    ext().execute_with(|| {
        let mut data = CacheDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_id = 1;
        let owner = accounts::alice::<Runtime>();
        let price = balance!(12).into();
        let amount = balance!(100).into();

        let order = LimitOrder::<Runtime>::new(
            order_id,
            owner.clone(),
            PriceVariant::Buy,
            price,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_ok!(data.insert_limit_order(&order_book_id, order.clone()));
        assert_eq!(
            data.get_limit_order(&order_book_id, order_id).unwrap(),
            order
        );
        assert_eq!(
            data.get_bids(&order_book_id, &price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_bid(&order_book_id), Some((price, amount)));
        assert_eq!(data.get_asks(&order_book_id, &price), None);
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_ask(&order_book_id), None);
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_id]
        );

        // check storage before commit
        assert_eq!(OrderBookPallet::limit_orders(order_book_id, order_id), None);
        assert_eq!(OrderBookPallet::bids(order_book_id, price), None);
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(OrderBookPallet::asks(order_book_id, price), None);
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, order_book_id),
            None
        );

        // check storage after commit
        data.commit();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_id).unwrap(),
            order
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(OrderBookPallet::asks(order_book_id, price), None);
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, order_book_id).unwrap(),
            vec![order_id]
        );
    });
}

#[test]
fn should_work_as_storage() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_id = 1;
        let owner = accounts::alice::<Runtime>();
        let price = balance!(12).into();
        let amount = balance!(100).into();

        let order = LimitOrder::<Runtime>::new(
            order_id,
            owner.clone(),
            PriceVariant::Buy,
            price,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_ok!(data.insert_limit_order(&order_book_id, order.clone()));
        assert_eq!(
            data.get_limit_order(&order_book_id, order_id).unwrap(),
            order
        );
        assert_eq!(
            data.get_bids(&order_book_id, &price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_bid(&order_book_id), Some((price, amount)));
        assert_eq!(data.get_asks(&order_book_id, &price), None);
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_ask(&order_book_id), None);
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_id]
        );

        // check storage
        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_id).unwrap(),
            order
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(OrderBookPallet::asks(order_book_id, price), None);
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, order_book_id).unwrap(),
            vec![order_id]
        );
    });
}

#[test]
fn cache_should_get_all_limit_orders() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    should_get_all_limit_orders(&mut cache);
}

#[test]
fn storage_should_get_all_limit_orders() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    should_get_all_limit_orders(&mut storage);
}

fn should_get_all_limit_orders(data: &mut (impl DataLayer<Runtime> + StoragePush)) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_buy_id1 = 1;
        let order_buy_id2 = 2;
        let order_sell_id1 = 3;
        let order_sell_id2 = 4;
        let order_sell_id3 = 5;
        let owner = accounts::alice::<Runtime>();
        let price1 = balance!(12).into();
        let price2 = balance!(13).into();
        let amount = balance!(10).into();

        let order_buy1 = LimitOrder::<Runtime>::new(
            order_buy_id1,
            owner.clone(),
            PriceVariant::Buy,
            price1,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let order_buy2 = LimitOrder::<Runtime>::new(
            order_buy_id2,
            owner.clone(),
            PriceVariant::Buy,
            price1,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let order_sell1 = LimitOrder::<Runtime>::new(
            order_sell_id1,
            owner.clone(),
            PriceVariant::Sell,
            price1,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let order_sell2 = LimitOrder::<Runtime>::new(
            order_sell_id2,
            owner.clone(),
            PriceVariant::Sell,
            price1,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let order_sell3 = LimitOrder::<Runtime>::new(
            order_sell_id3,
            owner.clone(),
            PriceVariant::Sell,
            price2,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // add orders
        assert_ok!(data.insert_limit_order(&order_book_id, order_buy1.clone()));
        assert_ok!(data.insert_limit_order(&order_book_id, order_buy2.clone()));
        assert_ok!(data.insert_limit_order(&order_book_id, order_sell1.clone()));
        assert_ok!(data.insert_limit_order(&order_book_id, order_sell2.clone()));
        assert_ok!(data.insert_limit_order(&order_book_id, order_sell3.clone()));

        data.push_to_storage();
        data.reset();

        let mut orders = data.get_all_limit_orders(&order_book_id);
        orders.sort_by(|a, b| a.id.cmp(&b.id));

        let expected_orders = vec![
            order_buy1,
            order_buy2,
            order_sell1,
            order_sell2,
            order_sell3,
        ];

        assert_eq!(orders, expected_orders);
    });
}

#[test]
fn cache_should_insert_limit_order() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    should_insert_limit_order(&mut cache);
}

#[test]
fn storage_should_insert_limit_order() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    should_insert_limit_order(&mut storage);
}

fn should_insert_limit_order(data: &mut (impl DataLayer<Runtime> + StoragePush)) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_buy_id = 1;
        let order_sell_id = 2;
        let owner = accounts::alice::<Runtime>();
        let price = balance!(12).into();
        let amount = balance!(10).into();

        let order_buy = LimitOrder::<Runtime>::new(
            order_buy_id,
            owner.clone(),
            PriceVariant::Buy,
            price,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let order_sell = LimitOrder::<Runtime>::new(
            order_sell_id,
            owner.clone(),
            PriceVariant::Sell,
            price,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_ok!(data.insert_limit_order(&order_book_id, order_buy.clone()));
        assert_eq!(
            data.get_limit_order(&order_book_id, order_buy_id).unwrap(),
            order_buy
        );
        assert_eq!(
            data.get_bids(&order_book_id, &price).unwrap(),
            vec![order_buy_id]
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_bid(&order_book_id), Some((price, amount)));
        assert_eq!(data.get_asks(&order_book_id, &price), None);
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_ask(&order_book_id), None);
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_buy_id]
        );

        // check storage
        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_buy_id).unwrap(),
            order_buy
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, price).unwrap(),
            vec![order_buy_id]
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(OrderBookPallet::asks(order_book_id, price), None);
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, order_book_id).unwrap(),
            vec![order_buy_id]
        );

        assert_ok!(data.insert_limit_order(&order_book_id, order_sell.clone()));
        assert_eq!(
            data.get_limit_order(&order_book_id, order_sell_id).unwrap(),
            order_sell
        );
        assert_eq!(
            data.get_bids(&order_book_id, &price).unwrap(),
            vec![order_buy_id]
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_bid(&order_book_id), Some((price, amount)));
        assert_eq!(
            data.get_asks(&order_book_id, &price).unwrap(),
            vec![order_sell_id]
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_ask(&order_book_id), Some((price, amount)));
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_buy_id, order_sell_id]
        );

        // check storage
        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::bids(order_book_id, price).unwrap(),
            vec![order_buy_id]
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, price).unwrap(),
            vec![order_sell_id]
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, order_book_id).unwrap(),
            vec![order_buy_id, order_sell_id]
        );
    });
}

#[test]
fn cache_should_not_insert_limit_order() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    should_not_insert_limit_order(&mut cache);
}

#[test]
fn storage_should_not_insert_limit_order() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    should_not_insert_limit_order(&mut storage);
}

fn should_not_insert_limit_order(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let owner = accounts::alice::<Runtime>();
        let price = balance!(12).into();
        let amount = balance!(10).into();

        let mut order = LimitOrder::<Runtime>::new(
            0,
            owner.clone(),
            PriceVariant::Sell,
            price,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let max_per_user: u32 = <Runtime as Config>::MaxOpenedLimitOrdersPerUser::get();
        let max_for_price: u32 = <Runtime as Config>::MaxLimitOrdersForPrice::get();
        let max = max_per_user.min(max_for_price);

        for id in 0..max {
            order.id = id as u128;
            assert_ok!(data.insert_limit_order(&order_book_id, order.clone()));
        }

        // Error if storage overflow
        order.id = (max + 1) as u128;
        assert_err!(
            data.insert_limit_order(&order_book_id, order),
            E::LimitOrderStorageOverflow
        );
    });
}

#[test]
fn cache_should_delete_limit_order() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    should_delete_limit_order(&mut cache);
}

#[test]
fn storage_should_delete_limit_order() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    should_delete_limit_order(&mut storage);
}

fn should_delete_limit_order(data: &mut (impl DataLayer<Runtime> + StoragePush)) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_buy_id1 = 1;
        let order_buy_id2 = 2;
        let order_sell_id1 = 3;
        let order_sell_id2 = 4;
        let order_sell_id3 = 5;
        let owner = accounts::alice::<Runtime>();
        let price1 = balance!(12).into();
        let price2 = balance!(13).into();
        let amount = balance!(10).into();
        let double_amount = balance!(20).into();

        let order_buy1 = LimitOrder::<Runtime>::new(
            order_buy_id1,
            owner.clone(),
            PriceVariant::Buy,
            price1,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let order_buy2 = LimitOrder::<Runtime>::new(
            order_buy_id2,
            owner.clone(),
            PriceVariant::Buy,
            price1,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let order_sell1 = LimitOrder::<Runtime>::new(
            order_sell_id1,
            owner.clone(),
            PriceVariant::Sell,
            price1,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let order_sell2 = LimitOrder::<Runtime>::new(
            order_sell_id2,
            owner.clone(),
            PriceVariant::Sell,
            price1,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let order_sell3 = LimitOrder::<Runtime>::new(
            order_sell_id3,
            owner.clone(),
            PriceVariant::Sell,
            price2,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // add orders
        assert_ok!(data.insert_limit_order(&order_book_id, order_buy1.clone()));
        assert_ok!(data.insert_limit_order(&order_book_id, order_buy2.clone()));
        assert_ok!(data.insert_limit_order(&order_book_id, order_sell1.clone()));
        assert_ok!(data.insert_limit_order(&order_book_id, order_sell2.clone()));
        assert_ok!(data.insert_limit_order(&order_book_id, order_sell3.clone()));

        // check they added
        assert_eq!(
            data.get_limit_order(&order_book_id, order_buy_id1).unwrap(),
            order_buy1
        );
        assert_eq!(
            data.get_limit_order(&order_book_id, order_buy_id2).unwrap(),
            order_buy2
        );
        assert_eq!(
            data.get_limit_order(&order_book_id, order_sell_id1)
                .unwrap(),
            order_sell1
        );
        assert_eq!(
            data.get_limit_order(&order_book_id, order_sell_id2)
                .unwrap(),
            order_sell2
        );
        assert_eq!(
            data.get_limit_order(&order_book_id, order_sell_id3)
                .unwrap(),
            order_sell3
        );
        assert_eq!(
            data.get_bids(&order_book_id, &price1).unwrap(),
            vec![order_buy_id1, order_buy_id2]
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(price1, double_amount)])
        );
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_bid(&order_book_id), Some((price1, double_amount)));
        assert_eq!(
            data.get_asks(&order_book_id, &price1).unwrap(),
            vec![order_sell_id1, order_sell_id2]
        );
        assert_eq!(
            data.get_asks(&order_book_id, &price2).unwrap(),
            vec![order_sell_id3]
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(price1, double_amount), (price2, amount)])
        );
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 2);
        assert_eq!(data.best_ask(&order_book_id), Some((price1, double_amount)));
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![
                order_buy_id1,
                order_buy_id2,
                order_sell_id1,
                order_sell_id2,
                order_sell_id3
            ]
        );

        // check they added in storage
        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_buy_id1).unwrap(),
            order_buy1
        );
        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_buy_id2).unwrap(),
            order_buy2
        );
        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_sell_id1).unwrap(),
            order_sell1
        );
        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_sell_id2).unwrap(),
            order_sell2
        );
        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_sell_id3).unwrap(),
            order_sell3
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, price1).unwrap(),
            vec![order_buy_id1, order_buy_id2]
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([(price1, double_amount)])
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, price1).unwrap(),
            vec![order_sell_id1, order_sell_id2]
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, price2).unwrap(),
            vec![order_sell_id3]
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([(price1, double_amount), (price2, amount)])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![
                order_buy_id1,
                order_buy_id2,
                order_sell_id1,
                order_sell_id2,
                order_sell_id3
            ]
        );

        // delete order sell 1
        assert_ok!(data.delete_limit_order(&order_book_id, order_sell_id1));
        assert_err!(
            data.get_limit_order(&order_book_id, order_sell_id1),
            E::UnknownLimitOrder
        );
        assert_eq!(
            data.get_bids(&order_book_id, &price1).unwrap(),
            vec![order_buy_id1, order_buy_id2]
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(price1, double_amount)])
        );
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_bid(&order_book_id), Some((price1, double_amount)));
        assert_eq!(
            data.get_asks(&order_book_id, &price1).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(
            data.get_asks(&order_book_id, &price2).unwrap(),
            vec![order_sell_id3]
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(price1, amount), (price2, amount)])
        );
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 2);
        assert_eq!(data.best_ask(&order_book_id), Some((price1, amount)));
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_buy_id1, order_buy_id2, order_sell_id2, order_sell_id3]
        );

        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_sell_id1),
            None
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, price1).unwrap(),
            vec![order_buy_id1, order_buy_id2]
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([(price1, double_amount)])
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, price1).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, price2).unwrap(),
            vec![order_sell_id3]
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([(price1, amount), (price2, amount)])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_buy_id1, order_buy_id2, order_sell_id2, order_sell_id3]
        );

        // delete order buy 1
        assert_ok!(data.delete_limit_order(&order_book_id, order_buy_id1));

        assert_err!(
            data.get_limit_order(&order_book_id, order_buy_id1),
            E::UnknownLimitOrder
        );
        assert_eq!(
            data.get_bids(&order_book_id, &price1).unwrap(),
            vec![order_buy_id2]
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(price1, amount)])
        );
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_bid(&order_book_id), Some((price1, amount)));
        assert_eq!(
            data.get_asks(&order_book_id, &price1).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(
            data.get_asks(&order_book_id, &price2).unwrap(),
            vec![order_sell_id3]
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(price1, amount), (price2, amount)])
        );
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 2);
        assert_eq!(data.best_ask(&order_book_id), Some((price1, amount)));
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_buy_id2, order_sell_id2, order_sell_id3]
        );

        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_buy_id1),
            None
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, price1).unwrap(),
            vec![order_buy_id2]
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([(price1, amount)])
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, price1).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, price2).unwrap(),
            vec![order_sell_id3]
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([(price1, amount), (price2, amount)])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_buy_id2, order_sell_id2, order_sell_id3]
        );

        // delete order buy 2
        assert_ok!(data.delete_limit_order(&order_book_id, order_buy_id2));

        assert_err!(
            data.get_limit_order(&order_book_id, order_buy_id2),
            E::UnknownLimitOrder
        );
        assert_eq!(data.get_bids(&order_book_id, &price1), None);
        assert_eq!(data.get_aggregated_bids(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_bid(&order_book_id), None);
        assert_eq!(
            data.get_asks(&order_book_id, &price1).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(
            data.get_asks(&order_book_id, &price2).unwrap(),
            vec![order_sell_id3]
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(price1, amount), (price2, amount)])
        );
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 2);
        assert_eq!(data.best_ask(&order_book_id), Some((price1, amount)));
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_sell_id2, order_sell_id3]
        );

        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_buy_id2),
            None
        );
        assert_eq!(OrderBookPallet::bids(order_book_id, price1), None);
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, price1).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, price2).unwrap(),
            vec![order_sell_id3]
        );
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([(price1, amount), (price2, amount)])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_sell_id2, order_sell_id3]
        );

        // delete order sell 3
        assert_ok!(data.delete_limit_order(&order_book_id, order_sell_id3));

        assert_err!(
            data.get_limit_order(&order_book_id, order_sell_id3),
            E::UnknownLimitOrder
        );
        assert_eq!(data.get_bids(&order_book_id, &price1), None);
        assert_eq!(data.get_aggregated_bids(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_bid(&order_book_id), None);
        assert_eq!(
            data.get_asks(&order_book_id, &price1).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(data.get_asks(&order_book_id, &price2), None);
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(price1, amount)])
        );
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_ask(&order_book_id), Some((price1, amount)));
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_sell_id2]
        );

        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_sell_id3),
            None
        );
        assert_eq!(OrderBookPallet::bids(order_book_id, price1), None);
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::asks(order_book_id, price1).unwrap(),
            vec![order_sell_id2]
        );
        assert_eq!(OrderBookPallet::asks(order_book_id, price2), None);
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([(price1, amount)])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_sell_id2]
        );

        // delete order sell 2
        assert_ok!(data.delete_limit_order(&order_book_id, order_sell_id2));

        assert_err!(
            data.get_limit_order(&order_book_id, order_sell_id2),
            E::UnknownLimitOrder
        );
        assert_eq!(data.get_bids(&order_book_id, &price1), None);
        assert_eq!(data.get_aggregated_bids(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_bid(&order_book_id), None);
        assert_eq!(data.get_asks(&order_book_id, &price1), None);
        assert_eq!(data.get_asks(&order_book_id, &price2), None);
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_ask(&order_book_id), None);
        assert_eq!(data.get_user_limit_orders(&owner, &order_book_id), None);

        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_sell_id2),
            None
        );
        assert_eq!(OrderBookPallet::bids(order_book_id, price1), None);
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(OrderBookPallet::asks(order_book_id, price1), None);
        assert_eq!(OrderBookPallet::asks(order_book_id, price2), None);
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, &order_book_id),
            None
        );
    });
}

#[test]
fn cache_should_not_delete_unknown_limit_order() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    should_not_delete_unknown_limit_order(&mut cache);
}

#[test]
fn storage_should_not_delete_unknown_limit_order() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    should_not_delete_unknown_limit_order(&mut storage);
}

fn should_not_delete_unknown_limit_order(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_id = 1;

        assert_err!(
            data.delete_limit_order(&order_book_id, order_id),
            E::UnknownLimitOrder
        );
    });
}

#[test]
fn cache_should_update_limit_order() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    should_update_limit_order(&mut cache);
}

#[test]
fn storage_should_update_limit_order() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    should_update_limit_order(&mut storage);
}

fn should_update_limit_order(data: &mut (impl DataLayer<Runtime> + StoragePush)) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_id = 1;
        let owner = accounts::alice::<Runtime>();
        let price = balance!(10).into();
        let amount = balance!(100).into();
        let new_amount = balance!(80).into();

        let mut order = LimitOrder::<Runtime>::new(
            order_id,
            owner.clone(),
            PriceVariant::Buy,
            price,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // insert order
        assert_ok!(data.insert_limit_order(&order_book_id, order.clone()));
        assert_eq!(
            data.get_limit_order(&order_book_id, order_id).unwrap(),
            order
        );
        assert_eq!(
            data.get_bids(&order_book_id, &price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_bid(&order_book_id), Some((price, amount)));
        assert_eq!(data.get_asks(&order_book_id, &price), None);
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_ask(&order_book_id), None);
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_id]
        );

        // check storage
        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_id).unwrap(),
            order
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(OrderBookPallet::asks(order_book_id, price), None);
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, order_book_id).unwrap(),
            vec![order_id]
        );

        // update order
        assert_ok!(data.update_limit_order_amount(&order_book_id, order_id, new_amount));
        order.amount = new_amount;

        assert_eq!(
            data.get_limit_order(&order_book_id, order_id).unwrap(),
            order
        );
        assert_eq!(
            data.get_bids(&order_book_id, &price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(price, new_amount)])
        );
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_bid(&order_book_id), Some((price, new_amount)));
        assert_eq!(data.get_asks(&order_book_id, &price), None);
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_ask(&order_book_id), None);
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_id]
        );

        // check storage
        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_id).unwrap(),
            order
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([(price, new_amount)])
        );
        assert_eq!(OrderBookPallet::asks(order_book_id, price), None);
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, order_book_id).unwrap(),
            vec![order_id]
        );
    });
}

#[test]
fn cache_should_update_limit_order_with_zero_amount() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    should_update_limit_order_with_zero_amount(&mut cache);
}

#[test]
fn storage_should_update_limit_order_with_zero_amount() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    should_update_limit_order_with_zero_amount(&mut storage);
}

fn should_update_limit_order_with_zero_amount(data: &mut (impl DataLayer<Runtime> + StoragePush)) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_id = 1;
        let owner = accounts::alice::<Runtime>();
        let price = balance!(10).into();
        let amount = balance!(100).into();
        let new_amount = balance!(0).into();

        let order = LimitOrder::<Runtime>::new(
            order_id,
            owner.clone(),
            PriceVariant::Buy,
            price,
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // insert order
        assert_ok!(data.insert_limit_order(&order_book_id, order.clone()));
        assert_eq!(
            data.get_limit_order(&order_book_id, order_id).unwrap(),
            order
        );
        assert_eq!(
            data.get_bids(&order_book_id, &price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 1);
        assert_eq!(data.best_bid(&order_book_id), Some((price, amount)));
        assert_eq!(data.get_asks(&order_book_id, &price), None);
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_ask(&order_book_id), None);
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            vec![order_id]
        );

        // check storage
        data.push_to_storage();

        assert_eq!(
            OrderBookPallet::limit_orders(order_book_id, order_id).unwrap(),
            order
        );
        assert_eq!(
            OrderBookPallet::bids(order_book_id, price).unwrap(),
            vec![order_id]
        );
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([(price, amount)])
        );
        assert_eq!(OrderBookPallet::asks(order_book_id, price), None);
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, order_book_id).unwrap(),
            vec![order_id]
        );

        // update order, if amount is 0 then order should be deleted
        assert_ok!(data.update_limit_order_amount(&order_book_id, order_id, new_amount));

        assert_err!(
            data.get_limit_order(&order_book_id, order_id),
            E::UnknownLimitOrder
        );
        assert_eq!(data.get_bids(&order_book_id, &price), None);
        assert_eq!(data.get_aggregated_bids(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_bids_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_bid(&order_book_id), None);
        assert_eq!(data.get_asks(&order_book_id, &price), None);
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_asks_len(&order_book_id).unwrap_or(0), 0);
        assert_eq!(data.best_ask(&order_book_id), None);
        assert_eq!(data.get_user_limit_orders(&owner, &order_book_id), None);

        // check storage
        data.push_to_storage();

        assert_eq!(OrderBookPallet::limit_orders(order_book_id, order_id), None);
        assert_eq!(OrderBookPallet::bids(order_book_id, price), None);
        assert_eq!(
            OrderBookPallet::aggregated_bids(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(OrderBookPallet::asks(order_book_id, price), None);
        assert_eq!(
            OrderBookPallet::aggregated_asks(order_book_id),
            BTreeMap::from([])
        );
        assert_eq!(
            OrderBookPallet::user_limit_orders(&owner, order_book_id),
            None
        );
    });
}

#[test]
fn cache_should_not_update_unknown_limit_order() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    should_not_update_unknown_limit_order(&mut cache);
}

#[test]
fn storage_should_not_update_unknown_limit_order() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    should_not_update_unknown_limit_order(&mut storage);
}

fn should_not_update_unknown_limit_order(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_id = 1;
        let amount = balance!(100).into();

        assert_err!(
            data.update_limit_order_amount(&order_book_id, order_id, amount),
            E::UnknownLimitOrder
        );
    });
}

#[test]
fn cache_should_not_update_equal_limit_order() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    should_not_update_equal_limit_order(&mut cache);
}

#[test]
fn storage_should_not_update_equal_limit_order() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    should_not_update_equal_limit_order(&mut storage);
}

fn should_not_update_equal_limit_order(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_id = 1;
        let amount = balance!(100).into();

        let order = LimitOrder::<Runtime>::new(
            order_id,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(12).into(),
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_ok!(data.insert_limit_order(&order_book_id, order.clone()));

        // the same amount doesn't change anything, but returns ok
        assert_ok!(data.update_limit_order_amount(&order_book_id, order_id, amount));

        assert_eq!(
            data.get_limit_order(&order_book_id, order_id).unwrap(),
            order
        );
    });
}

#[test]
fn cache_should_not_update_limit_order_with_bigger_amount() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    should_not_update_limit_order_with_bigger_amount(&mut cache);
}

#[test]
fn storage_should_not_update_limit_order_with_bigger_amount() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    should_not_update_limit_order_with_bigger_amount(&mut storage);
}

fn should_not_update_limit_order_with_bigger_amount(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let order_id = 1;
        let amount = balance!(100).into();
        let new_amount = balance!(110).into();

        let order = LimitOrder::<Runtime>::new(
            order_id,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(12).into(),
            amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_ok!(data.insert_limit_order(&order_book_id, order));

        assert_err!(
            data.update_limit_order_amount(&order_book_id, order_id, new_amount),
            E::UpdateLimitOrderError
        );
    });
}

#[test]
fn cache_should_get_limit_orders_by_price() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    get_limit_orders_by_price(&mut cache);
}

#[test]
fn storage_should_get_limit_orders_by_price() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    get_limit_orders_by_price(&mut storage);
}

fn get_limit_orders_by_price(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        let buy_price = balance!(10).into();
        let sell_price = balance!(11).into();

        assert_eq!(
            data.get_bids(&order_book_id, &buy_price),
            data.get_limit_orders_by_price(&order_book_id, PriceVariant::Buy, &buy_price)
        );
        assert_eq!(
            data.get_asks(&order_book_id, &sell_price),
            data.get_limit_orders_by_price(&order_book_id, PriceVariant::Sell, &sell_price)
        );
    });
}

#[test]
fn cache_should_get_user_limit_orders() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    get_user_limit_orders(&mut cache);
}

#[test]
fn storage_should_get_user_limit_orders() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    get_user_limit_orders(&mut storage);
}

fn get_user_limit_orders(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book::<Runtime>(order_book_id);

        let empty_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: PSWAP.into(),
            quote: XOR.into(),
        };

        create_empty_order_book::<Runtime>(empty_order_book_id);

        assert_eq!(
            data.get_user_limit_orders(&accounts::bob::<Runtime>(), &empty_order_book_id),
            None
        );

        assert_eq!(
            data.get_user_limit_orders(&accounts::bob::<Runtime>(), &order_book_id)
                .unwrap(),
            vec![1, 3, 5, 7, 9, 11]
        );
    });
}

#[test]
fn cache_should_get_all_user_limit_orders() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    get_all_user_limit_orders(&mut cache);
}

#[test]
fn storage_should_get_all_user_limit_orders() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    get_all_user_limit_orders(&mut storage);
}

fn get_all_user_limit_orders(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id1 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book::<Runtime>(order_book_id1);

        let order_book_id2 = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: PSWAP.into(),
            quote: XOR.into(),
        };

        create_and_fill_order_book::<Runtime>(order_book_id2);

        let empty_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: ETH.into(),
            quote: XOR.into(),
        };

        create_empty_order_book::<Runtime>(empty_order_book_id);

        // no orders from empty_order_book_id
        assert_eq!(
            data.get_all_user_limit_orders(&accounts::bob::<Runtime>()),
            BTreeMap::from([
                (
                    order_book_id1,
                    BoundedVec::try_from(vec![1, 3, 5, 7, 9, 11]).unwrap()
                ),
                (
                    order_book_id2,
                    BoundedVec::try_from(vec![1, 3, 5, 7, 9, 11]).unwrap()
                )
            ])
        );
    });
}

fn best_bid_should_work_multiple_prices(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        let mut order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let best_price = BalanceUnit::divisible(balance!(10));
        let best_price_aggregated_volume = BalanceUnit::divisible(balance!(168.5));
        assert_eq!(
            data.best_bid(&order_book_id),
            Some((best_price, best_price_aggregated_volume))
        );

        // rest is for adding/removing orders and checking that it still shows correct results

        let owner = accounts::alice::<Runtime>();
        let order_amount = BalanceUnit::divisible(balance!(10.0));

        let order1 = LimitOrder::<Runtime>::new(
            order_book.next_order_id(),
            owner.clone(),
            PriceVariant::Buy,
            best_price,
            order_amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // add an order into current best price
        assert_ok!(data.insert_limit_order(&order_book_id, order1.clone()));
        assert_eq!(
            data.best_bid(&order_book_id),
            Some((best_price, best_price_aggregated_volume + order_amount))
        );

        let new_best_price = BalanceUnit::divisible(balance!(10.7));
        let order2 = LimitOrder::<Runtime>::new(
            order_book.next_order_id(),
            owner.clone(),
            PriceVariant::Buy,
            new_best_price,
            order_amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // add order with new best price
        assert_ok!(data.insert_limit_order(&order_book_id, order2.clone()));
        assert_eq!(
            data.best_bid(&order_book_id),
            Some((new_best_price, order_amount))
        );

        // remove the order, old best price is again the best
        assert_ok!(data.delete_limit_order(&order_book_id, order2.id));
        assert_eq!(
            data.best_bid(&order_book_id),
            Some((best_price, best_price_aggregated_volume + order_amount))
        );

        // remove order1, amount should return back to the original
        assert_ok!(data.delete_limit_order(&order_book_id, order1.id));
        assert_eq!(
            data.best_bid(&order_book_id),
            Some((best_price, best_price_aggregated_volume))
        );
    })
}

fn best_ask_should_work_multiple_prices(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        let mut order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let best_price = BalanceUnit::divisible(balance!(11));
        let best_price_aggregated_volume = BalanceUnit::divisible(balance!(176.3));
        assert_eq!(
            data.best_ask(&order_book_id),
            Some((best_price, best_price_aggregated_volume))
        );

        // rest is for adding/removing orders and checking that it still shows correct results

        let owner = accounts::alice::<Runtime>();
        let order_amount = BalanceUnit::divisible(balance!(10.0));

        let order1 = LimitOrder::<Runtime>::new(
            order_book.next_order_id(),
            owner.clone(),
            PriceVariant::Sell,
            best_price,
            order_amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // add an order into current best price
        assert_ok!(data.insert_limit_order(&order_book_id, order1.clone()));
        assert_eq!(
            data.best_ask(&order_book_id),
            Some((best_price, best_price_aggregated_volume + order_amount))
        );

        let new_best_price = BalanceUnit::divisible(balance!(10.3));
        let order2 = LimitOrder::<Runtime>::new(
            order_book.next_order_id(),
            owner.clone(),
            PriceVariant::Sell,
            new_best_price,
            order_amount,
            10,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // add order with new best price
        assert_ok!(data.insert_limit_order(&order_book_id, order2.clone()));
        assert_eq!(
            data.best_ask(&order_book_id),
            Some((new_best_price, order_amount))
        );

        // remove the order, old best price is again the best
        assert_ok!(data.delete_limit_order(&order_book_id, order2.id));
        assert_eq!(
            data.best_ask(&order_book_id),
            Some((best_price, best_price_aggregated_volume + order_amount))
        );

        // remove order1, amount should return back to the original
        assert_ok!(data.delete_limit_order(&order_book_id, order1.id));
        assert_eq!(
            data.best_ask(&order_book_id),
            Some((best_price, best_price_aggregated_volume))
        );
    })
}

#[test]
fn cache_best_bid_should_work_multiple_prices() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    best_bid_should_work_multiple_prices(&mut cache);
}

#[test]
fn storage_best_bid_should_work_multiple_prices() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    best_bid_should_work_multiple_prices(&mut storage);
}

#[test]
fn cache_best_ask_should_work_multiple_prices() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    best_ask_should_work_multiple_prices(&mut cache);
}

#[test]
fn storage_best_ask_should_work_multiple_prices() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    best_ask_should_work_multiple_prices(&mut storage);
}

fn fill_single_price(
    data: &mut impl DataLayer<Runtime>,
    mut order_book: OrderBook<Runtime>,
    price: OrderPrice,
    side: PriceVariant,
) {
    let fill_settings = fill_tools::FillSettings::max();
    let amount = order_book.min_lot_size;
    let mut users = fill_tools::users_iterator::<Runtime>(
        order_book.order_book_id,
        amount,
        price,
        fill_settings.max_orders_per_user,
    )
    .peekable();
    let current_block = frame_system::Pallet::<Runtime>::block_number();
    let mut lifespans = fill_tools::lifespans_iterator::<Runtime>(
        fill_settings.max_expiring_orders_per_block,
        (current_block + 10).into(),
    )
    .peekable();
    fill_tools::fill_price(
        data,
        fill_settings,
        &mut order_book,
        side,
        amount,
        price,
        &mut users,
        &mut lifespans,
    );
    <OrderBooks<Runtime>>::insert(order_book.order_book_id, order_book.clone());
}

fn is_bid_price_full_works(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        let order_book = create_empty_order_book::<Runtime>(order_book_id);
        let price = order_book.tick_size;
        assert!(data.is_bid_price_full(&order_book_id, &price).is_none());
        fill_single_price(data, order_book, price, PriceVariant::Buy);
        assert!(data.is_bid_price_full(&order_book_id, &price).unwrap());
        let order_book = OrderBookPallet::order_books(order_book_id).unwrap();
        data.delete_limit_order(&order_book_id, order_book.last_order_id)
            .unwrap();
        assert!(!data.is_bid_price_full(&order_book_id, &price).unwrap());
    })
}

fn is_ask_price_full_works(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        let order_book = create_empty_order_book::<Runtime>(order_book_id);
        let price = order_book.tick_size;
        assert!(data.is_ask_price_full(&order_book_id, &price).is_none());
        fill_single_price(data, order_book, price, PriceVariant::Sell);
        assert!(data.is_ask_price_full(&order_book_id, &price).unwrap());
        let order_book = OrderBookPallet::order_books(order_book_id).unwrap();
        data.delete_limit_order(&order_book_id, order_book.last_order_id)
            .unwrap();
        assert!(!data.is_ask_price_full(&order_book_id, &price).unwrap());
    })
}

#[test]
fn cache_is_bid_price_full_works() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    is_bid_price_full_works(&mut cache);
}

#[test]
fn storage_is_bid_price_full_works() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    is_bid_price_full_works(&mut storage);
}

#[test]
fn cache_is_ask_price_full_works() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    is_ask_price_full_works(&mut cache);
}

#[test]
fn storage_is_ask_price_full_works() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    is_ask_price_full_works(&mut storage);
}

fn is_user_limit_orders_full_works(data: &mut impl DataLayer<Runtime>) {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };
        let mut order_book = create_empty_order_book::<Runtime>(order_book_id);
        let user = accounts::alice::<Runtime>();
        let fill_settings = fill_tools::FillSettings::max();
        let current_block = frame_system::Pallet::<Runtime>::block_number();
        let mut lifespans = fill_tools::lifespans_iterator::<Runtime>(
            fill_settings.max_expiring_orders_per_block,
            (current_block + 10).into(),
        )
        .peekable();
        let amount = order_book.min_lot_size;
        assert!(data
            .is_user_limit_orders_full(&user, &order_book_id)
            .is_none());
        fill_tools::fill_user_orders(
            data,
            fill_settings,
            &mut order_book,
            PriceVariant::Buy,
            amount,
            user.clone(),
            &mut lifespans,
        );
        assert!(data
            .is_user_limit_orders_full(&user, &order_book_id)
            .unwrap());
        data.delete_limit_order(&order_book_id, order_book.last_order_id)
            .unwrap();
        assert!(!data
            .is_user_limit_orders_full(&user, &order_book_id)
            .unwrap());
    })
}

#[test]
fn cache_is_user_limit_orders_full_works() {
    let mut cache = CacheDataLayer::<Runtime>::new();
    is_user_limit_orders_full_works(&mut cache);
}

#[test]
fn storage_is_user_limit_orders_full_works() {
    let mut storage = StorageDataLayer::<Runtime>::new();
    is_user_limit_orders_full_works(&mut storage);
}
