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
    CurrencyLocker, CurrencyUnlocker, DataLayer, DealInfo, Error, ExpirationScheduler, LimitOrder,
    MarketChange, MarketOrder, MarketRole, OrderAmount, OrderBookId, OrderBookStatus, OrderPrice,
    OrderVolume, Payment,
};
use assets::AssetIdOf;
use codec::{Decode, Encode, MaxEncodedLen};
use common::prelude::{FixedWrapper, QuoteAmount};
use common::{balance, PriceVariant};
use core::fmt::Debug;
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::{One, Zero};
use sp_std::cmp::Ordering;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::ops::Add;

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
            order_book_id,
            dex_id,
            status: OrderBookStatus::Trade,
            last_order_id: T::OrderId::zero(),
            tick_size,
            step_lot_size,
            min_lot_size,
            max_lot_size,
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

    /// Tries to place the limit order and returns market input & deal input amounts.
    /// In some cases if the limit order crosses the spread, part or all of the amount could be converted into a market order and as a result, the deal input is not empty.
    pub fn place_limit_order<Locker, Unlocker, Scheduler>(
        &self,
        order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(Option<OrderAmount>, Option<OrderAmount>), DispatchError>
    where
        Locker: CurrencyLocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Unlocker: CurrencyUnlocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Scheduler:
            ExpirationScheduler<T::BlockNumber, OrderBookId<T::AssetId>, T::OrderId, DispatchError>,
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

        let market_change = if cross_spread {
            if self.status == OrderBookStatus::Trade {
                self.cross_spread(order, data)?
            } else {
                return Err(Error::<T>::InvalidLimitOrderPrice.into());
            }
        } else {
            self.calculate_limit_order_impact(order)?
        };

        let market_input = market_change.market_input;
        let deal_input = market_change.deal_input;

        self.apply_market_change::<Locker, Unlocker, Scheduler>(market_change, data)?;

        Ok((market_input, deal_input))
    }

    pub fn cancel_limit_order<Locker, Unlocker, Scheduler>(
        &self,
        order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(), DispatchError>
    where
        Locker: CurrencyLocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Unlocker: CurrencyUnlocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Scheduler:
            ExpirationScheduler<T::BlockNumber, OrderBookId<T::AssetId>, T::OrderId, DispatchError>,
    {
        ensure!(
            self.status == OrderBookStatus::Trade
                || self.status == OrderBookStatus::PlaceAndCancel
                || self.status == OrderBookStatus::OnlyCancel,
            Error::<T>::CancellationOfLimitOrdersIsForbidden
        );

        self.cancel_limit_order_unchecked::<Locker, Unlocker, Scheduler>(order, data, false)
    }

    pub fn cancel_all_limit_orders<Locker, Unlocker, Scheduler>(
        &self,
        data: &mut impl DataLayer<T>,
    ) -> Result<usize, DispatchError>
    where
        Locker: CurrencyLocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Unlocker: CurrencyUnlocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Scheduler:
            ExpirationScheduler<T::BlockNumber, OrderBookId<T::AssetId>, T::OrderId, DispatchError>,
    {
        let market_change = self.calculate_cancelation_of_all_limit_orders_impact(data)?;

        let count = market_change.to_delete.len();

        self.apply_market_change::<Locker, Unlocker, Scheduler>(market_change, data)?;

        Ok(count)
    }

    /// Executes market order and returns input & output amounts
    pub fn execute_market_order<Locker, Unlocker, Scheduler>(
        &self,
        order: MarketOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(OrderAmount, OrderAmount), DispatchError>
    where
        Locker: CurrencyLocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Unlocker: CurrencyUnlocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Scheduler:
            ExpirationScheduler<T::BlockNumber, OrderBookId<T::AssetId>, T::OrderId, DispatchError>,
    {
        ensure!(
            self.status == OrderBookStatus::Trade,
            Error::<T>::TradingIsForbidden
        );

        self.ensure_market_order_valid(&order)?;

        let market_change = self.calculate_market_order_impact(order, data)?;

        let (Some(input), Some(output)) =
            (market_change.deal_input, market_change.deal_output) else {
            // should never happen
            return Err(Error::<T>::PriceCalculationFailed.into());
        };

        self.apply_market_change::<Locker, Unlocker, Scheduler>(market_change, data)?;

        Ok((input, output))
    }

    pub fn calculate_market_order_impact(
        &self,
        order: MarketOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<
        MarketChange<
            T::AccountId,
            T::AssetId,
            T::DEXId,
            T::OrderId,
            LimitOrder<T>,
            BlockNumberFor<T>,
        >,
        DispatchError,
    > {
        let receiver = order.to.unwrap_or(order.owner.clone());

        match order.side {
            PriceVariant::Buy => self.calculate_market_impact(
                order.side,
                order.owner,
                receiver,
                order.amount,
                data.get_aggregated_asks(&self.order_book_id).iter(),
                data,
            ),
            PriceVariant::Sell => self.calculate_market_impact(
                order.side,
                order.owner,
                receiver,
                order.amount,
                data.get_aggregated_bids(&self.order_book_id).iter().rev(),
                data,
            ),
        }
    }

    pub fn calculate_limit_order_impact(
        &self,
        order: LimitOrder<T>,
    ) -> Result<
        MarketChange<
            T::AccountId,
            T::AssetId,
            T::DEXId,
            T::OrderId,
            LimitOrder<T>,
            BlockNumberFor<T>,
        >,
        DispatchError,
    > {
        let mut payment = Payment::new(self.dex_id, self.order_book_id);

        // necessary to lock the liquidity that taker should receive if execute the limit order
        let lock_amount = order.deal_amount(MarketRole::Taker, None)?;
        let lock_asset = lock_amount.associated_asset(&self.order_book_id);

        payment
            .to_lock
            .entry(*lock_asset)
            .or_default()
            .entry(order.owner.clone())
            .and_modify(|amount| *amount += *lock_amount.value())
            .or_insert(*lock_amount.value());

        Ok(MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: Some(lock_amount),
            market_output: None,
            to_add: BTreeMap::from([(order.id, order)]),
            to_update: BTreeMap::new(),
            to_delete: BTreeMap::new(),
            payment,
            ignore_unschedule_error: false,
        })
    }

    pub fn calculate_cancelation_limit_order_impact(
        &self,
        limit_order: LimitOrder<T>,
        ignore_unschedule_error: bool,
    ) -> Result<
        MarketChange<
            T::AccountId,
            T::AssetId,
            T::DEXId,
            T::OrderId,
            LimitOrder<T>,
            BlockNumberFor<T>,
        >,
        DispatchError,
    > {
        let mut limit_order_ids_to_delete = BTreeMap::new();
        let mut payment = Payment::new(self.dex_id, self.order_book_id);

        let unlock_amount = limit_order.deal_amount(MarketRole::Taker, None)?;
        let unlock_asset = unlock_amount.associated_asset(&self.order_book_id);

        limit_order_ids_to_delete.insert(limit_order.id, limit_order.expires_at);

        payment
            .to_unlock
            .entry(*unlock_asset)
            .or_default()
            .entry(limit_order.owner)
            .and_modify(|pay| *pay += unlock_amount.value())
            .or_insert(*unlock_amount.value());

        Ok(MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: Some(unlock_amount),
            to_add: BTreeMap::new(),
            to_update: BTreeMap::new(),
            to_delete: limit_order_ids_to_delete,
            payment,
            ignore_unschedule_error,
        })
    }

    pub fn calculate_cancelation_of_all_limit_orders_impact(
        &self,
        data: &mut impl DataLayer<T>,
    ) -> Result<
        MarketChange<
            T::AccountId,
            T::AssetId,
            T::DEXId,
            T::OrderId,
            LimitOrder<T>,
            BlockNumberFor<T>,
        >,
        DispatchError,
    > {
        let mut limit_order_ids_to_delete = BTreeMap::new();
        let mut payment = Payment::new(self.dex_id, self.order_book_id);

        let limit_orders = data.get_all_limit_orders(&self.order_book_id);

        for limit_order in limit_orders {
            let unlock_amount = limit_order.deal_amount(MarketRole::Taker, None)?;
            let unlock_asset = unlock_amount.associated_asset(&self.order_book_id);

            limit_order_ids_to_delete.insert(limit_order.id, limit_order.expires_at);

            payment
                .to_unlock
                .entry(*unlock_asset)
                .or_default()
                .entry(limit_order.owner)
                .and_modify(|pay| *pay += unlock_amount.value())
                .or_insert(*unlock_amount.value());
        }

        Ok(MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None, // NA for this case, because all the liquidity of both types go out of market
            to_add: BTreeMap::new(),
            to_update: BTreeMap::new(),
            to_delete: limit_order_ids_to_delete,
            payment,
            ignore_unschedule_error: false,
        })
    }

    /// Calculates how the deal with `taker_base_amount` impacts on the market
    fn calculate_market_impact<'a>(
        &self,
        side: PriceVariant,
        taker: T::AccountId,
        receiver: T::AccountId,
        taker_base_amount: OrderVolume,
        market_data: impl Iterator<Item = (&'a OrderPrice, &'a OrderVolume)>,
        data: &mut impl DataLayer<T>,
    ) -> Result<
        MarketChange<
            T::AccountId,
            T::AssetId,
            T::DEXId,
            T::OrderId,
            LimitOrder<T>,
            BlockNumberFor<T>,
        >,
        DispatchError,
    > {
        let mut remaining_amount = taker_base_amount;
        let mut taker_amount = OrderVolume::zero();
        let mut maker_amount = OrderVolume::zero();
        let mut limit_order_ids_to_delete = BTreeMap::new();
        let mut limit_orders_to_update = BTreeMap::new();
        let mut payment = Payment::new(self.dex_id, self.order_book_id);

        let (maker_out_asset, taker_out_asset) = match side {
            PriceVariant::Buy => (self.order_book_id.quote, self.order_book_id.base),
            PriceVariant::Sell => (self.order_book_id.base, self.order_book_id.quote),
        };

        for (price, _) in market_data {
            let Some(price_level) = data.get_limit_orders_by_price(&self.order_book_id, side.switched(), price) else {
                return Err(Error::<T>::NotEnoughLiquidityInOrderBook.into());
            };

            for limit_order_id in price_level.into_iter() {
                let limit_order = data.get_limit_order(&self.order_book_id, limit_order_id)?;

                let (amount, dust) = if limit_order.amount % self.step_lot_size != 0 {
                    let aligned = self.align_amount(limit_order.amount);
                    (aligned, limit_order.amount - aligned)
                } else {
                    (limit_order.amount, OrderVolume::zero())
                };

                if !dust.is_zero() {
                    payment
                        .to_unlock
                        .entry(taker_out_asset)
                        .or_default()
                        .entry(limit_order.owner.clone())
                        .and_modify(|unlock_amount| *unlock_amount += dust)
                        .or_insert(dust);
                }

                if remaining_amount >= amount {
                    remaining_amount -= amount;
                    taker_amount += limit_order
                        .deal_amount(MarketRole::Taker, Some(amount))?
                        .value();
                    let maker_payment = *limit_order
                        .deal_amount(MarketRole::Maker, Some(amount))?
                        .value();
                    maker_amount += maker_payment;
                    payment
                        .to_unlock
                        .entry(maker_out_asset)
                        .or_default()
                        .entry(limit_order.owner.clone())
                        .and_modify(|payment| *payment += maker_payment)
                        .or_insert(maker_payment);
                    limit_order_ids_to_delete.insert(limit_order.id, limit_order.expires_at);

                    if remaining_amount.is_zero() {
                        break;
                    }
                } else {
                    taker_amount += limit_order
                        .deal_amount(MarketRole::Taker, Some(remaining_amount))?
                        .value();
                    let maker_payment = *limit_order
                        .deal_amount(MarketRole::Maker, Some(remaining_amount))?
                        .value();
                    maker_amount += maker_payment;
                    payment
                        .to_unlock
                        .entry(maker_out_asset)
                        .or_default()
                        .entry(limit_order.owner.clone())
                        .and_modify(|payment| *payment += maker_payment)
                        .or_insert(maker_payment);
                    let new_amount = amount - remaining_amount;
                    remaining_amount = OrderVolume::zero();
                    limit_orders_to_update.insert(limit_order.id, new_amount);
                    break;
                }
            }

            if remaining_amount.is_zero() {
                break;
            }
        }

        ensure!(
            remaining_amount.is_zero(),
            Error::<T>::NotEnoughLiquidityInOrderBook
        );

        payment
            .to_lock
            .entry(maker_out_asset)
            .or_default()
            .entry(taker)
            .and_modify(|lock_amount| *lock_amount += maker_amount)
            .or_insert(maker_amount);

        payment
            .to_unlock
            .entry(taker_out_asset)
            .or_default()
            .entry(receiver)
            .and_modify(|unlock_amount| *unlock_amount += taker_amount)
            .or_insert(taker_amount);

        let (deal_input, deal_output) = match side {
            PriceVariant::Buy => (
                Some(OrderAmount::Quote(maker_amount)),
                Some(OrderAmount::Base(taker_amount)),
            ),
            PriceVariant::Sell => (
                Some(OrderAmount::Base(maker_amount)),
                Some(OrderAmount::Quote(taker_amount)),
            ),
        };

        Ok(MarketChange {
            deal_input,
            deal_output,
            market_input: None,
            market_output: deal_output,
            to_add: BTreeMap::new(),
            to_update: limit_orders_to_update,
            to_delete: limit_order_ids_to_delete,
            payment,
            ignore_unschedule_error: false,
        })
    }

    pub fn calculate_deal(
        &self,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        amount: QuoteAmount<OrderVolume>,
        data: &mut impl DataLayer<T>,
    ) -> Result<DealInfo<AssetIdOf<T>>, DispatchError> {
        let side = self.get_side(input_asset_id, output_asset_id)?;

        let (base, quote) = match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => match side {
                PriceVariant::Buy => self.sum_market(
                    data.get_aggregated_asks(&self.order_book_id).iter(),
                    Some(OrderAmount::Quote(desired_amount_in)),
                )?,
                PriceVariant::Sell => self.sum_market(
                    data.get_aggregated_bids(&self.order_book_id).iter().rev(),
                    Some(OrderAmount::Base(desired_amount_in)),
                )?,
            },
            QuoteAmount::WithDesiredOutput { desired_amount_out } => match side {
                PriceVariant::Buy => self.sum_market(
                    data.get_aggregated_asks(&self.order_book_id).iter(),
                    Some(OrderAmount::Base(desired_amount_out)),
                )?,
                PriceVariant::Sell => self.sum_market(
                    data.get_aggregated_bids(&self.order_book_id).iter().rev(),
                    Some(OrderAmount::Quote(desired_amount_out)),
                )?,
            },
        };

        ensure!(
            *base.value() > OrderVolume::zero() && *quote.value() > OrderVolume::zero(),
            Error::<T>::InvalidOrderAmount
        );

        let (input_amount, output_amount) = match side {
            PriceVariant::Buy => (quote, base),
            PriceVariant::Sell => (base, quote),
        };

        let average_price = (FixedWrapper::from(*quote.value())
            / FixedWrapper::from(*base.value()))
        .try_into_balance()
        .map_err(|_| Error::<T>::PriceCalculationFailed)?;

        Ok(DealInfo::<AssetIdOf<T>> {
            input_asset_id: *input_asset_id,
            input_amount,
            output_asset_id: *output_asset_id,
            output_amount,
            average_price,
            side,
        })
    }

    /// Summarizes and returns `base` and `quote` volumes of market depth.
    /// If `depth_limit` is defined, it counts the maximum possible `base` and `quote` volumes under the limit,
    /// Otherwise returns the sum of whole market depth.
    pub fn sum_market<'a>(
        &self,
        market_data: impl Iterator<Item = (&'a OrderPrice, &'a OrderVolume)>,
        depth_limit: Option<OrderAmount>,
    ) -> Result<(OrderAmount, OrderAmount), DispatchError> {
        let mut market_base_volume = OrderVolume::zero();
        let mut market_quote_volume = OrderVolume::zero();

        let mut enough_liquidity = false;

        for (price, base_volume) in market_data {
            let quote_volume = (FixedWrapper::from(*price) * FixedWrapper::from(*base_volume))
                .try_into_balance()
                .map_err(|_| Error::<T>::AmountCalculationFailed)?;

            if let Some(limit) = depth_limit {
                match limit {
                    OrderAmount::Base(base_limit) => {
                        if market_base_volume + base_volume > base_limit {
                            let delta = self.align_amount(base_limit - market_base_volume);
                            market_base_volume += delta;
                            market_quote_volume += (FixedWrapper::from(*price)
                                * FixedWrapper::from(delta))
                            .try_into_balance()
                            .map_err(|_| Error::<T>::AmountCalculationFailed)?;
                            enough_liquidity = true;
                            break;
                        }
                    }
                    OrderAmount::Quote(quote_limit) => {
                        if market_quote_volume + quote_volume > quote_limit {
                            // delta in base asset
                            let delta = self.align_amount(
                                (FixedWrapper::from(quote_limit - market_quote_volume)
                                    / FixedWrapper::from(*price))
                                .try_into_balance()
                                .map_err(|_| Error::<T>::AmountCalculationFailed)?,
                            );
                            market_base_volume += delta;
                            market_quote_volume += (FixedWrapper::from(*price)
                                * FixedWrapper::from(delta))
                            .try_into_balance()
                            .map_err(|_| Error::<T>::AmountCalculationFailed)?;
                            enough_liquidity = true;
                            break;
                        }
                    }
                }
            }

            market_base_volume += base_volume;
            market_quote_volume += quote_volume;
        }

        ensure!(
            depth_limit.is_none() || enough_liquidity,
            Error::<T>::NotEnoughLiquidityInOrderBook
        );

        Ok((
            OrderAmount::Base(market_base_volume),
            OrderAmount::Quote(market_quote_volume),
        ))
    }

    pub fn apply_market_change<Locker, Unlocker, Scheduler>(
        &self,
        market_change: MarketChange<
            T::AccountId,
            T::AssetId,
            T::DEXId,
            T::OrderId,
            LimitOrder<T>,
            BlockNumberFor<T>,
        >,
        data: &mut impl DataLayer<T>,
    ) -> Result<(), DispatchError>
    where
        Locker: CurrencyLocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Unlocker: CurrencyUnlocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Scheduler:
            ExpirationScheduler<T::BlockNumber, OrderBookId<T::AssetId>, T::OrderId, DispatchError>,
    {
        market_change.payment.execute_all::<Locker, Unlocker>()?;

        for (delete_id, expires_at) in market_change.to_delete {
            data.delete_limit_order(&self.order_book_id, delete_id)?;
            let unschedule_result =
                Scheduler::unschedule(expires_at, self.order_book_id, delete_id);
            if !market_change.ignore_unschedule_error {
                unschedule_result?;
            }
        }

        for (order_id, new_amount) in market_change.to_update.iter() {
            data.update_limit_order_amount(&self.order_book_id, *order_id, *new_amount)?;
        }

        for limit_order in market_change.to_add.into_values() {
            let order_id = limit_order.id;
            let expires_at = limit_order.expires_at;
            data.insert_limit_order(&self.order_book_id, limit_order)?;
            Scheduler::schedule(expires_at, self.order_book_id, order_id)?;
        }

        Ok(())
    }

    pub fn get_side(
        &self,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
    ) -> Result<PriceVariant, DispatchError> {
        match self.order_book_id {
            OrderBookId::<AssetIdOf<T>> { base, quote }
                if base == *output_asset_id && quote == *input_asset_id =>
            {
                Ok(PriceVariant::Buy)
            }
            OrderBookId::<AssetIdOf<T>> { base, quote }
                if base == *input_asset_id && quote == *output_asset_id =>
            {
                Ok(PriceVariant::Sell)
            }
            _ => Err(Error::<T>::InvalidAsset.into()),
        }
    }

    pub fn align_amount(&self, amount: OrderVolume) -> OrderVolume {
        let steps = amount / self.step_lot_size;
        let aligned = steps * self.step_lot_size;
        aligned
    }

    /// ### `ignore_unschedule_error`
    /// We might ignore error from `unschedule()` with `ignore_unschedule_error = true`.
    ///
    /// This is useful for expiration of orders where we want to use the universal interface
    /// to remove an order. In such case the schedule already does not have the order, because
    /// it is removed more efficiently than in `unschedule()`
    pub(crate) fn cancel_limit_order_unchecked<Locker, Unlocker, Scheduler>(
        &self,
        limit_order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
        ignore_unschedule_error: bool,
    ) -> Result<(), DispatchError>
    where
        Locker: CurrencyLocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Unlocker: CurrencyUnlocker<T::AccountId, T::AssetId, T::DEXId, DispatchError>,
        Scheduler:
            ExpirationScheduler<T::BlockNumber, OrderBookId<T::AssetId>, T::OrderId, DispatchError>,
    {
        let market_change =
            self.calculate_cancelation_limit_order_impact(limit_order, ignore_unschedule_error)?;

        self.apply_market_change::<Locker, Unlocker, Scheduler>(market_change, data)?;

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

    fn ensure_market_order_valid(&self, order: &MarketOrder<T>) -> Result<(), DispatchError> {
        order.ensure_valid()?;
        ensure!(
            order.order_book_id == self.order_book_id,
            Error::<T>::InvalidOrderBookId
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
                    if order.price < best_bid_price {
                        let diff = best_bid_price.abs_diff(order.price);
                        ensure!(
                            diff <= T::MAX_PRICE_SHIFT * best_bid_price,
                            Error::<T>::InvalidLimitOrderPrice
                        );
                    }
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
                    if order.price > best_ask_price {
                        let diff = best_ask_price.abs_diff(order.price);
                        ensure!(
                            diff <= T::MAX_PRICE_SHIFT * best_ask_price,
                            Error::<T>::InvalidLimitOrderPrice
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub fn best_bid(&self, data: &mut impl DataLayer<T>) -> Option<(OrderPrice, OrderVolume)> {
        let bids = data.get_aggregated_bids(&self.order_book_id);
        bids.iter().max().map(|(k, v)| (*k, *v))
    }

    pub fn best_ask(&self, data: &mut impl DataLayer<T>) -> Option<(OrderPrice, OrderVolume)> {
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

    pub fn cross_spread<'a>(
        &self,
        limit_order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<
        MarketChange<
            T::AccountId,
            T::AssetId,
            T::DEXId,
            T::OrderId,
            LimitOrder<T>,
            BlockNumberFor<T>,
        >,
        DispatchError,
    > {
        let (mut market_amount, mut limit_amout) = match limit_order.side {
            PriceVariant::Buy => Self::calculate_market_depth_to_price(
                limit_order.side.switched(),
                limit_order.price,
                limit_order.amount,
                data.get_aggregated_asks(&self.order_book_id).iter(),
            ),
            PriceVariant::Sell => Self::calculate_market_depth_to_price(
                limit_order.side.switched(),
                limit_order.price,
                limit_order.amount,
                data.get_aggregated_bids(&self.order_book_id).iter().rev(),
            ),
        };

        if limit_amout < self.min_lot_size {
            let market_volume = self.market_volume(limit_order.side.switched(), data);
            if market_volume - market_amount >= limit_amout {
                market_amount += limit_amout;
                limit_amout = OrderVolume::zero();
            } else {
                limit_amout = OrderVolume::zero();
            }
        }

        let mut market_change = MarketChange::new(self.dex_id, self.order_book_id);

        if !market_amount.is_zero() {
            let market_order = MarketOrder::<T>::new(
                limit_order.owner.clone(),
                limit_order.side,
                self.order_book_id,
                market_amount,
                None,
            );
            market_change = self.calculate_market_order_impact(market_order, data)?;
        }

        if !limit_amout.is_zero() {
            let mut new_limit_order = limit_order.clone();
            new_limit_order.amount = limit_amout;
            market_change
                .merge(self.calculate_limit_order_impact(new_limit_order)?)
                .map_err(|_| Error::<T>::AmountCalculationFailed)?;
        }

        Ok(market_change)
    }

    /// Calculates and returns the sum of limit orders up to the `price` or until the `amount` is reached
    /// and remaining `amount` if it is greater than the market volume.
    pub fn calculate_market_depth_to_price<'a>(
        side: PriceVariant,
        price: OrderPrice,
        mut amount: OrderVolume,
        market_data: impl Iterator<Item = (&'a OrderPrice, &'a OrderVolume)>,
    ) -> (OrderVolume, OrderVolume) {
        let ord = match side {
            PriceVariant::Buy => Ordering::Less,
            PriceVariant::Sell => Ordering::Greater,
        };

        let mut market_amount = OrderVolume::zero();

        for (market_price, volume) in market_data {
            if market_price.cmp(&price) == ord {
                break;
            }

            if amount >= *volume {
                market_amount += volume;
                amount -= volume;
            } else {
                market_amount += amount;
                amount = OrderVolume::zero();
            }

            if amount.is_zero() {
                break;
            }
        }

        (market_amount, amount)
    }
}
