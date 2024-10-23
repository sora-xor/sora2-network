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

use crate::{
    Config, LimitOrder, MarketSide, OrderBookEvent, OrderBookId, OrderPrice, OrderVolume,
    PriceOrders, UserOrders,
};
use common::AssetIdOf;
use common::PriceVariant;
use frame_support::sp_runtime::DispatchError;
use frame_support::weights::WeightMeter;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

/// This trait is used by Order Book as a storage to get limit orders and their derived data and to change them
pub trait DataLayer<T>
where
    T: Config,
{
    /// Returns the limit order if exists, otherwise returns error.
    fn get_limit_order(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        order_id: T::OrderId,
    ) -> Result<LimitOrder<T>, DispatchError>;

    /// Returns all limit orders of order book
    fn get_all_limit_orders(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Vec<LimitOrder<T>>;

    /// Inserts limit order consistently in all necessary storages.
    /// Must check before call. If returns error, it means we have problems with data consistency.
    /// If order_id already exists - returns error. Use `update_limit_order` to update the existing order.
    fn insert_limit_order(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        order: LimitOrder<T>,
    ) -> Result<(), DispatchError>;

    /// Updates the amount of the limit order.
    /// If order doesn't exist - return error
    fn update_limit_order_amount(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        order_id: T::OrderId,
        new_amount: OrderVolume,
    ) -> Result<(), DispatchError>;

    /// Deletes limit order consistently from all necessary storages.
    /// If order doesn't exist - return error
    /// Must check before call. If returns error, it means we have problems with data consistency.
    fn delete_limit_order(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        order_id: T::OrderId,
    ) -> Result<(), DispatchError>;

    /// Returns order ids of orders inside the bid or ask price
    fn get_limit_orders_by_price(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        side: PriceVariant,
        price: &OrderPrice,
    ) -> Option<PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>> {
        match side {
            PriceVariant::Buy => self.get_bids(order_book_id, price),
            PriceVariant::Sell => self.get_asks(order_book_id, price),
        }
    }

    /// Returns order ids of orders inside the bid price
    fn get_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
    ) -> Option<PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>>;

    /// Returns whether there is no place for orders inside the bid price. None if no entry for
    /// the price exists
    fn is_bid_price_full(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
    ) -> Option<bool>;

    /// Returns order ids of orders inside the ask price
    fn get_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
    ) -> Option<PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>>;

    /// Returns whether there is no place for orders inside the ask price. None if no entry for
    /// the price exists
    fn is_ask_price_full(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
    ) -> Option<bool>;

    /// Returns all bid prices with their volumes
    fn get_aggregated_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> MarketSide<T::MaxSidePriceCount>;

    /// Length of aggregated asks list for the order book.
    /// `None` if the value is not present in the storage. Ignores default value, so `unwrap_or(0)`
    /// usually makes sense.
    fn get_aggregated_bids_len(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<usize>;

    fn best_bid(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<(OrderPrice, OrderVolume)>;

    /// Returns all ask prices with their volumes
    fn get_aggregated_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> MarketSide<T::MaxSidePriceCount>;

    /// Length of aggregated asks list for the order book.
    /// `None` if the value is not present in the storage. Ignores default value, so `unwrap_or(0)`
    /// usually makes sense.
    fn get_aggregated_asks_len(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<usize>;

    fn best_ask(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<(OrderPrice, OrderVolume)>;

    /// Returns order ids of user from the order book with `order_book_id`
    fn get_user_limit_orders(
        &mut self,
        account: &T::AccountId,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<UserOrders<T::OrderId, T::MaxOpenedLimitOrdersPerUser>>;

    /// Returns order ids of user from all order books
    fn get_all_user_limit_orders(
        &mut self,
        account: &T::AccountId,
    ) -> BTreeMap<
        OrderBookId<AssetIdOf<T>, T::DexId>,
        UserOrders<T::OrderId, T::MaxOpenedLimitOrdersPerUser>,
    >;

    /// Returns whether there is no place for the user's orders in the order book.
    /// `None` if there is no entry tracking orders of the user in the order book.
    fn is_user_limit_orders_full(
        &mut self,
        account: &T::AccountId,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<bool>;
}

pub trait CurrencyLocker<AccountId, AssetId, DexId, Error> {
    /// Lock `amount` of liquidity in `order_book_id`'s asset chosen by `asset`.
    /// The assets are taken from `account`.
    fn lock_liquidity(
        account: &AccountId,
        order_book_id: OrderBookId<AssetId, DexId>,
        asset_id: &AssetId,
        amount: OrderVolume,
    ) -> Result<(), Error>;
}

pub trait CurrencyUnlocker<AccountId, AssetId, DexId, Error> {
    /// Unlock `amount` of liquidity in `order_book_id`'s asset chosen by `asset`.
    /// The assets are taken from `account`.
    fn unlock_liquidity(
        account: &AccountId,
        order_book_id: OrderBookId<AssetId, DexId>,
        asset_id: &AssetId,
        amount: OrderVolume,
    ) -> Result<(), Error>;

    fn unlock_liquidity_batch(
        order_book_id: OrderBookId<AssetId, DexId>,
        asset_id: &AssetId,
        receivers: &BTreeMap<AccountId, OrderVolume>,
    ) -> Result<(), Error>;
}

pub trait ExpirationScheduler<BlockNumber, OrderBookId, DexId, OrderId, Error> {
    /// Execute scheduled expirations considering this block to be `current_block`
    /// and weight limit to be set by `weight`.
    ///
    /// If the weight limit is reached, it should continue where it's left at the
    /// next block.
    fn service_expiration(current_block: BlockNumber, weight: &mut WeightMeter);

    /// Schedule the order for expiration at block `when`.
    fn schedule_expiration(
        when: BlockNumber,
        order_book_id: OrderBookId,
        order_id: OrderId,
    ) -> Result<(), Error>;

    /// Remove the order from expiration schedule for block `when`.
    fn unschedule_expiration(
        when: BlockNumber,
        order_book_id: OrderBookId,
        order_id: OrderId,
    ) -> Result<(), Error>;
}

pub trait AlignmentScheduler {
    fn service_alignment(weight: &mut WeightMeter);
}

pub trait Delegate<AccountId, AssetId, OrderId, DexId, Moment> {
    fn emit_event(
        order_book_id: OrderBookId<AssetId, DexId>,
        event: OrderBookEvent<AccountId, OrderId, Moment>,
    );
}
