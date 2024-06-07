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
use frame_support::weights::{Weight, WeightMeter};
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
    pub const SMALL_REFERENCE_AMOUNT: Balance = balance!(0.2);
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

/// Check if it possible to consume the given weight `max_n` times.
/// If weight is only enough to consume `n <= max_n` times, it returns `n`.
/// If `consume_weight` is true, it consumes it `n` times.
pub fn check_accrue_n(meter: &mut WeightMeter, w: Weight, max_n: u64, consume_weight: bool) -> u64 {
    let n = {
        let weight_left = meter.remaining();
        // Maximum possible subtractions that we can do on each value
        // If None, then can subtract the value infinitely
        // thus we can use max value (more will likely be infeasible)
        let n_ref_time = weight_left
            .ref_time()
            .checked_div(w.ref_time())
            .unwrap_or(u64::MAX);
        let n_proof_size = weight_left
            .proof_size()
            .checked_div(w.proof_size())
            .unwrap_or(u64::MAX);
        let max_possible_n = n_ref_time.min(n_proof_size);
        max_possible_n.min(max_n)
    };

    if consume_weight {
        // `n` was obtained as integer division `left/w`, so multiplying `n*w` will not exceed `left`;
        // it means it will fit into u64
        let to_consume = w.saturating_mul(n);
        meter.defensive_saturating_accrue(to_consume);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::check_accrue_n;

    #[test]
    fn test_check_accrue_n_works() {
        // Within limits
        let mut weight_counter = frame_support::weights::WeightMeter::from_limit(100.into());
        assert_eq!(
            check_accrue_n(&mut weight_counter, 10.into(), 10, false),
            10,
            "Should accrue within limits"
        );
        assert_eq!(weight_counter.remaining(), 100.into());

        assert_eq!(
            check_accrue_n(&mut weight_counter, 10.into(), 10, true),
            10,
            "Should accrue within limits"
        );
        assert_eq!(weight_counter.remaining(), 0.into());

        // Just above limit
        let mut weight_counter = frame_support::weights::WeightMeter::from_limit(100.into());
        assert_eq!(
            check_accrue_n(&mut weight_counter, 11.into(), 10, false),
            9,
            "Should partially accrue"
        );
        assert_eq!(weight_counter.remaining(), 100.into());

        assert_eq!(
            check_accrue_n(&mut weight_counter, 11.into(), 10, true),
            9,
            "Should partially accrue"
        );
        assert_eq!(weight_counter.remaining(), 1.into()); // 100-99

        // Can't accrue at all
        let mut weight_counter = frame_support::weights::WeightMeter::from_limit(100.into());
        assert_eq!(
            check_accrue_n(&mut weight_counter, 101.into(), 1, false),
            0,
            "Should restrict even a single consumption exceeding limits"
        );
        assert_eq!(weight_counter.remaining(), 100.into());

        // Won't accrue if 0 needed
        assert_eq!(
            check_accrue_n(&mut weight_counter, 1.into(), 0, true),
            0,
            "Should work with 0 maximum consumptions"
        );
        assert_eq!(weight_counter.remaining(), 100.into());

        // 0 weight is freely consumed
        assert_eq!(
            check_accrue_n(&mut weight_counter, 0.into(), u64::MAX, false),
            u64::MAX,
            "0 weight should be allowed to be consumed infinitely (max_int is reasonably high for weight)"
        );
        assert_eq!(weight_counter.remaining(), 100.into());

        assert_eq!(
            check_accrue_n(&mut weight_counter, 0.into(), u64::MAX, true),
            u64::MAX,
            "0 weight should be allowed to be consumed infinitely (max_int is reasonably high for weight)"
        );
        assert_eq!(weight_counter.remaining(), 100.into());
    }
}
