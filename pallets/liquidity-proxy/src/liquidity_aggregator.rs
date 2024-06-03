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

use codec::{Decode, Encode};
use common::prelude::{OutcomeFee, SwapAmount};
use frame_support::RuntimeDebug;
use sp_std::vec::Vec;

#[cfg(feature = "wip")] // ALT
use {
    crate::{Config, Error},
    common::alt::SwapLimits,
    common::alt::{AlignReason, DiscreteQuotation, SideAmount, SwapChunk},
    common::prelude::SwapVariant,
    common::AssetIdOf,
    common::{fixed, Balance},
    frame_support::traits::Get,
    itertools::Itertools,
    sp_runtime::traits::Zero,
    sp_runtime::DispatchError,
    sp_std::collections::btree_map::BTreeMap,
    sp_std::collections::btree_set::BTreeSet,
    sp_std::collections::vec_deque::VecDeque,
    sp_std::vec,
};

#[cfg(feature = "wip")] // ALT
/// Result of aggregation
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AggregationResult<AssetId: Ord, LiquiditySourceType, AmountType> {
    pub swap_info: SwapInfo<LiquiditySourceType, AmountType>,
    /// A distribution of amounts each liquidity sources gets to swap in the entire trade
    pub distribution: Vec<(LiquiditySourceType, SwapAmount<AmountType>)>,
    /// The best possible desired amount
    pub desired_amount: AmountType,
    /// The best possible result amount
    pub result_amount: AmountType,
    pub swap_variant: SwapVariant,
    /// Total fee amount, nominated in XOR
    pub fee: OutcomeFee<AssetId, AmountType>,
}

#[cfg(feature = "wip")] // ALT
impl<AssetId: Ord, LiquiditySourceType, AmountType>
    AggregationResult<AssetId, LiquiditySourceType, AmountType>
{
    pub fn new(
        swap_info: SwapInfo<LiquiditySourceType, AmountType>,
        distribution: Vec<(LiquiditySourceType, SwapAmount<AmountType>)>,
        desired_amount: AmountType,
        result_amount: AmountType,
        swap_variant: SwapVariant,
        fee: OutcomeFee<AssetId, AmountType>,
    ) -> Self {
        Self {
            swap_info,
            distribution,
            desired_amount,
            result_amount,
            swap_variant,
            fee,
        }
    }
}

/// Info with input & output amounts for liquidity source
#[cfg(feature = "wip")] // ALT
type SwapInfo<LiquiditySourceType, AmountType> =
    BTreeMap<LiquiditySourceType, (AmountType, AmountType)>;

/// Output of the aggregated LiquidityProxy::quote() price.
#[derive(
    Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, scale_info::TypeInfo,
)]
pub struct AggregatedSwapOutcome<AssetId: Ord, LiquiditySourceType, AmountType> {
    /// A distribution of amounts each liquidity sources gets to swap in the entire trade
    pub distribution: Vec<(LiquiditySourceType, SwapAmount<AmountType>)>,
    /// The best possible output/input amount for a given trade and a set of liquidity sources
    pub amount: AmountType,
    /// Total fee amount, nominated in XOR
    pub fee: OutcomeFee<AssetId, AmountType>,
}

impl<AssetId: Ord, LiquiditySourceIdType, AmountType>
    AggregatedSwapOutcome<AssetId, LiquiditySourceIdType, AmountType>
{
    pub fn new(
        distribution: Vec<(LiquiditySourceIdType, SwapAmount<AmountType>)>,
        amount: AmountType,
        fee: OutcomeFee<AssetId, AmountType>,
    ) -> Self {
        Self {
            distribution,
            amount,
            fee,
        }
    }
}

#[cfg(feature = "wip")] // ALT
impl<AssetId: Ord, LiquiditySourceType, AmountType>
    From<AggregationResult<AssetId, LiquiditySourceType, AmountType>>
    for AggregatedSwapOutcome<AssetId, LiquiditySourceType, AmountType>
{
    fn from(value: AggregationResult<AssetId, LiquiditySourceType, AmountType>) -> Self {
        AggregatedSwapOutcome {
            distribution: value.distribution,
            amount: value.result_amount,
            fee: value.fee,
        }
    }
}

#[cfg(feature = "wip")] // ALT
#[derive(Debug)]
struct Selector<T: Config, LiquiditySourceType> {
    variant: SwapVariant,
    liquidity_quotations: BTreeMap<LiquiditySourceType, DiscreteQuotation<AssetIdOf<T>, Balance>>,
    locked_sources: BTreeSet<LiquiditySourceType>,
}

#[cfg(feature = "wip")] // ALT
impl<T, LiquiditySourceType> Selector<T, LiquiditySourceType>
where
    T: Config,
    LiquiditySourceType: Ord + Clone,
{
    pub fn new(variant: SwapVariant) -> Self {
        Self {
            variant,
            liquidity_quotations: BTreeMap::new(),
            locked_sources: BTreeSet::new(),
        }
    }

    pub fn add_source(
        &mut self,
        source: LiquiditySourceType,
        discrete_quotation: DiscreteQuotation<AssetIdOf<T>, Balance>,
    ) {
        self.liquidity_quotations.insert(source, discrete_quotation);
    }

    pub fn is_empty(&self) -> bool {
        self.liquidity_quotations.is_empty()
    }

    pub fn lock_source(&mut self, source: LiquiditySourceType) {
        self.locked_sources.insert(source);
    }

    pub fn get_limits(
        &self,
        source: &LiquiditySourceType,
    ) -> Result<&SwapLimits<Balance>, DispatchError> {
        let limits = self
            .liquidity_quotations
            .get(source)
            .map(|quotation| &quotation.limits)
            .ok_or(Error::<T>::AggregationError)?;
        Ok(limits)
    }

    pub fn push_chunk(
        &mut self,
        source: &LiquiditySourceType,
        chunk: SwapChunk<AssetIdOf<T>, Balance>,
    ) -> Result<(), DispatchError> {
        let quotation = self
            .liquidity_quotations
            .get_mut(&source)
            .ok_or(Error::<T>::AggregationError)?;
        quotation.chunks.push_front(chunk);
        Ok(())
    }

    pub fn payback(
        &mut self,
        mut amount: SideAmount<Balance>,
        source: &LiquiditySourceType,
        chunks: &mut VecDeque<SwapChunk<AssetIdOf<T>, Balance>>,
    ) -> Result<bool, DispatchError> {
        let mut delete = false;
        while *amount.amount() > Balance::zero() {
            // it is necessary to return chunks back till `remainder` volume is filled
            let Some(chunk) = chunks.pop_back() else {
                break;
            };
            if chunk <= amount {
                let value = amount
                    .amount()
                    .checked_sub(*chunk.get_associated_field(self.variant).amount())
                    .ok_or(Error::<T>::CalculationError)?;
                amount.set_amount(value);
                self.push_chunk(source, chunk)?;
            } else {
                let remainder_chunk = chunk
                    .clone()
                    .rescale_by_side_amount(amount)
                    .ok_or(Error::<T>::CalculationError)?;
                let chunk = chunk.saturating_sub(remainder_chunk.clone());
                chunks.push_back(chunk);
                self.push_chunk(source, remainder_chunk)?;
                amount.set_amount(Balance::zero());
            }
        }

        if chunks.is_empty() {
            // chunks are over, already returned all chunks
            delete = true;
        }
        Ok(delete)
    }

    /// Selects the chunk with best price.
    /// If there are several best chunks, we select the source that already was selected before.
    pub fn select_chunk(
        &mut self,
        aggregation: &BTreeMap<LiquiditySourceType, VecDeque<SwapChunk<AssetIdOf<T>, Balance>>>,
    ) -> Result<(LiquiditySourceType, SwapChunk<AssetIdOf<T>, Balance>), DispatchError> {
        let mut candidates = Vec::new();
        let mut max = fixed!(0);

        for (source, discrete_quotation) in self.liquidity_quotations.iter() {
            // skip the locked source
            if self.locked_sources.contains(source) {
                continue;
            }

            // skip the empty source
            let Some(front) = discrete_quotation.chunks.front() else {
                continue;
            };

            let price = front.price().ok_or(Error::<T>::CalculationError)?;

            if price == max {
                candidates.push(source.clone())
            }

            if price > max {
                candidates.clear();
                max = price;
                candidates.push(source.clone());
            }
        }

        let mut source = candidates
            .first()
            .ok_or(Error::<T>::InsufficientLiquidity)?;

        // if there are several candidates with the same best price,
        // then we need to select the source that already been selected
        for candidate in candidates.iter() {
            if aggregation.keys().contains(&candidate) {
                source = candidate;
                break;
            }
        }

        let chunk = self
            .liquidity_quotations
            .get_mut(&source)
            .ok_or(Error::<T>::AggregationError)?
            .chunks
            .pop_front()
            .ok_or(Error::<T>::AggregationError)?;

        Ok((source.clone(), chunk))
    }

    /// Similar with `select_chunk` with considering the `remaining_amount` and precision limit.
    /// If `remaining_amount` < source precision and there are no other availabe sources, then we take this source and calculate the new remaining amount.
    pub fn select_remaining_chunk(
        &mut self,
        aggregation: &BTreeMap<LiquiditySourceType, VecDeque<SwapChunk<AssetIdOf<T>, Balance>>>,
        remaining_amount: Balance,
    ) -> Result<
        (
            LiquiditySourceType,
            SwapChunk<AssetIdOf<T>, Balance>,
            Option<Balance>,
        ),
        DispatchError,
    > {
        let mut candidates = Vec::new();
        let mut max = fixed!(0);

        // Special list for sources with precision limit.
        // If `candidates` is empty we will take the chunk from the `wait_list`
        let mut wait_list = Vec::new();

        for (source, discrete_quotation) in self.liquidity_quotations.iter() {
            // skip the locked source
            if self.locked_sources.contains(source) {
                continue;
            }

            // skip the empty source
            let Some(front) = discrete_quotation.chunks.front() else {
                continue;
            };

            let price = front.price().ok_or(Error::<T>::CalculationError)?;

            if let Some(precision) = discrete_quotation.limits.amount_precision {
                let new_remaining_amount = match (self.variant, precision) {
                    (SwapVariant::WithDesiredInput, SideAmount::Input(value)) => {
                        if remaining_amount < value {
                            // round down
                            Some(Balance::zero())
                        } else {
                            None
                        }
                    }
                    (SwapVariant::WithDesiredOutput, SideAmount::Output(value)) => {
                        if remaining_amount < value {
                            // round up
                            Some(value)
                        } else {
                            None
                        }
                    }
                    (SwapVariant::WithDesiredInput, SideAmount::Output(value)) => {
                        let remaining_output = front
                            .proportional_output(remaining_amount)
                            .ok_or(Error::<T>::CalculationError)?;
                        if remaining_output < value {
                            // round down
                            Some(Balance::zero())
                        } else {
                            None
                        }
                    }
                    (SwapVariant::WithDesiredOutput, SideAmount::Input(value)) => {
                        let remaining_input = front
                            .proportional_input(remaining_amount)
                            .ok_or(Error::<T>::CalculationError)?;
                        if remaining_input < value {
                            // round up
                            Some(
                                front
                                    .proportional_output(value)
                                    .ok_or(Error::<T>::CalculationError)?,
                            )
                        } else {
                            None
                        }
                    }
                };

                if new_remaining_amount.is_some() {
                    wait_list.push((source.clone(), price, new_remaining_amount));
                    continue;
                }
            }

            if price == max {
                candidates.push(source.clone())
            }

            if price > max {
                candidates.clear();
                max = price;
                candidates.push(source.clone());
            }
        }

        let (source, new_remaining_amount) = if let Some(mut source) = candidates.first().cloned() {
            // if there are several candidates with the same best price,
            // then we need to select the source that already been selected
            for candidate in candidates.into_iter() {
                if aggregation.keys().contains(&candidate) {
                    source = candidate;
                    break;
                }
            }
            (source, None)
        } else {
            let (source, _, new_remaining_amount) = wait_list
                .into_iter()
                .fold(None, |max, (source, price, new_remaining_amount)| {
                    if let Some((max_source, max_price, max_new_remaining_amount)) = max {
                        if price > max_price {
                            Some((source, price, new_remaining_amount))
                        } else {
                            Some((max_source, max_price, max_new_remaining_amount))
                        }
                    } else {
                        Some((source, price, new_remaining_amount))
                    }
                })
                .ok_or(Error::<T>::InsufficientLiquidity)?;
            (source, new_remaining_amount)
        };

        let chunk = self
            .liquidity_quotations
            .get_mut(&source)
            .ok_or(Error::<T>::AggregationError)?
            .chunks
            .pop_front()
            .ok_or(Error::<T>::AggregationError)?;

        Ok((source.clone(), chunk, new_remaining_amount))
    }
}

#[cfg(feature = "wip")] // ALT
pub struct LiquidityAggregator<T: Config, LiquiditySourceType> {
    variant: SwapVariant,
    selector: Selector<T, LiquiditySourceType>,
    aggregation: BTreeMap<LiquiditySourceType, VecDeque<SwapChunk<AssetIdOf<T>, Balance>>>,
}

#[cfg(feature = "wip")] // ALT
impl<T, LiquiditySourceType> LiquidityAggregator<T, LiquiditySourceType>
where
    T: Config,
    LiquiditySourceType: Ord + Clone,
{
    pub fn new(variant: SwapVariant) -> Self {
        Self {
            variant,
            selector: Selector::new(variant),
            aggregation: BTreeMap::new(),
        }
    }

    pub fn add_source(
        &mut self,
        source: LiquiditySourceType,
        discrete_quotation: DiscreteQuotation<AssetIdOf<T>, Balance>,
    ) {
        self.selector.add_source(source, discrete_quotation);
    }

    /// Aggregates the liquidity from the provided liquidity sources.
    /// Liquidity sources provide discretized liquidity curve by chunks and then Liquidity Aggregator selects the best chunks from different sources to gain the best swap amount.
    pub fn aggregate_liquidity(
        mut self,
        amount: Balance,
    ) -> Result<AggregationResult<AssetIdOf<T>, LiquiditySourceType, Balance>, DispatchError> {
        if self.selector.is_empty() {
            return Err(Error::<T>::InsufficientLiquidity.into());
        }

        self.aggregate_main_amount(amount)?;
        let remaining_amount = self.align_aggregation()?;
        if !remaining_amount.is_zero() {
            self.aggregate_remaining_amount(remaining_amount, amount)?;
        }

        self.calculate_result()
    }

    /// Aggregates the main liquidity amount.
    fn aggregate_main_amount(&mut self, mut amount: Balance) -> Result<(), DispatchError> {
        while amount > Balance::zero() {
            let (source, chunk) = self.selector.select_chunk(&self.aggregation)?;

            let chunk = self.fit_chunk(chunk, &source, amount)?;

            if chunk.is_zero() {
                continue;
            }

            let delta = *chunk.get_associated_field(self.variant).amount();

            self.aggregation
                .entry(source.clone())
                .and_modify(|chunks: &mut VecDeque<SwapChunk<AssetIdOf<T>, Balance>>| {
                    chunks.push_back(chunk.clone())
                })
                .or_insert(vec![chunk].into());
            amount = amount
                .checked_sub(delta)
                .ok_or(Error::<T>::CalculationError)?;
        }
        Ok(())
    }

    /// Aggregates the remaining liquidity after the alignment of selected aggregation.
    fn aggregate_remaining_amount(
        &mut self,
        mut remaining_amount: Balance,
        origin_amount: Balance,
    ) -> Result<(), DispatchError> {
        while remaining_amount > Balance::zero() {
            let (source, chunk, new_remaining_amount) = self
                .selector
                .select_remaining_chunk(&self.aggregation, remaining_amount)?;

            if let Some(new_remaining_amount) = new_remaining_amount {
                let diff = remaining_amount.abs_diff(new_remaining_amount);

                if diff > T::InternalSlippageTolerance::get() * origin_amount {
                    // diff exceeds the allowed slippage
                    return Err(Error::<T>::InsufficientLiquidity.into());
                }

                if new_remaining_amount.is_zero() {
                    break;
                }

                remaining_amount = new_remaining_amount
            }

            let chunk = self.fit_chunk(chunk, &source, remaining_amount)?;

            if chunk.is_zero() {
                continue;
            }

            let remaining_delta = *chunk.get_associated_field(self.variant).amount();

            self.aggregation
                .entry(source.clone())
                .and_modify(|chunks: &mut VecDeque<SwapChunk<AssetIdOf<T>, Balance>>| {
                    chunks.push_back(chunk.clone())
                })
                .or_insert(vec![chunk].into());
            remaining_amount = remaining_amount
                .checked_sub(remaining_delta)
                .ok_or(Error::<T>::CalculationError)?;

            if remaining_amount.is_zero() {
                remaining_amount = self.align_aggregation()?;
            }
        }

        Ok(())
    }

    /// Change the `chunk` if it's necessary.
    /// Rescale the `chunk` if it exceeds the max amount for its source (if there is such limit for this source).
    /// Rescale the `chunk` if adding this chunk will exceed the necessary `amount`
    fn fit_chunk(
        &mut self,
        mut chunk: SwapChunk<AssetIdOf<T>, Balance>,
        source: &LiquiditySourceType,
        amount: Balance,
    ) -> Result<SwapChunk<AssetIdOf<T>, Balance>, DispatchError> {
        let limits = self.selector.get_limits(&source)?;

        let mut payback = SwapChunk::zero();

        let total = Self::sum_chunks(self.aggregation.entry(source.clone()).or_default());
        let (aligned, remainder) = limits
            .align_extra_chunk_max(total.clone(), chunk.clone())
            .ok_or(Error::<T>::CalculationError)?;
        if !remainder.is_zero() {
            // max amount (already selected + new chunk) exceeded
            chunk = aligned;
            payback = remainder;
            self.selector.lock_source(source.clone());
        }

        let side_amount = SideAmount::new(amount, self.variant);
        if chunk > side_amount {
            let rescaled = chunk
                .clone()
                .rescale_by_side_amount(side_amount)
                .ok_or(Error::<T>::CalculationError)?;
            payback = payback.saturating_add(chunk.clone().saturating_sub(rescaled.clone()));
            chunk = rescaled;
        }

        if !payback.is_zero() {
            // push remains of the chunk back
            self.selector.push_chunk(&source, payback)?;
        }

        Ok(chunk)
    }

    /// Align the selected aggregation in according with source limits.
    fn align_aggregation(&mut self) -> Result<Balance, DispatchError> {
        let mut remaining_amount = Balance::zero();
        let mut to_delete = Vec::new();
        for (source, mut chunks) in self.aggregation.iter_mut() {
            let total = Self::sum_chunks(chunks);

            let limits = self.selector.get_limits(&source)?;

            let (_, remainder, align_reason) = limits
                .align_chunk(total)
                .ok_or(Error::<T>::CalculationError)?;

            match align_reason {
                AlignReason::NotAligned => {} // do nothing
                AlignReason::Min | AlignReason::Max => {
                    remaining_amount = remaining_amount
                        .checked_add(*remainder.get_associated_field(self.variant).amount())
                        .ok_or(Error::<T>::CalculationError)?;

                    self.selector.lock_source(source.clone());

                    let remainder = remainder.get_associated_field(self.variant);
                    if self.selector.payback(remainder, &source, &mut chunks)? {
                        to_delete.push(source.clone());
                    }
                }
                AlignReason::Precision => {
                    remaining_amount = remaining_amount
                        .checked_add(*remainder.get_associated_field(self.variant).amount())
                        .ok_or(Error::<T>::CalculationError)?;

                    let remainder = remainder.get_associated_field(self.variant);
                    if self.selector.payback(remainder, &source, &mut chunks)? {
                        to_delete.push(source.clone());
                    }
                }
            }
        }
        self.aggregation
            .retain(|source, _| !to_delete.contains(source));

        Ok(remaining_amount)
    }

    fn calculate_result(
        &self,
    ) -> Result<AggregationResult<AssetIdOf<T>, LiquiditySourceType, Balance>, DispatchError> {
        let mut distribution = Vec::new();
        let mut swap_info: SwapInfo<LiquiditySourceType, Balance> = SwapInfo::new();
        let mut desired_amount = Balance::zero();
        let mut result_amount = Balance::zero();
        let mut fee = OutcomeFee::default();

        for (source, chunks) in &self.aggregation {
            let total = Self::sum_chunks(chunks);

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

    fn sum_chunks(
        chunks: &VecDeque<SwapChunk<AssetIdOf<T>, Balance>>,
    ) -> SwapChunk<AssetIdOf<T>, Balance> {
        chunks
            .iter()
            .fold(SwapChunk::<AssetIdOf<T>, Balance>::zero(), |acc, next| {
                acc.saturating_add(next.clone())
            })
    }
}

#[cfg(feature = "wip")] // ALT
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::Runtime;
    use crate::Error;
    use common::alt::{DiscreteQuotation, SideAmount, SwapChunk, SwapLimits};
    use common::prelude::{OutcomeFee, SwapAmount, SwapVariant};
    use common::{balance, LiquiditySourceType, XOR, XST};
    use frame_support::assert_err;
    use sp_std::collections::vec_deque::VecDeque;

    #[test]
    fn check_select_chunk() {
        let push_chunk = {
            |aggregation: &mut BTreeMap<
                LiquiditySourceType,
                VecDeque<SwapChunk<AssetIdOf<Runtime>, Balance>>,
            >,
             source: LiquiditySourceType,
             chunk: SwapChunk<AssetIdOf<Runtime>, Balance>| {
                aggregation
                    .entry(source)
                    .and_modify(|chunks| chunks.push_back(chunk.clone()))
                    .or_insert(vec![chunk].into());
            }
        };

        let mut selector: Selector<Runtime, _> = Selector::new(SwapVariant::WithDesiredInput);
        selector.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                ]),
                limits: Default::default(),
            },
        );
        selector.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
            },
        );
        selector.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(120), Default::default()),
                    SwapChunk::new(balance!(10), balance!(100), Default::default()),
                    SwapChunk::new(balance!(10), balance!(87), Default::default()),
                    SwapChunk::new(balance!(10), balance!(80), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(1))),
                    Some(SideAmount::Input(balance!(1000))),
                    Some(SideAmount::Input(balance!(0.1))),
                ),
            },
        );

        let mut aggregation = BTreeMap::new();

        // select Order Book because it has the best price
        let (source, chunk) = selector.select_chunk(&mut aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(120), Default::default())
        );
        push_chunk(&mut aggregation, source, chunk);

        // select Order Book instead of XYK pool because it was already selected
        let (source, chunk) = selector.select_chunk(&mut aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(100), Default::default())
        );
        push_chunk(&mut aggregation, source, chunk);

        // just take the best price in all cases below

        let (source, chunk) = selector.select_chunk(&mut aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XYKPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1)))
        );
        push_chunk(&mut aggregation, source, chunk);

        let (source, chunk) = selector.select_chunk(&mut aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XYKPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9)))
        );
        push_chunk(&mut aggregation, source, chunk);

        let (source, chunk) = selector.select_chunk(&mut aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(87), Default::default()),
        );
        push_chunk(&mut aggregation, source, chunk);

        let (source, chunk) = selector.select_chunk(&mut aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XSTPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85)))
        );
        push_chunk(&mut aggregation, source, chunk);

        let (source, chunk) = selector.select_chunk(&mut aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XSTPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85)))
        );
        push_chunk(&mut aggregation, source, chunk);

        let (source, chunk) = selector.select_chunk(&mut aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(80), Default::default()),
        );
        push_chunk(&mut aggregation, source, chunk);

        // liquidity is empty
        assert_err!(
            selector.select_chunk(&mut aggregation),
            Error::<Runtime>::InsufficientLiquidity
        );
    }

    #[test]
    fn check_select_remaining_chunk() {
        let push_chunk = {
            |aggregation: &mut BTreeMap<
                LiquiditySourceType,
                VecDeque<SwapChunk<AssetIdOf<Runtime>, Balance>>,
            >,
             source: LiquiditySourceType,
             chunk: SwapChunk<AssetIdOf<Runtime>, Balance>| {
                aggregation
                    .entry(source)
                    .and_modify(|chunks| chunks.push_back(chunk.clone()))
                    .or_insert(vec![chunk].into());
            }
        };

        let mut selector: Selector<Runtime, _> = Selector::new(SwapVariant::WithDesiredInput);
        selector.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                ]),
                limits: Default::default(),
            },
        );
        selector.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
            },
        );
        selector.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(120), Default::default()),
                    SwapChunk::new(balance!(10), balance!(100), Default::default()),
                    SwapChunk::new(balance!(10), balance!(87), Default::default()),
                    SwapChunk::new(balance!(10), balance!(80), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(1))),
                    Some(SideAmount::Input(balance!(1000))),
                    Some(SideAmount::Input(balance!(0.1))),
                ),
            },
        );

        let mut aggregation = BTreeMap::new();

        // select Order Book because it has the best price
        let (source, chunk, amount) = selector
            .select_remaining_chunk(&mut aggregation, balance!(10))
            .unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(120), Default::default())
        );
        assert_eq!(amount, None);
        push_chunk(&mut aggregation, source, chunk);

        // select Order Book instead of XYK pool because it was already selected
        let (source, chunk, amount) = selector
            .select_remaining_chunk(&mut aggregation, balance!(10))
            .unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(100), Default::default())
        );
        assert_eq!(amount, None);
        push_chunk(&mut aggregation, source, chunk);

        // just take the best price
        let (source, chunk, amount) = selector
            .select_remaining_chunk(&mut aggregation, balance!(10))
            .unwrap();
        assert_eq!(source, LiquiditySourceType::XYKPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1)))
        );
        assert_eq!(amount, None);
        push_chunk(&mut aggregation, source, chunk);

        // just take the best price
        let (source, chunk, amount) = selector
            .select_remaining_chunk(&mut aggregation, balance!(10))
            .unwrap();
        assert_eq!(source, LiquiditySourceType::XYKPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9)))
        );
        assert_eq!(amount, None);
        push_chunk(&mut aggregation, source, chunk);

        // take XST instead of Order Book because remaining amount < precision, but there are others candidates without precision limit
        let (source, chunk, amount) = selector
            .select_remaining_chunk(&mut aggregation, balance!(0.05))
            .unwrap();
        assert_eq!(source, LiquiditySourceType::XSTPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85)))
        );
        assert_eq!(amount, None);
        push_chunk(&mut aggregation, source, chunk);

        // just take the best price
        let (source, chunk, amount) = selector
            .select_remaining_chunk(&mut aggregation, balance!(10))
            .unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(87), Default::default()),
        );
        assert_eq!(amount, None);
        push_chunk(&mut aggregation, source, chunk);

        // just take the best price
        let (source, chunk, amount) = selector
            .select_remaining_chunk(&mut aggregation, balance!(10))
            .unwrap();
        assert_eq!(source, LiquiditySourceType::XSTPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85)))
        );
        assert_eq!(amount, None);
        push_chunk(&mut aggregation, source, chunk);

        // take Order Book despite remaining amount < precision, because it is only one available source
        let (source, chunk, amount) = selector
            .select_remaining_chunk(&mut aggregation, balance!(0.05))
            .unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(80), Default::default()),
        );
        assert_eq!(amount.unwrap(), balance!(0));
        push_chunk(&mut aggregation, source, chunk);

        // liquidity is empty
        assert_err!(
            selector.select_remaining_chunk(&mut aggregation, balance!(10)),
            Error::<Runtime>::InsufficientLiquidity
        );
    }

    #[test]
    fn check_new_remaining_amount_from_select_remaining_chunk() {
        let amount = balance!(0.05); // less than precision
        let mut aggregation = BTreeMap::new();
        let origin_chunk = SwapChunk::new(balance!(10), balance!(120), Default::default());

        // WithDesiredInput & Input precision
        let mut selector: Selector<Runtime, _> = Selector::new(SwapVariant::WithDesiredInput);
        selector.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([origin_chunk.clone()]),
                limits: SwapLimits::new(None, None, Some(SideAmount::Input(balance!(0.1)))),
            },
        );

        let (source, chunk, new_remaining_amount) = selector
            .select_remaining_chunk(&mut aggregation, amount)
            .unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(chunk, origin_chunk);
        assert_eq!(new_remaining_amount.unwrap(), balance!(0));

        // WithDesiredOutput & Output precision
        let mut selector: Selector<Runtime, _> = Selector::new(SwapVariant::WithDesiredOutput);
        selector.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([origin_chunk.clone()]),
                limits: SwapLimits::new(None, None, Some(SideAmount::Output(balance!(0.1)))),
            },
        );

        let (source, chunk, new_remaining_amount) = selector
            .select_remaining_chunk(&mut aggregation, amount)
            .unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(chunk, origin_chunk);
        assert_eq!(new_remaining_amount.unwrap(), balance!(0.1));

        // WithDesiredInput & Output precision
        let mut selector: Selector<Runtime, _> = Selector::new(SwapVariant::WithDesiredInput);
        selector.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([origin_chunk.clone()]),
                limits: SwapLimits::new(None, None, Some(SideAmount::Output(balance!(10)))),
            },
        );

        let (source, chunk, new_remaining_amount) = selector
            .select_remaining_chunk(&mut aggregation, amount)
            .unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(chunk, origin_chunk);
        assert_eq!(new_remaining_amount.unwrap(), balance!(0));

        // WithDesiredOutput & Input precision
        let mut selector: Selector<Runtime, _> = Selector::new(SwapVariant::WithDesiredOutput);
        selector.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([origin_chunk.clone()]),
                limits: SwapLimits::new(None, None, Some(SideAmount::Input(balance!(0.1)))),
            },
        );

        let (source, chunk, new_remaining_amount) = selector
            .select_remaining_chunk(&mut aggregation, amount)
            .unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(chunk, origin_chunk);
        assert_eq!(new_remaining_amount.unwrap(), balance!(1.2));
    }

    fn get_liquidity_aggregator_with_desired_input_and_equal_chunks(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);
        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                    SwapChunk::new(balance!(10), balance!(80), OutcomeFee::xor(balance!(0.8))),
                    SwapChunk::new(balance!(10), balance!(70), OutcomeFee::xor(balance!(0.7))),
                    SwapChunk::new(balance!(10), balance!(60), OutcomeFee::xor(balance!(0.6))),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(120), Default::default()),
                    SwapChunk::new(balance!(10), balance!(100), Default::default()),
                    SwapChunk::new(balance!(10), balance!(80), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(1))),
                    Some(SideAmount::Input(balance!(1000))),
                    Some(SideAmount::Input(balance!(0.00001))),
                ),
            },
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_output_and_equal_chunks(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(11), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(12), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(13), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(14), balance!(100), OutcomeFee::xor(balance!(1))),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1000000))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(8), balance!(100), Default::default()),
                    SwapChunk::new(balance!(10), balance!(100), Default::default()),
                    SwapChunk::new(balance!(13), balance!(100), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(1))),
                    Some(SideAmount::Input(balance!(1000))),
                    Some(SideAmount::Input(balance!(0.00001))),
                ),
            },
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_input_and_different_chunks(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);
        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(12), balance!(108), OutcomeFee::xor(balance!(1.08))),
                    SwapChunk::new(balance!(14), balance!(112), OutcomeFee::xor(balance!(1.12))),
                    SwapChunk::new(balance!(16), balance!(112), OutcomeFee::xor(balance!(1.12))),
                    SwapChunk::new(balance!(18), balance!(108), OutcomeFee::xor(balance!(1.08))),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(
                        balance!(11),
                        balance!(93.5),
                        OutcomeFee::xst(balance!(0.935)),
                    ),
                    SwapChunk::new(balance!(12), balance!(102), OutcomeFee::xst(balance!(1.02))),
                    SwapChunk::new(
                        balance!(13),
                        balance!(110.5),
                        OutcomeFee::xst(balance!(1.105)),
                    ),
                    SwapChunk::new(balance!(14), balance!(119), OutcomeFee::xst(balance!(1.19))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(12), balance!(144), Default::default()),
                    SwapChunk::new(balance!(10), balance!(100), Default::default()),
                    SwapChunk::new(balance!(14), balance!(112), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(1))),
                    Some(SideAmount::Input(balance!(1000))),
                    Some(SideAmount::Input(balance!(0.00001))),
                ),
            },
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_output_and_different_chunks(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), Default::default()),
                    SwapChunk::new(balance!(5.5), balance!(50), Default::default()),
                    SwapChunk::new(balance!(3), balance!(25), Default::default()),
                    SwapChunk::new(balance!(26), balance!(200), Default::default()),
                    SwapChunk::new(balance!(7), balance!(50), Default::default()),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(12.5), balance!(100), Default::default()),
                    SwapChunk::new(balance!(10), balance!(80), Default::default()),
                    SwapChunk::new(balance!(9), balance!(72), Default::default()),
                    SwapChunk::new(balance!(8), balance!(64), Default::default()),
                    SwapChunk::new(balance!(7), balance!(56), Default::default()),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1000000))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(8), balance!(100), Default::default()),
                    SwapChunk::new(balance!(9), balance!(90), Default::default()),
                    SwapChunk::new(balance!(13), balance!(100), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(1))),
                    Some(SideAmount::Input(balance!(1000))),
                    Some(SideAmount::Input(balance!(0.00001))),
                ),
            },
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_input_and_max_amount_limits(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                    SwapChunk::new(balance!(10), balance!(80), OutcomeFee::xor(balance!(0.8))),
                    SwapChunk::new(balance!(10), balance!(70), OutcomeFee::xor(balance!(0.7))),
                    SwapChunk::new(balance!(10), balance!(60), OutcomeFee::xor(balance!(0.6))),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(15))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(12), balance!(144), Default::default()),
                    SwapChunk::new(balance!(10), balance!(100), Default::default()),
                    SwapChunk::new(balance!(14), balance!(112), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(1))),
                    Some(SideAmount::Input(balance!(22))),
                    Some(SideAmount::Input(balance!(0.00001))),
                ),
            },
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_output_and_max_amount_limits(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(11), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(12), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(13), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(14), balance!(100), OutcomeFee::xor(balance!(1))),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(150))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(8), balance!(100), Default::default()),
                    SwapChunk::new(balance!(9), balance!(90), Default::default()),
                    SwapChunk::new(balance!(10.5), balance!(100), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Output(balance!(1))),
                    Some(SideAmount::Output(balance!(190))),
                    Some(SideAmount::Input(balance!(0.00001))),
                ),
            },
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_input_and_min_amount_limits(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                    SwapChunk::new(balance!(10), balance!(80), OutcomeFee::xor(balance!(0.8))),
                    SwapChunk::new(balance!(10), balance!(70), OutcomeFee::xor(balance!(0.7))),
                    SwapChunk::new(balance!(10), balance!(60), OutcomeFee::xor(balance!(0.6))),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(12), balance!(144), Default::default()),
                    SwapChunk::new(balance!(10), balance!(100), Default::default()),
                    SwapChunk::new(balance!(14), balance!(112), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(21))),
                    Some(SideAmount::Input(balance!(1000))),
                    Some(SideAmount::Input(balance!(0.00001))),
                ),
            },
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_output_and_min_amount_limits(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(11), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(12), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(13), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(14), balance!(100), OutcomeFee::xor(balance!(1))),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1000000))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(8), balance!(100), Default::default()),
                    SwapChunk::new(balance!(9), balance!(90), Default::default()),
                    SwapChunk::new(balance!(10.5), balance!(100), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Output(balance!(200))),
                    Some(SideAmount::Output(balance!(1000))),
                    Some(SideAmount::Input(balance!(0.00001))),
                ),
            },
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_input_and_precision_limits(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                    SwapChunk::new(balance!(10), balance!(80), OutcomeFee::xor(balance!(0.8))),
                    SwapChunk::new(balance!(10), balance!(70), OutcomeFee::xor(balance!(0.7))),
                    SwapChunk::new(balance!(10), balance!(60), OutcomeFee::xor(balance!(0.6))),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                    SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(11), balance!(132), Default::default()),
                    SwapChunk::new(balance!(10), balance!(90), Default::default()),
                    SwapChunk::new(balance!(14), balance!(112), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(1))),
                    Some(SideAmount::Input(balance!(1000))),
                    Some(SideAmount::Input(balance!(0.1))),
                ),
            },
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_output_and_precision_limits_for_input(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(11), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(12), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(13), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(14), balance!(100), OutcomeFee::xor(balance!(1))),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(150))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(120), Default::default()),
                    SwapChunk::new(balance!(9), balance!(90), Default::default()),
                    SwapChunk::new(balance!(13), balance!(100), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(1))),
                    Some(SideAmount::Input(balance!(1000))),
                    Some(SideAmount::Input(balance!(0.01))),
                ),
            },
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_output_and_precision_limits_for_output(
    ) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(11), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(12), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(13), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(14), balance!(100), OutcomeFee::xor(balance!(1))),
                ]),
                limits: Default::default(),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                    SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(150))), None),
            },
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(120), Default::default()),
                    SwapChunk::new(balance!(9), balance!(90), Default::default()),
                    SwapChunk::new(balance!(13), balance!(100), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Input(balance!(1))),
                    Some(SideAmount::Input(balance!(1000))),
                    Some(SideAmount::Output(balance!(0.01))),
                ),
            },
        );

        aggregator
    }

    #[test]
    fn check_empty_chunks() {
        let aggregator =
            LiquidityAggregator::<Runtime, LiquiditySourceType>::new(SwapVariant::WithDesiredInput);
        assert_err!(
            aggregator.aggregate_liquidity(balance!(1)),
            Error::<Runtime>::InsufficientLiquidity
        );
    }

    #[test]
    fn check_not_enough_liquidity() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_err!(
            aggregator.aggregate_liquidity(balance!(10000)),
            Error::<Runtime>::InsufficientLiquidity
        );
    }

    #[test]
    fn check_aggregate_liquidity_with_desired_input_and_equal_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(10)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(10), balance!(120))
                )]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(10), balance!(120))
                )],
                balance!(10),
                balance!(120),
                SwapVariant::WithDesiredInput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(20)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(20), balance!(220))
                )]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(20), balance!(220))
                )],
                balance!(20),
                balance!(220),
                SwapVariant::WithDesiredInput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(30)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(10), balance!(100))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(20), balance!(220))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(10), balance!(100))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(20), balance!(220))
                    )
                ],
                balance!(30),
                balance!(320),
                SwapVariant::WithDesiredInput,
                OutcomeFee::xor(balance!(1))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(40)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(20), balance!(220))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(20), balance!(190))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(20), balance!(220))
                    )
                ],
                balance!(40),
                balance!(410),
                SwapVariant::WithDesiredInput,
                OutcomeFee::xor(balance!(1.9))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(50)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                    (LiquiditySourceType::XSTPool, (balance!(10), balance!(85))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(20), balance!(220))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(20), balance!(190))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_input(balance!(10), balance!(85))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(20), balance!(220))
                    )
                ],
                balance!(50),
                balance!(495),
                SwapVariant::WithDesiredInput,
                OutcomeFee(BTreeMap::from([
                    (XOR, balance!(1.9)),
                    (XST, balance!(0.85))
                ]))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(60)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                    (LiquiditySourceType::XSTPool, (balance!(20), balance!(170))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(20), balance!(220))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(20), balance!(190))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_input(balance!(20), balance!(170))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(20), balance!(220))
                    )
                ],
                balance!(60),
                balance!(580),
                SwapVariant::WithDesiredInput,
                OutcomeFee(BTreeMap::from([(XOR, balance!(1.9)), (XST, balance!(1.7))]))
            )
        );
    }

    #[test]
    fn check_aggregate_liquidity_with_desired_output_and_equal_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(100)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(LiquiditySourceType::OrderBook, (balance!(8), balance!(100)))]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(100), balance!(8))
                )],
                balance!(100),
                balance!(8),
                SwapVariant::WithDesiredOutput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(200)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(18), balance!(200))
                )]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(200), balance!(18))
                )],
                balance!(200),
                balance!(18),
                SwapVariant::WithDesiredOutput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(300)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(10), balance!(100))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18), balance!(200))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(100), balance!(10))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(200), balance!(18))
                    )
                ],
                balance!(300),
                balance!(28),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(1))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(400)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(21), balance!(200))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18), balance!(200))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(200), balance!(21))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(200), balance!(18))
                    )
                ],
                balance!(400),
                balance!(39),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(2))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(500)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18), balance!(200))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(300), balance!(33))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(200), balance!(18))
                    )
                ],
                balance!(500),
                balance!(51),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(3))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(600)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                    (
                        LiquiditySourceType::XSTPool,
                        (balance!(12.5), balance!(100))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18), balance!(200))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(300), balance!(33))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_output(balance!(100), balance!(12.5))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(200), balance!(18))
                    )
                ],
                balance!(600),
                balance!(63.5),
                SwapVariant::WithDesiredOutput,
                OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(1))]))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(700)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                    (LiquiditySourceType::XSTPool, (balance!(25), balance!(200))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18), balance!(200))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(300), balance!(33))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_output(balance!(200), balance!(25))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(200), balance!(18))
                    )
                ],
                balance!(700),
                balance!(76),
                SwapVariant::WithDesiredOutput,
                OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(2))]))
            )
        );
    }

    #[test]
    fn check_aggregate_liquidity_with_desired_input_and_different_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(10)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(10), balance!(120))
                )]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(10), balance!(120))
                )],
                balance!(10),
                balance!(120),
                SwapVariant::WithDesiredInput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(20)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(20), balance!(224))
                )]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(20), balance!(224))
                )],
                balance!(20),
                balance!(224),
                SwapVariant::WithDesiredInput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(30)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(8), balance!(80))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(8), balance!(80))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(22), balance!(244))
                    )
                ],
                balance!(30),
                balance!(324),
                SwapVariant::WithDesiredInput,
                OutcomeFee::xor(balance!(0.8))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(40)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(18), balance!(172))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(18), balance!(172))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(22), balance!(244))
                    )
                ],
                balance!(40),
                balance!(416),
                SwapVariant::WithDesiredInput,
                OutcomeFee::xor(balance!(1.719999999999999999))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(50)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(22), balance!(208))),
                    (LiquiditySourceType::XSTPool, (balance!(6), balance!(51))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(22), balance!(208))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_input(balance!(6), balance!(51))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(22), balance!(244))
                    )
                ],
                balance!(50),
                balance!(503),
                SwapVariant::WithDesiredInput,
                OutcomeFee(BTreeMap::from([
                    (XOR, balance!(2.08)),
                    (XST, balance!(0.51))
                ]))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(60)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(22), balance!(208))),
                    (LiquiditySourceType::XSTPool, (balance!(16), balance!(136))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(22), balance!(208))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_input(balance!(16), balance!(136))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(22), balance!(244))
                    )
                ],
                balance!(60),
                balance!(588),
                SwapVariant::WithDesiredInput,
                OutcomeFee(BTreeMap::from([
                    (XOR, balance!(2.08)),
                    (XST, balance!(1.359999999999999999))
                ]))
            )
        );
    }

    #[test]
    fn check_aggregate_liquidity_with_desired_output_and_different_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(100)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(LiquiditySourceType::OrderBook, (balance!(8), balance!(100)))]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(100), balance!(8))
                )],
                balance!(100),
                balance!(8),
                SwapVariant::WithDesiredOutput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(150)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(13), balance!(150))
                )]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(150), balance!(13))
                )],
                balance!(150),
                balance!(13),
                SwapVariant::WithDesiredOutput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(250)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(6), balance!(60))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(190))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(60), balance!(6))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(190), balance!(17))
                    )
                ],
                balance!(250),
                balance!(23),
                SwapVariant::WithDesiredOutput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(340)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(15.5), balance!(150))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(190))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(150), balance!(15.5))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(190), balance!(17))
                    )
                ],
                balance!(340),
                balance!(32.5),
                SwapVariant::WithDesiredOutput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(405)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(18.5), balance!(175))
                    ),
                    (LiquiditySourceType::XSTPool, (balance!(5), balance!(40))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(190))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(175), balance!(18.5))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_output(balance!(40), balance!(5))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(190), balance!(17))
                    )
                ],
                balance!(405),
                balance!(40.5),
                SwapVariant::WithDesiredOutput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(505)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(18.5), balance!(175))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        (balance!(17.5), balance!(140))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(190))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(175), balance!(18.5))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_output(balance!(140), balance!(17.5))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(190), balance!(17))
                    )
                ],
                balance!(505),
                balance!(53),
                SwapVariant::WithDesiredOutput,
                Default::default()
            )
        );
    }

    #[test]
    fn check_aggregate_liquidity_with_desired_input_and_max_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(10)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(10), balance!(120))
                )]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(10), balance!(120))
                )],
                balance!(10),
                balance!(120),
                SwapVariant::WithDesiredInput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(20)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(20), balance!(224))
                )]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(20), balance!(224))
                )],
                balance!(20),
                balance!(224),
                SwapVariant::WithDesiredInput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(30)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(8), balance!(80))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(8), balance!(80))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(22), balance!(244))
                    )
                ],
                balance!(30),
                balance!(324),
                SwapVariant::WithDesiredInput,
                OutcomeFee::xor(balance!(0.8))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(50)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                    (LiquiditySourceType::XSTPool, (balance!(8), balance!(68))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(20), balance!(190))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_input(balance!(8), balance!(68))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(22), balance!(244))
                    )
                ],
                balance!(50),
                balance!(502),
                SwapVariant::WithDesiredInput,
                OutcomeFee(BTreeMap::from([
                    (XOR, balance!(1.9)),
                    (XST, balance!(0.68))
                ]))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(60)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(23), balance!(214))),
                    (
                        LiquiditySourceType::XSTPool,
                        (balance!(15), balance!(127.5))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(23), balance!(214))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_input(balance!(15), balance!(127.5))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(22), balance!(244))
                    )
                ],
                balance!(60),
                balance!(585.5),
                SwapVariant::WithDesiredInput,
                OutcomeFee(BTreeMap::from([
                    (XOR, balance!(2.14)),
                    (XST, balance!(1.275))
                ]))
            )
        );
    }

    #[test]
    fn check_aggregate_liquidity_with_desired_output_and_max_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(100)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(LiquiditySourceType::OrderBook, (balance!(8), balance!(100)))]),
                vec![(
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(100), balance!(8))
                )],
                balance!(100),
                balance!(8),
                SwapVariant::WithDesiredOutput,
                Default::default()
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(200)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(1), balance!(10))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(190))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(10), balance!(1))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(190), balance!(17))
                    )
                ],
                balance!(200),
                balance!(18),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(0.1))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(300)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(11.1), balance!(110))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(190))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(110), balance!(11.1))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(190), balance!(17))
                    )
                ],
                balance!(300),
                balance!(28.1),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(1.1))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(500)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                    (LiquiditySourceType::XSTPool, (balance!(1.25), balance!(10))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(190))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(300), balance!(33))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_output(balance!(10), balance!(1.25))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(190), balance!(17))
                    )
                ],
                balance!(500),
                balance!(51.25),
                SwapVariant::WithDesiredOutput,
                OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(0.1))]))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(600)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                    (
                        LiquiditySourceType::XSTPool,
                        (balance!(13.75), balance!(110))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(190))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(300), balance!(33))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_output(balance!(110), balance!(13.75))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(190), balance!(17))
                    )
                ],
                balance!(600),
                balance!(63.75),
                SwapVariant::WithDesiredOutput,
                OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(1.1))]))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(700)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(40.8), balance!(360))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        (balance!(18.75), balance!(150))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(190))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(360), balance!(40.8))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_output(balance!(150), balance!(18.75))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(190), balance!(17))
                    )
                ],
                balance!(700),
                balance!(76.55),
                SwapVariant::WithDesiredOutput,
                OutcomeFee(BTreeMap::from([(XOR, balance!(3.6)), (XST, balance!(1.5))]))
            )
        );
    }

    #[test]
    fn check_aggregate_liquidity_with_desired_input_and_min_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(10)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(10), balance!(100)))]),
                vec![(
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(10), balance!(100))
                )],
                balance!(10),
                balance!(100),
                SwapVariant::WithDesiredInput,
                OutcomeFee::xor(balance!(1))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(20)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(20), balance!(190)))]),
                vec![(
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(20), balance!(190))
                )],
                balance!(20),
                balance!(190),
                SwapVariant::WithDesiredInput,
                OutcomeFee::xor(balance!(1.9))
            )
        );

        // order-book appears only when it exceeds the min amount
        let aggregator = get_liquidity_aggregator_with_desired_input_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(30)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(8), balance!(80))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(8), balance!(80))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(22), balance!(244))
                    )
                ],
                balance!(30),
                balance!(324),
                SwapVariant::WithDesiredInput,
                OutcomeFee::xor(balance!(0.8))
            )
        );
    }

    #[test]
    fn check_aggregate_liquidity_with_desired_output_and_min_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(100)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(10), balance!(100)))]),
                vec![(
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(100), balance!(10))
                )],
                balance!(100),
                balance!(10),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(1))
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(200)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(21), balance!(200)))]),
                vec![(
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(200), balance!(21))
                )],
                balance!(200),
                balance!(21),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(2))
            )
        );

        // order-book appears only when it exceeds the min amount
        let aggregator = get_liquidity_aggregator_with_desired_output_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(300)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(10), balance!(100))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18.05), balance!(200))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(100), balance!(10))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(200), balance!(18.05))
                    )
                ],
                balance!(300),
                balance!(28.05),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(1))
            )
        );
    }

    #[test]
    fn check_aggregate_liquidity_with_desired_input_and_precision_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_precision_limits();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(10.65)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(0.05), balance!(0.5))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(10.6), balance!(127.2))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(0.05), balance!(0.5))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_input(balance!(10.6), balance!(127.2))
                    )
                ],
                balance!(10.65),
                balance!(127.7),
                SwapVariant::WithDesiredInput,
                OutcomeFee::xor(balance!(0.005))
            )
        );
    }

    #[test]
    fn check_aggregate_liquidity_with_desired_output_and_precision_limits() {
        let aggregator =
            get_liquidity_aggregator_with_desired_output_and_precision_limits_for_output();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(101.585)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(0.0005), balance!(0.005))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(8.465), balance!(101.58))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(0.005), balance!(0.0005))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(101.58), balance!(8.465))
                    )
                ],
                balance!(101.585),
                balance!(8.4655),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(0.00005))
            )
        );

        let aggregator =
            get_liquidity_aggregator_with_desired_output_and_precision_limits_for_input();
        assert_eq!(
            aggregator.aggregate_liquidity(balance!(101.585)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (
                            balance!(0.006499999999999999),
                            balance!(0.064999999999999993)
                        )
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(8.46), balance!(101.520000000000000007))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(
                            balance!(0.064999999999999993),
                            balance!(0.006499999999999999)
                        )
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(
                            balance!(101.520000000000000007),
                            balance!(8.46)
                        )
                    )
                ],
                balance!(101.585),
                balance!(8.466499999999999999),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(0.000649999999999999))
            )
        );
    }

    #[test]
    fn check_returning_back_several_chunks() {
        let mut aggregator =
            LiquidityAggregator::<Runtime, LiquiditySourceType>::new(SwapVariant::WithDesiredInput);
        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            DiscreteQuotation {
                chunks: vec![SwapChunk::new(balance!(0.1), balance!(1), Default::default()); 100]
                    .into(),
                limits: SwapLimits::new(None, None, Some(SideAmount::Input(balance!(1)))),
            },
        );
        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            DiscreteQuotation {
                chunks: vec![SwapChunk::new(balance!(1), balance!(8), Default::default()); 100]
                    .into(),
                limits: SwapLimits::new(None, None, None),
            },
        );

        assert_eq!(
            aggregator.aggregate_liquidity(balance!(1.5)).unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(0.5), balance!(4))),
                    (LiquiditySourceType::XSTPool, (balance!(1), balance!(10)))
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_input(balance!(0.5), balance!(4))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        SwapAmount::with_desired_input(balance!(1), balance!(10))
                    )
                ],
                balance!(1.5),
                balance!(14),
                SwapVariant::WithDesiredInput,
                Default::default()
            )
        );
    }
}
