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

use assets::AssetIdOf;
use common::{balance, AssetInfoProvider, DEXId, PriceVariant, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::order_book::storage_data_layer::StorageDataLayer;
use framenode_runtime::order_book::{
    self, Config, DataLayer, LimitOrder, OrderBook, OrderBookId, OrderBookStatus,
};
use framenode_runtime::{Runtime, RuntimeOrigin};
use sp_std::collections::btree_map::BTreeMap;

type E = order_book::Error<Runtime>;
pub const DEX: DEXId = DEXId::Polkaswap;

fn alice() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([1u8; 32])
}

fn bob() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([2u8; 32])
}

// Fills the order book
// price | volume | orders
//          Asks
//  11.5 |  255.8 | sell4, sell5, sell6
//  11.2 |  178.6 | sell2, sell3
//  11.0 |  176.3 | sell1
//  spread
//  10.0 |  168.5 | buy1
//   9.8 |  139.9 | buy2, buy3
//   9.5 |  264.3 | buy4, buy5, buy6
//          Bids
fn fill_order_book(
    order_book_id: &OrderBookId<AssetIdOf<Runtime>>,
    data: &mut impl DataLayer<Runtime>,
) {
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

    // orders
    let buy1 = LimitOrder::<Runtime>::new(1, bob(), PriceVariant::Buy, bp1, amount1, 10, 10000);
    let buy2 = LimitOrder::<Runtime>::new(2, bob(), PriceVariant::Buy, bp2, amount2, 10, 10000);
    let buy3 = LimitOrder::<Runtime>::new(3, bob(), PriceVariant::Buy, bp2, amount3, 10, 10000);
    let buy4 = LimitOrder::<Runtime>::new(4, bob(), PriceVariant::Buy, bp3, amount4, 10, 10000);
    let buy5 = LimitOrder::<Runtime>::new(5, bob(), PriceVariant::Buy, bp3, amount5, 10, 10000);
    let buy6 = LimitOrder::<Runtime>::new(6, bob(), PriceVariant::Buy, bp3, amount6, 10, 10000);

    let sell1 = LimitOrder::<Runtime>::new(7, bob(), PriceVariant::Sell, sp1, amount7, 10, 10000);
    let sell2 = LimitOrder::<Runtime>::new(8, bob(), PriceVariant::Sell, sp2, amount8, 10, 10000);
    let sell3 = LimitOrder::<Runtime>::new(9, bob(), PriceVariant::Sell, sp2, amount9, 10, 10000);
    let sell4 = LimitOrder::<Runtime>::new(10, bob(), PriceVariant::Sell, sp3, amount10, 10, 10000);
    let sell5 = LimitOrder::<Runtime>::new(11, bob(), PriceVariant::Sell, sp3, amount11, 10, 10000);
    let sell6 = LimitOrder::<Runtime>::new(12, bob(), PriceVariant::Sell, sp3, amount12, 10, 10000);

    // inserts
    assert_ok!(data.insert_limit_order(&order_book_id, buy1));
    assert_ok!(data.insert_limit_order(&order_book_id, buy2));
    assert_ok!(data.insert_limit_order(&order_book_id, buy3));
    assert_ok!(data.insert_limit_order(&order_book_id, buy4));
    assert_ok!(data.insert_limit_order(&order_book_id, buy5));
    assert_ok!(data.insert_limit_order(&order_book_id, buy6));

    assert_ok!(data.insert_limit_order(&order_book_id, sell1));
    assert_ok!(data.insert_limit_order(&order_book_id, sell2));
    assert_ok!(data.insert_limit_order(&order_book_id, sell3));
    assert_ok!(data.insert_limit_order(&order_book_id, sell4));
    assert_ok!(data.insert_limit_order(&order_book_id, sell5));
    assert_ok!(data.insert_limit_order(&order_book_id, sell6));

    // check
    assert_eq!(data.get_bids(&order_book_id, &bp1).unwrap(), vec![1]);
    assert_eq!(data.get_bids(&order_book_id, &bp2).unwrap(), vec![2, 3]);
    assert_eq!(data.get_bids(&order_book_id, &bp3).unwrap(), vec![4, 5, 6]);

    assert_eq!(data.get_asks(&order_book_id, &sp1).unwrap(), vec![7]);
    assert_eq!(data.get_asks(&order_book_id, &sp2).unwrap(), vec![8, 9]);
    assert_eq!(
        data.get_asks(&order_book_id, &sp3).unwrap(),
        vec![10, 11, 12]
    );

    assert_eq!(
        data.get_aggregated_bids(&order_book_id),
        BTreeMap::from([
            (bp1, amount1),
            (bp2, amount2 + amount3),
            (bp3, amount4 + amount5 + amount6)
        ])
    );
    assert_eq!(
        data.get_aggregated_asks(&order_book_id),
        BTreeMap::from([
            (sp1, amount7),
            (sp2, amount8 + amount9),
            (sp3, amount10 + amount11 + amount12)
        ])
    );
}

#[test]
fn should_create_new() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base_asset_id: XOR.into(),
        target_asset_id: VAL.into(),
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
        base_asset_id: XOR.into(),
        target_asset_id: VAL.into(),
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
        base_asset_id: XOR.into(),
        target_asset_id: VAL.into(),
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
        base_asset_id: XOR.into(),
        target_asset_id: VAL.into(),
    };

    let mut order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());
    assert_eq!(order_book.last_order_id, 0);

    assert_eq!(order_book.next_order_id(), 1);
    assert_eq!(order_book.last_order_id, 1);

    order_book.last_order_id = 8;

    assert_eq!(order_book.next_order_id(), 9);
    assert_eq!(order_book.last_order_id, 9);
}

// todo
// place_limit_order:
// ensure_limit_order_valid & nft
// check_restrictions
// cross_spread
// success: insert order & liquidity locked & nft

#[test]
fn should_place_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base_asset_id: XOR.into(),
            target_asset_id: VAL.into(),
        };

        let order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());

        fill_order_book(&order_book_id, &mut data);

        let order_id = 100;
        let owner = alice();
        let price = balance!(10);
        let amount = balance!(100);

        assert_ok!(assets::Pallet::<Runtime>::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            order_book_id.base_asset_id,
            balance!(10000).try_into().unwrap()
        ));

        // fix state before
        let bids_before = data.get_bids(&order_book_id, &price).unwrap_or_default();
        let agg_bids_before = data.get_aggregated_bids(&order_book_id);
        let price_volume_before = agg_bids_before.get(&price).cloned().unwrap_or_default();
        let user_orders_before = data
            .get_user_limit_orders(&owner, &order_book_id)
            .unwrap_or_default();
        let balance_before = <Runtime as Config>::AssetInfoProvider::free_balance(
            &order_book_id.base_asset_id,
            &owner,
        )
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

        // place new order
        assert_ok!(order_book.place_limit_order(order, &mut data));

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

        let balance = <Runtime as Config>::AssetInfoProvider::free_balance(
            &order_book_id.base_asset_id,
            &owner,
        )
        .unwrap();
        //let expected_balance = balance_before - amount; // todo (m.tagirov) lock liquidity
        let expected_balance = balance_before;
        assert_eq!(balance, expected_balance);
    });
}

#[test]
fn should_place_nft_limit_order() {
    ext().execute_with(|| {
        // todo
    })
}

#[test]
fn should_not_place_limit_order_when_status_doesnt_allow() {
    // todo
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base_asset_id: XOR.into(),
            target_asset_id: VAL.into(),
        };

        let mut order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());

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
            order_book.place_limit_order(order.clone(), &mut data),
            E::PlacementOfLimitOrdersIsForbidden
        );

        order_book.status = OrderBookStatus::OnlyCancel;
        assert_err!(
            order_book.place_limit_order(order.clone(), &mut data),
            E::PlacementOfLimitOrdersIsForbidden
        );

        order_book.status = OrderBookStatus::PlaceAndCancel;
        assert_ok!(order_book.place_limit_order(order.clone(), &mut data));

        order_book.status = OrderBookStatus::Trade;
        order.id = 2;
        assert_ok!(order_book.place_limit_order(order.clone(), &mut data));
    });
}

#[test]
fn should_not_place_invalid_limit_order() {
    ext().execute_with(|| {
        // todo
    })
}

#[test]
fn should_not_place_invalid_nft_limit_order() {
    ext().execute_with(|| {
        // todo
    })
}

#[test]
fn should_not_place_limit_order_that_doesnt_meet_restrictions() {
    ext().execute_with(|| {
        // todo
    })
}

#[test]
fn should_not_place_limit_order_in_spread() {
    // todo
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
            base_asset_id: XOR.into(),
            target_asset_id: VAL.into(),
        };

        let order_book = OrderBook::<Runtime>::default(order_book_id, DEX.into());

        fill_order_book(&order_book_id, &mut data);
    });
}
