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
    BlockExecutionWeight, ExtrinsicBaseWeight, WEIGHT_PER_SECOND,
};
use frame_support::weights::{
    DispatchClass, Weight, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use frame_system::limits;
use smallvec::smallvec;
use sp_arithmetic::Perbill;
use sp_std::marker::PhantomData;

use crate::primitives::Balance;
pub mod constants {
    use frame_support::weights::Weight;

    pub const EXTRINSIC_FIXED_WEIGHT: Weight = 100_000_000;
}

pub struct PresetWeightInfo<T>(PhantomData<T>);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;
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
        limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub const TransactionByteFee: Balance = 0;
}

pub struct WeightToFixedFee;

impl WeightToFeePolynomial for WeightToFixedFee {
    type Balance = Balance;

    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        smallvec!(WeightToFeeCoefficient {
            coeff_integer: 7_000_000,
            coeff_frac: Perbill::zero(),
            negative: false,
            degree: 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::balance;
    use super::*;
    use frame_support::weights::Weight;

    type Fee = WeightToFixedFee;

    #[test]
    fn weight_to_fixed_fee_works() {
        assert_eq!(Fee::calc(&100_000_000_000), balance!(0.7));
        assert_eq!(Fee::calc(&500_000_000), balance!(0.0035));
        assert_eq!(Fee::calc(&72_000_000), balance!(0.000504));
        assert_eq!(Fee::calc(&210_200_000_000), balance!(1.4714));
    }

    #[test]
    fn weight_to_fixed_fee_does_not_underflow() {
        assert_eq!(Fee::calc(&0), 0);
    }

    #[test]
    fn weight_to_fixed_fee_does_not_overflow() {
        assert_eq!(Fee::calc(&Weight::max_value()), 129127208515966861305000000);
    }
}
