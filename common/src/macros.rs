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

#[macro_export]
macro_rules! fixed {
    ($val:literal) => {
        $crate::fixnum::fixnum!($val, 18)
    };
}

#[macro_export]
macro_rules! fixed_const {
    ($val:literal) => {
        $crate::fixnum::fixnum_const!($val, 18)
    };
}

#[macro_export]
macro_rules! fixed_u256 {
    ($value:literal) => {{
        use sp_core::U256;
        use $crate::fixed::FixedU256;

        let value_inner: U256 = $crate::fixed::parse_fixed(stringify!($value));
        FixedU256::from_inner(value_inner).into()
    }};
}

#[macro_export]
macro_rules! fixed_wrapper_u256 {
    ($val:literal) => {{
        let val: $crate::fixed_wrapper_u256::FixedWrapper256 = $crate::fixed_u256!($val);
        val
    }};
}

#[macro_export]
macro_rules! balance {
    ($value:literal) => {{
        use $crate::fixnum::_priv::parse_fixed;
        const VALUE_SIGNED: i128 = parse_fixed(stringify!($value), 1_000_000_000_000_000_000);
        const VALUE: $crate::Balance = VALUE_SIGNED.abs() as u128;
        VALUE
    }};
    ($e:expr) => {{
        use sp_std::convert::TryFrom;
        let fixed = $crate::Fixed::try_from($e).unwrap();
        $crate::Balance::try_from(fixed.into_bits()).unwrap()
    }};
}

#[macro_export]
macro_rules! fixed_wrapper {
    ($val:literal) => {{
        let val: $crate::prelude::FixedWrapper = $crate::fixed!($val);
        val
    }};
}

#[allow(unused)]
#[macro_export]
macro_rules! dbg {
    () => {
        log::info!("[{}]", core::line!());
    };
    ($val:expr) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                log::info!("[{}] {} = {:#?}",
                    core::line!(), core::stringify!($val), &tmp);
                tmp
            }
        }
    };
    // Trailing comma with single argument is ignored
    ($val:expr,) => { log::info!($val) };
    ($($val:expr),+ $(,)?) => {
        ($(log::info!($val)),+,)
    };
}

#[macro_export]
macro_rules! location_stamp {
    ($name:tt) => {
        &format!("{} at {}:{}", $name, core::file!(), core::line!())
    };
}

// The macro is used in rewards_*.in.
// It's required instead of vec! because vec! places all data on the stack and it causes overflow.
#[macro_export]
macro_rules! vec_push {
    ($($x:expr),+ $(,)?) => (
        {
            let mut vec = Vec::new();
            $(
                vec.push($x);
            )+
            vec
        }
    );
}

#[macro_export]
macro_rules! our_include {
    ($x:expr) => {{
        #[cfg(all(feature = "include-real-files", not(feature = "test")))]
        let output = include!($x);

        #[cfg(any(not(feature = "include-real-files"), feature = "test"))]
        let output = Default::default();

        output
    }};
}

#[macro_export]
macro_rules! our_include_bytes {
    ($x:expr) => {{
        #[cfg(all(feature = "include-real-files", not(feature = "test")))]
        static OUTPUT: &'static [u8] = include_bytes!($x);

        #[cfg(any(not(feature = "include-real-files"), feature = "test"))]
        static OUTPUT: &'static [u8] = &[];

        OUTPUT
    }};
}

/// Assertion that two values are approximately equal up to some absolute tolerance (constant value)
///
/// **NOTE**: It is preferred to utilize to exact equalities even in tests. Fixed point arithmetic
/// allows predictable behaviour of inner arithmetics, so it should be considered everywhere where
/// possible. Use approximate equalities only when you know what you're doing and such inaccurate
/// behavior is expected.
#[macro_export]
macro_rules! assert_approx_eq_abs {
    ($left:expr, $right:expr, $tolerance:expr $(,)?) => {{
        // using `FixedWrapper` allows to work with `Fixed`, `f64`, and int types.
        let left = $crate::prelude::FixedWrapper::from($left)
            .get()
            .expect("cannot approx compare errors");
        let right = $crate::prelude::FixedWrapper::from($right)
            .get()
            .expect("cannot approx compare errors");
        let tolerance = $crate::prelude::FixedWrapper::from($tolerance)
            .get()
            .expect("cannot approx compare errors");
        assert!(
            $crate::test_utils::are_approx_eq_abs(left, right, tolerance).unwrap(),
            "{:?} != {:?} with absolute tolerance {:?}",
            $left,
            $right,
            $tolerance
        );
    }};
}

/// Assertion that two values are approximately equal
/// up to some relative tolerance (percentage of their magnitude `a.abs() + b.abs()`)
///
/// **NOTE**: It is preferred to utilize to exact equalities even in tests. Fixed point arithmetic
/// allows predictable behaviour of inner arithmetics, so it should be considered everywhere where
/// possible. Use approximate equalities only when you know what you're doing and such inaccurate
/// behavior is expected.
#[macro_export]
macro_rules! assert_approx_eq_rel {
    ($left:expr, $right:expr, $tolerance_percentage:expr $(,)?) => {{
        // using `FixedWrapper` allows to work with `Fixed`, `f64`, and int types.
        let left = $crate::prelude::FixedWrapper::from($left)
            .get()
            .expect("cannot approx compare errors");
        let right = $crate::prelude::FixedWrapper::from($right)
            .get()
            .expect("cannot approx compare errors");
        let tolerance = $crate::prelude::FixedWrapper::from($tolerance_percentage)
            .get()
            .expect("cannot approx compare errors");
        assert!(
            $crate::test_utils::are_approx_eq_rel(left, right, tolerance).unwrap(),
            "{:?} != {:?} with relative tolerance (%) {:?}",
            $left,
            $right,
            $tolerance_percentage
        );
    }};
}

/// Assertion if two numbers `left` and `right` are equal up to some tolerance.
///
/// See details in [crate::test_utils::are_approx_eq].
///
/// **NOTE**: It is preferred to utilize to exact equalities even in tests. Fixed point arithmetic
/// allows predictable behaviour of inner arithmetics, so it should be considered everywhere where
/// possible. Use approximate equalities only when you know what you're doing and such inaccurate
/// behavior is expected.
#[macro_export]
macro_rules! assert_approx_eq {
    ($left:expr, $right:expr, $abs_tolerance:expr, $rel_percentage:expr $(,)?) => {{
        // using `FixedWrapper` allows to work with `Fixed`, `f64`, and int types.
        let left = $crate::prelude::FixedWrapper::from($left)
            .get()
            .expect("cannot approx compare errors");
        let right = $crate::prelude::FixedWrapper::from($right)
            .get()
            .expect("cannot approx compare errors");
        let abs_tolerance = $crate::prelude::FixedWrapper::from($abs_tolerance)
            .get()
            .expect("cannot approx compare errors");
        let rel_percentage = $crate::prelude::FixedWrapper::from($rel_percentage)
            .get()
            .expect("cannot approx compare errors");
        assert!(
            $crate::test_utils::are_approx_eq(left, right, abs_tolerance, rel_percentage).unwrap(),
            "{:?} != {:?} with absolute tolerance {:?} and relative tolerance (%) {:?}",
            $left,
            $right,
            $abs_tolerance,
            $rel_percentage,
        );
    }};
}

#[macro_export]
macro_rules! storage_remove_all {
    ($x:ty) => {{
        let mut clear_result = <$x>::clear(u32::max_value(), None);
        while let Some(cursor) = &clear_result.maybe_cursor {
            clear_result = <$x>::clear(u32::max_value(), Some(cursor));
        }
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn should_calculate_formula() {
        use crate::Fixed;

        fn fp(s: &str) -> Fixed {
            s.parse().unwrap()
        }

        let f: Fixed = fixed!(1);
        assert_eq!(f, fp("1"));
        let f: Fixed = fixed!(1.2);
        assert_eq!(f, fp("1.2"));
        let f: Fixed = fixed!(10.09);
        assert_eq!(f, fp("10.09"));
    }

    #[test]
    fn assert_approx_eq_works() {
        use crate::{Fixed, FixedInner};

        assert_approx_eq!(
            balance!(0.99),
            balance!(1.01),
            balance!(0.02),
            balance!(0.01)
        );
        assert_approx_eq!(
            Fixed::from_bits(100000000000000),
            Fixed::from_bits(100000000000002),
            Fixed::from_bits(2),
            Fixed::from_bits(balance!(0.0000000000001) as FixedInner),
        );
        assert_approx_eq!(49f64, 51f64, 2.01f64, 0.02f64);
    }

    #[test]
    #[should_panic]
    fn assert_approx_eq_fails_u128() {
        assert_approx_eq!(
            balance!(0.99),
            balance!(1.01001),
            balance!(0.02),
            balance!(0.01)
        );
    }

    #[test]
    #[should_panic]
    fn assert_approx_eq_fails_fixed() {
        use crate::{Fixed, FixedInner};
        assert_approx_eq!(
            Fixed::from_bits(100000000000000),
            Fixed::from_bits(100000000000003),
            Fixed::from_bits(2),
            Fixed::from_bits(balance!(0.000000000000005) as FixedInner),
        );
    }

    #[test]
    #[should_panic]
    fn assert_approx_eq_fails_f64() {
        // both fail
        assert_approx_eq!(49f64, 51.1f64, 2f64, 0.02f64);
    }

    #[test]
    fn assert_approx_eq_abs_works() {
        use crate::Fixed;

        assert_approx_eq_abs!(balance!(0.99), balance!(1.01), balance!(0.02));
        assert_approx_eq_abs!(
            Fixed::from_bits(100000000000000),
            Fixed::from_bits(100000000000002),
            Fixed::from_bits(2),
        );
        assert_approx_eq_abs!(49f64, 51f64, 2.01f64);
    }

    #[test]
    fn assert_approx_eq_rel_works() {
        use crate::{Fixed, FixedInner};

        assert_approx_eq_rel!(balance!(0.99), balance!(1.01), balance!(0.01));
        assert_approx_eq_rel!(
            Fixed::from_bits(100000000000000),
            Fixed::from_bits(100000000000002),
            Fixed::from_bits(balance!(0.0000000000001) as FixedInner),
        );
        assert_approx_eq_rel!(49f64, 51f64, 0.02f64);
    }

    #[test]
    fn fixed_256_macro_works() {
        use crate::fixed::FixedU256;
        use sp_core::U256;

        let f: FixedU256 = fixed_u256!(
            115792089237316195423570985008687907853269984665640564039457.584007913129639935
        );
        assert_eq!(f, FixedU256::from_inner(U256::max_value()));
        let f: FixedU256 = fixed_u256!(1.0);
        assert_eq!(f, FixedU256::from_inner(U256::from(balance!(1))));
        let f: FixedU256 = fixed_u256!(1);
        assert_eq!(f, FixedU256::from_inner(U256::from(balance!(1))));
        let f: FixedU256 = fixed_u256!(0.0000001);
        assert_eq!(f, FixedU256::from_inner(U256::from(balance!(0.0000001))));
    }
}
