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
use common::{balance, PriceVariant, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_runtime::order_book::{
    Config, LimitOrder, MarketOrder, MarketRole, OrderAmount, OrderBookId,
};
use framenode_runtime::Runtime;

#[test]
fn should_return_error_for_invalid_limit_order_lifespan() {
    let wrong_lifespan1 = 0;
    let order1 = LimitOrder::<Runtime>::new(
        0,
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        balance!(10).into(),
        balance!(100).into(),
        1000,
        wrong_lifespan1,
        1,
    );
    assert_err!(order1.ensure_valid(), E::InvalidLifespan);

    let wrong_lifespan2 = Runtime::MAX_ORDER_LIFESPAN + 1;
    let order2 = LimitOrder::<Runtime>::new(
        0,
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        balance!(10).into(),
        balance!(100).into(),
        1000,
        wrong_lifespan2,
        1,
    );
    assert_err!(order2.ensure_valid(), E::InvalidLifespan);
}

#[test]
fn should_return_error_for_invalid_limit_order_amount() {
    let wrong_amount = balance!(0).into();
    let order = LimitOrder::<Runtime>::new(
        0,
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        balance!(10).into(),
        wrong_amount,
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        1,
    );
    assert_err!(order.ensure_valid(), E::InvalidOrderAmount);
}

#[test]
fn should_return_error_for_invalid_market_order_amount() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL,
        quote: XOR,
    };

    let wrong_amount = balance!(0).into();
    let order = MarketOrder::<Runtime>::new(
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        order_book_id,
        wrong_amount,
        None,
    );
    assert_err!(order.ensure_valid(), E::InvalidOrderAmount);
}

#[test]
fn should_return_error_for_invalid_limit_order_price() {
    let wrong_price = balance!(0).into();
    let order = LimitOrder::<Runtime>::new(
        0,
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        wrong_price,
        balance!(100).into(),
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        1,
    );
    assert_err!(order.ensure_valid(), E::InvalidLimitOrderPrice);
}

#[test]
fn should_pass_valid_limit_order() {
    let price = balance!(10).into();
    let amount = balance!(100).into();
    let lifespan1 = Runtime::MIN_ORDER_LIFESPAN;
    let lifespan2 = Runtime::MIN_ORDER_LIFESPAN + 1000;
    let lifespan3 = Runtime::MAX_ORDER_LIFESPAN;

    let mut order = LimitOrder::<Runtime>::new(
        0,
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        price,
        amount,
        1000,
        lifespan1,
        1,
    );
    assert_ok!(order.ensure_valid());

    order.lifespan = lifespan2;
    assert_ok!(order.ensure_valid());

    order.lifespan = lifespan3;
    assert_ok!(order.ensure_valid());
}

#[test]
fn should_pass_valid_market_order() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL,
        quote: XOR,
    };

    let amount = balance!(10).into();
    let order = MarketOrder::<Runtime>::new(
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        order_book_id,
        amount,
        None,
    );
    assert_ok!(order.ensure_valid());
}

#[test]
fn should_not_return_limit_order_deal_amount_with_big_base_limit() {
    let price = balance!(11).into();
    let amount = balance!(100).into();
    let base_amount_limit = balance!(101).into();

    let buy_order = LimitOrder::<Runtime>::new(
        1,
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        price,
        amount,
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        1,
    );

    let sell_order = LimitOrder::<Runtime>::new(
        2,
        accounts::alice::<Runtime>(),
        PriceVariant::Sell,
        price,
        amount,
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        1,
    );

    assert_err!(
        buy_order.deal_amount(MarketRole::Maker, Some(base_amount_limit)),
        E::InvalidOrderAmount
    );
    assert_err!(
        buy_order.deal_amount(MarketRole::Taker, Some(base_amount_limit)),
        E::InvalidOrderAmount
    );
    assert_err!(
        sell_order.deal_amount(MarketRole::Maker, Some(base_amount_limit)),
        E::InvalidOrderAmount
    );
    assert_err!(
        sell_order.deal_amount(MarketRole::Taker, Some(base_amount_limit)),
        E::InvalidOrderAmount
    );
}

#[test]
fn should_return_limit_order_deal_amount() {
    let price = balance!(11).into();
    let amount = balance!(100).into();

    let buy_order = LimitOrder::<Runtime>::new(
        1,
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        price,
        amount,
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        1,
    );

    let sell_order = LimitOrder::<Runtime>::new(
        2,
        accounts::alice::<Runtime>(),
        PriceVariant::Sell,
        price,
        amount,
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        1,
    );

    assert_eq!(
        buy_order.deal_amount(MarketRole::Maker, None).unwrap(),
        OrderAmount::Base(amount)
    );
    assert_eq!(
        buy_order.deal_amount(MarketRole::Taker, None).unwrap(),
        OrderAmount::Quote(balance!(1100).into())
    );
    assert_eq!(
        sell_order.deal_amount(MarketRole::Maker, None).unwrap(),
        OrderAmount::Quote(balance!(1100).into())
    );
    assert_eq!(
        sell_order.deal_amount(MarketRole::Taker, None).unwrap(),
        OrderAmount::Base(amount)
    );

    let base_amount_limit = balance!(50).into();
    assert_eq!(
        buy_order
            .deal_amount(MarketRole::Maker, Some(base_amount_limit))
            .unwrap(),
        OrderAmount::Base(base_amount_limit)
    );
    assert_eq!(
        buy_order
            .deal_amount(MarketRole::Taker, Some(base_amount_limit))
            .unwrap(),
        OrderAmount::Quote(balance!(550).into())
    );
    assert_eq!(
        sell_order
            .deal_amount(MarketRole::Maker, Some(base_amount_limit))
            .unwrap(),
        OrderAmount::Quote(balance!(550).into())
    );
    assert_eq!(
        sell_order
            .deal_amount(MarketRole::Taker, Some(base_amount_limit))
            .unwrap(),
        OrderAmount::Base(base_amount_limit)
    );

    assert_eq!(
        buy_order
            .deal_amount(MarketRole::Maker, Some(amount))
            .unwrap(),
        OrderAmount::Base(amount)
    );
    assert_eq!(
        buy_order
            .deal_amount(MarketRole::Taker, Some(amount))
            .unwrap(),
        OrderAmount::Quote(balance!(1100).into())
    );
    assert_eq!(
        sell_order
            .deal_amount(MarketRole::Maker, Some(amount))
            .unwrap(),
        OrderAmount::Quote(balance!(1100).into())
    );
    assert_eq!(
        sell_order
            .deal_amount(MarketRole::Taker, Some(amount))
            .unwrap(),
        OrderAmount::Base(amount)
    );
}
