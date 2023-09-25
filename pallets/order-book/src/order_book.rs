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
    DataLayer, DealInfo, Delegate, Error, ExpirationScheduler, LimitOrder, MarketChange,
    MarketOrder, MarketRole, OrderAmount, OrderBookEvent, OrderBookId, OrderBookStatus, OrderPrice,
    OrderVolume, Payment,
};
use assets::AssetIdOf;
use codec::{Decode, Encode, MaxEncodedLen};
use common::prelude::QuoteAmount;
use common::{balance, Balance, PriceVariant};
use core::fmt::Debug;
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::Get;
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One, Saturating, Zero};
use sp_std::cmp::Ordering;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::ops::Add;

#[derive(Encode, Decode, PartialEq, Eq, Clone, Debug, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct OrderBook<T>
where
    T: crate::Config,
{
    pub order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    pub status: OrderBookStatus,
    pub last_order_id: T::OrderId,
    pub tick_size: OrderPrice,      // price precision
    pub step_lot_size: OrderVolume, // amount precision
    pub min_lot_size: OrderVolume,
    pub max_lot_size: OrderVolume,
}

impl<T: crate::Config + Sized> OrderBook<T> {
    pub fn new(
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        tick_size: OrderPrice,
        step_lot_size: OrderVolume,
        min_lot_size: OrderVolume,
        max_lot_size: OrderVolume,
    ) -> Self {
        Self {
            order_book_id,
            status: OrderBookStatus::Trade,
            last_order_id: T::OrderId::zero(),
            tick_size,
            step_lot_size,
            min_lot_size,
            max_lot_size,
        }
    }

    pub fn default(order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>) -> Self {
        Self::new(
            order_book_id,
            OrderPrice::divisible(balance!(0.00001)), // TODO: order-book clarify
            OrderVolume::divisible(balance!(0.00001)), // TODO: order-book clarify
            OrderVolume::divisible(balance!(1)),      // TODO: order-book clarify
            OrderVolume::divisible(balance!(100000)), // TODO: order-book clarify
        )
    }

    pub fn default_indivisible(order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>) -> Self {
        Self::new(
            order_book_id,
            OrderPrice::divisible(balance!(0.00001)), // TODO: order-book clarify
            OrderVolume::indivisible(1),              // TODO: order-book clarify
            OrderVolume::indivisible(1),              // TODO: order-book clarify
            OrderVolume::indivisible(100000),         // TODO: order-book clarify
        )
    }

    pub fn next_order_id(&mut self) -> T::OrderId {
        self.last_order_id = self.last_order_id.add(T::OrderId::one());
        self.last_order_id
    }

    /// Tries to place the limit order and returns market input & deal input amounts.
    /// In some cases if the limit order crosses the spread, part or all of the amount could be converted into a market order and as a result, the deal input is not empty.
    pub fn place_limit_order(
        &self,
        limit_order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(), DispatchError> {
        ensure!(
            self.status == OrderBookStatus::Trade || self.status == OrderBookStatus::PlaceAndCancel,
            Error::<T>::PlacementOfLimitOrdersIsForbidden
        );

        self.ensure_limit_order_valid(&limit_order)?;
        self.check_restrictions(&limit_order, data)?;

        let cross_spread = match limit_order.side {
            PriceVariant::Buy => {
                if let Some((best_ask_price, _)) = self.best_ask(data) {
                    limit_order.price >= best_ask_price
                } else {
                    false
                }
            }
            PriceVariant::Sell => {
                if let Some((best_bid_price, _)) = self.best_bid(data) {
                    limit_order.price <= best_bid_price
                } else {
                    false
                }
            }
        };

        let order_id = limit_order.id;
        let owner_id = limit_order.owner.clone();
        let amount = limit_order.amount;

        let market_change = if cross_spread {
            if self.status == OrderBookStatus::Trade {
                self.cross_spread(limit_order, data)?
            } else {
                return Err(Error::<T>::InvalidLimitOrderPrice.into());
            }
        } else {
            self.calculate_limit_order_impact(limit_order)?
        };

        let maybe_average_price = market_change.average_deal_price();
        let maybe_deal_amount = market_change.deal_base_amount();
        let (market_input, deal_input) = (market_change.market_input, market_change.deal_input);

        self.apply_market_change(market_change, data)?;

        match (market_input, deal_input) {
            (None, Some(market_order_input)) => {
                let direction = if market_order_input.is_quote() {
                    PriceVariant::Buy
                } else {
                    PriceVariant::Sell
                };
                T::Delegate::emit_event(
                    self.order_book_id,
                    OrderBookEvent::LimitOrderConvertedToMarketOrder {
                        owner_id,
                        direction,
                        amount: OrderAmount::Base(amount),
                    },
                );
            }
            (Some(..), None) => (),
            (Some(..), Some(market_order_input)) => {
                let market_order_direction = if market_order_input.is_quote() {
                    PriceVariant::Buy
                } else {
                    PriceVariant::Sell
                };
                let (Some(deal_amount), Some(market_order_average_price)) = (maybe_deal_amount, maybe_average_price) else {
                    // should never happen
                    return Err(Error::<T>::PriceCalculationFailed.into());
                };
                T::Delegate::emit_event(
                    self.order_book_id,
                    OrderBookEvent::LimitOrderIsSplitIntoMarketOrderAndLimitOrder {
                        owner_id,
                        market_order_direction,
                        market_order_amount: OrderAmount::Base(deal_amount),
                        market_order_average_price,
                        limit_order_id: order_id,
                    },
                );
            }
            _ => {
                // should never happen
                return Err(Error::<T>::InvalidOrderAmount.into());
            }
        }

        Ok(())
    }

    pub fn cancel_limit_order(
        &self,
        limit_order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(), DispatchError> {
        ensure!(
            self.status == OrderBookStatus::Trade
                || self.status == OrderBookStatus::PlaceAndCancel
                || self.status == OrderBookStatus::OnlyCancel,
            Error::<T>::CancellationOfLimitOrdersIsForbidden
        );

        self.cancel_limit_order_unchecked(limit_order, data, false)
    }

    pub fn cancel_all_limit_orders(
        &self,
        data: &mut impl DataLayer<T>,
    ) -> Result<usize, DispatchError> {
        let market_change = self.calculate_cancellation_of_all_limit_orders_impact(data)?;

        let count = market_change.to_cancel.len();

        self.apply_market_change(market_change, data)?;

        Ok(count)
    }

    /// Executes market order and returns input & output amounts
    pub fn execute_market_order(
        &self,
        market_order: MarketOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(OrderAmount, OrderAmount), DispatchError> {
        ensure!(
            self.status == OrderBookStatus::Trade,
            Error::<T>::TradingIsForbidden
        );

        self.ensure_market_order_valid(&market_order)?;

        let market_change = self.calculate_market_order_impact(market_order.clone(), data)?;

        let (Some(input), Some(output)) =
            (market_change.deal_input, market_change.deal_output) else {
            // should never happen
            return Err(Error::<T>::PriceCalculationFailed.into());
        };

        let Some(average_price) = market_change.average_deal_price() else {
            // should never happen
            return Err(Error::<T>::PriceCalculationFailed.into());
        };

        self.apply_market_change(market_change, data)?;

        T::Delegate::emit_event(
            self.order_book_id,
            OrderBookEvent::MarketOrderExecuted {
                owner_id: market_order.owner,
                direction: market_order.direction,
                amount: OrderAmount::Base(market_order.amount),
                average_price,
                to: market_order.to,
            },
        );

        Ok((input, output))
    }

    pub fn align_limit_orders(&self, data: &mut impl DataLayer<T>) -> Result<(), DispatchError> {
        let market_change = self.calculate_align_limit_orders_impact(data)?;
        self.apply_market_change(market_change, data)?;
        Ok(())
    }

    pub fn calculate_market_order_impact(
        &self,
        market_order: MarketOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<
        MarketChange<T::AccountId, T::AssetId, T::DEXId, T::OrderId, LimitOrder<T>>,
        DispatchError,
    > {
        let receiver = market_order.to.unwrap_or(market_order.owner.clone());

        match market_order.direction {
            PriceVariant::Buy => self.calculate_market_impact(
                market_order.direction,
                market_order.owner,
                receiver,
                market_order.amount,
                data.get_aggregated_asks(&self.order_book_id).iter(),
                data,
            ),
            PriceVariant::Sell => self.calculate_market_impact(
                market_order.direction,
                market_order.owner,
                receiver,
                market_order.amount,
                data.get_aggregated_bids(&self.order_book_id).iter().rev(),
                data,
            ),
        }
    }

    pub fn calculate_limit_order_impact(
        &self,
        limit_order: LimitOrder<T>,
    ) -> Result<
        MarketChange<T::AccountId, T::AssetId, T::DEXId, T::OrderId, LimitOrder<T>>,
        DispatchError,
    > {
        let mut payment = Payment::new(self.order_book_id);

        // necessary to lock the liquidity that taker should receive if execute the limit order
        let lock_amount = limit_order.deal_amount(MarketRole::Taker, None)?;
        let lock_asset = lock_amount.associated_asset(&self.order_book_id);

        payment
            .to_lock
            .entry(*lock_asset)
            .or_default()
            .entry(limit_order.owner.clone())
            .and_modify(|amount| *amount = amount.saturating_add(*lock_amount.value()))
            .or_insert(*lock_amount.value());

        Ok(MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: Some(lock_amount),
            market_output: None,
            to_place: BTreeMap::from([(limit_order.id, limit_order)]),
            to_part_execute: BTreeMap::new(),
            to_full_execute: BTreeMap::new(),
            to_cancel: BTreeMap::new(),
            to_force_update: BTreeMap::new(),
            payment,
            ignore_unschedule_error: false,
        })
    }

    pub fn calculate_cancellation_limit_order_impact(
        &self,
        limit_order: LimitOrder<T>,
        ignore_unschedule_error: bool,
    ) -> Result<
        MarketChange<T::AccountId, T::AssetId, T::DEXId, T::OrderId, LimitOrder<T>>,
        DispatchError,
    > {
        let mut limit_orders_to_cancel = BTreeMap::new();
        let mut payment = Payment::new(self.order_book_id);

        let unlock_amount = limit_order.deal_amount(MarketRole::Taker, None)?;
        let unlock_asset = unlock_amount.associated_asset(&self.order_book_id);

        payment
            .to_unlock
            .entry(*unlock_asset)
            .or_default()
            .entry(limit_order.owner.clone())
            .and_modify(|pay| *pay = pay.saturating_add(*unlock_amount.value()))
            .or_insert(*unlock_amount.value());

        limit_orders_to_cancel.insert(limit_order.id, limit_order);

        Ok(MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: Some(unlock_amount),
            to_place: BTreeMap::new(),
            to_part_execute: BTreeMap::new(),
            to_full_execute: BTreeMap::new(),
            to_cancel: limit_orders_to_cancel,
            to_force_update: BTreeMap::new(),
            payment,
            ignore_unschedule_error,
        })
    }

    pub fn calculate_cancellation_of_all_limit_orders_impact(
        &self,
        data: &mut impl DataLayer<T>,
    ) -> Result<
        MarketChange<T::AccountId, T::AssetId, T::DEXId, T::OrderId, LimitOrder<T>>,
        DispatchError,
    > {
        let mut limit_orders_to_cancel = BTreeMap::new();
        let mut payment = Payment::new(self.order_book_id);

        let limit_orders = data.get_all_limit_orders(&self.order_book_id);

        for limit_order in limit_orders {
            let unlock_amount = limit_order.deal_amount(MarketRole::Taker, None)?;
            let unlock_asset = unlock_amount.associated_asset(&self.order_book_id);

            payment
                .to_unlock
                .entry(*unlock_asset)
                .or_default()
                .entry(limit_order.owner.clone())
                .and_modify(|pay| *pay = pay.saturating_add(*unlock_amount.value()))
                .or_insert(*unlock_amount.value());

            limit_orders_to_cancel.insert(limit_order.id, limit_order);
        }

        Ok(MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None, // NA for this case, because all the liquidity of both types goes out of market
            to_place: BTreeMap::new(),
            to_part_execute: BTreeMap::new(),
            to_full_execute: BTreeMap::new(),
            to_cancel: limit_orders_to_cancel,
            to_force_update: BTreeMap::new(),
            payment,
            ignore_unschedule_error: false,
        })
    }

    /// Calculates how the deal with `taker_base_amount` impacts the market
    fn calculate_market_impact<'a>(
        &self,
        direction: PriceVariant,
        taker: T::AccountId,
        receiver: T::AccountId,
        taker_base_amount: OrderVolume,
        market_data: impl Iterator<Item = (&'a OrderPrice, &'a OrderVolume)>,
        data: &mut impl DataLayer<T>,
    ) -> Result<
        MarketChange<T::AccountId, T::AssetId, T::DEXId, T::OrderId, LimitOrder<T>>,
        DispatchError,
    > {
        let mut remaining_amount = taker_base_amount;
        let mut taker_amount = OrderVolume::zero();
        let mut maker_amount = OrderVolume::zero();
        let mut limit_orders_to_part_execute = BTreeMap::new();
        let mut limit_orders_to_full_execute = BTreeMap::new();
        let mut payment = Payment::new(self.order_book_id);

        let (maker_out_asset, taker_out_asset) = match direction {
            PriceVariant::Buy => (self.order_book_id.quote, self.order_book_id.base),
            PriceVariant::Sell => (self.order_book_id.base, self.order_book_id.quote),
        };

        for (price, _) in market_data {
            let Some(price_level) = data.get_limit_orders_by_price(&self.order_book_id, direction.switched(), price) else {
                return Err(Error::<T>::NotEnoughLiquidityInOrderBook.into());
            };

            for limit_order_id in price_level.into_iter() {
                let mut limit_order = data.get_limit_order(&self.order_book_id, limit_order_id)?;

                if remaining_amount >= limit_order.amount {
                    remaining_amount = remaining_amount
                        .checked_sub(&limit_order.amount)
                        .ok_or(Error::<T>::AmountCalculationFailed)?;
                    taker_amount = taker_amount
                        .checked_add(limit_order.deal_amount(MarketRole::Taker, None)?.value())
                        .ok_or(Error::<T>::AmountCalculationFailed)?;
                    let maker_payment = *limit_order.deal_amount(MarketRole::Maker, None)?.value();
                    maker_amount = maker_amount
                        .checked_add(&maker_payment)
                        .ok_or(Error::<T>::AmountCalculationFailed)?;
                    payment
                        .to_unlock
                        .entry(maker_out_asset)
                        .or_default()
                        .entry(limit_order.owner.clone())
                        .and_modify(|payment| *payment = payment.saturating_add(maker_payment))
                        .or_insert(maker_payment);
                    limit_orders_to_full_execute.insert(limit_order.id, limit_order);

                    if remaining_amount.is_zero() {
                        break;
                    }
                } else {
                    taker_amount = taker_amount
                        .checked_add(
                            limit_order
                                .deal_amount(MarketRole::Taker, Some(remaining_amount))?
                                .value(),
                        )
                        .ok_or(Error::<T>::AmountCalculationFailed)?;
                    let maker_payment = *limit_order
                        .deal_amount(MarketRole::Maker, Some(remaining_amount))?
                        .value();
                    maker_amount = maker_amount
                        .checked_add(&maker_payment)
                        .ok_or(Error::<T>::AmountCalculationFailed)?;
                    payment
                        .to_unlock
                        .entry(maker_out_asset)
                        .or_default()
                        .entry(limit_order.owner.clone())
                        .and_modify(|payment| *payment = payment.saturating_add(maker_payment))
                        .or_insert(maker_payment);
                    limit_order.amount = limit_order
                        .amount
                        .checked_sub(&remaining_amount)
                        .ok_or(Error::<T>::AmountCalculationFailed)?;
                    limit_orders_to_part_execute.insert(
                        limit_order.id,
                        (limit_order, OrderAmount::Base(remaining_amount)),
                    );
                    remaining_amount = OrderVolume::zero();
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
            .and_modify(|lock_amount| *lock_amount = lock_amount.saturating_add(maker_amount))
            .or_insert(maker_amount);

        payment
            .to_unlock
            .entry(taker_out_asset)
            .or_default()
            .entry(receiver)
            .and_modify(|unlock_amount| *unlock_amount = unlock_amount.saturating_add(taker_amount))
            .or_insert(taker_amount);

        let (deal_input, deal_output) = match direction {
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
            to_place: BTreeMap::new(),
            to_part_execute: limit_orders_to_part_execute,
            to_full_execute: limit_orders_to_full_execute,
            to_cancel: BTreeMap::new(),
            to_force_update: BTreeMap::new(),
            payment,
            ignore_unschedule_error: false,
        })
    }

    pub fn calculate_align_limit_orders_impact(
        &self,
        data: &mut impl DataLayer<T>,
    ) -> Result<
        MarketChange<T::AccountId, T::AssetId, T::DEXId, T::OrderId, LimitOrder<T>>,
        DispatchError,
    > {
        let mut limit_orders_to_cancel = BTreeMap::new();
        let mut limit_orders_to_force_update = BTreeMap::new();
        let mut payment = Payment::new(self.order_book_id);

        let limit_orders = data.get_all_limit_orders(&self.order_book_id);

        for mut limit_order in limit_orders {
            if limit_order.amount.balance() % self.step_lot_size.balance() != 0 {
                let refund = if limit_order.amount < self.step_lot_size {
                    limit_orders_to_cancel.insert(limit_order.id, limit_order.clone());
                    limit_order.amount
                } else {
                    let amount = self.align_amount(limit_order.amount);
                    let dust = limit_order
                        .amount
                        .checked_sub(&amount)
                        .ok_or(Error::<T>::AmountCalculationFailed)?;
                    limit_order.amount = amount;
                    limit_orders_to_force_update.insert(limit_order.id, limit_order.clone());
                    dust
                };

                let refund = limit_order.deal_amount(MarketRole::Taker, Some(refund))?;

                payment
                    .to_unlock
                    .entry(*refund.associated_asset(&self.order_book_id))
                    .or_default()
                    .entry(limit_order.owner)
                    .and_modify(|unlock_amount| {
                        *unlock_amount = unlock_amount.saturating_add(*refund.value())
                    })
                    .or_insert(*refund.value());
            }
        }

        Ok(MarketChange {
            deal_input: None,
            deal_output: None,
            market_input: None,
            market_output: None, // NA for this case, because all the liquidity of both types can go out of market
            to_place: BTreeMap::new(),
            to_part_execute: BTreeMap::new(),
            to_full_execute: BTreeMap::new(),
            to_cancel: limit_orders_to_cancel,
            to_force_update: limit_orders_to_force_update,
            payment,
            ignore_unschedule_error: false,
        })
    }

    pub fn calculate_deal(
        &self,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        amount: QuoteAmount<Balance>,
        data: &mut impl DataLayer<T>,
    ) -> Result<DealInfo<AssetIdOf<T>>, DispatchError> {
        let direction = self.get_direction(input_asset_id, output_asset_id)?;

        let (base, quote) = match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => match direction {
                PriceVariant::Buy => self.sum_market(
                    data.get_aggregated_asks(&self.order_book_id).iter(),
                    Some(OrderAmount::Quote(
                        self.tick_size.copy_divisibility(desired_amount_in),
                    )),
                )?,
                PriceVariant::Sell => self.sum_market(
                    data.get_aggregated_bids(&self.order_book_id).iter().rev(),
                    Some(OrderAmount::Base(
                        self.step_lot_size.copy_divisibility(desired_amount_in),
                    )),
                )?,
            },
            QuoteAmount::WithDesiredOutput { desired_amount_out } => match direction {
                PriceVariant::Buy => self.sum_market(
                    data.get_aggregated_asks(&self.order_book_id).iter(),
                    Some(OrderAmount::Base(
                        self.step_lot_size.copy_divisibility(desired_amount_out),
                    )),
                )?,
                PriceVariant::Sell => self.sum_market(
                    data.get_aggregated_bids(&self.order_book_id).iter().rev(),
                    Some(OrderAmount::Quote(
                        self.tick_size.copy_divisibility(desired_amount_out),
                    )),
                )?,
            },
        };

        ensure!(
            *base.value() > OrderVolume::zero() && *quote.value() > OrderVolume::zero(),
            Error::<T>::InvalidOrderAmount
        );

        let (input_amount, output_amount) = match direction {
            PriceVariant::Buy => (quote, base),
            PriceVariant::Sell => (base, quote),
        };

        let average_price = OrderAmount::average_price(input_amount, output_amount)
            .map_err(|_| Error::<T>::PriceCalculationFailed)?;

        Ok(DealInfo::<AssetIdOf<T>> {
            input_asset_id: *input_asset_id,
            input_amount,
            output_asset_id: *output_asset_id,
            output_amount,
            average_price,
            direction,
        })
    }

    /// Summarizes and returns `base` and `quote` volumes of market depth.
    /// If `target_depth` is defined, it counts `base` and `quote` volumes under the limit and
    /// checks if there is enough volume,
    /// Otherwise returns the sum of whole market depth.
    pub fn sum_market<'a>(
        &self,
        market_data: impl Iterator<Item = (&'a OrderPrice, &'a OrderVolume)>,
        target_depth: Option<OrderAmount>,
    ) -> Result<(OrderAmount, OrderAmount), DispatchError> {
        let mut market_base_volume = OrderVolume::zero();
        let mut market_quote_volume = OrderVolume::zero();

        let mut enough_liquidity = false;

        for (price, base_volume) in market_data {
            let quote_volume = price
                .checked_mul(base_volume)
                .ok_or(Error::<T>::AmountCalculationFailed)?;

            if let Some(target_depth) = target_depth {
                match target_depth {
                    OrderAmount::Base(base_target) => {
                        if market_base_volume
                            .checked_add(base_volume)
                            .ok_or(Error::<T>::AmountCalculationFailed)?
                            > base_target
                        {
                            let delta = self.align_amount(
                                base_target
                                    .checked_sub(&market_base_volume)
                                    .ok_or(Error::<T>::AmountCalculationFailed)?,
                            );
                            market_base_volume = market_base_volume
                                .checked_add(&delta)
                                .ok_or(Error::<T>::AmountCalculationFailed)?;
                            market_quote_volume = market_quote_volume
                                .checked_add(
                                    &price
                                        .checked_mul(&delta)
                                        .ok_or(Error::<T>::AmountCalculationFailed)?,
                                )
                                .ok_or(Error::<T>::AmountCalculationFailed)?;
                            enough_liquidity = true;
                            break;
                        }
                    }
                    OrderAmount::Quote(quote_target) => {
                        if market_quote_volume
                            .checked_add(&quote_volume)
                            .ok_or(Error::<T>::AmountCalculationFailed)?
                            > quote_target
                        {
                            // delta in base asset
                            let delta = self.align_amount(
                                quote_target
                                    .checked_sub(&market_quote_volume)
                                    .ok_or(Error::<T>::AmountCalculationFailed)?
                                    .checked_div(price)
                                    .ok_or(Error::<T>::AmountCalculationFailed)?,
                            );
                            market_base_volume = market_base_volume
                                .checked_add(&delta)
                                .ok_or(Error::<T>::AmountCalculationFailed)?;
                            market_quote_volume = market_quote_volume
                                .checked_add(
                                    &price
                                        .checked_mul(&delta)
                                        .ok_or(Error::<T>::AmountCalculationFailed)?,
                                )
                                .ok_or(Error::<T>::AmountCalculationFailed)?;
                            enough_liquidity = true;
                            break;
                        }
                    }
                }
            }

            market_base_volume = market_base_volume
                .checked_add(base_volume)
                .ok_or(Error::<T>::AmountCalculationFailed)?;
            market_quote_volume = market_quote_volume
                .checked_add(&quote_volume)
                .ok_or(Error::<T>::AmountCalculationFailed)?;
        }

        // if we exactly match the limit, it means there is enough liquidity
        if let Some(target_depth) = target_depth {
            match target_depth {
                OrderAmount::Base(base_target) if market_base_volume == base_target => {
                    enough_liquidity = true;
                }
                OrderAmount::Quote(quote_target) if market_quote_volume == quote_target => {
                    enough_liquidity = true;
                }
                _ => {} // leave as is
            }
        };
        ensure!(
            target_depth.is_none() || enough_liquidity,
            Error::<T>::NotEnoughLiquidityInOrderBook
        );

        Ok((
            OrderAmount::Base(market_base_volume),
            OrderAmount::Quote(market_quote_volume),
        ))
    }

    pub fn apply_market_change(
        &self,
        market_change: MarketChange<T::AccountId, T::AssetId, T::DEXId, T::OrderId, LimitOrder<T>>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(), DispatchError> {
        market_change
            .payment
            .execute_all::<T::Locker, T::Unlocker>()?;

        for limit_order in market_change.to_cancel.into_values() {
            data.delete_limit_order(&self.order_book_id, limit_order.id)?;
            let unschedule_result = T::Scheduler::unschedule(
                limit_order.expires_at,
                self.order_book_id,
                limit_order.id,
            );
            if !market_change.ignore_unschedule_error {
                unschedule_result?;
            }

            T::Delegate::emit_event(
                self.order_book_id,
                OrderBookEvent::LimitOrderCanceled {
                    order_id: limit_order.id,
                    owner_id: limit_order.owner,
                },
            );
        }

        for limit_order in market_change.to_full_execute.into_values() {
            data.delete_limit_order(&self.order_book_id, limit_order.id)?;
            let unschedule_result = T::Scheduler::unschedule(
                limit_order.expires_at,
                self.order_book_id,
                limit_order.id,
            );
            if !market_change.ignore_unschedule_error {
                unschedule_result?;
            }

            T::Delegate::emit_event(
                self.order_book_id,
                OrderBookEvent::LimitOrderExecuted {
                    order_id: limit_order.id,
                    owner_id: limit_order.owner,
                    side: limit_order.side,
                    amount: OrderAmount::Base(limit_order.amount),
                },
            );
        }

        for (limit_order, executed_amount) in market_change.to_part_execute.into_values() {
            data.update_limit_order_amount(
                &self.order_book_id,
                limit_order.id,
                limit_order.amount,
            )?;

            T::Delegate::emit_event(
                self.order_book_id,
                OrderBookEvent::LimitOrderExecuted {
                    order_id: limit_order.id,
                    owner_id: limit_order.owner,
                    side: limit_order.side,
                    amount: executed_amount,
                },
            );
        }

        for limit_order in market_change.to_force_update.into_values() {
            data.update_limit_order_amount(
                &self.order_book_id,
                limit_order.id,
                limit_order.amount,
            )?;

            T::Delegate::emit_event(
                self.order_book_id,
                OrderBookEvent::LimitOrderUpdated {
                    order_id: limit_order.id,
                    owner_id: limit_order.owner,
                },
            );
        }

        for limit_order in market_change.to_place.into_values() {
            let order_id = limit_order.id;
            let owner_id = limit_order.owner.clone();
            let expires_at = limit_order.expires_at;
            data.insert_limit_order(&self.order_book_id, limit_order)?;
            T::Scheduler::schedule(expires_at, self.order_book_id, order_id)?;

            T::Delegate::emit_event(
                self.order_book_id,
                OrderBookEvent::LimitOrderPlaced { order_id, owner_id },
            );
        }

        Ok(())
    }

    pub fn get_direction(
        &self,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
    ) -> Result<PriceVariant, DispatchError> {
        let OrderBookId::<AssetIdOf<T>, _> { base, quote, .. } = self.order_book_id;
        if base == *output_asset_id && quote == *input_asset_id {
            Ok(PriceVariant::Buy)
        } else if base == *input_asset_id && quote == *output_asset_id {
            Ok(PriceVariant::Sell)
        } else {
            Err(Error::<T>::InvalidAsset.into())
        }
    }

    pub fn align_amount(&self, mut amount: OrderVolume) -> OrderVolume {
        let steps = amount
            .balance()
            .saturating_div(*self.step_lot_size.balance());
        let aligned = steps.saturating_mul(*self.step_lot_size.balance());
        amount.set(aligned);
        amount
    }

    /// ### `ignore_unschedule_error`
    /// We might ignore error from `unschedule()` with `ignore_unschedule_error = true`.
    ///
    /// This is useful for expiration of orders where we want to use the universal interface
    /// to remove an order. In such case the schedule already does not have the order, because
    /// it is removed more efficiently than in `unschedule()`
    pub(crate) fn cancel_limit_order_unchecked(
        &self,
        limit_order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
        ignore_unschedule_error: bool,
    ) -> Result<(), DispatchError> {
        let market_change =
            self.calculate_cancellation_limit_order_impact(limit_order, ignore_unschedule_error)?;

        self.apply_market_change(market_change, data)
    }

    fn ensure_limit_order_valid(&self, limit_order: &LimitOrder<T>) -> Result<(), DispatchError> {
        limit_order.ensure_valid()?;
        ensure!(
            limit_order.price.balance() % self.tick_size.balance() == 0,
            Error::<T>::InvalidLimitOrderPrice
        );
        ensure!(
            self.min_lot_size <= limit_order.amount && limit_order.amount <= self.max_lot_size,
            Error::<T>::InvalidOrderAmount
        );
        ensure!(
            limit_order.amount.balance() % self.step_lot_size.balance() == 0,
            Error::<T>::InvalidOrderAmount
        );
        Ok(())
    }

    fn ensure_market_order_valid(
        &self,
        market_order: &MarketOrder<T>,
    ) -> Result<(), DispatchError> {
        market_order.ensure_valid()?;
        ensure!(
            market_order.order_book_id == self.order_book_id,
            Error::<T>::InvalidOrderBookId
        );
        ensure!(
            self.min_lot_size <= market_order.amount && market_order.amount <= self.max_lot_size,
            Error::<T>::InvalidOrderAmount
        );
        ensure!(
            market_order.amount.balance() % self.step_lot_size.balance() == 0,
            Error::<T>::InvalidOrderAmount
        );
        Ok(())
    }

    fn check_restrictions(
        &self,
        limit_order: &LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<(), DispatchError> {
        if let Some(is_user_orders_full) =
            data.is_user_limit_orders_full(&limit_order.owner, &self.order_book_id)
        {
            ensure!(
                !is_user_orders_full,
                Error::<T>::UserHasMaxCountOfOpenedOrders
            );
        }
        match limit_order.side {
            PriceVariant::Buy => {
                if let Some(is_bids_full) =
                    data.is_bids_full(&self.order_book_id, &limit_order.price)
                {
                    ensure!(!is_bids_full, Error::<T>::PriceReachedMaxCountOfLimitOrders);
                } else {
                    // there are no orders for the price, thus no entry for the given price.
                    // if there was an entry, we won't need to add another price and, therefore,
                    // check the length
                    ensure!(
                        data.get_aggregated_bids_len(&self.order_book_id)
                            .unwrap_or(0)
                            < T::MaxSidePriceCount::get() as usize,
                        Error::<T>::OrderBookReachedMaxCountOfPricesForSide
                    );
                }

                if let Some((best_bid_price, _)) = self.best_bid(data) {
                    if limit_order.price < best_bid_price {
                        let diff = best_bid_price
                            .balance()
                            .abs_diff(*limit_order.price.balance());
                        ensure!(
                            diff <= T::MAX_PRICE_SHIFT * (*best_bid_price.balance()),
                            Error::<T>::InvalidLimitOrderPrice
                        );
                    }
                }
            }
            PriceVariant::Sell => {
                if let Some(is_asks_full) =
                    data.is_asks_full(&self.order_book_id, &limit_order.price)
                {
                    ensure!(!is_asks_full, Error::<T>::PriceReachedMaxCountOfLimitOrders);
                } else {
                    // there are no orders for the price, thus no entry for the given price.
                    // if there was an entry, we won't need to add another price and, therefore,
                    // check the length
                    ensure!(
                        data.get_aggregated_asks_len(&self.order_book_id)
                            .unwrap_or(0)
                            < T::MaxSidePriceCount::get() as usize,
                        Error::<T>::OrderBookReachedMaxCountOfPricesForSide
                    );
                }

                if let Some((best_ask_price, _)) = self.best_ask(data) {
                    if limit_order.price > best_ask_price {
                        let diff = best_ask_price
                            .balance()
                            .abs_diff(*limit_order.price.balance());
                        ensure!(
                            diff <= T::MAX_PRICE_SHIFT * (*best_ask_price.balance()),
                            Error::<T>::InvalidLimitOrderPrice
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub fn best_bid(&self, data: &mut impl DataLayer<T>) -> Option<(OrderPrice, OrderVolume)> {
        data.best_bid(&self.order_book_id)
    }

    pub fn best_ask(&self, data: &mut impl DataLayer<T>) -> Option<(OrderPrice, OrderVolume)> {
        data.best_ask(&self.order_book_id)
    }

    fn market_volume(&self, side: PriceVariant, data: &mut impl DataLayer<T>) -> OrderVolume {
        let volume = match side {
            PriceVariant::Buy => {
                let bids = data.get_aggregated_bids(&self.order_book_id);
                bids.iter().fold(OrderVolume::zero(), |sum, (_, volume)| {
                    sum.saturating_add(*volume)
                })
            }
            PriceVariant::Sell => {
                let asks = data.get_aggregated_asks(&self.order_book_id);
                asks.iter().fold(OrderVolume::zero(), |sum, (_, volume)| {
                    sum.saturating_add(*volume)
                })
            }
        };

        volume
    }

    pub fn cross_spread<'a>(
        &self,
        limit_order: LimitOrder<T>,
        data: &mut impl DataLayer<T>,
    ) -> Result<
        MarketChange<T::AccountId, T::AssetId, T::DEXId, T::OrderId, LimitOrder<T>>,
        DispatchError,
    > {
        let (mut market_amount, mut limit_amount) = match limit_order.side {
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

        if limit_amount < self.min_lot_size {
            let market_volume = self.market_volume(limit_order.side.switched(), data);
            if market_volume
                .checked_sub(&market_amount)
                .ok_or(Error::<T>::AmountCalculationFailed)?
                >= limit_amount
            {
                market_amount = market_amount
                    .checked_add(&limit_amount)
                    .ok_or(Error::<T>::AmountCalculationFailed)?;
                limit_amount = OrderVolume::zero();
            } else {
                limit_amount = OrderVolume::zero();
            }
        }

        let mut market_change = MarketChange::new(self.order_book_id);

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

        if !limit_amount.is_zero() {
            let mut new_limit_order = limit_order.clone();
            new_limit_order.amount = limit_amount;
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
                market_amount = market_amount.saturating_add(*volume);
                amount = amount.saturating_sub(*volume);
            } else {
                market_amount = market_amount.saturating_add(amount);
                amount = OrderVolume::zero();
            }

            if amount.is_zero() {
                break;
            }
        }

        (market_amount, amount)
    }
}
