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

use codec::{Decode, Encode, MaxEncodedLen};
use common::{Balance, PriceVariant, TradingPair};
use frame_support::{BoundedBTreeMap, BoundedVec, RuntimeDebug};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type OrderPrice = Balance;
pub type OrderVolume = Balance;
pub type PriceOrders<OrderId, MaxLimitOrdersForPrice> = BoundedVec<OrderId, MaxLimitOrdersForPrice>;
pub type MarketSide<MaxSidePriceCount> =
    BoundedBTreeMap<OrderPrice, OrderVolume, MaxSidePriceCount>;
pub type UserOrders<OrderId, MaxOpenedLimitOrdersPerUser> =
    BoundedVec<OrderId, MaxOpenedLimitOrdersPerUser>;

#[derive(Eq, PartialEq, Clone, Copy, RuntimeDebug)]
pub enum OrderAmount {
    Base(OrderVolume),
    Quote(OrderVolume),
}

impl OrderAmount {
    pub fn value(&self) -> &OrderVolume {
        match self {
            OrderAmount::Base(value) => value,
            OrderAmount::Quote(value) => value,
        }
    }

    pub fn associated_asset<'a, AssetId>(
        &'a self,
        order_book_id: &'a OrderBookId<AssetId>,
    ) -> &AssetId {
        match self {
            OrderAmount::Base(..) => &order_book_id.base,
            OrderAmount::Quote(..) => &order_book_id.quote,
        }
    }
}

#[derive(Eq, PartialEq, Clone, Copy, RuntimeDebug)]
pub enum MarketRole {
    Maker,
    Taker,
}

#[derive(
    Encode,
    Decode,
    Eq,
    PartialEq,
    Copy,
    Clone,
    PartialOrd,
    Ord,
    RuntimeDebug,
    Hash,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OrderBookId<AssetId> {
    /// Base asset.
    pub base: AssetId,
    /// Quote asset. It should be a base asset of DEX.
    pub quote: AssetId,
}

impl<AssetId> From<TradingPair<AssetId>> for OrderBookId<AssetId> {
    fn from(trading_pair: TradingPair<AssetId>) -> Self {
        Self {
            base: trading_pair.target_asset_id,
            quote: trading_pair.base_asset_id,
        }
    }
}

impl<AssetId> From<OrderBookId<AssetId>> for TradingPair<AssetId> {
    fn from(order_book_id: OrderBookId<AssetId>) -> Self {
        Self {
            base_asset_id: order_book_id.quote,
            target_asset_id: order_book_id.base,
        }
    }
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct DealInfo<AssetId> {
    pub input_asset_id: AssetId,
    pub input_amount: OrderVolume,
    pub output_asset_id: AssetId,
    pub output_amount: OrderVolume,
    pub average_price: OrderPrice,
    pub side: PriceVariant,
}

#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct MarketChange<AccountId, OrderId, LimitOrder> {
    pub market_input: OrderAmount,
    pub market_output: OrderAmount,
    pub to_delete: Vec<OrderId>,
    pub to_update: Vec<LimitOrder>,
    pub makers_output: BTreeMap<AccountId, OrderVolume>,
}
