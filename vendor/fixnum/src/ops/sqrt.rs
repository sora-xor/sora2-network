use core::convert::TryFrom;
use core::mem;

use crate::ArithmeticError;

pub(crate) trait Sqrt: Sized {
    type Error;

    /// Checked square root.
    /// For given non-negative number S returns max possible number Q such that:
    /// `Q ≤ sqrt(S)`.
    /// Returns `Error` for negative arguments.
    fn sqrt(self) -> Result<Self, Self::Error>;
}

macro_rules! impl_sqrt {
    ($( $int:ty ),+ $(,)?) => {
        $( impl_sqrt!(@single $int); )*
    };
    (@single $int:ty) => {
        impl Sqrt for $int {
            type Error = ArithmeticError;

            /// Checked integer square root.
            /// Sqrt implementation courtesy of [`num` crate][num].
            ///
            /// [num]: https://github.com/rust-num/num-integer/blob/4d166cbb754244760e28ea4ce826d54fafd3e629/src/roots.rs#L278
            #[inline]
            fn sqrt(self) -> Result<Self, Self::Error> {
                #[inline]
                const fn bits<T>() -> u32 {
                    (mem::size_of::<T>() * 8) as _
                }

                #[cfg(feature = "std")]
                #[inline]
                fn guess(x: $int) -> $int {
                    (x as f64).sqrt() as $int
                }

                #[cfg(not(feature = "std"))]
                #[inline]
                fn guess(x: $int) -> $int {
                    #[inline]
                    fn log2_estimate(x: $int) -> u32 {
                        debug_assert!(x > 0);
                        bits::<$int>() - 1 - x.leading_zeros()
                    }

                    1 << ((log2_estimate(x) + 1) / 2)
                }

                #[inline]
                fn fixpoint(mut x: $int, f: impl Fn($int) -> $int) -> $int {
                    let mut xn = f(x);
                    while x < xn {
                        x = xn;
                        xn = f(x);
                    }
                    while x > xn {
                        x = xn;
                        xn = f(x);
                    }
                    x
                }

                #[allow(unused_comparisons)]
                if self < 0 {
                    return Err(ArithmeticError::DomainViolation);
                }
                if bits::<$int>() > 64 {
                    // 128-bit division is slow, so do a recursive bitwise `sqrt` until it's small enough.
                    let result = match u64::try_from(self) {
                        Ok(x) => x.sqrt()? as _,
                        Err(_) => {
                            let lo = (self >> 2u32).sqrt()? << 1;
                            let hi = lo + 1;
                            if hi * hi <= self { hi } else { lo }
                        }
                    };
                    return Ok(result);
                }
                if self < 4 {
                    return Ok((self > 0).into());
                }
                // https://en.wikipedia.org/wiki/Methods_of_computing_square_roots#Babylonian_method
                let next = |x: $int| (self / x + x) >> 1;
                Ok(fixpoint(guess(self), next))
            }
        }
    }
}

impl_sqrt!(i8, u8, i16, u16, i32, u32, i64, u64, i128, u128);
