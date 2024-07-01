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

use crate::test_utils::*;
use assets::AssetIdOf;
use common::prelude::QuoteAmount;
use common::{balance, AssetName, AssetSymbol, PriceVariant, DOT, KSM, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::order_book::cache_data_layer::CacheDataLayer;
use framenode_runtime::order_book::storage_data_layer::StorageDataLayer;
use framenode_runtime::order_book::{
    CancelReason, Config, DataLayer, DealInfo, LimitOrder, MarketChange, MarketOrder, MarketRole,
    OrderAmount, OrderBook, OrderBookId, OrderBookStatus, OrderBookTechStatus, OrderPrice,
    OrderVolume, Payment,
};
use framenode_runtime::{Runtime, RuntimeOrigin};
use sp_core::Get;
use sp_runtime::traits::Zero;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::iter::repeat;
use sp_std::vec::Vec;

#[test]
fn should_create_new() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL,
        quote: XOR,
    };

    let expected = OrderBook::<Runtime> {
        order_book_id,
        status: OrderBookStatus::Trade,
        last_order_id: 0,
        tick_size: balance!(0.001).into(),
        step_lot_size: balance!(0.1).into(),
        min_lot_size: balance!(1).into(),
        max_lot_size: balance!(10000).into(),
        tech_status: OrderBookTechStatus::Ready,
    };

    assert_eq!(
        OrderBook::<Runtime>::new(
            order_book_id,
            balance!(0.001).into(),
            balance!(0.1).into(),
            balance!(1).into(),
            balance!(10000).into()
        ),
        expected
    );
}

#[test]
fn should_increment_order_id() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL,
        quote: XOR,
    };

    let mut order_book = OrderBook::<Runtime>::new(
        order_book_id,
        OrderPrice::divisible(balance!(0.00001)),
        OrderVolume::divisible(balance!(0.00001)),
        OrderVolume::divisible(balance!(1)),
        OrderVolume::divisible(balance!(1000)),
    );
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
        let owner = accounts::alice::<Runtime>();
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(owner.clone(), order_book_id);

        let order_id = 100;
        let price = balance!(10).into();
        let amount = balance!(100).into();

        // fix state before
        let bids_before = data.get_bids(&order_book_id, &price).unwrap_or_default();
        let agg_bids_before = data.get_aggregated_bids(&order_book_id);
        let price_volume_before = agg_bids_before.get(&price).cloned().unwrap_or_default();
        let user_orders_before = data
            .get_user_limit_orders(&owner, &order_book_id)
            .unwrap_or_default();
        let balance_before = free_balance::<Runtime>(&order_book_id.quote, &owner);

        // new order
        let order = LimitOrder::<Runtime>::new(
            order_id,
            owner.clone(),
            PriceVariant::Buy,
            price,
            amount,
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let deal_amount = *order.deal_amount(MarketRole::Taker, None).unwrap().value();

        // place new order
        assert_eq!(order_book.place_limit_order(order, &mut data).unwrap(), 0);

        // check
        let mut expected_bids = bids_before;
        assert_ok!(expected_bids.try_push(order_id));
        assert_eq!(
            data.get_bids(&order_book_id, &price).unwrap(),
            expected_bids
        );

        let expected_price_volume = price_volume_before + amount;
        let mut expected_agg_bids = agg_bids_before;
        assert_ok!(expected_agg_bids.try_insert(price, expected_price_volume));
        assert_eq!(data.get_aggregated_bids(&order_book_id), expected_agg_bids);

        let mut expected_user_orders = user_orders_before;
        assert_ok!(expected_user_orders.try_push(order_id));
        assert_eq!(
            data.get_user_limit_orders(&owner, &order_book_id).unwrap(),
            expected_user_orders
        );

        let balance = free_balance::<Runtime>(&order_book_id.quote, &owner);
        let expected_balance = balance_before - deal_amount.balance();
        assert_eq!(balance, expected_balance);
    });
}

#[test]
fn should_place_nft_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let owner = accounts::alice::<Runtime>();
        frame_system::Pallet::<Runtime>::inc_providers(&owner);

        let nft = assets::Pallet::<Runtime>::register_from(
            &owner,
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1,
            false,
            common::AssetType::NFT,
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

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft,
            quote: XOR,
        };

        let order_book = OrderBook::<Runtime>::new(
            order_book_id,
            OrderPrice::divisible(balance!(0.00001)),
            OrderVolume::indivisible(1),
            OrderVolume::indivisible(1),
            OrderVolume::indivisible(1000),
        );

        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        let order_id = 11;
        let price = balance!(10).into();
        let amount = OrderVolume::indivisible(1);

        // new order
        let order = LimitOrder::<Runtime>::new(
            order_id,
            owner.clone(),
            PriceVariant::Sell,
            price,
            amount,
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // place new order
        assert_eq!(order_book.place_limit_order(order, &mut data).unwrap(), 0);

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

        let balance = free_balance::<Runtime>(&order_book_id.base, &owner);
        assert_eq!(balance, balance!(0));
    })
}

#[test]
fn should_place_limit_order_out_of_spread() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);

        let bid_price1 = balance!(10).into();
        let bid_price2 = balance!(9.8).into();
        let bid_price3 = balance!(9.5).into();
        let new_bid_price = balance!(11.1).into();

        let ask_price1 = balance!(11).into();
        let ask_price2 = balance!(11.2).into();
        let ask_price3 = balance!(11.5).into();
        let new_ask_price = balance!(9.9).into();

        let mut alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        let mut alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        // check state before

        assert_eq!(data.get_bids(&order_book_id, &bid_price1).unwrap(), vec![1]);
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(data.get_asks(&order_book_id, &ask_price1).unwrap(), vec![7]);
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(168.5).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price1, balance!(176.3).into()),
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        // buy order 1
        let buy_order_id1 = 101;
        let buy_order1 = LimitOrder::<Runtime>::new(
            buy_order_id1,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            new_bid_price,
            balance!(26.3).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book.place_limit_order(buy_order1, &mut data).unwrap(),
            1
        );

        // check state

        assert_eq!(data.get_bids(&order_book_id, &bid_price1).unwrap(), vec![1]);
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(data.get_asks(&order_book_id, &ask_price1).unwrap(), vec![7]);
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(168.5).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price1, balance!(150).into()),
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        assert_eq!(
            alice_base_balance + balance!(26.3),
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>())
        );
        assert_eq!(
            alice_quote_balance - balance!(289.3),
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>())
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        // buy order 2
        let buy_order_id2 = 102;
        let mut buy_order2 = LimitOrder::<Runtime>::new(
            buy_order_id2,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            new_bid_price,
            balance!(300).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book
                .place_limit_order(buy_order2.clone(), &mut data)
                .unwrap(),
            1
        );

        // check state

        assert_eq!(
            data.get_bids(&order_book_id, &new_bid_price).unwrap(),
            vec![buy_order_id2]
        );
        assert_eq!(data.get_bids(&order_book_id, &bid_price1).unwrap(), vec![1]);
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(data.get_asks(&order_book_id, &ask_price1), None);
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (new_bid_price, balance!(150).into()),
                (bid_price1, balance!(168.5).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        buy_order2.amount = balance!(150).into();
        assert_eq!(
            data.get_limit_order(&order_book_id, buy_order_id2).unwrap(),
            buy_order2
        );

        assert_eq!(
            alice_base_balance + balance!(150),
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>())
        );
        assert_eq!(
            alice_quote_balance - balance!(3315),
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>())
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        // delete limit order
        assert_ok!(data.delete_limit_order(&order_book_id, buy_order_id2));

        // sell order 1
        let sell_order_id1 = 201;
        let sell_order1 = LimitOrder::<Runtime>::new(
            sell_order_id1,
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            new_ask_price,
            balance!(18.5).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book
                .place_limit_order(sell_order1, &mut data)
                .unwrap(),
            1
        );

        // check state

        assert_eq!(data.get_bids(&order_book_id, &bid_price1).unwrap(), vec![1]);
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(data.get_asks(&order_book_id, &ask_price1), None);
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(150).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        assert_eq!(
            alice_base_balance - balance!(18.5),
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>())
        );
        assert_eq!(
            alice_quote_balance + balance!(185),
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>())
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        // sell order 2
        let sell_order_id2 = 202;
        let mut sell_order2 = LimitOrder::<Runtime>::new(
            sell_order_id2,
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            new_ask_price,
            balance!(300).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book
                .place_limit_order(sell_order2.clone(), &mut data)
                .unwrap(),
            1
        );

        // check state

        assert_eq!(data.get_bids(&order_book_id, &bid_price1), None);
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            data.get_asks(&order_book_id, &new_ask_price).unwrap(),
            vec![sell_order_id2]
        );
        assert_eq!(data.get_asks(&order_book_id, &ask_price1), None);
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (new_ask_price, balance!(150).into()),
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        sell_order2.amount = balance!(150).into();
        assert_eq!(
            data.get_limit_order(&order_book_id, sell_order_id2)
                .unwrap(),
            sell_order2
        );

        assert_eq!(
            alice_base_balance - balance!(300),
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>())
        );
        assert_eq!(
            alice_quote_balance + balance!(1500),
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>())
        );
    });
}

#[test]
fn should_not_place_limit_order_when_status_doesnt_allow() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let mut order_book = OrderBook::<Runtime>::new(
            order_book_id,
            OrderPrice::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(1)),
            OrderVolume::divisible(balance!(1000)),
        );
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);

        let mut order = LimitOrder::<Runtime>::new(
            1,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(10).into(),
            balance!(100).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
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
        assert_ok!(order_book.place_limit_order(order, &mut data));
    });
}

#[test]
fn should_not_place_invalid_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = OrderBook::<Runtime>::new(
            order_book_id,
            OrderPrice::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(1)),
            OrderVolume::divisible(balance!(1000)),
        );

        let order = LimitOrder::<Runtime>::new(
            1,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(10).into(),
            balance!(100).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let mut wrong_price_order = order.clone();
        wrong_price_order.price = (balance!(10) + order_book.tick_size.balance() / 100).into();
        assert_err!(
            order_book.place_limit_order(wrong_price_order, &mut data),
            E::InvalidLimitOrderPrice
        );

        let mut too_small_amount_order = order.clone();
        too_small_amount_order.amount = (order_book.min_lot_size.balance() / 2).into();
        assert_err!(
            order_book.place_limit_order(too_small_amount_order, &mut data),
            E::InvalidOrderAmount
        );

        let mut too_big_amount_order = order.clone();
        too_big_amount_order.amount = (order_book.max_lot_size.balance() + 1).into();
        assert_err!(
            order_book.place_limit_order(too_big_amount_order, &mut data),
            E::InvalidOrderAmount
        );

        let mut wrong_amount_order = order;
        wrong_amount_order.amount =
            (balance!(100) + order_book.step_lot_size.balance() / 100).into();
        assert_err!(
            order_book.place_limit_order(wrong_amount_order, &mut data),
            E::InvalidOrderAmount
        );
    })
}

#[test]
fn should_not_place_invalid_nft_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();
        frame_system::Pallet::<Runtime>::inc_providers(&accounts::alice::<Runtime>());

        let nft = assets::Pallet::<Runtime>::register_from(
            &accounts::alice::<Runtime>(),
            AssetSymbol(b"NFT".to_vec()),
            AssetName(b"Nft".to_vec()),
            0,
            1,
            false,
            common::AssetType::NFT,
            None,
            None,
        )
        .unwrap();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: nft,
            quote: XOR,
        };

        let order_book = OrderBook::<Runtime>::new(
            order_book_id,
            OrderPrice::divisible(balance!(0.00001)),
            OrderVolume::indivisible(1),
            OrderVolume::indivisible(1),
            OrderVolume::indivisible(1000),
        );

        let order = LimitOrder::<Runtime>::new(
            1,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(10).into(),
            OrderVolume::indivisible(1),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let mut wrong_price_order = order.clone();
        wrong_price_order.price = (balance!(10) + order_book.tick_size.balance() / 100).into();
        assert_err!(
            order_book.place_limit_order(wrong_price_order, &mut data),
            E::InvalidLimitOrderPrice
        );

        let mut too_small_amount_order = order.clone();
        too_small_amount_order.amount = OrderVolume::indivisible(0);
        assert_err!(
            order_book.place_limit_order(too_small_amount_order, &mut data),
            E::InvalidOrderAmount
        );

        let mut too_big_amount_order = order;
        too_big_amount_order.amount = order_book.max_lot_size + OrderVolume::indivisible(1);
        assert_err!(
            order_book.place_limit_order(too_big_amount_order, &mut data),
            E::InvalidOrderAmount
        );
    })
}

#[test]
fn should_not_place_limit_order_that_doesnt_meet_restrictions_for_user() {
    ext().execute_with(|| {
        let mut data = CacheDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = OrderBook::<Runtime>::new(
            order_book_id,
            OrderPrice::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(1)),
            OrderVolume::divisible(balance!(1000)),
        );
        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);

        let mut order = LimitOrder::<Runtime>::new(
            0,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(10).into(),
            balance!(1).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let max_orders_per_user: u32 = <Runtime as Config>::MaxOpenedLimitOrdersPerUser::get();
        let max_side_price_count: u32 = <Runtime as Config>::MaxSidePriceCount::get();
        let max_orders_per_price: u32 = <Runtime as Config>::MaxLimitOrdersForPrice::get();
        let max_expiring_orders_per_block: u32 =
            <Runtime as Config>::MaxExpiringOrdersPerBlock::get();
        let current_block = frame_system::Pallet::<Runtime>::block_number();

        let mut prices =
            fill_tools::bid_prices_iterator(order_book.tick_size, max_side_price_count)
                .flat_map(move |price| repeat(price).take(max_orders_per_price as usize));
        let mut lifespans =
            fill_tools::lifespans_iterator::<Runtime>(max_expiring_orders_per_block, 3);

        for _ in 0..max_orders_per_user {
            order.id += 1;
            order.price = prices.next().unwrap();
            order.lifespan = lifespans.next().unwrap();
            order.expires_at =
                LimitOrder::<Runtime>::resolve_lifespan(current_block, order.lifespan);
            assert_ok!(order_book.place_limit_order(order.clone(), &mut data));
        }

        order.id += 1;
        order.price += balance!(0.001).into();
        assert_err!(
            order_book.place_limit_order(order, &mut data),
            E::UserHasMaxCountOfOpenedOrders
        );
    })
}

#[test]
fn should_not_place_limit_order_that_doesnt_meet_restrictions_for_orders_in_price() {
    ext().execute_with(|| {
        let mut data = CacheDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = OrderBook::<Runtime>::new(
            order_book_id,
            OrderPrice::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(1)),
            OrderVolume::divisible(balance!(1000)),
        );

        OrderBookPallet::register_tech_account(order_book_id).unwrap();
        let max_orders_for_price: u32 = <Runtime as Config>::MaxLimitOrdersForPrice::get();

        let mut buy_order = LimitOrder::<Runtime>::new(
            0,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(10).into(),
            balance!(100).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let mut sell_order = LimitOrder::<Runtime>::new(
            max_orders_for_price as u128 + 1000,
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            balance!(11).into(),
            balance!(100).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        for i in 0..max_orders_for_price {
            // get new owner for each order to not get UserHasMaxCountOfOpenedOrders error
            let account = accounts::generate_account::<Runtime>(i);

            fill_balance::<Runtime>(account.clone(), order_book_id);

            buy_order.id += 1;
            buy_order.owner = account.clone();
            // should ideally be set through `LimitOrder::new`
            // but we do it in a hacky way for simplicity
            buy_order.expires_at += 1;
            sell_order.id += 1;
            sell_order.owner = account;
            // should ideally be set through `LimitOrder::new`
            // but we do it in a hacky way for simplicity
            sell_order.expires_at += 1;

            assert_ok!(order_book.place_limit_order(buy_order.clone(), &mut data));
            assert_ok!(order_book.place_limit_order(sell_order.clone(), &mut data));
        }

        buy_order.id += 1;
        sell_order.id += 1;
        assert_err!(
            order_book.place_limit_order(buy_order, &mut data),
            E::PriceReachedMaxCountOfLimitOrders
        );
        assert_err!(
            order_book.place_limit_order(sell_order, &mut data),
            E::PriceReachedMaxCountOfLimitOrders
        );
    })
}

#[test]
fn should_not_place_limit_order_that_doesnt_meet_restrictions_for_side() {
    ext().execute_with(|| {
        let mut data = CacheDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = OrderBook::<Runtime>::new(
            order_book_id,
            OrderPrice::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(0.00001)),
            OrderVolume::divisible(balance!(1)),
            OrderVolume::divisible(balance!(1000)),
        );
        OrderBookPallet::register_tech_account(order_book_id).unwrap();
        let max_prices_for_side: u32 = <Runtime as Config>::MaxSidePriceCount::get();

        let mut buy_order = LimitOrder::<Runtime>::new(
            0,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(1000).into(),
            balance!(100).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let mut sell_order = LimitOrder::<Runtime>::new(
            max_prices_for_side as u128 + 1000,
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            balance!(1001).into(),
            balance!(100).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        for i in 0..max_prices_for_side {
            // get new owner for each order to not get UserHasMaxCountOfOpenedOrders error
            let account = accounts::generate_account::<Runtime>(i);

            fill_balance::<Runtime>(account.clone(), order_book_id);

            buy_order.id += 1;
            buy_order.owner = account.clone();
            buy_order.price -= order_book.tick_size;
            // should ideally be set through `LimitOrder::new`
            // but we do it in a hacky way for simplicity
            buy_order.expires_at += 1;

            sell_order.id += 1;
            sell_order.owner = account;
            sell_order.price += order_book.tick_size;
            // should ideally be set through `LimitOrder::new`
            // but we do it in a hacky way for simplicity
            sell_order.expires_at += 1;

            assert_ok!(order_book.place_limit_order(buy_order.clone(), &mut data));
            assert_ok!(order_book.place_limit_order(sell_order.clone(), &mut data));
        }

        buy_order.id += 1;
        buy_order.price -= order_book.tick_size;

        sell_order.id += 1;
        sell_order.price += order_book.tick_size;

        assert_err!(
            order_book.place_limit_order(buy_order, &mut data),
            E::OrderBookReachedMaxCountOfPricesForSide
        );
        assert_err!(
            order_book.place_limit_order(sell_order, &mut data),
            E::OrderBookReachedMaxCountOfPricesForSide
        );
    })
}

#[test]
fn should_not_place_limit_order_in_spread() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let mut order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let buy_price = balance!(11.1).into(); // above the spread, in the asks zone
        let buy_order = LimitOrder::<Runtime>::new(
            101,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            buy_price,
            balance!(100).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let sell_price = balance!(9.9).into(); // below the spread, in the bids zone
        let sell_order = LimitOrder::<Runtime>::new(
            102,
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            sell_price,
            balance!(100).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        // Stop & OnlyCancel statuses don't allow to place limit orders
        // Trade status should proceed another market mechanism
        // This test case is reachable only for PlaceAndCancel status
        order_book.status = OrderBookStatus::PlaceAndCancel;

        assert_err!(
            order_book.place_limit_order(buy_order, &mut data),
            E::InvalidLimitOrderPrice
        );
        assert_err!(
            order_book.place_limit_order(sell_order, &mut data),
            E::InvalidLimitOrderPrice
        );
    });
}

#[test]
fn should_cancel_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

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
        let balance_before = free_balance::<Runtime>(&order_book_id.quote, &order.owner);

        // cancel the limit order
        assert_ok!(order_book.cancel_limit_order(order.clone(), &mut data));

        let deal_amount = *order.deal_amount(MarketRole::Taker, None).unwrap().value();

        // check
        let mut expected_bids = bids_before;
        expected_bids.retain(|&id| id != order.id);
        assert_eq!(
            data.get_bids(&order_book_id, &order.price).unwrap(),
            expected_bids
        );

        let expected_price_volume = price_volume_before - order.amount;
        let mut expected_agg_bids = agg_bids_before;
        assert_ok!(expected_agg_bids.try_insert(order.price, expected_price_volume));
        assert_eq!(data.get_aggregated_bids(&order_book_id), expected_agg_bids);

        let mut expected_user_orders = user_orders_before;
        expected_user_orders.retain(|&id| id != order.id);
        assert_eq!(
            data.get_user_limit_orders(&order.owner, &order_book_id)
                .unwrap(),
            expected_user_orders
        );

        let balance = free_balance::<Runtime>(&order_book_id.quote, &order.owner);
        let expected_balance = balance_before + deal_amount.balance();
        assert_eq!(balance, expected_balance);
    });
}

#[test]
fn should_not_cancel_unknown_limit_order() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let unknown_order = LimitOrder::<Runtime>::new(
            1234,
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            balance!(10).into(),
            balance!(100).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_err!(
            order_book.cancel_limit_order(unknown_order, &mut data),
            E::UnknownLimitOrder
        );
    });
}

#[test]
fn should_not_cancel_limit_order_when_status_doesnt_allow() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let mut order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        let order2 = data.get_limit_order(&order_book_id, 2).unwrap();
        let order3 = data.get_limit_order(&order_book_id, 3).unwrap();

        order_book.status = OrderBookStatus::Stop;
        assert_err!(
            order_book.cancel_limit_order(order1.clone(), &mut data),
            E::CancellationOfLimitOrdersIsForbidden
        );

        order_book.status = OrderBookStatus::Trade;
        assert_ok!(order_book.cancel_limit_order(order1, &mut data));

        order_book.status = OrderBookStatus::PlaceAndCancel;
        assert_ok!(order_book.cancel_limit_order(order2, &mut data));

        order_book.status = OrderBookStatus::OnlyCancel;
        assert_ok!(order_book.cancel_limit_order(order3, &mut data));
    });
}

#[test]
fn should_cancel_all_limit_orders() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();
        let owner = accounts::bob::<Runtime>();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let tech_account = technical::Pallet::<Runtime>::tech_account_id_to_account_id(
            &OrderBookPallet::tech_account_for_order_book(order_book_id),
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
        assert_ne!(
            free_balance::<Runtime>(&order_book_id.base, &owner),
            INIT_BALANCE
        );
        assert_ne!(
            free_balance::<Runtime>(&order_book_id.quote, &owner),
            INIT_BALANCE
        );

        // tech account keeps the locked assets
        assert!(free_balance::<Runtime>(&order_book_id.base, &tech_account) > balance!(0));
        assert!(free_balance::<Runtime>(&order_book_id.quote, &tech_account) > balance!(0));

        // cancel all orders
        assert_ok!(order_book.cancel_all_limit_orders(CancelReason::Manual, &mut data));

        // empty after canceling of all limit orders
        assert!(data.get_all_limit_orders(&order_book_id).is_empty());
        assert!(data.get_aggregated_bids(&order_book_id).is_empty());
        assert!(data.get_aggregated_asks(&order_book_id).is_empty());
        assert_eq!(data.get_user_limit_orders(&owner, &order_book_id), None);

        // locked balance is unlocked
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &owner),
            INIT_BALANCE
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &owner),
            INIT_BALANCE
        );

        // tech account balance is empty after canceling of all limit orders
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &tech_account),
            balance!(0)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &tech_account),
            balance!(0)
        );
    });
}

#[test]
fn should_not_get_best_bid_from_empty_order_book() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_empty_order_book::<Runtime>(order_book_id);

        assert_eq!(order_book.best_bid(&mut data), None);
    });
}

#[test]
fn should_get_best_bid() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_eq!(
            order_book.best_bid(&mut data).unwrap(),
            (balance!(10).into(), balance!(168.5).into())
        );
    });
}

#[test]
fn should_not_get_best_ask_from_empty_order_book() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_empty_order_book::<Runtime>(order_book_id);

        assert_eq!(order_book.best_ask(&mut data), None);
    });
}

#[test]
fn should_get_best_ask() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_eq!(
            order_book.best_ask(&mut data).unwrap(),
            (balance!(11).into(), balance!(176.3).into())
        );
    });
}

#[test]
fn should_not_get_direction_if_any_asset_is_not_in_order_book_id() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_err!(order_book.get_direction(&DOT, &KSM), E::InvalidAsset);
        assert_err!(order_book.get_direction(&XOR, &KSM), E::InvalidAsset);
        assert_err!(order_book.get_direction(&DOT, &VAL), E::InvalidAsset);
        assert_err!(order_book.get_direction(&VAL, &VAL), E::InvalidAsset);
        assert_err!(order_book.get_direction(&XOR, &XOR), E::InvalidAsset);
    });
}

#[test]
fn should_get_direction() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_eq!(
            order_book.get_direction(&XOR, &VAL).unwrap(),
            PriceVariant::Buy
        );
        assert_eq!(
            order_book.get_direction(&VAL, &XOR).unwrap(),
            PriceVariant::Sell
        );
    });
}

#[test]
fn should_align_amount() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_empty_order_book::<Runtime>(order_book_id);

        // default step = 0.00001
        assert_eq!(
            order_book.align_amount(balance!(10.01).into()),
            balance!(10.01).into()
        );
        assert_eq!(
            order_book.align_amount(balance!(10.00001).into()),
            balance!(10.00001).into()
        );
        assert_eq!(
            order_book.align_amount(balance!(10.000011).into()),
            balance!(10.00001).into()
        );
        assert_eq!(
            order_book.align_amount(balance!(10.000001).into()),
            balance!(10).into()
        );
        assert_eq!(
            order_book.align_amount(balance!(10).into()),
            balance!(10).into()
        );
        assert_eq!(
            order_book.align_amount(balance!(0.00001).into()),
            balance!(0.00001).into()
        );
        assert_eq!(
            order_book.align_amount(balance!(0.00000123).into()),
            balance!(0).into()
        );
        assert_eq!(
            order_book.align_amount(balance!(9.999999999999).into()),
            balance!(9.99999).into()
        );
        assert_eq!(
            order_book.align_amount(OrderVolume::zero()),
            OrderVolume::zero()
        );
    });
}

#[test]
fn should_not_sum_market_with_filled_target_if_limit_is_greater_than_liquidity() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let asks = data.get_aggregated_asks(&order_book_id);
        let bids = data.get_aggregated_bids(&order_book_id);

        let filled_target = true;

        assert_err!(
            order_book.sum_market(
                asks.iter(),
                Some(OrderAmount::Base(balance!(1000).into())),
                filled_target
            ),
            E::NotEnoughLiquidityInOrderBook
        );
        assert_err!(
            order_book.sum_market(
                asks.iter(),
                Some(OrderAmount::Quote(balance!(10000).into())),
                filled_target
            ),
            E::NotEnoughLiquidityInOrderBook
        );
        assert_err!(
            order_book.sum_market(
                bids.iter().rev(),
                Some(OrderAmount::Base(balance!(1000).into())),
                filled_target
            ),
            E::NotEnoughLiquidityInOrderBook
        );
        assert_err!(
            order_book.sum_market(
                bids.iter().rev(),
                Some(OrderAmount::Quote(balance!(10000).into())),
                filled_target
            ),
            E::NotEnoughLiquidityInOrderBook
        );
    });
}

#[test]
fn should_sum_market_without_filled_target_if_limit_is_greater_than_liquidity() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let asks = data.get_aggregated_asks(&order_book_id);
        let bids = data.get_aggregated_bids(&order_book_id);

        let filled_target = false;

        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Base(balance!(1000).into())),
                    filled_target
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(610.7).into()),
                OrderAmount::Quote(balance!(6881.32).into())
            )
        );
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Quote(balance!(10000).into())),
                    filled_target
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(610.7).into()),
                OrderAmount::Quote(balance!(6881.32).into())
            )
        );
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Base(balance!(1000).into())),
                    filled_target
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(569.7).into()),
                OrderAmount::Quote(balance!(5538.37).into())
            )
        );
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Quote(balance!(10000).into())),
                    filled_target
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(569.7).into()),
                OrderAmount::Quote(balance!(5538.37).into())
            )
        );
    });
}

#[test]
fn should_sum_market_with_zero_limit() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let asks = data.get_aggregated_asks(&order_book_id);
        let bids = data.get_aggregated_bids(&order_book_id);

        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Base(balance!(0).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(0).into()),
                OrderAmount::Quote(balance!(0).into())
            )
        );
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Quote(balance!(0).into())),
                    false
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(0).into()),
                OrderAmount::Quote(balance!(0).into())
            )
        );
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Base(balance!(0).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(0).into()),
                OrderAmount::Quote(balance!(0).into())
            )
        );
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Quote(balance!(0).into())),
                    false
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(0).into()),
                OrderAmount::Quote(balance!(0).into())
            )
        );
    });
}

#[test]
fn should_sum_market() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let asks = data.get_aggregated_asks(&order_book_id);
        let bids = data.get_aggregated_bids(&order_book_id);

        // impacts 1 price
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Base(balance!(100).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(100).into()),
                OrderAmount::Quote(balance!(1100).into())
            )
        );
        // impacts 2 prices
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Base(balance!(200).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(200).into()),
                OrderAmount::Quote(balance!(2204.74).into())
            )
        );
        // impacts 3 prices
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Base(balance!(400).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(400).into()),
                OrderAmount::Quote(balance!(4458.27).into())
            )
        );
        // impacts all orders
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Base(balance!(610.7).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(610.7).into()),
                OrderAmount::Quote(balance!(6881.32).into())
            )
        );

        // impacts 1 price
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Quote(balance!(1000).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(90.90909).into()),
                OrderAmount::Quote(balance!(999.99999).into())
            )
        );
        // impacts 2 prices
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Quote(balance!(3000).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(271.00535).into()),
                OrderAmount::Quote(balance!(2999.99992).into())
            )
        );
        // impacts 3 prices
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Quote(balance!(5000).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(447.10695).into()),
                OrderAmount::Quote(balance!(4999.999925).into())
            )
        );
        // impacts all orders
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Quote(balance!(6881.32).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(610.7).into()),
                OrderAmount::Quote(balance!(6881.32).into())
            )
        );

        // impacts 1 price
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Base(balance!(100).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(100).into()),
                OrderAmount::Quote(balance!(1000).into())
            )
        );
        // impacts 2 prices
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Base(balance!(200).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(200).into()),
                OrderAmount::Quote(balance!(1993.7).into())
            )
        );
        // impacts 3 prices
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Base(balance!(400).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(400).into()),
                OrderAmount::Quote(balance!(3926.22).into())
            )
        );
        // impacts all orders
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Base(balance!(569.7).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(569.7).into()),
                OrderAmount::Quote(balance!(5538.37).into())
            )
        );

        // impacts 1 price
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Quote(balance!(1000).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(100).into()),
                OrderAmount::Quote(balance!(1000).into())
            )
        );
        // impacts 2 prices
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Quote(balance!(2500).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(251.66326).into()),
                OrderAmount::Quote(balance!(2499.999948).into())
            )
        );
        // impacts 3 prices
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Quote(balance!(4500).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(460.39789).into()),
                OrderAmount::Quote(balance!(4499.999955).into())
            )
        );
        // impacts all orders
        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Quote(balance!(5538.37).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(569.7).into()),
                OrderAmount::Quote(balance!(5538.37).into())
            )
        );

        // without depth limit
        assert_eq!(
            order_book.sum_market(asks.iter(), None, false).unwrap(),
            (
                OrderAmount::Base(balance!(610.7).into()),
                OrderAmount::Quote(balance!(6881.32).into())
            )
        );
        assert_eq!(
            order_book
                .sum_market(bids.iter().rev(), None, true)
                .unwrap(),
            (
                OrderAmount::Base(balance!(569.7).into()),
                OrderAmount::Quote(balance!(5538.37).into())
            )
        );

        // base is aligned
        assert_eq!(
            order_book
                .sum_market(
                    asks.iter(),
                    Some(OrderAmount::Base(balance!(200.123456789).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(200.12345).into()),
                OrderAmount::Quote(balance!(2206.12264).into())
            )
        );

        assert_eq!(
            order_book
                .sum_market(
                    bids.iter().rev(),
                    Some(OrderAmount::Base(balance!(200.00000123).into())),
                    true
                )
                .unwrap(),
            (
                OrderAmount::Base(balance!(200).into()),
                OrderAmount::Quote(balance!(1993.7).into())
            )
        );
    });
}

#[test]
fn should_not_calculate_deal_with_small_amount() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

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

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

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
                input_amount: OrderAmount::Quote(balance!(2999.99992).into()),
                output_asset_id: VAL,
                output_amount: OrderAmount::Base(balance!(271.00535).into()),
                average_price: balance!(11.069891867448373251).into(),
                direction: PriceVariant::Buy
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
                input_amount: OrderAmount::Quote(balance!(2204.74).into()),
                output_asset_id: VAL,
                output_amount: OrderAmount::Base(balance!(200).into()),
                average_price: balance!(11.0237).into(),
                direction: PriceVariant::Buy
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
                input_amount: OrderAmount::Base(balance!(200).into()),
                output_asset_id: XOR,
                output_amount: OrderAmount::Quote(balance!(1993.7).into()),
                average_price: balance!(9.9685).into(),
                direction: PriceVariant::Sell
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
                input_amount: OrderAmount::Base(balance!(251.66326).into()),
                output_asset_id: XOR,
                output_amount: OrderAmount::Quote(balance!(2499.999948).into()),
                average_price: balance!(9.933909097418510751).into(),
                direction: PriceVariant::Sell
            }
        );
    });
}

#[test]
fn should_not_execute_market_order_with_non_trade_status() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let mut order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let order = MarketOrder::<Runtime>::new(
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            order_book_id,
            balance!(10).into(),
            None,
        );

        order_book.status = OrderBookStatus::PlaceAndCancel;
        assert_err!(
            order_book.execute_market_order(order.clone(), &mut data),
            E::TradingIsForbidden
        );

        order_book.status = OrderBookStatus::OnlyCancel;
        assert_err!(
            order_book.execute_market_order(order.clone(), &mut data),
            E::TradingIsForbidden
        );

        order_book.status = OrderBookStatus::Stop;
        assert_err!(
            order_book.execute_market_order(order, &mut data),
            E::TradingIsForbidden
        );
    });
}

#[test]
fn should_not_execute_market_order_with_empty_amount() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let wrong_amount = OrderVolume::zero();
        let order = MarketOrder::<Runtime>::new(
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            order_book_id,
            wrong_amount,
            None,
        );

        assert_err!(
            order_book.execute_market_order(order, &mut data),
            E::InvalidOrderAmount
        );
    });
}

#[test]
fn should_not_execute_market_order_with_invalid_order_book_id() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let wrong_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: DOT,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let order = MarketOrder::<Runtime>::new(
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            wrong_order_book_id,
            balance!(100).into(),
            None,
        );

        assert_err!(
            order_book.execute_market_order(order, &mut data),
            E::InvalidOrderBookId
        );
    });
}

#[test]
fn should_not_execute_market_order_with_invalid_amount() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let order = MarketOrder::<Runtime>::new(
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            order_book_id,
            balance!(1).into(),
            None,
        );

        let mut wrong_amount_order = order.clone();
        wrong_amount_order.amount = balance!(1.123456).into();
        assert_err!(
            order_book.execute_market_order(wrong_amount_order, &mut data),
            E::InvalidOrderAmount
        );

        let mut too_small_amount_order = order.clone();
        too_small_amount_order.amount = (order_book.min_lot_size.balance() / 2).into();
        assert_err!(
            order_book.execute_market_order(too_small_amount_order, &mut data),
            E::InvalidOrderAmount
        );

        let mut too_big_amount_order = order;
        too_big_amount_order.amount = (order_book.max_lot_size.balance() + 1).into();
        assert_err!(
            order_book.execute_market_order(too_big_amount_order, &mut data),
            E::InvalidOrderAmount
        );
    });
}

#[test]
fn should_execute_market_order_and_transfer_to_owner() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);

        let mut alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        let mut alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        let mut bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        let mut bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        let mut charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        let mut charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        let mut buy_order = MarketOrder::<Runtime>::new(
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            order_book_id,
            balance!(150).into(),
            None,
        );
        let mut sell_order = MarketOrder::<Runtime>::new(
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            order_book_id,
            balance!(150).into(),
            None,
        );

        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11).into(), balance!(176.3).into()),
                (balance!(11.2).into(), balance!(178.6).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(10).into(), balance!(168.5).into())
            ])
        );

        // 1st buy order
        assert_eq!(
            order_book
                .execute_market_order(buy_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1650).into()),
                OrderAmount::Base(balance!(150).into()),
                1
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(1650)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance + balance!(1650)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11).into(), balance!(26.3).into()),
                (balance!(11.2).into(), balance!(178.6).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(10).into(), balance!(168.5).into())
            ])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        // 2nd buy order
        assert_eq!(
            order_book
                .execute_market_order(buy_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1674.74).into()),
                OrderAmount::Base(balance!(150).into()),
                3
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(1674.74)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance + balance!(718.26)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance + balance!(956.48)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11.2).into(), balance!(54.9).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(10).into(), balance!(168.5).into())
            ])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        // 3rd buy order
        assert_eq!(
            order_book
                .execute_market_order(buy_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1708.53).into()),
                OrderAmount::Base(balance!(150).into()),
                3
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(1708.53)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance + balance!(1287.63)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance + balance!(420.9)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5).into(), balance!(160.7).into())])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(10).into(), balance!(168.5).into())
            ])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        // 1st sell order
        assert_eq!(
            order_book
                .execute_market_order(sell_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Base(balance!(150).into()),
                OrderAmount::Quote(balance!(1500).into()),
                1
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance + balance!(1500)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5).into(), balance!(160.7).into())])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(10).into(), balance!(18.5).into())
            ])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        // 2nd sell order
        assert_eq!(
            order_book
                .execute_market_order(sell_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Base(balance!(150).into()),
                OrderAmount::Quote(balance!(1473.7).into()),
                3
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance + balance!(1473.7)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance + balance!(54.8)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance + balance!(95.2)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5).into(), balance!(160.7).into())])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(8.4).into()),
            ])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        // 3rd sell order
        assert_eq!(
            order_book
                .execute_market_order(sell_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Base(balance!(150).into()),
                OrderAmount::Quote(balance!(1427.52).into()),
                3
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance + balance!(1427.52)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance + balance!(93.6)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance + balance!(56.4)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5).into(), balance!(160.7).into())])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(balance!(9.5).into(), balance!(119.7).into()),])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        // buy & sell remaining amounts
        buy_order.amount = balance!(160.7).into();
        sell_order.amount = balance!(119.7).into();

        assert_eq!(
            order_book
                .execute_market_order(buy_order, &mut data)
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1848.05).into()),
                OrderAmount::Base(balance!(160.7).into()),
                2
            )
        );
        assert_eq!(
            order_book
                .execute_market_order(sell_order, &mut data)
                .unwrap(),
            (
                OrderAmount::Base(balance!(119.7).into()),
                OrderAmount::Quote(balance!(1137.15).into()),
                2
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance + balance!(160.7) - balance!(119.7)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(1848.05) + balance!(1137.15)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance + balance!(4.7)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance + balance!(1690.5)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance + balance!(115)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
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

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);

        let mut alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        let mut alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        let mut bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        let mut bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        let mut charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        let mut charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        let mut dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        let mut dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        let mut buy_order = MarketOrder::<Runtime>::new(
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            order_book_id,
            balance!(150).into(),
            Some(accounts::dave::<Runtime>()),
        );
        let mut sell_order = MarketOrder::<Runtime>::new(
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            order_book_id,
            balance!(150).into(),
            Some(accounts::dave::<Runtime>()),
        );

        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11).into(), balance!(176.3).into()),
                (balance!(11.2).into(), balance!(178.6).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(10).into(), balance!(168.5).into())
            ])
        );

        // 1st buy order
        assert_eq!(
            order_book
                .execute_market_order(buy_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1650).into()),
                OrderAmount::Base(balance!(150).into()),
                1
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(1650)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance + balance!(1650)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11).into(), balance!(26.3).into()),
                (balance!(11.2).into(), balance!(178.6).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(10).into(), balance!(168.5).into())
            ])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // 2nd buy order
        assert_eq!(
            order_book
                .execute_market_order(buy_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1674.74).into()),
                OrderAmount::Base(balance!(150).into()),
                3
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(1674.74)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance + balance!(718.26)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance + balance!(956.48)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (balance!(11.2).into(), balance!(54.9).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(10).into(), balance!(168.5).into())
            ])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // 3rd buy order
        assert_eq!(
            order_book
                .execute_market_order(buy_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1708.53).into()),
                OrderAmount::Base(balance!(150).into()),
                3
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(1708.53)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance + balance!(1287.63)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance + balance!(420.9)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5).into(), balance!(160.7).into())])
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(10).into(), balance!(168.5).into())
            ])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // 1st sell order
        assert_eq!(
            order_book
                .execute_market_order(sell_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Base(balance!(150).into()),
                OrderAmount::Quote(balance!(1500).into()),
                1
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance + balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance + balance!(1500)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5).into(), balance!(160.7).into())])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(10).into(), balance!(18.5).into())
            ])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // 2nd sell order
        assert_eq!(
            order_book
                .execute_market_order(sell_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Base(balance!(150).into()),
                OrderAmount::Quote(balance!(1473.7).into()),
                3
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance + balance!(54.8)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance + balance!(95.2)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance + balance!(1473.7)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5).into(), balance!(160.7).into())])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (balance!(9.5).into(), balance!(261.3).into()),
                (balance!(9.8).into(), balance!(8.4).into()),
            ])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // 3rd sell order
        assert_eq!(
            order_book
                .execute_market_order(sell_order.clone(), &mut data)
                .unwrap(),
            (
                OrderAmount::Base(balance!(150).into()),
                OrderAmount::Quote(balance!(1427.52).into()),
                3
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(150)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance + balance!(93.6)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance + balance!(56.4)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance + balance!(1427.52)
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([(balance!(11.5).into(), balance!(160.7).into())])
        );
        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([(balance!(9.5).into(), balance!(119.7).into()),])
        );

        alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());

        bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());

        charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());

        dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // buy & sell remaining amounts
        buy_order.amount = balance!(160.7).into();
        sell_order.amount = balance!(119.7).into();

        assert_eq!(
            order_book
                .execute_market_order(buy_order, &mut data)
                .unwrap(),
            (
                OrderAmount::Quote(balance!(1848.05).into()),
                OrderAmount::Base(balance!(160.7).into()),
                2
            )
        );
        assert_eq!(
            order_book
                .execute_market_order(sell_order, &mut data)
                .unwrap(),
            (
                OrderAmount::Base(balance!(119.7).into()),
                OrderAmount::Quote(balance!(1137.15).into()),
                2
            )
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>()),
            alice_base_balance - balance!(119.7)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>()),
            alice_quote_balance - balance!(1848.05)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>()),
            bob_base_balance + balance!(4.7)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>()),
            bob_quote_balance + balance!(1690.5)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>()),
            charlie_base_balance + balance!(115)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>()),
            charlie_quote_balance + balance!(157.55)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>()),
            dave_base_balance + balance!(160.7)
        );
        assert_eq!(
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>()),
            dave_quote_balance + balance!(1137.15)
        );
        assert_eq!(data.get_aggregated_asks(&order_book_id), BTreeMap::from([]));
        assert_eq!(data.get_aggregated_bids(&order_book_id), BTreeMap::from([]));
    });
}

#[test]
fn should_align_limit_orders() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let mut order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        // change lot size precision
        order_book.step_lot_size = balance!(1).into();

        let limit_order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        let limit_order2 = data.get_limit_order(&order_book_id, 2).unwrap();
        let limit_order3 = data.get_limit_order(&order_book_id, 3).unwrap();
        let limit_order4 = data.get_limit_order(&order_book_id, 4).unwrap();
        let limit_order5 = data.get_limit_order(&order_book_id, 5).unwrap();
        let limit_order6 = data.get_limit_order(&order_book_id, 6).unwrap();
        let limit_order7 = data.get_limit_order(&order_book_id, 7).unwrap();
        let limit_order8 = data.get_limit_order(&order_book_id, 8).unwrap();
        let limit_order9 = data.get_limit_order(&order_book_id, 9).unwrap();
        let limit_order10 = data.get_limit_order(&order_book_id, 10).unwrap();
        let limit_order11 = data.get_limit_order(&order_book_id, 11).unwrap();
        let limit_order12 = data.get_limit_order(&order_book_id, 12).unwrap();

        // check that amounts are original before align
        assert_eq!(limit_order1.amount, balance!(168.5).into());
        assert_eq!(limit_order2.amount, balance!(95.2).into());
        assert_eq!(limit_order3.amount, balance!(44.7).into());
        assert_eq!(limit_order4.amount, balance!(56.4).into());
        assert_eq!(limit_order5.amount, balance!(89.9).into());
        assert_eq!(limit_order6.amount, balance!(115).into());
        assert_eq!(limit_order7.amount, balance!(176.3).into());
        assert_eq!(limit_order8.amount, balance!(85.4).into());
        assert_eq!(limit_order9.amount, balance!(93.2).into());
        assert_eq!(limit_order10.amount, balance!(36.6).into());
        assert_eq!(limit_order11.amount, balance!(205.5).into());
        assert_eq!(limit_order12.amount, balance!(13.7).into());

        let limit_orders = OrderBookPallet::get_limit_orders(&order_book_id, None, 100);

        // align
        assert_ok!(order_book.align_limit_orders(limit_orders, &mut data));

        let limit_order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        let limit_order2 = data.get_limit_order(&order_book_id, 2).unwrap();
        let limit_order3 = data.get_limit_order(&order_book_id, 3).unwrap();
        let limit_order4 = data.get_limit_order(&order_book_id, 4).unwrap();
        let limit_order5 = data.get_limit_order(&order_book_id, 5).unwrap();
        let limit_order6 = data.get_limit_order(&order_book_id, 6).unwrap();
        let limit_order7 = data.get_limit_order(&order_book_id, 7).unwrap();
        let limit_order8 = data.get_limit_order(&order_book_id, 8).unwrap();
        let limit_order9 = data.get_limit_order(&order_book_id, 9).unwrap();
        let limit_order10 = data.get_limit_order(&order_book_id, 10).unwrap();
        let limit_order11 = data.get_limit_order(&order_book_id, 11).unwrap();
        let limit_order12 = data.get_limit_order(&order_book_id, 12).unwrap();

        // check that amouts are aligned
        assert_eq!(limit_order1.amount, balance!(168).into());
        assert_eq!(limit_order2.amount, balance!(95).into());
        assert_eq!(limit_order3.amount, balance!(44).into());
        assert_eq!(limit_order4.amount, balance!(56).into());
        assert_eq!(limit_order5.amount, balance!(89).into());
        assert_eq!(limit_order6.amount, balance!(115).into());
        assert_eq!(limit_order7.amount, balance!(176).into());
        assert_eq!(limit_order8.amount, balance!(85).into());
        assert_eq!(limit_order9.amount, balance!(93).into());
        assert_eq!(limit_order10.amount, balance!(36).into());
        assert_eq!(limit_order11.amount, balance!(205).into());
        assert_eq!(limit_order12.amount, balance!(13).into());
    });
}

#[test]
fn should_not_calculate_market_order_impact_with_empty_side() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_empty_order_book::<Runtime>(order_book_id);

        assert_err!(
            order_book.calculate_market_order_impact(
                MarketOrder::<Runtime>::new(
                    accounts::alice::<Runtime>(),
                    PriceVariant::Buy,
                    order_book_id,
                    balance!(1).into(),
                    None
                ),
                &mut data
            ),
            E::NotEnoughLiquidityInOrderBook
        );
        assert_err!(
            order_book.calculate_market_order_impact(
                MarketOrder::<Runtime>::new(
                    accounts::alice::<Runtime>(),
                    PriceVariant::Sell,
                    order_book_id,
                    balance!(1).into(),
                    None
                ),
                &mut data
            ),
            E::NotEnoughLiquidityInOrderBook
        );
    });
}

#[test]
fn should_not_calculate_market_order_impact_if_liquidity_is_not_enough() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        assert_err!(
            order_book.calculate_market_order_impact(
                MarketOrder::<Runtime>::new(
                    accounts::alice::<Runtime>(),
                    PriceVariant::Buy,
                    order_book_id,
                    balance!(1000).into(),
                    None
                ),
                &mut data
            ),
            E::NotEnoughLiquidityInOrderBook
        );
        assert_err!(
            order_book.calculate_market_order_impact(
                MarketOrder::<Runtime>::new(
                    accounts::alice::<Runtime>(),
                    PriceVariant::Sell,
                    order_book_id,
                    balance!(1000).into(),
                    None
                ),
                &mut data
            ),
            E::NotEnoughLiquidityInOrderBook
        );
    });
}

#[test]
fn should_calculate_market_order_impact() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let limit_order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        let limit_order2 = data.get_limit_order(&order_book_id, 2).unwrap();
        let limit_order3 = data.get_limit_order(&order_book_id, 3).unwrap();
        let limit_order4 = data.get_limit_order(&order_book_id, 4).unwrap();
        let limit_order5 = data.get_limit_order(&order_book_id, 5).unwrap();
        let limit_order6 = data.get_limit_order(&order_book_id, 6).unwrap();
        let limit_order7 = data.get_limit_order(&order_book_id, 7).unwrap();
        let limit_order8 = data.get_limit_order(&order_book_id, 8).unwrap();
        let limit_order9 = data.get_limit_order(&order_book_id, 9).unwrap();
        let limit_order10 = data.get_limit_order(&order_book_id, 10).unwrap();
        let limit_order11 = data.get_limit_order(&order_book_id, 11).unwrap();
        let limit_order12 = data.get_limit_order(&order_book_id, 12).unwrap();

        let buy_amount1 = balance!(100).into();
        let buy_amount2 = balance!(300).into();
        let buy_amount3 = balance!(600).into();
        let buy_amount4 = balance!(391.5).into();
        let buy_amount5 = balance!(610.7).into();

        let mut limit_order1_changed = limit_order1.clone();
        limit_order1_changed.amount -= buy_amount1;
        let mut limit_order3_changed = limit_order3.clone();
        limit_order3_changed.amount -= balance!(6.3).into();
        let mut limit_order5_changed = limit_order5.clone();
        limit_order5_changed.amount -= balance!(35.2).into();
        let mut limit_order7_changed = limit_order7.clone();
        limit_order7_changed.amount -= buy_amount1;
        let mut limit_order9_changed = limit_order9.clone();
        limit_order9_changed.amount -= balance!(38.3).into();
        let mut limit_order12_changed = limit_order12.clone();
        limit_order12_changed.amount -= balance!(3).into();

        assert_eq!(
            order_book
                .calculate_market_order_impact(
                    MarketOrder::<Runtime>::new(
                        accounts::alice::<Runtime>(),
                        PriceVariant::Buy,
                        order_book_id,
                        buy_amount1,
                        None
                    ),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Quote(balance!(1100).into())),
                deal_output: Some(OrderAmount::Base(buy_amount1)),
                market_input: None,
                market_output: Some(OrderAmount::Base(buy_amount1)),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([(
                    7,
                    (limit_order7_changed, OrderAmount::Base(buy_amount1))
                )]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(1100).into())])
                    )]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(accounts::alice::<Runtime>(), buy_amount1)])
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([(accounts::bob::<Runtime>(), balance!(1100).into())])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_market_order_impact(
                    MarketOrder::<Runtime>::new(
                        accounts::alice::<Runtime>(),
                        PriceVariant::Buy,
                        order_book_id,
                        buy_amount2,
                        Some(accounts::dave::<Runtime>())
                    ),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Quote(balance!(3324.74).into())),
                deal_output: Some(OrderAmount::Base(buy_amount2)),
                market_input: None,
                market_output: Some(OrderAmount::Base(buy_amount2)),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([(
                    9,
                    (
                        limit_order9_changed,
                        OrderAmount::Base(balance!(38.3).into())
                    )
                )]),
                to_full_execute: BTreeMap::from([
                    (7, limit_order7.clone()),
                    (8, limit_order8.clone())
                ]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(3324.74).into())])
                    )]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(accounts::dave::<Runtime>(), buy_amount2)])
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(2368.26).into()),
                                (accounts::charlie::<Runtime>(), balance!(956.48).into())
                            ])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_market_order_impact(
                    MarketOrder::<Runtime>::new(
                        accounts::alice::<Runtime>(),
                        PriceVariant::Buy,
                        order_book_id,
                        buy_amount3,
                        None
                    ),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Quote(balance!(6758.27).into())),
                deal_output: Some(OrderAmount::Base(buy_amount3)),
                market_input: None,
                market_output: Some(OrderAmount::Base(buy_amount3)),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([(
                    12,
                    (limit_order12_changed, OrderAmount::Base(balance!(3).into()))
                )]),
                to_full_execute: BTreeMap::from([
                    (7, limit_order7.clone()),
                    (8, limit_order8.clone()),
                    (9, limit_order9.clone()),
                    (10, limit_order10.clone()),
                    (11, limit_order11.clone()),
                ]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(6758.27).into())])
                    )]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(accounts::alice::<Runtime>(), buy_amount3)])
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(5346.39).into()),
                                (accounts::charlie::<Runtime>(), balance!(1411.88).into())
                            ])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_market_order_impact(
                    MarketOrder::<Runtime>::new(
                        accounts::alice::<Runtime>(),
                        PriceVariant::Buy,
                        order_book_id,
                        buy_amount4,
                        Some(accounts::dave::<Runtime>())
                    ),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Quote(balance!(4360.52).into())),
                deal_output: Some(OrderAmount::Base(buy_amount4)),
                market_input: None,
                market_output: Some(OrderAmount::Base(buy_amount4)),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([
                    (7, limit_order7.clone()),
                    (8, limit_order8.clone()),
                    (9, limit_order9.clone()),
                    (10, limit_order10.clone()),
                ]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(4360.52).into())])
                    )]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(accounts::dave::<Runtime>(), buy_amount4)])
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(2983.14).into()),
                                (accounts::charlie::<Runtime>(), balance!(1377.38).into())
                            ])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_market_order_impact(
                    MarketOrder::<Runtime>::new(
                        accounts::alice::<Runtime>(),
                        PriceVariant::Buy,
                        order_book_id,
                        buy_amount5,
                        None
                    ),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Quote(balance!(6881.32).into())),
                deal_output: Some(OrderAmount::Base(buy_amount5)),
                market_input: None,
                market_output: Some(OrderAmount::Base(buy_amount5)),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([
                    (7, limit_order7),
                    (8, limit_order8),
                    (9, limit_order9),
                    (10, limit_order10),
                    (11, limit_order11),
                    (12, limit_order12),
                ]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(6881.32).into())])
                    )]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(accounts::alice::<Runtime>(), buy_amount5)])
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(5346.39).into()),
                                (accounts::charlie::<Runtime>(), balance!(1534.93).into())
                            ])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        let sell_amount1 = balance!(100).into();
        let sell_amount2 = balance!(270).into();
        let sell_amount3 = balance!(400).into();
        let sell_amount4 = balance!(364.8).into();
        let sell_amount5 = balance!(569.7).into();

        assert_eq!(
            order_book
                .calculate_market_order_impact(
                    MarketOrder::<Runtime>::new(
                        accounts::alice::<Runtime>(),
                        PriceVariant::Sell,
                        order_book_id,
                        sell_amount1,
                        None
                    ),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Base(sell_amount1)),
                deal_output: Some(OrderAmount::Quote(balance!(1000).into())),
                market_input: None,
                market_output: Some(OrderAmount::Quote(balance!(1000).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([(
                    1,
                    (limit_order1_changed, OrderAmount::Base(buy_amount1))
                )]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(accounts::alice::<Runtime>(), sell_amount1)])
                    )]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.quote,
                            BTreeMap::from([(accounts::alice::<Runtime>(), balance!(1000).into())])
                        ),
                        (
                            order_book_id.base,
                            BTreeMap::from([(accounts::bob::<Runtime>(), sell_amount1)])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_market_order_impact(
                    MarketOrder::<Runtime>::new(
                        accounts::alice::<Runtime>(),
                        PriceVariant::Sell,
                        order_book_id,
                        sell_amount2,
                        Some(accounts::dave::<Runtime>())
                    ),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Base(sell_amount2)),
                deal_output: Some(OrderAmount::Quote(balance!(2679.7).into())),
                market_input: None,
                market_output: Some(OrderAmount::Quote(balance!(2679.7).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([(
                    3,
                    (
                        limit_order3_changed,
                        OrderAmount::Base(balance!(6.3).into())
                    )
                )]),
                to_full_execute: BTreeMap::from([
                    (1, limit_order1.clone()),
                    (2, limit_order2.clone()),
                ]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(accounts::alice::<Runtime>(), sell_amount2)])
                    )]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.quote,
                            BTreeMap::from([(
                                accounts::dave::<Runtime>(),
                                balance!(2679.7).into()
                            )])
                        ),
                        (
                            order_book_id.base,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(174.8).into()),
                                (accounts::charlie::<Runtime>(), balance!(95.2).into())
                            ])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_market_order_impact(
                    MarketOrder::<Runtime>::new(
                        accounts::alice::<Runtime>(),
                        PriceVariant::Sell,
                        order_book_id,
                        sell_amount3,
                        None
                    ),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Base(sell_amount3)),
                deal_output: Some(OrderAmount::Quote(balance!(3926.22).into())),
                market_input: None,
                market_output: Some(OrderAmount::Quote(balance!(3926.22).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([(
                    5,
                    (
                        limit_order5_changed,
                        OrderAmount::Base(balance!(35.2).into())
                    )
                )]),
                to_full_execute: BTreeMap::from([
                    (1, limit_order1.clone()),
                    (2, limit_order2.clone()),
                    (3, limit_order3.clone()),
                    (4, limit_order4.clone()),
                ]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(accounts::alice::<Runtime>(), sell_amount3)])
                    )]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.quote,
                            BTreeMap::from([(
                                accounts::alice::<Runtime>(),
                                balance!(3926.22).into()
                            )])
                        ),
                        (
                            order_book_id.base,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(248.4).into()),
                                (accounts::charlie::<Runtime>(), balance!(151.6).into())
                            ])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_market_order_impact(
                    MarketOrder::<Runtime>::new(
                        accounts::alice::<Runtime>(),
                        PriceVariant::Sell,
                        order_book_id,
                        sell_amount4,
                        Some(accounts::dave::<Runtime>())
                    ),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Base(sell_amount4)),
                deal_output: Some(OrderAmount::Quote(balance!(3591.82).into())),
                market_input: None,
                market_output: Some(OrderAmount::Quote(balance!(3591.82).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([
                    (1, limit_order1.clone()),
                    (2, limit_order2.clone()),
                    (3, limit_order3.clone()),
                    (4, limit_order4.clone()),
                ]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(accounts::alice::<Runtime>(), sell_amount4)])
                    )]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.quote,
                            BTreeMap::from([(
                                accounts::dave::<Runtime>(),
                                balance!(3591.82).into()
                            )])
                        ),
                        (
                            order_book_id.base,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(213.2).into()),
                                (accounts::charlie::<Runtime>(), balance!(151.6).into())
                            ])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_market_order_impact(
                    MarketOrder::<Runtime>::new(
                        accounts::alice::<Runtime>(),
                        PriceVariant::Sell,
                        order_book_id,
                        sell_amount5,
                        None
                    ),
                    &mut data
                )
                .unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Base(sell_amount5)),
                deal_output: Some(OrderAmount::Quote(balance!(5538.37).into())),
                market_input: None,
                market_output: Some(OrderAmount::Quote(balance!(5538.37).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([
                    (1, limit_order1),
                    (2, limit_order2),
                    (3, limit_order3),
                    (4, limit_order4),
                    (5, limit_order5),
                    (6, limit_order6),
                ]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(accounts::alice::<Runtime>(), sell_amount5)])
                    )]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.quote,
                            BTreeMap::from([(
                                accounts::alice::<Runtime>(),
                                balance!(5538.37).into()
                            )])
                        ),
                        (
                            order_book_id.base,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(303.1).into()),
                                (accounts::charlie::<Runtime>(), balance!(266.6).into())
                            ])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );
    });
}

#[test]
fn should_calculate_limit_order_impact() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_empty_order_book::<Runtime>(order_book_id);

        let limit_order_buy = LimitOrder::<Runtime>::new(
            1,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(10).into(),
            balance!(100).into(),
            100,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );
        let limit_order_sell = LimitOrder::<Runtime>::new(
            2,
            accounts::bob::<Runtime>(),
            PriceVariant::Sell,
            balance!(11).into(),
            balance!(150).into(),
            100,
            1000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book
                .calculate_limit_order_impact(limit_order_buy.clone())
                .unwrap(),
            MarketChange {
                deal_input: None,
                deal_output: None,
                market_input: Some(OrderAmount::Quote(balance!(1000).into())),
                market_output: None,
                to_place: BTreeMap::from([(1, limit_order_buy)]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(1000).into())])
                    )]),
                    to_unlock: BTreeMap::new(),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_limit_order_impact(limit_order_sell.clone())
                .unwrap(),
            MarketChange {
                deal_input: None,
                deal_output: None,
                market_input: Some(OrderAmount::Base(balance!(150).into())),
                market_output: None,
                to_place: BTreeMap::from([(2, limit_order_sell)]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(accounts::bob::<Runtime>(), balance!(150).into())])
                    )]),
                    to_unlock: BTreeMap::new(),
                },
                ignore_unschedule_error: false
            }
        );
    });
}

#[test]
fn should_calculate_cancellation_limit_order_impact() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let limit_order2 = data.get_limit_order(&order_book_id, 2).unwrap();
        let limit_order8 = data.get_limit_order(&order_book_id, 8).unwrap();

        assert_eq!(
            order_book
                .calculate_cancellation_limit_order_impact(
                    limit_order2.clone(),
                    CancelReason::Manual,
                    false
                )
                .unwrap(),
            MarketChange {
                deal_input: None,
                deal_output: None,
                market_input: None,
                market_output: Some(limit_order2.deal_amount(MarketRole::Taker, None).unwrap()),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([(2, (limit_order2.clone(), CancelReason::Manual))]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::new(),
                    to_unlock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(
                            limit_order2.owner.clone(),
                            *limit_order2
                                .deal_amount(MarketRole::Taker, None)
                                .unwrap()
                                .value()
                        )])
                    )]),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_cancellation_limit_order_impact(
                    limit_order2.clone(),
                    CancelReason::Expired,
                    true
                )
                .unwrap(),
            MarketChange {
                deal_input: None,
                deal_output: None,
                market_input: None,
                market_output: Some(limit_order2.deal_amount(MarketRole::Taker, None).unwrap()),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([(2, (limit_order2.clone(), CancelReason::Expired))]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::new(),
                    to_unlock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(
                            limit_order2.owner.clone(),
                            *limit_order2
                                .deal_amount(MarketRole::Taker, None)
                                .unwrap()
                                .value()
                        )])
                    )]),
                },
                ignore_unschedule_error: true
            }
        );

        assert_eq!(
            order_book
                .calculate_cancellation_limit_order_impact(
                    limit_order8.clone(),
                    CancelReason::Aligned,
                    false
                )
                .unwrap(),
            MarketChange {
                deal_input: None,
                deal_output: None,
                market_input: None,
                market_output: Some(limit_order8.deal_amount(MarketRole::Taker, None).unwrap()),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([(8, (limit_order8.clone(), CancelReason::Aligned))]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::new(),
                    to_unlock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(
                            limit_order8.owner.clone(),
                            *limit_order8
                                .deal_amount(MarketRole::Taker, None)
                                .unwrap()
                                .value()
                        )])
                    )]),
                },
                ignore_unschedule_error: false
            }
        );

        assert_eq!(
            order_book
                .calculate_cancellation_limit_order_impact(
                    limit_order8.clone(),
                    CancelReason::Expired,
                    true
                )
                .unwrap(),
            MarketChange {
                deal_input: None,
                deal_output: None,
                market_input: None,
                market_output: Some(limit_order8.deal_amount(MarketRole::Taker, None).unwrap()),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([(8, (limit_order8.clone(), CancelReason::Expired))]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::new(),
                    to_unlock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(
                            limit_order8.owner.clone(),
                            *limit_order8
                                .deal_amount(MarketRole::Taker, None)
                                .unwrap()
                                .value()
                        )])
                    )]),
                },
                ignore_unschedule_error: true
            }
        );
    });
}

#[test]
fn should_calculate_cancellation_of_all_limit_orders_impact() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let limit_order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        let limit_order2 = data.get_limit_order(&order_book_id, 2).unwrap();
        let limit_order3 = data.get_limit_order(&order_book_id, 3).unwrap();
        let limit_order4 = data.get_limit_order(&order_book_id, 4).unwrap();
        let limit_order5 = data.get_limit_order(&order_book_id, 5).unwrap();
        let limit_order6 = data.get_limit_order(&order_book_id, 6).unwrap();
        let limit_order7 = data.get_limit_order(&order_book_id, 7).unwrap();
        let limit_order8 = data.get_limit_order(&order_book_id, 8).unwrap();
        let limit_order9 = data.get_limit_order(&order_book_id, 9).unwrap();
        let limit_order10 = data.get_limit_order(&order_book_id, 10).unwrap();
        let limit_order11 = data.get_limit_order(&order_book_id, 11).unwrap();
        let limit_order12 = data.get_limit_order(&order_book_id, 12).unwrap();

        assert_eq!(
            order_book
                .calculate_cancellation_of_all_limit_orders_impact(CancelReason::Manual, &mut data)
                .unwrap(),
            MarketChange {
                deal_input: None,
                deal_output: None,
                market_input: None,
                market_output: None,
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([
                    (1, (limit_order1, CancelReason::Manual)),
                    (2, (limit_order2, CancelReason::Manual)),
                    (3, (limit_order3, CancelReason::Manual)),
                    (4, (limit_order4, CancelReason::Manual)),
                    (5, (limit_order5, CancelReason::Manual)),
                    (6, (limit_order6, CancelReason::Manual)),
                    (7, (limit_order7, CancelReason::Manual)),
                    (8, (limit_order8, CancelReason::Manual)),
                    (9, (limit_order9, CancelReason::Manual)),
                    (10, (limit_order10, CancelReason::Manual)),
                    (11, (limit_order11, CancelReason::Manual)),
                    (12, (limit_order12, CancelReason::Manual)),
                ]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::new(),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(475).into()),
                                (accounts::charlie::<Runtime>(), balance!(135.7).into())
                            ])
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(2977.11).into()),
                                (accounts::charlie::<Runtime>(), balance!(2561.26).into())
                            ])
                        )
                    ]),
                },
                ignore_unschedule_error: false
            }
        );
    });
}

#[test]
fn should_calculate_align_limit_orders_impact() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let mut order_book = create_and_fill_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);

        assert_ok!(order_book.place_limit_order(
            LimitOrder::<Runtime>::new(
                13,
                accounts::alice::<Runtime>(),
                PriceVariant::Buy,
                balance!(10.1).into(),
                balance!(1.1).into(),
                10,
                <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
                frame_system::Pallet::<Runtime>::block_number(),
            ),
            &mut data
        ));

        // the remaining balance of limit order 13 becomes 0.1 after that
        assert_ok!(order_book.execute_market_order(
            MarketOrder::<Runtime>::new(
                accounts::alice::<Runtime>(),
                PriceVariant::Sell,
                order_book_id,
                balance!(1).into(),
                None
            ),
            &mut data
        ));

        assert_ok!(order_book.place_limit_order(
            LimitOrder::<Runtime>::new(
                14,
                accounts::alice::<Runtime>(),
                PriceVariant::Buy,
                balance!(10.1).into(),
                balance!(1).into(),
                10,
                <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
                frame_system::Pallet::<Runtime>::block_number(),
            ),
            &mut data
        ));

        assert_ok!(order_book.place_limit_order(
            LimitOrder::<Runtime>::new(
                15,
                accounts::alice::<Runtime>(),
                PriceVariant::Sell,
                balance!(10.9).into(),
                balance!(1.1).into(),
                10,
                <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
                frame_system::Pallet::<Runtime>::block_number(),
            ),
            &mut data
        ));

        // the remaining balance of limit order 15 becomes 0.1 after that
        assert_ok!(order_book.execute_market_order(
            MarketOrder::<Runtime>::new(
                accounts::alice::<Runtime>(),
                PriceVariant::Buy,
                order_book_id,
                balance!(1).into(),
                None
            ),
            &mut data
        ));

        assert_ok!(order_book.place_limit_order(
            LimitOrder::<Runtime>::new(
                16,
                accounts::alice::<Runtime>(),
                PriceVariant::Sell,
                balance!(10.9).into(),
                balance!(1).into(),
                10,
                <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
                frame_system::Pallet::<Runtime>::block_number(),
            ),
            &mut data
        ));

        let mut limit_order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        let mut limit_order2 = data.get_limit_order(&order_book_id, 2).unwrap();
        let mut limit_order3 = data.get_limit_order(&order_book_id, 3).unwrap();
        let mut limit_order4 = data.get_limit_order(&order_book_id, 4).unwrap();
        let mut limit_order5 = data.get_limit_order(&order_book_id, 5).unwrap();
        let mut limit_order7 = data.get_limit_order(&order_book_id, 7).unwrap();
        let mut limit_order8 = data.get_limit_order(&order_book_id, 8).unwrap();
        let mut limit_order9 = data.get_limit_order(&order_book_id, 9).unwrap();
        let mut limit_order10 = data.get_limit_order(&order_book_id, 10).unwrap();
        let mut limit_order11 = data.get_limit_order(&order_book_id, 11).unwrap();
        let mut limit_order12 = data.get_limit_order(&order_book_id, 12).unwrap();
        let limit_order13 = data.get_limit_order(&order_book_id, 13).unwrap();
        let limit_order15 = data.get_limit_order(&order_book_id, 15).unwrap();

        let limit_orders = OrderBookPallet::get_limit_orders(&order_book_id, None, 100);

        // empty market change if all limit orders have suitable amount
        assert_eq!(
            order_book
                .calculate_align_limit_orders_impact(limit_orders.clone())
                .unwrap(),
            MarketChange {
                deal_input: None,
                deal_output: None,
                market_input: None,
                market_output: None,
                to_place: BTreeMap::new(),
                to_part_execute: BTreeMap::new(),
                to_full_execute: BTreeMap::new(),
                to_cancel: BTreeMap::new(),
                to_force_update: BTreeMap::new(),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::new(),
                    to_unlock: BTreeMap::new(),
                },
                ignore_unschedule_error: false,
            }
        );

        // change lot size precision
        order_book.step_lot_size = balance!(1).into();

        limit_order1.amount = balance!(168).into();
        limit_order2.amount = balance!(95).into();
        limit_order3.amount = balance!(44).into();
        limit_order4.amount = balance!(56).into();
        limit_order5.amount = balance!(89).into();

        limit_order7.amount = balance!(176).into();
        limit_order8.amount = balance!(85).into();
        limit_order9.amount = balance!(93).into();
        limit_order10.amount = balance!(36).into();
        limit_order11.amount = balance!(205).into();
        limit_order12.amount = balance!(13).into();

        // limit orders 6, 14 & 16 are not presented because they already have suitable amount for new step_lot_size
        assert_eq!(
            order_book
                .calculate_align_limit_orders_impact(limit_orders)
                .unwrap(),
            MarketChange {
                deal_input: None,
                deal_output: None,
                market_input: None,
                market_output: None,
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([
                    (13, (limit_order13, CancelReason::Aligned)),
                    (15, (limit_order15, CancelReason::Aligned))
                ]),
                to_force_update: BTreeMap::from([
                    (1, limit_order1),
                    (2, limit_order2),
                    (3, limit_order3),
                    (4, limit_order4),
                    (5, limit_order5),
                    (7, limit_order7),
                    (8, limit_order8),
                    (9, limit_order9),
                    (10, limit_order10),
                    (11, limit_order11),
                    (12, limit_order12),
                ]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([
                                (accounts::alice::<Runtime>(), balance!(0.1).into()),
                                (accounts::bob::<Runtime>(), balance!(1).into()),
                                (accounts::charlie::<Runtime>(), balance!(1.7).into())
                            ])
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([
                                (accounts::alice::<Runtime>(), balance!(1.01).into()),
                                (accounts::bob::<Runtime>(), balance!(20.41).into()),
                                (accounts::charlie::<Runtime>(), balance!(5.76).into())
                            ])
                        )
                    ]),
                },
                ignore_unschedule_error: false,
            }
        );
    });
}

#[test]
fn should_apply_market_change() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);
        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);
        fill_balance::<Runtime>(accounts::dave::<Runtime>(), order_book_id);

        let bid_price1 = balance!(10).into();
        let bid_price2 = balance!(9.8).into();
        let bid_price3 = balance!(9.5).into();

        let ask_price1 = balance!(11).into();
        let ask_price2 = balance!(11.2).into();
        let ask_price3 = balance!(11.5).into();

        let mut limit_order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        let limit_order2 = data.get_limit_order(&order_book_id, 2).unwrap();
        let mut limit_order7 = data.get_limit_order(&order_book_id, 7).unwrap();
        let limit_order8 = data.get_limit_order(&order_book_id, 8).unwrap();

        limit_order1.amount = balance!(100).into();
        limit_order7.amount = balance!(100).into();

        let new_order_id1 = 101;
        let new_order_id2 = 102;

        let new_limit_order1 = LimitOrder::<Runtime>::new(
            new_order_id1,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            bid_price1,
            balance!(300).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let new_limit_order2 = LimitOrder::<Runtime>::new(
            new_order_id2,
            accounts::dave::<Runtime>(),
            PriceVariant::Sell,
            ask_price1,
            balance!(300).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let balance_diff = balance!(50);

        let alice_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>());
        let alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>());
        let bob_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>());
        let bob_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>());
        let charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>());
        let charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>());
        let dave_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>());
        let dave_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>());

        // check state before

        assert_eq!(data.get_bids(&order_book_id, &bid_price1).unwrap(), vec![1]);
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price2).unwrap(),
            vec![2, 3]
        );
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(data.get_asks(&order_book_id, &ask_price1).unwrap(), vec![7]);
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price2).unwrap(),
            vec![8, 9]
        );
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(168.5).into()),
                (bid_price2, balance!(139.9).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price1, balance!(176.3).into()),
                (ask_price2, balance!(178.6).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        let market_change = MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None,
            to_place: BTreeMap::from([
                (new_order_id1, new_limit_order1.clone()),
                (new_order_id2, new_limit_order2.clone()),
            ]),
            to_part_execute: BTreeMap::from([
                (1, (limit_order1, OrderAmount::Base(balance!(68.5).into()))),
                (7, (limit_order7, OrderAmount::Base(balance!(76.3).into()))),
            ]),
            to_full_execute: BTreeMap::from([(8, limit_order8)]),
            to_cancel: BTreeMap::from([(2, (limit_order2, CancelReason::Manual))]),
            to_force_update: BTreeMap::from([]),
            payment: Payment {
                order_book_id,
                to_lock: BTreeMap::from([
                    (
                        order_book_id.base,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance_diff.into())]),
                    ),
                    (
                        order_book_id.quote,
                        BTreeMap::from([(accounts::bob::<Runtime>(), balance_diff.into())]),
                    ),
                ]),
                to_unlock: BTreeMap::from([
                    (
                        order_book_id.base,
                        BTreeMap::from([(accounts::charlie::<Runtime>(), balance_diff.into())]),
                    ),
                    (
                        order_book_id.quote,
                        BTreeMap::from([(accounts::dave::<Runtime>(), balance_diff.into())]),
                    ),
                ]),
            },
            ignore_unschedule_error: false,
        };

        // apply market change
        assert_ok!(order_book.apply_market_change(market_change, &mut data));

        // check state after

        assert_eq!(
            data.get_bids(&order_book_id, &bid_price1).unwrap(),
            vec![1, new_order_id1]
        );
        assert_eq!(data.get_bids(&order_book_id, &bid_price2).unwrap(), vec![3]);
        assert_eq!(
            data.get_bids(&order_book_id, &bid_price3).unwrap(),
            vec![4, 5, 6]
        );

        assert_eq!(
            data.get_asks(&order_book_id, &ask_price1).unwrap(),
            vec![7, new_order_id2]
        );
        assert_eq!(data.get_asks(&order_book_id, &ask_price2).unwrap(), vec![9]);
        assert_eq!(
            data.get_asks(&order_book_id, &ask_price3).unwrap(),
            vec![10, 11, 12]
        );

        assert_eq!(
            data.get_aggregated_bids(&order_book_id),
            BTreeMap::from([
                (bid_price1, balance!(400).into()),
                (bid_price2, balance!(44.7).into()),
                (bid_price3, balance!(261.3).into())
            ])
        );
        assert_eq!(
            data.get_aggregated_asks(&order_book_id),
            BTreeMap::from([
                (ask_price1, balance!(400).into()),
                (ask_price2, balance!(93.2).into()),
                (ask_price3, balance!(255.8).into())
            ])
        );

        assert_eq!(
            alice_base_balance - balance_diff,
            free_balance::<Runtime>(&order_book_id.base, &accounts::alice::<Runtime>())
        );
        assert_eq!(
            alice_quote_balance,
            free_balance::<Runtime>(&order_book_id.quote, &accounts::alice::<Runtime>())
        );
        assert_eq!(
            bob_base_balance,
            free_balance::<Runtime>(&order_book_id.base, &accounts::bob::<Runtime>())
        );
        assert_eq!(
            bob_quote_balance - balance_diff,
            free_balance::<Runtime>(&order_book_id.quote, &accounts::bob::<Runtime>())
        );
        assert_eq!(
            charlie_base_balance + balance_diff,
            free_balance::<Runtime>(&order_book_id.base, &accounts::charlie::<Runtime>())
        );
        assert_eq!(
            charlie_quote_balance,
            free_balance::<Runtime>(&order_book_id.quote, &accounts::charlie::<Runtime>())
        );
        assert_eq!(
            dave_base_balance,
            free_balance::<Runtime>(&order_book_id.base, &accounts::dave::<Runtime>())
        );
        assert_eq!(
            dave_quote_balance + balance_diff,
            free_balance::<Runtime>(&order_book_id.quote, &accounts::dave::<Runtime>())
        );

        assert_eq!(
            data.get_limit_order(&order_book_id, new_order_id1).unwrap(),
            new_limit_order1
        );
        assert_eq!(
            data.get_limit_order(&order_book_id, new_order_id2).unwrap(),
            new_limit_order2
        );
    });
}

#[test]
fn should_calculate_market_depth_volume_to_price() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        create_and_fill_order_book::<Runtime>(order_book_id);

        let bids = data.get_aggregated_bids(&order_book_id);
        let asks = data.get_aggregated_asks(&order_book_id);

        let bid_price1 = balance!(10.5).into();
        let bid_price2 = balance!(9.9).into();
        let bid_price3 = balance!(9.7).into();
        let bid_price4 = balance!(9).into();

        let ask_price1 = balance!(10.5).into();
        let ask_price2 = balance!(11.1).into();
        let ask_price3 = balance!(11.3).into();
        let ask_price4 = balance!(12).into();

        let regular_amount = balance!(200).into();
        let big_amount = balance!(1000).into();

        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Buy,
                bid_price1,
                regular_amount,
                bids.iter().rev()
            ),
            (OrderVolume::zero(), regular_amount)
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Buy,
                bid_price2,
                regular_amount,
                bids.iter().rev()
            ),
            (balance!(168.5).into(), balance!(31.5).into())
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Buy,
                bid_price3,
                regular_amount,
                bids.iter().rev()
            ),
            (regular_amount, OrderVolume::zero())
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Buy,
                bid_price4,
                regular_amount,
                bids.iter().rev()
            ),
            (regular_amount, OrderVolume::zero())
        );

        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Buy,
                bid_price1,
                big_amount,
                bids.iter().rev()
            ),
            (OrderVolume::zero(), big_amount)
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Buy,
                bid_price2,
                big_amount,
                bids.iter().rev()
            ),
            (balance!(168.5).into(), balance!(831.5).into())
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Buy,
                bid_price3,
                big_amount,
                bids.iter().rev()
            ),
            (balance!(308.4).into(), balance!(691.6).into())
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Buy,
                bid_price4,
                big_amount,
                bids.iter().rev()
            ),
            (balance!(569.7).into(), balance!(430.3).into())
        );

        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Sell,
                ask_price1,
                regular_amount,
                asks.iter()
            ),
            (OrderVolume::zero(), regular_amount)
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Sell,
                ask_price2,
                regular_amount,
                asks.iter()
            ),
            (balance!(176.3).into(), balance!(23.7).into())
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Sell,
                ask_price3,
                regular_amount,
                asks.iter()
            ),
            (regular_amount, OrderVolume::zero())
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Sell,
                ask_price4,
                regular_amount,
                asks.iter()
            ),
            (regular_amount, OrderVolume::zero())
        );

        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Sell,
                ask_price1,
                big_amount,
                asks.iter()
            ),
            (OrderVolume::zero(), big_amount)
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Sell,
                ask_price2,
                big_amount,
                asks.iter()
            ),
            (balance!(176.3).into(), balance!(823.7).into())
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Sell,
                ask_price3,
                big_amount,
                asks.iter()
            ),
            (balance!(354.9).into(), balance!(645.1).into())
        );
        assert_eq!(
            OrderBook::<Runtime>::calculate_market_depth_volume_to_price(
                PriceVariant::Sell,
                ask_price4,
                big_amount,
                asks.iter()
            ),
            (balance!(610.7).into(), balance!(389.3).into())
        );
    });
}

#[test]
fn should_cross_spread() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let new_bid_price = balance!(11.1).into();
        let new_ask_price = balance!(9.9).into();

        let limit_order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        let limit_order7 = data.get_limit_order(&order_book_id, 7).unwrap();

        let mut limit_order1_changed = limit_order1.clone();
        limit_order1_changed.amount = balance!(150).into();
        let mut limit_order7_changed = limit_order7.clone();
        limit_order7_changed.amount = balance!(150).into();

        // buy order 1
        let buy_order_id1 = 101;
        let buy_order1 = LimitOrder::<Runtime>::new(
            buy_order_id1,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            new_bid_price,
            balance!(26.3).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book.cross_spread(buy_order1, &mut data).unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Quote(balance!(289.3).into())),
                deal_output: Some(OrderAmount::Base(balance!(26.3).into())),
                market_input: None,
                market_output: Some(OrderAmount::Base(balance!(26.3).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([(
                    7,
                    (
                        limit_order7_changed,
                        OrderAmount::Base(balance!(26.3).into())
                    )
                )]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(289.3).into())])
                    ),]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(accounts::alice::<Runtime>(), balance!(26.3).into())]),
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([(accounts::bob::<Runtime>(), balance!(289.3).into())]),
                        ),
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        // buy order 2
        let buy_order_id2 = 102;
        let buy_order2 = LimitOrder::<Runtime>::new(
            buy_order_id2,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            new_bid_price,
            balance!(300).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let mut expected_buy_order2 = buy_order2.clone();
        expected_buy_order2.amount = balance!(123.7).into();

        assert_eq!(
            order_book.cross_spread(buy_order2, &mut data).unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Quote(balance!(1939.3).into())),
                deal_output: Some(OrderAmount::Base(balance!(176.3).into())),
                market_input: Some(OrderAmount::Quote(balance!(1373.07).into())),
                market_output: Some(OrderAmount::Base(balance!(176.3).into())),
                to_place: BTreeMap::from([(buy_order_id2, expected_buy_order2)]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([(7, limit_order7)]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(3312.37).into())])
                    ),]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(
                                accounts::alice::<Runtime>(),
                                balance!(176.3).into()
                            )]),
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([(accounts::bob::<Runtime>(), balance!(1939.3).into())]),
                        ),
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        // sell order 1
        let sell_order_id1 = 201;
        let sell_order1 = LimitOrder::<Runtime>::new(
            sell_order_id1,
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            new_ask_price,
            balance!(18.5).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book.cross_spread(sell_order1, &mut data).unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Base(balance!(18.5).into())),
                deal_output: Some(OrderAmount::Quote(balance!(185).into())),
                market_input: None,
                market_output: Some(OrderAmount::Quote(balance!(185).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([(
                    1,
                    (
                        limit_order1_changed,
                        OrderAmount::Base(balance!(18.5).into())
                    )
                )]),
                to_full_execute: BTreeMap::from([]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(18.5).into())])
                    ),]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(accounts::bob::<Runtime>(), balance!(18.5).into())]),
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([(accounts::alice::<Runtime>(), balance!(185).into())]),
                        ),
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        // sell order 2
        let sell_order_id2 = 202;
        let sell_order2 = LimitOrder::<Runtime>::new(
            sell_order_id2,
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            new_ask_price,
            balance!(300).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        let mut expected_sell_order2 = sell_order2.clone();
        expected_sell_order2.amount = balance!(131.5).into();

        assert_eq!(
            order_book.cross_spread(sell_order2, &mut data).unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Base(balance!(168.5).into())),
                deal_output: Some(OrderAmount::Quote(balance!(1685).into())),
                market_input: Some(OrderAmount::Base(balance!(131.5).into())),
                market_output: Some(OrderAmount::Quote(balance!(1685).into())),
                to_place: BTreeMap::from([(sell_order_id2, expected_sell_order2)]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([(1, limit_order1)]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(300).into())])
                    ),]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(accounts::bob::<Runtime>(), balance!(168.5).into())]),
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([(accounts::alice::<Runtime>(), balance!(1685).into())]),
                        ),
                    ]),
                },
                ignore_unschedule_error: false
            }
        );
    });
}

#[test]
fn should_cross_spread_with_small_remaining_amount() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book::<Runtime>(order_book_id);

        let limit_order1 = data.get_limit_order(&order_book_id, 1).unwrap();
        let limit_order2 = data.get_limit_order(&order_book_id, 2).unwrap();
        let limit_order3 = data.get_limit_order(&order_book_id, 3).unwrap();
        let limit_order4 = data.get_limit_order(&order_book_id, 4).unwrap();
        let limit_order5 = data.get_limit_order(&order_book_id, 5).unwrap();
        let limit_order6 = data.get_limit_order(&order_book_id, 6).unwrap();
        let limit_order7 = data.get_limit_order(&order_book_id, 7).unwrap();
        let limit_order8 = data.get_limit_order(&order_book_id, 8).unwrap();
        let limit_order9 = data.get_limit_order(&order_book_id, 9).unwrap();
        let limit_order10 = data.get_limit_order(&order_book_id, 10).unwrap();
        let limit_order11 = data.get_limit_order(&order_book_id, 11).unwrap();
        let limit_order12 = data.get_limit_order(&order_book_id, 12).unwrap();

        let mut limit_order2_changed = limit_order2.clone();
        limit_order2_changed.amount = balance!(94.7).into();

        let mut limit_order8_changed = limit_order8.clone();
        limit_order8_changed.amount = balance!(84.7).into();

        // buy order 1
        // small remaining amount executes in market
        let buy_order_id1 = 101;
        let buy_order1 = LimitOrder::<Runtime>::new(
            buy_order_id1,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(11.1).into(),
            balance!(177).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book.cross_spread(buy_order1, &mut data).unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Quote(balance!(1947.14).into())),
                deal_output: Some(OrderAmount::Base(balance!(177).into())),
                market_input: None,
                market_output: Some(OrderAmount::Base(balance!(177).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([(
                    8,
                    (
                        limit_order8_changed,
                        OrderAmount::Base(balance!(0.7).into())
                    )
                )]),
                to_full_execute: BTreeMap::from([(7, limit_order7.clone())]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(1947.14).into())])
                    ),]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(accounts::alice::<Runtime>(), balance!(177).into())]),
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(1939.3).into()),
                                (accounts::charlie::<Runtime>(), balance!(7.84).into())
                            ]),
                        ),
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        // buy order 2
        // small remaining amount cancelled
        let buy_order_id2 = 102;
        let buy_order2 = LimitOrder::<Runtime>::new(
            buy_order_id2,
            accounts::alice::<Runtime>(),
            PriceVariant::Buy,
            balance!(11.6).into(),
            balance!(611).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book.cross_spread(buy_order2, &mut data).unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Quote(balance!(6881.32).into())),
                deal_output: Some(OrderAmount::Base(balance!(610.7).into())),
                market_input: None,
                market_output: Some(OrderAmount::Base(balance!(610.7).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([
                    (7, limit_order7),
                    (8, limit_order8),
                    (9, limit_order9),
                    (10, limit_order10),
                    (11, limit_order11),
                    (12, limit_order12),
                ]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.quote,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(6881.32).into())])
                    ),]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([(
                                accounts::alice::<Runtime>(),
                                balance!(610.7).into()
                            )]),
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(5346.39).into()),
                                (accounts::charlie::<Runtime>(), balance!(1534.93).into())
                            ]),
                        ),
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        // sell order 1
        // small remaining amount executes in market
        let sell_order_id1 = 201;
        let sell_order1 = LimitOrder::<Runtime>::new(
            sell_order_id1,
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            balance!(9.9).into(),
            balance!(169).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book.cross_spread(sell_order1, &mut data).unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Base(balance!(169).into())),
                deal_output: Some(OrderAmount::Quote(balance!(1689.9).into())),
                market_input: None,
                market_output: Some(OrderAmount::Quote(balance!(1689.9).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([(
                    2,
                    (
                        limit_order2_changed,
                        OrderAmount::Base(balance!(0.5).into())
                    )
                )]),
                to_full_execute: BTreeMap::from([(1, limit_order1.clone())]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(169).into())])
                    ),]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(168.5).into()),
                                (accounts::charlie::<Runtime>(), balance!(0.5).into())
                            ]),
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([(
                                accounts::alice::<Runtime>(),
                                balance!(1689.9).into()
                            )]),
                        ),
                    ]),
                },
                ignore_unschedule_error: false
            }
        );

        // sell order 2
        // small remaining amount cancelled
        let sell_order_id2 = 202;
        let sell_order2 = LimitOrder::<Runtime>::new(
            sell_order_id2,
            accounts::alice::<Runtime>(),
            PriceVariant::Sell,
            balance!(9.4).into(),
            balance!(570).into(),
            10,
            <Runtime as Config>::MIN_ORDER_LIFESPAN + 100000,
            frame_system::Pallet::<Runtime>::block_number(),
        );

        assert_eq!(
            order_book.cross_spread(sell_order2, &mut data).unwrap(),
            MarketChange {
                deal_input: Some(OrderAmount::Base(balance!(569.7).into())),
                deal_output: Some(OrderAmount::Quote(balance!(5538.37).into())),
                market_input: None,
                market_output: Some(OrderAmount::Quote(balance!(5538.37).into())),
                to_place: BTreeMap::from([]),
                to_part_execute: BTreeMap::from([]),
                to_full_execute: BTreeMap::from([
                    (1, limit_order1),
                    (2, limit_order2),
                    (3, limit_order3),
                    (4, limit_order4),
                    (5, limit_order5),
                    (6, limit_order6),
                ]),
                to_cancel: BTreeMap::from([]),
                to_force_update: BTreeMap::from([]),
                payment: Payment {
                    order_book_id,
                    to_lock: BTreeMap::from([(
                        order_book_id.base,
                        BTreeMap::from([(accounts::alice::<Runtime>(), balance!(569.7).into())])
                    ),]),
                    to_unlock: BTreeMap::from([
                        (
                            order_book_id.base,
                            BTreeMap::from([
                                (accounts::bob::<Runtime>(), balance!(303.1).into()),
                                (accounts::charlie::<Runtime>(), balance!(266.6).into())
                            ]),
                        ),
                        (
                            order_book_id.quote,
                            BTreeMap::from([(
                                accounts::alice::<Runtime>(),
                                balance!(5538.37).into()
                            )]),
                        ),
                    ]),
                },
                ignore_unschedule_error: false
            }
        );
    });
}

#[test]
fn should_return_empty_market_depth() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_empty_order_book(order_book_id);

        assert_eq!(
            order_book.market_depth(PriceVariant::Buy, None, &mut data),
            Vec::new()
        );

        assert_eq!(
            order_book.market_depth(PriceVariant::Sell, None, &mut data),
            Vec::new()
        );
    });
}

#[test]
fn should_return_market_depth() {
    ext().execute_with(|| {
        let mut data = StorageDataLayer::<Runtime>::new();

        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        let order_book = create_and_fill_order_book(order_book_id);

        // bids without limit
        assert_eq!(
            order_book.market_depth(PriceVariant::Buy, None, &mut data),
            Vec::from([
                (balance!(10).into(), balance!(168.5).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(9.5).into(), balance!(261.3).into())
            ])
        );

        // bids, limit is a base asset

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Buy,
                Some(OrderAmount::Base(balance!(0).into())),
                &mut data
            ),
            Vec::new()
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Buy,
                Some(OrderAmount::Base(balance!(100).into())),
                &mut data
            ),
            Vec::from([(balance!(10).into(), balance!(168.5).into())])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Buy,
                Some(OrderAmount::Base(balance!(200).into())),
                &mut data
            ),
            Vec::from([
                (balance!(10).into(), balance!(168.5).into()),
                (balance!(9.8).into(), balance!(139.9).into())
            ])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Buy,
                Some(OrderAmount::Base(balance!(400).into())),
                &mut data
            ),
            Vec::from([
                (balance!(10).into(), balance!(168.5).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(9.5).into(), balance!(261.3).into())
            ])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Buy,
                Some(OrderAmount::Base(balance!(1000).into())),
                &mut data
            ),
            Vec::from([
                (balance!(10).into(), balance!(168.5).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(9.5).into(), balance!(261.3).into())
            ])
        );

        // bids, limit is a quote asset

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Buy,
                Some(OrderAmount::Quote(balance!(0).into())),
                &mut data
            ),
            Vec::new()
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Buy,
                Some(OrderAmount::Quote(balance!(1000).into())),
                &mut data
            ),
            Vec::from([(balance!(10).into(), balance!(168.5).into())])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Buy,
                Some(OrderAmount::Quote(balance!(2000).into())),
                &mut data
            ),
            Vec::from([
                (balance!(10).into(), balance!(168.5).into()),
                (balance!(9.8).into(), balance!(139.9).into())
            ])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Buy,
                Some(OrderAmount::Quote(balance!(4000).into())),
                &mut data
            ),
            Vec::from([
                (balance!(10).into(), balance!(168.5).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(9.5).into(), balance!(261.3).into())
            ])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Buy,
                Some(OrderAmount::Quote(balance!(10000).into())),
                &mut data
            ),
            Vec::from([
                (balance!(10).into(), balance!(168.5).into()),
                (balance!(9.8).into(), balance!(139.9).into()),
                (balance!(9.5).into(), balance!(261.3).into())
            ])
        );

        // asks without limit
        assert_eq!(
            order_book.market_depth(PriceVariant::Sell, None, &mut data),
            Vec::from([
                (balance!(11).into(), balance!(176.3).into()),
                (balance!(11.2).into(), balance!(178.6).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );

        // asks, limit is a base asset

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Sell,
                Some(OrderAmount::Base(balance!(0).into())),
                &mut data
            ),
            Vec::new()
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Sell,
                Some(OrderAmount::Base(balance!(100).into())),
                &mut data
            ),
            Vec::from([(balance!(11).into(), balance!(176.3).into()),])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Sell,
                Some(OrderAmount::Base(balance!(200).into())),
                &mut data
            ),
            Vec::from([
                (balance!(11).into(), balance!(176.3).into()),
                (balance!(11.2).into(), balance!(178.6).into())
            ])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Sell,
                Some(OrderAmount::Base(balance!(400).into())),
                &mut data
            ),
            Vec::from([
                (balance!(11).into(), balance!(176.3).into()),
                (balance!(11.2).into(), balance!(178.6).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Sell,
                Some(OrderAmount::Base(balance!(1000).into())),
                &mut data
            ),
            Vec::from([
                (balance!(11).into(), balance!(176.3).into()),
                (balance!(11.2).into(), balance!(178.6).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );

        // asks, limit is a quote asset

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Sell,
                Some(OrderAmount::Quote(balance!(0).into())),
                &mut data
            ),
            Vec::new()
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Sell,
                Some(OrderAmount::Quote(balance!(1000).into())),
                &mut data
            ),
            Vec::from([(balance!(11).into(), balance!(176.3).into()),])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Sell,
                Some(OrderAmount::Quote(balance!(2000).into())),
                &mut data
            ),
            Vec::from([
                (balance!(11).into(), balance!(176.3).into()),
                (balance!(11.2).into(), balance!(178.6).into())
            ])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Sell,
                Some(OrderAmount::Quote(balance!(4000).into())),
                &mut data
            ),
            Vec::from([
                (balance!(11).into(), balance!(176.3).into()),
                (balance!(11.2).into(), balance!(178.6).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );

        assert_eq!(
            order_book.market_depth(
                PriceVariant::Sell,
                Some(OrderAmount::Quote(balance!(10000).into())),
                &mut data
            ),
            Vec::from([
                (balance!(11).into(), balance!(176.3).into()),
                (balance!(11.2).into(), balance!(178.6).into()),
                (balance!(11.5).into(), balance!(255.8).into())
            ])
        );
    });
}
