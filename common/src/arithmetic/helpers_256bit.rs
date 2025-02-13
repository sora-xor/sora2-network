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

//! Some helper functions to work with 256bit numbers. Note that the functionality provided here is
//! only sensible to use with 256bit numbers because for smaller sizes, you can always rely on
//! assumptions of a bigger type (U256) being available, or simply create a per-thing and use the
//! multiplication implementation provided there.

use sp_arithmetic::{biguint, Rounding};
use sp_core::U256;

/// Helper gcd function used in Rational128 implementation.
// pub fn gcd(a: U256, b: U256) -> U256 {
//     match ((a, b), (a & U256::one(), b & U256::one())) {
//         ((x, y), _) if x == y => y,
//         ((U256::zero(), x), _) | ((x, U256::zero()), _) => x,
//         ((x, y), (U256::zero(), U256::one())) | ((y, x), (U256::one(), U256::zero())) => gcd(x >> U256::one(), y),
//         ((x, y), (U256::zero(), U256::zero())) => gcd(x >> 1, y >> 1) << 1,
//         ((x, y), (U256::one(), U256::one())) => {
//             let (x, y) = (min(x, y), max(x, y));
//             gcd((y - x) >> 1, x)
//         },
//         _ => unreachable!(),
//     }
// }

/// Split a U256 into four u64 limbs (from least significant to most significant).
pub fn split(a: U256) -> (u64, u64, u64, u64) {
    (a.0[0], a.0[1], a.0[2], a.0[3])
}

pub const fn high(a: U256) -> U256 {
    U256([a.0[2], a.0[3], 0, 0])
}

pub const fn low(a: U256) -> U256 {
    U256([a.0[0], a.0[1], 0, 0])
}

/// Convert a U256 to a u32-based BigUint.
pub fn to_big_uint(x: U256) -> biguint::BigUint {
    let (x3, x2, x1, x0) = split(x);
    let (x3h, x3l) = biguint::split(x3);
    let (x2h, x2l) = biguint::split(x2);
    let (x1h, x1l) = biguint::split(x1);
    let (x0h, x0l) = biguint::split(x0);
    let mut n = biguint::BigUint::from_limbs(&[x3h, x3l, x2h, x2l, x1h, x1l, x0h, x0l]);
    n.lstrip();
    n
}

mod double256 {
    use super::*;

    pub const fn overflow_to_256(overflow: bool) -> U256 {
        if overflow {
            U256::one()
        } else {
            U256::zero()
        }
    }

    /// Returns 2^256 - a (two's complement)
    pub fn neg256(a: U256) -> U256 {
        (!a).overflowing_add(U256::one()).0
    }

    /// Returns 2^256 / a
    pub fn div256(a: U256) -> U256 {
        (neg256(a) / a).overflowing_add(U256::one()).0
    }

    /// Returns 2^256 % a
    pub fn mod256(a: U256) -> U256 {
        neg256(a) % a
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct Double256 {
        pub high: U256,
        pub low: U256,
    }

    impl Double256 {
        #[allow(unused)]
        pub fn new(high: U256, low: U256) -> Self {
            Double256 { high, low }
        }
    }

    impl Double256 {
        pub const fn try_into_u256(self) -> Result<U256, ()> {
            if self.high.is_zero() {
                Ok(self.low)
            } else {
                Err(())
            }
        }

        pub const fn zero() -> Self {
            Self {
                high: U256::zero(),
                low: U256::zero(),
            }
        }

        /// Return a `Double256` value representing the `scaled_value << 128`.
        ///
        /// This means the lower half of the `high` component will be equal to the upper 128-bits of
        /// `scaled_value` (in the lower positions) and the upper half of the `low` component will
        /// be equal to the lower 128-bits of `scaled_value`.
        pub fn left_shift_128(scaled_value: U256) -> Self {
            Self {
                high: scaled_value >> 128,
                low: scaled_value << 128,
            }
        }

        /// Construct a value from the upper 256 bits only, with the lower being zeroed.
        pub const fn from_low(low: U256) -> Self {
            Self {
                high: U256::zero(),
                low,
            }
        }

        /// Returns the same value ignoring anything in the high 256-bits.
        pub const fn low_part(self) -> Self {
            Self {
                high: U256::zero(),
                ..self
            }
        }

        /// Returns a * b (in 512 bits)
        pub fn product_of(a: U256, b: U256) -> Self {
            // Split U256 into two 128-bit chunks
            let a_low: U256 = low(a);
            let a_high: U256 = high(a);
            let b_low: U256 = low(b);
            let b_high: U256 = high(b);

            // Perform 128-bit multiplications
            let low: U256 = a_low * b_low; // 128-bit * 128-bit = 256-bit result
            let mid1: U256 = a_low * b_high;
            let mid2: U256 = a_high * b_low;
            let high: U256 = a_high * b_high;

            // Convert to Double256 and shift appropriately
            let product = Self { low, high };

            let mid1_shifted = Self::left_shift_128(mid1);
            let mid2_shifted = Self::left_shift_128(mid2);

            product.add(mid1_shifted).add(mid2_shifted)
        }

        pub fn add(self, b: Self) -> Self {
            let (low, overflow) = self.low.overflowing_add(b.low);
            let carry = overflow_to_256(overflow);

            let high = self.high.overflowing_add(b.high).0.overflowing_add(carry).0;

            Double256 { high, low }
        }

        pub fn div(mut self, rhs: U256) -> (Self, U256) {
            if rhs == U256::one() {
                return (self, U256::zero());
            }

            // Decompose the division process
            // (self === a; rhs === b)
            // Calculate a / b = (a_high << 256 + a_low) / b
            let (q, r) = (div256(rhs), mod256(rhs));

            let mut x = Self::zero();

            // Divide the high part
            while !self.high.is_zero() {
                // x += a.low * q
                x = x.add(Self::product_of(self.high, q));
                // a = a.low * r + a.high
                self = Self::product_of(self.high, r).add(self.low_part());
            }

            let low_result = self.low / rhs;
            let remainder = self.low % rhs;

            (x.add(Self::from_low(low_result)), remainder)
        }
    }
}

/// Returns `a * b / c` and `(a * b) % c` (wrapping to 256 bits) or `None` in the case of
/// overflow and c = 0.
pub fn multiply_by_rational_with_rounding(a: U256, b: U256, c: U256, r: Rounding) -> Option<U256> {
    use double256::Double256;
    if c.is_zero() {
        return None;
    }
    let (result, remainder) = Double256::product_of(a, b).div(c);
    let mut result: U256 = match result.try_into_u256() {
        Ok(v) => v,
        Err(_) => return None,
    };
    if match r {
        Rounding::Up => remainder > U256::zero(),
        // cannot be `(c + 1) / 2` since `c` might be `max_value` and overflow.
        Rounding::NearestPrefUp => remainder >= c / 2 + c % 2,
        Rounding::NearestPrefDown => remainder > c / 2,
        Rounding::Down => false,
    } {
        result = match result.checked_add(U256::one()) {
            Some(v) => v,
            None => return None,
        };
    }
    Some(result)
}

pub fn sqrt(mut n: U256) -> U256 {
    // Modified from https://github.com/derekdreery/integer-sqrt-rs (Apache/MIT).
    if n.is_zero() {
        return U256::zero();
    }

    // Compute bit, the largest power of 4 <= n
    let max_shift: u32 = U256::zero().leading_zeros() - 1;
    let shift: u32 = (max_shift - n.leading_zeros()) & !1;
    let mut bit = U256::one() << shift;

    // Algorithm based on the implementation in:
    // https://en.wikipedia.org/wiki/Methods_of_computing_square_roots#Binary_numeral_system_(base_2)
    // Note that result/bit are logically unsigned (even if T is signed).
    let mut result = U256::zero();
    while !bit.is_zero() {
        if n >= result + bit {
            n -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arithmetic::helpers_256bit::double256::Double256;
    use codec::{Decode, Encode};
    use multiply_by_rational_with_rounding as mulrat;
    use Rounding::*;

    const MAX: U256 = U256::max_value();

    #[test]
    fn test_double256_addition() {
        let a = Double256::new(U256::from(1), U256::from(2));
        let b = Double256::new(U256::from(3), U256::from(4));

        let result = a.add(b);

        assert_eq!(result.high, U256::from(4));
        assert_eq!(result.low, U256::from(6));
    }

    #[test]
    fn test_double256_division() {
        let a = Double256 {
            high: U256::from(10),
            low: U256::from(100),
        };
        let b = U256::from(5);
        let (quotient, remainder) = a.div(b);

        assert_eq!(quotient.high, U256::from(2));
        assert_eq!(quotient.low, U256::from(20));
        assert_eq!(remainder, U256::zero());

        let b = MAX;
        let (quotient, remainder) = a.div(b);
        assert_eq!(quotient.high, U256::zero());
        assert_eq!(quotient.low, U256::from(10));
        assert_eq!(remainder, U256::from(110));
    }

    #[test]
    fn test_double256_division_by_one() {
        let a = Double256 {
            high: U256::from(10),
            low: U256::from(100),
        };
        let b = U256::one();

        let (quotient, remainder) = a.div(b);

        assert_eq!(quotient.high, U256::from(10));
        assert_eq!(quotient.low, U256::from(100));
        assert_eq!(remainder, U256::zero());
    }

    #[test]
    fn test_double256_product_of() {
        let a = U256::from(10);
        let b = U256::from(5);

        let result = Double256::product_of(a, b);

        assert_eq!(result.high, U256::zero());
        assert_eq!(result.low, U256::from(50));
    }

    #[test]
    fn test_double256_product_large_numbers() {
        let a = U256::from(1_000_000_000);
        let b = U256::from(2_000_000_000);

        let result = Double256::product_of(a, b);

        let expected_low = a * b;
        assert_eq!(result.high, U256::zero());
        assert_eq!(result.low, expected_low);
    }

    #[test]
    fn test_double256_addition_overflow() {
        let a = Double256 {
            high: MAX,
            low: MAX,
        };
        let b = Double256 {
            high: U256::from(1),
            low: U256::from(1),
        };

        let result = a.add(b);

        assert_eq!(result.high, U256::from(1));
        assert_eq!(result.low, U256::from(0));
    }

    #[test]
    fn rational_multiply_basic_rounding_works() {
        assert_eq!(
            mulrat(U256::one(), U256::one(), U256::one(), Up),
            Some(U256::one())
        );
        assert_eq!(
            mulrat(U256::from(3), U256::one(), U256::from(3), Up),
            Some(U256::one())
        );
        assert_eq!(
            mulrat(U256::one(), U256::one(), U256::from(3), Up),
            Some(U256::one())
        );
        assert_eq!(
            mulrat(U256::one(), U256::from(2), U256::from(3), Down),
            Some(U256::zero())
        );
        assert_eq!(
            mulrat(U256::one(), U256::one(), U256::from(3), NearestPrefDown),
            Some(U256::zero())
        );
        assert_eq!(
            mulrat(U256::one(), U256::one(), U256::from(2), NearestPrefDown),
            Some(U256::zero())
        );
        assert_eq!(
            mulrat(U256::one(), U256::from(2), U256::from(3), NearestPrefDown),
            Some(U256::one())
        );
        assert_eq!(
            mulrat(U256::one(), U256::one(), U256::from(3), NearestPrefUp),
            Some(U256::zero())
        );
        assert_eq!(
            mulrat(U256::one(), U256::one(), U256::from(2), NearestPrefUp),
            Some(U256::one())
        );
        assert_eq!(
            mulrat(U256::one(), U256::from(2), U256::from(3), NearestPrefUp),
            Some(U256::one())
        );
    }

    #[test]
    fn rational_multiply_big_number_works() {
        assert_eq!(
            mulrat(MAX, MAX - U256::one(), MAX, Down),
            Some(MAX - U256::one())
        );
        assert_eq!(mulrat(MAX, U256::one(), MAX, Down), Some(U256::one()));
        assert_eq!(
            mulrat(MAX, MAX - U256::one(), MAX, Up),
            Some(MAX - U256::one())
        );
        assert_eq!(mulrat(MAX, U256::one(), MAX, Up), Some(U256::one()));
        assert_eq!(
            mulrat(U256::one(), MAX - U256::one(), MAX, Down),
            Some(U256::zero())
        );
        assert_eq!(mulrat(U256::one(), U256::one(), MAX, Up), Some(U256::one()));
        assert_eq!(
            mulrat(U256::one(), MAX / 2, MAX, NearestPrefDown),
            Some(U256::zero())
        );
        assert_eq!(
            mulrat(U256::one(), MAX / 2 + U256::one(), MAX, NearestPrefDown),
            Some(U256::one())
        );
        assert_eq!(
            mulrat(U256::one(), MAX / 2, MAX, NearestPrefUp),
            Some(U256::zero())
        );
        assert_eq!(
            mulrat(U256::one(), MAX / 2 + U256::one(), MAX, NearestPrefUp),
            Some(U256::one())
        );
    }

    #[test]
    fn sqrt_works() {
        for i in 0..100_000u32 {
            let a = sqrt(random_u256(i));
            assert_eq!(sqrt(a * a), a);
        }
    }

    fn random_u256(seed: u32) -> U256 {
        U256::decode(&mut &seed.using_encoded(sp_core::hashing::twox_256)[..])
            .unwrap_or(U256::zero())
    }

    #[test]
    fn op_checked_rounded_div_works() {
        for i in 0..100_000u32 {
            let a = random_u256(i);
            let b = random_u256(i + (1 << 30));
            let c = random_u256(i + (1 << 31));
            let x = mulrat(a, b, c, NearestPrefDown);
            let y = multiply_by_rational_with_rounding(a, b, c, NearestPrefDown);
            assert_eq!(x.is_some(), y.is_some());
            let x = x.unwrap_or(U256::zero());
            let y = y.unwrap_or(U256::zero());
            let d = x.max(y) - x.min(y);
            assert_eq!(d, U256::zero());
        }
    }
}
