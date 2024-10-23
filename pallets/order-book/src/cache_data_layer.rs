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
    AggregatedAsks, AggregatedBids, Asks, Bids, Config, DataLayer, Error, LimitOrder, LimitOrders,
    MarketSide, OrderBookId, OrderPrice, OrderVolume, PriceOrders, UserLimitOrders, UserOrders,
};
use common::cache_storage::{CacheStorageDoubleMap, CacheStorageMap};
use common::AssetIdOf;
use common::PriceVariant;
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::Len;
use sp_runtime::traits::{CheckedAdd, CheckedSub, Zero};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

pub struct CacheDataLayer<T: Config> {
    limit_orders: CacheStorageDoubleMap<
        OrderBookId<AssetIdOf<T>, T::DexId>,
        T::OrderId,
        LimitOrder<T>,
        LimitOrders<T>,
    >,
    bids: CacheStorageDoubleMap<
        OrderBookId<AssetIdOf<T>, T::DexId>,
        OrderPrice,
        PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>,
        Bids<T>,
    >,
    asks: CacheStorageDoubleMap<
        OrderBookId<AssetIdOf<T>, T::DexId>,
        OrderPrice,
        PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>,
        Asks<T>,
    >,
    aggregated_bids: CacheStorageMap<
        OrderBookId<AssetIdOf<T>, T::DexId>,
        MarketSide<T::MaxSidePriceCount>,
        AggregatedBids<T>,
    >,
    aggregated_asks: CacheStorageMap<
        OrderBookId<AssetIdOf<T>, T::DexId>,
        MarketSide<T::MaxSidePriceCount>,
        AggregatedAsks<T>,
    >,
    user_limit_orders: CacheStorageDoubleMap<
        T::AccountId,
        OrderBookId<AssetIdOf<T>, T::DexId>,
        UserOrders<T::OrderId, T::MaxOpenedLimitOrdersPerUser>,
        UserLimitOrders<T>,
    >,
}

impl<T: Config> Default for CacheDataLayer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Config> CacheDataLayer<T> {
    pub fn new() -> Self {
        Self {
            limit_orders: CacheStorageDoubleMap::new(),
            bids: CacheStorageDoubleMap::new(),
            asks: CacheStorageDoubleMap::new(),
            aggregated_bids: CacheStorageMap::new(),
            aggregated_asks: CacheStorageMap::new(),
            user_limit_orders: CacheStorageDoubleMap::new(),
        }
    }

    pub fn commit(&mut self) {
        self.limit_orders.commit();
        self.bids.commit();
        self.asks.commit();
        self.aggregated_bids.commit();
        self.aggregated_asks.commit();
        self.user_limit_orders.commit();
    }

    pub fn reset(&mut self) {
        self.limit_orders.reset();
        self.bids.reset();
        self.asks.reset();
        self.aggregated_bids.reset();
        self.aggregated_asks.reset();
        self.user_limit_orders.reset();
    }

    fn add_to_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        limit_order: &LimitOrder<T>,
    ) -> Result<(), ()> {
        if let Some(bids) = self.bids.get_mut(order_book_id, &limit_order.price) {
            bids.try_push(limit_order.id).map_err(|_| ())?;
        } else {
            let bids = sp_runtime::BoundedVec::<T::OrderId, T::MaxLimitOrdersForPrice>::try_from(
                Vec::from([limit_order.id]),
            )
            .map_err(|_| ())?;
            self.bids.set(order_book_id, &limit_order.price, bids);
        }
        Ok(())
    }

    fn remove_from_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        limit_order: &LimitOrder<T>,
    ) {
        if let Some(bids) = self.bids.get_mut(order_book_id, &limit_order.price) {
            bids.retain(|x| *x != limit_order.id);
            if bids.is_empty() {
                self.bids.remove(order_book_id, &limit_order.price);
            }
        }
        // don't need to do anything if `bids` is empty
    }

    fn add_to_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        limit_order: &LimitOrder<T>,
    ) -> Result<(), ()> {
        if let Some(asks) = self.asks.get_mut(order_book_id, &limit_order.price) {
            asks.try_push(limit_order.id).map_err(|_| ())?;
        } else {
            let asks = sp_runtime::BoundedVec::<T::OrderId, T::MaxLimitOrdersForPrice>::try_from(
                Vec::from([limit_order.id]),
            )
            .map_err(|_| ())?;
            self.asks.set(order_book_id, &limit_order.price, asks);
        }
        Ok(())
    }

    fn remove_from_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        limit_order: &LimitOrder<T>,
    ) {
        if let Some(asks) = self.asks.get_mut(order_book_id, &limit_order.price) {
            asks.retain(|x| *x != limit_order.id);
            if asks.is_empty() {
                self.asks.remove(order_book_id, &limit_order.price);
            }
        }
        // don't need to do anything if `asks` is empty
    }

    fn add_to_aggregated_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
        value: &OrderVolume,
    ) -> Result<(), ()> {
        if let Some(agg_bids) = self.aggregated_bids.get_mut(order_book_id) {
            let volume = agg_bids
                .get(price)
                .copied()
                .unwrap_or_default()
                .checked_add(value)
                .ok_or(())?;
            agg_bids.try_insert(*price, volume).map_err(|_| ())?;
        } else {
            let agg_bids = sp_runtime::BoundedBTreeMap::<
                OrderPrice,
                OrderVolume,
                T::MaxSidePriceCount,
            >::try_from(BTreeMap::from([(*price, *value)]))
            .map_err(|_| ())?;
            self.aggregated_bids.set(*order_book_id, agg_bids);
        }
        Ok(())
    }

    fn sub_from_aggregated_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
        value: &OrderVolume,
    ) -> Result<(), ()> {
        if let Some(agg_bids) = self.aggregated_bids.get_mut(order_book_id) {
            let volume = agg_bids.get(price).copied().unwrap_or_default();
            let volume = volume.checked_sub(value).ok_or(())?;
            if volume.is_zero() {
                agg_bids.remove(price);
            } else {
                // realistically error should never be triggered;
                // if there was value at `price` then it should insert at the same key
                // if there was no value with this key, then subtracting unsigned number from 0
                //     cannot give non-zero value (thus avoiding this branch).
                agg_bids.try_insert(*price, volume).map_err(|_| ())?;
            }
            Ok(())
        } else if !price.is_zero() {
            // no aggregated bids for the order_book, thus we assume 0's for all prices
            // can't subtract non-zero from 0u128
            return Err(());
        } else {
            // 0 - 0 = 0
            Ok(())
        }
    }

    fn add_to_aggregated_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
        value: &OrderVolume,
    ) -> Result<(), ()> {
        if let Some(agg_asks) = self.aggregated_asks.get_mut(order_book_id) {
            let volume = agg_asks
                .get(price)
                .copied()
                .unwrap_or_default()
                .checked_add(value)
                .ok_or(())?;
            agg_asks.try_insert(*price, volume).map_err(|_| ())?;
        } else {
            let agg_asks = sp_runtime::BoundedBTreeMap::<
                OrderPrice,
                OrderVolume,
                T::MaxSidePriceCount,
            >::try_from(BTreeMap::from([(*price, *value)]))
            .map_err(|_| ())?;
            self.aggregated_asks.set(*order_book_id, agg_asks);
        }
        Ok(())
    }

    fn sub_from_aggregated_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
        value: &OrderVolume,
    ) -> Result<(), ()> {
        if let Some(agg_asks) = self.aggregated_asks.get_mut(order_book_id) {
            let volume = agg_asks.get(price).copied().unwrap_or_default();
            let volume = volume.checked_sub(value).ok_or(())?;
            if volume.is_zero() {
                agg_asks.remove(price);
            } else {
                // realistically error should never be triggered;
                // if there was value at `price` then it should insert at the same key
                // if there was no value with this key, then subtracting unsigned number from 0
                //     cannot give non-zero value (thus avoiding this branch).
                agg_asks.try_insert(*price, volume).map_err(|_| ())?;
            }
            Ok(())
        } else if !price.is_zero() {
            // no aggregated asks for the order_book, thus we assume 0's for all prices
            // can't subtract non-zero from 0u128
            return Err(());
        } else {
            // 0 - 0 = 0
            Ok(())
        }
    }
}

impl<T: Config> DataLayer<T> for CacheDataLayer<T> {
    fn get_limit_order(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        order_id: T::OrderId,
    ) -> Result<LimitOrder<T>, DispatchError> {
        if let Some(order) = self.limit_orders.get(order_book_id, &order_id) {
            Ok(order.clone())
        } else {
            Err(Error::<T>::UnknownLimitOrder.into())
        }
    }

    fn get_all_limit_orders(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Vec<LimitOrder<T>> {
        let orders = self.limit_orders.get_by_prefix(order_book_id);
        orders.into_values().collect()
    }

    fn insert_limit_order(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        limit_order: LimitOrder<T>,
    ) -> Result<(), DispatchError> {
        if self
            .limit_orders
            .contains_key(order_book_id, &limit_order.id)
        {
            return Err(Error::<T>::LimitOrderAlreadyExists.into());
        }

        match limit_order.side {
            PriceVariant::Buy => {
                self.add_to_bids(order_book_id, &limit_order)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
                self.add_to_aggregated_bids(order_book_id, &limit_order.price, &limit_order.amount)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
            }
            PriceVariant::Sell => {
                self.add_to_asks(order_book_id, &limit_order)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
                self.add_to_aggregated_asks(order_book_id, &limit_order.price, &limit_order.amount)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
            }
        }

        let mut user_orders = self
            .user_limit_orders
            .get(&limit_order.owner, order_book_id)
            .cloned()
            .unwrap_or_default();
        user_orders
            .try_push(limit_order.id)
            .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
        self.user_limit_orders
            .set(&limit_order.owner, order_book_id, user_orders);

        self.limit_orders
            .set(order_book_id, &limit_order.id.clone(), limit_order);

        Ok(())
    }

    fn update_limit_order_amount(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        order_id: T::OrderId,
        new_amount: OrderVolume,
    ) -> Result<(), DispatchError> {
        let mut order = self
            .limit_orders
            .get(order_book_id, &order_id)
            .cloned()
            .ok_or(Error::<T>::UnknownLimitOrder)?;
        if new_amount == order.amount {
            // nothing to update
            return Ok(());
        }
        if new_amount.is_zero() {
            return self.delete_limit_order(order_book_id, order_id);
        }
        ensure!(order.amount > new_amount, Error::<T>::UpdateLimitOrderError);

        let delta = order
            .amount
            .checked_sub(&new_amount)
            .ok_or(Error::<T>::AmountCalculationFailed)?;

        match order.side {
            PriceVariant::Buy => {
                self.sub_from_aggregated_bids(order_book_id, &order.price, &delta)
                    .map_err(|_| Error::<T>::UpdateLimitOrderError)?;
            }
            PriceVariant::Sell => {
                self.sub_from_aggregated_asks(order_book_id, &order.price, &delta)
                    .map_err(|_| Error::<T>::UpdateLimitOrderError)?;
            }
        }

        order.amount = new_amount;
        self.limit_orders.set(order_book_id, &order_id, order);

        Ok(())
    }

    fn delete_limit_order(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        order_id: T::OrderId,
    ) -> Result<(), DispatchError> {
        let order = self
            .limit_orders
            .get(order_book_id, &order_id)
            .cloned()
            .ok_or(Error::<T>::UnknownLimitOrder)?;

        match order.side {
            PriceVariant::Buy => {
                self.remove_from_bids(order_book_id, &order);
                self.sub_from_aggregated_bids(order_book_id, &order.price, &order.amount)
                    .map_err(|_| Error::<T>::DeleteLimitOrderError)?;
            }
            PriceVariant::Sell => {
                self.remove_from_asks(order_book_id, &order);
                self.sub_from_aggregated_asks(order_book_id, &order.price, &order.amount)
                    .map_err(|_| Error::<T>::DeleteLimitOrderError)?;
            }
        }

        if let Some(mut user_orders) = self
            .user_limit_orders
            .get(&order.owner, order_book_id)
            .cloned()
        {
            user_orders.retain(|x| *x != order.id);
            if user_orders.is_empty() {
                self.user_limit_orders.remove(&order.owner, order_book_id);
            } else {
                self.user_limit_orders
                    .set(&order.owner, order_book_id, user_orders);
            }
        } else {
            return Err(Error::<T>::DeleteLimitOrderError.into());
        }

        self.limit_orders.remove(order_book_id, &order_id);

        Ok(())
    }

    fn get_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
    ) -> Option<PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>> {
        self.bids.get(order_book_id, price).cloned()
    }

    fn is_bid_price_full(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
    ) -> Option<bool> {
        self.bids
            .get(order_book_id, price)
            .map(|orders| orders.is_full())
    }

    fn get_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
    ) -> Option<PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>> {
        self.asks.get(order_book_id, price).cloned()
    }

    fn is_ask_price_full(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
        price: &OrderPrice,
    ) -> Option<bool> {
        self.asks
            .get(order_book_id, price)
            .map(|orders| orders.is_full())
    }

    fn get_aggregated_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> MarketSide<T::MaxSidePriceCount> {
        self.aggregated_bids
            .get(order_book_id)
            .cloned()
            .unwrap_or_default()
    }

    fn get_aggregated_bids_len(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<usize> {
        self.aggregated_bids.get(order_book_id).map(|l| l.len())
    }

    fn best_bid(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<(OrderPrice, OrderVolume)> {
        self.aggregated_bids
            .get(order_book_id)
            .and_then(|side| side.iter().max().map(|(k, v)| (*k, *v)))
    }

    fn get_aggregated_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> MarketSide<T::MaxSidePriceCount> {
        self.aggregated_asks
            .get(order_book_id)
            .cloned()
            .unwrap_or_default()
    }

    fn get_aggregated_asks_len(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<usize> {
        self.aggregated_asks.get(order_book_id).map(|l| l.len())
    }

    fn best_ask(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<(OrderPrice, OrderVolume)> {
        self.aggregated_asks
            .get(order_book_id)
            .and_then(|side| side.iter().min().map(|(k, v)| (*k, *v)))
    }

    fn get_user_limit_orders(
        &mut self,
        account: &T::AccountId,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<UserOrders<T::OrderId, T::MaxOpenedLimitOrdersPerUser>> {
        self.user_limit_orders.get(account, order_book_id).cloned()
    }

    fn is_user_limit_orders_full(
        &mut self,
        account: &T::AccountId,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DexId>,
    ) -> Option<bool> {
        self.user_limit_orders
            .get_mut(account, order_book_id)
            .map(|orders| orders.is_full())
    }

    fn get_all_user_limit_orders(
        &mut self,
        account: &T::AccountId,
    ) -> BTreeMap<
        OrderBookId<AssetIdOf<T>, T::DexId>,
        UserOrders<T::OrderId, T::MaxOpenedLimitOrdersPerUser>,
    > {
        self.user_limit_orders.get_by_prefix(account)
    }
}
