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
use crate::{Balance, Fixed, FixedPrecision};
use codec::{Decode, Encode, MaxEncodedLen};
use core::cmp::Ordering;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};
use fixnum::ops::RoundMode;
use fixnum::ArithmeticError;
use num_traits::Unsigned;
use sp_arithmetic::traits::IntegerSquareRoot;
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Saturating, Zero};

#[cfg(feature = "std")]
use {
    serde::{Deserialize, Serialize},
    sp_std::fmt::Display,
    static_assertions::_core::fmt::Formatter,
};

const RATIO: u128 = 1_000_000_000_000_000_000;

/// BalanceUnit wraps Balance and provides proper math operations between divisible & non-divisible balances that have different precision.
#[derive(
    Encode, Decode, Copy, Clone, Debug, PartialEq, Eq, scale_info::TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct BalanceUnit {
    inner: Balance,

    /// Defines how to interpret the `inner` value.
    /// true - means the precision of 18 digits
    /// false - means the precision of 0 digits
    is_divisible: bool,
}

impl Ord for BalanceUnit {
    fn cmp(&self, other: &Self) -> Ordering {
        let result = self.integer().cmp(&other.integer());
        if result.is_eq() {
            self.fractional().cmp(&other.fractional())
        } else {
            result
        }
    }
}

impl PartialOrd for BalanceUnit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl Default for BalanceUnit {
    fn default() -> Self {
        Self::zero()
    }
}

impl Zero for BalanceUnit {
    fn zero() -> Self {
        // `is_divisible` = false here because if this zero value will have some operations with divisible asset, `is_divisible` is changed to true
        Self::new(0, false)
    }

    fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }
}

#[cfg(feature = "std")]
impl Display for BalanceUnit {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        let s = if self.is_divisible {
            FixedWrapper::from(self.inner)
                .get()
                .expect("Failed to convert into Fixed")
                .to_string()
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

    pub fn copy_divisibility(&self, balance: Balance) -> Self {
        Self::new(balance, self.is_divisible)
    }

    pub fn balance(&self) -> &Balance {
        &self.inner
    }

    pub fn set(&mut self, value: Balance) {
        self.inner = value
    }

    pub fn is_divisible(&self) -> bool {
        self.is_divisible
    }

    /// Returns integer part of balance
    fn integer(&self) -> Balance {
        if self.is_divisible {
            self.inner / RATIO
        } else {
            self.inner
        }
    }

    /// Returns fractional part of balance
    fn fractional(&self) -> Balance {
        if self.is_divisible {
            self.inner % RATIO
        } else {
            Balance::zero()
        }
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

    pub fn into_indivisible(mut self, mode: RoundMode) -> Self {
        if self.is_divisible {
            let div_coefficient: u128 =
                10u128.pow(<FixedPrecision as fixnum::typenum::Unsigned>::U32.into());
            self.inner = match mode {
                RoundMode::Ceil => self.inner.div_ceil(div_coefficient),
                RoundMode::Floor => self.inner.div_floor(div_coefficient),
            };
            self.is_divisible = false;
        }
        self
    }

    pub fn into_divisible(mut self) -> Option<Self> {
        if !self.is_divisible {
            let div_coefficient: u128 =
                10u128.pow(<FixedPrecision as fixnum::typenum::Unsigned>::U32.into());
            self.inner = self.inner.checked_mul(div_coefficient)?;
            self.is_divisible = true;
        }
        Some(self)
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
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let (left, right) = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => (self.inner * RATIO, rhs.inner),
            (true, false) => (self.inner, rhs.inner * RATIO),
            _ => (self.inner, rhs.inner),
        };
        let balance = left + right;
        Self::new(balance, self.is_divisible || rhs.is_divisible)
    }
}

impl AddAssign for BalanceUnit {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs
    }
}

impl CheckedAdd for BalanceUnit {
    fn checked_add(&self, rhs: &Self) -> Option<Self> {
        let (left, right) = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => (self.inner.checked_mul(RATIO)?, rhs.inner),
            (true, false) => (self.inner, rhs.inner.checked_mul(RATIO)?),
            _ => (self.inner, rhs.inner),
        };
        let balance = left.checked_add(right)?;
        Some(Self::new(balance, self.is_divisible || rhs.is_divisible))
    }
}

impl Sub for BalanceUnit {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        let (left, right) = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => (self.inner * RATIO, rhs.inner),
            (true, false) => (self.inner, rhs.inner * RATIO),
            _ => (self.inner, rhs.inner),
        };
        let balance = left - right;
        Self::new(balance, self.is_divisible || rhs.is_divisible)
    }
}

impl SubAssign for BalanceUnit {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs
    }
}

impl CheckedSub for BalanceUnit {
    fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        let (left, right) = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => (self.inner.checked_mul(RATIO)?, rhs.inner),
            (true, false) => (self.inner, rhs.inner.checked_mul(RATIO)?),
            _ => (self.inner, rhs.inner),
        };
        let balance = left.checked_sub(right)?;
        Some(Self::new(balance, self.is_divisible || rhs.is_divisible))
    }
}

impl Mul for BalanceUnit {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let balance = if self.is_divisible && rhs.is_divisible {
            (FixedWrapper::from(self.inner) * (FixedWrapper::from(rhs.inner)))
                .try_into_balance()
                .unwrap()
        } else {
            self.inner * rhs.inner
        };
        Self::new(balance, self.is_divisible || rhs.is_divisible)
    }
}

impl MulAssign for BalanceUnit {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs
    }
}

impl CheckedMul for BalanceUnit {
    fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        let balance = if self.is_divisible && rhs.is_divisible {
            (FixedWrapper::from(self.inner) * (FixedWrapper::from(rhs.inner)))
                .try_into_balance()
                .ok()?
        } else {
            self.inner.checked_mul(rhs.inner)?
        };
        Some(Self::new(balance, self.is_divisible || rhs.is_divisible))
    }
}

impl Div for BalanceUnit {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        let balance = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => (FixedWrapper::from(self.inner * RATIO)
                / FixedWrapper::from(rhs.inner))
            .try_into_balance()
            .unwrap(),
            (true, true) => (FixedWrapper::from(self.inner) / FixedWrapper::from(rhs.inner))
                .try_into_balance()
                .unwrap(),
            _ => self.inner / rhs.inner,
        };
        Self::new(balance, self.is_divisible || rhs.is_divisible)
    }
}

impl DivAssign for BalanceUnit {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs
    }
}

impl CheckedDiv for BalanceUnit {
    fn checked_div(&self, rhs: &Self) -> Option<Self> {
        let balance = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => {
                let left = self.inner.checked_mul(RATIO)?;
                (FixedWrapper::from(left) / FixedWrapper::from(rhs.inner))
                    .try_into_balance()
                    .ok()?
            }
            (true, true) => (FixedWrapper::from(self.inner) / FixedWrapper::from(rhs.inner))
                .try_into_balance()
                .ok()?,
            _ => self.inner.checked_div(rhs.inner)?,
        };
        Some(Self::new(balance, self.is_divisible || rhs.is_divisible))
    }
}

impl Saturating for BalanceUnit {
    fn saturating_add(self, rhs: Self) -> Self {
        let (left, right) = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => (self.inner.saturating_mul(RATIO), rhs.inner),
            (true, false) => (self.inner, rhs.inner.saturating_mul(RATIO)),
            _ => (self.inner, rhs.inner),
        };
        let balance = left.saturating_add(right);
        Self::new(balance, self.is_divisible || rhs.is_divisible)
    }

    fn saturating_sub(self, rhs: Self) -> Self {
        let (left, right) = match (self.is_divisible, rhs.is_divisible) {
            (false, true) => (self.inner.saturating_mul(RATIO), rhs.inner),
            (true, false) => (self.inner, rhs.inner.saturating_mul(RATIO)),
            _ => (self.inner, rhs.inner),
        };
        let balance = left.saturating_sub(right);
        Self::new(balance, self.is_divisible || rhs.is_divisible)
    }

    fn saturating_mul(self, rhs: Self) -> Self {
        let balance = if self.is_divisible && rhs.is_divisible {
            (FixedWrapper::from(self.inner) * (FixedWrapper::from(rhs.inner)))
                .try_into_balance()
                .unwrap_or(Balance::MAX)
        } else {
            self.inner.saturating_mul(rhs.inner)
        };
        Self::new(balance, self.is_divisible || rhs.is_divisible)
    }

    fn saturating_pow(self, exp: usize) -> Self {
        let balance = if self.is_divisible {
            FixedWrapper::from(self.inner)
                .pow(exp as u32)
                .try_into_balance()
                .unwrap_or(Balance::MAX)
        } else {
            self.inner.saturating_pow(exp as u32)
        };
        Self::new(balance, self.is_divisible)
    }
}

/// `BalanceUnit` can be multiplied by scalars using this type.
#[derive(Copy, Clone)]
pub struct Scalar<N>(pub N);

impl<N: Unsigned + Into<u128>> Mul<Scalar<N>> for BalanceUnit {
    type Output = Self;

    fn mul(mut self, rhs: Scalar<N>) -> Self::Output {
        self.inner *= rhs.0.into();
        self
    }
}

impl<N: Unsigned + Into<u128>> MulAssign<Scalar<N>> for BalanceUnit {
    fn mul_assign(&mut self, rhs: Scalar<N>) {
        *self = *self * rhs
    }
}

// `num_traits::CheckedMul` trait doesn't allow `Rhs` other than `Self`
impl BalanceUnit {
    pub fn checked_mul_by_scalar<N: Unsigned + Into<u128> + Copy>(
        &self,
        rhs: Scalar<N>,
    ) -> Option<Self> {
        Some(Self::new(
            self.inner.checked_mul(rhs.0.into())?,
            self.is_divisible,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::balance;
    use crate::balance_unit::*;

    #[test]
    fn check_constructors() {
        assert_eq!(
            BalanceUnit::default(),
            BalanceUnit {
                inner: 0,
                is_divisible: false
            }
        );

        assert_eq!(
            BalanceUnit::zero(),
            BalanceUnit {
                inner: 0,
                is_divisible: false
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

        assert_eq!(
            BalanceUnit::divisible(1).copy_divisibility(13),
            BalanceUnit {
                inner: 13,
                is_divisible: true
            }
        );

        assert_eq!(
            BalanceUnit::indivisible(1).copy_divisibility(14),
            BalanceUnit {
                inner: 14,
                is_divisible: false
            }
        );
    }

    #[test]
    fn check_parts() {
        assert_eq!(BalanceUnit::divisible(balance!(0.12)).integer(), 0);
        assert_eq!(
            BalanceUnit::divisible(balance!(0.12)).fractional(),
            balance!(0.12)
        );

        assert_eq!(BalanceUnit::divisible(balance!(12.34)).integer(), 12);
        assert_eq!(
            BalanceUnit::divisible(balance!(12.34)).fractional(),
            balance!(0.34)
        );

        assert_eq!(BalanceUnit::divisible(balance!(56)).integer(), 56);
        assert_eq!(BalanceUnit::divisible(balance!(56)).fractional(), 0);

        assert_eq!(BalanceUnit::indivisible(78).integer(), 78);
        assert_eq!(BalanceUnit::indivisible(78).fractional(), 0);
    }

    #[test]
    fn check_add() {
        assert_eq!(
            BalanceUnit::divisible(balance!(1.1)) + BalanceUnit::divisible(balance!(1.2)),
            BalanceUnit::divisible(balance!(2.3))
        );

        assert_eq!(
            BalanceUnit::indivisible(2) + BalanceUnit::divisible(balance!(1.1)),
            BalanceUnit::divisible(balance!(3.1))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(1.1)) + BalanceUnit::indivisible(3),
            BalanceUnit::divisible(balance!(4.1))
        );

        assert_eq!(
            BalanceUnit::indivisible(2) + BalanceUnit::indivisible(3),
            BalanceUnit::indivisible(5)
        );
    }

    #[test]
    fn check_add_assign() {
        let mut value = BalanceUnit::divisible(balance!(1.1));
        value += BalanceUnit::divisible(balance!(1.2));
        assert_eq!(value, BalanceUnit::divisible(balance!(2.3)));

        let mut value = BalanceUnit::indivisible(2);
        value += BalanceUnit::divisible(balance!(1.1));
        assert_eq!(value, BalanceUnit::divisible(balance!(3.1)));

        let mut value = BalanceUnit::divisible(balance!(1.1));
        value += BalanceUnit::indivisible(3);
        assert_eq!(value, BalanceUnit::divisible(balance!(4.1)));

        let mut value = BalanceUnit::indivisible(2);
        value += BalanceUnit::indivisible(3);
        assert_eq!(value, BalanceUnit::indivisible(5));
    }

    #[test]
    fn check_checked_add() {
        assert_eq!(
            BalanceUnit::divisible(balance!(1.1))
                .checked_add(&BalanceUnit::divisible(balance!(1.2))),
            Some(BalanceUnit::divisible(balance!(2.3)))
        );

        assert_eq!(
            BalanceUnit::indivisible(2).checked_add(&BalanceUnit::divisible(balance!(1.1))),
            Some(BalanceUnit::divisible(balance!(3.1)))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(1.1)).checked_add(&BalanceUnit::indivisible(3)),
            Some(BalanceUnit::divisible(balance!(4.1)))
        );

        assert_eq!(
            BalanceUnit::indivisible(2).checked_add(&BalanceUnit::indivisible(3)),
            Some(BalanceUnit::indivisible(5))
        );

        // overflow
        assert_eq!(
            BalanceUnit::divisible(Balance::MAX)
                .checked_add(&BalanceUnit::divisible(balance!(1.2))),
            None
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX)
                .checked_add(&BalanceUnit::divisible(balance!(1.1))),
            None
        );

        assert_eq!(
            BalanceUnit::divisible(Balance::MAX).checked_add(&BalanceUnit::indivisible(3)),
            None
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX).checked_add(&BalanceUnit::indivisible(3)),
            None
        );
    }

    #[test]
    fn check_saturating_add() {
        assert_eq!(
            BalanceUnit::divisible(balance!(1.1))
                .saturating_add(BalanceUnit::divisible(balance!(1.2))),
            BalanceUnit::divisible(balance!(2.3))
        );

        assert_eq!(
            BalanceUnit::indivisible(2).saturating_add(BalanceUnit::divisible(balance!(1.1))),
            BalanceUnit::divisible(balance!(3.1))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(1.1)).saturating_add(BalanceUnit::indivisible(3)),
            BalanceUnit::divisible(balance!(4.1))
        );

        assert_eq!(
            BalanceUnit::indivisible(2).saturating_add(BalanceUnit::indivisible(3)),
            BalanceUnit::indivisible(5)
        );

        // overflow
        assert_eq!(
            BalanceUnit::divisible(Balance::MAX)
                .saturating_add(BalanceUnit::divisible(balance!(1.2))),
            BalanceUnit::divisible(Balance::MAX)
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX)
                .saturating_add(BalanceUnit::divisible(balance!(1.1))),
            BalanceUnit::divisible(Balance::MAX)
        );

        assert_eq!(
            BalanceUnit::divisible(Balance::MAX).saturating_add(BalanceUnit::indivisible(3)),
            BalanceUnit::divisible(Balance::MAX)
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX).saturating_add(BalanceUnit::indivisible(3)),
            BalanceUnit::indivisible(Balance::MAX)
        );
    }

    #[test]
    fn check_sub() {
        assert_eq!(
            BalanceUnit::divisible(balance!(1.2)) - BalanceUnit::divisible(balance!(1.1)),
            BalanceUnit::divisible(balance!(0.1))
        );

        assert_eq!(
            BalanceUnit::indivisible(3) - BalanceUnit::divisible(balance!(1.1)),
            BalanceUnit::divisible(balance!(1.9))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(4.1)) - BalanceUnit::indivisible(3),
            BalanceUnit::divisible(balance!(1.1))
        );

        assert_eq!(
            BalanceUnit::indivisible(5) - BalanceUnit::indivisible(3),
            BalanceUnit::indivisible(2)
        );
    }

    #[test]
    fn check_sub_assign() {
        let mut value = BalanceUnit::divisible(balance!(1.2));
        value -= BalanceUnit::divisible(balance!(1.1));
        assert_eq!(value, BalanceUnit::divisible(balance!(0.1)));

        let mut value = BalanceUnit::indivisible(3);
        value -= BalanceUnit::divisible(balance!(1.1));
        assert_eq!(value, BalanceUnit::divisible(balance!(1.9)));

        let mut value = BalanceUnit::divisible(balance!(4.1));
        value -= BalanceUnit::indivisible(3);
        assert_eq!(value, BalanceUnit::divisible(balance!(1.1)));

        let mut value = BalanceUnit::indivisible(5);
        value -= BalanceUnit::indivisible(3);
        assert_eq!(value, BalanceUnit::indivisible(2));
    }

    #[test]
    fn check_checked_sub() {
        assert_eq!(
            BalanceUnit::divisible(balance!(1.2))
                .checked_sub(&BalanceUnit::divisible(balance!(1.1))),
            Some(BalanceUnit::divisible(balance!(0.1)))
        );

        assert_eq!(
            BalanceUnit::indivisible(3).checked_sub(&BalanceUnit::divisible(balance!(1.1))),
            Some(BalanceUnit::divisible(balance!(1.9)))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(4.1)).checked_sub(&BalanceUnit::indivisible(3)),
            Some(BalanceUnit::divisible(balance!(1.1)))
        );

        assert_eq!(
            BalanceUnit::indivisible(5).checked_sub(&BalanceUnit::indivisible(3)),
            Some(BalanceUnit::indivisible(2))
        );

        // overflow
        assert_eq!(
            BalanceUnit::divisible(balance!(1.2))
                .checked_sub(&BalanceUnit::divisible(balance!(1.3))),
            None
        );

        assert_eq!(
            BalanceUnit::indivisible(3).checked_sub(&BalanceUnit::divisible(balance!(4.1))),
            None
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(4.1)).checked_sub(&BalanceUnit::indivisible(5)),
            None
        );

        assert_eq!(
            BalanceUnit::indivisible(5).checked_sub(&BalanceUnit::indivisible(7)),
            None
        );
    }

    #[test]
    fn check_saturating_sub() {
        assert_eq!(
            BalanceUnit::divisible(balance!(1.2))
                .saturating_sub(BalanceUnit::divisible(balance!(1.1))),
            BalanceUnit::divisible(balance!(0.1))
        );

        assert_eq!(
            BalanceUnit::indivisible(3).saturating_sub(BalanceUnit::divisible(balance!(1.1))),
            BalanceUnit::divisible(balance!(1.9))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(4.1)).saturating_sub(BalanceUnit::indivisible(3)),
            BalanceUnit::divisible(balance!(1.1))
        );

        assert_eq!(
            BalanceUnit::indivisible(5).saturating_sub(BalanceUnit::indivisible(3)),
            BalanceUnit::indivisible(2)
        );

        // overflow
        assert_eq!(
            BalanceUnit::divisible(balance!(1.2))
                .saturating_sub(BalanceUnit::divisible(balance!(1.3))),
            BalanceUnit::divisible(0)
        );

        assert_eq!(
            BalanceUnit::indivisible(3).saturating_sub(BalanceUnit::divisible(balance!(4.1))),
            BalanceUnit::divisible(0)
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(4.1)).saturating_sub(BalanceUnit::indivisible(5)),
            BalanceUnit::divisible(0)
        );

        assert_eq!(
            BalanceUnit::indivisible(5).saturating_sub(BalanceUnit::indivisible(7)),
            BalanceUnit::indivisible(0)
        );
    }

    #[test]
    fn check_mul() {
        assert_eq!(
            BalanceUnit::divisible(balance!(1.1)) * BalanceUnit::divisible(balance!(1.2)),
            BalanceUnit::divisible(balance!(1.32))
        );

        assert_eq!(
            BalanceUnit::indivisible(2) * BalanceUnit::divisible(balance!(1.1)),
            BalanceUnit::divisible(balance!(2.2))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(1.1)) * BalanceUnit::indivisible(3),
            BalanceUnit::divisible(balance!(3.3))
        );

        assert_eq!(
            BalanceUnit::indivisible(2) * BalanceUnit::indivisible(3),
            BalanceUnit::indivisible(6)
        );
    }

    #[test]
    fn check_mul_assign() {
        let mut value = BalanceUnit::divisible(balance!(1.1));
        value *= BalanceUnit::divisible(balance!(1.2));
        assert_eq!(value, BalanceUnit::divisible(balance!(1.32)));

        let mut value = BalanceUnit::indivisible(2);
        value *= BalanceUnit::divisible(balance!(1.1));
        assert_eq!(value, BalanceUnit::divisible(balance!(2.2)));

        let mut value = BalanceUnit::divisible(balance!(1.1));
        value *= BalanceUnit::indivisible(3);
        assert_eq!(value, BalanceUnit::divisible(balance!(3.3)));

        let mut value = BalanceUnit::indivisible(2);
        value *= BalanceUnit::indivisible(3);
        assert_eq!(value, BalanceUnit::indivisible(6));
    }

    #[test]
    fn check_checked_mul() {
        assert_eq!(
            BalanceUnit::divisible(balance!(1.1))
                .checked_mul(&BalanceUnit::divisible(balance!(1.2))),
            Some(BalanceUnit::divisible(balance!(1.32)))
        );

        assert_eq!(
            BalanceUnit::indivisible(2).checked_mul(&BalanceUnit::divisible(balance!(1.1))),
            Some(BalanceUnit::divisible(balance!(2.2)))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(1.1)).checked_mul(&BalanceUnit::indivisible(3)),
            Some(BalanceUnit::divisible(balance!(3.3)))
        );

        assert_eq!(
            BalanceUnit::indivisible(2).checked_mul(&BalanceUnit::indivisible(3)),
            Some(BalanceUnit::indivisible(6))
        );

        // overflow
        assert_eq!(
            BalanceUnit::divisible(Balance::MAX - 1)
                .checked_mul(&BalanceUnit::divisible(balance!(1.2))),
            None
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX - 1)
                .checked_mul(&BalanceUnit::divisible(balance!(1.1))),
            None
        );

        assert_eq!(
            BalanceUnit::divisible(Balance::MAX - 1).checked_mul(&BalanceUnit::indivisible(3)),
            None
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX - 1).checked_mul(&BalanceUnit::indivisible(3)),
            None
        );
    }

    #[test]
    fn check_saturating_mul() {
        assert_eq!(
            BalanceUnit::divisible(balance!(1.1))
                .saturating_mul(BalanceUnit::divisible(balance!(1.2))),
            BalanceUnit::divisible(balance!(1.32))
        );

        assert_eq!(
            BalanceUnit::indivisible(2).saturating_mul(BalanceUnit::divisible(balance!(1.1))),
            BalanceUnit::divisible(balance!(2.2))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(1.1)).saturating_mul(BalanceUnit::indivisible(3)),
            BalanceUnit::divisible(balance!(3.3))
        );

        assert_eq!(
            BalanceUnit::indivisible(2).saturating_mul(BalanceUnit::indivisible(3)),
            BalanceUnit::indivisible(6)
        );

        // overflow
        assert_eq!(
            BalanceUnit::divisible(Balance::MAX - 1)
                .saturating_mul(BalanceUnit::divisible(balance!(1.2))),
            BalanceUnit::divisible(Balance::MAX)
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX - 1)
                .saturating_mul(BalanceUnit::divisible(balance!(1.1))),
            BalanceUnit::divisible(Balance::MAX)
        );

        assert_eq!(
            BalanceUnit::divisible(Balance::MAX - 1).saturating_mul(BalanceUnit::indivisible(3)),
            BalanceUnit::divisible(Balance::MAX)
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX - 1).saturating_mul(BalanceUnit::indivisible(3)),
            BalanceUnit::indivisible(Balance::MAX)
        );
    }

    #[test]
    fn check_div() {
        // check regular cases
        assert_eq!(
            BalanceUnit::divisible(balance!(5.55)) / BalanceUnit::divisible(balance!(3.7)),
            BalanceUnit::divisible(balance!(1.5))
        );

        assert_eq!(
            BalanceUnit::indivisible(9) / BalanceUnit::divisible(balance!(2)),
            BalanceUnit::divisible(balance!(4.5))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(3.3)) / BalanceUnit::indivisible(3),
            BalanceUnit::divisible(balance!(1.1))
        );

        assert_eq!(
            BalanceUnit::indivisible(6) / BalanceUnit::indivisible(3),
            BalanceUnit::indivisible(2)
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX) / BalanceUnit::indivisible(Balance::MAX),
            BalanceUnit::indivisible(1)
        );
        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX) / BalanceUnit::indivisible(1),
            BalanceUnit::indivisible(Balance::MAX)
        );

        assert_eq!(
            BalanceUnit::divisible(Balance::MAX) / BalanceUnit::indivisible(1),
            BalanceUnit::divisible(Balance::MAX)
        );

        // check rounding
        assert_eq!(
            BalanceUnit::divisible(balance!(10)) / BalanceUnit::divisible(balance!(3)),
            BalanceUnit::divisible(balance!(3.333333333333333333))
        );

        assert_eq!(
            BalanceUnit::indivisible(10) / BalanceUnit::divisible(balance!(3)),
            BalanceUnit::divisible(balance!(3.333333333333333333))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(10)) / BalanceUnit::indivisible(3),
            BalanceUnit::divisible(balance!(3.333333333333333333))
        );

        assert_eq!(
            BalanceUnit::indivisible(10) / BalanceUnit::indivisible(3),
            BalanceUnit::indivisible(3)
        );

        assert_eq!(
            BalanceUnit::indivisible(14) / BalanceUnit::indivisible(5),
            BalanceUnit::indivisible(2)
        );

        assert_eq!(
            BalanceUnit::indivisible(3) / BalanceUnit::indivisible(4),
            BalanceUnit::indivisible(0)
        );
    }

    #[test]
    fn check_div_assign() {
        // check regular cases
        let mut value = BalanceUnit::divisible(balance!(5.55));
        value /= BalanceUnit::divisible(balance!(3.7));
        assert_eq!(value, BalanceUnit::divisible(balance!(1.5)));

        let mut value = BalanceUnit::indivisible(9);
        value /= BalanceUnit::divisible(balance!(2));
        assert_eq!(value, BalanceUnit::divisible(balance!(4.5)));

        let mut value = BalanceUnit::divisible(balance!(3.3));
        value /= BalanceUnit::indivisible(3);
        assert_eq!(value, BalanceUnit::divisible(balance!(1.1)));

        let mut value = BalanceUnit::indivisible(6);
        value /= BalanceUnit::indivisible(3);
        assert_eq!(value, BalanceUnit::indivisible(2));

        // check rounding
        let mut value = BalanceUnit::divisible(balance!(10));
        value /= BalanceUnit::divisible(balance!(3));
        assert_eq!(
            value,
            BalanceUnit::divisible(balance!(3.333333333333333333))
        );

        let mut value = BalanceUnit::indivisible(10);
        value /= BalanceUnit::divisible(balance!(3));
        assert_eq!(
            value,
            BalanceUnit::divisible(balance!(3.333333333333333333))
        );

        let mut value = BalanceUnit::divisible(balance!(10));
        value /= BalanceUnit::indivisible(3);
        assert_eq!(
            value,
            BalanceUnit::divisible(balance!(3.333333333333333333))
        );

        let mut value = BalanceUnit::indivisible(10);
        value /= BalanceUnit::indivisible(3);
        assert_eq!(value, BalanceUnit::indivisible(3));

        let mut value = BalanceUnit::indivisible(14);
        value /= BalanceUnit::indivisible(5);
        assert_eq!(value, BalanceUnit::indivisible(2));

        let mut value = BalanceUnit::indivisible(3);
        value /= BalanceUnit::indivisible(4);
        assert_eq!(value, BalanceUnit::indivisible(0));
    }

    #[test]
    fn check_checked_div() {
        // check regular cases
        assert_eq!(
            BalanceUnit::divisible(balance!(5.55))
                .checked_div(&BalanceUnit::divisible(balance!(3.7))),
            Some(BalanceUnit::divisible(balance!(1.5)))
        );

        assert_eq!(
            BalanceUnit::indivisible(9).checked_div(&BalanceUnit::divisible(balance!(2))),
            Some(BalanceUnit::divisible(balance!(4.5)))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(3.3)).checked_div(&BalanceUnit::indivisible(3)),
            Some(BalanceUnit::divisible(balance!(1.1)))
        );

        assert_eq!(
            BalanceUnit::indivisible(6).checked_div(&BalanceUnit::indivisible(3)),
            Some(BalanceUnit::indivisible(2))
        );

        // check rounding
        assert_eq!(
            BalanceUnit::divisible(balance!(10)).checked_div(&BalanceUnit::divisible(balance!(3))),
            Some(BalanceUnit::divisible(balance!(3.333333333333333333)))
        );

        assert_eq!(
            BalanceUnit::indivisible(10).checked_div(&BalanceUnit::divisible(balance!(3))),
            Some(BalanceUnit::divisible(balance!(3.333333333333333333)))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(10)).checked_div(&BalanceUnit::indivisible(3)),
            Some(BalanceUnit::divisible(balance!(3.333333333333333333)))
        );

        assert_eq!(
            BalanceUnit::indivisible(10).checked_div(&BalanceUnit::indivisible(3)),
            Some(BalanceUnit::indivisible(3))
        );

        assert_eq!(
            BalanceUnit::indivisible(14).checked_div(&BalanceUnit::indivisible(5)),
            Some(BalanceUnit::indivisible(2))
        );

        assert_eq!(
            BalanceUnit::indivisible(3).checked_div(&BalanceUnit::indivisible(4)),
            Some(BalanceUnit::indivisible(0))
        );

        // div by zero
        assert_eq!(
            BalanceUnit::divisible(balance!(5.55)).checked_div(&BalanceUnit::divisible(0)),
            None
        );

        assert_eq!(
            BalanceUnit::indivisible(9).checked_div(&BalanceUnit::divisible(0)),
            None
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(3.3)).checked_div(&BalanceUnit::indivisible(0)),
            None
        );

        assert_eq!(
            BalanceUnit::indivisible(6).checked_div(&BalanceUnit::indivisible(0)),
            None
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
    fn check_saturating_pow() {
        assert_eq!(
            BalanceUnit::divisible(balance!(2)).saturating_pow(3),
            BalanceUnit::divisible(balance!(8))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(1.2)).saturating_pow(4),
            BalanceUnit::divisible(balance!(2.0736))
        );

        assert_eq!(
            BalanceUnit::divisible(balance!(4)).saturating_pow(0),
            BalanceUnit::divisible(balance!(1))
        );

        assert_eq!(
            BalanceUnit::indivisible(2).saturating_pow(3),
            BalanceUnit::indivisible(8)
        );

        assert_eq!(
            BalanceUnit::indivisible(4).saturating_pow(0),
            BalanceUnit::indivisible(1)
        );

        // overflow
        assert_eq!(
            BalanceUnit::divisible(Balance::MAX - 1).saturating_pow(3),
            BalanceUnit::divisible(Balance::MAX)
        );

        assert_eq!(
            BalanceUnit::divisible(Balance::MAX - 1).saturating_pow(4),
            BalanceUnit::divisible(Balance::MAX)
        );

        assert_eq!(
            BalanceUnit::divisible(Balance::MAX - 1).saturating_pow(0),
            BalanceUnit::divisible(balance!(1))
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX - 1).saturating_pow(3),
            BalanceUnit::indivisible(Balance::MAX)
        );

        assert_eq!(
            BalanceUnit::indivisible(Balance::MAX - 1).saturating_pow(0),
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

    #[test]
    fn check_into_divisible() {
        let coefficient = 10u128.pow(<FixedPrecision as fixnum::typenum::Unsigned>::U32.into());

        for n in [0, 1, 100, u128::MAX / coefficient] {
            assert_eq!(
                BalanceUnit::divisible(n).into_divisible(),
                Some(BalanceUnit::divisible(n))
            );
            assert_eq!(
                BalanceUnit::indivisible(n).into_divisible(),
                Some(BalanceUnit::divisible(n * coefficient))
            );
        }

        // overflow
        for n in [u128::MAX / coefficient + 1, u128::MAX] {
            assert_eq!(
                BalanceUnit::divisible(n).into_divisible(),
                Some(BalanceUnit::divisible(n))
            );
            assert_eq!(BalanceUnit::indivisible(n).into_divisible(), None);
        }
    }

    #[test]
    fn check_into_indivisible() {
        let coefficient = 10u128.pow(<FixedPrecision as fixnum::typenum::Unsigned>::U32.into());

        for n in [
            0,
            1,
            100,
            u128::MAX / coefficient,
            u128::MAX / coefficient + 1,
            u128::MAX,
        ] {
            assert_eq!(
                BalanceUnit::divisible(n).into_indivisible(RoundMode::Ceil),
                BalanceUnit::indivisible(n.div_ceil(coefficient))
            );
            assert_eq!(
                BalanceUnit::divisible(n).into_indivisible(RoundMode::Floor),
                BalanceUnit::indivisible(n.div_floor(coefficient))
            );
        }
    }
}
