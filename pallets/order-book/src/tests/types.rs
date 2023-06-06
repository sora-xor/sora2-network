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
use common::{balance, PriceVariant, DAI, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_runtime::order_book::{
    DealInfo, LimitOrder, MarketChange, OrderAmount, OrderBookId, Payment,
};
use framenode_runtime::Runtime;
use sp_std::collections::btree_map::BTreeMap;

#[test]
fn check_order_amount() {
    let base_balance = balance!(10);
    let quote_balance = balance!(11);

    let base = OrderAmount::Base(base_balance);
    let quote = OrderAmount::Quote(quote_balance);

    assert_eq!(*base.value(), base_balance);
    assert_eq!(*quote.value(), quote_balance);

    assert!(base.is_base());
    assert!(!quote.is_base());

    assert!(!base.is_quote());
    assert!(quote.is_quote());

    assert!(base.is_same(&base));
    assert!(quote.is_same(&quote));
    assert!(!base.is_same(&quote));
    assert!(!quote.is_same(&base));

    assert_eq!(
        base.copy_type(balance!(100)),
        OrderAmount::Base(balance!(100))
    );
    assert_eq!(
        quote.copy_type(balance!(110)),
        OrderAmount::Quote(balance!(110))
    );

    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    assert_eq!(*base.associated_asset(&order_book_id), VAL);
    assert_eq!(*quote.associated_asset(&order_book_id), XOR);

    let base_balance2 = balance!(5);
    let quote_balance2 = balance!(6);

    let base2 = OrderAmount::Base(base_balance2);
    let quote2 = OrderAmount::Quote(quote_balance2);

    assert_eq!(
        (base + base2).unwrap(),
        OrderAmount::Base(base_balance + base_balance2)
    );
    assert_eq!(
        (quote + quote2).unwrap(),
        OrderAmount::Quote(quote_balance + quote_balance2)
    );
    assert_err!(base + quote, ());
    assert_err!(quote + base, ());

    assert_eq!(
        (base - base2).unwrap(),
        OrderAmount::Base(base_balance - base_balance2)
    );
    assert_eq!(
        (quote - quote2).unwrap(),
        OrderAmount::Quote(quote_balance - quote_balance2)
    );
    assert_err!(base - quote, ());
    assert_err!(quote - base, ());
}

#[test]
fn check_deal_info_valid() {
    // zero input amount
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(0)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0.5),
        side: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(0)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0.5),
        side: PriceVariant::Sell
    }
    .is_valid());

    // zero output amount
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(0)),
        average_price: balance!(0.5),
        side: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(0)),
        average_price: balance!(0.5),
        side: PriceVariant::Sell
    }
    .is_valid());

    // zero average price
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0),
        side: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0),
        side: PriceVariant::Sell
    }
    .is_valid());

    // equal assets
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0.5),
        side: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0.5),
        side: PriceVariant::Sell
    }
    .is_valid());

    // both are base
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0.5),
        side: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0.5),
        side: PriceVariant::Sell
    }
    .is_valid());

    // both are quote
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0.5),
        side: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0.5),
        side: PriceVariant::Sell
    }
    .is_valid());

    // valid
    assert!(DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0.5),
        side: PriceVariant::Buy
    }
    .is_valid());

    assert!(DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0.5),
        side: PriceVariant::Sell
    }
    .is_valid());
}

#[test]
fn check_deal_info_amounts() {
    assert_eq!(
        DealInfo {
            input_asset_id: XOR,
            input_amount: OrderAmount::Quote(balance!(1)),
            output_asset_id: VAL,
            output_amount: OrderAmount::Base(balance!(2)),
            average_price: balance!(0.5),
            side: PriceVariant::Buy
        }
        .base_amount(),
        balance!(2)
    );

    assert_eq!(
        DealInfo {
            input_asset_id: VAL,
            input_amount: OrderAmount::Base(balance!(1)),
            output_asset_id: XOR,
            output_amount: OrderAmount::Quote(balance!(2)),
            average_price: balance!(0.5),
            side: PriceVariant::Sell
        }
        .base_amount(),
        balance!(1)
    );

    assert_eq!(
        DealInfo {
            input_asset_id: XOR,
            input_amount: OrderAmount::Quote(balance!(1)),
            output_asset_id: VAL,
            output_amount: OrderAmount::Base(balance!(2)),
            average_price: balance!(0.5),
            side: PriceVariant::Buy
        }
        .quote_amount(),
        balance!(1)
    );

    assert_eq!(
        DealInfo {
            input_asset_id: VAL,
            input_amount: OrderAmount::Base(balance!(1)),
            output_asset_id: XOR,
            output_amount: OrderAmount::Quote(balance!(2)),
            average_price: balance!(0.5),
            side: PriceVariant::Sell
        }
        .quote_amount(),
        balance!(2)
    );
}

#[test]
fn should_fail_payment_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    let other_order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: DAI.into(),
        quote: XOR.into(),
    };

    assert_err!(
        Payment {
            dex_id: DEX,
            order_book_id,
            to_lock: BTreeMap::from([(XOR, BTreeMap::from([(alice(), balance!(100))]))]),
            to_unlock: BTreeMap::from([(VAL, BTreeMap::from([(bob(), balance!(50))]))])
        }
        .merge(&Payment {
            dex_id: common::DEXId::PolkaswapXSTUSD,
            order_book_id,
            to_lock: BTreeMap::from([(XOR, BTreeMap::from([(alice(), balance!(100))]))]),
            to_unlock: BTreeMap::from([(VAL, BTreeMap::from([(bob(), balance!(50))]))])
        }),
        ()
    );

    assert_err!(
        Payment {
            dex_id: DEX,
            order_book_id,
            to_lock: BTreeMap::from([(XOR, BTreeMap::from([(alice(), balance!(100))]))]),
            to_unlock: BTreeMap::from([(VAL, BTreeMap::from([(bob(), balance!(50))]))])
        }
        .merge(&Payment {
            dex_id: DEX,
            order_book_id: other_order_book_id,
            to_lock: BTreeMap::from([(XOR, BTreeMap::from([(alice(), balance!(100))]))]),
            to_unlock: BTreeMap::from([(DAI, BTreeMap::from([(bob(), balance!(50))]))])
        }),
        ()
    );
}

#[test]
fn check_payment_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    let origin = Payment {
        dex_id: DEX,
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([(alice(), balance!(10)), (bob(), balance!(20))]),
            ),
            (
                VAL,
                BTreeMap::from([(alice(), balance!(30)), (charlie(), balance!(40))]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([(bob(), balance!(50)), (charlie(), balance!(60))]),
            ),
            (
                XOR,
                BTreeMap::from([(bob(), balance!(70)), (dave(), balance!(80))]),
            ),
        ]),
    };

    let different = Payment {
        dex_id: DEX,
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([(charlie(), balance!(100)), (dave(), balance!(110))]),
            ),
            (
                VAL,
                BTreeMap::from([(bob(), balance!(120)), (dave(), balance!(130))]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([(alice(), balance!(140)), (dave(), balance!(150))]),
            ),
            (
                XOR,
                BTreeMap::from([(alice(), balance!(160)), (charlie(), balance!(170))]),
            ),
        ]),
    };

    let mut payment = origin.clone();
    assert_ok!(payment.merge(&different));
    assert_eq!(
        payment,
        Payment {
            dex_id: DEX,
            order_book_id,
            to_lock: BTreeMap::from([
                (
                    XOR,
                    BTreeMap::from([
                        (alice(), balance!(10)),
                        (bob(), balance!(20)),
                        (charlie(), balance!(100)),
                        (dave(), balance!(110))
                    ]),
                ),
                (
                    VAL,
                    BTreeMap::from([
                        (alice(), balance!(30)),
                        (bob(), balance!(120)),
                        (charlie(), balance!(40)),
                        (dave(), balance!(130))
                    ]),
                ),
            ]),
            to_unlock: BTreeMap::from([
                (
                    VAL,
                    BTreeMap::from([
                        (alice(), balance!(140)),
                        (bob(), balance!(50)),
                        (charlie(), balance!(60)),
                        (dave(), balance!(150))
                    ]),
                ),
                (
                    XOR,
                    BTreeMap::from([
                        (alice(), balance!(160)),
                        (bob(), balance!(70)),
                        (charlie(), balance!(170)),
                        (dave(), balance!(80))
                    ]),
                ),
            ]),
        }
    );

    let partial_match = Payment {
        dex_id: DEX,
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([(alice(), balance!(200)), (charlie(), balance!(210))]),
            ),
            (
                VAL,
                BTreeMap::from([(bob(), balance!(220)), (charlie(), balance!(230))]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([(bob(), balance!(240)), (dave(), balance!(250))]),
            ),
            (
                XOR,
                BTreeMap::from([(alice(), balance!(260)), (dave(), balance!(270))]),
            ),
        ]),
    };

    payment = origin.clone();
    assert_ok!(payment.merge(&partial_match));
    assert_eq!(
        payment,
        Payment {
            dex_id: DEX,
            order_book_id,
            to_lock: BTreeMap::from([
                (
                    XOR,
                    BTreeMap::from([
                        (alice(), balance!(210)),
                        (bob(), balance!(20)),
                        (charlie(), balance!(210))
                    ]),
                ),
                (
                    VAL,
                    BTreeMap::from([
                        (alice(), balance!(30)),
                        (bob(), balance!(220)),
                        (charlie(), balance!(270))
                    ]),
                ),
            ]),
            to_unlock: BTreeMap::from([
                (
                    VAL,
                    BTreeMap::from([
                        (bob(), balance!(290)),
                        (charlie(), balance!(60)),
                        (dave(), balance!(250))
                    ]),
                ),
                (
                    XOR,
                    BTreeMap::from([
                        (alice(), balance!(260)),
                        (bob(), balance!(70)),
                        (dave(), balance!(350))
                    ]),
                ),
            ]),
        }
    );

    let full_match = Payment {
        dex_id: DEX,
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([(alice(), balance!(300)), (bob(), balance!(310))]),
            ),
            (
                VAL,
                BTreeMap::from([(alice(), balance!(320)), (charlie(), balance!(330))]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([(bob(), balance!(340)), (charlie(), balance!(350))]),
            ),
            (
                XOR,
                BTreeMap::from([(bob(), balance!(360)), (dave(), balance!(370))]),
            ),
        ]),
    };

    payment = origin.clone();
    assert_ok!(payment.merge(&full_match));
    assert_eq!(
        payment,
        Payment {
            dex_id: DEX,
            order_book_id,
            to_lock: BTreeMap::from([
                (
                    XOR,
                    BTreeMap::from([(alice(), balance!(310)), (bob(), balance!(330))]),
                ),
                (
                    VAL,
                    BTreeMap::from([(alice(), balance!(350)), (charlie(), balance!(370))]),
                ),
            ]),
            to_unlock: BTreeMap::from([
                (
                    VAL,
                    BTreeMap::from([(bob(), balance!(390)), (charlie(), balance!(410))]),
                ),
                (
                    XOR,
                    BTreeMap::from([(bob(), balance!(430)), (dave(), balance!(450))]),
                ),
            ]),
        }
    );

    let empty = Payment {
        dex_id: DEX,
        order_book_id,
        to_lock: BTreeMap::new(),
        to_unlock: BTreeMap::new(),
    };

    payment = origin.clone();
    assert_ok!(payment.merge(&empty));
    assert_eq!(payment, origin);
}

#[test]
fn should_fail_market_change_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    let expiration_block = 3;

    let payment = Payment {
        dex_id: DEX,
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([(alice(), balance!(10)), (bob(), balance!(20))]),
            ),
            (
                VAL,
                BTreeMap::from([(alice(), balance!(30)), (charlie(), balance!(40))]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([(bob(), balance!(50)), (charlie(), balance!(60))]),
            ),
            (
                XOR,
                BTreeMap::from([(bob(), balance!(70)), (dave(), balance!(80))]),
            ),
        ]),
    };

    let origin = MarketChange {
        deal_input: None,
        deal_output: None,
        market_input: None,
        market_output: None,
        to_add: BTreeMap::from([(
            3,
            LimitOrder::<Runtime>::new(
                3,
                alice(),
                PriceVariant::Buy,
                balance!(10),
                balance!(100),
                1000,
                10000,
                100,
            ),
        )]),
        to_update: BTreeMap::from([(2, balance!(20))]),
        to_delete: BTreeMap::from([(1, expiration_block)]),
        payment,
    };

    let mut market_change = origin.clone();
    market_change.deal_input = Some(OrderAmount::Base(balance!(100)));
    market_change.deal_output = Some(OrderAmount::Quote(balance!(200)));
    market_change.market_input = Some(OrderAmount::Base(balance!(300)));
    market_change.market_output = Some(OrderAmount::Quote(balance!(400)));

    let mut diff_deal_input = origin.clone();
    diff_deal_input.deal_input = Some(OrderAmount::Quote(balance!(50)));
    assert_err!(market_change.merge(diff_deal_input), ());

    let mut diff_deal_output = origin.clone();
    diff_deal_output.deal_output = Some(OrderAmount::Base(balance!(50)));
    assert_err!(market_change.merge(diff_deal_output), ());

    let mut diff_market_input = origin.clone();
    diff_market_input.market_input = Some(OrderAmount::Quote(balance!(50)));
    assert_err!(market_change.merge(diff_market_input), ());

    let mut diff_market_output = origin.clone();
    diff_market_output.market_output = Some(OrderAmount::Base(balance!(50)));
    assert_err!(market_change.merge(diff_market_output), ());
}

#[test]
fn check_market_change_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    let expiration_block = 3;

    let payment = Payment {
        dex_id: DEX,
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([(alice(), balance!(10)), (bob(), balance!(20))]),
            ),
            (
                VAL,
                BTreeMap::from([(alice(), balance!(30)), (charlie(), balance!(40))]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([(bob(), balance!(50)), (charlie(), balance!(60))]),
            ),
            (
                XOR,
                BTreeMap::from([(bob(), balance!(70)), (dave(), balance!(80))]),
            ),
        ]),
    };

    let empty_payment = Payment {
        dex_id: DEX,
        order_book_id,
        to_lock: BTreeMap::new(),
        to_unlock: BTreeMap::new(),
    };

    let add_id1 = 301;
    let add_id2 = 302;
    let add_id3 = 303;
    let add_id4 = 304;
    let add_id5 = 305;

    let limit_order1 = LimitOrder::<Runtime>::new(
        add_id1,
        alice(),
        PriceVariant::Buy,
        balance!(10),
        balance!(100),
        10,
        100,
        100,
    );

    let limit_order1_changed = LimitOrder::<Runtime>::new(
        add_id1,
        alice(),
        PriceVariant::Buy,
        balance!(9),
        balance!(1000),
        10,
        100,
        100,
    );

    let limit_order2 = LimitOrder::<Runtime>::new(
        add_id2,
        bob(),
        PriceVariant::Sell,
        balance!(15),
        balance!(100),
        10,
        100,
        100,
    );

    let limit_order3 = LimitOrder::<Runtime>::new(
        add_id3,
        charlie(),
        PriceVariant::Buy,
        balance!(11),
        balance!(100),
        10,
        100,
        100,
    );

    let limit_order4 = LimitOrder::<Runtime>::new(
        add_id4,
        dave(),
        PriceVariant::Sell,
        balance!(16),
        balance!(100),
        10,
        100,
        100,
    );

    let limit_order5 = LimitOrder::<Runtime>::new(
        add_id5,
        alice(),
        PriceVariant::Buy,
        balance!(12),
        balance!(100),
        10,
        100,
        100,
    );

    let update_id1 = 201;
    let update_id2 = 202;
    let update_id3 = 203;
    let update_id4 = 204;
    let update_id5 = 205;

    let delete_id1 = 101;
    let delete_id2 = 102;
    let delete_id3 = 103;
    let delete_id4 = 104;
    let delete_id5 = 105;

    let origin = MarketChange {
        deal_input: Some(OrderAmount::Base(balance!(1000))),
        deal_output: Some(OrderAmount::Quote(balance!(2000))),
        market_input: Some(OrderAmount::Base(balance!(3000))),
        market_output: Some(OrderAmount::Quote(balance!(4000))),
        to_add: BTreeMap::from([
            (add_id1, limit_order1.clone()),
            (add_id2, limit_order2.clone()),
            (add_id3, limit_order3.clone()),
        ]),
        to_update: BTreeMap::from([
            (update_id1, balance!(20)),
            (update_id2, balance!(30)),
            (update_id3, balance!(40)),
        ]),
        to_delete: BTreeMap::from([
            (delete_id1, expiration_block),
            (delete_id2, expiration_block),
            (delete_id3, expiration_block),
        ]),
        payment: payment.clone(),
    };

    let different = MarketChange {
        deal_input: None,
        deal_output: None,
        market_input: None,
        market_output: None,
        to_add: BTreeMap::from([
            (add_id4, limit_order4.clone()),
            (add_id5, limit_order5.clone()),
        ]),
        to_update: BTreeMap::from([(update_id4, balance!(50)), (update_id5, balance!(60))]),
        to_delete: BTreeMap::from([
            (delete_id4, expiration_block),
            (delete_id5, expiration_block),
        ]),
        payment: empty_payment.clone(),
    };

    let mut market_change = origin.clone();
    assert_ok!(market_change.merge(different));
    assert_eq!(
        market_change,
        MarketChange {
            deal_input: Some(OrderAmount::Base(balance!(1000))),
            deal_output: Some(OrderAmount::Quote(balance!(2000))),
            market_input: Some(OrderAmount::Base(balance!(3000))),
            market_output: Some(OrderAmount::Quote(balance!(4000))),
            to_add: BTreeMap::from([
                (add_id1, limit_order1.clone()),
                (add_id2, limit_order2.clone()),
                (add_id3, limit_order3.clone()),
                (add_id4, limit_order4.clone()),
                (add_id5, limit_order5.clone()),
            ]),
            to_update: BTreeMap::from([
                (update_id1, balance!(20)),
                (update_id2, balance!(30)),
                (update_id3, balance!(40)),
                (update_id4, balance!(50)),
                (update_id5, balance!(60))
            ]),
            to_delete: BTreeMap::from([
                (delete_id1, expiration_block),
                (delete_id2, expiration_block),
                (delete_id3, expiration_block),
                (delete_id4, expiration_block),
                (delete_id5, expiration_block)
            ]),
            payment: payment.clone(),
        }
    );

    let partial_match = MarketChange {
        deal_input: Some(OrderAmount::Base(balance!(7000))),
        deal_output: Some(OrderAmount::Quote(balance!(8000))),
        market_input: None,
        market_output: None,
        to_add: BTreeMap::from([
            (add_id1, limit_order1_changed.clone()),
            (add_id2, limit_order2.clone()),
            (add_id5, limit_order5.clone()),
        ]),
        to_update: BTreeMap::from([
            (update_id1, balance!(120)),
            (update_id2, balance!(30)),
            (update_id5, balance!(60)),
        ]),
        to_delete: BTreeMap::from([
            (delete_id1, expiration_block),
            (delete_id2, expiration_block),
            (delete_id5, expiration_block),
        ]),
        payment: empty_payment.clone(),
    };

    market_change = origin.clone();
    assert_ok!(market_change.merge(partial_match));
    assert_eq!(
        market_change,
        MarketChange {
            deal_input: Some(OrderAmount::Base(balance!(8000))),
            deal_output: Some(OrderAmount::Quote(balance!(10000))),
            market_input: Some(OrderAmount::Base(balance!(3000))),
            market_output: Some(OrderAmount::Quote(balance!(4000))),
            to_add: BTreeMap::from([
                (add_id1, limit_order1_changed.clone()),
                (add_id2, limit_order2.clone()),
                (add_id3, limit_order3.clone()),
                (add_id5, limit_order5.clone()),
            ]),
            to_update: BTreeMap::from([
                (update_id1, balance!(120)),
                (update_id2, balance!(30)),
                (update_id3, balance!(40)),
                (update_id5, balance!(60))
            ]),
            to_delete: BTreeMap::from([
                (delete_id1, expiration_block),
                (delete_id2, expiration_block),
                (delete_id3, expiration_block),
                (delete_id5, expiration_block)
            ]),
            payment: payment.clone(),
        }
    );

    let full_match = MarketChange {
        deal_input: Some(OrderAmount::Base(balance!(1000))),
        deal_output: Some(OrderAmount::Quote(balance!(2000))),
        market_input: Some(OrderAmount::Base(balance!(3000))),
        market_output: Some(OrderAmount::Quote(balance!(4000))),
        to_add: BTreeMap::from([
            (add_id1, limit_order1.clone()),
            (add_id2, limit_order2.clone()),
            (add_id3, limit_order3.clone()),
        ]),
        to_update: BTreeMap::from([
            (update_id1, balance!(20)),
            (update_id2, balance!(30)),
            (update_id3, balance!(40)),
        ]),
        to_delete: BTreeMap::from([
            (delete_id1, expiration_block),
            (delete_id2, expiration_block),
            (delete_id3, expiration_block),
        ]),
        payment: empty_payment.clone(),
    };

    market_change = origin.clone();
    assert_ok!(market_change.merge(full_match));
    assert_eq!(
        market_change,
        MarketChange {
            deal_input: Some(OrderAmount::Base(balance!(2000))),
            deal_output: Some(OrderAmount::Quote(balance!(4000))),
            market_input: Some(OrderAmount::Base(balance!(6000))),
            market_output: Some(OrderAmount::Quote(balance!(8000))),
            to_add: BTreeMap::from([
                (add_id1, limit_order1.clone()),
                (add_id2, limit_order2.clone()),
                (add_id3, limit_order3.clone()),
            ]),
            to_update: BTreeMap::from([
                (update_id1, balance!(20)),
                (update_id2, balance!(30)),
                (update_id3, balance!(40)),
            ]),
            to_delete: BTreeMap::from([
                (delete_id1, expiration_block),
                (delete_id2, expiration_block),
                (delete_id3, expiration_block)
            ]),
            payment: payment.clone(),
        }
    );

    let empty = MarketChange {
        deal_input: None,
        deal_output: None,
        market_input: None,
        market_output: None,
        to_add: BTreeMap::new(),
        to_update: BTreeMap::new(),
        to_delete: BTreeMap::new(),
        payment: empty_payment.clone(),
    };

    market_change = origin.clone();
    assert_ok!(market_change.merge(empty));
    assert_eq!(market_change, origin);
}
