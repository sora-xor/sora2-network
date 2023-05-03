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
use common::{balance, PriceVariant, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_runtime::order_book::{Config, LimitOrder, OrderBookId};
use framenode_runtime::Runtime;

#[test]
fn should_return_error_for_invalid_lifetime() {
    let wrong_lifespan1 = 0;
    let order1 = LimitOrder::<Runtime>::new(
        0,
        alice(),
        PriceVariant::Buy,
        balance!(10),
        balance!(100),
        1000,
        wrong_lifespan1,
    );
    assert_err!(order1.ensure_valid(), E::InvalidLifespan);

    let wrong_lifespan2 = Runtime::MAX_ORDER_LIFETIME + 1;
    let order2 = LimitOrder::<Runtime>::new(
        0,
        alice(),
        PriceVariant::Buy,
        balance!(10),
        balance!(100),
        1000,
        wrong_lifespan2,
    );
    assert_err!(order2.ensure_valid(), E::InvalidLifespan);
}

#[test]
fn should_return_error_for_invalid_amount() {
    let wrong_balance = balance!(0);
    let order = LimitOrder::<Runtime>::new(
        0,
        alice(),
        PriceVariant::Buy,
        balance!(10),
        wrong_balance,
        1000,
        10000,
    );
    assert_err!(order.ensure_valid(), E::InvalidOrderAmount);
}

#[test]
fn should_return_error_for_invalid_price() {
    let wrong_price = balance!(0);
    let order = LimitOrder::<Runtime>::new(
        0,
        alice(),
        PriceVariant::Buy,
        wrong_price,
        balance!(100),
        1000,
        10000,
    );
    assert_err!(order.ensure_valid(), E::InvalidLimitOrderPrice);
}

#[test]
fn should_pass_valid_limit_order() {
    let price = balance!(10);
    let amount = balance!(100);
    let lifespan1 = Runtime::MIN_ORDER_LIFETIME;
    let lifespan2 = Runtime::MIN_ORDER_LIFETIME + 1000;
    let lifespan3 = Runtime::MAX_ORDER_LIFETIME;

    let mut order = LimitOrder::<Runtime>::new(
        0,
        alice(),
        PriceVariant::Buy,
        price,
        amount,
        1000,
        lifespan1,
    );
    assert_ok!(order.ensure_valid());

    order.lifespan = lifespan2;
    assert_ok!(order.ensure_valid());

    order.lifespan = lifespan3;
    assert_ok!(order.ensure_valid());
}

#[test]
fn should_return_appropriate_amount() {
    let price = balance!(11);
    let amount = balance!(100);

    let buy_order =
        LimitOrder::<Runtime>::new(1, alice(), PriceVariant::Buy, price, amount, 1000, 10000);

    let sell_order =
        LimitOrder::<Runtime>::new(2, alice(), PriceVariant::Sell, price, amount, 1000, 10000);

    let buy_appropriate_amount = buy_order.appropriate_amount().unwrap();
    let sell_appropriate_amount = sell_order.appropriate_amount().unwrap();

    assert_eq!(buy_appropriate_amount, balance!(1100));
    assert_eq!(sell_appropriate_amount, amount);
}

#[test]
fn should_return_appropriate_asset_and_amount() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    let price = balance!(12.5);
    let amount = balance!(100);

    let buy_order =
        LimitOrder::<Runtime>::new(1, alice(), PriceVariant::Buy, price, amount, 1000, 10000);

    let sell_order =
        LimitOrder::<Runtime>::new(2, alice(), PriceVariant::Sell, price, amount, 1000, 10000);

    let (buy_appropriate_asset, buy_appropriate_amount) = buy_order
        .appropriate_asset_and_amount(&order_book_id)
        .unwrap();
    let (sell_appropriate_asset, sell_appropriate_amount) = sell_order
        .appropriate_asset_and_amount(&order_book_id)
        .unwrap();

    assert_eq!(*buy_appropriate_asset, order_book_id.quote);
    assert_eq!(buy_appropriate_amount, balance!(1250));

    assert_eq!(*sell_appropriate_asset, order_book_id.base);
    assert_eq!(sell_appropriate_amount, amount);
}
