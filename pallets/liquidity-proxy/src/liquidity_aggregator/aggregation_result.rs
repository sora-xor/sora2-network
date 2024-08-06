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
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;
#[cfg(feature = "wip")] // ALT
use {common::prelude::SwapVariant, sp_std::collections::btree_map::BTreeMap};

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
pub type SwapInfo<LiquiditySourceType, AmountType> =
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
