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

use crate::prelude::FixedWrapper;
use crate::{balance, Balance};
use core::ops::{Add, Div, Mul, Sub};
use fixnum::ArithmeticError;
use sp_arithmetic::traits::IntegerSquareRoot;

#[cfg(feature = "std")]
use {sp_std::fmt::Display, static_assertions::_core::fmt::Formatter};

/// BalanceUnit wraps Balance and provides proper math operations between divisible & non-divisible balances that have different precision.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BalanceUnit {
    inner: Balance,

    /// Defines how to interpret the `inner` value.
    /// true - means the precision of 18 digits
    /// false - means the precision of 0 digits
    is_divisible: bool,
}

impl Default for BalanceUnit {
    fn default() -> Self {
        Self::new(0, true)
    }
}

#[cfg(feature = "std")]
impl Display for BalanceUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        let s = if self.is_divisible {
            FixedWrapper::from(self.inner).get().unwrap().to_string()
        } else {
            self.inner.to_string()
        };
        write!(f, "{}", s)
    }
}

impl BalanceUnit {
    pub fn new(balance: Balance, is_divisible: bool) -> Self {
        Self {
            inner: balance,
            is_divisible,
        }
    }

    pub fn divisible(balance: Balance) -> Self {
        Self::new(balance, true)
    }

    pub fn indivisible(balance: Balance) -> Self {
        Self::new(balance, false)
    }

    pub fn get(&self) -> Balance {
        self.inner
    }

    pub fn set(&mut self, value: Balance) {
        self.inner = value
    }

    pub fn is_divisible(&self) -> bool {
        self.is_divisible
    }

    pub fn pow(&self, x: u32) -> Result<Self, ArithmeticError> {
        let balance = if self.is_divisible {
            FixedWrapper::from(self.inner).pow(x).try_into_balance()?
        } else {
            self.inner.checked_pow(x).ok_or(ArithmeticError::Overflow)?
        };
        Ok(Self::new(balance, self.is_divisible))
    }

    pub fn sqrt(&self) -> Result<Self, ArithmeticError> {
        let balance = if self.is_divisible {
            FixedWrapper::from(self.inner)
                .sqrt_accurate()
                .try_into_balance()?
        } else {
            self.inner.integer_sqrt()
        };
        Ok(Self::new(balance, self.is_divisible))
    }
}

impl From<Balance> for BalanceUnit {
    fn from(balance: Balance) -> Self {
        Self {
            inner: balance,
            is_divisible: true,
        }
    }
}

impl Add for BalanceUnit {
    type Output = Result<Self, ArithmeticError>;

    fn add(self, rhs: Self) -> Self::Output {
        let (left, right) = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => (balance!(self.inner), rhs.inner),
            (true, false) => (self.inner, balance!(rhs.inner)),
            _ => (self.inner, rhs.inner),
        };
        let balance = left.checked_add(right).ok_or(ArithmeticError::Overflow)?;
        Ok(Self::new(balance, self.is_divisible || rhs.is_divisible))
    }
}

impl Sub for BalanceUnit {
    type Output = Result<Self, ArithmeticError>;

    fn sub(self, rhs: Self) -> Self::Output {
        let (left, right) = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => (balance!(self.inner), rhs.inner),
            (true, false) => (self.inner, balance!(rhs.inner)),
            _ => (self.inner, rhs.inner),
        };
        let balance = left.checked_sub(right).ok_or(ArithmeticError::Overflow)?;
        Ok(Self::new(balance, self.is_divisible || rhs.is_divisible))
    }
}

impl Mul for BalanceUnit {
    type Output = Result<Self, ArithmeticError>;

    fn mul(self, rhs: Self) -> Self::Output {
        let balance = if self.is_divisible && rhs.is_divisible {
            (FixedWrapper::from(self.inner) * (FixedWrapper::from(rhs.inner))).try_into_balance()?
        } else {
            self.inner
                .checked_mul(rhs.inner)
                .ok_or(ArithmeticError::Overflow)?
        };
        Ok(Self::new(balance, self.is_divisible || rhs.is_divisible))
    }
}

impl Div for BalanceUnit {
    type Output = Result<Self, ArithmeticError>;

    fn div(self, rhs: Self) -> Self::Output {
        let balance = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => (FixedWrapper::from(balance!(self.inner))
                / (FixedWrapper::from(rhs.inner)))
            .try_into_balance()?,
            (true, true) => (FixedWrapper::from(self.inner) / (FixedWrapper::from(rhs.inner)))
                .try_into_balance()?,
            _ => self
                .inner
                .checked_div(rhs.inner)
                .ok_or(ArithmeticError::DivisionByZero)?,
        };
        Ok(Self::new(balance, self.is_divisible || rhs.is_divisible))
    }
}

#[cfg(test)]
mod tests {
    use crate::balance;
    use crate::balance_unit::*;
    use frame_support::assert_err;

    #[test]
    fn check_constructors() {
        assert_eq!(
            BalanceUnit::default(),
            BalanceUnit {
                inner: 0,
                is_divisible: true
            }
        );

        assert_eq!(
            BalanceUnit::new(10, false),
            BalanceUnit {
                inner: 10,
                is_divisible: false
            }
        );

        assert_eq!(
            BalanceUnit::divisible(11),
            BalanceUnit {
                inner: 11,
                is_divisible: true
            }
        );

        assert_eq!(
            BalanceUnit::indivisible(12),
            BalanceUnit {
                inner: 12,
                is_divisible: false
            }
        );
    }

    #[test]
    fn check_add() {
        assert_eq!(
            (BalanceUnit::divisible(balance!(1.1)) + BalanceUnit::divisible(balance!(1.2)))
                .unwrap(),
            BalanceUnit::divisible(balance!(2.3))
        );

        assert_eq!(
            (BalanceUnit::indivisible(2) + BalanceUnit::divisible(balance!(1.1))).unwrap(),
            BalanceUnit::divisible(balance!(3.1))
        );

        assert_eq!(
            (BalanceUnit::divisible(balance!(1.1)) + BalanceUnit::indivisible(3)).unwrap(),
            BalanceUnit::divisible(balance!(4.1))
        );

        assert_eq!(
            (BalanceUnit::indivisible(2) + BalanceUnit::indivisible(3)).unwrap(),
            BalanceUnit::indivisible(5)
        );
    }

    #[test]
    fn check_sub() {
        assert_eq!(
            (BalanceUnit::divisible(balance!(1.2)) - BalanceUnit::divisible(balance!(1.1)))
                .unwrap(),
            BalanceUnit::divisible(balance!(0.1))
        );

        assert_eq!(
            (BalanceUnit::indivisible(3) - BalanceUnit::divisible(balance!(1.1))).unwrap(),
            BalanceUnit::divisible(balance!(1.9))
        );

        assert_eq!(
            (BalanceUnit::divisible(balance!(4.1)) - BalanceUnit::indivisible(3)).unwrap(),
            BalanceUnit::divisible(balance!(1.1))
        );

        assert_eq!(
            (BalanceUnit::indivisible(5) - BalanceUnit::indivisible(3)).unwrap(),
            BalanceUnit::indivisible(2)
        );

        // check sub zero
        assert_err!(
            BalanceUnit::divisible(balance!(1)) - BalanceUnit::divisible(balance!(1.1)),
            ArithmeticError::Overflow
        );

        assert_err!(
            BalanceUnit::indivisible(3) - BalanceUnit::divisible(balance!(4.1)),
            ArithmeticError::Overflow
        );

        assert_err!(
            BalanceUnit::divisible(balance!(4.1)) - BalanceUnit::indivisible(5),
            ArithmeticError::Overflow
        );

        assert_err!(
            BalanceUnit::indivisible(5) - BalanceUnit::indivisible(6),
            ArithmeticError::Overflow
        );
    }

    #[test]
    fn check_mul() {
        assert_eq!(
            (BalanceUnit::divisible(balance!(1.1)) * BalanceUnit::divisible(balance!(1.2)))
                .unwrap(),
            BalanceUnit::divisible(balance!(1.32))
        );

        assert_eq!(
            (BalanceUnit::indivisible(2) * BalanceUnit::divisible(balance!(1.1))).unwrap(),
            BalanceUnit::divisible(balance!(2.2))
        );

        assert_eq!(
            (BalanceUnit::divisible(balance!(1.1)) * BalanceUnit::indivisible(3)).unwrap(),
            BalanceUnit::divisible(balance!(3.3))
        );

        assert_eq!(
            (BalanceUnit::indivisible(2) * BalanceUnit::indivisible(3)).unwrap(),
            BalanceUnit::indivisible(6)
        );
    }

    #[test]
    fn check_div() {
        // check regular cases
        assert_eq!(
            (BalanceUnit::divisible(balance!(5.55)) / BalanceUnit::divisible(balance!(3.7)))
                .unwrap(),
            BalanceUnit::divisible(balance!(1.5))
        );

        assert_eq!(
            (BalanceUnit::indivisible(9) / BalanceUnit::divisible(balance!(2))).unwrap(),
            BalanceUnit::divisible(balance!(4.5))
        );

        assert_eq!(
            (BalanceUnit::divisible(balance!(3.3)) / BalanceUnit::indivisible(3)).unwrap(),
            BalanceUnit::divisible(balance!(1.1))
        );

        assert_eq!(
            (BalanceUnit::indivisible(6) / BalanceUnit::indivisible(3)).unwrap(),
            BalanceUnit::indivisible(2)
        );

        // check div by 0
        assert_err!(
            (BalanceUnit::divisible(balance!(1.1)) / BalanceUnit::divisible(0)),
            ArithmeticError::DivisionByZero
        );

        assert_err!(
            (BalanceUnit::divisible(balance!(1.1)) / BalanceUnit::indivisible(0)),
            ArithmeticError::DivisionByZero
        );

        assert_err!(
            (BalanceUnit::indivisible(5) / BalanceUnit::divisible(0)),
            ArithmeticError::DivisionByZero
        );

        assert_err!(
            (BalanceUnit::indivisible(5) / BalanceUnit::indivisible(0)),
            ArithmeticError::DivisionByZero
        );

        // check rounding
        assert_eq!(
            (BalanceUnit::divisible(balance!(10)) / BalanceUnit::divisible(balance!(3))).unwrap(),
            BalanceUnit::divisible(balance!(3.333333333333333333))
        );

        assert_eq!(
            (BalanceUnit::indivisible(10) / BalanceUnit::divisible(balance!(3))).unwrap(),
            BalanceUnit::divisible(balance!(3.333333333333333333))
        );

        assert_eq!(
            (BalanceUnit::divisible(balance!(10)) / BalanceUnit::indivisible(3)).unwrap(),
            BalanceUnit::divisible(balance!(3.333333333333333333))
        );

        assert_eq!(
            (BalanceUnit::indivisible(10) / BalanceUnit::indivisible(3)).unwrap(),
            BalanceUnit::indivisible(3)
        );

        assert_eq!(
            (BalanceUnit::indivisible(14) / BalanceUnit::indivisible(5)).unwrap(),
            BalanceUnit::indivisible(2)
        );

        assert_eq!(
            (BalanceUnit::indivisible(3) / BalanceUnit::indivisible(4)).unwrap(),
            BalanceUnit::indivisible(0)
        );
    }

    #[test]
    fn check_pow() {
        assert_eq!(
            BalanceUnit::divisible(balance!(2)).pow(3).unwrap(),
            BalanceUnit::divisible(balance!(8))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(1.2)).pow(4).unwrap(),
            BalanceUnit::divisible(balance!(2.0736))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(4)).pow(0).unwrap(),
            BalanceUnit::divisible(balance!(1))
        );

        assert_eq!(
            BalanceUnit::indivisible(2).pow(3).unwrap(),
            BalanceUnit::indivisible(8)
        );

        assert_eq!(
            BalanceUnit::indivisible(4).pow(0).unwrap(),
            BalanceUnit::indivisible(1)
        );
    }

    #[test]
    fn check_sqrt() {
        assert_eq!(
            BalanceUnit::divisible(balance!(16)).sqrt().unwrap(),
            BalanceUnit::divisible(balance!(4))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(1.44)).sqrt().unwrap(),
            BalanceUnit::divisible(balance!(1.2))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(0.16)).sqrt().unwrap(),
            BalanceUnit::divisible(balance!(0.4))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(0)).sqrt().unwrap(),
            BalanceUnit::divisible(balance!(0))
        );

        assert_eq!(
            BalanceUnit::indivisible(16).sqrt().unwrap(),
            BalanceUnit::indivisible(4)
        );

        assert_eq!(
            BalanceUnit::indivisible(10).sqrt().unwrap(),
            BalanceUnit::indivisible(3)
        );

        assert_eq!(
            BalanceUnit::indivisible(0).sqrt().unwrap(),
            BalanceUnit::indivisible(0)
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn check_display() {
        assert_eq!(BalanceUnit::divisible(balance!(100)).to_string(), "100.0");
        assert_eq!(BalanceUnit::divisible(balance!(1.234)).to_string(), "1.234");
        assert_eq!(BalanceUnit::divisible(balance!(0)).to_string(), "0.0");

        assert_eq!(BalanceUnit::indivisible(100).to_string(), "100");
        assert_eq!(BalanceUnit::indivisible(123).to_string(), "123");
        assert_eq!(BalanceUnit::indivisible(0).to_string(), "0");
    }
}
