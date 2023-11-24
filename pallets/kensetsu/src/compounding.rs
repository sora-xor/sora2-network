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

use common::{balance, Balance};
use libm::pow;
use sp_arithmetic::FixedU128;

#[derive(Debug)]
pub enum Error {
    ArithmeticError,
    Overflow,
}

/// Continuous compounding formula
///
/// Interest is accrued over infinite number of periods
/// - initial_balance - initial balance
/// - rate - annual percentage rate
/// - time - period in seconds
/// Returns the new balance with interest over time
pub fn continuous_compound(
    initial_balance: Balance,
    rate: FixedU128,
    time: u64,
) -> Result<Balance, Error> {
    let euler_number = 2.71828182845904523536028747135266250_f64;
    // Seconds in Gregorian calendar year
    let seconds_in_year = 31556952f64;
    let time_in_years = time as f64 / seconds_in_year;

    // TODO implement without std
    // let res =
    //     (initial_balance as f64 * pow(euler_number, rate.to_float() * time_in_years)) as Balance;
    // if res == Balance::MAX {
    //     Err(Error::Overflow)
    // } else {
    //     Ok(res)
    // }

    Ok(initial_balance)
}

/// Returns accrued interest using continuous compouding formula
///
/// - initial_balance - initial balance
/// - rate - annual percentage rate
/// - time - period in seconds
pub fn get_accrued_interest(
    loan_balance: Balance,
    rate: FixedU128,
    time: u64,
) -> Result<Balance, Error> {
    let new_loan_balance = continuous_compound(loan_balance, rate, time)?;
    new_loan_balance
        .checked_sub(loan_balance)
        .ok_or(Error::ArithmeticError)
}

#[test]
fn test_contionuous_compound() {
    // 10.000 with 15% interest for 1 year = 11618,342427282829737984
    let initial_balance = balance!(10000);
    let rate = FixedU128::from_float(0.15);
    let time = 31556952; // 1 year in seconds
    assert_eq!(
        continuous_compound(initial_balance, rate, time).unwrap(),
        balance!(11618.342427282829737984)
    );
}

#[test]
fn test_contionuous_compound_zero_rate() {
    let initial_balance = balance!(10000);
    let rate = FixedU128::from_float(0f64);
    let time = 31556952; // 1 year in seconds
    assert_eq!(
        continuous_compound(initial_balance, rate, time).unwrap(),
        initial_balance
    );
}

#[test]
fn test_accrued_interest() {
    // 10.000 with 15% interest for 1 year = 11618,342427282829737984
    let initial_balance = balance!(10000);
    let rate = FixedU128::from_float(0.15);
    let time = 31556952; // 1 year in seconds
    assert_eq!(
        get_accrued_interest(initial_balance, rate, time).unwrap(),
        balance!(1618.342427282829737984)
    );
}
