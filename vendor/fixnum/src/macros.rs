use crate::FixedPoint;

// TODO: make it Sealed
pub trait Operand<R> {
    type Promotion;
    fn promote(self) -> Self::Promotion;
}

// TODO: restrict `I` and `P`.
impl<I, P> Operand<FixedPoint<I, P>> for FixedPoint<I, P> {
    type Promotion = FixedPoint<I, P>;
    #[inline]
    fn promote(self) -> Self::Promotion {
        self
    }
}

macro_rules! impl_int_operand {
    ($int:ty => $($to:ty),*) => {
        $(
            impl Operand<$to> for $int {
                type Promotion = $to;
                #[inline]
                fn promote(self) -> Self::Promotion {
                    self.into()
                }
            }
        )*

        impl<I, P> Operand<FixedPoint<I, P>> for $int
            where $int: Operand<I>
        {
            type Promotion = <$int as Operand<I>>::Promotion;
            #[inline]
            fn promote(self) -> Self::Promotion {
                Operand::<I>::promote(self)
            }
        }

    }
}

// TODO: unsigned?
impl_int_operand!(i8 => i8, i16, i32, i64, i128);
impl_int_operand!(i16 => i16, i32, i64, i128);
impl_int_operand!(i32 => i32, i64, i128);
impl_int_operand!(i64 => i64, i128);
impl_int_operand!(i128 => i128);

#[macro_export]
macro_rules! impl_op {
    ($lhs:ty [cadd] $rhs:ty = $res:tt) => {
        impl $crate::ops::CheckedAdd<$rhs> for $lhs {
            type Output = $res;
            type Error = $crate::ArithmeticError;

            #[inline]
            fn cadd(self, rhs: $rhs) -> Result<$res, $crate::ArithmeticError> {
                $crate::impl_op!(@checked_method (l = self, r = rhs) => l.cadd(r), $res)
            }

            #[inline]
            fn saturating_add(self, rhs: $rhs) -> Self::Output {
                $crate::impl_op!(@method (l = self, r = rhs) => l.saturating_add(r), $res)
            }
        }
    };
    ($lhs:ty [csub] $rhs:ty = $res:tt) => {
        impl $crate::ops::CheckedSub<$rhs> for $lhs {
            type Output = $res;
            type Error = $crate::ArithmeticError;

            #[inline]
            fn csub(self, rhs: $rhs) -> Result<$res, $crate::ArithmeticError> {
                $crate::impl_op!(@checked_method (l = self, r = rhs) => l.csub(r), $res)
            }

            #[inline]
            fn saturating_sub(self, rhs: $rhs) -> Self::Output {
                $crate::impl_op!(@method (l = self, r = rhs) => l.saturating_sub(r), $res)
            }
        }
    };
    ($lhs:ty [cmul] $rhs:ty = $res:tt) => {
        impl $crate::ops::CheckedMul<$rhs> for $lhs {
            type Output = $res;
            type Error = $crate::ArithmeticError;

            #[inline]
            fn cmul(self, rhs: $rhs) -> Result<$res, $crate::ArithmeticError> {
                $crate::impl_op!(@checked_method (l = self, r = rhs) => l.cmul(r), $res)
            }
        }
    };
    ($lhs:ty [rmul] $rhs:ty = $res:tt) => {
        impl $crate::ops::RoundingMul<$rhs> for $lhs {
            type Output = $res;
            type Error = $crate::ArithmeticError;

            #[inline]
            fn rmul(
                self,
                rhs: $rhs,
                mode: $crate::ops::RoundMode,
            ) -> Result<$res, $crate::ArithmeticError> {
                $crate::impl_op!(@checked_method (l = self, r = rhs) => l.rmul(r, mode), $res)
            }

            #[inline]
            fn lossless_mul(
                self,
                rhs: $rhs,
            ) -> Result<Option<$res>, $crate::ArithmeticError> {
                use $crate::_priv::*;
                fn up<I, O: Operand<I>>(operand: O, _: impl FnOnce(I) -> $res) -> O::Promotion {
                    operand.promote()
                }
                let l = up(self.0, $res);
                let r = up(rhs.0, $res);
                l.lossless_mul(r).map(|p| p.map($res))
            }
        }
    };
    ($lhs:ty [rdiv] $rhs:ty = $res:tt) => {
        impl $crate::ops::RoundingDiv<$rhs> for $lhs {
            type Output = $res;
            type Error = $crate::ArithmeticError;

            #[inline]
            fn rdiv(
                self,
                rhs: $rhs,
                mode: $crate::ops::RoundMode,
            ) -> Result<$res, $crate::ArithmeticError> {
                use core::convert::TryInto;
                $crate::impl_op!(@checked_method (l = self, r = rhs) => {
                    $res(
                        l.try_into().map_err(|_| $crate::ArithmeticError::Overflow)?
                    ).0.rdiv(r, mode)
                }, $res)
            }

            #[inline]
            fn lossless_div(
                self,
                rhs: $rhs,
            ) -> Result<Option<$res>, $crate::ArithmeticError> {
                use core::convert::TryInto;
                use $crate::_priv::*;
                fn up<I, O: Operand<I>>(operand: O, _: impl FnOnce(I) -> $res) -> O::Promotion {
                    operand.promote()
                }
                let l = up(self.0, $res);
                let r = up(rhs.0, $res);
                $res(
                    l.try_into().map_err(|_| $crate::ArithmeticError::Overflow)?
                ).0.lossless_div(r).map(|p| p.map($res))
            }
        }
    };
    (@method ($l:ident = $lhs:expr, $r:ident = $rhs:expr) => $op:expr, $res:tt) => {{
        use $crate::_priv::*;
        fn up<I, O: Operand<I>>(operand: O, _: impl FnOnce(I) -> $res) -> O::Promotion {
            operand.promote()
        }
        let $l = up($lhs.0, $res);
        let $r = up($rhs.0, $res);
        $res::from($op)
    }};
    (@checked_method ($l:ident = $lhs:expr, $r:ident = $rhs:expr) => $op:expr, $res:tt) => {{
        use $crate::_priv::*;
        fn up<I, O: Operand<I>>(operand: O, _: impl FnOnce(I) -> $res) -> O::Promotion {
            operand.promote()
        }
        let $l = up($lhs.0, $res);
        let $r = up($rhs.0, $res);
        $op.map($res)
    }};
}

/// Macro to create fixed-point const "literals".
///
/// ```
/// use derive_more::From;
/// use fixnum::{FixedPoint, typenum::U9, fixnum_const};
///
/// type Amount = FixedPoint<i64, U9>;
///
/// const AMOUNT: Amount = fixnum_const!(12.34, 9);
/// ```
///
/// Probably you'd like to implement your own wrapper around this macro (see also `examples`).
///
/// ```
/// use fixnum::{FixedPoint, typenum::U9};
///
/// type Amount = FixedPoint<i64, U9>;
///
/// macro_rules! fp_const {
///     ($value:literal) => {
///         fixnum::fixnum_const!($value, 9);
///     };
/// }
///
/// const AMOUNT: Amount = fp_const!(12.34);
/// ```
#[macro_export]
macro_rules! fixnum_const {
    ($value:literal, $precision:literal) => {{
        use $crate::FixedPoint;
        use $crate::_priv::*;
        const VALUE_INNER: Int = parse_fixed(stringify!($value), pow10($precision));
        FixedPoint::from_bits(VALUE_INNER as _)
    }};
}

/// Macro to create fixed-point "literals". Contains `.into()` call inside so you can use it with your
/// `From<FixedPoint>` wrapper types.
///
/// ```
/// use derive_more::From;
/// use fixnum::{FixedPoint, typenum::U9, fixnum};
///
/// type Currency = FixedPoint<i64, U9>;
///
/// #[derive(From)]
/// struct Price(Currency);
///
/// #[derive(From)]
/// struct Deposit(Currency);
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let p: Price = fixnum!(12.34, 9);
/// let d: Deposit = fixnum!(-0.4321, 9);
/// # Ok(()) }
/// ```
///
/// Probably you'd like to implement your own wrapper around this macro (see also `examples`).
///
/// ```
/// use fixnum::{FixedPoint, typenum::U9};
///
/// type Currency = FixedPoint<i64, U9>;
///
/// macro_rules! fp {
///     ($val:literal) => {
///         fixnum::fixnum!($val, 9);
///     };
/// }
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let c: Currency = fp!(12.34);
/// # Ok(()) }
/// ```
#[macro_export]
macro_rules! fixnum {
    ($value:literal, $precision:literal) => {
        $crate::fixnum_const!($value, $precision).into()
    };
}
