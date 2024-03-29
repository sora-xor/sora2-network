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
    common::prelude::SwapVariant,
    common::{fixed, Balance, SwapChunk},
    itertools::Itertools,
    sp_runtime::traits::Zero,
    sp_std::collections::btree_map::BTreeMap,
    sp_std::collections::vec_deque::VecDeque,
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
pub struct LiquidityAggregator<LiquiditySourceType> {
    liquidity_chunks: BTreeMap<LiquiditySourceType, VecDeque<SwapChunk<Balance>>>,
    variant: SwapVariant,
}

#[cfg(feature = "wip")] // ALT
impl<LiquiditySourceType> LiquidityAggregator<LiquiditySourceType>
where
    LiquiditySourceType: Clone + Ord,
{
    pub fn new(variant: SwapVariant) -> Self {
        Self {
            liquidity_chunks: BTreeMap::new(),
            variant,
        }
    }

    pub fn add_source(
        &mut self,
        source: LiquiditySourceType,
        sorted_chunks: VecDeque<SwapChunk<Balance>>,
    ) {
        self.liquidity_chunks.insert(source, sorted_chunks);
    }

    pub fn aggregate_swap_outcome<AssetId>(
        mut self,
        amount: Balance,
    ) -> Option<(
        SwapInfo<LiquiditySourceType, Balance>,
        AggregatedSwapOutcome<AssetId, LiquiditySourceType, Balance>,
    )>
    where
        AssetId: Ord + From<common::AssetId32<common::PredefinedAssetId>>,
    {
        if self.liquidity_chunks.is_empty() {
            return None;
        }

        let mut remaining_amount = amount;
        let mut result_amount = Balance::zero();
        let mut fee = Balance::zero();

        let mut distribution: BTreeMap<LiquiditySourceType, Balance> = BTreeMap::new();
        let mut swap_info: SwapInfo<LiquiditySourceType, Balance> = SwapInfo::new();

        while remaining_amount > Balance::zero() {
            let candidates = self.find_best_price_candidates();

            let mut source = candidates.first()?;

            // if there are several candidates with the same best price,
            // then we need to select the source that already been selected
            for candidate in candidates.iter() {
                if distribution.keys().contains(candidate) {
                    source = candidate;
                    break;
                }
            }

            let mut chunk = self.liquidity_chunks.get_mut(source)?.pop_front()?;

            let (remaining_delta, result_delta, fee_delta) = match self.variant {
                SwapVariant::WithDesiredInput => {
                    if remaining_amount < chunk.input {
                        chunk = chunk.rescale_by_input(remaining_amount)?;
                    }
                    (chunk.input, chunk.output, chunk.fee)
                }
                SwapVariant::WithDesiredOutput => {
                    if remaining_amount < chunk.output {
                        chunk = chunk.rescale_by_output(remaining_amount)?;
                    }
                    (chunk.output, chunk.input, chunk.fee)
                }
            };

            swap_info
                .entry(source.clone())
                .and_modify(|(input, output)| {
                    *input = input.saturating_add(chunk.input);
                    *output = output.saturating_add(chunk.output);
                })
                .or_insert((chunk.input, chunk.output));

            distribution
                .entry(source.clone())
                .and_modify(|amount| *amount = amount.saturating_add(remaining_delta))
                .or_insert(remaining_delta);
            result_amount = result_amount.checked_add(result_delta)?;
            remaining_amount = remaining_amount.checked_sub(remaining_delta)?;
            fee = fee.checked_add(fee_delta)?;
        }

        Some((
            swap_info,
            AggregatedSwapOutcome {
                distribution: distribution
                    .into_iter()
                    .map(|(source, amount)| {
                        (source, QuoteAmount::with_variant(self.variant, amount))
                    })
                    .collect(),
                amount: result_amount,
                fee: OutcomeFee::<AssetId, Balance>::xor(fee), // todo fix (m.tagirov)
            },
        ))
    }

    /// Find liquidity sources where the top chunk has the best price.
    fn find_best_price_candidates(&self) -> Vec<LiquiditySourceType> {
        let mut candidates = Vec::new();
        let mut max = fixed!(0);
        for (source, chunks) in self.liquidity_chunks.iter() {
            let Some(front) = chunks.front() else {
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
}

#[cfg(feature = "wip")] // ALT
#[cfg(test)]
mod tests {
    use crate::liquidity_aggregator::*;
    use common::prelude::{OutcomeFee, QuoteAmount, SwapVariant};
    use common::{balance, LiquiditySourceType, SwapChunk};
    use sp_std::collections::vec_deque::VecDeque;

    type AssetId = common::AssetId32<common::PredefinedAssetId>;

    fn get_liquidity_aggregator_with_desired_input_and_equal_chunks(
    ) -> LiquidityAggregator<LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);
        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), balance!(1)),
                SwapChunk::new(balance!(10), balance!(90), balance!(0.9)),
                SwapChunk::new(balance!(10), balance!(80), balance!(0.8)),
                SwapChunk::new(balance!(10), balance!(70), balance!(0.7)),
                SwapChunk::new(balance!(10), balance!(60), balance!(0.6)),
            ]),
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            VecDeque::from([
                SwapChunk::new(balance!(10), balance!(85), balance!(0.85)),
                SwapChunk::new(balance!(10), balance!(85), balance!(0.85)),
                SwapChunk::new(balance!(10), balance!(85), balance!(0.85)),
                SwapChunk::new(balance!(10), balance!(85), balance!(0.85)),
                SwapChunk::new(balance!(10), balance!(85), balance!(0.85)),
            ]),
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            VecDeque::from([
                SwapChunk::new(balance!(10), balance!(120), balance!(0)),
                SwapChunk::new(balance!(10), balance!(100), balance!(0)),
                SwapChunk::new(balance!(10), balance!(80), balance!(0)),
            ]),
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_output_and_equal_chunks(
    ) -> LiquidityAggregator<LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), balance!(1)),
                SwapChunk::new(balance!(11), balance!(100), balance!(1)),
                SwapChunk::new(balance!(12), balance!(100), balance!(1)),
                SwapChunk::new(balance!(13), balance!(100), balance!(1)),
                SwapChunk::new(balance!(14), balance!(100), balance!(1)),
            ]),
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            VecDeque::from([
                SwapChunk::new(balance!(12.5), balance!(100), balance!(1)),
                SwapChunk::new(balance!(12.5), balance!(100), balance!(1)),
                SwapChunk::new(balance!(12.5), balance!(100), balance!(1)),
                SwapChunk::new(balance!(12.5), balance!(100), balance!(1)),
                SwapChunk::new(balance!(12.5), balance!(100), balance!(1)),
            ]),
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            VecDeque::from([
                SwapChunk::new(balance!(8), balance!(100), balance!(0)),
                SwapChunk::new(balance!(10), balance!(100), balance!(0)),
                SwapChunk::new(balance!(13), balance!(100), balance!(0)),
            ]),
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_input_and_different_chunks(
    ) -> LiquidityAggregator<LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);
        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), balance!(1)),
                SwapChunk::new(balance!(12), balance!(108), balance!(1.08)),
                SwapChunk::new(balance!(14), balance!(112), balance!(1.12)),
                SwapChunk::new(balance!(16), balance!(112), balance!(1.12)),
                SwapChunk::new(balance!(18), balance!(108), balance!(1.08)),
            ]),
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            VecDeque::from([
                SwapChunk::new(balance!(10), balance!(85), balance!(0.85)),
                SwapChunk::new(balance!(11), balance!(93.5), balance!(0.935)),
                SwapChunk::new(balance!(12), balance!(102), balance!(1.02)),
                SwapChunk::new(balance!(13), balance!(110.5), balance!(1.105)),
                SwapChunk::new(balance!(14), balance!(119), balance!(1.19)),
            ]),
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            VecDeque::from([
                SwapChunk::new(balance!(12), balance!(144), balance!(0)),
                SwapChunk::new(balance!(10), balance!(100), balance!(0)),
                SwapChunk::new(balance!(14), balance!(112), balance!(0)),
            ]),
        );

        aggregator
    }

    fn get_liquidity_aggregator_with_desired_output_and_different_chunks(
    ) -> LiquidityAggregator<LiquiditySourceType> {
        let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

        aggregator.add_source(
            LiquiditySourceType::XYKPool,
            VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), balance!(0)),
                SwapChunk::new(balance!(5.5), balance!(50), balance!(0)),
                SwapChunk::new(balance!(3), balance!(25), balance!(0)),
                SwapChunk::new(balance!(26), balance!(200), balance!(0)),
                SwapChunk::new(balance!(7), balance!(50), balance!(0)),
            ]),
        );

        aggregator.add_source(
            LiquiditySourceType::XSTPool,
            VecDeque::from([
                SwapChunk::new(balance!(12.5), balance!(100), balance!(0)),
                SwapChunk::new(balance!(10), balance!(80), balance!(0)),
                SwapChunk::new(balance!(9), balance!(72), balance!(0)),
                SwapChunk::new(balance!(8), balance!(64), balance!(0)),
                SwapChunk::new(balance!(7), balance!(56), balance!(0)),
            ]),
        );

        aggregator.add_source(
            LiquiditySourceType::OrderBook,
            VecDeque::from([
                SwapChunk::new(balance!(8), balance!(100), balance!(0)),
                SwapChunk::new(balance!(9), balance!(90), balance!(0)),
                SwapChunk::new(balance!(13), balance!(100), balance!(0)),
            ]),
        );

        aggregator
    }

    #[test]
    fn check_find_best_price_candidates_with_desired_input_and_equal_chunks() {
        let mut aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::OrderBook]);

        // remove order book chunk 1
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // remove order book chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
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
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // remove order book chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 3
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(1))
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(1.9))
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(2.75))
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(3.6))
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(1))
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(2))
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(3))
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(4))
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(5))
                )
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
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // remove order book chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
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
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(
            candidates,
            vec![LiquiditySourceType::XYKPool, LiquiditySourceType::OrderBook]
        );

        // remove order book chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::OrderBook)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 1
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XYKPool]);

        // remove xyk pool chunk 3
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XYKPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 1
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
        assert_eq!(candidates, vec![LiquiditySourceType::XSTPool]);

        // remove xst pool chunk 2
        aggregator
            .liquidity_chunks
            .get_mut(&LiquiditySourceType::XSTPool)
            .unwrap()
            .pop_front();

        let candidates = aggregator.find_best_price_candidates();
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(0.8))
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(1.719999999999999999))
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(2.59))
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
                    OutcomeFee::<AssetId, Balance>::xor(balance!(3.439999999999999999))
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::new()
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
                    OutcomeFee::<AssetId, Balance>::new()
                )
            )
        );
    }
}
