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

use common::Balance;
use sp_arithmetic::traits::{EnsureAdd, EnsureMul, Saturating};
use sp_arithmetic::{ArithmeticError, FixedU128};

#[cfg(test)]
use common::balance;

/// Per second compounding formula
///
/// Interest is accrued over number of periods
/// `A = P * (1 + period_rate) ^ period`, where:
/// - principal - (P) - initial balance
/// - rate_per_second - (period_rate) - rate per second, where
/// rate_secondly = (1 + rate_annual)^(1/seconds_per_year) - 1
/// - period - time passed in seconds
/// Returns (A) - the new balance with interest over time
pub fn compound(
    principal: Balance,
    rate_per_second: FixedU128,
    period: u64,
) -> Result<Balance, ArithmeticError> {
    let res = FixedU128::from_inner(principal)
        .ensure_mul(
            FixedU128::from(1)
                .ensure_add(rate_per_second)?
                .saturating_pow(period as usize),
        )?
        .into_inner();
    if res == Balance::MAX {
        Err(ArithmeticError::Overflow)
    } else {
        Ok(res)
    }
}

#[test]
fn tests_compound_zero_rate() {
    let initial_balance = balance!(10000);
    let rate = FixedU128::from(0);
    // 1 year in seconds
    let time = 31556952;
    // balance shall not change
    assert_eq!(
        compound(initial_balance, rate, time).unwrap(),
        initial_balance
    );
}

#[test]
fn test_compound_zero_principal() {
    let initial_balance = balance!(0);
    let rate = FixedU128::from(11);
    // 1 year in seconds
    let time = 31556952;
    // shall not change
    assert_eq!(
        compound(initial_balance, rate, time).unwrap(),
        initial_balance
    );
}

#[test]
fn test_compound_0_period() {
    // per second rate
    let rate = FixedU128::from_float(0.15);
    let initial_balance = balance!(100);
    // 1 second
    let time = 0;
    assert_eq!(
        compound(initial_balance, rate, time).unwrap(),
        balance!(100)
    );
}

#[test]
fn test_compound_1_period() {
    // per second rate
    let rate = FixedU128::from_float(0.15);
    let initial_balance = balance!(100);
    // 1 second
    let time = 1;
    assert_eq!(
        compound(initial_balance, rate, time).unwrap(),
        balance!(115)
    );
}

#[test]
fn test_compound_2_periods() {
    // per second rate
    let rate = FixedU128::from_float(0.1);
    let initial_balance = balance!(100);
    // 1 second
    let time = 2;
    assert_eq!(
        compound(initial_balance, rate, time).unwrap(),
        balance!(121)
    );
}

#[test]
fn test_compound_overflow() {
    // per second rate
    let rate = FixedU128::from_float(0.15);
    let initial_balance = Balance::MAX;
    // 1 second
    let time = 1;
    assert_eq!(
        compound(initial_balance, rate, time),
        Err(ArithmeticError::Overflow)
    );
}
