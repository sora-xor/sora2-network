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

use crate::{Config, Error};
use common::alt::SwapChunk;
use common::AssetIdOf;
use common::Balance;
use sp_runtime::DispatchError;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::vec_deque::VecDeque;
use sp_std::vec::Vec;

/// Cluster of liquidity that stores the aggregated liquidity chunks from one source.
#[derive(Debug, Clone, Default)]
pub(crate) struct Cluster<T: Config> {
    total: SwapChunk<AssetIdOf<T>, Balance>,
    chunks: VecDeque<SwapChunk<AssetIdOf<T>, Balance>>,
}

impl<T: Config> Cluster<T> {
    pub fn new() -> Self {
        Self {
            total: Default::default(),
            chunks: VecDeque::new(),
        }
    }

    pub fn get_total(&self) -> &SwapChunk<AssetIdOf<T>, Balance> {
        &self.total
    }

    pub fn push_back(&mut self, chunk: SwapChunk<AssetIdOf<T>, Balance>) {
        self.chunks.push_back(chunk.clone());
        self.total = self.total.clone().saturating_add(chunk);
    }

    pub fn pop_back(&mut self) -> Option<SwapChunk<AssetIdOf<T>, Balance>> {
        let Some(chunk) = self.chunks.pop_back() else {
            return None;
        };
        self.total = self.total.clone().saturating_sub(chunk.clone());
        Some(chunk)
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
}

/// Aggregation of liquidity from all sources.
#[derive(Clone)]
pub(crate) struct Aggregation<T: Config, LiquiditySourceType>(
    pub BTreeMap<LiquiditySourceType, Cluster<T>>,
);

impl<T, LiquiditySourceType> Aggregation<T, LiquiditySourceType>
where
    T: Config,
    LiquiditySourceType: Ord + Clone,
{
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn get_total(&self, source: &LiquiditySourceType) -> SwapChunk<AssetIdOf<T>, Balance> {
        self.0
            .get(source)
            .map(|cluster| cluster.get_total())
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_mut_cluster(
        &mut self,
        source: &LiquiditySourceType,
    ) -> Result<&mut Cluster<T>, DispatchError> {
        self.0
            .get_mut(source)
            .ok_or(Error::<T>::AggregationError.into())
    }

    pub fn push_chunk(
        &mut self,
        source: LiquiditySourceType,
        chunk: SwapChunk<AssetIdOf<T>, Balance>,
    ) {
        self.0
            .entry(source)
            .and_modify(|cluster| cluster.push_back(chunk.clone()))
            .or_insert_with(|| {
                let mut cluster = Cluster::new();
                cluster.push_back(chunk);
                cluster
            });
    }

    /// Returns the queue with sources in ascending order
    pub fn get_total_price_ascending_queue(&self) -> Vec<LiquiditySourceType> {
        let mut queue: Vec<_> = self
            .0
            .iter()
            .filter_map(|(source, cluster)| Some(source.clone()).zip(cluster.get_total().price()))
            .collect();
        queue.sort_by(|(_, price_left), (_, price_right)| price_left.cmp(price_right));
        queue.into_iter().map(|(source, _)| source).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::Runtime;
    use common::{balance, LiquiditySourceType};
    use sp_std::vec;

    #[test]
    fn check_price_ascending_queue() {
        let mut aggregation = Aggregation::<Runtime, _>::new();

        // xyk pool - average price = 9
        aggregation.push_chunk(
            LiquiditySourceType::XYKPool,
            SwapChunk::new(balance!(10), balance!(100), Default::default()),
        );
        aggregation.push_chunk(
            LiquiditySourceType::XYKPool,
            SwapChunk::new(balance!(10), balance!(80), Default::default()),
        );

        // xst pool - average price = 8
        aggregation.push_chunk(
            LiquiditySourceType::XSTPool,
            SwapChunk::new(balance!(10), balance!(80), Default::default()),
        );
        aggregation.push_chunk(
            LiquiditySourceType::XSTPool,
            SwapChunk::new(balance!(10), balance!(80), Default::default()),
        );

        // tbc pool - average price = 10
        aggregation.push_chunk(
            LiquiditySourceType::MulticollateralBondingCurvePool,
            SwapChunk::new(balance!(10), balance!(110), Default::default()),
        );
        aggregation.push_chunk(
            LiquiditySourceType::MulticollateralBondingCurvePool,
            SwapChunk::new(balance!(10), balance!(90), Default::default()),
        );

        // order book - average price = 13
        aggregation.push_chunk(
            LiquiditySourceType::OrderBook,
            SwapChunk::new(balance!(10), balance!(160), Default::default()),
        );
        aggregation.push_chunk(
            LiquiditySourceType::OrderBook,
            SwapChunk::new(balance!(10), balance!(100), Default::default()),
        );

        assert_eq!(
            aggregation.get_total_price_ascending_queue(),
            vec![
                LiquiditySourceType::XSTPool,                         // price 8
                LiquiditySourceType::XYKPool,                         // price 9
                LiquiditySourceType::MulticollateralBondingCurvePool, // price 10
                LiquiditySourceType::OrderBook                        // price 13
            ]
        );
    }
}
