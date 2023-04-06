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

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn enable_liquidity_source() -> Weight {
        Weight::from_parts(21_575_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn disable_liquidity_source() -> Weight {
        Weight::from_parts(20_003_000, 0)
            .saturating_add(T::DbWeight::get().reads(1))
            .saturating_add(T::DbWeight::get().writes(1))
    }
    fn check_indivisible_assets() -> Weight {
        Weight::zero()
    }
    fn new_trivial() -> Weight {
        Weight::zero()
    }
    fn is_forbidden_filter() -> Weight {
        Weight::zero()
    }
    fn list_liquidity_sources() -> Weight {
        Weight::zero()
    }
}

impl crate::WeightInfo for () {
    fn enable_liquidity_source() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn disable_liquidity_source() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn check_indivisible_assets() -> Weight {
        EXTRINSIC_FIXED_WEIGHT.saturating_div(10)
    }
    fn new_trivial() -> Weight {
        EXTRINSIC_FIXED_WEIGHT.saturating_div(10)
    }
    fn is_forbidden_filter() -> Weight {
        EXTRINSIC_FIXED_WEIGHT.saturating_div(10)
    }
    fn list_liquidity_sources() -> Weight {
        EXTRINSIC_FIXED_WEIGHT.saturating_div(4)
    }
}
