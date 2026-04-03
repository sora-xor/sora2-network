use crate::ArithmeticError;

pub(crate) mod sqrt;

use sqrt::Sqrt;

pub trait Zero {
    const ZERO: Self;
}

pub trait One {
    const ONE: Self;
}

pub trait Bounded {
    const MIN: Self;
    const MAX: Self;
}

pub trait CheckedAdd<Rhs = Self> {
    type Output;
    type Error;

    /// Checked addition. Returns `Err` on overflow.
    ///
    /// ```ignore
    /// use fixnum::{FixedPoint, typenum::U9, ops::CheckedAdd};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "0.1".parse()?;
    /// let b: Amount = "0.2".parse()?;
    /// let c: Amount = "0.3".parse()?;
    /// assert_eq!(a.cadd(b)?, c);
    /// # Ok(()) }
    /// ```
    fn cadd(self, rhs: Rhs) -> Result<Self::Output, Self::Error>;

    /// Saturating addition. Computes `self + rhs`, saturating at the numeric bounds
    /// ([`MIN`][MIN], [`MAX`][MAX]) instead of overflowing.
    ///
    /// ```ignore
    /// use fixnum::{FixedPoint, typenum::U9, ops::{Bounded, RoundMode::*, CheckedAdd}};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "1000.00002".parse()?;
    /// let b: Amount = "9222000000".parse()?;
    /// let c: Amount = "9222001000.00002".parse()?;
    /// // 1000.00002 + 9222000000 = 9222001000.00002
    /// assert_eq!(a.saturating_add(b), c);
    ///
    /// // 9222000000 + 9222000000 = MAX
    /// assert_eq!(c.saturating_add(c), Amount::MAX);
    ///
    /// let d: Amount = "-9222000000".parse()?;
    /// // -9222000000 + (-9222000000) = MIN
    /// assert_eq!(d.saturating_add(d), Amount::MIN);
    /// # Ok(()) }
    /// ```
    ///
    /// [MAX]: ./trait.Bounded.html#associatedconstant.MAX
    /// [MIN]: ./trait.Bounded.html#associatedconstant.MIN
    fn saturating_add(self, rhs: Rhs) -> Self::Output
    where
        Self: Sized,
        Rhs: PartialOrd + Zero,
        Self::Output: Bounded,
    {
        let is_rhs_negative = rhs < Rhs::ZERO;
        self.cadd(rhs).unwrap_or_else(|_| {
            if is_rhs_negative {
                Self::Output::MIN
            } else {
                Self::Output::MAX
            }
        })
    }
}

pub trait CheckedSub<Rhs = Self> {
    type Output;
    type Error;

    /// Checked subtraction. Returns `Err` on overflow.
    ///
    /// ```ignore
    /// use fixnum::{FixedPoint, typenum::U9, ops::CheckedSub};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "0.3".parse()?;
    /// let b: Amount = "0.1".parse()?;
    /// let c: Amount = "0.2".parse()?;
    /// assert_eq!(a.csub(b)?, c);
    /// # Ok(()) }
    /// ```
    fn csub(self, rhs: Rhs) -> Result<Self::Output, Self::Error>;

    /// Saturating subtraction. Computes `self - rhs`, saturating at the numeric bounds
    /// ([`MIN`][MIN], [`MAX`][MAX]) instead of overflowing.
    ///
    /// ```ignore
    /// use fixnum::{FixedPoint, typenum::U9, ops::{Bounded, RoundMode::*, CheckedSub}};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "9222001000.00002".parse()?;
    /// let b: Amount = "9222000000".parse()?;
    /// let c: Amount = "1000.00002".parse()?;
    /// // 9222001000.00002 - 9222000000 = 1000.00002
    /// assert_eq!(a.saturating_sub(b), c);
    ///
    /// let d: Amount = "-9222000000".parse()?;
    /// // 9222000000 - (-9222000000) = MAX
    /// assert_eq!(b.saturating_sub(d), Amount::MAX);
    ///
    /// // -9222000000 - 9222000000 = MIN
    /// assert_eq!(d.saturating_sub(b), Amount::MIN);
    /// # Ok(()) }
    /// ```
    ///
    /// [MAX]: ./trait.Bounded.html#associatedconstant.MAX
    /// [MIN]: ./trait.Bounded.html#associatedconstant.MIN
    fn saturating_sub(self, rhs: Rhs) -> Self::Output
    where
        Self: Sized,
        Rhs: PartialOrd + Zero,
        Self::Output: Bounded,
    {
        let is_rhs_negative = rhs < Rhs::ZERO;
        self.csub(rhs).unwrap_or_else(|_| {
            if is_rhs_negative {
                Self::Output::MAX
            } else {
                Self::Output::MIN
            }
        })
    }
}

pub trait CheckedMul<Rhs = Self> {
    type Output;
    type Error;

    /// Checked multiplication. Returns `Err` on overflow.
    /// This is multiplication without rounding, hence it's available only when at least one operand is integer.
    ///
    /// ```ignore
    /// use fixnum::{FixedPoint, typenum::U9, ops::CheckedMul};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "0.000000001".parse()?;
    /// let b: Amount = "0.000000012".parse()?;
    /// assert_eq!(a.cmul(12)?, b);
    /// assert_eq!(12.cmul(a)?, b);
    /// # Ok(()) }
    /// ```
    fn cmul(self, rhs: Rhs) -> Result<Self::Output, Self::Error>;

    /// Saturating multiplication. Computes `self * rhs`, saturating at the numeric bounds
    /// ([`MIN`][MIN], [`MAX`][MAX]) instead of overflowing.
    /// This is multiplication without rounding, hence it's available only when at least one operand is integer.
    ///
    /// ```ignore
    /// use fixnum::{FixedPoint, typenum::U9, ops::{Zero, Bounded, RoundMode::*, CheckedMul}};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "0.000000001".parse()?;
    /// let b: Amount = "0.000000012".parse()?;
    /// assert_eq!(a.saturating_mul(12), b);
    /// assert_eq!(12.saturating_mul(a), b);
    ///
    /// // i64::MAX * 1e-9 = MAX
    /// assert_eq!(a.saturating_mul(i64::MAX), Amount::MAX);
    ///
    /// let c: Amount = "-1.000000001".parse()?;
    /// // -1.000000001 * (SaturatingCeil) MAX = MIN
    /// assert_eq!(c.saturating_mul(i64::MAX), Amount::MIN);
    /// # Ok(()) }
    /// ```
    ///
    /// [FixedPoint]: ../struct.FixedPoint.html
    /// [MAX]: ./trait.Bounded.html#associatedconstant.MAX
    /// [MIN]: ./trait.Bounded.html#associatedconstant.MIN
    /// [RoundMode]: ./enum.RoundMode.html
    fn saturating_mul(self, rhs: Rhs) -> Self::Output
    where
        Self: PartialOrd + Zero + Sized,
        Rhs: PartialOrd + Zero,
        Self::Output: Bounded,
    {
        let is_lhs_negative = self < Self::ZERO;
        let is_rhs_negative = rhs < Rhs::ZERO;
        self.cmul(rhs).unwrap_or_else(|_| {
            if is_lhs_negative == is_rhs_negative {
                Self::Output::MAX
            } else {
                Self::Output::MIN
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RoundMode {
    Ceil = 1,
    Floor = -1,
}

pub trait RoundingMul<Rhs = Self> {
    type Output;
    type Error;

    /// Checked rounded multiplication. Returns `Err` on overflow.
    /// Because of provided [`RoundMode`][RoundMode] it's possible to perform across the [`FixedPoint`][FixedPoint]
    /// values.
    ///
    /// ```ignore
    /// use fixnum::{FixedPoint, typenum::U9, ops::{Zero, RoundingMul, RoundMode::*}};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "0.000000001".parse()?;
    /// let b: Amount = "0.000000002".parse()?;
    /// // 1e-9 * (Ceil) 2e-9 = 1e-9
    /// assert_eq!(a.rmul(b, Ceil)?, a);
    /// assert_eq!(b.rmul(a, Ceil)?, a);
    /// // 1e-9 * (Floor) 2e-9 = 0
    /// assert_eq!(a.rmul(b, Floor)?, Amount::ZERO);
    /// assert_eq!(b.rmul(a, Floor)?, Amount::ZERO);
    /// # Ok(()) }
    /// ```
    ///
    /// [FixedPoint]: ../struct.FixedPoint.html
    /// [RoundMode]: ./enum.RoundMode.html
    fn rmul(self, rhs: Rhs, mode: RoundMode) -> Result<Self::Output, Self::Error>;

    /// Saturating rounding multiplication. Computes `self * rhs`, saturating at the numeric bounds
    /// ([`MIN`][MIN], [`MAX`][MAX]) instead of overflowing.
    /// Because of provided [`RoundMode`][RoundMode] it's possible to perform across the [`FixedPoint`][FixedPoint]
    /// values.
    ///
    /// ```ignore
    /// use fixnum::{FixedPoint, typenum::U9, ops::{Zero, Bounded, RoundMode::*, RoundingMul}};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "0.000000001".parse()?;
    /// let b: Amount = "0.000000002".parse()?;
    /// // 1e-9 * (SaturatingCeil) 2e9 = 1e-9
    /// assert_eq!(a.saturating_rmul(b, Ceil), a);
    /// // 1e-9 * (SaturatingFloor) 2e9 = 0
    /// assert_eq!(a.saturating_rmul(b, Floor), Amount::ZERO);
    ///
    /// // MIN * (SaturatingFloor) MIN = MAX
    /// assert_eq!(Amount::MIN.saturating_rmul(Amount::MIN, Floor), Amount::MAX);
    ///
    /// let c: Amount = "-1.000000001".parse()?;
    /// // -1.000000001 * (SaturatingCeil) MAX = MIN
    /// assert_eq!(c.saturating_rmul(Amount::MAX, Ceil), Amount::MIN);
    /// # Ok(()) }
    /// ```
    ///
    /// [FixedPoint]: ../struct.FixedPoint.html
    /// [MAX]: ./trait.Bounded.html#associatedconstant.MAX
    /// [MIN]: ./trait.Bounded.html#associatedconstant.MIN
    /// [RoundMode]: ./enum.RoundMode.html
    fn saturating_rmul(self, rhs: Rhs, round_mode: RoundMode) -> Self::Output
    where
        Self: PartialOrd + Zero + Sized,
        Rhs: PartialOrd + Zero,
        Self::Output: Bounded,
    {
        let is_lhs_negative = self < Self::ZERO;
        let is_rhs_negative = rhs < Rhs::ZERO;
        self.rmul(rhs, round_mode).unwrap_or_else(|_| {
            if is_lhs_negative == is_rhs_negative {
                Self::Output::MAX
            } else {
                Self::Output::MIN
            }
        })
    }

    /// Rounding-free multiplication. Returns `Err` on overflow and `Ok(None)`
    /// instead of rounding the result.
    ///
    /// ```
    /// use fixnum::{FixedPoint, typenum::U9, ops::RoundingMul};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "0.000000002".parse()?;
    /// let b: Amount = "0.5".parse()?;
    /// let b_1: Amount = "0.1".parse()?;
    /// let c: Amount = "0.000000001".parse()?;
    /// // 2e-9 * 0.5 = 1e-9
    /// assert_eq!(a.lossless_mul(b)?, Some(c));
    /// assert_eq!(b.lossless_mul(a)?, Some(c));
    /// // 2e-9 * 0.1 = 2e-10 (needs to be rounded, so `None`)
    /// assert_eq!(a.lossless_mul(b_1)?, None);
    /// assert_eq!(b_1.lossless_mul(a)?, None);
    /// # Ok(()) }
    /// ```
    fn lossless_mul(self, rhs: Rhs) -> Result<Option<Self::Output>, Self::Error>;
}

pub trait RoundingDiv<Rhs = Self> {
    type Output;
    type Error;

    /// Checked rounded division. Returns `Err` on overflow or attempt to divide by zero.
    /// Because of provided [`RoundMode`][RoundMode] it's possible to perform across
    /// the [`FixedPoint`][FixedPoint] values.
    ///
    /// ```ignore
    /// use fixnum::{FixedPoint, typenum::U9, ops::{Zero, RoundingDiv, RoundMode::*}};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "0.000000001".parse()?;
    /// let b: Amount = "1000000000".parse()?;
    /// // 1e-9 / (Ceil) 1e9 = 1e-9
    /// assert_eq!(a.rdiv(b, Ceil)?, a);
    /// // 1e-9 / (Floor) 1e9 = 0
    /// assert_eq!(a.rdiv(b, Floor)?, Amount::ZERO);
    /// # Ok(()) }
    /// ```
    ///
    /// [FixedPoint]: ../struct.FixedPoint.html
    /// [RoundMode]: ./enum.RoundMode.html
    fn rdiv(self, rhs: Rhs, mode: RoundMode) -> Result<Self::Output, Self::Error>;

    /// Rounding-free division. Returns `Err` on overflow or attempt to divide by zero
    /// and `Ok(None)` instead of rounding the result.
    ///
    /// ```
    /// use fixnum::{FixedPoint, typenum::U9, ops::RoundingDiv};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "0.000000002".parse()?;
    /// let b: Amount = "2".parse()?;
    /// let b_1: Amount = "10".parse()?;
    /// let c: Amount = "0.000000001".parse()?;
    /// // 2e-9 / 2 = 1e-9
    /// assert_eq!(a.lossless_div(b)?, Some(c));
    /// // 2e-9 / 10 = 2e-10 (needs to be rounded, so `None`)
    /// assert_eq!(a.lossless_div(b_1)?, None);
    /// # Ok(()) }
    /// ```
    fn lossless_div(self, rhs: Rhs) -> Result<Option<Self::Output>, Self::Error>;
}

pub trait RoundingSqrt: Sized {
    type Error;

    /// Checked [rounding][RoundMode] square root.
    /// Returns `Err` for negative argument.
    ///
    /// ```ignore
    /// use fixnum::{ArithmeticError, FixedPoint, typenum::U9};
    /// use fixnum::ops::{Zero, RoundingSqrt, RoundMode::*};
    ///
    /// type Amount = FixedPoint<i64, U9>;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let a: Amount = "81".parse()?;
    /// let b: Amount = "2".parse()?;
    /// let c: Amount = "-100".parse()?;
    /// assert_eq!(a.rsqrt(Floor)?, "9".parse()?);
    /// assert_eq!(b.rsqrt(Floor)?, "1.414213562".parse()?);
    /// assert_eq!(b.rsqrt(Ceil)?, "1.414213563".parse()?);
    /// assert_eq!(c.rsqrt(Floor), Err(ArithmeticError::DomainViolation));
    /// # Ok(()) }
    /// ```
    ///
    /// [RoundMode]: ./enum.RoundMode.html
    fn rsqrt(self, mode: RoundMode) -> Result<Self, Self::Error>;
}

// Impls for primitives.

macro_rules! impl_for_ints {
    ($( $int:ty ),+ $(,)?) => {
        $( impl_for_ints!(@single $int); )*
    };
    (@single $int:ty) => {
        impl Zero for $int {
            const ZERO: Self = 0;
        }

        impl One for $int {
            const ONE: Self = 1;
        }

        impl Bounded for $int {
            const MIN: Self = <$int>::MIN;
            const MAX: Self = <$int>::MAX;
        }

        impl CheckedAdd for $int {
            type Output = $int;
            type Error = ArithmeticError;

            #[inline]
            fn cadd(self, rhs: Self) -> Result<Self::Output, Self::Error> {
                self.checked_add(rhs).ok_or(ArithmeticError::Overflow)
            }

            #[inline]
            fn saturating_add(self, rhs: Self) -> Self::Output {
                <$int>::saturating_add(self, rhs)
            }
        }

        impl CheckedSub for $int {
            type Output = $int;
            type Error = ArithmeticError;

            #[inline]
            fn csub(self, rhs: Self) -> Result<Self::Output, Self::Error> {
                self.checked_sub(rhs).ok_or(ArithmeticError::Overflow)
            }

            #[inline]
            fn saturating_sub(self, rhs: Self) -> Self::Output {
                <$int>::saturating_sub(self, rhs)
            }
        }

        impl CheckedMul for $int {
            type Output = $int;
            type Error = ArithmeticError;

            #[inline]
            fn cmul(self, rhs: Self) -> Result<Self::Output, Self::Error> {
                self.checked_mul(rhs).ok_or(ArithmeticError::Overflow)
            }

            #[inline]
            fn saturating_mul(self, rhs: Self) -> Self::Output {
                <$int>::saturating_mul(self, rhs)
            }
        }

        impl RoundingDiv for $int {
            type Output = $int;
            type Error = ArithmeticError;

            #[inline]
            fn rdiv(self, rhs: Self, mode: RoundMode) -> Result<Self::Output, Self::Error> {
                if rhs == 0 {
                    return Err(ArithmeticError::DivisionByZero);
                }

                let mut result = self / rhs;
                let loss = self - result * rhs;

                if loss != 0 {
                    let sign = self.signum() * rhs.signum();

                    if mode as i32 == sign as i32 {
                        result = result.checked_add(sign).ok_or(ArithmeticError::Overflow)?;
                    }
                }

                Ok(result)
            }

            #[inline]
            fn lossless_div(self, rhs: Self) -> Result<Option<Self::Output>, Self::Error> {
                if rhs == 0 {
                    return Err(ArithmeticError::DivisionByZero);
                }

                let result = self / rhs;
                let loss = self - result * rhs;

                if loss != 0 {
                    return Ok(None)
                }

                Ok(Some(result))
            }
        }

        impl RoundingSqrt for $int {
            type Error = ArithmeticError;

            #[inline]
            fn rsqrt(self, mode: RoundMode) -> Result<Self, Self::Error> {
                let lo = self.sqrt()?;
                Ok(match mode {
                    RoundMode::Floor => lo,
                    RoundMode::Ceil => if lo * lo == self { lo } else {
                        lo + <$int>::ONE
                    },
                })
            }
        }
    };
}

impl_for_ints!(i8, i16, i32, i64, i128); // TODO: unsigned?
