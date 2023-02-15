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

use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use frame_support::weights::Weight;
use sp_runtime::traits::Get;
use sp_std::marker::PhantomData;

use common::prelude::SwapVariant;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn swap(variant: SwapVariant) -> Weight {
        // Todo: Use all 6 weight formulas defined in benchmarks
        match variant {
            // swap_exact_input_multiple
            SwapVariant::WithDesiredInput => Weight::zero(),
            //swap_exact_output_multiple
            SwapVariant::WithDesiredOutput => Weight::zero(),
        }
    }
    fn enable_liquidity_source() -> Weight {
        Weight::from_ref_time(21_575_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn disable_liquidity_source() -> Weight {
        Weight::from_ref_time(20_003_000)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    // Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
    // Storage: DEXManager DEXInfos (r:1 w:0)
    // Storage: XSTPool EnabledSynthetics (r:1 w:0)
    // Storage: DEXAPI EnabledSourceTypes (r:1 w:0)
    // Storage: PoolXYK Properties (r:1 w:0)
    // Storage: TradingPair LockedLiquiditySources (r:1 w:0)
    // Storage: System Account (r:103 w:103)
    // Storage: Tokens Accounts (r:102 w:102)
    // Storage: Technical TechAccounts (r:2 w:0)
    // Storage: PriceTools PriceInfos (r:1 w:0)
    // Storage: PoolXYK Reserves (r:0 w:1)
    /// The range of component `n` is `[1, 10]`.
    /// The range of component `m` is `[10, 100]`.
    fn swap_transfer_batch(n: u32, m: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 42_166_000
            .saturating_add((1_601_298_000 as Weight).saturating_mul(n as Weight))
            // Standard Error: 4_153_000
            .saturating_add((240_616_000 as Weight).saturating_mul(m as Weight))
            .saturating_add(T::DbWeight::get().reads((8 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().reads((2 as Weight).saturating_mul(m as Weight)))
            .saturating_add(T::DbWeight::get().writes((5 as Weight).saturating_mul(n as Weight)))
            .saturating_add(T::DbWeight::get().writes((2 as Weight).saturating_mul(m as Weight)))
    }
}

impl crate::WeightInfo for () {
    fn swap(_variant: SwapVariant) -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn enable_liquidity_source() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn disable_liquidity_source() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn swap_transfer_batch(_: u32, _: u32) -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
