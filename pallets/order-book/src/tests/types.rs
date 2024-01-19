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
use common::{balance, PriceVariant, DAI, VAL, XOR};
use frame_support::assert_ok;
use framenode_chain_spec::ext;
use framenode_runtime::order_book::{
    CancelReason, Config, DealInfo, LimitOrder, MarketChange, OrderAmount, OrderBookId,
    OrderVolume, Payment,
};
use framenode_runtime::Runtime;
use sp_std::collections::btree_map::BTreeMap;

#[test]
fn check_order_amount() {
    let base_balance = balance!(10).into();
    let quote_balance = balance!(11).into();

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
        base.copy_type(balance!(100).into()),
        OrderAmount::Base(balance!(100).into())
    );
    assert_eq!(
        quote.copy_type(balance!(110).into()),
        OrderAmount::Quote(balance!(110).into())
    );

    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL,
        quote: XOR,
    };

    assert_eq!(*base.associated_asset(&order_book_id), VAL);
    assert_eq!(*quote.associated_asset(&order_book_id), XOR);

    let base_balance2 = balance!(5).into();
    let quote_balance2 = balance!(6).into();

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
    assert_eq!(base + quote, None);
    assert_eq!(quote + base, None);

    assert_eq!(
        (base - base2).unwrap(),
        OrderAmount::Base(base_balance - base_balance2)
    );
    assert_eq!(
        (quote - quote2).unwrap(),
        OrderAmount::Quote(quote_balance - quote_balance2)
    );
    assert_eq!(base - quote, None);
    assert_eq!(quote - base, None);
}

#[test]
fn check_deal_info_valid() {
    // zero input amount
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(0).into()),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(0).into()),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // zero output amount
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1).into()),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(0).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1).into()),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(0).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // zero average price
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1).into()),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2).into()),
        average_price: balance!(0).into(),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1).into()),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2).into()),
        average_price: balance!(0).into(),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // equal assets
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1).into()),
        output_asset_id: XOR,
        output_amount: OrderAmount::Base(balance!(2).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1).into()),
        output_asset_id: VAL,
        output_amount: OrderAmount::Quote(balance!(2).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // both are base
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Base(balance!(1).into()),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1).into()),
        output_asset_id: XOR,
        output_amount: OrderAmount::Base(balance!(2).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // both are quote
    assert!(!DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1).into()),
        output_asset_id: VAL,
        output_amount: OrderAmount::Quote(balance!(2).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(!DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Quote(balance!(1).into()),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Sell
    }
    .is_valid());

    // valid
    assert!(DealInfo {
        input_asset_id: XOR,
        input_amount: OrderAmount::Quote(balance!(1).into()),
        output_asset_id: VAL,
        output_amount: OrderAmount::Base(balance!(2).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Buy
    }
    .is_valid());

    assert!(DealInfo {
        input_asset_id: VAL,
        input_amount: OrderAmount::Base(balance!(1).into()),
        output_asset_id: XOR,
        output_amount: OrderAmount::Quote(balance!(2).into()),
        average_price: balance!(0.5).into(),
        direction: PriceVariant::Sell
    }
    .is_valid());
}

#[test]
fn check_deal_info_amounts() {
    assert_eq!(
        DealInfo {
            input_asset_id: XOR,
            input_amount: OrderAmount::Quote(balance!(1).into()),
            output_asset_id: VAL,
            output_amount: OrderAmount::Base(balance!(2).into()),
            average_price: balance!(0.5).into(),
            direction: PriceVariant::Buy
        }
        .base_amount(),
        balance!(2).into()
    );

    assert_eq!(
        DealInfo {
            input_asset_id: VAL,
            input_amount: OrderAmount::Base(balance!(1).into()),
            output_asset_id: XOR,
            output_amount: OrderAmount::Quote(balance!(2).into()),
            average_price: balance!(0.5).into(),
            direction: PriceVariant::Sell
        }
        .base_amount(),
        balance!(1).into()
    );

    assert_eq!(
        DealInfo {
            input_asset_id: XOR,
            input_amount: OrderAmount::Quote(balance!(1).into()),
            output_asset_id: VAL,
            output_amount: OrderAmount::Base(balance!(2).into()),
            average_price: balance!(0.5).into(),
            direction: PriceVariant::Buy
        }
        .quote_amount(),
        balance!(1).into()
    );

    assert_eq!(
        DealInfo {
            input_asset_id: VAL,
            input_amount: OrderAmount::Base(balance!(1).into()),
            output_asset_id: XOR,
            output_amount: OrderAmount::Quote(balance!(2).into()),
            average_price: balance!(0.5).into(),
            direction: PriceVariant::Sell
        }
        .quote_amount(),
        balance!(2).into()
    );
}

#[test]
fn should_fail_payment_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL,
        quote: XOR,
    };

    let other_order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: DAI,
        quote: XOR,
    };

    assert_eq!(
        Payment {
            order_book_id,
            to_lock: BTreeMap::from([(
                XOR,
                BTreeMap::from([(accounts::alice::<Runtime>(), balance!(100).into())])
            )]),
            to_unlock: BTreeMap::from([(
                VAL,
                BTreeMap::from([(accounts::bob::<Runtime>(), balance!(50).into())])
            )])
        }
        .merge(&Payment {
            order_book_id: other_order_book_id,
            to_lock: BTreeMap::from([(
                XOR,
                BTreeMap::from([(accounts::alice::<Runtime>(), balance!(100).into())])
            )]),
            to_unlock: BTreeMap::from([(
                DAI,
                BTreeMap::from([(accounts::bob::<Runtime>(), balance!(50).into())])
            )])
        }),
        None
    );
}

#[test]
fn check_payment_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL,
        quote: XOR,
    };

    let origin = Payment {
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([
                    (accounts::alice::<Runtime>(), balance!(10).into()),
                    (accounts::bob::<Runtime>(), balance!(20).into()),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (accounts::alice::<Runtime>(), balance!(30).into()),
                    (accounts::charlie::<Runtime>(), balance!(40).into()),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(50).into()),
                    (accounts::charlie::<Runtime>(), balance!(60).into()),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(70).into()),
                    (accounts::dave::<Runtime>(), balance!(80).into()),
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
                    (accounts::charlie::<Runtime>(), balance!(100).into()),
                    (accounts::dave::<Runtime>(), balance!(110).into()),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(120).into()),
                    (accounts::dave::<Runtime>(), balance!(130).into()),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (accounts::alice::<Runtime>(), balance!(140).into()),
                    (accounts::dave::<Runtime>(), balance!(150).into()),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (accounts::alice::<Runtime>(), balance!(160).into()),
                    (accounts::charlie::<Runtime>(), balance!(170).into()),
                ]),
            ),
        ]),
    };

    let mut payment = origin.clone();
    assert_eq!(payment.merge(&different), Some(()));
    assert_eq!(
        payment,
        Payment {
            order_book_id,
            to_lock: BTreeMap::from([
                (
                    XOR,
                    BTreeMap::from([
                        (accounts::alice::<Runtime>(), balance!(10).into()),
                        (accounts::bob::<Runtime>(), balance!(20).into()),
                        (accounts::charlie::<Runtime>(), balance!(100).into()),
                        (accounts::dave::<Runtime>(), balance!(110).into())
                    ]),
                ),
                (
                    VAL,
                    BTreeMap::from([
                        (accounts::alice::<Runtime>(), balance!(30).into()),
                        (accounts::bob::<Runtime>(), balance!(120).into()),
                        (accounts::charlie::<Runtime>(), balance!(40).into()),
                        (accounts::dave::<Runtime>(), balance!(130).into())
                    ]),
                ),
            ]),
            to_unlock: BTreeMap::from([
                (
                    VAL,
                    BTreeMap::from([
                        (accounts::alice::<Runtime>(), balance!(140).into()),
                        (accounts::bob::<Runtime>(), balance!(50).into()),
                        (accounts::charlie::<Runtime>(), balance!(60).into()),
                        (accounts::dave::<Runtime>(), balance!(150).into())
                    ]),
                ),
                (
                    XOR,
                    BTreeMap::from([
                        (accounts::alice::<Runtime>(), balance!(160).into()),
                        (accounts::bob::<Runtime>(), balance!(70).into()),
                        (accounts::charlie::<Runtime>(), balance!(170).into()),
                        (accounts::dave::<Runtime>(), balance!(80).into())
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
                    (accounts::alice::<Runtime>(), balance!(200).into()),
                    (accounts::charlie::<Runtime>(), balance!(210).into()),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(220).into()),
                    (accounts::charlie::<Runtime>(), balance!(230).into()),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(240).into()),
                    (accounts::dave::<Runtime>(), balance!(250).into()),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (accounts::alice::<Runtime>(), balance!(260).into()),
                    (accounts::dave::<Runtime>(), balance!(270).into()),
                ]),
            ),
        ]),
    };

    payment = origin.clone();
    assert_eq!(payment.merge(&partial_match), Some(()));
    assert_eq!(
        payment,
        Payment {
            order_book_id,
            to_lock: BTreeMap::from([
                (
                    XOR,
                    BTreeMap::from([
                        (accounts::alice::<Runtime>(), balance!(210).into()),
                        (accounts::bob::<Runtime>(), balance!(20).into()),
                        (accounts::charlie::<Runtime>(), balance!(210).into())
                    ]),
                ),
                (
                    VAL,
                    BTreeMap::from([
                        (accounts::alice::<Runtime>(), balance!(30).into()),
                        (accounts::bob::<Runtime>(), balance!(220).into()),
                        (accounts::charlie::<Runtime>(), balance!(270).into())
                    ]),
                ),
            ]),
            to_unlock: BTreeMap::from([
                (
                    VAL,
                    BTreeMap::from([
                        (accounts::bob::<Runtime>(), balance!(290).into()),
                        (accounts::charlie::<Runtime>(), balance!(60).into()),
                        (accounts::dave::<Runtime>(), balance!(250).into())
                    ]),
                ),
                (
                    XOR,
                    BTreeMap::from([
                        (accounts::alice::<Runtime>(), balance!(260).into()),
                        (accounts::bob::<Runtime>(), balance!(70).into()),
                        (accounts::dave::<Runtime>(), balance!(350).into())
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
                    (accounts::alice::<Runtime>(), balance!(300).into()),
                    (accounts::bob::<Runtime>(), balance!(310).into()),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (accounts::alice::<Runtime>(), balance!(320).into()),
                    (accounts::charlie::<Runtime>(), balance!(330).into()),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(340).into()),
                    (accounts::charlie::<Runtime>(), balance!(350).into()),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(360).into()),
                    (accounts::dave::<Runtime>(), balance!(370).into()),
                ]),
            ),
        ]),
    };

    payment = origin.clone();
    assert_eq!(payment.merge(&full_match), Some(()));
    assert_eq!(
        payment,
        Payment {
            order_book_id,
            to_lock: BTreeMap::from([
                (
                    XOR,
                    BTreeMap::from([
                        (accounts::alice::<Runtime>(), balance!(310).into()),
                        (accounts::bob::<Runtime>(), balance!(330).into())
                    ]),
                ),
                (
                    VAL,
                    BTreeMap::from([
                        (accounts::alice::<Runtime>(), balance!(350).into()),
                        (accounts::charlie::<Runtime>(), balance!(370).into())
                    ]),
                ),
            ]),
            to_unlock: BTreeMap::from([
                (
                    VAL,
                    BTreeMap::from([
                        (accounts::bob::<Runtime>(), balance!(390).into()),
                        (accounts::charlie::<Runtime>(), balance!(410).into())
                    ]),
                ),
                (
                    XOR,
                    BTreeMap::from([
                        (accounts::bob::<Runtime>(), balance!(430).into()),
                        (accounts::dave::<Runtime>(), balance!(450).into())
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
    assert_eq!(payment.merge(&empty), Some(()));
    assert_eq!(payment, origin);
}

#[test]
fn check_payment_execute_all() {
    ext().execute_with(|| {
        let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
            dex_id: DEX.into(),
            base: VAL,
            quote: XOR,
        };

        OrderBookPallet::register_tech_account(order_book_id).unwrap();

        fill_balance::<Runtime>(accounts::alice::<Runtime>(), order_book_id);
        fill_balance::<Runtime>(accounts::bob::<Runtime>(), order_book_id);
        fill_balance::<Runtime>(accounts::charlie::<Runtime>(), order_book_id);
        fill_balance::<Runtime>(accounts::dave::<Runtime>(), order_book_id);

        let balance_diff = balance!(150);

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

        let payment = Payment {
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
        };

        assert_ok!(payment.execute_all::<OrderBookPallet, OrderBookPallet>());

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
    });
}

#[test]
fn should_fail_market_change_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL,
        quote: XOR,
    };

    let payment = Payment {
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([
                    (accounts::alice::<Runtime>(), balance!(10).into()),
                    (accounts::bob::<Runtime>(), balance!(20).into()),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (accounts::alice::<Runtime>(), balance!(30).into()),
                    (accounts::charlie::<Runtime>(), balance!(40).into()),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(50).into()),
                    (accounts::charlie::<Runtime>(), balance!(60).into()),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(70).into()),
                    (accounts::dave::<Runtime>(), balance!(80).into()),
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
            5,
            LimitOrder::<Runtime>::new(
                5,
                accounts::alice::<Runtime>(),
                PriceVariant::Buy,
                balance!(10).into(),
                balance!(100).into(),
                1000,
                <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
                100,
            ),
        )]),
        to_part_execute: BTreeMap::from([(
            4,
            (
                LimitOrder::<Runtime>::new(
                    4,
                    accounts::alice::<Runtime>(),
                    PriceVariant::Buy,
                    balance!(20).into(),
                    balance!(100).into(),
                    1000,
                    <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
                    100,
                ),
                OrderAmount::Base(balance!(10).into()),
            ),
        )]),
        to_full_execute: BTreeMap::from([(
            3,
            LimitOrder::<Runtime>::new(
                3,
                accounts::alice::<Runtime>(),
                PriceVariant::Buy,
                balance!(20).into(),
                balance!(100).into(),
                1000,
                <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
                100,
            ),
        )]),
        to_cancel: BTreeMap::from([(
            2,
            (
                LimitOrder::<Runtime>::new(
                    2,
                    accounts::alice::<Runtime>(),
                    PriceVariant::Buy,
                    balance!(10).into(),
                    balance!(100).into(),
                    1000,
                    <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
                    100,
                ),
                CancelReason::Manual,
            ),
        )]),
        to_force_update: BTreeMap::from([(
            1,
            LimitOrder::<Runtime>::new(
                1,
                accounts::alice::<Runtime>(),
                PriceVariant::Buy,
                balance!(10).into(),
                balance!(100).into(),
                1000,
                <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
                100,
            ),
        )]),
        payment,
        ignore_unschedule_error: false,
    };

    let mut market_change = origin.clone();
    market_change.deal_input = Some(OrderAmount::Base(balance!(100).into()));
    market_change.deal_output = Some(OrderAmount::Quote(balance!(200).into()));
    market_change.market_input = Some(OrderAmount::Base(balance!(300).into()));
    market_change.market_output = Some(OrderAmount::Quote(balance!(400).into()));

    let mut diff_deal_input = origin.clone();
    diff_deal_input.deal_input = Some(OrderAmount::Quote(balance!(50).into()));
    assert_eq!(market_change.merge(diff_deal_input), None);

    let mut diff_deal_output = origin.clone();
    diff_deal_output.deal_output = Some(OrderAmount::Base(balance!(50).into()));
    assert_eq!(market_change.merge(diff_deal_output), None);

    let mut diff_market_input = origin.clone();
    diff_market_input.market_input = Some(OrderAmount::Quote(balance!(50).into()));
    assert_eq!(market_change.merge(diff_market_input), None);

    let mut diff_market_output = origin;
    diff_market_output.market_output = Some(OrderAmount::Base(balance!(50).into()));
    assert_eq!(market_change.merge(diff_market_output), None);
}

#[test]
fn check_market_change_merge() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL,
        quote: XOR,
    };

    let payment = Payment {
        order_book_id,
        to_lock: BTreeMap::from([
            (
                XOR,
                BTreeMap::from([
                    (accounts::alice::<Runtime>(), balance!(10).into()),
                    (accounts::bob::<Runtime>(), balance!(20).into()),
                ]),
            ),
            (
                VAL,
                BTreeMap::from([
                    (accounts::alice::<Runtime>(), balance!(30).into()),
                    (accounts::charlie::<Runtime>(), balance!(40).into()),
                ]),
            ),
        ]),
        to_unlock: BTreeMap::from([
            (
                VAL,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(50).into()),
                    (accounts::charlie::<Runtime>(), balance!(60).into()),
                ]),
            ),
            (
                XOR,
                BTreeMap::from([
                    (accounts::bob::<Runtime>(), balance!(70).into()),
                    (accounts::dave::<Runtime>(), balance!(80).into()),
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
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        balance!(10).into(),
        balance!(100).into(),
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        100,
    );

    let order1_other = LimitOrder::<Runtime>::new(
        order_id1,
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        balance!(9).into(),
        balance!(1000).into(),
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        100,
    );

    let order2_origin = LimitOrder::<Runtime>::new(
        order_id2,
        accounts::bob::<Runtime>(),
        PriceVariant::Sell,
        balance!(15).into(),
        balance!(100).into(),
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        100,
    );

    let order2_other = LimitOrder::<Runtime>::new(
        order_id2,
        accounts::bob::<Runtime>(),
        PriceVariant::Buy,
        balance!(14).into(),
        balance!(200).into(),
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        100,
    );

    let order3_origin = LimitOrder::<Runtime>::new(
        order_id3,
        accounts::charlie::<Runtime>(),
        PriceVariant::Buy,
        balance!(11).into(),
        balance!(100).into(),
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        100,
    );

    let order3_other = LimitOrder::<Runtime>::new(
        order_id3,
        accounts::charlie::<Runtime>(),
        PriceVariant::Buy,
        balance!(12).into(),
        balance!(1000).into(),
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        100,
    );

    let order4_origin = LimitOrder::<Runtime>::new(
        order_id4,
        accounts::dave::<Runtime>(),
        PriceVariant::Sell,
        balance!(16).into(),
        balance!(100).into(),
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        100,
    );

    let order5_origin = LimitOrder::<Runtime>::new(
        order_id5,
        accounts::alice::<Runtime>(),
        PriceVariant::Buy,
        balance!(12).into(),
        balance!(100).into(),
        1000,
        <Runtime as Config>::MIN_ORDER_LIFESPAN + 10000,
        100,
    );

    let origin = MarketChange {
        deal_input: Some(OrderAmount::Base(balance!(1000).into())),
        deal_output: Some(OrderAmount::Quote(balance!(2000).into())),
        market_input: Some(OrderAmount::Base(balance!(3000).into())),
        market_output: Some(OrderAmount::Quote(balance!(4000).into())),
        to_place: BTreeMap::from([
            (order_id1, order1_origin.clone()),
            (order_id2, order2_origin.clone()),
            (order_id3, order3_origin.clone()),
        ]),
        to_part_execute: BTreeMap::from([
            (
                order_id1,
                (
                    order1_origin.clone(),
                    OrderAmount::Base(balance!(20).into()),
                ),
            ),
            (
                order_id2,
                (
                    order2_origin.clone(),
                    OrderAmount::Base(balance!(30).into()),
                ),
            ),
            (
                order_id3,
                (
                    order3_origin.clone(),
                    OrderAmount::Base(balance!(40).into()),
                ),
            ),
        ]),
        to_full_execute: BTreeMap::from([
            (order_id1, order1_origin.clone()),
            (order_id2, order2_origin.clone()),
            (order_id3, order3_origin.clone()),
        ]),
        to_cancel: BTreeMap::from([
            (order_id1, (order1_origin.clone(), CancelReason::Manual)),
            (order_id2, (order2_origin.clone(), CancelReason::Manual)),
            (order_id3, (order3_origin.clone(), CancelReason::Manual)),
        ]),
        to_force_update: BTreeMap::from([
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
                (
                    order4_origin.clone(),
                    OrderAmount::Base(balance!(50).into()),
                ),
            ),
            (
                order_id5,
                (
                    order5_origin.clone(),
                    OrderAmount::Base(balance!(60).into()),
                ),
            ),
        ]),
        to_full_execute: BTreeMap::from([
            (order_id4, order4_origin.clone()),
            (order_id5, order5_origin.clone()),
        ]),
        to_cancel: BTreeMap::from([
            (order_id4, (order4_origin.clone(), CancelReason::Manual)),
            (order_id5, (order5_origin.clone(), CancelReason::Manual)),
        ]),
        to_force_update: BTreeMap::from([
            (order_id4, order4_origin.clone()),
            (order_id5, order5_origin.clone()),
        ]),
        payment: empty_payment.clone(),
        ignore_unschedule_error: false,
    };

    let mut market_change = origin.clone();
    assert_eq!(market_change.merge(different), Some(()));
    assert_eq!(
        market_change,
        MarketChange {
            deal_input: Some(OrderAmount::Base(balance!(1000).into())),
            deal_output: Some(OrderAmount::Quote(balance!(2000).into())),
            market_input: Some(OrderAmount::Base(balance!(3000).into())),
            market_output: Some(OrderAmount::Quote(balance!(4000).into())),
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
                    (
                        order1_origin.clone(),
                        OrderAmount::Base(balance!(20).into())
                    )
                ),
                (
                    order_id2,
                    (
                        order2_origin.clone(),
                        OrderAmount::Base(balance!(30).into())
                    )
                ),
                (
                    order_id3,
                    (
                        order3_origin.clone(),
                        OrderAmount::Base(balance!(40).into())
                    )
                ),
                (
                    order_id4,
                    (
                        order4_origin.clone(),
                        OrderAmount::Base(balance!(50).into())
                    )
                ),
                (
                    order_id5,
                    (
                        order5_origin.clone(),
                        OrderAmount::Base(balance!(60).into())
                    )
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
                (order_id1, (order1_origin.clone(), CancelReason::Manual)),
                (order_id2, (order2_origin.clone(), CancelReason::Manual)),
                (order_id3, (order3_origin.clone(), CancelReason::Manual)),
                (order_id4, (order4_origin.clone(), CancelReason::Manual)),
                (order_id5, (order5_origin.clone(), CancelReason::Manual)),
            ]),
            to_force_update: BTreeMap::from([
                (order_id1, order1_origin.clone()),
                (order_id2, order2_origin.clone()),
                (order_id3, order3_origin.clone()),
                (order_id4, order4_origin),
                (order_id5, order5_origin.clone()),
            ]),
            payment: payment.clone(),
            ignore_unschedule_error: false
        }
    );

    let partial_match = MarketChange {
        deal_input: Some(OrderAmount::Base(balance!(7000).into())),
        deal_output: Some(OrderAmount::Quote(balance!(8000).into())),
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
                (
                    order1_other.clone(),
                    OrderAmount::Base(balance!(120).into()),
                ),
            ),
            (
                order_id2,
                (
                    order2_origin.clone(),
                    OrderAmount::Base(balance!(30).into()),
                ),
            ),
            (
                order_id5,
                (
                    order5_origin.clone(),
                    OrderAmount::Base(balance!(60).into()),
                ),
            ),
        ]),
        to_full_execute: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_origin.clone()),
            (order_id5, order5_origin.clone()),
        ]),
        to_cancel: BTreeMap::from([
            (order_id1, (order1_origin.clone(), CancelReason::Manual)),
            (order_id2, (order2_origin.clone(), CancelReason::Manual)),
            (order_id5, (order5_origin.clone(), CancelReason::Manual)),
        ]),
        to_force_update: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_origin.clone()),
            (order_id5, order5_origin.clone()),
        ]),
        payment: empty_payment.clone(),
        ignore_unschedule_error: false,
    };

    market_change = origin.clone();
    assert_eq!(market_change.merge(partial_match), Some(()));
    assert_eq!(
        market_change,
        MarketChange {
            deal_input: Some(OrderAmount::Base(balance!(8000).into())),
            deal_output: Some(OrderAmount::Quote(balance!(10000).into())),
            market_input: Some(OrderAmount::Base(balance!(3000).into())),
            market_output: Some(OrderAmount::Quote(balance!(4000).into())),
            to_place: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_origin.clone()),
                (order_id3, order3_origin.clone()),
                (order_id5, order5_origin.clone()),
            ]),
            to_part_execute: BTreeMap::from([
                (
                    order_id1,
                    (
                        order1_other.clone(),
                        OrderAmount::Base(balance!(120).into())
                    )
                ),
                (
                    order_id2,
                    (
                        order2_origin.clone(),
                        OrderAmount::Base(balance!(30).into())
                    )
                ),
                (
                    order_id3,
                    (
                        order3_origin.clone(),
                        OrderAmount::Base(balance!(40).into())
                    )
                ),
                (
                    order_id5,
                    (
                        order5_origin.clone(),
                        OrderAmount::Base(balance!(60).into())
                    )
                ),
            ]),
            to_full_execute: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_origin.clone()),
                (order_id3, order3_origin.clone()),
                (order_id5, order5_origin.clone()),
            ]),
            to_cancel: BTreeMap::from([
                (order_id1, (order1_origin, CancelReason::Manual)),
                (order_id2, (order2_origin.clone(), CancelReason::Manual)),
                (order_id3, (order3_origin.clone(), CancelReason::Manual)),
                (order_id5, (order5_origin.clone(), CancelReason::Manual)),
            ]),
            to_force_update: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_origin),
                (order_id3, order3_origin),
                (order_id5, order5_origin),
            ]),
            payment: payment.clone(),
            ignore_unschedule_error: false
        }
    );

    let full_match = MarketChange {
        deal_input: Some(OrderAmount::Base(balance!(1000).into())),
        deal_output: Some(OrderAmount::Quote(balance!(2000).into())),
        market_input: Some(OrderAmount::Base(balance!(3000).into())),
        market_output: Some(OrderAmount::Quote(balance!(4000).into())),
        to_place: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_other.clone()),
            (order_id3, order3_other.clone()),
        ]),
        to_part_execute: BTreeMap::from([
            (
                order_id1,
                (
                    order1_other.clone(),
                    OrderAmount::Base(balance!(120).into()),
                ),
            ),
            (
                order_id2,
                (
                    order2_other.clone(),
                    OrderAmount::Base(balance!(130).into()),
                ),
            ),
            (
                order_id3,
                (
                    order3_other.clone(),
                    OrderAmount::Base(balance!(140).into()),
                ),
            ),
        ]),
        to_full_execute: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_other.clone()),
            (order_id3, order3_other.clone()),
        ]),
        to_cancel: BTreeMap::from([
            (order_id1, (order1_other.clone(), CancelReason::Manual)),
            (order_id2, (order2_other.clone(), CancelReason::Manual)),
            (order_id3, (order3_other.clone(), CancelReason::Manual)),
        ]),
        to_force_update: BTreeMap::from([
            (order_id1, order1_other.clone()),
            (order_id2, order2_other.clone()),
            (order_id3, order3_other.clone()),
        ]),
        payment: empty_payment.clone(),
        ignore_unschedule_error: false,
    };

    market_change = origin.clone();
    assert_eq!(market_change.merge(full_match), Some(()));
    assert_eq!(
        market_change,
        MarketChange {
            deal_input: Some(OrderAmount::Base(balance!(2000).into())),
            deal_output: Some(OrderAmount::Quote(balance!(4000).into())),
            market_input: Some(OrderAmount::Base(balance!(6000).into())),
            market_output: Some(OrderAmount::Quote(balance!(8000).into())),
            to_place: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_other.clone()),
                (order_id3, order3_other.clone()),
            ]),
            to_part_execute: BTreeMap::from([
                (
                    order_id1,
                    (
                        order1_other.clone(),
                        OrderAmount::Base(balance!(120).into())
                    )
                ),
                (
                    order_id2,
                    (
                        order2_other.clone(),
                        OrderAmount::Base(balance!(130).into())
                    )
                ),
                (
                    order_id3,
                    (
                        order3_other.clone(),
                        OrderAmount::Base(balance!(140).into())
                    )
                ),
            ]),
            to_full_execute: BTreeMap::from([
                (order_id1, order1_other.clone()),
                (order_id2, order2_other.clone()),
                (order_id3, order3_other.clone()),
            ]),
            to_cancel: BTreeMap::from([
                (order_id1, (order1_other.clone(), CancelReason::Manual)),
                (order_id2, (order2_other.clone(), CancelReason::Manual)),
                (order_id3, (order3_other.clone(), CancelReason::Manual)),
            ]),
            to_force_update: BTreeMap::from([
                (order_id1, order1_other),
                (order_id2, order2_other),
                (order_id3, order3_other),
            ]),
            payment,
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
        to_force_update: BTreeMap::new(),
        payment: empty_payment,
        ignore_unschedule_error: false,
    };

    market_change = origin.clone();
    assert_eq!(market_change.merge(empty), Some(()));
    assert_eq!(market_change, origin);
}

#[test]
fn check_market_change_count_of_executed_orders() {
    let order_book_id = OrderBookId::<AssetIdOf<Runtime>, DEXId> {
        dex_id: DEX.into(),
        base: VAL,
        quote: XOR,
    };

    let empty_payment =
        Payment::<AssetIdOf<Runtime>, <Runtime as frame_system::Config>::AccountId, DEXId> {
            order_book_id,
            to_lock: BTreeMap::<
                AssetIdOf<Runtime>,
                BTreeMap<<Runtime as frame_system::Config>::AccountId, OrderVolume>,
            >::new(),
            to_unlock: BTreeMap::<
                AssetIdOf<Runtime>,
                BTreeMap<<Runtime as frame_system::Config>::AccountId, OrderVolume>,
            >::new(),
        };

    assert_eq!(
        MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None,
            to_place: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            to_part_execute: BTreeMap::<
                <Runtime as Config>::OrderId,
                (LimitOrder::<Runtime>, OrderAmount),
            >::new(),
            to_full_execute: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            to_cancel: BTreeMap::<
                <Runtime as Config>::OrderId,
                (LimitOrder::<Runtime>, CancelReason),
            >::new(),
            to_force_update: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            payment: empty_payment.clone(),
            ignore_unschedule_error: false,
        }
        .count_of_executed_orders(),
        0
    );

    assert_eq!(
        MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None,
            to_place: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            to_part_execute: BTreeMap::from([(
                2,
                (
                    LimitOrder::<Runtime>::new(
                        2,
                        accounts::alice::<Runtime>(),
                        PriceVariant::Buy,
                        balance!(20).into(),
                        balance!(100).into(),
                        1000,
                        10000,
                        100,
                    ),
                    OrderAmount::Base(balance!(10).into()),
                ),
            )]),
            to_full_execute: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            to_cancel: BTreeMap::<
                <Runtime as Config>::OrderId,
                (LimitOrder::<Runtime>, CancelReason),
            >::new(),
            to_force_update: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            payment: empty_payment.clone(),
            ignore_unschedule_error: false,
        }
        .count_of_executed_orders(),
        1
    );

    assert_eq!(
        MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None,
            to_place: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            to_part_execute: BTreeMap::<
                <Runtime as Config>::OrderId,
                (LimitOrder::<Runtime>, OrderAmount),
            >::new(),
            to_full_execute: BTreeMap::from([(
                1,
                LimitOrder::<Runtime>::new(
                    1,
                    accounts::alice::<Runtime>(),
                    PriceVariant::Buy,
                    balance!(20).into(),
                    balance!(100).into(),
                    1000,
                    10000,
                    100,
                ),
            )]),
            to_cancel: BTreeMap::<
                <Runtime as Config>::OrderId,
                (LimitOrder::<Runtime>, CancelReason),
            >::new(),
            to_force_update: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            payment: empty_payment.clone(),
            ignore_unschedule_error: false,
        }
        .count_of_executed_orders(),
        1
    );

    assert_eq!(
        MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None,
            to_place: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            to_part_execute: BTreeMap::from([(
                2,
                (
                    LimitOrder::<Runtime>::new(
                        2,
                        accounts::alice::<Runtime>(),
                        PriceVariant::Buy,
                        balance!(20).into(),
                        balance!(100).into(),
                        1000,
                        10000,
                        100,
                    ),
                    OrderAmount::Base(balance!(10).into()),
                ),
            )]),
            to_full_execute: BTreeMap::from([(
                1,
                LimitOrder::<Runtime>::new(
                    1,
                    accounts::alice::<Runtime>(),
                    PriceVariant::Buy,
                    balance!(20).into(),
                    balance!(100).into(),
                    1000,
                    10000,
                    100,
                ),
            )]),
            to_cancel: BTreeMap::<
                <Runtime as Config>::OrderId,
                (LimitOrder::<Runtime>, CancelReason),
            >::new(),
            to_force_update: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            payment: empty_payment.clone(),
            ignore_unschedule_error: false,
        }
        .count_of_executed_orders(),
        2
    );

    assert_eq!(
        MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None,
            to_place: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            to_part_execute: BTreeMap::from([
                (
                    4,
                    (
                        LimitOrder::<Runtime>::new(
                            4,
                            accounts::alice::<Runtime>(),
                            PriceVariant::Buy,
                            balance!(20).into(),
                            balance!(100).into(),
                            1000,
                            10000,
                            100,
                        ),
                        OrderAmount::Base(balance!(10).into()),
                    ),
                ),
                (
                    5,
                    (
                        LimitOrder::<Runtime>::new(
                            5,
                            accounts::alice::<Runtime>(),
                            PriceVariant::Buy,
                            balance!(20).into(),
                            balance!(100).into(),
                            1000,
                            10000,
                            100,
                        ),
                        OrderAmount::Base(balance!(10).into()),
                    ),
                )
            ]),
            to_full_execute: BTreeMap::from([
                (
                    1,
                    LimitOrder::<Runtime>::new(
                        1,
                        accounts::alice::<Runtime>(),
                        PriceVariant::Buy,
                        balance!(20).into(),
                        balance!(100).into(),
                        1000,
                        10000,
                        100,
                    ),
                ),
                (
                    2,
                    LimitOrder::<Runtime>::new(
                        2,
                        accounts::alice::<Runtime>(),
                        PriceVariant::Buy,
                        balance!(20).into(),
                        balance!(100).into(),
                        1000,
                        10000,
                        100,
                    ),
                ),
                (
                    3,
                    LimitOrder::<Runtime>::new(
                        3,
                        accounts::alice::<Runtime>(),
                        PriceVariant::Buy,
                        balance!(20).into(),
                        balance!(100).into(),
                        1000,
                        10000,
                        100,
                    ),
                )
            ]),
            to_cancel: BTreeMap::<
                <Runtime as Config>::OrderId,
                (LimitOrder::<Runtime>, CancelReason),
            >::new(),
            to_force_update: BTreeMap::<<Runtime as Config>::OrderId, LimitOrder::<Runtime>>::new(),
            payment: empty_payment,
            ignore_unschedule_error: false,
        }
        .count_of_executed_orders(),
        5
    );
}
