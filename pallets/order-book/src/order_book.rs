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
    CurrencyLocker, CurrencyUnlocker, DataLayer, Error, LimitOrder, OrderBookId, OrderPrice,
    OrderVolume,
};
use assets::AssetIdOf;
use codec::{Decode, Encode, MaxEncodedLen};
use common::{balance, PriceVariant};
use core::fmt::Debug;
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::Get;
use sp_runtime::traits::{One, Zero};
use sp_std::ops::Add;

#[derive(
    Encode, Decode, PartialEq, Eq, Copy, Clone, Debug, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum OrderBookStatus {
    /// All operations are allowed.
    Trade,

    /// Users can place and cancel limit order, but trading is forbidden.
    PlaceAndCancel,

    /// Users can only cancel their limit orders. Placement and trading are forbidden.
    OnlyCancel,

    /// All operations with order book are forbidden. Current limit orders are frozen and users cannot cancel them.
    Stop,
}

#[derive(Encode, Decode, PartialEq, Eq, Clone, Debug, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct OrderBook<T>
where
    T: crate::Config,
{
    pub order_book_id: OrderBookId<AssetIdOf<T>>,
    pub dex_id: T::DEXId,
    pub status: OrderBookStatus,
    pub last_order_id: T::OrderId,
    pub tick_size: OrderPrice,      // price precision
    pub step_lot_size: OrderVolume, // amount precision
    pub min_lot_size: OrderVolume,
    pub max_lot_size: OrderVolume,
}

impl<T: crate::Config + Sized> OrderBook<T> {
    pub fn new(
        order_book_id: OrderBookId<AssetIdOf<T>>,
        dex_id: T::DEXId,
        tick_size: OrderPrice,
        step_lot_size: OrderVolume,
        min_lot_size: OrderVolume,
        max_lot_size: OrderVolume,
    ) -> Self {
        Self {
            order_book_id: order_book_id,
            dex_id: dex_id,
            status: OrderBookStatus::Trade,
            last_order_id: T::OrderId::zero(),
            tick_size: tick_size,
            step_lot_size: step_lot_size,
            min_lot_size: min_lot_size,
            max_lot_size: max_lot_size,
        }
    }

    pub fn default(order_book_id: OrderBookId<AssetIdOf<T>>, dex_id: T::DEXId) -> Self {
        Self::new(
            order_book_id,
            dex_id,
            balance!(0.00001), // TODO: order-book clarify
            balance!(0.00001), // TODO: order-book clarify
            balance!(1),       // TODO: order-book clarify
            balance!(100000),  // TODO: order-book clarify
        )
    }

    pub fn default_nft(order_book_id: OrderBookId<AssetIdOf<T>>, dex_id: T::DEXId) -> Self {
        Self::new(
            order_book_id,
            dex_id,
            balance!(0.00001), // TODO: order-book clarify
            balance!(1),       // TODO: order-book clarify
            balance!(1),       // TODO: order-book clarify
            balance!(100000),  // TODO: order-book clarify
        )
    }

    pub fn next_order_id(&mut self) -> T::OrderId {
        self.last_order_id = self.last_order_id.add(T::OrderId::one());
        self.last_order_id
    }

    pub fn place_limit_order<Locker>(
        &self,
        order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(), DispatchError>
    where
        Locker: CurrencyLocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
    {
        ensure!(
            self.status == OrderBookStatus::Trade || self.status == OrderBookStatus::PlaceAndCancel,
            Error::<T>::PlacementOfLimitOrdersIsForbidden
        );

        self.ensure_limit_order_valid(&order)?;
        self.check_restrictions(&order, data)?;

        let cross_spread = match order.side {
            PriceVariant::Buy => {
                if let Some((best_ask_price, _)) = self.best_ask(data) {
                    order.price >= best_ask_price
                } else {
                    false
                }
            }
            PriceVariant::Sell => {
                if let Some((best_bid_price, _)) = self.best_bid(data) {
                    order.price <= best_bid_price
                } else {
                    false
                }
            }
        };

        if cross_spread {
            if self.status == OrderBookStatus::Trade {
                self.cross_spread();
            } else {
                return Err(Error::<T>::InvalidLimitOrderPrice.into());
            }
        }

        let (lock_asset, lock_amount) = order.appropriate_asset_and_amount(&self.order_book_id)?;

        Locker::lock_liquidity(
            self.dex_id,
            &order.owner,
            self.order_book_id,
            lock_asset,
            lock_amount,
        )?;

        data.insert_limit_order(&self.order_book_id, order)?;
        Ok(())
    }

    pub fn cancel_limit_order<Unlocker>(
        &self,
        order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(), DispatchError>
    where
        Unlocker: CurrencyUnlocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
    {
        ensure!(
            self.status == OrderBookStatus::Trade
                || self.status == OrderBookStatus::PlaceAndCancel
                || self.status == OrderBookStatus::OnlyCancel,
            Error::<T>::CancellationOfLimitOrdersIsForbidden
        );

        self.cancel_limit_order_unchecked::<Unlocker>(order, data)
    }

    pub fn cancel_all_limit_orders<Unlocker>(
        &self,
        data: &mut impl DataLayer<T>,
    ) -> Result<usize, DispatchError>
    where
        Unlocker: CurrencyUnlocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
    {
        let orders = data.get_all_limit_orders(&self.order_book_id);
        let count = orders.len();

        for order in orders {
            self.cancel_limit_order_unchecked::<Unlocker>(order, data)?;
        }

        Ok(count)
    }

    pub fn cancel_limit_order_unchecked<Unlocker>(
        &self,
        order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(), DispatchError>
    where
        Unlocker: CurrencyUnlocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
    {
        let (lock_asset, lock_amount) = order.appropriate_asset_and_amount(&self.order_book_id)?;

        Unlocker::unlock_liquidity(
            self.dex_id,
            &order.owner,
            self.order_book_id,
            lock_asset,
            lock_amount,
        )?;

        data.delete_limit_order(&self.order_book_id, order.id)?;
        Ok(())
    }

    fn ensure_limit_order_valid(&self, order: &LimitOrder<T>) -> Result<(), DispatchError> {
        order.ensure_valid()?;
        ensure!(
            order.price % self.tick_size == 0,
            Error::<T>::InvalidLimitOrderPrice
        );
        ensure!(
            self.min_lot_size <= order.amount && order.amount <= self.max_lot_size,
            Error::<T>::InvalidOrderAmount
        );
        ensure!(
            order.amount % self.step_lot_size == 0,
            Error::<T>::InvalidOrderAmount
        );
        Ok(())
    }

    fn check_restrictions(
        &self,
        order: &LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(), DispatchError> {
        if let Some(user_orders) = data.get_user_limit_orders(&order.owner, &self.order_book_id) {
            ensure!(
                !user_orders.is_full(),
                Error::<T>::UserHasMaxCountOfOpenedOrders
            );
        }
        match order.side {
            PriceVariant::Buy => {
                if let Some(bids) = data.get_bids(&self.order_book_id, &order.price) {
                    ensure!(
                        !bids.is_full(),
                        Error::<T>::PriceReachedMaxCountOfLimitOrders
                    );
                }

                let agg_bids = data.get_aggregated_bids(&self.order_book_id);
                ensure!(
                    agg_bids.len() < T::MaxSidePriceCount::get() as usize,
                    Error::<T>::OrderBookReachedMaxCountOfPricesForSide
                );

                if let Some((best_bid_price, _)) = self.best_bid(data) {
                    let diff = best_bid_price.abs_diff(order.price);
                    ensure!(
                        diff <= T::MAX_PRICE_SHIFT * best_bid_price,
                        Error::<T>::InvalidLimitOrderPrice
                    );
                }
            }
            PriceVariant::Sell => {
                if let Some(asks) = data.get_asks(&self.order_book_id, &order.price) {
                    ensure!(
                        !asks.is_full(),
                        Error::<T>::PriceReachedMaxCountOfLimitOrders
                    );
                }

                let agg_asks = data.get_aggregated_asks(&self.order_book_id);
                ensure!(
                    agg_asks.len() < T::MaxSidePriceCount::get() as usize,
                    Error::<T>::OrderBookReachedMaxCountOfPricesForSide
                );

                if let Some((best_ask_price, _)) = self.best_ask(data) {
                    let diff = best_ask_price.abs_diff(order.price);
                    ensure!(
                        diff <= T::MAX_PRICE_SHIFT * best_ask_price,
                        Error::<T>::InvalidLimitOrderPrice
                    );
                }
            }
        }
        Ok(())
    }

    fn best_bid(&self, data: &mut impl DataLayer<T>) -> Option<(OrderPrice, OrderVolume)> {
        let bids = data.get_aggregated_bids(&self.order_book_id);
        bids.iter().max().map(|(k, v)| (*k, *v))
    }

    fn best_ask(&self, data: &mut impl DataLayer<T>) -> Option<(OrderPrice, OrderVolume)> {
        let asks = data.get_aggregated_asks(&self.order_book_id);
        asks.iter().min().map(|(k, v)| (*k, *v))
    }

    fn market_volume(&self, side: PriceVariant, data: &mut impl DataLayer<T>) -> OrderVolume {
        let volume = match side {
            PriceVariant::Buy => {
                let bids = data.get_aggregated_bids(&self.order_book_id);
                bids.iter()
                    .fold(OrderVolume::zero(), |sum, (_, volume)| sum + volume)
            }
            PriceVariant::Sell => {
                let asks = data.get_aggregated_asks(&self.order_book_id);
                asks.iter()
                    .fold(OrderVolume::zero(), |sum, (_, volume)| sum + volume)
            }
        };

        volume
    }

    fn cross_spread(&self) {
        // todo (m.tagirov)
        todo!()
    }
}
