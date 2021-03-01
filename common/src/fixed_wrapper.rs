use core::convert::TryInto;
use core::ops::*;
use core::result::Result;

use fixnum::ops::{CheckedAdd, CheckedSub, RoundMode::*, RoundingDiv, RoundingMul};
use fixnum::ArithmeticError;
use static_assertions::_core::cmp::Ordering;

use crate::{fixed, pow, Balance, Fixed, FixedInner, FIXED_PRECISION};

/// A convenient wrapper around `Fixed` type for safe math.
///
/// Supported operations: `+`, '-', '/', '*', 'sqrt'.
#[cfg_attr(feature = "std", derive(Debug))]
#[derive(Clone)]
pub struct FixedWrapper {
    inner: Result<Fixed, ArithmeticError>,
}

impl FixedWrapper {
    /// Retrieve the result.
    pub fn get(self) -> Result<Fixed, ArithmeticError> {
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
        let mul_after = self.clone().sqrt_accurate() * lhs.clone().sqrt_accurate();
        if mul_after.inner.is_ok() {
            return mul_after;
        }
        FixedWrapper {
            inner: Err(ArithmeticError::Overflow),
        }
    }

    pub fn pow(&self, x: u32) -> Self {
        (0..x).fold(fixed!(1), |acc, _| acc * self.clone())
    }

    /// Calculates square root of self using [Babylonian method][babylonian].
    /// Precision is `1e-10`.
    /// [babylonian]: https://en.wikipedia.org/wiki/Methods_of_computing_square_roots#Babylonian_method
    pub fn sqrt_accurate(self) -> Self {
        fn eq_eps(left: Fixed, right: Fixed, eps: Fixed) -> bool {
            let delta = left.csub(right).unwrap();
            if delta < fixed!(0) {
                delta.cneg().unwrap() < eps
            } else {
                delta < eps
            }
        }

        fn half_sum(a: Fixed, b: Fixed) -> Fixed {
            a.cadd(b).unwrap().rdiv(2, Floor).unwrap()
        }

        let eps = fixed!(0.0000000001);
        #[cfg(feature = "std")]
        let initial_sqrt = self.sqrt().inner;
        #[cfg(not(feature = "std"))]
        let initial_sqrt = self.inner.clone().map(|x| x.rdiv(2, Floor).unwrap());
        let sqrt_opt = zip(&self.inner, &initial_sqrt).map(|(&s, &n_prev)| {
            let mut n_prev = n_prev;
            let mut n;
            loop {
                n = half_sum(n_prev, s.rdiv(n_prev, Floor).unwrap());
                if eq_eps(n.rmul(n, Floor).unwrap(), s, eps) {
                    break;
                }
                n_prev = n;
            }
            n
        });
        Self::from(sqrt_opt)
    }

    /// Calculates square root of self using fractional representation.
    #[cfg(feature = "std")]
    pub fn sqrt(&self) -> Self {
        match self.to_fraction() {
            Err(_) => self.clone(),
            Ok(x) => Self::from(x.sqrt()),
        }
    }

    pub fn to_fraction(&self) -> Result<f64, ArithmeticError> {
        self.inner.clone().map(Fixed::to_f64)
    }

    pub fn try_into_balance(self) -> Result<Balance, ArithmeticError> {
        match self.inner {
            Ok(fixed) => fixed
                .into_bits()
                .try_into()
                .map_err(|_| ArithmeticError::Overflow),
            Err(e) => Err(e),
        }
    }

    /// For development it panics if cannot convert the inner value into Balance.
    /// For production it returns Balance saturated from 0
    pub fn into_balance(self) -> Balance {
        // TODO: Make it saturate
        self.inner.unwrap().into_bits().try_into().unwrap()
    }
}

impl From<Result<Fixed, ArithmeticError>> for FixedWrapper {
    fn from(result: Result<Fixed, ArithmeticError>) -> Self {
        FixedWrapper { inner: result }
    }
}

impl From<Fixed> for FixedWrapper {
    fn from(fixed: Fixed) -> Self {
        FixedWrapper::from(Ok(fixed))
    }
}

impl From<f64> for FixedWrapper {
    fn from(value: f64) -> Self {
        const COEF: f64 = pow(10, FIXED_PRECISION) as f64;
        let value = value * COEF;
        let result = if value.is_finite() {
            Ok(Fixed::from_bits(value as FixedInner))
        } else {
            Err(ArithmeticError::Overflow)
        };
        Self::from(result)
    }
}

macro_rules! impl_from_for_fixed_wrapper {
    ($( $T:ty ),+) => {
        $( impl_from_for_fixed_wrapper!(@single $T); )*
    };
    (@single $T:ty) => {
        impl From<$T> for FixedWrapper {
            fn from(value: $T) -> Self {
                match value.try_into() {
                    Ok(raw) => Self {
                        inner: Ok(Fixed::from_bits(raw)),
                    },
                    Err(_) => Self {
                        inner: Err(ArithmeticError::Overflow),
                    },
                }
            }
        }
    };
}

impl_from_for_fixed_wrapper!(usize, isize, u128, i128, u64, i64, u32, i32);

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
        impl $op for FixedWrapper {
            type Output = Self;

            fn $op_fn(self, rhs: Self) -> Self::Output {
                zip(&self.inner, &rhs.inner)
                    .and_then(|(lhs, &rhs)| lhs.$checked_op_fn(rhs))
                    .into()
            }
        }
    };
}

impl_op_for_fixed_wrapper!(Add, add, cadd);
impl_op_for_fixed_wrapper!(Sub, sub, csub);

macro_rules! impl_floor_op_for_fixed_wrapper {
    (
        $op:ty,
        $op_fn:ident,
        $checked_op_fn:ident
    ) => {
        impl $op for FixedWrapper {
            type Output = Self;

            fn $op_fn(self, rhs: Self) -> Self::Output {
                zip(&self.inner, &rhs.inner)
                    .and_then(|(lhs, &rhs)| lhs.$checked_op_fn(rhs, Floor))
                    .into()
            }
        }
    };
}

impl_floor_op_for_fixed_wrapper!(Mul, mul, rmul);
impl_floor_op_for_fixed_wrapper!(Div, div, rdiv);

impl PartialEq for FixedWrapper {
    fn eq(&self, other: &Self) -> bool {
        zip(&self.inner, &other.inner)
            .map(|(lhs, rhs)| lhs.eq(rhs))
            .unwrap_or(false)
    }
}

impl Neg for FixedWrapper {
    type Output = Self;

    fn neg(self) -> Self::Output {
        self.inner.and_then(|value| value.cneg()).into()
    }
}

impl PartialOrd for FixedWrapper {
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
        // left (FixedWrapper + $type)
        impl $op<$type> for FixedWrapper {
            type Output = Self;

            fn $op_fn(self, rhs: $type) -> Self::Output {
                if self.inner.is_err() {
                    return Err(ArithmeticError::Overflow).into();
                }
                let rhs: FixedWrapper = rhs.into();
                self.$op_fn(rhs)
            }
        }
        // right ($type + FixedWrapper)
        impl $op<FixedWrapper> for $type {
            type Output = FixedWrapper;

            fn $op_fn(self, rhs: FixedWrapper) -> Self::Output {
                if rhs.inner.is_err() {
                    return Err(ArithmeticError::Overflow).into();
                }
                let lhs: FixedWrapper = self.into();
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
impl_fixed_wrapper_for_type!(Fixed);
impl_fixed_wrapper_for_type!(u128);
