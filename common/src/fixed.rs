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

use crate::arithmetic::helpers_256bit::sqrt;
use codec::{CompactAs, Decode, Encode};
use fixnum::ArithmeticError;
// use num_traits::Signed;
use sp_arithmetic::{
    // PerThing,
    // Perbill,
    Rounding,
    SignedRounding,
};
use sp_core::U256;
use sp_std::{
    fmt::Debug,
    ops::{self, Add, Div, Mul, Sub},
    prelude::*,
};

use crate::arithmetic::{
    bounds::Bounded,
    checked::{CheckedAdd, CheckedDiv, CheckedMul, CheckedNeg, CheckedSub},
    helpers_256bit::multiply_by_rational_with_rounding,
    identities::{One, Zero},
    saturating::{SaturatedConversion, UniqueSaturatedInto},
    sp_saturating::Saturating,
};
#[cfg(feature = "std")]
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// Integer types that can be used to interact with `FixedPointNumber` implementations.
pub trait FixedPointOperand:
    Copy
    + Clone
    + Bounded
    + Zero
    + Saturating
    + UniqueSaturatedInto<U256>
    + PartialOrd
    + TryFrom<U256>
    + CheckedNeg
{
}

impl FixedPointOperand for U256 {}
impl FixedPointOperand for i128 {}
impl FixedPointOperand for u128 {}
impl FixedPointOperand for i64 {}
impl FixedPointOperand for u64 {}
impl FixedPointOperand for i32 {}
impl FixedPointOperand for u32 {}
impl FixedPointOperand for i16 {}
impl FixedPointOperand for u16 {}
impl FixedPointOperand for i8 {}
impl FixedPointOperand for u8 {}

/// Data type used as intermediate storage in some computations to avoid overflow.
struct I257 {
    value: U256,
    negative: bool,
}

impl<N: FixedPointOperand> From<N> for I257 {
    fn from(n: N) -> I257 {
        if n < N::zero() {
            let value: U256 = n
                .checked_neg()
                .map(|n| n.unique_saturated_into())
                .unwrap_or_else(|| {
                    N::max_value()
                        .unique_saturated_into()
                        .saturating_add(U256::one())
                });
            I257 {
                value,
                negative: true,
            }
        } else {
            I257 {
                value: n.unique_saturated_into(),
                negative: false,
            }
        }
    }
}

/// Transforms an `I257` to `N` if it is possible.
fn from_i257<N: FixedPointOperand>(n: I257) -> Option<N> {
    let max_plus_one: U256 = N::max_value()
        .unique_saturated_into()
        .saturating_add(U256::one());
    if n.negative && N::min_value() < N::zero() && n.value == max_plus_one {
        Some(N::min_value())
    } else {
        let unsigned_inner: N = n.value.try_into().ok()?;
        let inner = if n.negative {
            unsigned_inner.checked_neg()?
        } else {
            unsigned_inner
        };
        Some(inner)
    }
}

/// Returns `R::max` if the sign of `n * m` is positive, `R::min` otherwise.
fn to_bound<N: FixedPointOperand, D: FixedPointOperand, R: Bounded>(n: N, m: D) -> R {
    if (n < N::zero()) != (m < D::zero()) {
        R::min_value()
    } else {
        R::max_value()
    }
}

/// Something that implements a decimal fixed point number.
///
/// The precision is given by `Self::DIV`, i.e. `1 / DIV` can be represented.
///
/// Each type can store numbers from `Self::Inner::min_value() / Self::DIV`
/// to `Self::Inner::max_value() / Self::DIV`.
/// This is also referred to as the _accuracy_ of the type in the documentation.
pub trait FixedPointNumber:
    Sized
    + Copy
    + Default
    + Debug
    + Saturating
    + Bounded
    + Eq
    + PartialEq
    + Ord
    + PartialOrd
    + CheckedSub
    + CheckedAdd
    + CheckedMul
    + CheckedDiv
    + Add
    + Sub
    + Div
    + Mul
    + Zero
    + One
{
    /// The underlying data type used for this fixed point number.
    type Inner: Debug + One + CheckedMul + CheckedDiv + FixedPointOperand + From<u128>;

    /// Precision of this fixed point implementation. It should be a power of `10`.
    const DIV: Self::Inner;

    /// Indicates if this fixed point implementation is signed or not.
    const SIGNED: bool;

    /// Precision of this fixed point implementation.
    fn accuracy() -> Self::Inner {
        Self::DIV
    }

    /// Builds this type from an integer number.
    fn from_inner(int: Self::Inner) -> Self;

    /// Consumes `self` and returns the inner raw value.
    fn into_inner(self) -> Self::Inner;

    /// Creates self from an integer number `int`.
    ///
    /// Returns `Self::max` or `Self::min` if `int` exceeds accuracy.
    fn saturating_from_integer<N: FixedPointOperand>(int: N) -> Self {
        let mut n: I257 = int.into();
        n.value = n.value.saturating_mul(Self::DIV.saturated_into());
        Self::from_inner(from_i257(n).unwrap_or_else(|| to_bound(int, 0)))
    }

    /// Creates `self` from an integer number `int`.
    ///
    /// Returns `None` if `int` exceeds accuracy.
    fn checked_from_integer<N: UniqueSaturatedInto<Self::Inner>>(int: N) -> Option<Self> {
        let int: Self::Inner = int.unique_saturated_into();
        int.checked_mul(&Self::DIV).map(Self::from_inner)
    }

    /// Creates `self` from a rational number. Equal to `n / d`.
    ///
    /// Panics if `d = 0`. Returns `Self::max` or `Self::min` if `n / d` exceeds accuracy.
    fn saturating_from_rational<N: FixedPointOperand, D: FixedPointOperand>(n: N, d: D) -> Self {
        if d == D::zero() {
            panic!("attempt to divide by zero")
        }
        Self::checked_from_rational(n, d).unwrap_or_else(|| to_bound(n, d))
    }

    /// Creates `self` from a rational number. Equal to `n / d`.
    ///
    /// Returns `None` if `d == 0` or `n / d` exceeds accuracy.
    fn checked_from_rational<N: FixedPointOperand, D: FixedPointOperand>(
        n: N,
        d: D,
    ) -> Option<Self> {
        if d == D::zero() {
            return None;
        }

        let n: I257 = n.into();
        let d: I257 = d.into();
        let negative = n.negative != d.negative;

        multiply_by_rational_with_rounding(
            n.value,
            Self::DIV.unique_saturated_into(),
            d.value,
            Rounding::from_signed(SignedRounding::Minor, negative),
        )
        .and_then(|value| from_i257(I257 { value, negative }))
        .map(Self::from_inner)
    }

    /// Checked multiplication for integer type `N`. Equal to `self * n`.
    ///
    /// Returns `None` if the result does not fit in `N`.
    fn checked_mul_int<N: FixedPointOperand>(self, n: N) -> Option<N> {
        let lhs: I257 = self.into_inner().into();
        let rhs: I257 = n.into();
        let negative = lhs.negative != rhs.negative;

        multiply_by_rational_with_rounding(
            lhs.value,
            rhs.value,
            Self::DIV.unique_saturated_into(),
            Rounding::from_signed(SignedRounding::Minor, negative),
        )
        .and_then(|value| from_i257(I257 { value, negative }))
    }

    /// Saturating multiplication for integer type `N`. Equal to `self * n`.
    ///
    /// Returns `N::min` or `N::max` if the result does not fit in `N`.
    fn saturating_mul_int<N: FixedPointOperand>(self, n: N) -> N {
        self.checked_mul_int(n)
            .unwrap_or_else(|| to_bound(self.into_inner(), n))
    }

    /// Checked division for integer type `N`. Equal to `self / d`.
    ///
    /// Returns `None` if the result does not fit in `N` or `d == 0`.
    fn checked_div_int<N: FixedPointOperand>(self, d: N) -> Option<N> {
        let lhs: I257 = self.into_inner().into();
        let rhs: I257 = d.into();
        let negative = lhs.negative != rhs.negative;

        lhs.value
            .checked_div(rhs.value)
            .and_then(|n| n.checked_div(Self::DIV.unique_saturated_into()))
            .and_then(|value| from_i257(I257 { value, negative }))
    }

    /// Saturating division for integer type `N`. Equal to `self / d`.
    ///
    /// Panics if `d == 0`. Returns `N::min` or `N::max` if the result does not fit in `N`.
    fn saturating_div_int<N: FixedPointOperand>(self, d: N) -> N {
        if d == N::zero() {
            panic!("attempt to divide by zero")
        }
        self.checked_div_int(d)
            .unwrap_or_else(|| to_bound(self.into_inner(), d))
    }

    /// Saturating multiplication for integer type `N`, adding the result back.
    /// Equal to `self * n + n`.
    ///
    /// Returns `N::min` or `N::max` if the multiplication or final result does not fit in `N`.
    fn saturating_mul_acc_int<N: FixedPointOperand>(self, n: N) -> N {
        if self.is_negative() && n > N::zero() {
            n.saturating_sub(Self::zero().saturating_sub(self).saturating_mul_int(n))
        } else {
            self.saturating_mul_int(n).saturating_add(n)
        }
    }

    /// Saturating absolute value.
    ///
    /// Returns `Self::max` if `self == Self::min`.
    fn saturating_abs(self) -> Self {
        let inner = self.into_inner();
        if inner >= Self::Inner::zero() {
            self
        } else {
            Self::from_inner(inner.checked_neg().unwrap_or_else(Self::Inner::max_value))
        }
    }

    /// Takes the reciprocal (inverse). Equal to `1 / self`.
    ///
    /// Returns `None` if `self = 0`.
    fn reciprocal(self) -> Option<Self> {
        Self::one().checked_div(&self)
    }

    /// Checks if the number is one.
    fn is_one(&self) -> bool {
        self.into_inner() == Self::Inner::one()
    }

    /// Returns `true` if `self` is positive and `false` if the number is zero or negative.
    fn is_positive(self) -> bool {
        self.into_inner() > Self::Inner::zero()
    }

    /// Returns `true` if `self` is negative and `false` if the number is zero or positive.
    fn is_negative(self) -> bool {
        self.into_inner() < Self::Inner::zero()
    }

    /// Returns the integer part.
    fn trunc(self) -> Self {
        self.into_inner()
            .checked_div(&Self::DIV)
            .expect("panics only if DIV is zero, DIV is not zero; qed")
            .checked_mul(&Self::DIV)
            .map(Self::from_inner)
            .expect("can not overflow since fixed number is >= integer part")
    }

    /// Returns the fractional part.
    ///
    /// Note: the returned fraction will be non-negative for negative numbers,
    /// except in the case where the integer part is zero.
    fn frac(self) -> Self {
        let integer = self.trunc();
        let fractional = self.saturating_sub(integer);
        if integer == Self::zero() {
            fractional
        } else {
            fractional.saturating_abs()
        }
    }

    /// Returns the smallest integer greater than or equal to a number.
    ///
    /// Saturates to `Self::max` (truncated) if the result does not fit.
    fn ceil(self) -> Self {
        if self.is_negative() {
            self.trunc()
        } else if self.frac() == Self::zero() {
            self
        } else {
            self.saturating_add(Self::one()).trunc()
        }
    }

    /// Returns the largest integer less than or equal to a number.
    ///
    /// Saturates to `Self::min` (truncated) if the result does not fit.
    fn floor(self) -> Self {
        if self.is_negative() {
            self.saturating_sub(Self::one()).trunc()
        } else {
            self.trunc()
        }
    }

    /// Returns the number rounded to the nearest integer. Rounds half-way cases away from 0.0.
    ///
    /// Saturates to `Self::min` or `Self::max` (truncated) if the result does not fit.
    fn round(self) -> Self {
        let n = self
            .frac()
            .saturating_mul(Self::saturating_from_integer(10));
        if n < Self::saturating_from_integer(5) {
            self.trunc()
        } else if self.is_positive() {
            self.saturating_add(Self::one()).trunc()
        } else {
            self.saturating_sub(Self::one()).trunc()
        }
    }
}

/// A fixed point number representation in the range.
/// _Fixed Point 256 bits unsigned, range = [0.000000000000000000, ]_
#[derive(
    Encode,
    Decode,
    CompactAs,
    Default,
    Copy,
    Clone,
    codec::MaxEncodedLen,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    scale_info::TypeInfo,
)]
pub struct FixedU256(U256);

macro_rules! impl_from_for_fixed {
    ($( $T:ty ),+) => {
        $( impl_from_for_fixed!(@single $T); )*
    };
    (@single $T:ty) => {
        impl TryFrom<$T> for FixedU256 {
            type Error = ArithmeticError;

            fn try_from(value: $T) -> Result<Self, Self::Error> {
                if value < <$T>::zero() {
                    return Err(ArithmeticError::DomainViolation)
                } else {
                    Ok(Self(U256::from(value)))
                }
            }
        }
    };
}

impl_from_for_fixed!(usize, isize, U256, u128, i128, u64, i64, u32, i32);

impl TryFrom<f64> for FixedU256 {
    type Error = ArithmeticError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if value.is_sign_negative() {
            return Err(ArithmeticError::DomainViolation);
        }
        let value = value * Self::DIV.as_u128() as f64;
        if value.is_finite() {
            Ok(Self(U256::from(value as u128)))
        } else {
            Err(ArithmeticError::Overflow)
        }
    }
}

impl<N: FixedPointOperand, D: FixedPointOperand> From<(N, D)> for FixedU256 {
    fn from(r: (N, D)) -> Self {
        FixedU256::saturating_from_rational(r.0, r.1)
    }
}

impl FixedPointNumber for FixedU256 {
    type Inner = U256;

    const DIV: Self::Inner = U256([1_000_000_000_000_000_000, 0, 0, 0]);
    const SIGNED: bool = false;

    fn from_inner(inner: Self::Inner) -> Self {
        Self(inner)
    }

    fn into_inner(self) -> Self::Inner {
        self.0
    }
}

impl FixedU256 {
    /// Create a new instance from the given `inner` value.
    ///
    /// `const` version of `FixedPointNumber::from_inner`.
    pub const fn from_inner(inner: U256) -> Self {
        Self(inner)
    }

    /// Return the instance's inner value.
    ///
    /// `const` version of `FixedPointNumber::into_inner`.
    pub const fn into_inner(self) -> U256 {
        self.0
    }

    /// Return a new instance from the given `inner` value, but more times the precision
    pub fn fixed(self) -> Result<Self, ArithmeticError> {
        self.0
            .checked_mul(Self::accuracy())
            .map(Self)
            .ok_or(ArithmeticError::Overflow)
    }

    /// Creates self from a `u32`.
    ///
    /// WARNING: This function designed for convenient use at build time and
    /// will panic on overflow. Ensure that any inputs are sensible.
    pub fn from_u32(n: u32) -> Self {
        Self::from_inner(U256::from(n) * Self::DIV)
    }

    /// Convert from a `float` value.
    #[cfg(any(feature = "std", test))]
    pub fn from_float(x: f64) -> Result<Self, ArithmeticError> {
        if x.is_sign_negative() {
            return Err(ArithmeticError::DomainViolation);
        }
        let value = x * Self::DIV.as_u128() as f64;
        if value.is_finite() {
            Ok(Self(U256::from(value as u128)))
        } else {
            Err(ArithmeticError::Overflow)
        }
    }

    // /// Convert from a `Perbill` value.
    // pub const fn from_perbill(n: Perbill) -> Self {
    //     Self::from_rational(n.deconstruct() as u128, 1_000_000_000)
    // }
    //
    // /// Convert into a `Perbill` value. Will saturate if above one or below zero.
    // pub const fn into_perbill(self) -> Perbill {
    //     if self.0 <= U256::zero() {
    //         Perbill::zero()
    //     } else if self.0 >= Self::DIV {
    //         Perbill::one()
    //     } else {
    //         match multiply_by_rational_with_rounding(
    //             self.0.as_u128(),
    //             1_000_000_000,
    //             Self::DIV.as_u128(),
    //             Rounding::NearestPrefDown,
    //         ) {
    //             Some(value) => {
    //                 if value > (u32::max_value() as u128) {
    //                     panic!(
    //                         "prior logic ensures 0<self.0<DIV; \
    //                         multiply ensures 0<self.0<1000000000; \
    //                         qed"
    //                     );
    //                 }
    //                 Perbill::from_parts(value as u32)
    //             },
    //             None => Perbill::zero(),
    //         }
    //     }
    // }

    /// Convert into a `float` value.
    #[cfg(any(feature = "std", test))]
    pub fn to_float(self) -> f64 {
        self.0.as_u128() as f64 / Self::DIV.as_u128() as f64
    }

    pub fn to_u128(self) -> Result<u128, ArithmeticError> {
        if self.0 .0[2] != 0 || self.0 .0[3] != 0 {
            Err(ArithmeticError::Overflow)
        } else {
            Ok(self.0.low_u128())
        }
    }

    // /// Attempt to convert into a `PerThing`. This will succeed iff `self` is at least zero
    // /// and at most one. If it is out of bounds, it will result in an error returning the
    // /// clamped value.
    // pub fn try_into_perthing<P: PerThing>(self) -> Result<P, P> {
    //     if self < Self::zero() {
    //         Err(P::zero())
    //     } else if self > Self::one() {
    //         Err(P::one())
    //     } else {
    //         Ok(P::from_rational(self.0, Self::DIV))
    //     }
    // }

    // /// Attempt to convert into a `PerThing`. This will always succeed resulting in a
    // /// clamped value if `self` is less than zero or greater than one.
    // pub fn into_clamped_perthing<P: PerThing>(self) -> P {
    //     if self < Self::zero() {
    //         P::zero()
    //     } else if self > Self::one() {
    //         P::one()
    //     } else {
    //         P::from_rational(self.0, Self::DIV)
    //     }
    // }

    // /// Negate the value.
    // ///
    // /// WARNING: This is a `const` function designed for convenient use at build time and
    // /// will panic on overflow. Ensure that any inputs are sensible.
    // pub const fn neg(self) -> Self {
    //     Self(U256::zero() - self.0)
    // }

    /// Take the square root of a positive value.
    ///
    /// WARNING: This function designed for convenient use at build time and
    /// will panic on overflow. Ensure that any inputs are sensible.
    pub fn sqrt(self) -> Result<Self, ArithmeticError> {
        match self.try_sqrt() {
            Some(v) => Ok(v),
            None => Err(ArithmeticError::Overflow),
        }
    }

    /// Compute the square root, rounding as desired. If it overflows or is negative, then
    /// `None` is returned.
    pub fn try_sqrt(self) -> Option<Self> {
        if self.is_zero() {
            return Some(Self(U256::zero()));
        }

        let v = self.0;

        // Want x' = sqrt(x) where x = n/D and x' = n'/D (D is fixed)
        // Our prefered way is:
        //   sqrt(n/D) = sqrt(nD / D^2) = sqrt(nD)/sqrt(D^2) = sqrt(nD)/D
        //   ergo n' = sqrt(nD)
        // but this requires nD to fit into our type.
        // if nD doesn't fit then we can fall back on:
        //   sqrt(nD) = sqrt(n)*sqrt(D)
        // computing them individually and taking the product at the end. we will lose some
        // precision though.
        let maybe_vd = U256::checked_mul(v, Self::DIV);
        let r = if let Some(vd) = maybe_vd {
            sqrt(vd)
        } else {
            sqrt(v) * sqrt(Self::DIV)
        };
        Some(Self(r))
    }

    /// Convert into an `I257` format value.
    ///
    /// WARNING: This function designed for convenient use at build time and
    /// will panic on overflow. Ensure that any inputs are sensible.
    const fn into_i257(self) -> I257 {
        // if self.0 < U256::zero() {
        //     let value = match self.0.checked_neg() {
        //         Some(n) => n.as_u128(),
        //         None => u128::saturating_add(U256::max_value().as_u128(), 1),
        //     };
        //     I257 { value, negative: true }
        // } else {
        I257 {
            value: self.0,
            negative: false,
        }
        // }
    }

    /// Convert from an `I257` format value.
    ///
    /// WARNING: This function designed for convenient use at build time and
    /// will panic on overflow. Ensure that any inputs are sensible.
    fn from_i257(n: I257) -> Option<Self> {
        let max_plus_one = U256::saturating_add(U256::max_value(), U256::one());
        let inner = if n.negative && U256::min_value() < U256::zero() && n.value == max_plus_one {
            U256::min_value()
        } else {
            let unsigned_inner = n.value;
            if unsigned_inner != n.value
                || (unsigned_inner > U256::zero()) != (n.value > U256::zero())
            {
                return None;
            };
            if n.negative {
                match unsigned_inner.checked_neg() {
                    Some(v) => v,
                    None => return None,
                }
            } else {
                unsigned_inner
            }
        };
        Some(Self(inner))
    }

    /// Calculate an approximation of a rational.
    ///
    /// Result will be rounded to the nearest representable value, rounding down if it is
    /// equidistant between two neighbours.
    ///
    /// WARNING: This function designed for convenient use at build time and
    /// will panic on overflow. Ensure that any inputs are sensible.
    pub fn from_rational(a: U256, b: U256) -> Self {
        Self::from_rational_with_rounding(a, b, Rounding::NearestPrefDown)
    }

    /// Calculate an approximation of a rational with custom rounding.
    ///
    /// WARNING: This function designed for convenient use at build time and
    /// will panic on overflow. Ensure that any inputs are sensible.
    pub fn from_rational_with_rounding(a: U256, b: U256, rounding: Rounding) -> Self {
        if b.is_zero() {
            panic!("attempt to divide by zero in from_rational");
        }
        match multiply_by_rational_with_rounding(Self::DIV, a, b, rounding) {
            Some(value) => match Self::from_i257(I257 {
                value,
                negative: false,
            }) {
                Some(x) => x,
                None => panic!("overflow in from_rational"),
            },
            None => panic!("overflow in from_rational"),
        }
    }

    /// Multiply by another value, returning `None` in the case of an error.
    ///
    /// Result will be rounded to the nearest representable value, rounding down if it is
    /// equidistant between two neighbours.
    pub fn const_checked_mul(self, other: Self) -> Option<Self> {
        self.const_checked_mul_with_rounding(other, SignedRounding::NearestPrefLow)
    }

    /// Multiply by another value with custom rounding, returning `None` in the case of an
    /// error.
    ///
    /// Result will be rounded to the nearest representable value, rounding down if it is
    /// equidistant between two neighbours.
    pub fn const_checked_mul_with_rounding(
        self,
        other: Self,
        rounding: SignedRounding,
    ) -> Option<Self> {
        let lhs = self.into_i257();
        let rhs = other.into_i257();
        let negative = lhs.negative != rhs.negative;

        match multiply_by_rational_with_rounding(
            lhs.value,
            rhs.value,
            Self::DIV,
            Rounding::from_signed(rounding, negative),
        ) {
            Some(value) => Self::from_i257(I257 { value, negative }),
            None => None,
        }
    }

    /// Divide by another value, returning `None` in the case of an error.
    ///
    /// Result will be rounded to the nearest representable value, rounding down if it is
    /// equidistant between two neighbours.
    pub fn const_checked_div(self, other: Self) -> Option<Self> {
        self.checked_rounding_div(other, SignedRounding::NearestPrefLow)
    }

    /// Divide by another value with custom rounding, returning `None` in the case of an
    /// error.
    ///
    /// Result will be rounded to the nearest representable value, rounding down if it is
    /// equidistant between two neighbours.
    pub fn checked_rounding_div(self, other: Self, rounding: SignedRounding) -> Option<Self> {
        if other.0.is_zero() {
            return None;
        }

        let lhs = self.into_i257();
        let rhs = other.into_i257();
        let negative = lhs.negative != rhs.negative;

        match multiply_by_rational_with_rounding(
            lhs.value,
            Self::DIV,
            rhs.value,
            Rounding::from_signed(rounding, negative),
        ) {
            Some(value) => Self::from_i257(I257 { value, negative }),
            None => None,
        }
    }
}

impl Saturating for FixedU256 {
    fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    fn saturating_mul(self, rhs: Self) -> Self {
        self.checked_mul(&rhs)
            .unwrap_or_else(|| to_bound(self.0, rhs.0))
    }

    fn saturating_pow(self, exp: usize) -> Self {
        if exp == 0 {
            return Self::saturating_from_integer(1);
        }

        let exp = exp as u32;
        let msb_pos = 32 - exp.leading_zeros();

        let mut result = Self::saturating_from_integer(1);
        let mut pow_val = self;
        for i in 0..msb_pos {
            if ((1 << i) & exp) > 0 {
                result = result.saturating_mul(pow_val);
            }
            pow_val = pow_val.saturating_mul(pow_val);
        }
        result
    }
}

impl ops::Neg for FixedU256 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(U256::zero() - self.0)
    }
}

impl Add for FixedU256 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for FixedU256 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul for FixedU256 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        self.checked_mul(&rhs)
            .unwrap_or_else(|| panic!("attempt to multiply with overflow"))
    }
}

impl Div for FixedU256 {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        if rhs.0 == U256::zero() {
            panic!("attempt to divide by zero");
        }
        self.checked_div(&rhs)
            .unwrap_or_else(|| panic!("attempt to divide with overflow"))
    }
}

impl CheckedSub for FixedU256 {
    type Error = ArithmeticError;

    fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    fn csub(&self, rhs: &Self) -> Result<Self, Self::Error> {
        self.checked_sub(rhs).ok_or(ArithmeticError::Overflow)
    }
}

impl CheckedAdd for FixedU256 {
    type Error = ArithmeticError;

    fn checked_add(&self, rhs: &Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    fn cadd(&self, rhs: &Self) -> Result<Self, Self::Error> {
        self.checked_add(rhs).ok_or(ArithmeticError::Overflow)
    }
}

impl CheckedDiv for FixedU256 {
    type Error = ArithmeticError;

    fn checked_div(&self, other: &Self) -> Option<Self> {
        if other.0.is_zero() {
            return None;
        }

        let lhs: I257 = self.0.into();
        let rhs: I257 = other.0.into();
        let negative = lhs.negative != rhs.negative;

        // Note that this uses the old (well-tested) code with sign-ignorant rounding. This
        // is equivalent to the `SignedRounding::NearestPrefMinor`. This means it is
        // expected to give exactly the same result as `const_checked_div` when the result
        // is positive and a result up to one epsilon greater when it is negative.
        multiply_by_rational_with_rounding(
            lhs.value,
            Self::DIV,
            rhs.value,
            Rounding::from_signed(SignedRounding::Minor, negative),
        )
        .and_then(|value| from_i257(I257 { value, negative }))
        .map(Self)
    }

    fn cdiv(&self, other: &Self) -> Result<Self, Self::Error> {
        if other.0.is_zero() {
            return Err(ArithmeticError::DivisionByZero);
        }

        let lhs: I257 = self.0.into();
        let rhs: I257 = other.0.into();
        let negative = lhs.negative != rhs.negative;

        // Note that this uses the old (well-tested) code with sign-ignorant rounding. This
        // is equivalent to the `SignedRounding::NearestPrefMinor`. This means it is
        // expected to give exactly the same result as `const_checked_div` when the result
        // is positive and a result up to one epsilon greater when it is negative.
        multiply_by_rational_with_rounding(
            lhs.value,
            Self::DIV,
            rhs.value,
            Rounding::from_signed(SignedRounding::Minor, negative),
        )
        .and_then(|value| from_i257(I257 { value, negative }))
        .map(Self)
        .ok_or(ArithmeticError::Overflow)
    }
}

impl CheckedMul for FixedU256 {
    type Error = ArithmeticError;

    fn checked_mul(&self, other: &Self) -> Option<Self> {
        let lhs: I257 = self.0.into();
        let rhs: I257 = other.0.into();
        let negative = lhs.negative != rhs.negative;

        multiply_by_rational_with_rounding(
            lhs.value,
            rhs.value,
            Self::DIV,
            Rounding::from_signed(SignedRounding::Minor, negative),
        )
        .and_then(|value| from_i257(I257 { value, negative }))
        .map(Self)
    }

    fn cmul(&self, other: &Self) -> Result<Self, Self::Error> {
        self.checked_mul(other).ok_or(ArithmeticError::Overflow)
    }
}

impl CheckedNeg for FixedU256 {
    type Error = ArithmeticError;

    fn checked_neg(&self) -> Option<Self> {
        self.0.checked_neg().map(Self)
    }

    fn cneg(&self) -> Result<Self, Self::Error> {
        self.checked_neg().ok_or(ArithmeticError::Overflow)
    }
}

impl Bounded for FixedU256 {
    fn min_value() -> Self {
        Self(<Self as FixedPointNumber>::Inner::min_value())
    }

    fn max_value() -> Self {
        Self(<Self as FixedPointNumber>::Inner::max_value())
    }
}

impl Zero for FixedU256 {
    fn zero() -> Self {
        Self::from_inner(<Self as FixedPointNumber>::Inner::zero())
    }

    fn is_zero(&self) -> bool {
        self.into_inner() == <Self as FixedPointNumber>::Inner::zero()
    }
}

impl One for FixedU256 {
    fn one() -> Self {
        Self::from_inner(Self::DIV)
    }
}

impl Debug for FixedU256 {
    #[cfg(feature = "std")]
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        let integral = {
            let int = self.0 / Self::accuracy();
            let signum_for_zero = if int.is_zero() && self.is_negative() {
                "-"
            } else {
                ""
            };
            format!("{}{}", signum_for_zero, int)
        };
        let precision = (Self::accuracy().as_u128() as f64).log10() as usize;
        let fractional = format!(
            "{:0>weight$}",
            ((self.0 % Self::accuracy()).as_u128() as i128).abs(),
            weight = precision
        );
        write!(f, "{}({}.{})", stringify!(FixedU256), integral, fractional)
    }

    #[cfg(not(feature = "std"))]
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

// impl<P: PerThing> From<P> for FixedU256
// where
// 	P::Inner: FixedPointOperand,
// {
// 	fn from(p: P) -> Self {
// 		let accuracy = P::ACCURACY;
// 		let value = p.deconstruct();
// 		FixedU256::saturating_from_rational(value, accuracy)
// 	}
// }

#[cfg(feature = "std")]
impl sp_std::fmt::Display for FixedU256 {
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(feature = "std")]
impl sp_std::str::FromStr for FixedU256 {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner: <Self as FixedPointNumber>::Inner = s
            .parse()
            .map_err(|_| "invalid string input for fixed point number")?;
        Ok(Self::from_inner(inner))
    }
}

// Manual impl `Serialize` as serde_json does not support i128.
// TODO: remove impl if issue https://github.com/serde-rs/json/issues/548 fixed.
#[cfg(feature = "std")]
impl Serialize for FixedU256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// Manual impl `Deserialize` as serde_json does not support i128.
// TODO: remove impl if issue https://github.com/serde-rs/json/issues/548 fixed.
#[cfg(feature = "std")]
impl<'de> Deserialize<'de> for FixedU256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use sp_std::str::FromStr;
        let s = String::deserialize(deserializer)?;
        FixedU256::from_str(&s).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod fixed_u256_test {
    use super::*;
    // use sp_arithmetic::{Perbill, Percent, Permill, Perquintill};
    // use crate::arithmetic::saturating::UniqueSaturatedInto;

    fn max() -> FixedU256 {
        FixedU256::max_value()
    }

    fn min() -> FixedU256 {
        FixedU256::min_value()
    }

    #[allow(unused)]
    fn precision() -> usize {
        (FixedU256::accuracy().as_u128() as f64).log10() as usize
    }

    #[test]
    fn macro_preconditions() {
        assert!(FixedU256::DIV > U256::zero());
    }

    #[test]
    fn has_max_encoded_len() {
        struct AsMaxEncodedLen<T: codec::MaxEncodedLen> {
            _data: T,
        }

        let _ = AsMaxEncodedLen {
            _data: FixedU256::min_value(),
        };
    }

    #[test]
    fn from_i257_works() {
        let a = I257 {
            value: U256::one(),
            negative: true,
        };

        // Can't convert negative number to unsigned.
        assert_eq!(from_i257::<U256>(a), None);

        let a = I257 {
            value: U256::MAX - 1,
            negative: false,
        };

        // Max - 1 value fits.
        assert_eq!(from_i257::<U256>(a), Some(U256::MAX - 1));

        let a = I257 {
            value: U256::MAX,
            negative: false,
        };

        // Max value fits.
        assert_eq!(from_i257::<U256>(a), Some(U256::MAX));

        let a = I257 {
            value: U256::from(i128::MAX) + 1,
            negative: true,
        };

        // Min value fits.
        assert_eq!(from_i257::<i128>(a), Some(i128::MIN));

        let a = I257 {
            value: U256::from(i128::MAX) + 1,
            negative: false,
        };

        // Max + 1 does not fit.
        assert_eq!(from_i257::<i128>(a), None);

        let a = I257 {
            value: U256::from(i128::MAX),
            negative: false,
        };

        // Max value fits.
        assert_eq!(from_i257::<i128>(a), Some(i128::MAX));
    }

    #[test]
    fn to_bound_works() {
        let a = 1i32;
        let b = 1i32;

        // Pos + Pos => Max.
        assert_eq!(to_bound::<_, _, i32>(a, b), i32::MAX);

        let a = -1i32;
        let b = -1i32;

        // Neg + Neg => Max.
        assert_eq!(to_bound::<_, _, i32>(a, b), i32::MAX);

        let a = 1i32;
        let b = -1i32;

        // Pos + Neg => Min.
        assert_eq!(to_bound::<_, _, i32>(a, b), i32::MIN);

        let a = -1i32;
        let b = 1i32;

        // Neg + Pos => Min.
        assert_eq!(to_bound::<_, _, i32>(a, b), i32::MIN);

        let a = 1i32;
        let b = -1i32;

        // Pos + Neg => Min (unsigned).
        assert_eq!(to_bound::<_, _, u32>(a, b), 0);
    }

    #[test]
    fn op_neg_works() {
        let a = FixedU256::zero();
        let b = -a;

        // Zero.
        assert_eq!(a, b);

        // if FixedU256::SIGNED {
        //     let a = FixedU256::saturating_from_integer(5);
        //     let b = -a;
        //
        //     // Positive.
        //     assert_eq!(FixedU256::saturating_from_integer(-5), b);
        //
        //     let a = FixedU256::saturating_from_integer(-5);
        //     let b = -a;
        //
        //     // Negative
        //     assert_eq!(FixedU256::saturating_from_integer(5), b);
        //
        //     let a = FixedU256::max_value();
        //     let b = -a;
        //
        //     // Max.
        //     assert_eq!(FixedU256::min_value() + FixedU256::from_inner(U256::one()), b);
        //
        //     let a = FixedU256::min_value() + FixedU256::from_inner(U256::one());
        //     let b = -a;
        //
        //     // Min.
        //     assert_eq!(FixedU256::max_value(), b);
        // }
    }

    #[test]
    fn op_checked_add_overflow_works() {
        let a = FixedU256::max_value();
        let b = 1.try_into().unwrap();
        assert!(a.checked_add(&b).is_none());
    }

    #[test]
    fn op_add_works() {
        let a = FixedU256::saturating_from_rational(5, 2);
        let b = FixedU256::saturating_from_rational(1, 2);

        // Positive case: 6/2 = 3.
        assert_eq!(FixedU256::saturating_from_integer(3), a + b);

        // if FixedU256::SIGNED {
        //     // Negative case: 4/2 = 2.
        //     let b = FixedU256::saturating_from_rational(1, -2);
        //     assert_eq!(FixedU256::saturating_from_integer(2), a + b);
        // }
    }

    #[test]
    fn op_checked_sub_underflow_works() {
        let a = FixedU256::min_value();
        let b = 1.try_into().unwrap();
        assert!(a.checked_sub(&b).is_none());
    }

    #[test]
    fn op_sub_works() {
        let a = FixedU256::saturating_from_rational(5, 2);
        let b = FixedU256::saturating_from_rational(1, 2);

        assert_eq!(FixedU256::saturating_from_integer(2), a - b);
        assert_eq!(FixedU256::saturating_from_integer(-2), b.saturating_sub(a));
    }

    #[test]
    fn op_checked_mul_overflow_works() {
        let a = FixedU256::max_value();
        let b = FixedU256::try_from(2).unwrap().fixed().unwrap();
        assert!(a.checked_mul(&b).is_none());
    }

    #[test]
    fn op_mul_works() {
        let a = FixedU256::saturating_from_integer(42);
        let b = FixedU256::saturating_from_integer(2);
        assert_eq!(FixedU256::saturating_from_integer(84), a * b);

        let a = FixedU256::saturating_from_integer(42);
        let b = FixedU256::saturating_from_integer(-2);
        assert_eq!(FixedU256::saturating_from_integer(-84), a * b);
    }

    #[test]
    #[should_panic(expected = "attempt to divide by zero")]
    fn op_div_panics_on_zero_divisor() {
        let a = FixedU256::saturating_from_integer(1);
        let b: FixedU256 = 0.try_into().unwrap();
        let _c = a / b;
    }

    // #[test]
    // fn op_checked_div_overflow_works() {
    //     if FixedU256::SIGNED {
    //         let a = FixedU256::min_value();
    //         let b = FixedU256::zero().saturating_sub(FixedU256::one());
    //         assert!(a.checked_div(&b).is_none());
    //     }
    // }

    #[test]
    fn op_sqrt_works() {
        for i in 1..1_000i64 {
            let x = FixedU256::saturating_from_rational(i, 1_000i64);
            assert_eq!((x * x).try_sqrt(), Some(x));
            let x = FixedU256::saturating_from_rational(i, 1i64);
            assert_eq!((x * x).try_sqrt(), Some(x));
        }
    }

    #[test]
    fn op_div_works() {
        let a = FixedU256::saturating_from_integer(42);
        let b = FixedU256::saturating_from_integer(2);
        assert_eq!(FixedU256::saturating_from_integer(21), a / b);

        // if FixedU256::SIGNED {
        //     let a = FixedU256::saturating_from_integer(42);
        //     let b = FixedU256::saturating_from_integer(-2);
        //     assert_eq!(FixedU256::saturating_from_integer(-21), a / b);
        // }
    }

    #[test]
    fn saturating_from_integer_works() {
        let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
        let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();
        let accuracy = FixedU256::accuracy();

        // Cases where integer fits.
        let a = FixedU256::saturating_from_integer(42);
        let a_256 = U256::from(42);
        assert_eq!(a.into_inner(), a_256 * accuracy);

        let a = FixedU256::saturating_from_integer(-42);
        assert_eq!(
            a.into_inner(),
            U256::zero().saturating_sub(a_256 * accuracy)
        );

        // Max/min integers that fit.
        let a = FixedU256::saturating_from_integer(inner_max / accuracy);
        assert_eq!(a.into_inner(), (inner_max / accuracy) * accuracy);

        let a = FixedU256::saturating_from_integer(inner_min / accuracy);
        assert_eq!(a.into_inner(), (inner_min / accuracy) * accuracy);

        // Cases where integer doesn't fit, so it saturates.
        let a = FixedU256::saturating_from_integer(inner_max / accuracy + 1);
        assert_eq!(a.into_inner(), inner_max);

        let a =
            FixedU256::saturating_from_integer((inner_min / accuracy).saturating_sub(U256::one()));
        assert_eq!(a.into_inner(), inner_min);
    }

    #[test]
    fn checked_from_integer_works() {
        let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
        // let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();
        let accuracy = FixedU256::accuracy();

        // Case where integer fits.
        let a = FixedU256::checked_from_integer(42).expect("42 * accuracy <= inner_max; qed");
        assert_eq!(a.into_inner(), U256::from(42) * accuracy);

        // Max integer that fit.
        let a = FixedU256::checked_from_integer(inner_max / accuracy)
            .expect("(inner_max / accuracy) * accuracy <= inner_max; qed");
        assert_eq!(a.into_inner(), (inner_max / accuracy) * accuracy);

        // Case where integer doesn't fit, so it returns `None`.
        let a = FixedU256::checked_from_integer(inner_max / accuracy + 1);
        assert_eq!(a, None);

        // if FixedU256::SIGNED {
        //     // Case where integer fits.
        //     let a = FixedU256::checked_from_integer::<U256>(U256::zero().saturating_sub(42.unique_saturated_into()))
        //         .expect("-42 * accuracy >= inner_min; qed");
        //     assert_eq!(a.into_inner(), 0 - 42 * accuracy);
        //
        //     // Min integer that fit.
        //     let a = FixedU256::checked_from_integer::<U256>(inner_min / accuracy)
        //         .expect("(inner_min / accuracy) * accuracy <= inner_min; qed");
        //     assert_eq!(a.into_inner(), (inner_min / accuracy) * accuracy);
        //
        //     // Case where integer doesn't fit, so it returns `None`.
        //     let a = FixedU256::checked_from_integer::<U256>(inner_min / accuracy - 1);
        //     assert_eq!(a, None);
        // }
    }

    #[test]
    fn from_inner_works() {
        let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
        let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();

        assert_eq!(max(), FixedU256::from_inner(inner_max));
        assert_eq!(min(), FixedU256::from_inner(inner_min));
    }

    #[test]
    #[should_panic(expected = "attempt to divide by zero")]
    fn saturating_from_rational_panics_on_zero_divisor() {
        let _ = FixedU256::saturating_from_rational(1, 0);
    }

    #[test]
    fn saturating_from_rational_works() {
        let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
        let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();
        let accuracy = FixedU256::accuracy();

        let a = FixedU256::saturating_from_rational(5, 2);

        // Positive case: 2.5
        assert_eq!(a.into_inner(), U256::from(25) * accuracy / 10);

        // Max - 1.
        let a = FixedU256::saturating_from_rational(inner_max - 1, accuracy);
        assert_eq!(a.into_inner(), inner_max - 1);

        // Min + 1.
        let a = FixedU256::saturating_from_rational(inner_min + 1, accuracy);
        assert_eq!(a.into_inner(), inner_min + 1);

        // Max.
        let a = FixedU256::saturating_from_rational(inner_max, accuracy);
        assert_eq!(a.into_inner(), inner_max);

        // Min.
        let a = FixedU256::saturating_from_rational(inner_min, accuracy);
        assert_eq!(a.into_inner(), inner_min);

        // Zero.
        let a = FixedU256::saturating_from_rational(0, 1);
        assert_eq!(a.into_inner(), U256::zero());

        // if FixedU256::SIGNED {
        //     // Negative case: -2.5
        //     let a = FixedU256::saturating_from_rational(-5, 2);
        //     assert_eq!(a.into_inner(), 0 - 25.unique_saturated_into() * accuracy / 10);
        //
        //     // Other negative case: -2.5
        //     let a = FixedU256::saturating_from_rational(5, -2);
        //     assert_eq!(a.into_inner(), 0 - 25.unique_saturated_into() * accuracy / 10);
        //
        //     // Other positive case: 2.5
        //     let a = FixedU256::saturating_from_rational(-5, -2);
        //     assert_eq!(a.into_inner(), 25.unique_saturated_into() * accuracy / 10);
        //
        //     // Max + 1, saturates.
        //     let a = FixedU256::saturating_from_rational(inner_max as U256 + 1, accuracy);
        //     assert_eq!(a.into_inner(), inner_max);
        //
        //     // Min - 1, saturates.
        //     let a = FixedU256::saturating_from_rational(inner_max as U256 + 2, 0 - accuracy);
        //     assert_eq!(a.into_inner(), inner_min);
        //
        //     let a = FixedU256::saturating_from_rational(inner_max, 0 - accuracy);
        //     assert_eq!(a.into_inner(), 0 - inner_max);
        //
        //     let a = FixedU256::saturating_from_rational(inner_min, 0 - accuracy);
        //     assert_eq!(a.into_inner(), inner_max);
        //
        //     let a = FixedU256::saturating_from_rational(inner_min + 1, 0 - accuracy);
        //     assert_eq!(a.into_inner(), inner_max);
        //
        //     let a = FixedU256::saturating_from_rational(inner_min, 0 - 1);
        //     assert_eq!(a.into_inner(), inner_max);
        //
        //     let a = FixedU256::saturating_from_rational(inner_max, 0 - 1);
        //     assert_eq!(a.into_inner(), inner_min);
        //
        //     let a = FixedU256::saturating_from_rational(inner_max, 0 - inner_max);
        //     assert_eq!(a.into_inner(), 0 - accuracy);
        //
        //     let a = FixedU256::saturating_from_rational(0 - inner_max, inner_max);
        //     assert_eq!(a.into_inner(), 0 - accuracy);
        //
        //     let a = FixedU256::saturating_from_rational(inner_max, 0 - 3 * accuracy);
        //     assert_eq!(a.into_inner(), 0 - inner_max / 3);
        //
        //     let a = FixedU256::saturating_from_rational(inner_min, 0 - accuracy / 3);
        //     assert_eq!(a.into_inner(), inner_max);
        //
        //     let a = FixedU256::saturating_from_rational(1, 0 - accuracy);
        //     assert_eq!(a.into_inner(), 0.saturating_sub(1));
        //
        //     let a = FixedU256::saturating_from_rational(inner_min, inner_min);
        //     assert_eq!(a.into_inner(), accuracy);
        //
        //     // Out of accuracy.
        //     let a = FixedU256::saturating_from_rational(1, 0 - accuracy - 1);
        //     assert_eq!(a.into_inner(), 0);
        // }

        let a = FixedU256::saturating_from_rational(inner_max - 1, accuracy);
        assert_eq!(a.into_inner(), inner_max - 1);

        let a = FixedU256::saturating_from_rational(inner_min + 1, accuracy);
        assert_eq!(a.into_inner(), inner_min + 1);

        let a = FixedU256::saturating_from_rational(inner_max, 1);
        assert_eq!(a.into_inner(), inner_max);

        let a = FixedU256::saturating_from_rational(inner_min, 1);
        assert_eq!(a.into_inner(), inner_min);

        let a = FixedU256::saturating_from_rational(inner_max, inner_max);
        assert_eq!(a.into_inner(), accuracy);

        let a = FixedU256::saturating_from_rational(inner_max, U256::from(3) * accuracy);
        assert_eq!(a.into_inner(), inner_max / 3);

        let a = FixedU256::saturating_from_rational(inner_min, U256::from(2) * accuracy);
        assert_eq!(a.into_inner(), inner_min / 2);

        let a = FixedU256::saturating_from_rational(inner_min, accuracy / 3);
        assert_eq!(a.into_inner(), inner_min);

        let a = FixedU256::saturating_from_rational(1, accuracy);
        assert_eq!(a.into_inner(), U256::one());

        // Out of accuracy.
        let a = FixedU256::saturating_from_rational(1, accuracy + 1);
        assert_eq!(a.into_inner(), U256::zero());
    }

    #[test]
    fn checked_from_rational_works() {
        let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
        let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();
        let accuracy = FixedU256::accuracy();

        // Divide by zero => None.
        let a = FixedU256::checked_from_rational(1, 0);
        assert_eq!(a, None);

        // Max - 1.
        let a = FixedU256::checked_from_rational(inner_max - 1, accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_max - 1);

        // Min + 1.
        let a = FixedU256::checked_from_rational(inner_min + 1, accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_min + 1);

        // Max.
        let a = FixedU256::checked_from_rational(inner_max, accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_max);

        // Min.
        let a = FixedU256::checked_from_rational(inner_min, accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_min);

        // Max + 1 => Overflow => None.
        let a = FixedU256::checked_from_rational(inner_min, U256::zero().saturating_sub(accuracy));
        assert_eq!(a, None);

        // if FixedU256::SIGNED {
        //     // Min - 1 => Underflow => None.
        //     let a = FixedU256::checked_from_rational(
        //         inner_max as U256 + 2,
        //         0.saturating_sub(accuracy),
        //     );
        //     assert_eq!(a, None);
        //
        //     let a = FixedU256::checked_from_rational(inner_max, 0 - 3 * accuracy).unwrap();
        //     assert_eq!(a.into_inner(), 0 - inner_max / 3);
        //
        //     let a = FixedU256::checked_from_rational(inner_min, 0 - accuracy / 3);
        //     assert_eq!(a, None);
        //
        //     let a = FixedU256::checked_from_rational(1, 0 - accuracy).unwrap();
        //     assert_eq!(a.into_inner(), 0.saturating_sub(1));
        //
        //     let a = FixedU256::checked_from_rational(1, 0 - accuracy - 1).unwrap();
        //     assert_eq!(a.into_inner(), 0);
        //
        //     let a = FixedU256::checked_from_rational(inner_min, accuracy / 3);
        //     assert_eq!(a, None);
        // }

        let a = FixedU256::checked_from_rational(inner_max, U256::from(3) * accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_max / 3);

        let a = FixedU256::checked_from_rational(inner_min, U256::from(2) * accuracy).unwrap();
        assert_eq!(a.into_inner(), inner_min / 2);

        let a = FixedU256::checked_from_rational(1, accuracy).unwrap();
        assert_eq!(a.into_inner(), U256::one());

        let a = FixedU256::checked_from_rational(1, accuracy + 1).unwrap();
        assert_eq!(a.into_inner(), U256::zero());
    }

    #[test]
    fn from_rational_works() {
        let inner_max: U256 = <FixedU256 as FixedPointNumber>::Inner::max_value();
        let inner_min: U256 = U256::zero();
        let accuracy: U256 = FixedU256::accuracy();

        // Max - 1.
        let a = FixedU256::from_rational(inner_max - 1, accuracy);
        assert_eq!(a.into_inner(), inner_max - 1);

        // Min + 1.
        let a = FixedU256::from_rational(inner_min + 1, accuracy);
        assert_eq!(a.into_inner(), inner_min + 1);

        // Max.
        let a = FixedU256::from_rational(inner_max, accuracy);
        assert_eq!(a.into_inner(), inner_max);

        // Min.
        let a = FixedU256::from_rational(inner_min, accuracy);
        assert_eq!(a.into_inner(), inner_min);

        let a = FixedU256::from_rational(inner_max, U256::from(3) * accuracy);
        assert_eq!(a.into_inner(), inner_max / 3);

        let a = FixedU256::from_rational(U256::one(), accuracy);
        assert_eq!(a.into_inner(), U256::one());

        let a = FixedU256::from_rational(U256::one(), accuracy + 1);
        assert_eq!(a.into_inner(), U256::one());

        let a = FixedU256::from_rational_with_rounding(U256::one(), accuracy + 1, Rounding::Down);
        assert_eq!(a.into_inner(), U256::zero());
    }

    #[test]
    fn checked_mul_int_works() {
        let a = FixedU256::saturating_from_integer(2);
        // Max - 1.
        assert_eq!(a.checked_mul_int((i128::MAX - 1) / 2), Some(i128::MAX - 1));
        // Max.
        assert_eq!(a.checked_mul_int(i128::MAX / 2), Some(i128::MAX - 1));
        // Max + 1 => None.
        assert_eq!(a.checked_mul_int(i128::MAX / 2 + 1), None);

        // if FixedU256::SIGNED {
        //     // Min - 1.
        //     assert_eq!(a.checked_mul_int((i128::MIN + 1) / 2), Some(i128::MIN + 2));
        //     // Min.
        //     assert_eq!(a.checked_mul_int(i128::MIN / 2), Some(i128::MIN));
        //     // Min + 1 => None.
        //     assert_eq!(a.checked_mul_int(i128::MIN / 2 - 1), None);
        //
        //     let b = FixedU256::saturating_from_rational(1, -2);
        //     assert_eq!(b.checked_mul_int(42i128), Some(-21));
        //     assert_eq!(b.checked_mul_int(u128::MAX), None);
        //     assert_eq!(b.checked_mul_int(i128::MAX), Some(i128::MAX / -2));
        //     assert_eq!(b.checked_mul_int(i128::MIN), Some(i128::MIN / -2));
        // }

        let a = FixedU256::saturating_from_rational(1, 2);
        assert_eq!(a.checked_mul_int(42i128), Some(21));
        assert_eq!(a.checked_mul_int(i128::MAX), Some(i128::MAX / 2));
        assert_eq!(a.checked_mul_int(i128::MIN), Some(i128::MIN / 2));

        let c = FixedU256::saturating_from_integer(255);
        assert_eq!(c.checked_mul_int(2i8), None);
        assert_eq!(c.checked_mul_int(2i128), Some(510));
        assert_eq!(c.checked_mul_int(i128::MAX), None);
        assert_eq!(c.checked_mul_int(i128::MIN), None);
    }

    #[test]
    fn saturating_mul_int_works() {
        let a = FixedU256::saturating_from_integer(2);
        // Max - 1.
        assert_eq!(a.saturating_mul_int((i128::MAX - 1) / 2), i128::MAX - 1);
        // Max.
        assert_eq!(a.saturating_mul_int(i128::MAX / 2), i128::MAX - 1);
        // Max + 1 => saturates to max.
        assert_eq!(a.saturating_mul_int(i128::MAX / 2 + 1), i128::MAX);

        // Min - 1.
        assert_eq!(a.saturating_mul_int((i128::MIN + 1) / 2), i128::MIN + 2);
        // Min.
        assert_eq!(a.saturating_mul_int(i128::MIN / 2), i128::MIN);
        // Min + 1 => saturates to min.
        assert_eq!(a.saturating_mul_int(i128::MIN / 2 - 1), i128::MIN);

        // if FixedU256::SIGNED {
        //     let b = FixedU256::saturating_from_rational(1, -2);
        //     assert_eq!(b.saturating_mul_int(42i32), -21);
        //     assert_eq!(b.saturating_mul_int(i128::MAX), i128::MAX / -2);
        //     assert_eq!(b.saturating_mul_int(i128::MIN), i128::MIN / -2);
        //     assert_eq!(b.saturating_mul_int(u128::MAX), u128::MIN);
        // }

        let a = FixedU256::saturating_from_rational(1, 2);
        assert_eq!(a.saturating_mul_int(42i32), 21);
        assert_eq!(a.saturating_mul_int(i128::MAX), i128::MAX / 2);
        assert_eq!(a.saturating_mul_int(i128::MIN), i128::MIN / 2);

        let c = FixedU256::saturating_from_integer(255);
        assert_eq!(c.saturating_mul_int(2i8), i8::MAX);
        assert_eq!(c.saturating_mul_int(-2i8), i8::MIN);
        assert_eq!(c.saturating_mul_int(i128::MAX), i128::MAX);
        assert_eq!(c.saturating_mul_int(i128::MIN), i128::MIN);
    }

    #[test]
    fn checked_mul_works() {
        let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
        // let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();

        let a = FixedU256::saturating_from_integer(2);

        // Max - 1.
        let b = FixedU256::from_inner(inner_max - 1);

        assert_eq!(
            a.checked_mul(&(b / FixedU256::try_from(2).unwrap().fixed().unwrap())),
            Some(b)
        );

        // Max.
        let c = FixedU256::from_inner(inner_max);
        assert_eq!(
            a.checked_mul(&(c / FixedU256::try_from(2).unwrap().fixed().unwrap())),
            Some(b)
        );

        // Max + 1 => None.
        let e = FixedU256::from_inner(U256::one());
        assert_eq!(
            a.checked_mul(&(c / FixedU256::try_from(2).unwrap().fixed().unwrap() + e)),
            None
        );

        // if FixedU256::SIGNED {
        //     // Min + 1.
        //     let b = FixedU256::from_inner(inner_min + 1) / 2.into();
        //     let c = FixedU256::from_inner(inner_min + 2);
        //     assert_eq!(a.checked_mul(&b), Some(c));
        //
        //     // Min.
        //     let b = FixedU256::from_inner(inner_min) / 2.into();
        //     let c = FixedU256::from_inner(inner_min);
        //     assert_eq!(a.checked_mul(&b), Some(c));
        //
        //     // Min - 1 => None.
        //     let b = FixedU256::from_inner(inner_min) / 2.into() - FixedU256::from_inner(U256::one());
        //     assert_eq!(a.checked_mul(&b), None);
        //
        //     let c = FixedU256::saturating_from_integer(255);
        //     let b = FixedU256::saturating_from_rational(1, -2);
        //
        //     assert_eq!(b.checked_mul(&42.into()), Some(0.saturating_sub(21).into()));
        //     assert_eq!(
        //         b.checked_mul(&FixedU256::max_value()),
        //         FixedU256::max_value().checked_div(&0.saturating_sub(2).into())
        //     );
        //     assert_eq!(
        //         b.checked_mul(&FixedU256::min_value()),
        //         FixedU256::min_value().checked_div(&0.saturating_sub(2).into())
        //     );
        //     assert_eq!(c.checked_mul(&FixedU256::min_value()), None);
        // }

        let a = FixedU256::saturating_from_rational(1, 2);
        let c = FixedU256::saturating_from_integer(255);

        assert_eq!(
            a.checked_mul(&42.try_into().unwrap()),
            Some(21.try_into().unwrap())
        );
        assert_eq!(
            c.checked_mul(&2.try_into().unwrap()),
            Some(510.try_into().unwrap())
        );
        assert_eq!(c.checked_mul(&FixedU256::max_value()), None);
        assert_eq!(
            a.checked_mul(&FixedU256::max_value()),
            FixedU256::max_value().checked_div(&FixedU256::try_from(2).unwrap().fixed().unwrap())
        );
        assert_eq!(
            a.checked_mul(&FixedU256::min_value()),
            FixedU256::min_value().checked_div(&FixedU256::try_from(2).unwrap().fixed().unwrap())
        );
    }

    #[test]
    fn const_checked_mul_works() {
        let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
        // let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();

        let a = FixedU256::saturating_from_integer(2u32);

        // Max - 1.
        let b = FixedU256::from_inner(inner_max - 1);
        assert_eq!(
            a.const_checked_mul(b / FixedU256::try_from(2).unwrap().fixed().unwrap()),
            Some(b)
        );

        // Max.
        let c = FixedU256::from_inner(inner_max);
        assert_eq!(
            a.const_checked_mul(c / FixedU256::try_from(2).unwrap().fixed().unwrap()),
            Some(b)
        );

        // Max + 1 => None.
        let e = FixedU256::from_inner(U256::one());
        assert_eq!(
            a.const_checked_mul(c / FixedU256::try_from(2).unwrap().fixed().unwrap() + e),
            None
        );

        // if FixedU256::SIGNED {
        //     // Min + 1.
        //     let b = FixedU256::from_inner(inner_min + 1) / 2.into();
        //     let c = FixedU256::from_inner(inner_min + 2);
        //     assert_eq!(a.const_checked_mul(b), Some(c));
        //
        //     // Min.
        //     let b = FixedU256::from_inner(inner_min) / 2.into();
        //     let c = FixedU256::from_inner(inner_min);
        //     assert_eq!(a.const_checked_mul(b), Some(c));
        //
        //     // Min - 1 => None.
        //     let b = FixedU256::from_inner(inner_min) / 2.into() - FixedU256::from_inner(1);
        //     assert_eq!(a.const_checked_mul(b), None);
        //
        //     let b = FixedU256::saturating_from_rational(1i32, -2i32);
        //     let c = FixedU256::saturating_from_integer(-21i32);
        //     let d = FixedU256::saturating_from_integer(42);
        //
        //     assert_eq!(b.const_checked_mul(d), Some(c));
        //
        //     let minus_two = FixedU256::saturating_from_integer(-2i32);
        //     assert_eq!(
        //         b.const_checked_mul(FixedU256::max_value()),
        //         FixedU256::max_value().const_checked_div(minus_two)
        //     );
        //     assert_eq!(
        //         b.const_checked_mul(FixedU256::min_value()),
        //         FixedU256::min_value().const_checked_div(minus_two)
        //     );
        //
        //     let c = FixedU256::saturating_from_integer(255u32);
        //     assert_eq!(c.const_checked_mul(FixedU256::min_value()), None);
        // }

        let a = FixedU256::saturating_from_rational(1i32, 2i32);
        let c = FixedU256::saturating_from_integer(255i32);

        assert_eq!(
            a.const_checked_mul(42.try_into().unwrap()),
            Some(21.try_into().unwrap())
        );
        assert_eq!(
            c.const_checked_mul(2.try_into().unwrap()),
            Some(510.try_into().unwrap())
        );
        assert_eq!(c.const_checked_mul(FixedU256::max_value()), None);
        assert_eq!(
            a.const_checked_mul(FixedU256::max_value()),
            FixedU256::max_value().checked_div(&FixedU256::try_from(2).unwrap().fixed().unwrap())
        );
        assert_eq!(
            a.const_checked_mul(FixedU256::min_value()),
            FixedU256::min_value().const_checked_div(FixedU256::saturating_from_integer(2))
        );
    }

    #[test]
    fn checked_div_int_works() {
        let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
        let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();
        let accuracy = FixedU256::accuracy();

        let a = FixedU256::from_inner(inner_max);
        let b = FixedU256::from_inner(inner_min);
        let c = FixedU256::zero();
        let d = FixedU256::one();
        let e = FixedU256::saturating_from_integer(6);
        let f = FixedU256::saturating_from_integer(5);

        assert_eq!(e.checked_div_int(2), Some(3));
        assert_eq!(f.checked_div_int(2), Some(2));

        assert_eq!(a.checked_div_int(U256::MAX), Some(U256::zero()));
        assert_eq!(
            a.checked_div_int(U256::from(2)),
            Some(inner_max / (U256::from(2) * accuracy))
        );
        assert_eq!(a.checked_div_int(inner_max / accuracy), Some(U256::from(1)));
        assert_eq!(a.checked_div_int(1i8), None);

        if b < c {
            // Not executed by unsigned inners.
            assert_eq!(
                a.checked_div_int(U256::zero().saturating_sub(U256::from(2))),
                Some(U256::zero().saturating_sub(inner_max / (U256::from(2) * accuracy)))
            );
            assert_eq!(
                a.checked_div_int(U256::zero().saturating_sub(inner_max / accuracy)),
                Some(U256::zero().saturating_sub(U256::from(1)))
            );
            assert_eq!(b.checked_div_int(U256::min_value()), Some(U256::zero()));
            assert_eq!(b.checked_div_int(inner_min / accuracy), Some(U256::from(1)));
            assert_eq!(b.checked_div_int(U256::from(1)), None);
            assert_eq!(
                b.checked_div_int(U256::zero().saturating_sub(U256::from(2))),
                Some(U256::zero().saturating_sub(inner_min / (U256::from(2) * accuracy)))
            );
            assert_eq!(
                b.checked_div_int(U256::zero().saturating_sub(inner_min / accuracy)),
                Some(U256::zero().saturating_sub(U256::from(1)))
            );
            assert_eq!(c.checked_div_int(U256::min_value()), Some(U256::zero()));
            assert_eq!(d.checked_div_int(U256::min_value()), Some(U256::zero()));
        }

        assert_eq!(
            b.checked_div_int(U256::from(2)),
            Some(inner_min / (U256::from(2) * accuracy))
        );

        assert_eq!(c.checked_div_int(U256::from(1)), Some(U256::zero()));
        assert_eq!(c.checked_div_int(U256::MAX), Some(U256::zero()));
        assert_eq!(c.checked_div_int(U256::from(1)), Some(U256::zero()));

        assert_eq!(d.checked_div_int(U256::from(1)), Some(U256::from(1)));
        assert_eq!(d.checked_div_int(U256::MAX), Some(U256::zero()));
        assert_eq!(d.checked_div_int(U256::from(1)), Some(U256::from(1)));

        assert_eq!(a.checked_div_int(U256::zero()), None);
        assert_eq!(b.checked_div_int(U256::zero()), None);
        assert_eq!(c.checked_div_int(U256::zero()), None);
        assert_eq!(d.checked_div_int(U256::zero()), None);
    }

    #[test]
    #[should_panic(expected = "attempt to divide by zero")]
    fn saturating_div_int_panics_when_divisor_is_zero() {
        let _ = FixedU256::one().saturating_div_int(0);
    }

    #[test]
    fn saturating_div_int_works() {
        // let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
        let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();
        let accuracy = FixedU256::accuracy();

        let a = FixedU256::saturating_from_integer(5);
        assert_eq!(a.saturating_div_int(U256::from(2)), U256::from(2));

        let a = FixedU256::min_value();
        assert_eq!(a.saturating_div_int(U256::from(1)), inner_min / accuracy);

        // if FixedU256::SIGNED {
        //     let a = FixedU256::saturating_from_integer(5);
        //     assert_eq!(a.saturating_div_int(-2), -2);
        //
        //     let a = FixedU256::min_value();
        //     assert_eq!(a.saturating_div_int(-1i128), (inner_max / accuracy) as i128);
        // }
    }

    #[test]
    fn saturating_abs_works() {
        let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
        // let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();

        assert_eq!(
            FixedU256::from_inner(inner_max).saturating_abs(),
            FixedU256::max_value()
        );
        assert_eq!(FixedU256::zero().saturating_abs(), 0.try_into().unwrap());

        // if FixedU256::SIGNED {
        //     assert_eq!(FixedU256::from_inner(inner_min).saturating_abs(), FixedU256::max_value());
        //     assert_eq!(
        //         FixedU256::saturating_from_rational(-1, 2).saturating_abs(),
        //         (1, 2).into()
        //     );
        // }
    }

    // #[test]
    // fn saturating_mul_acc_int_works() {
    //     assert_eq!(FixedU256::zero().saturating_mul_acc_int(42i8), 42i8);
    //     assert_eq!(FixedU256::one().saturating_mul_acc_int(42i8), 2 * 42i8);
    //
    //     assert_eq!(FixedU256::one().saturating_mul_acc_int(i128::MAX), i128::MAX);
    //     assert_eq!(FixedU256::one().saturating_mul_acc_int(i128::MIN), i128::MIN);
    //
    //     assert_eq!(FixedU256::one().saturating_mul_acc_int(u128::MAX / 2), u128::MAX - 1);
    //     assert_eq!(FixedU256::one().saturating_mul_acc_int(u128::MIN), u128::MIN);
    //
    //     if FixedU256::SIGNED {
    //         let a = FixedU256::saturating_from_rational(-1, 2);
    //         assert_eq!(a.saturating_mul_acc_int(42i8), 21i8);
    //         assert_eq!(a.saturating_mul_acc_int(42u8), 21u8);
    //         assert_eq!(a.saturating_mul_acc_int(u128::MAX - 1), u128::MAX / 2);
    //     }
    // }
    //
    // #[test]
    // fn saturating_pow_should_work() {
    //     assert_eq!(
    //         FixedU256::saturating_from_integer(2).saturating_pow(0),
    //         FixedU256::saturating_from_integer(1)
    //     );
    //     assert_eq!(
    //         FixedU256::saturating_from_integer(2).saturating_pow(1),
    //         FixedU256::saturating_from_integer(2)
    //     );
    //     assert_eq!(
    //         FixedU256::saturating_from_integer(2).saturating_pow(2),
    //         FixedU256::saturating_from_integer(4)
    //     );
    //     assert_eq!(
    //         FixedU256::saturating_from_integer(2).saturating_pow(3),
    //         FixedU256::saturating_from_integer(8)
    //     );
    //     assert_eq!(
    //         FixedU256::saturating_from_integer(2).saturating_pow(50),
    //         FixedU256::saturating_from_integer(1125899906842624i64)
    //     );
    //
    //     assert_eq!(FixedU256::saturating_from_integer(1).saturating_pow(1000), (1).into());
    //     assert_eq!(
    //         FixedU256::saturating_from_integer(1).saturating_pow(usize::MAX),
    //         (1).into()
    //     );
    //
    //     if FixedU256::SIGNED {
    //         // Saturating.
    //         assert_eq!(
    //             FixedU256::saturating_from_integer(2).saturating_pow(68),
    //             FixedU256::max_value()
    //         );
    //
    //         assert_eq!(FixedU256::saturating_from_integer(-1).saturating_pow(1000), (1).into());
    //         assert_eq!(
    //             FixedU256::saturating_from_integer(-1).saturating_pow(1001),
    //             0.saturating_sub(1).into()
    //         );
    //         assert_eq!(
    //             FixedU256::saturating_from_integer(-1).saturating_pow(usize::MAX),
    //             0.saturating_sub(1).into()
    //         );
    //         assert_eq!(
    //             FixedU256::saturating_from_integer(-1).saturating_pow(usize::MAX - 1),
    //             (1).into()
    //         );
    //     }
    //
    //     assert_eq!(
    //         FixedU256::saturating_from_integer(114209).saturating_pow(5),
    //         FixedU256::max_value()
    //     );
    //
    //     assert_eq!(
    //         FixedU256::saturating_from_integer(1).saturating_pow(usize::MAX),
    //         (1).into()
    //     );
    //     assert_eq!(
    //         FixedU256::saturating_from_integer(0).saturating_pow(usize::MAX),
    //         (0).into()
    //     );
    //     assert_eq!(
    //         FixedU256::saturating_from_integer(2).saturating_pow(usize::MAX),
    //         FixedU256::max_value()
    //     );
    // }
    //
    // #[test]
    // fn checked_div_works() {
    //     let inner_max = <FixedU256 as FixedPointNumber>::Inner::max_value();
    //     let inner_min = <FixedU256 as FixedPointNumber>::Inner::min_value();
    //
    //     let a = FixedU256::from_inner(inner_max);
    //     let b = FixedU256::from_inner(inner_min);
    //     let c = FixedU256::zero();
    //     let d = FixedU256::one();
    //     let e = FixedU256::saturating_from_integer(6);
    //     let f = FixedU256::saturating_from_integer(5);
    //
    //     assert_eq!(e.checked_div(&2.into()), Some(3.into()));
    //     assert_eq!(f.checked_div(&2.into()), Some((5, 2).into()));
    //
    //     assert_eq!(a.checked_div(&inner_max.into()), Some(1.into()));
    //     assert_eq!(a.checked_div(&2.into()), Some(FixedU256::from_inner(inner_max / 2)));
    //     assert_eq!(a.checked_div(&FixedU256::max_value()), Some(1.into()));
    //     assert_eq!(a.checked_div(&d), Some(a));
    //
    //     if b < c {
    //         // Not executed by unsigned inners.
    //         assert_eq!(
    //             a.checked_div(&0.saturating_sub(2).into()),
    //             Some(FixedU256::from_inner(0.saturating_sub(inner_max / 2)))
    //         );
    //         assert_eq!(
    //             a.checked_div(&-FixedU256::max_value()),
    //             Some(0.saturating_sub(1).into())
    //         );
    //         assert_eq!(
    //             b.checked_div(&0.saturating_sub(2).into()),
    //             Some(FixedU256::from_inner(0.saturating_sub(inner_min / 2)))
    //         );
    //         assert_eq!(c.checked_div(&FixedU256::max_value()), Some(0.into()));
    //         assert_eq!(b.checked_div(&b), Some(FixedU256::one()));
    //     }
    //
    //     assert_eq!(b.checked_div(&2.into()), Some(FixedU256::from_inner(inner_min / 2)));
    //     assert_eq!(b.checked_div(&a), Some(0.saturating_sub(1).into()));
    //     assert_eq!(c.checked_div(&1.into()), Some(0.into()));
    //     assert_eq!(d.checked_div(&1.into()), Some(1.into()));
    //
    //     assert_eq!(a.checked_div(&FixedU256::one()), Some(a));
    //     assert_eq!(b.checked_div(&FixedU256::one()), Some(b));
    //     assert_eq!(c.checked_div(&FixedU256::one()), Some(c));
    //     assert_eq!(d.checked_div(&FixedU256::one()), Some(d));
    //
    //     assert_eq!(a.checked_div(&FixedU256::zero()), None);
    //     assert_eq!(b.checked_div(&FixedU256::zero()), None);
    //     assert_eq!(c.checked_div(&FixedU256::zero()), None);
    //     assert_eq!(d.checked_div(&FixedU256::zero()), None);
    // }
    //
    // #[test]
    // fn is_positive_negative_works() {
    //     let one = FixedU256::one();
    //     assert!(one.is_positive());
    //     assert!(!one.is_negative());
    //
    //     let zero = FixedU256::zero();
    //     assert!(!zero.is_positive());
    //     assert!(!zero.is_negative());
    //
    //     if $signed {
    //         let minus_one = FixedU256::saturating_from_integer(-1);
    //         assert!(minus_one.is_negative());
    //         assert!(!minus_one.is_positive());
    //     }
    // }
    //
    // #[test]
    // fn trunc_works() {
    //     let n = FixedU256::saturating_from_rational(5, 2).trunc();
    //     assert_eq!(n, FixedU256::saturating_from_integer(2));
    //
    //     if FixedU256::SIGNED {
    //         let n = FixedU256::saturating_from_rational(-5, 2).trunc();
    //         assert_eq!(n, FixedU256::saturating_from_integer(-2));
    //     }
    // }
    //
    // #[test]
    // fn frac_works() {
    //     let n = FixedU256::saturating_from_rational(5, 2);
    //     let i = n.trunc();
    //     let f = n.frac();
    //
    //     assert_eq!(n, i + f);
    //
    //     let n = FixedU256::saturating_from_rational(5, 2).frac().saturating_mul(10.into());
    //     assert_eq!(n, 5.into());
    //
    //     let n = FixedU256::saturating_from_rational(1, 2).frac().saturating_mul(10.into());
    //     assert_eq!(n, 5.into());
    //
    //     if FixedU256::SIGNED {
    //         let n = FixedU256::saturating_from_rational(-5, 2);
    //         let i = n.trunc();
    //         let f = n.frac();
    //         assert_eq!(n, i - f);
    //
    //         // The sign is attached to the integer part unless it is zero.
    //         let n = FixedU256::saturating_from_rational(-5, 2).frac().saturating_mul(10.into());
    //         assert_eq!(n, 5.into());
    //
    //         let n = FixedU256::saturating_from_rational(-1, 2).frac().saturating_mul(10.into());
    //         assert_eq!(n, 0.saturating_sub(5).into());
    //     }
    // }
    //
    // #[test]
    // fn ceil_works() {
    //     let n = FixedU256::saturating_from_rational(5, 2);
    //     assert_eq!(n.ceil(), 3.into());
    //
    //     let n = FixedU256::saturating_from_rational(-5, 2);
    //     assert_eq!(n.ceil(), 0.saturating_sub(2).into());
    //
    //     // On the limits:
    //     let n = FixedU256::max_value();
    //     assert_eq!(n.ceil(), n.trunc());
    //
    //     let n = FixedU256::min_value();
    //     assert_eq!(n.ceil(), n.trunc());
    // }
    //
    // #[test]
    // fn floor_works() {
    //     let n = FixedU256::saturating_from_rational(5, 2);
    //     assert_eq!(n.floor(), 2.into());
    //
    //     let n = FixedU256::saturating_from_rational(-5, 2);
    //     assert_eq!(n.floor(), 0.saturating_sub(3).into());
    //
    //     // On the limits:
    //     let n = FixedU256::max_value();
    //     assert_eq!(n.floor(), n.trunc());
    //
    //     let n = FixedU256::min_value();
    //     assert_eq!(n.floor(), n.trunc());
    // }
    //
    // #[test]
    // fn round_works() {
    //     let n = FixedU256::zero();
    //     assert_eq!(n.round(), n);
    //
    //     let n = FixedU256::one();
    //     assert_eq!(n.round(), n);
    //
    //     let n = FixedU256::saturating_from_rational(5, 2);
    //     assert_eq!(n.round(), 3.into());
    //
    //     let n = FixedU256::saturating_from_rational(-5, 2);
    //     assert_eq!(n.round(), 0.saturating_sub(3).into());
    //
    //     // Saturating:
    //     let n = FixedU256::max_value();
    //     assert_eq!(n.round(), n.trunc());
    //
    //     let n = FixedU256::min_value();
    //     assert_eq!(n.round(), n.trunc());
    //
    //     // On the limit:
    //
    //     // floor(max - 1) + 0.33..
    //     let n = FixedU256::max_value()
    //         .saturating_sub(1.into())
    //         .trunc()
    //         .saturating_add((1, 3).into());
    //
    //     assert_eq!(n.round(), (FixedU256::max_value() - 1.into()).trunc());
    //
    //     // floor(max - 1) + 0.5
    //     let n = FixedU256::max_value()
    //         .saturating_sub(1.into())
    //         .trunc()
    //         .saturating_add((1, 2).into());
    //
    //     assert_eq!(n.round(), FixedU256::max_value().trunc());
    //
    //     if FixedU256::SIGNED {
    //         // floor(min + 1) - 0.33..
    //         let n = FixedU256::min_value()
    //             .saturating_add(1.into())
    //             .trunc()
    //             .saturating_sub((1, 3).into());
    //
    //         assert_eq!(n.round(), (FixedU256::min_value() + 1.into()).trunc());
    //
    //         // floor(min + 1) - 0.5
    //         let n = FixedU256::min_value()
    //             .saturating_add(1.into())
    //             .trunc()
    //             .saturating_sub((1, 2).into());
    //
    //         assert_eq!(n.round(), FixedU256::min_value().trunc());
    //     }
    // }
    //
    // #[test]
    // fn perthing_into_works() {
    //     let ten_percent_percent: FixedU256 = Percent::from_percent(10).into();
    //     assert_eq!(ten_percent_percent.into_inner(), FixedU256::accuracy() / 10);
    //
    //     let ten_percent_permill: FixedU256 = Permill::from_percent(10).into();
    //     assert_eq!(ten_percent_permill.into_inner(), FixedU256::accuracy() / 10);
    //
    //     let ten_percent_perbill: FixedU256 = Perbill::from_percent(10).into();
    //     assert_eq!(ten_percent_perbill.into_inner(), FixedU256::accuracy() / 10);
    //
    //     let ten_percent_perquintill: FixedU256 = Perquintill::from_percent(10).into();
    //     assert_eq!(ten_percent_perquintill.into_inner(), FixedU256::accuracy() / 10);
    // }
    //
    // #[test]
    // fn fmt_should_work() {
    //     let zero = FixedU256::zero();
    //     assert_eq!(
    //         format!("{:?}", zero),
    //         format!("{}(0.{:0>weight$})", stringify!(FixedU256), 0, weight = precision())
    //     );
    //
    //     let one = FixedU256::one();
    //     assert_eq!(
    //         format!("{:?}", one),
    //         format!("{}(1.{:0>weight$})", stringify!(FixedU256), 0, weight = precision())
    //     );
    //
    //     let frac = FixedU256::saturating_from_rational(1, 2);
    //     assert_eq!(
    //         format!("{:?}", frac),
    //         format!("{}(0.{:0<weight$})", stringify!(FixedU256), 5, weight = precision())
    //     );
    //
    //     let frac = FixedU256::saturating_from_rational(5, 2);
    //     assert_eq!(
    //         format!("{:?}", frac),
    //         format!("{}(2.{:0<weight$})", stringify!(FixedU256), 5, weight = precision())
    //     );
    //
    //     let frac = FixedU256::saturating_from_rational(314, 100);
    //     assert_eq!(
    //         format!("{:?}", frac),
    //         format!("{}(3.{:0<weight$})", stringify!(FixedU256), 14, weight = precision())
    //     );
    //
    //     if FixedU256::SIGNED {
    //         let neg = -FixedU256::one();
    //         assert_eq!(
    //             format!("{:?}", neg),
    //             format!("{}(-1.{:0>weight$})", stringify!(FixedU256), 0, weight = precision())
    //         );
    //
    //         let frac = FixedU256::saturating_from_rational(-314, 100);
    //         assert_eq!(
    //             format!("{:?}", frac),
    //             format!("{}(-3.{:0<weight$})", stringify!(FixedU256), 14, weight = precision())
    //         );
    //     }
    // }
}
