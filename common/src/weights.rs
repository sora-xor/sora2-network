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

use frame_support::parameter_types;
use frame_support::weights::constants::{
    BlockExecutionWeight, ExtrinsicBaseWeight, WEIGHT_REF_TIME_PER_SECOND,
};
use frame_support::weights::Weight;
use frame_system::limits;
use sp_arithmetic::Perbill;
use sp_std::marker::PhantomData;

use crate::primitives::Balance;
use frame_support::dispatch::{
    DispatchClass, DispatchErrorWithPostInfo, DispatchResultWithPostInfo, Pays,
};
use sp_runtime::DispatchError;

pub mod constants {
    use crate::{balance, Balance};
    use frame_support::weights::Weight;

    pub const EXTRINSIC_FIXED_WEIGHT: Weight = Weight::from_parts(100_000_000, 0);
    pub const SMALL_FEE: Balance = balance!(0.0007);
    pub const BIG_FEE: Balance = balance!(0.007);
}

pub struct PresetWeightInfo<T>(PhantomData<T>);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight =
    Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_SECOND, u64::MAX);
pub const ON_INITIALIZE_RATIO: Perbill = Perbill::from_perthousand(20);

parameter_types! {
    /// Block weights base values and limits.
    pub BlockWeights: limits::BlockWeights = limits::BlockWeights::builder()
    .base_block(BlockExecutionWeight::get())
    .for_class(DispatchClass::all(), |weights| {
        weights.base_extrinsic = ExtrinsicBaseWeight::get();
    })
    .for_class(DispatchClass::Normal, |weights| {
        weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
    })
    .for_class(DispatchClass::Operational, |weights| {
        weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
        // Operational transactions have an extra reserved space, so that they
        // are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
        weights.reserved = Some(
            MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT,
        );
    })
    .avg_block_initialization(ON_INITIALIZE_RATIO)
    .build_or_panic();
    pub BlockLength: limits::BlockLength =
        limits::BlockLength::max_with_normal_ratio(7 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub const TransactionByteFee: Balance = 0;
}

#[inline(always)]
pub fn pays_no_with_maybe_weight<E: Into<DispatchError>>(
    result: Result<Option<Weight>, E>,
) -> DispatchResultWithPostInfo {
    result
        .map_err(|e| DispatchErrorWithPostInfo {
            post_info: Pays::No.into(),
            error: e.into(),
        })
        .map(|weight| (weight, Pays::No).into())
}

#[inline(always)]
pub fn pays_no<T, E: Into<DispatchError>>(result: Result<T, E>) -> DispatchResultWithPostInfo {
    pays_no_with_maybe_weight(result.map(|_| None))
}

#[inline(always)]
pub fn err_pays_no(err: impl Into<DispatchError>) -> DispatchErrorWithPostInfo {
    DispatchErrorWithPostInfo {
        post_info: Pays::No.into(),
        error: err.into(),
    }
}
