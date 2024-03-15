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
use common::prelude::{OutcomeFee, QuoteAmount};
use frame_support::RuntimeDebug;
use sp_std::vec::Vec;

#[cfg(feature = "wip")] // ALT
use {
    common::alt::{DiscreteQuotation, SideAmount, SwapChunk},
    common::prelude::SwapVariant,
    common::{fixed, Balance},
    itertools::Itertools,
    sp_runtime::traits::Zero,
    sp_std::collections::btree_map::BTreeMap,
    sp_std::collections::vec_deque::VecDeque,
    sp_std::vec,
};

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
    pub distribution: Vec<(LiquiditySourceType, QuoteAmount<AmountType>)>,
    /// The best possible output/input amount for a given trade and a set of liquidity sources
    pub amount: AmountType,
    /// Total fee amount, nominated in XOR
    pub fee: OutcomeFee<AssetId, AmountType>,
}

impl<AssetId: Ord, LiquiditySourceIdType, AmountType>
    AggregatedSwapOutcome<AssetId, LiquiditySourceIdType, AmountType>
{
    pub fn new(
        distribution: Vec<(LiquiditySourceIdType, QuoteAmount<AmountType>)>,
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

/// Aggregates the liquidity from the provided liquidity sources.
/// Liquidity sources provide discretized liquidity curve by chunks and then Liquidity Aggregator selects the best chunks from different sources to gain the best swap amount.
#[cfg(feature = "wip")] // ALT
#[derive(Clone)]
pub struct LiquidityAggregator<AssetId: Ord + Clone, LiquiditySourceType> {
    liquidity_quotations: BTreeMap<LiquiditySourceType, DiscreteQuotation<AssetId, Balance>>,
    variant: SwapVariant,
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
        }
    }

    pub fn add_source(
        &mut self,
        source: LiquiditySourceType,
        discrete_quotation: DiscreteQuotation<AssetId, Balance>,
    ) {
        self.liquidity_quotations.insert(source, discrete_quotation);
    }

    pub fn aggregate_swap_outcome(
        mut self,
        amount: Balance,
    ) -> Option<(
        SwapInfo<LiquiditySourceType, Balance>,
        AggregatedSwapOutcome<AssetId, LiquiditySourceType, Balance>,
    )> {
        if self.liquidity_quotations.is_empty() {
            return None;
        }

        let mut remaining_amount = amount;
        let mut locked_sources = Vec::new();
        let mut selected = BTreeMap::new();

        while remaining_amount > Balance::zero() {
            let candidates = self.find_best_price_candidates(&locked_sources);

            let mut source = candidates.first()?;

            // if there are several candidates with the same best price,
            // then we need to select the source that already been selected
            for candidate in candidates.iter() {
                if selected.keys().contains(candidate) {
                    source = candidate;
                    break;
                }
            }

            let discrete_quotation = self.liquidity_quotations.get_mut(source)?;
            let mut chunk = discrete_quotation.chunks.pop_front()?;
            let mut payback = SwapChunk::zero();

            let total = Self::sum_chunks(selected.entry(source.clone()).or_default());
            let (max_chunk, remainder) = discrete_quotation
                .limits
                .align_chunk_max(total.clone().saturating_add(chunk.clone()))?;
            if !remainder.is_zero() {
                // max amount exceeded
                let diff = max_chunk.saturating_sub(total);
                if diff.is_zero() {
                    // it means the total volume of the source is already equal with max amount
                    payback = chunk.clone();
                    chunk.set_zero();
                } else {
                    chunk =
                        chunk.rescale_by_side_amount(diff.get_associated_field(self.variant))?;
                    payback = remainder;
                }
                locked_sources.push(source.clone());
            }

            let remaining_side_amount = SideAmount::new(remaining_amount, self.variant);
            if chunk > remaining_side_amount {
                let rescaled = chunk
                    .clone()
                    .rescale_by_side_amount(remaining_side_amount)?;
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
            remaining_amount = remaining_amount.checked_sub(remaining_delta)?;

            if remaining_amount.is_zero() {
                let mut to_delete = Vec::new();
                for (source, chunks) in &mut selected {
                    let total = Self::sum_chunks(chunks);
                    let discrete_quotation = self.liquidity_quotations.get_mut(source)?;

                    let (aligned, remainder) = discrete_quotation.limits.align_chunk(total)?;
                    if !remainder.is_zero() {
                        remaining_amount = remaining_amount
                            .checked_add(*remainder.get_associated_field(self.variant).amount())?;
                        locked_sources.push(source.clone());

                        if aligned.is_zero() {
                            // liquidity is not enough even for the min amount
                            to_delete.push(source.clone());

                            for chunk in chunks.iter().rev() {
                                discrete_quotation.chunks.push_front(chunk.clone());
                            }
                        } else {
                            let mut remainder = remainder.get_associated_field(self.variant);
                            while *remainder.amount() > Balance::zero() {
                                let Some(chunk) = chunks.pop_back() else {
                                    to_delete.push(source.clone());
                                    break;
                                };
                                if chunk <= remainder {
                                    let value = remainder.amount().checked_sub(
                                        *chunk.get_associated_field(self.variant).amount(),
                                    )?;
                                    remainder.set_amount(value);
                                    discrete_quotation.chunks.push_front(chunk);
                                } else {
                                    let remainder_chunk =
                                        chunk.clone().rescale_by_side_amount(remainder)?;
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

        let mut distribution = Vec::new();
        let mut swap_info: SwapInfo<LiquiditySourceType, Balance> = SwapInfo::new();
        let mut result_amount = Balance::zero();
        let mut fee = OutcomeFee::default();

        for (source, chunks) in &selected {
            let total = Self::sum_chunks(chunks);

            swap_info.insert(source.clone(), (total.input, total.output));

            let (desired_part, result_part) = match self.variant {
                SwapVariant::WithDesiredInput => (total.input, total.output),
                SwapVariant::WithDesiredOutput => (total.output, total.input),
            };
            distribution.push((
                source.clone(),
                QuoteAmount::with_variant(self.variant, desired_part),
            ));
            result_amount = result_amount.checked_add(result_part)?;
            fee = fee.merge(total.fee);
        }

        Some((
            swap_info,
            AggregatedSwapOutcome {
                distribution,
                amount: result_amount,
                fee,
            },
        ))
    }

    /// Find liquidity sources where the top chunk has the best price.
    fn find_best_price_candidates(
        &self,
        locked: &Vec<LiquiditySourceType>,
    ) -> Vec<LiquiditySourceType> {
        let mut candidates = Vec::new();
        let mut max = fixed!(0);
        for (source, discrete_quotation) in self.liquidity_quotations.iter() {
            // skip the locked source
            if locked.contains(source) {
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
    use crate::liquidity_aggregator::*;
    use common::alt::{DiscreteQuotation, SideAmount, SwapChunk, SwapLimits};
    use common::prelude::{OutcomeFee, QuoteAmount, SwapVariant};
    use common::{balance, LiquiditySourceType, XOR, XST};
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
                    Some(SideAmount::Input(balance!(17))),
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
                    SwapChunk::new(balance!(13), balance!(100), Default::default()),
                ]),
                limits: SwapLimits::new(
                    Some(SideAmount::Output(balance!(1))),
                    Some(SideAmount::Output(balance!(170))),
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
    fn check_find_best_price_candidates_with_desired_input_and_equal_chunks() {
        let mut aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);

        // remove order book chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // check with locked sources
        let candidates = aggregator.find_best_price_candidates(&vec![LiquiditySourceType::XYKPool]);
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);
        let candidates =
            aggregator.find_best_price_candidates(&vec![LiquiditySourceType::OrderBook]);
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove order book chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);
    }

    #[test]
    fn check_find_best_price_candidates_with_desired_output_and_equal_chunks() {
        let mut aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);

        // remove order book chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // check with locked sources
        let candidates = aggregator.find_best_price_candidates(&vec![LiquiditySourceType::XYKPool]);
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);
        let candidates =
            aggregator.find_best_price_candidates(&vec![LiquiditySourceType::OrderBook]);
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove order book chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 3
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_input_and_equal_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(10)).unwrap(),
            (
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(10), balance!(120))
                )]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::OrderBook,
                        QuoteAmount::with_desired_input(balance!(10))
                    )],
                    balance!(120),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(20)).unwrap(),
            (
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(20), balance!(220))
                )]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::OrderBook,
                        QuoteAmount::with_desired_input(balance!(20))
                    )],
                    balance!(220),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(30)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(10), balance!(100))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(20), balance!(220))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(10))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(20))
                        )
                    ],
                    balance!(320),
                    OutcomeFee::xor(balance!(1))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(40)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(20), balance!(220))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(20))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(20))
                        )
                    ],
                    balance!(410),
                    OutcomeFee::xor(balance!(1.9))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(50)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                    (LiquiditySourceType::XSTPool, (balance!(10), balance!(85))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(20), balance!(220))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(20))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_input(balance!(10))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(20))
                        )
                    ],
                    balance!(495),
                    OutcomeFee(BTreeMap::from([
                        (XOR, balance!(1.9)),
                        (XST, balance!(0.85))
                    ]))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(60)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                    (LiquiditySourceType::XSTPool, (balance!(20), balance!(170))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(20), balance!(220))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(20))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_input(balance!(20))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(20))
                        )
                    ],
                    balance!(580),
                    OutcomeFee(BTreeMap::from([(XOR, balance!(1.9)), (XST, balance!(1.7))]))
                )
            )
        );
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_output_and_equal_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(100)).unwrap(),
            (
                SwapInfo::from([(LiquiditySourceType::OrderBook, (balance!(8), balance!(100)))]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::OrderBook,
                        QuoteAmount::with_desired_output(balance!(100))
                    )],
                    balance!(8),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(200)).unwrap(),
            (
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(18), balance!(200))
                )]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::OrderBook,
                        QuoteAmount::with_desired_output(balance!(200))
                    )],
                    balance!(18),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(300)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(10), balance!(100))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18), balance!(200))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(100))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(200))
                        )
                    ],
                    balance!(28),
                    OutcomeFee::xor(balance!(1))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(400)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(21), balance!(200))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18), balance!(200))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(200))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(200))
                        )
                    ],
                    balance!(39),
                    OutcomeFee::xor(balance!(2))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(500)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18), balance!(200))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(300))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(200))
                        )
                    ],
                    balance!(51),
                    OutcomeFee::xor(balance!(3))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(600)).unwrap(),
            (
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
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(300))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_output(balance!(100))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(200))
                        )
                    ],
                    balance!(63.5),
                    OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(1))]))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(700)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                    (LiquiditySourceType::XSTPool, (balance!(25), balance!(200))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18), balance!(200))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(300))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_output(balance!(200))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(200))
                        )
                    ],
                    balance!(76),
                    OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(2))]))
                )
            )
        );
    }

    #[test]
    fn check_find_best_price_candidates_with_desired_input_and_different_chunks() {
        let mut aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);

        // remove order book chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // check with locked sources
        let candidates = aggregator.find_best_price_candidates(&vec![LiquiditySourceType::XYKPool]);
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);
        let candidates =
            aggregator.find_best_price_candidates(&vec![LiquiditySourceType::OrderBook]);
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove order book chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);
    }

    #[test]
    fn check_find_best_price_candidates_with_desired_output_and_different_chunks() {
        let mut aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);

        // remove order book chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // check with locked sources
        let candidates = aggregator.find_best_price_candidates(&vec![LiquiditySourceType::XYKPool]);
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);
        let candidates =
            aggregator.find_best_price_candidates(&vec![LiquiditySourceType::OrderBook]);
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove order book chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 3
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_quotations
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .chunks
            .pop_front();

        let candidates = aggregator.find_best_price_candidates(&Vec::new());
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_input_and_different_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(10)).unwrap(),
            (
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(10), balance!(120))
                )]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::OrderBook,
                        QuoteAmount::with_desired_input(balance!(10))
                    )],
                    balance!(120),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(20)).unwrap(),
            (
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(20), balance!(224))
                )]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::OrderBook,
                        QuoteAmount::with_desired_input(balance!(20))
                    )],
                    balance!(224),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(30)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(8), balance!(80))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(8))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(22))
                        )
                    ],
                    balance!(324),
                    OutcomeFee::xor(balance!(0.8))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(40)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(18), balance!(172))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(18))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(22))
                        )
                    ],
                    balance!(416),
                    OutcomeFee::xor(balance!(1.719999999999999999))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(50)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(22), balance!(208))),
                    (LiquiditySourceType::XSTPool, (balance!(6), balance!(51))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(22))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_input(balance!(6))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(22))
                        )
                    ],
                    balance!(503),
                    OutcomeFee(BTreeMap::from([
                        (XOR, balance!(2.08)),
                        (XST, balance!(0.51))
                    ]))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(60)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(22), balance!(208))),
                    (LiquiditySourceType::XSTPool, (balance!(16), balance!(136))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(22))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_input(balance!(16))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(22))
                        )
                    ],
                    balance!(588),
                    OutcomeFee(BTreeMap::from([
                        (XOR, balance!(2.08)),
                        (XST, balance!(1.359999999999999999))
                    ]))
                )
            )
        );
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_output_and_different_chunks() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(100)).unwrap(),
            (
                SwapInfo::from([(LiquiditySourceType::OrderBook, (balance!(8), balance!(100)))]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::OrderBook,
                        QuoteAmount::with_desired_output(balance!(100))
                    )],
                    balance!(8),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(150)).unwrap(),
            (
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(13), balance!(150))
                )]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::OrderBook,
                        QuoteAmount::with_desired_output(balance!(150))
                    )],
                    balance!(13),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(250)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(6), balance!(60))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(190))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(60))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(190))
                        )
                    ],
                    balance!(23),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(340)).unwrap(),
            (
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
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(150))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(190))
                        )
                    ],
                    balance!(32.5),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(405)).unwrap(),
            (
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
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(175))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_output(balance!(40))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(190))
                        )
                    ],
                    balance!(40.5),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(505)).unwrap(),
            (
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
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(175))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_output(balance!(140))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(190))
                        )
                    ],
                    balance!(53),
                    Default::default()
                )
            )
        );
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_input_and_max_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(10)).unwrap(),
            (
                SwapInfo::from([(
                    LiquiditySourceType::OrderBook,
                    (balance!(10), balance!(120))
                )]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::OrderBook,
                        QuoteAmount::with_desired_input(balance!(10))
                    )],
                    balance!(120),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(20)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(3), balance!(30))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(194))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(3))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(17))
                        )
                    ],
                    balance!(224),
                    OutcomeFee::xor(balance!(0.3))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(30)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(13), balance!(127))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(194))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(13))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(17))
                        )
                    ],
                    balance!(321),
                    OutcomeFee::xor(balance!(1.27))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(40)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                    (LiquiditySourceType::XSTPool, (balance!(3), balance!(25.5))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(194))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(20))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_input(balance!(3))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(17))
                        )
                    ],
                    balance!(409.5),
                    OutcomeFee(BTreeMap::from([
                        (XOR, balance!(1.9)),
                        (XST, balance!(0.255))
                    ]))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(50)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                    (
                        LiquiditySourceType::XSTPool,
                        (balance!(13), balance!(110.5))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(194))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(20))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_input(balance!(13))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(17))
                        )
                    ],
                    balance!(494.5),
                    OutcomeFee(BTreeMap::from([
                        (XOR, balance!(1.9)),
                        (XST, balance!(1.105))
                    ]))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(60)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(28), balance!(254))),
                    (
                        LiquiditySourceType::XSTPool,
                        (balance!(15), balance!(127.5))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(17), balance!(194))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(28))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_input(balance!(15))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(17))
                        )
                    ],
                    balance!(575.5),
                    OutcomeFee(BTreeMap::from([
                        (XOR, balance!(2.54)),
                        (XST, balance!(1.275))
                    ]))
                )
            )
        );
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_output_and_max_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(100)).unwrap(),
            (
                SwapInfo::from([(LiquiditySourceType::OrderBook, (balance!(8), balance!(100)))]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::OrderBook,
                        QuoteAmount::with_desired_output(balance!(100))
                    )],
                    balance!(8),
                    Default::default()
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(200)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(3), balance!(30))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(15), balance!(170))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(30))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(170))
                        )
                    ],
                    balance!(18),
                    OutcomeFee::xor(balance!(0.3))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(300)).unwrap(),
            (
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(13.3), balance!(130))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(15), balance!(170))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(130))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(170))
                        )
                    ],
                    balance!(28.3),
                    OutcomeFee::xor(balance!(1.3))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(400)).unwrap(),
            (
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(24.6), balance!(230))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(15), balance!(170))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(230))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(170))
                        )
                    ],
                    balance!(39.6),
                    OutcomeFee::xor(balance!(2.3))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(500)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                    (LiquiditySourceType::XSTPool, (balance!(3.75), balance!(30))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(15), balance!(170))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(300))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_output(balance!(30))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(170))
                        )
                    ],
                    balance!(51.75),
                    OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(0.3))]))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(600)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                    (
                        LiquiditySourceType::XSTPool,
                        (balance!(16.25), balance!(130))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(15), balance!(170))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(300))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_output(balance!(130))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(170))
                        )
                    ],
                    balance!(64.25),
                    OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(1.3))]))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(700)).unwrap(),
            (
                SwapInfo::from([
                    (
                        LiquiditySourceType::XYKPool,
                        (balance!(43.4), balance!(380))
                    ),
                    (
                        LiquiditySourceType::XSTPool,
                        (balance!(18.75), balance!(150))
                    ),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(15), balance!(170))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(380))
                        ),
                        (
                            LiquiditySourceType::XSTPool,
                            QuoteAmount::with_desired_output(balance!(150))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(170))
                        )
                    ],
                    balance!(77.15),
                    OutcomeFee(BTreeMap::from([(XOR, balance!(3.8)), (XST, balance!(1.5))]))
                )
            )
        );
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_input_and_min_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(10)).unwrap(),
            (
                SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(10), balance!(100)))]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::XYKPool,
                        QuoteAmount::with_desired_input(balance!(10))
                    )],
                    balance!(100),
                    OutcomeFee::xor(balance!(1))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_input_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(20)).unwrap(),
            (
                SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(20), balance!(190)))]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::XYKPool,
                        QuoteAmount::with_desired_input(balance!(20))
                    )],
                    balance!(190),
                    OutcomeFee::xor(balance!(1.9))
                )
            )
        );

        // order-book appears only when it exceeds the min amount
        let aggregator = get_liquidity_aggregator_with_desired_input_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(30)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(8), balance!(80))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(22), balance!(244))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(8))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(22))
                        )
                    ],
                    balance!(324),
                    OutcomeFee::xor(balance!(0.8))
                )
            )
        );
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_output_and_min_amount_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_output_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(100)).unwrap(),
            (
                SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(10), balance!(100)))]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::XYKPool,
                        QuoteAmount::with_desired_output(balance!(100))
                    )],
                    balance!(10),
                    OutcomeFee::xor(balance!(1))
                )
            )
        );

        let aggregator = get_liquidity_aggregator_with_desired_output_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(200)).unwrap(),
            (
                SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(21), balance!(200)))]),
                AggregatedSwapOutcome::new(
                    vec![(
                        LiquiditySourceType::XYKPool,
                        QuoteAmount::with_desired_output(balance!(200))
                    )],
                    balance!(21),
                    OutcomeFee::xor(balance!(2))
                )
            )
        );

        // order-book appears only when it exceeds the min amount
        let aggregator = get_liquidity_aggregator_with_desired_output_and_min_amount_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(300)).unwrap(),
            (
                SwapInfo::from([
                    (LiquiditySourceType::XYKPool, (balance!(10), balance!(100))),
                    (
                        LiquiditySourceType::OrderBook,
                        (balance!(18.05), balance!(200))
                    )
                ]),
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(100))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(200))
                        )
                    ],
                    balance!(28.05),
                    OutcomeFee::xor(balance!(1))
                )
            )
        );
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_input_and_precision_limits() {
        let aggregator = get_liquidity_aggregator_with_desired_input_and_precision_limits();
        assert_eq!(
            aggregator.aggregate_swap_outcome(balance!(10.65)).unwrap(),
            (
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
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_input(balance!(0.05))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_input(balance!(10.6))
                        )
                    ],
                    balance!(127.7),
                    OutcomeFee::xor(balance!(0.005))
                )
            )
        );
    }

    #[test]
    fn check_aggregate_swap_outcome_with_desired_output_and_precision_limits() {
        let aggregator =
            get_liquidity_aggregator_with_desired_output_and_precision_limits_for_output();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome(balance!(101.585))
                .unwrap(),
            (
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
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(0.005))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(101.58))
                        )
                    ],
                    balance!(8.4655),
                    OutcomeFee::xor(balance!(0.00005))
                )
            )
        );

        let aggregator =
            get_liquidity_aggregator_with_desired_output_and_precision_limits_for_input();
        assert_eq!(
            aggregator
                .aggregate_swap_outcome(balance!(101.585))
                .unwrap(),
            (
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
                AggregatedSwapOutcome::new(
                    vec![
                        (
                            LiquiditySourceType::XYKPool,
                            QuoteAmount::with_desired_output(balance!(0.065))
                        ),
                        (
                            LiquiditySourceType::OrderBook,
                            QuoteAmount::with_desired_output(balance!(101.52))
                        )
                    ],
                    balance!(8.4665),
                    OutcomeFee::xor(balance!(0.00065))
                )
            )
        );
    }
}
