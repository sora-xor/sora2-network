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
use frame_support::traits::Get;
use frame_support::RuntimeDebug;
use sp_std::vec::Vec;

#[cfg(feature = "wip")] // ALT
use {
    crate::{Config, Error},
    common::alt::{AlignReason, DiscreteQuotation, SideAmount, SwapChunk},
    common::prelude::SwapVariant,
    common::{fixed, Balance},
    itertools::Itertools,
    sp_runtime::traits::Zero,
    sp_runtime::DispatchError,
    sp_std::collections::btree_map::BTreeMap,
    sp_std::collections::vec_deque::VecDeque,
    sp_std::vec,
};

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
#[derive(Clone)]
pub struct LiquidityAggregator<AssetId: Ord + Clone, LiquiditySourceType> {
    liquidity_quotations: BTreeMap<LiquiditySourceType, DiscreteQuotation<AssetId, Balance>>,
    variant: SwapVariant,
    locked_sources: Vec<LiquiditySourceType>,
}

#[cfg(feature = "wip")] // ALT
impl<AssetId, LiquiditySourceType> LiquidityAggregator<AssetId, LiquiditySourceType>
where
    AssetId: Ord + Clone + From<common::AssetId32<common::PredefinedAssetId>>,
    LiquiditySourceType: Ord + Clone,
{
    pub fn new(variant: SwapVariant) -> Self {
        Self {
            liquidity_quotations: BTreeMap::new(),
            variant,
            locked_sources: Vec::new(),
        }
    }

    pub fn lock_source(&mut self, source: LiquiditySourceType) {
        if !self.locked_sources.contains(&source) {
            self.locked_sources.push(source);
        }
    }

    pub fn add_source(
        &mut self,
        source: LiquiditySourceType,
        discrete_quotation: DiscreteQuotation<AssetId, Balance>,
    ) {
        self.liquidity_quotations.insert(source, discrete_quotation);
    }

    /// Aggregates the liquidity from the provided liquidity sources.
    /// Liquidity sources provide discretized liquidity curve by chunks and then Liquidity Aggregator selects the best chunks from different sources to gain the best swap amount.
    pub fn aggregate_swap_outcome<T: Config>(
        mut self,
        amount: Balance,
    ) -> Result<AggregationResult<AssetId, LiquiditySourceType, Balance>, DispatchError> {
        if self.liquidity_quotations.is_empty() {
            return Err(Error::<T>::InsufficientLiquidity.into());
        }

        let mut remaining_amount = amount;
        let mut selected = BTreeMap::new();

        while remaining_amount > Balance::zero() {
            let candidates = self.find_best_price_candidates();

            let mut source = candidates
                .first()
                .ok_or(Error::<T>::InsufficientLiquidity)?;

            // if there are several candidates with the same best price,
            // then we need to select the source that already been selected
            for candidate in candidates.iter() {
                if selected.keys().contains(candidate) {
                    source = candidate;
                    break;
                }
            }

            let discrete_quotation = self
                .liquidity_quotations
                .get_mut(source)
                .ok_or(Error::<T>::InsufficientLiquidity)?;
            let mut chunk = discrete_quotation
                .chunks
                .pop_front()
                .ok_or(Error::<T>::InsufficientLiquidity)?;
            let mut payback = SwapChunk::zero();

            let total = Self::sum_chunks(selected.entry(source.clone()).or_default());
            let (aligned, remainder) = discrete_quotation
                .limits
                .align_extra_chunk_max(total.clone(), chunk.clone())
                .ok_or(Error::<T>::CalculationError)?;
            if !remainder.is_zero() {
                // max amount (already selected + new chunk) exceeded
                chunk = aligned;
                payback = remainder;
                self.locked_sources.push(source.clone());
            }

            let remaining_side_amount = SideAmount::new(remaining_amount, self.variant);
            if chunk > remaining_side_amount {
                let rescaled = chunk
                    .clone()
                    .rescale_by_side_amount(remaining_side_amount)
                    .ok_or(Error::<T>::CalculationError)?;
                payback = payback.saturating_add(chunk.clone().saturating_sub(rescaled.clone()));
                chunk = rescaled;
            }

            let remaining_delta = *chunk.get_associated_field(self.variant).amount();

            if !payback.is_zero() {
                // push remains of the chunk back
                discrete_quotation.chunks.push_front(payback);
            }

            if chunk.is_zero() {
                continue;
            }

            selected
                .entry(source.clone())
                .and_modify(|chunks: &mut VecDeque<SwapChunk<AssetId, Balance>>| {
                    chunks.push_back(chunk.clone())
                })
                .or_insert(vec![chunk.clone()].into());
            remaining_amount = remaining_amount
                .checked_sub(remaining_delta)
                .ok_or(Error::<T>::CalculationError)?;

            if remaining_amount.is_zero() {
                let mut to_delete = Vec::new();
                for (source, chunks) in &mut selected {
                    let total = Self::sum_chunks(chunks);
                    let discrete_quotation = self
                        .liquidity_quotations
                        .get_mut(source)
                        .ok_or(Error::<T>::InsufficientLiquidity)?;

                    let (aligned, remainder, align_reason) = discrete_quotation
                        .limits
                        .align_chunk(total)
                        .ok_or(Error::<T>::CalculationError)?;
                    if !remainder.is_zero() {
                        remaining_amount = remaining_amount
                            .checked_add(*remainder.get_associated_field(self.variant).amount())
                            .ok_or(Error::<T>::CalculationError)?;
                        self.locked_sources.push(source.clone());

                        if aligned.is_zero() {
                            // liquidity is not enough even for the min amount
                            to_delete.push(source.clone());

                            for chunk in chunks.iter().rev() {
                                discrete_quotation.chunks.push_front(chunk.clone());
                            }
                        } else {
                            let mut remainder = remainder.get_associated_field(self.variant);
                            while *remainder.amount() > Balance::zero() {
                                // it is necessary to return chunks back till `remainder` volume is filled
                                let Some(chunk) = chunks.pop_back() else {
                                    // chunks are over, already returned all chunks
                                    to_delete.push(source.clone());
                                    break;
                                };
                                if chunk <= remainder {
                                    let value = remainder
                                        .amount()
                                        .checked_sub(
                                            *chunk.get_associated_field(self.variant).amount(),
                                        )
                                        .ok_or(Error::<T>::CalculationError)?;
                                    remainder.set_amount(value);
                                    discrete_quotation.chunks.push_front(chunk);
                                } else {
                                    let remainder_chunk = chunk
                                        .clone()
                                        .rescale_by_side_amount(remainder)
                                        .ok_or(Error::<T>::CalculationError)?;
                                    let chunk = chunk.saturating_sub(remainder_chunk.clone());
                                    chunks.push_back(chunk);
                                    discrete_quotation.chunks.push_front(remainder_chunk);
                                    remainder.set_amount(Balance::zero());
                                }
                            }
                        }
                    }
                }
                selected.retain(|source, _| !to_delete.contains(source));
            }
        }

        self.calculate_result::<T>(&selected)
    }

    /// Find liquidity sources where the top chunk has the best price.
    fn find_best_price_candidates(&self) -> Vec<LiquiditySourceType> {
        let mut candidates = Vec::new();
        let mut max = fixed!(0);
        for (source, discrete_quotation) in self.liquidity_quotations.iter() {
            // skip the locked source
            if self.locked_sources.contains(source) {
                continue;
            }

            let Some(front) = discrete_quotation.chunks.front() else {
                continue;
            };
            let Some(price) = front.price() else {
                continue;
            };

            if price == max {
                candidates.push(source.clone())
            }

            if price > max {
                candidates.clear();
                max = price;
                candidates.push(source.clone());
            }
        }
        candidates
    }

    fn calculate_result<T: Config>(
        &self,
        selected: &BTreeMap<LiquiditySourceType, VecDeque<SwapChunk<AssetId, Balance>>>,
    ) -> Result<AggregationResult<AssetId, LiquiditySourceType, Balance>, DispatchError> {
        let mut distribution = Vec::new();
        let mut swap_info: SwapInfo<LiquiditySourceType, Balance> = SwapInfo::new();
        let mut desired_amount = Balance::zero();
        let mut result_amount = Balance::zero();
        let mut fee = OutcomeFee::default();

        for (source, chunks) in selected {
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

    fn sum_chunks(chunks: &VecDeque<SwapChunk<AssetId, Balance>>) -> SwapChunk<AssetId, Balance> {
        chunks
            .iter()
            .fold(SwapChunk::<AssetId, Balance>::zero(), |acc, next| {
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

    type AssetId = common::AssetId32<common::PredefinedAssetId>;

    fn get_liquidity_aggregator_with_desired_input_and_equal_chunks(
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
    ) -> LiquidityAggregator<AssetId, LiquiditySourceType> {
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
            LiquidityAggregator::<AssetId, LiquiditySourceType>::new(SwapVariant::WithDesiredInput);
        assert_err!(
            aggregator.aggregate_swap_outcome::<Runtime>(balance!(1)),
            Error::<Runtime>::InsufficientLiquidity
        );
    }

    #[test]
    fn check_not_enough_liquidity() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_err!(
            aggregator.aggregate_swap_outcome::<Runtime>(balance!(10000)),
            Error::<Runtime>::InsufficientLiquidity
        );
    }

    #[test]
    fn check_find_best_price_candidates_with_desired_input_and_equal_chunks() {
        let mut aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);

        // remove order book chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // check with locked sources
        {
            let mut aggregator = aggregator.clone();
            aggregator.lock_source(LiquiditySourceType::XYKPool);
            let candidates = aggregator.find_best_price_candidates();
            assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);
        }
        {
            let mut aggregator = aggregator.clone();
            aggregator.lock_source(LiquiditySourceType::OrderBook);
            let candidates = aggregator.find_best_price_candidates();
            assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);
        }

        // remove order book chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);
    }

    #[test]
    fn check_find_best_price_candidates_with_desired_output_and_equal_chunks() {
        let mut aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);

        // remove order book chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // check with locked sources
        {
            let mut aggregator = aggregator.clone();
            aggregator.lock_source(LiquiditySourceType::XYKPool);
            let candidates = aggregator.find_best_price_candidates();
            assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);
        }
        {
            let mut aggregator = aggregator.clone();
            aggregator.lock_source(LiquiditySourceType::OrderBook);
            let candidates = aggregator.find_best_price_candidates();
            assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);
        }

        // remove order book chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 3
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_input_and_equal_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(10))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(20))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(30))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(40))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(50))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(60))
                .unwrap(),
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
    fn check_aggregate_swap_outcome_with_desired_output_and_equal_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(100))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(200))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(300))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(400))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(500))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(600))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(700))
                .unwrap(),
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
    fn check_find_best_price_candidates_with_desired_input_and_different_chunks() {
        let mut aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);

        // remove order book chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // check with locked sources
        {
            let mut aggregator = aggregator.clone();
            aggregator.lock_source(LiquiditySourceType::XYKPool);
            let candidates = aggregator.find_best_price_candidates();
            assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);
        }
        {
            let mut aggregator = aggregator.clone();
            aggregator.lock_source(LiquiditySourceType::OrderBook);
            let candidates = aggregator.find_best_price_candidates();
            assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);
        }

        // remove order book chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);
    }

    #[test]
    fn check_find_best_price_candidates_with_desired_output_and_different_chunks() {
        let mut aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);

        // remove order book chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // check with locked sources
        {
            let mut aggregator = aggregator.clone();
            aggregator.lock_source(LiquiditySourceType::XYKPool);
            let candidates = aggregator.find_best_price_candidates();
            assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);
        }
        {
            let mut aggregator = aggregator.clone();
            aggregator.lock_source(LiquiditySourceType::OrderBook);
            let candidates = aggregator.find_best_price_candidates();
            assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);
        }

        // remove order book chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 3
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_input_and_different_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(10))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(20))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(30))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(40))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(50))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(60))
                .unwrap(),
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
    fn check_aggregate_swap_outcome_with_desired_output_and_different_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(100))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(150))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(250))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(340))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(405))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(505))
                .unwrap(),
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
    fn check_aggregate_swap_outcome_with_desired_input_and_max_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(10))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(20))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(30))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(50))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(60))
                .unwrap(),
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
    fn check_aggregate_swap_outcome_with_desired_output_and_max_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(100))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(200))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(300))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(500))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(600))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(700))
                .unwrap(),
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
    fn check_aggregate_swap_outcome_with_desired_input_and_min_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_min_amount_limits();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(10))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(20))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(30))
                .unwrap(),
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
    fn check_aggregate_swap_outcome_with_desired_output_and_min_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_min_amount_limits();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(100))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(200))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(300))
                .unwrap(),
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
    fn check_aggregate_swap_outcome_with_desired_input_and_precision_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_precision_limits();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(10.65))
                .unwrap(),
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
    fn check_aggregate_swap_outcome_with_desired_output_and_precision_limits() {
        let aggregator =
            get_liquidity_aggregator_with_desired_output_and_precision_limits_for_output();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(101.585))
                .unwrap(),
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(101.585))
                .unwrap(),
            AggregationResult::new(
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(0.0065), balance!(0.065))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(8.46), balance!(101.52))
                    )
                ]),
                vec![
                    (
                        LiquiditySourceType::XYKPool,
                        SwapAmount::with_desired_output(balance!(0.065), balance!(0.0065))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        SwapAmount::with_desired_output(balance!(101.52), balance!(8.46))
                    )
                ],
                balance!(101.585),
                balance!(8.4665),
                SwapVariant::WithDesiredOutput,
                OutcomeFee::xor(balance!(0.00065))
            )
        );
    }

    #[test]
    fn check_returning_back_several_chunks() {
        let mut aggregator =
            LiquidityAggregator::<AssetId, LiquiditySourceType>::new(SwapVariant::WithDesiredInput);
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
            aggregator
                .aggregate_swap_outcome::<Runtime>(balance!(1.5))
                .unwrap(),
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
