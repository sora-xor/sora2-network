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

use crate::primitives::{AssetId32, Balance, DEXId, OrderBookId, OrderId, PriceVariant};
// use scale::Encode;
use sp_std::vec::Vec;
// use common::{AssetId32, Balance, PredefinedAssetId, PriceVariant};
// use crate::primitives::{OrderId, OrderBookId, DEXId};
/// It is a part of a pallet dispatchables API.
/// The indexes can be found in your pallet code's #[pallet::call] section and check #[pallet::call_index(x)] attribute of the call.
/// If these attributes are missing, use source-code order (0-based).
/// You may found list of callable extrinsic in `pallet_contracts::Config::CallFilter`
#[ink::scale_derive(Encode)]
pub enum OrderBookCall {
    /// Places the limit order into the order book
    /// `order_book::pallet::place_limit_order`
    #[codec(index = 4)]
    PlaceLimitOrder {
        order_book_id: OrderBookId<AssetId32, DEXId>,
        price: Balance,
        amount: Balance,
        side: PriceVariant,
        lifespan: u64,
    },
    /// Cancels the limit order
    /// `order_book::pallet::cancel_limit_order`
    #[codec(index = 5)]
    CancelLimitOrder {
        order_book_id: OrderBookId<AssetId32, DEXId>,
        order_id: OrderId,
    },
    /// Cancels the list of limit orders
    /// `order_book::pallet::cancel_limit_orders_batch`
    #[codec(index = 6)]
    CancelLimitOrdersBatch {
        limit_orders_to_cancel: Vec<(OrderBookId<AssetId32, DEXId>, Vec<OrderId>)>,
    },
    /// Executes the market order
    /// `order_book::pallet::execute_market_order`
    #[codec(index = 7)]
    ExecuteMarketOrder {
        order_book_id: OrderBookId<AssetId32, DEXId>,
        direction: PriceVariant,
        amount: Balance,
    },
}
