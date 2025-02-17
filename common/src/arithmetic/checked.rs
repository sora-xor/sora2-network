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

use crate::arithmetic::identities::{One, Zero};
use core::ops::{Add, Div, Mul, Rem, Shl, Shr, Sub};
use fixnum::ArithmeticError;
use sp_core::U256;

/// Performs addition that returns `None` instead of wrapping around on
/// overflow.
pub trait CheckedAdd: Sized + Add<Self, Output = Self> {
    type Error;

    /// Adds two numbers, checking for overflow. If overflow happens, `None` is
    /// returned.
    fn checked_add(&self, v: &Self) -> Option<Self>;

    /// Adds two numbers, checking for overflow. If overflow happens, `Error` is
    /// returned.
    fn cadd(&self, rhs: &Self) -> Result<Self, Self::Error>;
}

macro_rules! checked_impl {
    ($trait_name:ident, $method:ident, $cmethod:ident, $t:ty) => {
        impl $trait_name for $t {
            type Error = ArithmeticError;
            #[inline]
            fn $method(&self, v: &$t) -> Option<$t> {
                <$t>::$method(*self, *v)
            }

            #[inline]
            fn $cmethod(&self, v: &$t) -> Result<$t, Self::Error> {
                <$t>::$method(*self, *v).ok_or(ArithmeticError::Overflow)
            }
        }
    };
}

checked_impl!(CheckedAdd, checked_add, cadd, u8);
checked_impl!(CheckedAdd, checked_add, cadd, u16);
checked_impl!(CheckedAdd, checked_add, cadd, u32);
checked_impl!(CheckedAdd, checked_add, cadd, u64);
checked_impl!(CheckedAdd, checked_add, cadd, usize);
checked_impl!(CheckedAdd, checked_add, cadd, u128);
checked_impl!(CheckedAdd, checked_add, cadd, U256);

checked_impl!(CheckedAdd, checked_add, cadd, i8);
checked_impl!(CheckedAdd, checked_add, cadd, i16);
checked_impl!(CheckedAdd, checked_add, cadd, i32);
checked_impl!(CheckedAdd, checked_add, cadd, i64);
checked_impl!(CheckedAdd, checked_add, cadd, isize);
checked_impl!(CheckedAdd, checked_add, cadd, i128);

/// Performs subtraction that returns `None` instead of wrapping around on underflow.
pub trait CheckedSub: Sized + Sub<Self, Output = Self> {
    type Error;

    /// Subtracts two numbers, checking for underflow. If underflow happens,
    /// `None` is returned.
    fn checked_sub(&self, v: &Self) -> Option<Self>;

    /// Subtracts two numbers, checking for underflow. If underflow happens,
    /// `Error` is returned.
    fn csub(&self, v: &Self) -> Result<Self, Self::Error>;
}

checked_impl!(CheckedSub, checked_sub, csub, u8);
checked_impl!(CheckedSub, checked_sub, csub, u16);
checked_impl!(CheckedSub, checked_sub, csub, u32);
checked_impl!(CheckedSub, checked_sub, csub, u64);
checked_impl!(CheckedSub, checked_sub, csub, usize);
checked_impl!(CheckedSub, checked_sub, csub, u128);
checked_impl!(CheckedSub, checked_sub, csub, U256);

checked_impl!(CheckedSub, checked_sub, csub, i8);
checked_impl!(CheckedSub, checked_sub, csub, i16);
checked_impl!(CheckedSub, checked_sub, csub, i32);
checked_impl!(CheckedSub, checked_sub, csub, i64);
checked_impl!(CheckedSub, checked_sub, csub, isize);
checked_impl!(CheckedSub, checked_sub, csub, i128);

/// Performs multiplication that returns `None` instead of wrapping around on underflow or
/// overflow.
pub trait CheckedMul: Sized + Mul<Self, Output = Self> {
    type Error;

    /// Multiplies two numbers, checking for underflow or overflow. If underflow
    /// or overflow happens, `None` is returned.
    fn checked_mul(&self, v: &Self) -> Option<Self>;

    /// Multiplies two numbers, checking for underflow or overflow. If underflow
    /// or overflow happens, `Error` is returned.
    fn cmul(&self, v: &Self) -> Result<Self, Self::Error>;
}

checked_impl!(CheckedMul, checked_mul, cmul, u8);
checked_impl!(CheckedMul, checked_mul, cmul, u16);
checked_impl!(CheckedMul, checked_mul, cmul, u32);
checked_impl!(CheckedMul, checked_mul, cmul, u64);
checked_impl!(CheckedMul, checked_mul, cmul, usize);
checked_impl!(CheckedMul, checked_mul, cmul, u128);
checked_impl!(CheckedMul, checked_mul, cmul, U256);

checked_impl!(CheckedMul, checked_mul, cmul, i8);
checked_impl!(CheckedMul, checked_mul, cmul, i16);
checked_impl!(CheckedMul, checked_mul, cmul, i32);
checked_impl!(CheckedMul, checked_mul, cmul, i64);
checked_impl!(CheckedMul, checked_mul, cmul, isize);
checked_impl!(CheckedMul, checked_mul, cmul, i128);

macro_rules! checked_impl_zero_control {
    ($trait_name:ident, $method:ident, $cmethod:ident, $t:ty) => {
        impl $trait_name for $t {
            type Error = ArithmeticError;
            #[inline]
            fn $method(&self, v: &$t) -> Option<$t> {
                <$t>::$method(*self, *v)
            }

            #[inline]
            fn $cmethod(&self, v: &$t) -> Result<$t, Self::Error> {
                if v.is_zero() {
                    Err(ArithmeticError::DivisionByZero)
                } else {
                    <$t>::$method(*self, *v).ok_or(ArithmeticError::Overflow)
                }
            }
        }
    };
}

/// Performs division that returns `None` instead of panicking on division by zero and instead of
/// wrapping around on underflow and overflow.
pub trait CheckedDiv: Sized + Div<Self, Output = Self> {
    type Error;

    /// Divides two numbers, checking for underflow, overflow and division by
    /// zero. If any of that happens, `None` is returned.
    fn checked_div(&self, v: &Self) -> Option<Self>;

    /// Divides two numbers, checking for underflow, overflow and division by
    /// zero. If any of that happens, `Error` is returned.
    fn cdiv(&self, v: &Self) -> Result<Self, Self::Error>;
}

checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, u8);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, u16);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, u32);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, u64);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, usize);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, u128);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, U256);

checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, i8);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, i16);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, i32);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, i64);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, isize);
checked_impl_zero_control!(CheckedDiv, checked_div, cdiv, i128);

/// Performs an integral remainder that returns `None` instead of panicking on division by zero and
/// instead of wrapping around on underflow and overflow.
pub trait CheckedRem: Sized + Rem<Self, Output = Self> {
    type Error;

    /// Finds the remainder of dividing two numbers, checking for underflow, overflow and division
    /// by zero. If any of that happens, `None` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::CheckedRem;
    /// use std::i32::MIN;
    ///
    /// assert_eq!(CheckedRem::checked_rem(&10, &7), Some(3));
    /// assert_eq!(CheckedRem::checked_rem(&10, &-7), Some(3));
    /// assert_eq!(CheckedRem::checked_rem(&-10, &7), Some(-3));
    /// assert_eq!(CheckedRem::checked_rem(&-10, &-7), Some(-3));
    ///
    /// assert_eq!(CheckedRem::checked_rem(&10, &0), None);
    ///
    /// assert_eq!(CheckedRem::checked_rem(&MIN, &1), Some(0));
    /// assert_eq!(CheckedRem::checked_rem(&MIN, &-1), None);
    /// ```
    fn checked_rem(&self, v: &Self) -> Option<Self>;

    /// Finds the remainder of dividing two numbers, checking for underflow, overflow and division
    /// by zero. If any of that happens, `Error` is returned.
    fn crem(&self, v: &Self) -> Result<Self, Self::Error>;
}

checked_impl_zero_control!(CheckedRem, checked_rem, crem, u8);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, u16);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, u32);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, u64);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, usize);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, u128);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, U256);

checked_impl_zero_control!(CheckedRem, checked_rem, crem, i8);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, i16);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, i32);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, i64);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, isize);
checked_impl_zero_control!(CheckedRem, checked_rem, crem, i128);

macro_rules! checked_impl_unary {
    ($trait_name:ident, $method:ident, $cmethod:ident, $t:ty) => {
        impl $trait_name for $t {
            type Error = ArithmeticError;

            #[inline]
            fn $method(&self) -> Option<$t> {
                <$t>::$method(*self)
            }

            #[inline]
            fn $cmethod(&self) -> Result<$t, Self::Error> {
                <$t>::$method(*self).ok_or(ArithmeticError::Overflow)
            }
        }
    };
}

/// Performs negation that returns `None` if the result can't be represented.
pub trait CheckedNeg: Sized {
    type Error;
    /// Negates a number, returning `None` for results that can't be represented, like signed `MIN`
    /// values that can't be positive, or non-zero unsigned values that can't be negative.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_traits::CheckedNeg;
    /// use std::i32::MIN;
    ///
    /// assert_eq!(CheckedNeg::checked_neg(&1_i32), Some(-1));
    /// assert_eq!(CheckedNeg::checked_neg(&-1_i32), Some(1));
    /// assert_eq!(CheckedNeg::checked_neg(&MIN), None);
    ///
    /// assert_eq!(CheckedNeg::checked_neg(&0_u32), Some(0));
    /// assert_eq!(CheckedNeg::checked_neg(&1_u32), None);
    /// ```
    fn checked_neg(&self) -> Option<Self>;

    fn cneg(&self) -> Result<Self, Self::Error>;
}

checked_impl_unary!(CheckedNeg, checked_neg, cneg, u8);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, u16);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, u32);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, u64);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, usize);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, u128);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, U256);

checked_impl_unary!(CheckedNeg, checked_neg, cneg, i8);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, i16);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, i32);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, i64);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, isize);
checked_impl_unary!(CheckedNeg, checked_neg, cneg, i128);

/// Performs a left shift that returns `None` on shifts larger than
/// or equal to the type width.
pub trait CheckedShl: Sized + Shl<u32, Output = Self> {
    /// Checked shift left. Computes `self << rhs`, returning `None`
    /// if `rhs` is larger than or equal to the number of bits in `self`.
    ///
    /// ```
    /// use num_traits::CheckedShl;
    ///
    /// let x: u16 = 0x0001;
    ///
    /// assert_eq!(CheckedShl::checked_shl(&x, 0),  Some(0x0001));
    /// assert_eq!(CheckedShl::checked_shl(&x, 1),  Some(0x0002));
    /// assert_eq!(CheckedShl::checked_shl(&x, 15), Some(0x8000));
    /// assert_eq!(CheckedShl::checked_shl(&x, 16), None);
    /// ```
    fn checked_shl(&self, rhs: u32) -> Option<Self>;
}

macro_rules! checked_shift_impl {
    ($trait_name:ident, $method:ident, $t:ty) => {
        impl $trait_name for $t {
            #[inline]
            fn $method(&self, rhs: u32) -> Option<$t> {
                <$t>::$method(*self, rhs)
            }
        }
    };
}

checked_shift_impl!(CheckedShl, checked_shl, u8);
checked_shift_impl!(CheckedShl, checked_shl, u16);
checked_shift_impl!(CheckedShl, checked_shl, u32);
checked_shift_impl!(CheckedShl, checked_shl, u64);
checked_shift_impl!(CheckedShl, checked_shl, usize);
checked_shift_impl!(CheckedShl, checked_shl, u128);

checked_shift_impl!(CheckedShl, checked_shl, i8);
checked_shift_impl!(CheckedShl, checked_shl, i16);
checked_shift_impl!(CheckedShl, checked_shl, i32);
checked_shift_impl!(CheckedShl, checked_shl, i64);
checked_shift_impl!(CheckedShl, checked_shl, isize);
checked_shift_impl!(CheckedShl, checked_shl, i128);

/// Performs a right shift that returns `None` on shifts larger than
/// or equal to the type width.
pub trait CheckedShr: Sized + Shr<u32, Output = Self> {
    /// Checked shift right. Computes `self >> rhs`, returning `None`
    /// if `rhs` is larger than or equal to the number of bits in `self`.
    ///
    /// ```
    /// use num_traits::CheckedShr;
    ///
    /// let x: u16 = 0x8000;
    ///
    /// assert_eq!(CheckedShr::checked_shr(&x, 0),  Some(0x8000));
    /// assert_eq!(CheckedShr::checked_shr(&x, 1),  Some(0x4000));
    /// assert_eq!(CheckedShr::checked_shr(&x, 15), Some(0x0001));
    /// assert_eq!(CheckedShr::checked_shr(&x, 16), None);
    /// ```
    fn checked_shr(&self, rhs: u32) -> Option<Self>;
}

checked_shift_impl!(CheckedShr, checked_shr, u8);
checked_shift_impl!(CheckedShr, checked_shr, u16);
checked_shift_impl!(CheckedShr, checked_shr, u32);
checked_shift_impl!(CheckedShr, checked_shr, u64);
checked_shift_impl!(CheckedShr, checked_shr, usize);
checked_shift_impl!(CheckedShr, checked_shr, u128);

checked_shift_impl!(CheckedShr, checked_shr, i8);
checked_shift_impl!(CheckedShr, checked_shr, i16);
checked_shift_impl!(CheckedShr, checked_shr, i32);
checked_shift_impl!(CheckedShr, checked_shr, i64);
checked_shift_impl!(CheckedShr, checked_shr, isize);
checked_shift_impl!(CheckedShr, checked_shr, i128);

/// Raises a value to the power of exp, returning `None` if an overflow occurred.
///
/// Note that `0⁰` (`checked_pow(0, 0)`) returns `Some(1)`. Mathematically this is undefined.
///
/// Otherwise same as the `pow` function.
///
/// # Example
///
/// ```rust
/// use num_traits::checked_pow;
///
/// assert_eq!(checked_pow(2i8, 4), Some(16));
/// assert_eq!(checked_pow(7i8, 8), None);
/// assert_eq!(checked_pow(7u32, 8), Some(5_764_801));
/// assert_eq!(checked_pow(0u32, 0), Some(1)); // Be aware if this case affect you
/// ```
#[inline]
pub fn checked_pow<T: Clone + One + CheckedMul>(mut base: T, mut exp: usize) -> Option<T> {
    if exp == 0 {
        return Some(T::one());
    }

    while exp & 1 == 0 {
        base = base.checked_mul(&base)?;
        exp >>= 1;
    }
    if exp == 1 {
        return Some(base);
    }

    let mut acc = base.clone();
    while exp > 1 {
        exp >>= 1;
        base = base.checked_mul(&base)?;
        if exp & 1 == 1 {
            acc = acc.checked_mul(&base)?;
        }
    }
    Some(acc)
}
