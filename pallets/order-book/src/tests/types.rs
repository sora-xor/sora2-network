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
use common::{balance, PriceVariant, DAI, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
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

    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
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
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(0)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0.5),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // zero output amount
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(0)),
        average_price: balance!(0.5),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(0)),
        average_price: balance!(0.5),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // zero average price
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // equal assets
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0.5),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0.5),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // both are base
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0.5),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0.5),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // both are quote
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0.5),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0.5),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // valid
    assert!(DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1)),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2)),
        average_price: balance!(0.5),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1)),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2)),
        average_price: balance!(0.5),
        direction: PriceVariant::Sell
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
            direction: PriceVariant::Buy
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
            direction: PriceVariant::Sell
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
            direction: PriceVariant::Buy
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
            direction: PriceVariant::Sell
        }
        .quote_amount(),
        balance!(2)
    );
}

#[test]
fn should_fail_payment_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };

    let other_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: DAI.into(),
        quote: XOR.into(),
    };

    assert_err!(
        Payment {
            order_book_id,
            to_lock: BTreeMap::from([(XOR, BTreeMap::from([(alice::<Runtime>(), balance!(100))]))]),
            to_unlock: BTreeMap::from([(VAL, BTreeMap::from([(bob::<Runtime>(), balance!(50))]))])
        }
        .merge(&Payment {
            order_book_id: other_order_book_id,
            to_lock: BTreeMap::from([(XOR, BTreeMap::from([(alice::<Runtime>(), balance!(100))]))]),
            to_unlock: BTreeMap::from([(DAI, BTreeMap::from([(bob::<Runtime>(), balance!(50))]))])
        }),
        ()
    );
}

#[test]
fn check_payment_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };

    let origin = Payment {
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(10)),
                    (bob::<Runtime>(), balance!(20)),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(30)),
                    (charlie::<Runtime>(), balance!(40)),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(50)),
                    (charlie::<Runtime>(), balance!(60)),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(70)),
                    (dave::<Runtime>(), balance!(80)),
                ]),
            ),
        ]),
    };

    let different = Payment {
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([
                    (charlie::<Runtime>(), balance!(100)),
                    (dave::<Runtime>(), balance!(110)),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(120)),
                    (dave::<Runtime>(), balance!(130)),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(140)),
                    (dave::<Runtime>(), balance!(150)),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(160)),
                    (charlie::<Runtime>(), balance!(170)),
                ]),
            ),
        ]),
    };

    let mut payment = origin.clone();
    assert_ok!(payment.merge(&different));
    assert_eq!(
        payment,
        Payment {
            order_book_id,
            to_lock: BTreeMap::from([
                (
                    XOR,
                    BTreeMap::from([
                        (alice::<Runtime>(), balance!(10)),
                        (bob::<Runtime>(), balance!(20)),
                        (charlie::<Runtime>(), balance!(100)),
                        (dave::<Runtime>(), balance!(110))
                    ]),
                ),
                (
                    VAL,
                    BTreeMap::from([
                        (alice::<Runtime>(), balance!(30)),
                        (bob::<Runtime>(), balance!(120)),
                        (charlie::<Runtime>(), balance!(40)),
                        (dave::<Runtime>(), balance!(130))
                    ]),
                ),
            ]),
            to_unlock: BTreeMap::from([
                (
                    VAL,
                    BTreeMap::from([
                        (alice::<Runtime>(), balance!(140)),
                        (bob::<Runtime>(), balance!(50)),
                        (charlie::<Runtime>(), balance!(60)),
                        (dave::<Runtime>(), balance!(150))
                    ]),
                ),
                (
                    XOR,
                    BTreeMap::from([
                        (alice::<Runtime>(), balance!(160)),
                        (bob::<Runtime>(), balance!(70)),
                        (charlie::<Runtime>(), balance!(170)),
                        (dave::<Runtime>(), balance!(80))
                    ]),
                ),
            ]),
        }
    );

    let partial_match = Payment {
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(200)),
                    (charlie::<Runtime>(), balance!(210)),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(220)),
                    (charlie::<Runtime>(), balance!(230)),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(240)),
                    (dave::<Runtime>(), balance!(250)),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(260)),
                    (dave::<Runtime>(), balance!(270)),
                ]),
            ),
        ]),
    };

    payment = origin.clone();
    assert_ok!(payment.merge(&partial_match));
    assert_eq!(
        payment,
        Payment {
            order_book_id,
            to_lock: BTreeMap::from([
                (
                    XOR,
                    BTreeMap::from([
                        (alice::<Runtime>(), balance!(210)),
                        (bob::<Runtime>(), balance!(20)),
                        (charlie::<Runtime>(), balance!(210))
                    ]),
                ),
                (
                    VAL,
                    BTreeMap::from([
                        (alice::<Runtime>(), balance!(30)),
                        (bob::<Runtime>(), balance!(220)),
                        (charlie::<Runtime>(), balance!(270))
                    ]),
                ),
            ]),
            to_unlock: BTreeMap::from([
                (
                    VAL,
                    BTreeMap::from([
                        (bob::<Runtime>(), balance!(290)),
                        (charlie::<Runtime>(), balance!(60)),
                        (dave::<Runtime>(), balance!(250))
                    ]),
                ),
                (
                    XOR,
                    BTreeMap::from([
                        (alice::<Runtime>(), balance!(260)),
                        (bob::<Runtime>(), balance!(70)),
                        (dave::<Runtime>(), balance!(350))
                    ]),
                ),
            ]),
        }
    );

    let full_match = Payment {
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(300)),
                    (bob::<Runtime>(), balance!(310)),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(320)),
                    (charlie::<Runtime>(), balance!(330)),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(340)),
                    (charlie::<Runtime>(), balance!(350)),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(360)),
                    (dave::<Runtime>(), balance!(370)),
                ]),
            ),
        ]),
    };

    payment = origin.clone();
    assert_ok!(payment.merge(&full_match));
    assert_eq!(
        payment,
        Payment {
            order_book_id,
            to_lock: BTreeMap::from([
                (
                    XOR,
                    BTreeMap::from([
                        (alice::<Runtime>(), balance!(310)),
                        (bob::<Runtime>(), balance!(330))
                    ]),
                ),
                (
                    VAL,
                    BTreeMap::from([
                        (alice::<Runtime>(), balance!(350)),
                        (charlie::<Runtime>(), balance!(370))
                    ]),
                ),
            ]),
            to_unlock: BTreeMap::from([
                (
                    VAL,
                    BTreeMap::from([
                        (bob::<Runtime>(), balance!(390)),
                        (charlie::<Runtime>(), balance!(410))
                    ]),
                ),
                (
                    XOR,
                    BTreeMap::from([
                        (bob::<Runtime>(), balance!(430)),
                        (dave::<Runtime>(), balance!(450))
                    ]),
                ),
            ]),
        }
    );

    let empty = Payment {
        order_book_id,
        to_lock: BTreeMap::new(),
        to_unlock: BTreeMap::new(),
    };

    payment = origin.clone();
    assert_ok!(payment.merge(&empty));
    assert_eq!(payment, origin);
}

#[test]
fn check_payment_execute_all() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL.into(),
            quote: XOR.into(),
        };

        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        fill_balance(alice::<Runtime>(), order_book_id);
        fill_balance(bob::<Runtime>(), order_book_id);
        fill_balance(charlie::<Runtime>(), order_book_id);
        fill_balance(dave::<Runtime>(), order_book_id);

        let balance_diff = balance!(150);

        let alice_base_balance = free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>());
        let alice_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>());
        let bob_base_balance = free_balance::<Runtime>(&order_book_id.base, &bob::<Runtime>());
        let bob_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &bob::<Runtime>());
        let charlie_base_balance =
            free_balance::<Runtime>(&order_book_id.base, &charlie::<Runtime>());
        let charlie_quote_balance =
            free_balance::<Runtime>(&order_book_id.quote, &charlie::<Runtime>());
        let dave_base_balance = free_balance::<Runtime>(&order_book_id.base, &dave::<Runtime>());
        let dave_quote_balance = free_balance::<Runtime>(&order_book_id.quote, &dave::<Runtime>());

        let payment = Payment {
            order_book_id,
            to_lock: BTreeMap::from([
                (
                    order_book_id.base,
                    BTreeMap::from([(alice::<Runtime>(), balance_diff)]),
                ),
                (
                    order_book_id.quote,
                    BTreeMap::from([(bob::<Runtime>(), balance_diff)]),
                ),
            ]),
            to_unlock: BTreeMap::from([
                (
                    order_book_id.base,
                    BTreeMap::from([(charlie::<Runtime>(), balance_diff)]),
                ),
                (
                    order_book_id.quote,
                    BTreeMap::from([(dave::<Runtime>(), balance_diff)]),
                ),
            ]),
        };

        assert_ok!(payment.execute_all::<OrderBookPallet, OrderBookPallet>());

        assert_eq!(
            alice_base_balance - balance_diff,
            free_balance::<Runtime>(&order_book_id.base, &alice::<Runtime>())
        );
        assert_eq!(
            alice_quote_balance,
            free_balance::<Runtime>(&order_book_id.quote, &alice::<Runtime>())
        );
        assert_eq!(
            bob_base_balance,
            free_balance::<Runtime>(&order_book_id.base, &bob::<Runtime>())
        );
        assert_eq!(
            bob_quote_balance - balance_diff,
            free_balance::<Runtime>(&order_book_id.quote, &bob::<Runtime>())
        );
        assert_eq!(
            charlie_base_balance + balance_diff,
            free_balance::<Runtime>(&order_book_id.base, &charlie::<Runtime>())
        );
        assert_eq!(
            charlie_quote_balance,
            free_balance::<Runtime>(&order_book_id.quote, &charlie::<Runtime>())
        );
        assert_eq!(
            dave_base_balance,
            free_balance::<Runtime>(&order_book_id.base, &dave::<Runtime>())
        );
        assert_eq!(
            dave_quote_balance + balance_diff,
            free_balance::<Runtime>(&order_book_id.quote, &dave::<Runtime>())
        );
    });
}

#[test]
fn should_fail_market_change_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };

    let payment = Payment {
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(10)),
                    (bob::<Runtime>(), balance!(20)),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(30)),
                    (charlie::<Runtime>(), balance!(40)),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(50)),
                    (charlie::<Runtime>(), balance!(60)),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(70)),
                    (dave::<Runtime>(), balance!(80)),
                ]),
            ),
        ]),
    };

    let origin = MarketChange {
        deal_input: None,
        deal_output: None,
        market_input: None,
        market_output: None,
        to_place: BTreeMap::from([(
            4,
            LimitOrder::<Runtime>::new(
                4,
                alice::<Runtime>(),
                PriceVariant::Buy,
                balance!(10),
                balance!(100),
                1000,
                10000,
                100,
            ),
        )]),
        to_part_execute: BTreeMap::from([(
            3,
            (
                LimitOrder::<Runtime>::new(
                    3,
                    alice::<Runtime>(),
                    PriceVariant::Buy,
                    balance!(20),
                    balance!(100),
                    1000,
                    10000,
                    100,
                ),
                OrderAmount::Base(balance!(10)),
            ),
        )]),
        to_full_execute: BTreeMap::from([(
            2,
            LimitOrder::<Runtime>::new(
                2,
                alice::<Runtime>(),
                PriceVariant::Buy,
                balance!(20),
                balance!(100),
                1000,
                10000,
                100,
            ),
        )]),
        to_cancel: BTreeMap::from([(
            1,
            LimitOrder::<Runtime>::new(
                1,
                alice::<Runtime>(),
                PriceVariant::Buy,
                balance!(10),
                balance!(100),
                1000,
                10000,
                100,
            ),
        )]),
        payment,
        ignore_unschedule_error: false,
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
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL.into(),
        quote: XOR.into(),
    };

    let payment = Payment {
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(10)),
                    (bob::<Runtime>(), balance!(20)),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (alice::<Runtime>(), balance!(30)),
                    (charlie::<Runtime>(), balance!(40)),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(50)),
                    (charlie::<Runtime>(), balance!(60)),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (bob::<Runtime>(), balance!(70)),
                    (dave::<Runtime>(), balance!(80)),
                ]),
            ),
        ]),
    };

    let empty_payment = Payment {
        order_book_id,
        to_lock: BTreeMap::new(),
        to_unlock: BTreeMap::new(),
    };

    let order_id1 = 101;
    let order_id2 = 102;
    let order_id3 = 103;
    let order_id4 = 104;
    let order_id5 = 105;

    let order1_origin = LimitOrder::<Runtime>::new(
        order_id1,
        alice::<Runtime>(),
        PriceVariant::Buy,
        balance!(10),
        balance!(100),
        1000,
        10000,
        100,
    );

    let order1_other = LimitOrder::<Runtime>::new(
        order_id1,
        alice::<Runtime>(),
        PriceVariant::Buy,
        balance!(9),
        balance!(1000),
        1000,
        10000,
        100,
    );

    let order2_origin = LimitOrder::<Runtime>::new(
        order_id2,
        bob::<Runtime>(),
        PriceVariant::Sell,
        balance!(15),
        balance!(100),
        1000,
        10000,
        100,
    );

    let order2_other = LimitOrder::<Runtime>::new(
        order_id2,
        bob::<Runtime>(),
        PriceVariant::Buy,
        balance!(14),
        balance!(200),
        1000,
        10000,
        100,
    );

    let order3_origin = LimitOrder::<Runtime>::new(
        order_id3,
        charlie::<Runtime>(),
        PriceVariant::Buy,
        balance!(11),
        balance!(100),
        1000,
        10000,
        100,
    );

    let order3_other = LimitOrder::<Runtime>::new(
        order_id3,
        charlie::<Runtime>(),
        PriceVariant::Buy,
        balance!(12),
        balance!(1000),
        1000,
        10000,
        100,
    );

    let order4_origin = LimitOrder::<Runtime>::new(
        order_id4,
        dave::<Runtime>(),
        PriceVariant::Sell,
        balance!(16),
        balance!(100),
        1000,
        10000,
        100,
    );

    let order5_origin = LimitOrder::<Runtime>::new(
        order_id5,
        alice::<Runtime>(),
        PriceVariant::Buy,
        balance!(12),
        balance!(100),
        1000,
        10000,
        100,
    );

    let origin = MarketChange {
        deal_input: Some(OrderAmount::Base(balance!(1000))),
        deal_output: Some(OrderAmount::Quote(balance!(2000))),
        market_input: Some(OrderAmount::Base(balance!(3000))),
        market_output: Some(OrderAmount::Quote(balance!(4000))),
        to_place: BTreeMap::from([
            (order_id1, order1_origin.clone()),
            (order_id2, order2_origin.clone()),
            (order_id3, order3_origin.clone()),
        ]),
        to_part_execute: BTreeMap::from([
            (
                order_id1,
                (order1_origin.clone(), OrderAmount::Base(balance!(20))),
            ),
            (
                order_id2,
                (order2_origin.clone(), OrderAmount::Base(balance!(30))),
            ),
            (
                order_id3,
                (order3_origin.clone(), OrderAmount::Base(balance!(40))),
            ),
        ]),
        to_full_execute: BTreeMap::from([
            (order_id1, order1_origin.clone()),
            (order_id2, order2_origin.clone()),
            (order_id3, order3_origin.clone()),
        ]),
        to_cancel: BTreeMap::from([
            (order_id1, order1_origin.clone()),
            (order_id2, order2_origin.clone()),
            (order_id3, order3_origin.clone()),
        ]),
        payment: payment.clone(),
        ignore_unschedule_error: false,
    };

    let different = MarketChange {
        deal_input: None,
        deal_output: None,
        market_input: None,
        market_output: None,
        to_place: BTreeMap::from([
            (order_id4, order4_origin.clone()),
            (order_id5, order5_origin.clone()),
        ]),
        to_part_execute: BTreeMap::from([
            (
                order_id4,
                (order4_origin.clone(), OrderAmount::Base(balance!(50))),
            ),
            (
                order_id5,
                (order5_origin.clone(), OrderAmount::Base(balance!(60))),
            ),
        ]),
        to_full_execute: BTreeMap::from([
            (order_id4, order4_origin.clone()),
            (order_id5, order5_origin.clone()),
        ]),
        to_cancel: BTreeMap::from([
            (order_id4, order4_origin.clone()),
            (order_id5, order5_origin.clone()),
        ]),
        payment: empty_payment.clone(),
        ignore_unschedule_error: false,
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
            to_place: BTreeMap::from([
                (order_id1, order1_origin.clone()),
                (order_id2, order2_origin.clone()),
                (order_id3, order3_origin.clone()),
                (order_id4, order4_origin.clone()),
                (order_id5, order5_origin.clone()),
            ]),
            to_part_execute: BTreeMap::from([
                (
                    order_id1,
                    (order1_origin.clone(), OrderAmount::Base(balance!(20)))
                ),
                (
                    order_id2,
                    (order2_origin.clone(), OrderAmount::Base(balance!(30)))
                ),
                (
                    order_id3,
                    (order3_origin.clone(), OrderAmount::Base(balance!(40)))
                ),
                (
                    order_id4,
                    (order4_origin.clone(), OrderAmount::Base(balance!(50)))
                ),
                (
                    order_id5,
                    (order5_origin.clone(), OrderAmount::Base(balance!(60)))
                ),
            ]),
            to_full_execute: BTreeMap::from([
                (order_id1, order1_origin.clone()),
                (order_id2, order2_origin.clone()),
                (order_id3, order3_origin.clone()),
                (order_id4, order4_origin.clone()),
                (order_id5, order5_origin.clone()),
            ]),
            to_cancel: BTreeMap::from([
                (order_id1, order1_origin.clone()),
                (order_id2, order2_origin.clone()),
                (order_id3, order3_origin.clone()),
                (order_id4, order4_origin.clone()),
                (order_id5, order5_origin.clone()),
            ]),
            payment: payment.clone(),
            ignore_unschedule_error: false
        }
    );

    let partial_match = MarketChange {
        deal_input: Some(OrderAmount::Base(balance!(7000))),
        deal_output: Some(OrderAmount::Quote(balance!(8000))),
        market_input: None,
        market_output: None,
        to_place: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_origin.clone()),
            (order_id5, order5_origin.clone()),
        ]),
        to_part_execute: BTreeMap::from([
            (
                order_id1,
                (order1_other.clone(), OrderAmount::Base(balance!(120))),
            ),
            (
                order_id2,
                (order2_origin.clone(), OrderAmount::Base(balance!(30))),
            ),
            (
                order_id5,
                (order5_origin.clone(), OrderAmount::Base(balance!(60))),
            ),
        ]),
        to_full_execute: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_origin.clone()),
            (order_id5, order5_origin.clone()),
        ]),
        to_cancel: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_origin.clone()),
            (order_id5, order5_origin.clone()),
        ]),
        payment: empty_payment.clone(),
        ignore_unschedule_error: false,
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
            to_place: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_origin.clone()),
                (order_id3, order3_origin.clone()),
                (order_id5, order5_origin.clone()),
            ]),
            to_part_execute: BTreeMap::from([
                (
                    order_id1,
                    (order1_other.clone(), OrderAmount::Base(balance!(120)))
                ),
                (
                    order_id2,
                    (order2_origin.clone(), OrderAmount::Base(balance!(30)))
                ),
                (
                    order_id3,
                    (order3_origin.clone(), OrderAmount::Base(balance!(40)))
                ),
                (
                    order_id5,
                    (order5_origin.clone(), OrderAmount::Base(balance!(60)))
                ),
            ]),
            to_full_execute: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_origin.clone()),
                (order_id3, order3_origin.clone()),
                (order_id5, order5_origin.clone()),
            ]),
            to_cancel: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_origin.clone()),
                (order_id3, order3_origin.clone()),
                (order_id5, order5_origin.clone()),
            ]),
            payment: payment.clone(),
            ignore_unschedule_error: false
        }
    );

    let full_match = MarketChange {
        deal_input: Some(OrderAmount::Base(balance!(1000))),
        deal_output: Some(OrderAmount::Quote(balance!(2000))),
        market_input: Some(OrderAmount::Base(balance!(3000))),
        market_output: Some(OrderAmount::Quote(balance!(4000))),
        to_place: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_other.clone()),
            (order_id3, order3_other.clone()),
        ]),
        to_part_execute: BTreeMap::from([
            (
                order_id1,
                (order1_other.clone(), OrderAmount::Base(balance!(120))),
            ),
            (
                order_id2,
                (order2_other.clone(), OrderAmount::Base(balance!(130))),
            ),
            (
                order_id3,
                (order3_other.clone(), OrderAmount::Base(balance!(140))),
            ),
        ]),
        to_full_execute: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_other.clone()),
            (order_id3, order3_other.clone()),
        ]),
        to_cancel: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_other.clone()),
            (order_id3, order3_other.clone()),
        ]),
        payment: empty_payment.clone(),
        ignore_unschedule_error: false,
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
            to_place: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_other.clone()),
                (order_id3, order3_other.clone()),
            ]),
            to_part_execute: BTreeMap::from([
                (
                    order_id1,
                    (order1_other.clone(), OrderAmount::Base(balance!(120)))
                ),
                (
                    order_id2,
                    (order2_other.clone(), OrderAmount::Base(balance!(130)))
                ),
                (
                    order_id3,
                    (order3_other.clone(), OrderAmount::Base(balance!(140)))
                ),
            ]),
            to_full_execute: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_other.clone()),
                (order_id3, order3_other.clone()),
            ]),
            to_cancel: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_other.clone()),
                (order_id3, order3_other.clone()),
            ]),
            payment: payment.clone(),
            ignore_unschedule_error: false
        }
    );

    let empty = MarketChange {
        deal_input: None,
        deal_output: None,
        market_input: None,
        market_output: None,
        to_place: BTreeMap::new(),
        to_part_execute: BTreeMap::new(),
        to_full_execute: BTreeMap::new(),
        to_cancel: BTreeMap::new(),
        payment: empty_payment.clone(),
        ignore_unschedule_error: false,
    };

    market_change = origin.clone();
    assert_ok!(market_change.merge(empty));
    assert_eq!(market_change, origin);
}
