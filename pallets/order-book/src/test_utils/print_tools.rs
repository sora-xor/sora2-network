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

use super::order_book_imported;

use common::prelude::FixedWrapper;
use common::{AssetIdOf, PriceVariant};
use order_book_imported::{
    Asks, Bids, Config, ExpirationsAgenda, LimitOrder, LimitOrders, OrderBookId, OrderPrice,
    OrderVolume, PriceOrders,
};
use sp_runtime::traits::{CheckedAdd, Zero};
use sp_runtime::BoundedVec;
use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

fn print_side<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    side: PriceVariant,
    column_width: usize,
) {
    let side_orders: Vec<(
        OrderPrice,
        PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>,
    )> = match side {
        PriceVariant::Buy => {
            let mut side_orders: Vec<_> = Bids::<T>::iter_prefix(order_book_id).collect();
            side_orders.sort_by_key(|value| value.0);
            side_orders.reverse();
            side_orders
        }
        PriceVariant::Sell => {
            let mut side_orders: Vec<_> = Asks::<T>::iter_prefix(order_book_id).collect();
            side_orders.sort_by_key(|value| value.0);
            side_orders
        }
    };
    let order_data: BTreeMap<T::OrderId, LimitOrder<T>> =
        LimitOrders::<T>::iter_prefix(order_book_id).collect();
    for (price, price_order_ids) in side_orders {
        let price_orders: Vec<_> = price_order_ids
            .iter()
            .map(|id| order_data.get(id).unwrap())
            .collect();
        let volume: OrderVolume = price_orders
            .iter()
            .map(|order| order.amount)
            .fold(OrderVolume::zero(), |acc, item| {
                acc.checked_add(&item).unwrap()
            });
        print!(
            "{:>1$} |",
            FixedWrapper::from(*price.balance())
                .get()
                .unwrap()
                .to_string(),
            column_width - 1
        );
        print!(
            "{:>1$} |",
            FixedWrapper::from(*volume.balance())
                .get()
                .unwrap()
                .to_string(),
            column_width - 1
        );
        println!(
            " {}",
            price_order_ids
                .iter()
                .fold("".to_owned(), |s, id| s + &id.to_string() + ", ")
        );
    }
}

/// Print in the following form:
/// ```text
/// price | volume | orders
///          Asks
///  11.5 |  255.8 | sell4, sell5, sell6
///  11.2 |  178.6 | sell2, sell3
///  11.0 |  176.3 | sell1
///  spread
///  10.0 |  168.5 | buy1
///   9.8 |  139.9 | buy2, buy3
///   9.5 |  261.3 | buy4, buy5, buy6
///          Bids
/// ```
pub fn pretty_print_order_book<T: Config>(
    order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    column_width: Option<usize>,
) {
    let column_width = column_width.unwrap_or(8);
    println!(
        "{0:>3$} |{1:>3$} |{2:>3$} ",
        "price",
        "volume",
        "orders",
        column_width - 1
    );
    println!("\tAsks");
    print_side::<T>(order_book_id, PriceVariant::Sell, column_width);
    println!(") spread");
    print_side::<T>(order_book_id, PriceVariant::Buy, column_width);
    println!("\tBids\n");
}

fn print_block_expirations<T: Config>(block: u32)
where
    T::BlockNumber: From<u32>,
{
    let block = T::BlockNumber::from(block);
    let expirations: BoundedVec<
        (OrderBookId<AssetIdOf<T>, T::DEXId>, T::OrderId),
        T::MaxExpiringOrdersPerBlock,
    > = ExpirationsAgenda::<T>::get(block);
    for (order_book_id, order_id) in expirations {
        println!(
            "{:>5} | base: {:?}; quote: {:?} |{:>4} ",
            block, order_book_id.base, order_book_id.quote, order_id
        );
    }
}

/// Print expirations agenda in the form:
///
/// ```text
/// block number | order book id | order id
/// ```
pub fn pretty_print_expirations<T: Config>(blocks: sp_std::ops::Range<u32>)
where
    T::BlockNumber: TryFrom<u32>,
{
    println!("block |{:>148} | order id", "order book id");
    for block in blocks {
        print_block_expirations::<T>(block)
    }
}
