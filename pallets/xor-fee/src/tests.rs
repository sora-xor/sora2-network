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

use crate::mock::*;
use common::balance;

use frame_support::error::BadOrigin;
use frame_support::weights::{Weight, WeightToFee};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::{FixedPointNumber, FixedU128};

fn set_weight_to_fee_multiplier(mul: u64) {
    // Set WeightToFee multiplier to one to not affect the test
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(mul)
    ));
}

#[test]
fn weight_to_fee_works() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        set_weight_to_fee_multiplier(1);
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(100_000_000_000, 0)),
            balance!(0.7)
        );
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(500_000_000, 0)),
            balance!(0.0035)
        );
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(72_000_000, 0)),
            balance!(0.000504)
        );
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(210_200_000_000, 0)),
            balance!(1.4714)
        );
    });
}

#[test]
fn weight_to_fee_does_not_underflow() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        assert_eq!(XorFee::weight_to_fee(&Weight::zero()), 0);
    });
}

#[test]
fn weight_to_fee_does_not_overflow() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        set_weight_to_fee_multiplier(1);
        assert_eq!(
            XorFee::weight_to_fee(&Weight::MAX),
            129127208515966861305000000
        );
    });
}

#[test]
fn simple_update_works() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        // Update from root
        set_weight_to_fee_multiplier(3);
        assert_eq!(XorFee::multiplier(), FixedU128::saturating_from_integer(3));
    });
}

#[test]
fn non_root_update_fails() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        // We allow only root

        assert_noop!(
            XorFee::update_multiplier(RuntimeOrigin::signed(1), FixedU128::from(3)),
            BadOrigin
        );

        assert_noop!(
            XorFee::update_multiplier(RuntimeOrigin::none(), FixedU128::from(3)),
            BadOrigin
        );
    });
}
