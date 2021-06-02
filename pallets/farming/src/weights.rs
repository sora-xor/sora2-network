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
use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);

impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn refresh_pool(a: u32) -> Weight {
        (1_298_786_000 as Weight)
            // Standard Error: 4_529_000
            .saturating_add((152_660_000 as Weight).saturating_mul(a as Weight))
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(a as Weight)))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }

    fn prepare_accounts_for_vesting(a: u32, b: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 218_372_000
            .saturating_add((7_588_988_000 as Weight).saturating_mul(a as Weight))
            // Standard Error: 152_744_000
            .saturating_add((5_509_396_000 as Weight).saturating_mul(b as Weight))
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().reads((5 as Weight).saturating_mul(a as Weight)))
    }

    fn vest_account_rewards(a: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 9_371_000
            .saturating_add((274_031_000 as Weight).saturating_mul(a as Weight))
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().reads((2 as Weight).saturating_mul(a as Weight)))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
            .saturating_add(T::DbWeight::get().writes((2 as Weight).saturating_mul(a as Weight)))
    }

    fn save_data(a: u32, b: u32) -> Weight {
        (0 as Weight)
            // Standard Error: 211_137_000
            .saturating_add((7_336_948_000 as Weight).saturating_mul(a as Weight))
            // Standard Error: 147_683_000
            .saturating_add((4_968_017_000 as Weight).saturating_mul(b as Weight))
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().reads((5 as Weight).saturating_mul(a as Weight)))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
}

impl crate::WeightInfo for () {
    fn refresh_pool(_a: u32) -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }

    fn prepare_accounts_for_vesting(_a: u32, _b: u32) -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }

    fn vest_account_rewards(_a: u32) -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }

    fn save_data(_a: u32, _b: u32) -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
