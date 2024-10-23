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

use crate::liquidity_aggregator::aggregation::{Aggregation, Cluster};
use crate::{Config, Error};
use common::alt::{DiscreteQuotation, SideAmount, SwapChunk, SwapLimits};
use common::prelude::SwapVariant;
use common::AssetIdOf;
use common::{fixed, Balance};
use itertools::Itertools;
use sp_runtime::traits::Zero;
use sp_runtime::DispatchError;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;

/// Selector is intended to store undistributed liquidity from all sources and provide the best next liquidity chunk.
#[derive(Debug, Clone)]
pub(crate) struct Selector<T: Config, LiquiditySourceType> {
    variant: SwapVariant,
    liquidity_quotations: BTreeMap<LiquiditySourceType, DiscreteQuotation<AssetIdOf<T>, Balance>>,
    locked_sources: BTreeSet<LiquiditySourceType>,
}

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
            .get_mut(source)
            .ok_or(Error::<T>::AggregationError)?;
        quotation.chunks.push_front(chunk);
        Ok(())
    }

    /// Takes chunks from `cluster` and puts them back into the selector until it reaches `amount`.
    /// Returns the returned amount of liquidity and the flag true if `cluster` became empty.
    pub fn return_liquidity(
        &mut self,
        mut amount: SideAmount<Balance>,
        source: &LiquiditySourceType,
        cluster: &mut Cluster<T>,
    ) -> Result<(SwapChunk<AssetIdOf<T>, Balance>, bool), DispatchError> {
        let mut taken = SwapChunk::default();
        while *amount.amount() > Balance::zero() {
            // it is necessary to return chunks back till `remainder` volume is filled
            let Some(chunk) = cluster.pop_back() else {
                break;
            };
            if chunk <= amount {
                let value = amount
                    .amount()
                    .checked_sub(*chunk.get_same_type_amount(&amount).amount())
                    .ok_or(Error::<T>::CalculationError)?;
                amount.set_amount(value);
                taken = taken.saturating_add(chunk.clone());
                self.push_chunk(source, chunk)?;
            } else {
                let remainder_chunk = chunk
                    .clone()
                    .rescale_by_side_amount(amount)
                    .ok_or(Error::<T>::CalculationError)?;
                let chunk = chunk.saturating_sub(remainder_chunk.clone());
                cluster.push_back(chunk);
                taken = taken.saturating_add(remainder_chunk.clone());
                self.push_chunk(source, remainder_chunk)?;
                amount.set_amount(Balance::zero());
            }
        }
        Ok((taken, cluster.is_empty()))
    }

    /// Selects the chunk with best price.
    /// If there are several best chunks, we select the source that already was selected before.
    /// If the source has the precision limit and `amount` is less than precision - this source is used only if there are no other candidates even if it has the best price.
    pub fn select_chunk(
        &mut self,
        amount: Balance,
        aggregation: &Aggregation<T, LiquiditySourceType>,
    ) -> Result<(LiquiditySourceType, SwapChunk<AssetIdOf<T>, Balance>), DispatchError> {
        let mut candidates = Vec::new();
        let mut delayed = None;
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

            let step = discrete_quotation
                .limits
                .get_precision_step(&front, self.variant)
                .ok_or(Error::<T>::CalculationError)?;

            if price == max && amount >= step {
                candidates.push(source.clone());
            }

            if price > max {
                if amount < step {
                    delayed = Some(source.clone());
                } else {
                    candidates.clear();
                    max = price;
                    candidates.push(source.clone());
                }
            }
        }

        let source = if let Some(mut source) = candidates.first().cloned() {
            // if there are several candidates with the same best price,
            // then we need to select the source that already been selected
            for candidate in candidates {
                if aggregation.0.keys().contains(&candidate) {
                    source = candidate;
                    break;
                }
            }
            source
        } else {
            delayed.ok_or(Error::<T>::InsufficientLiquidity)?
        };

        let chunk = self
            .liquidity_quotations
            .get_mut(&source)
            .ok_or(Error::<T>::AggregationError)?
            .chunks
            .pop_front()
            .ok_or(Error::<T>::AggregationError)?;

        Ok((source, chunk))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::Runtime;
    use common::prelude::OutcomeFee;
    use common::{balance, LiquiditySourceType};
    use frame_support::assert_err;
    use sp_std::collections::vec_deque::VecDeque;

    #[test]
    fn check_select_chunk_with_regular_amount() {
        let amount = balance!(1000); // regular amount

        let mut selector: Selector<Runtime, _> = Selector::new(SwapVariant::WithDesiredInput);
        selector.add_source(
            LiquiditySourceType::XykPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                ]),
                limits: Default::default(),
            },
        );
        selector.add_source(
            LiquiditySourceType::XstPool,
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

        let mut aggregation = Aggregation::new();

        // select Order Book because it has the best price
        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(120), Default::default())
        );
        aggregation.push_chunk(source, chunk);

        // select Order Book instead of XYK pool because it was already selected
        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(100), Default::default())
        );
        aggregation.push_chunk(source, chunk);

        // just take the best price in all cases below

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XykPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1)))
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XykPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9)))
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(87), Default::default()),
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XstPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85)))
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XstPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85)))
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(80), Default::default()),
        );
        aggregation.push_chunk(source, chunk);

        // liquidity is empty
        assert_err!(
            selector.select_chunk(amount, &aggregation),
            Error::<Runtime>::InsufficientLiquidity
        );
    }

    #[test]
    fn check_select_chunk_with_small_amount() {
        let amount = balance!(0.01); // small amount < precision

        let mut selector: Selector<Runtime, _> = Selector::new(SwapVariant::WithDesiredInput);
        selector.add_source(
            LiquiditySourceType::XykPool,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                    SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                ]),
                limits: Default::default(),
            },
        );
        selector.add_source(
            LiquiditySourceType::XstPool,
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

        let mut aggregation = Aggregation::new();

        // Order Book has the best price, but the amount < precision. In this case Order Book moves to the end of the queue.
        // just take the best price in all cases below, excluding Order Book.

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XykPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1)))
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XykPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9)))
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XstPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85)))
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::XstPool);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85)))
        );
        aggregation.push_chunk(source, chunk);

        // start to take chunks from Order Book because other sources have ended

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(120), Default::default())
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(100), Default::default())
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(87), Default::default()),
        );
        aggregation.push_chunk(source, chunk);

        let (source, chunk) = selector.select_chunk(amount, &aggregation).unwrap();
        assert_eq!(source, LiquiditySourceType::OrderBook);
        assert_eq!(
            chunk,
            SwapChunk::new(balance!(10), balance!(80), Default::default()),
        );
        aggregation.push_chunk(source, chunk);

        // liquidity is empty
        assert_err!(
            selector.select_chunk(amount, &aggregation),
            Error::<Runtime>::InsufficientLiquidity
        );
    }
}
