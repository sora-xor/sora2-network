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
use common::{balance, PriceVariant, VAL, XOR};
use framenode_runtime::order_book::{DealInfo, OrderAmount, OrderBookId};
use framenode_runtime::Runtime;

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

    let order_book_id = OrderBookId::<AssetIdOf<Runtime>> {
        base: VAL.into(),
        quote: XOR.into(),
    };

    assert_eq!(*base.associated_asset(&order_book_id), VAL);
    assert_eq!(*quote.associated_asset(&order_book_id), XOR);
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
