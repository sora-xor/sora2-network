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
use assets::AssetIdOf;
use common::cache_storage::{CacheStorageDoubleMap, CacheStorageMap};
use common::PriceVariant;
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use sp_runtime::traits::Zero;

pub struct CacheDataLayer<T: Config> {
    limit_orders:
        CacheStorageDoubleMap<OrderBookId<AssetIdOf<T>>, T::OrderId, LimitOrder<T>, LimitOrders<T>>,
    bids: CacheStorageDoubleMap<
        OrderBookId<AssetIdOf<T>>,
        OrderPrice,
        PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>,
        Bids<T>,
    >,
    asks: CacheStorageDoubleMap<
        OrderBookId<AssetIdOf<T>>,
        OrderPrice,
        PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>,
        Asks<T>,
    >,
    aggregated_bids:
        CacheStorageMap<OrderBookId<AssetIdOf<T>>, MarketSide<T::MaxSidePrices>, AggregatedBids<T>>,
    aggregated_asks:
        CacheStorageMap<OrderBookId<AssetIdOf<T>>, MarketSide<T::MaxSidePrices>, AggregatedAsks<T>>,
    user_limit_orders: CacheStorageDoubleMap<
        T::AccountId,
        OrderBookId<AssetIdOf<T>>,
        UserOrders<T::OrderId, T::MaxOpenedLimitOrdersForAllOrderBooksPerUser>,
        UserLimitOrders<T>,
    >,
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
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        order: &LimitOrder<T>,
    ) -> Result<(), ()> {
        let mut bids = self
            .bids
            .get(order_book_id, &order.price)
            .cloned()
            .unwrap_or_default();
        bids.try_push(order.id).map_err(|_| ())?;
        self.bids.set(order_book_id, &order.price, bids);
        Ok(())
    }

    fn remove_from_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        order: &LimitOrder<T>,
    ) {
        let mut bids = self
            .bids
            .get(order_book_id, &order.price)
            .cloned()
            .unwrap_or_default();
        bids.retain(|x| *x != order.id);
        if bids.is_empty() {
            self.bids.remove(order_book_id, &order.price);
        } else {
            self.bids.set(order_book_id, &order.price, bids);
        }
    }

    fn add_to_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        order: &LimitOrder<T>,
    ) -> Result<(), ()> {
        let mut asks = self
            .asks
            .get(order_book_id, &order.price)
            .cloned()
            .unwrap_or_default();
        asks.try_push(order.id).map_err(|_| ())?;
        self.asks.set(order_book_id, &order.price, asks);
        Ok(())
    }

    fn remove_from_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        order: &LimitOrder<T>,
    ) {
        let mut asks = self
            .asks
            .get(order_book_id, &order.price)
            .cloned()
            .unwrap_or_default();
        asks.retain(|x| *x != order.id);
        if asks.is_empty() {
            self.asks.remove(order_book_id, &order.price);
        } else {
            self.asks.set(order_book_id, &order.price, asks);
        }
    }

    fn add_to_aggregated_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        price: &OrderPrice,
        value: &OrderVolume,
    ) -> Result<(), ()> {
        let mut agg_bids = self
            .aggregated_bids
            .get(order_book_id)
            .cloned()
            .unwrap_or_default();
        let volume = agg_bids
            .get(price)
            .map(|x| *x)
            .unwrap_or_default()
            .checked_add(*value)
            .ok_or(())?;
        agg_bids.try_insert(*price, volume).map_err(|_| ())?;
        self.aggregated_bids.set(order_book_id.clone(), agg_bids);
        Ok(())
    }

    fn sub_from_aggregated_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        price: &OrderPrice,
        value: &OrderVolume,
    ) -> Result<(), ()> {
        let mut agg_bids = self
            .aggregated_bids
            .get(order_book_id)
            .cloned()
            .unwrap_or_default();
        let volume = agg_bids
            .get(price)
            .map(|x| *x)
            .unwrap_or_default()
            .checked_sub(*value)
            .ok_or(())?;
        if volume.is_zero() {
            agg_bids.remove(price);
        } else {
            agg_bids.try_insert(*price, volume).map_err(|_| ())?;
        }
        self.aggregated_bids.set(order_book_id.clone(), agg_bids);
        Ok(())
    }

    fn add_to_aggregated_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        price: &OrderPrice,
        value: &OrderVolume,
    ) -> Result<(), ()> {
        let mut agg_asks = self
            .aggregated_asks
            .get(order_book_id)
            .cloned()
            .unwrap_or_default();
        let volume = agg_asks
            .get(price)
            .map(|x| *x)
            .unwrap_or_default()
            .checked_add(*value)
            .ok_or(())?;
        agg_asks.try_insert(*price, volume).map_err(|_| ())?;
        self.aggregated_asks.set(order_book_id.clone(), agg_asks);
        Ok(())
    }

    fn sub_from_aggregated_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        price: &OrderPrice,
        value: &OrderVolume,
    ) -> Result<(), ()> {
        let mut agg_asks = self
            .aggregated_asks
            .get(order_book_id)
            .cloned()
            .unwrap_or_default();
        let volume = agg_asks
            .get(price)
            .map(|x| *x)
            .unwrap_or_default()
            .checked_sub(*value)
            .ok_or(())?;
        if volume.is_zero() {
            agg_asks.remove(price);
        } else {
            agg_asks.try_insert(*price, volume).map_err(|_| ())?;
        }
        self.aggregated_asks.set(order_book_id.clone(), agg_asks);
        Ok(())
    }
}

impl<T: Config> DataLayer<T> for CacheDataLayer<T> {
    fn get_limit_order(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        order_id: T::OrderId,
    ) -> Result<LimitOrder<T>, DispatchError> {
        if let Some(order) = self.limit_orders.get(&order_book_id, &order_id) {
            Ok(order.clone())
        } else {
            Err(Error::<T>::UnknownLimitOrder.into())
        }
    }

    fn insert_limit_order(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        order: LimitOrder<T>,
    ) -> Result<(), DispatchError> {
        if self.limit_orders.contains_key(order_book_id, &order.id) {
            return Err(Error::<T>::LimitOrderAlreadyExists.into());
        }

        match order.side {
            PriceVariant::Buy => {
                self.add_to_bids(order_book_id, &order)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
                self.add_to_aggregated_bids(order_book_id, &order.price, &order.amount)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
            }
            PriceVariant::Sell => {
                self.add_to_asks(order_book_id, &order)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
                self.add_to_aggregated_asks(order_book_id, &order.price, &order.amount)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
            }
        }

        let mut user_orders = self
            .user_limit_orders
            .get(&order.owner, order_book_id)
            .cloned()
            .unwrap_or_default();
        user_orders
            .try_push(order.id)
            .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
        self.user_limit_orders
            .set(&order.owner, order_book_id, user_orders);

        self.limit_orders
            .set(order_book_id, &order.id.clone(), order);

        Ok(())
    }

    fn update_limit_order_amount(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
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

        let delta = order.amount - new_amount;

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
        order_book_id: &OrderBookId<AssetIdOf<T>>,
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
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        price: &OrderPrice,
    ) -> Result<PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>, DispatchError> {
        if let Some(bids) = self.bids.get(order_book_id, price) {
            Ok(bids.clone())
        } else {
            Err(Error::<T>::NoDataForPrice.into())
        }
    }

    fn get_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
        price: &OrderPrice,
    ) -> Result<PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>, DispatchError> {
        if let Some(asks) = self.asks.get(order_book_id, price) {
            Ok(asks.clone())
        } else {
            Err(Error::<T>::NoDataForPrice.into())
        }
    }

    fn get_aggregated_bids(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
    ) -> Result<MarketSide<T::MaxSidePrices>, DispatchError> {
        if let Some(agg_bids) = self.aggregated_bids.get(order_book_id) {
            Ok(agg_bids.clone())
        } else {
            Err(Error::<T>::NoAggregatedData.into())
        }
    }

    fn get_aggregated_asks(
        &mut self,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
    ) -> Result<MarketSide<T::MaxSidePrices>, DispatchError> {
        if let Some(agg_asks) = self.aggregated_asks.get(order_book_id) {
            Ok(agg_asks.clone())
        } else {
            Err(Error::<T>::NoAggregatedData.into())
        }
    }

    fn get_user_limit_orders(
        &mut self,
        account: &T::AccountId,
        order_book_id: &OrderBookId<AssetIdOf<T>>,
    ) -> Option<UserOrders<T::OrderId, T::MaxOpenedLimitOrdersForAllOrderBooksPerUser>> {
        self.user_limit_orders.get(account, order_book_id).cloned()
    }
}
