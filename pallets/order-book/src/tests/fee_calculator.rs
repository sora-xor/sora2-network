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

#![cfg(feature = "wip")] // order-book

use common::balance;
use common::prelude::constants::SMALL_FEE;
use framenode_runtime::order_book::fee_calculator::FeeCalculator;
use framenode_runtime::order_book::Config;
use framenode_runtime::Runtime;

#[test]
fn should_calculate_place_limit_order_fee() {
    let max_lifetime = <Runtime as Config>::MAX_ORDER_LIFESPAN;

    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(0), false, false).unwrap(),
        balance!(0.0002)
    );

    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(max_lifetime / 10), false, false)
            .unwrap(),
        balance!(0.000215)
    );

    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(max_lifetime / 5), false, false)
            .unwrap(),
        balance!(0.00023)
    );

    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(max_lifetime / 2), false, false)
            .unwrap(),
        balance!(0.000275)
    );

    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(max_lifetime), false, false).unwrap(),
        SMALL_FEE / 2
    );
}

#[test]
fn should_calculate_place_limit_order_fee_with_weight() {
    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(1000), true, false).unwrap(),
        SMALL_FEE
    );
    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(0), true, false).unwrap(),
        SMALL_FEE
    );
    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(1000000000000000), true, false)
            .unwrap(),
        SMALL_FEE
    );
    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(None, true, false).unwrap(),
        SMALL_FEE
    );
}

#[test]
fn should_calculate_place_limit_order_fee_with_error() {
    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(1000), false, true).unwrap(),
        SMALL_FEE
    );
    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(0), false, true).unwrap(),
        SMALL_FEE
    );
    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(Some(1000000000000000), false, true)
            .unwrap(),
        SMALL_FEE
    );
    assert_eq!(
        FeeCalculator::<Runtime>::place_limit_order_fee(None, false, true).unwrap(),
        SMALL_FEE
    );
}
