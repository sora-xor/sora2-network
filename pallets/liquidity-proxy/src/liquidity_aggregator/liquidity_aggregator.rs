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

#![cfg(feature = "wip")] // ALT

use crate::liquidity_aggregator::aggregation::Aggregation;
use crate::liquidity_aggregator::aggregation_result::{AggregationResult, SwapInfo};
use crate::liquidity_aggregator::selector::Selector;
use crate::{Config, Error};
use common::alt::{DiscreteQuotation, SideAmount, SwapChunk};
use common::prelude::{OutcomeFee, SwapAmount, SwapVariant};
use common::{AssetIdOf, Balance};
use frame_support::traits::Get;
use sp_runtime::traits::Zero;
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

/// Liquidity Aggregator selects and align the best chunks of liquidity from different sources to gain the best exchange result.
#[derive(Clone)]
pub struct LiquidityAggregator<T: Config, LiquiditySourceType> {
    variant: SwapVariant,
    selector: Selector<T, LiquiditySourceType>,
    aggregation: Aggregation<T, LiquiditySourceType>,
    origin_amount: Balance,
}

impl<T, LiquiditySourceType> LiquidityAggregator<T, LiquiditySourceType>
where
    T: Config,
    LiquiditySourceType: Ord + Clone,
{
    pub fn new(variant: SwapVariant) -> Self {
        Self {
            variant,
            selector: Selector::new(variant),
            aggregation: Aggregation::new(),
            origin_amount: Balance::zero(),
        }
    }

    pub fn add_source(
        &mut self,
        source: LiquiditySourceType,
        discrete_quotation: DiscreteQuotation<AssetIdOf<T>, Balance>,
    ) {
        self.selector.add_source(source, discrete_quotation)
    }

    /// Aggregates the liquidity from the provided liquidity sources.
    /// Liquidity sources provide discretized liquidity curve by chunks and then Liquidity Aggregator selects the best chunks from different sources to gain the best swap amount.
    pub fn aggregate_liquidity(
        mut self,
        amount: Balance,
    ) -> Result<AggregationResult<AssetIdOf<T>, LiquiditySourceType, Balance>, DispatchError> {
        self.origin_amount = amount;

        if self.selector.is_empty() {
            return Err(Error::<T>::InsufficientLiquidity.into());
        }

        self.aggregate_amount(amount)?;

        // max & precision limits are taken into account during the main aggregation
        // min limit requires the separate process
        self.align_min()?;

        self.calculate_result()
    }

    /// Aggregates the liquidity until it reaches the target `amount`.
    fn aggregate_amount(&mut self, mut amount: Balance) -> Result<(), DispatchError> {
        while amount > Balance::zero() {
            let (source, chunk) = self.selector.select_chunk(amount, &self.aggregation)?;

            let (chunk, new_amount) = self.fit_chunk(chunk, &source, amount)?;

            // there is a case when `fit_chunk` can edit the target amount
            // this change should not exceed the allowed slippage
            if new_amount != amount {
                let diff = amount.abs_diff(new_amount);

                if diff > T::InternalSlippageTolerance::get() * self.origin_amount {
                    // diff exceeds the allowed slippage
                    return Err(Error::<T>::InsufficientLiquidity.into());
                }

                if new_amount.is_zero() {
                    break;
                }

                amount = new_amount
            }

            if chunk.is_zero() {
                continue;
            }

            let delta = *chunk.get_associated_field(self.variant).amount();

            self.aggregation.push_chunk(source.clone(), chunk);
            amount = amount
                .checked_sub(delta)
                .ok_or(Error::<T>::CalculationError)?;
        }
        Ok(())
    }

    /// Change the `chunk` if it's necessary.
    /// Rescale the `chunk` if it exceeds the max amount for its source (if there is such limit for this source).
    /// Rescale the `chunk` if adding this chunk will exceed the necessary `amount`.
    /// Rescale the `chunk` if it doesn't match the precision limit.
    /// Return another `amount` if it's necessary.
    fn fit_chunk(
        &mut self,
        mut chunk: SwapChunk<AssetIdOf<T>, Balance>,
        source: &LiquiditySourceType,
        mut amount: Balance,
    ) -> Result<(SwapChunk<AssetIdOf<T>, Balance>, Balance), DispatchError> {
        let limits = self.selector.get_limits(&source).cloned()?;

        let mut refund = SwapChunk::zero();

        let total = self.aggregation.get_total(source);
        let (aligned, remainder) = limits
            .align_extra_chunk_max(total, chunk.clone())
            .ok_or(Error::<T>::CalculationError)?;
        if !remainder.is_zero() {
            // max amount (already selected + new chunk) exceeded
            chunk = aligned;
            refund = remainder;
            self.selector.lock_source(source.clone());
        }

        if !chunk.is_zero() {
            let step = limits
                .get_precision_step(&chunk, self.variant)
                .ok_or(Error::<T>::CalculationError)?;

            if amount < step {
                // This case means that this is the last available source,
                // it has precision limitation and `amount` doesn't match the precision.
                // We have to round the `amount`.
                match self.variant {
                    SwapVariant::WithDesiredInput => {
                        // round down
                        refund = refund.saturating_add(chunk);
                        chunk = SwapChunk::zero();
                        amount = Balance::zero();
                    }
                    SwapVariant::WithDesiredOutput => {
                        // round up
                        let precision = limits
                            .amount_precision
                            .ok_or(Error::<T>::AggregationError)?;
                        let rescaled = chunk
                            .clone()
                            .rescale_by_side_amount(precision)
                            .ok_or(Error::<T>::CalculationError)?;
                        refund =
                            refund.saturating_add(chunk.clone().saturating_sub(rescaled.clone()));
                        chunk = rescaled;
                        amount = step;
                    }
                }
            } else {
                // if `step` is not 0, it means the source has a precision limit
                // in this case the amount should be a multiple of the precision
                let side_amount = if !step.is_zero() && amount % step != Balance::zero() {
                    let count = amount.saturating_div(step);
                    let aligned = count.saturating_mul(step);
                    SideAmount::new(aligned, self.variant)
                } else {
                    SideAmount::new(amount, self.variant)
                };

                // if chunk is bigger than remaining amount, it is necessary to rescale it and take only required part
                if chunk > side_amount {
                    let rescaled = chunk
                        .clone()
                        .rescale_by_side_amount(side_amount)
                        .ok_or(Error::<T>::CalculationError)?;
                    refund = refund.saturating_add(chunk.clone().saturating_sub(rescaled.clone()));
                    chunk = rescaled;
                }

                let (aligned, reminder) = limits
                    .align_chunk_precision(chunk.clone())
                    .ok_or(Error::<T>::CalculationError)?;
                if !reminder.is_zero() {
                    chunk = aligned;
                    refund = refund.saturating_add(reminder);
                }
            }

            if chunk.is_zero() && !amount.is_zero() {
                // should never happen
                return Err(Error::<T>::AggregationError.into());
            }
        }

        if !refund.is_zero() {
            // push remains of the chunk back
            self.selector.push_chunk(&source, refund)?;
        }

        Ok((chunk, amount))
    }

    /// Align the selected aggregation in according with source min amount limits.
    fn align_min(&mut self) -> Result<(), DispatchError> {
        let mut to_delete = Vec::new();

        let queue = self.aggregation.get_total_price_ascending_queue();

        for source in queue {
            let mut cluster = self.aggregation.get_mut_cluster(&source)?;
            let limits = self.selector.get_limits(&source)?;

            let (_, remainder) = limits.align_chunk_min(cluster.get_total().clone());
            if !remainder.is_zero() {
                let min_amount = &limits.min_amount.ok_or(Error::<T>::AggregationError)?;
                let remainder = remainder.get_same_type_amount(min_amount);

                let (returned_liquidity, delete) =
                    self.selector
                        .return_liquidity(remainder, &source, &mut cluster)?;
                if delete {
                    to_delete.push(source.clone());
                }

                self.selector.lock_source(source.clone());

                let remaining_amount = *returned_liquidity
                    .get_associated_field(self.variant)
                    .amount();
                self.aggregate_amount(remaining_amount)?;
            }
        }

        self.aggregation
            .0
            .retain(|source, _| !to_delete.contains(source));

        Ok(())
    }

    fn calculate_result(
        &self,
    ) -> Result<AggregationResult<AssetIdOf<T>, LiquiditySourceType, Balance>, DispatchError> {
        let mut distribution = Vec::new();
        let mut swap_info: SwapInfo<LiquiditySourceType, Balance> = SwapInfo::new();
        let mut desired_amount = Balance::zero();
        let mut result_amount = Balance::zero();
        let mut fee = OutcomeFee::default();

        for (source, cluster) in &self.aggregation.0 {
            let total = cluster.get_total().clone();

            swap_info.insert(source.clone(), (total.input, total.output));

            let (desired_part, result_part) = match self.variant {
                SwapVariant::WithDesiredInput => (total.input, total.output),
                SwapVariant::WithDesiredOutput => (total.output, total.input),
            };
            distribution.push((
                source.clone(),
                SwapAmount::with_variant(self.variant, desired_part, result_part),
            ));
            desired_amount = desired_amount
                .checked_add(desired_part)
                .ok_or(Error::<T>::CalculationError)?;
            result_amount = result_amount
                .checked_add(result_part)
                .ok_or(Error::<T>::CalculationError)?;
            fee = fee.merge(total.fee);
        }

        Ok(AggregationResult {
            swap_info,
            distribution,
            desired_amount,
            result_amount,
            swap_variant: self.variant,
            fee,
        })
    }
}
