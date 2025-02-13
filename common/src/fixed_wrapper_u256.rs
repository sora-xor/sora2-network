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

use core::convert::TryInto;
use core::ops::*;
use core::result::Result;

use fixnum::ArithmeticError;
use frame_support::RuntimeDebug;
use sp_core::U256;
use static_assertions::_core::cmp::Ordering;

use crate::arithmetic::checked::{CheckedAdd, CheckedDiv, CheckedMul, CheckedNeg, CheckedSub};
#[cfg(not(feature = "test"))]
use crate::arithmetic::saturating::SaturatedConversion;
use crate::fixed::FixedU256;
use crate::Balance;

/// A convenient wrapper around `FixedU256` type for safe math.
///
/// Supported operations: `+`, `+=`, `-`, `-=`, `/`, `/=`, `*`, `*=`, `sqrt`.
#[derive(Clone, RuntimeDebug)]
pub struct FixedWrapper256 {
    inner: Result<FixedU256, ArithmeticError>,
}

impl FixedWrapper256 {
    /// Retrieve the result.
    pub fn get(self) -> Result<FixedU256, ArithmeticError> {
        self.inner
    }

    /// Calculation of sqrt(a*b) = c, if a*b fails than sqrt(a) * sqrt(b) is used.
    pub fn multiply_and_sqrt(&self, lhs: &Self) -> Self {
        /*
        FIXME: Has been running for over 60 seconds.
        let mul_first = (self.clone() * lhs.clone()).sqrt_accurate();
        if mul_first.inner.is_ok() {
            return mul_first;
        }
        */
        // let mul_after = self.clone().sqrt_accurate() * lhs.clone().sqrt_accurate();
        // if mul_after.inner.is_ok() {
        //     return mul_after;
        // }
        // FixedWrapper256 {
        //     inner: Err(ArithmeticError::Overflow),
        // }
        self.clone().sqrt_accurate() * lhs.clone().sqrt_accurate()
    }

    pub fn pow(&self, x: u32) -> Self {
        (0..x).fold(FixedWrapper256::from(1), |acc, _| acc * self.clone())
    }

    /// Calculates square root of underlying FixedU256 number.
    pub fn sqrt_accurate(self) -> Self {
        self.inner.and_then(|num| num.sqrt()).into()
    }

    // pub fn abs(self) -> Self {
    //     self.inner.and_then(|num| num.abs()).into()
    // }

    /// Calculates square root of self using fractional representation.
    #[cfg(feature = "std")]
    pub fn sqrt(&self) -> Self {
        match self.to_fraction() {
            Err(_) => self.clone(),
            Ok(x) => Self::from(x.sqrt()),
        }
    }

    #[cfg(feature = "std")]
    pub fn to_fraction(&self) -> Result<f64, ArithmeticError> {
        self.inner.clone().map(FixedU256::to_float)
    }

    pub fn try_into_balance(self) -> Result<Balance, ArithmeticError> {
        match self.inner {
            Ok(fixed) => fixed.to_u128(),
            Err(e) => Err(e),
        }
    }

    pub fn into_balance(self) -> Balance {
        #[cfg(feature = "test")]
        {
            self.inner.unwrap().to_u128().unwrap()
        }
        #[cfg(not(feature = "test"))]
        {
            self.inner
                .map(|v| v.into_inner().saturated_into())
                .unwrap_or(0)
        }
    }
}

impl From<Result<FixedU256, ArithmeticError>> for FixedWrapper256 {
    fn from(result: Result<FixedU256, ArithmeticError>) -> Self {
        FixedWrapper256 { inner: result }
    }
}

impl From<FixedU256> for FixedWrapper256 {
    fn from(fixed: FixedU256) -> Self {
        FixedWrapper256::from(Ok(fixed))
    }
}

// impl From<f64> for FixedWrapper256 {
//     fn from(value: f64) -> Self {
//         Self::from(FixedU256::from_float(value))
//     }
// }

macro_rules! impl_from_for_fixed_wrapper {
    ($( $T:ty ),+) => {
        $( impl_from_for_fixed_wrapper!(@single $T); )*
    };
    (@single $T:ty) => {
        impl From<$T> for FixedWrapper256 {
            fn from(value: $T) -> Self {
                match value.try_into() {
                    Ok(fixed) => Self {
                        inner: Ok(fixed),
                    },
                    Err(err) => Self {
                        inner: Err(err),
                    },
                }
            }
        }
    };
}

impl_from_for_fixed_wrapper!(usize, isize, U256, u128, i128, u64, i64, u32, i32, f64);

fn zip<'a, 'b, T, E: Clone>(a: &'a Result<T, E>, b: &'b Result<T, E>) -> Result<(&'a T, &'b T), E> {
    a.as_ref()
        .and_then(|a| b.as_ref().map(|b| (a, b)))
        .map_err(|err| err.clone())
}

macro_rules! impl_op_for_fixed_wrapper {
    (
        $op:ty,
        $op_fn:ident,
        $checked_op_fn:ident
    ) => {
        impl $op for FixedWrapper256 {
            type Output = Self;

            fn $op_fn(self, rhs: Self) -> Self::Output {
                zip(&self.inner, &rhs.inner)
                    .and_then(|(lhs, &rhs)| lhs.$checked_op_fn(&rhs))
                    .into()
            }
        }
    };
}

impl_op_for_fixed_wrapper!(Add, add, cadd);
impl_op_for_fixed_wrapper!(Sub, sub, csub);

impl_op_for_fixed_wrapper!(Mul, mul, cmul);
impl_op_for_fixed_wrapper!(Div, div, cdiv);

macro_rules! impl_assign_op_for_fixed_wrapper {
    (
        $op:ty,
        $op_fn:ident,
        $checked_op_fn:ident
    ) => {
        impl $op for FixedWrapper256 {
            fn $op_fn(&mut self, rhs: Self) {
                *self = zip(&self.inner, &rhs.inner)
                    .and_then(|(lhs, &rhs)| lhs.$checked_op_fn(&rhs))
                    .into();
            }
        }
    };
}

impl_assign_op_for_fixed_wrapper!(AddAssign, add_assign, cadd);
impl_assign_op_for_fixed_wrapper!(SubAssign, sub_assign, csub);

impl_assign_op_for_fixed_wrapper!(MulAssign, mul_assign, cmul);
impl_assign_op_for_fixed_wrapper!(DivAssign, div_assign, cdiv);

// macro_rules! impl_lossless_op_for_fixed_wrapper {
//     (
//         $op_fn:ident,
//         $lossless_op_fn:ident
//     ) => {
//         impl FixedWrapper256 {
//             pub fn $op_fn(self, rhs: Self) -> Option<Self> {
//                 zip(&self.inner, &rhs.inner)
//                     .and_then(|(lhs, &rhs)| lhs.$lossless_op_fn(rhs))
//                     .transpose()
//                     .map(|result| result.into())
//             }
//         }
//     };
// }
//
// impl_lossless_op_for_fixed_wrapper!(lossless_mul, lossless_mul);
// impl_lossless_op_for_fixed_wrapper!(lossless_div, lossless_div);

impl PartialEq for FixedWrapper256 {
    fn eq(&self, other: &Self) -> bool {
        zip(&self.inner, &other.inner)
            .map(|(lhs, rhs)| lhs.eq(rhs))
            .unwrap_or(false)
    }
}

impl Neg for FixedWrapper256 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        self.inner.and_then(|value| value.cneg()).into()
    }
}

impl PartialOrd for FixedWrapper256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        zip(&self.inner, &other.inner)
            .map(|(lhs, rhs)| lhs.partial_cmp(rhs))
            .ok()
            .flatten()
    }
}

macro_rules! impl_op_fixed_wrapper_for_type {
    (
        $op:ident,
        $op_fn:ident,
        $type:ty
    ) => {
        // left (FixedWrapper256 + $type)
        impl $op<$type> for FixedWrapper256 {
            type Output = Self;

            fn $op_fn(self, rhs: $type) -> Self::Output {
                if self.inner.is_err() {
                    return Err(ArithmeticError::Overflow).into();
                }
                let rhs: FixedWrapper256 = rhs.into();
                self.$op_fn(rhs)
            }
        }
        // right ($type + FixedWrapper256)
        impl $op<FixedWrapper256> for $type {
            type Output = FixedWrapper256;

            fn $op_fn(self, rhs: FixedWrapper256) -> Self::Output {
                if rhs.inner.is_err() {
                    return Err(ArithmeticError::Overflow).into();
                }
                let lhs: FixedWrapper256 = self.into();
                lhs.$op_fn(rhs)
            }
        }
    };
}

macro_rules! impl_fixed_wrapper_for_type {
    ($type:ty) => {
        impl_op_fixed_wrapper_for_type!(Add, add, $type);
        impl_op_fixed_wrapper_for_type!(Sub, sub, $type);
        impl_op_fixed_wrapper_for_type!(Mul, mul, $type);
        impl_op_fixed_wrapper_for_type!(Div, div, $type);
    };
}

// Here one can add more custom implementations.
impl_fixed_wrapper_for_type!(FixedU256);
impl_fixed_wrapper_for_type!(u128);

#[macro_export]
macro_rules! fixed_u256_int {
    ($val:literal) => {
        $crate::fixed::FixedU256::try_from($val)
            .unwrap()
            .fixed()
            .unwrap()
    };
}

#[macro_export]
macro_rules! fixed_wrapper_u256_int {
    ($val:literal) => {{
        let val: $crate::fixed_wrapper_u256::FixedWrapper256 = fixed_u256_int!($val).into();
        val
    }};
}

#[macro_export]
macro_rules! fixed_u256_float {
    ($val:literal) => {
        $crate::fixed::FixedU256::try_from($val).unwrap()
    };
}

#[macro_export]
macro_rules! fixed_wrapper_u256_float {
    ($val:literal) => {{
        let val: $crate::fixed_wrapper_u256::FixedWrapper256 = fixed_u256_float!($val).into();
        val
    }};
}

#[cfg(test)]
mod wrapper {
    use crate::fixed_wrapper_u256::FixedWrapper256;
    use crate::*;

    #[test]
    fn fixed_wrapper_sqrt_small_sanity_check() {
        // basic
        assert_eq!(
            fixed_wrapper_u256_int!(4).sqrt_accurate(),
            fixed_wrapper_u256_int!(2)
        );
        // zero
        assert_eq!(
            fixed_wrapper_u256_int!(0).sqrt_accurate(),
            fixed_wrapper_u256_int!(0)
        );
        // negative
        assert!((fixed_wrapper_u256_int!(0) - fixed_wrapper_u256_int!(4))
            .sqrt_accurate()
            .get()
            .is_err());
        let almost_max: [u64; 4] = [
            18446744073709551615,
            18446744073709551615,
            18446744073709551615,
            15,
        ];
        let to_max = sp_core::U256::MAX - sp_core::U256(almost_max) + 1;
        // over the max
        assert!(
            (FixedWrapper256::from(sp_core::U256(almost_max)) + FixedWrapper256::from(to_max))
                .sqrt_accurate()
                .get()
                .is_err()
        );
        // normal large
        // assert_eq!(
        //     fixed_wrapper_u256!(3743450969434.400440997399628828).sqrt_accurate(),
        //     fixed_wrapper_u256!(1934799.981764110013554299)
        // )
    }
}
