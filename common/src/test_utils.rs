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

use crate::Fixed;
use fixnum::_priv::RoundMode;
use fixnum::ops::{Bounded, CheckedAdd, RoundingMul, Zero};
use fixnum::typenum::Unsigned;
use thiserror::Error;

/// Can be useful to check that an extrinsic is failed due to an error in another pallet
#[macro_export]
macro_rules! assert_noop_msg {
    ( $x:expr, $msg:expr ) => {
        let h = frame_support::storage_root(frame_support::StateVersion::V1);
        if let Err(e) = $crate::with_transaction(|| $x) {
            if let frame_support::dispatch::DispatchError::Module(sp_runtime::ModuleError {
                message,
                ..
            }) = e.error
            {
                assert_eq!(message, Some($msg));
            } else {
                panic!("expected DispatchError::Module, got {:?}", e.error);
            }
        } else {
            panic!("expected Err(_), got Ok(_)");
        }
        assert_eq!(
            h,
            frame_support::storage_root(frame_support::StateVersion::V1)
        );
    };
}

pub fn init_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}

#[derive(Error, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum ApproxEqError {
    #[error("Expected absolute tolerance to be non-negative, got {0:?}")]
    NegativeAbsoluteTolerance(Fixed),
    #[error("Expected percentage to be in interval [0, 1), got {0:?}")]
    IncorrectRelativePercentage(Fixed),
}

#[inline]
fn are_approx_eq_abs_unchecked(left: Fixed, right: Fixed, tolerance: Fixed) -> bool {
    left.clone() <= right.clone().saturating_add(tolerance.clone())
        && right <= left.saturating_add(tolerance)
}

/// Calculate if two values are approximately equal
/// up to some absolute tolerance (constant value)
pub fn are_approx_eq_abs(
    left: Fixed,
    right: Fixed,
    tolerance: Fixed,
) -> Result<bool, ApproxEqError> {
    if tolerance >= Fixed::ZERO {
        Ok(are_approx_eq_abs_unchecked(left, right, tolerance))
    } else {
        Err(ApproxEqError::NegativeAbsoluteTolerance(tolerance))
    }
}

/// Calculate relative absolute tolerance for two numbers: percentage of their magnitude
/// `a.abs() + b.abs()`
fn calculate_relative_tolerance(
    a: Fixed,
    b: Fixed,
    percentage: Fixed,
) -> Result<Fixed, ApproxEqError> {
    let percentage_correct = percentage >= Fixed::ZERO
        && percentage < Fixed::from_bits(10i128.pow(crate::FixedPrecision::I32 as u32));
    if !percentage_correct {
        return Err(ApproxEqError::IncorrectRelativePercentage(percentage));
    }

    let magnitude = a
        .abs()
        .unwrap_or(Fixed::MAX)
        .saturating_add(b.abs().unwrap_or(Fixed::MAX));
    // should not saturate as tolerance is in [0, 1)
    Ok(magnitude.saturating_rmul(percentage, RoundMode::Ceil))
}

/// Calculate if two values are approximately equal
/// up to some relative tolerance (percentage of their magnitude `a.abs() + b.abs()`)
pub fn are_approx_eq_rel(
    left: Fixed,
    right: Fixed,
    percentage: Fixed,
) -> Result<bool, ApproxEqError> {
    let tolerance = calculate_relative_tolerance(left, right, percentage)?;
    are_approx_eq_abs(left, right, tolerance)
}

/// Determine if two numbers `left` and `right` are equal up to some tolerance.
///
/// ## Tolerance
/// Both relative and absolute tolerances are considered here.
///
/// Absolute tolerance is a constant `A > 0`. `left` is approx equal to `right` if
/// `left + a = right` for some `-A <= a <= A`.
///
/// Relative tolerance for two numbers (`R > 0`) is calculated as percentage of their magnitude
/// (`M = left.abs() + right.abs()`). So `left` is approx equal to `right` if
/// `left + r = right` for some `-M*R <= r <= M*R`.
///
/// Satisfying any of the tolerances is enough to consider the numbers approximately equal.
pub fn are_approx_eq(
    left: Fixed,
    right: Fixed,
    absolute_tolerance: Fixed,
    relative_percentage: Fixed,
) -> Result<bool, ApproxEqError> {
    let relative_tolerance = calculate_relative_tolerance(left, right, relative_percentage)?;
    // `max` may overshadow incorrect argumant, so we need to check it here as well
    if absolute_tolerance >= Fixed::ZERO {
        Ok(are_approx_eq_abs_unchecked(
            left,
            right,
            absolute_tolerance.max(relative_tolerance),
        ))
    } else {
        Err(ApproxEqError::NegativeAbsoluteTolerance(absolute_tolerance))
    }
}

#[cfg(test)]
mod test {
    use crate::test_utils::{are_approx_eq, are_approx_eq_abs, are_approx_eq_rel, ApproxEqError};
    use crate::{balance, Fixed, FixedInner};
    use fixnum::ops::{Bounded, Zero};

    #[test]
    fn should_equalize_exact_numbers() {
        for number in [
            Fixed::ZERO,
            Fixed::MAX,
            Fixed::MIN,
            Fixed::from_bits(1),
            Fixed::from_bits(-1),
        ] {
            assert!(are_approx_eq(number, number, Fixed::ZERO, Fixed::ZERO).unwrap());
            assert!(are_approx_eq(number, number, Fixed::MAX, Fixed::ZERO).unwrap());
            assert!(are_approx_eq(
                number,
                number,
                Fixed::ZERO,
                Fixed::from_bits(balance!(1) as FixedInner - 1)
            )
            .unwrap());
            assert!(are_approx_eq(
                number,
                number,
                Fixed::MAX,
                Fixed::from_bits(balance!(1) as FixedInner - 1)
            )
            .unwrap());
        }
    }

    // abs tolerance is drawn as (<=.=>)
    // rel tolerance is drawn as ({#.#})
    struct ApproxEqTestCase {
        left: FixedInner,
        right: FixedInner,
        absolute_tolerance: FixedInner,
        relative_percentage: FixedInner,
    }

    impl ApproxEqTestCase {
        const fn new(
            left: FixedInner,
            right: FixedInner,
            absolute_tolerance: FixedInner,
            relative_percentage: FixedInner,
        ) -> Self {
            Self {
                left,
                right,
                absolute_tolerance,
                relative_percentage,
            }
        }
    }

    // Test cases where the numbers are approx. equal only by absolute tolerance
    const APPROX_EQ_ABS_MATCH_CASES: &[ApproxEqTestCase] = &[
        // -5        0 1       5
        // |         | |       |
        // <=========.=========>
        //           ^right    ^left
        // abs tolerance: [-5, 5]
        // rel tolerance: [-0.05, 0.05]
        ApproxEqTestCase::new(
            balance!(5) as FixedInner,
            balance!(0) as FixedInner,
            balance!(5) as FixedInner,
            balance!(0.01) as FixedInner,
        ),
        // -5        0 1       5
        // |         | |       |
        // <=========.=========>
        // ^left     ^right
        // abs tolerance: [-5, 5]
        // rel tolerance: [-0.05, 0.05]
        ApproxEqTestCase::new(
            balance!(-5) as FixedInner,
            balance!(0) as FixedInner,
            balance!(5) as FixedInner,
            balance!(0.01) as FixedInner,
        ),
        // -5        0 1       5
        // |         | |       |
        // <=========.=========>
        //           ^right
        //            ^~left
        // abs tolerance: [-5, 5]
        // rel tolerance: [-0.05, 0.05]
        ApproxEqTestCase::new(
            balance!(0.05) as FixedInner + 1,
            balance!(0) as FixedInner,
            balance!(5) as FixedInner,
            balance!(0.01) as FixedInner,
        ),
        // -5        0 1       5
        // |         | |       |
        // <=========.=========>
        //           ^right
        //          ^~left
        // abs tolerance: [-5, 5]
        // rel tolerance: [-0.05, 0.05]
        ApproxEqTestCase::new(
            balance!(-0.05) as FixedInner + 1,
            balance!(0) as FixedInner,
            balance!(5) as FixedInner,
            balance!(0.01) as FixedInner,
        ),
        // 47        52        57
        // |         |         |
        // <=========.=========>
        // ^left     ^right
        // abs tolerance: [-5, 5]
        // rel tolerance: [-4.95, 4.95]
        ApproxEqTestCase::new(
            balance!(47) as FixedInner,
            balance!(52) as FixedInner,
            balance!(5) as FixedInner,
            balance!(0.01) as FixedInner,
        ),
        // closer to rel tolerance:
        // 47.02        51.98
        // |            |
        // <============.============>
        // ^left     ^right
        // abs tolerance: [-5, 5]
        // rel tolerance: [-4.95, 4.95]
        ApproxEqTestCase::new(
            balance!(47.02) as FixedInner,
            balance!(51.98) as FixedInner,
            balance!(5) as FixedInner,
            balance!(0.01) as FixedInner,
        ),
    ];

    #[test]
    fn should_approx_eq_match_abs_tolerance() {
        for ApproxEqTestCase {
            left,
            right,
            absolute_tolerance,
            relative_percentage,
        } in APPROX_EQ_ABS_MATCH_CASES
        {
            let left = Fixed::from_bits(*left);
            let right = Fixed::from_bits(*right);
            let absolute_tolerance = Fixed::from_bits(*absolute_tolerance);
            let relative_percentage = Fixed::from_bits(*relative_percentage);
            assert!(
                are_approx_eq(left, right, absolute_tolerance, relative_percentage).unwrap(),
                "Expected {} = {} with absolute tolerance {} and relative tolerance (%) {}, but got '!='",
                left, right, absolute_tolerance, relative_percentage
            );
            assert!(
                are_approx_eq(right, left, absolute_tolerance, relative_percentage).unwrap(),
                "Expected approx eq to be symmetrical; {} = {}, but {} != {} for abs tolerance {} rel tolerance (%) {}",
                left, right, right, left, absolute_tolerance, relative_percentage
            );
        }
    }

    #[test]
    fn should_approx_eq_abs_match_abs_tolerance() {
        for ApproxEqTestCase {
            left,
            right,
            absolute_tolerance,
            relative_percentage: _,
        } in APPROX_EQ_ABS_MATCH_CASES
        {
            let left = Fixed::from_bits(*left);
            let right = Fixed::from_bits(*right);
            let absolute_tolerance = Fixed::from_bits(*absolute_tolerance);
            assert!(
                are_approx_eq_abs(left, right, absolute_tolerance).unwrap(),
                "Expected {} = {} with absolute tolerance {}, but got '!='",
                left,
                right,
                absolute_tolerance
            );
            assert!(
                are_approx_eq_abs(right, left, absolute_tolerance).unwrap(),
                "Expected approx eq to be symmetrical; {} = {}, but {} != {} for abs tolerance {}",
                left,
                right,
                right,
                left,
                absolute_tolerance
            );
        }
    }

    #[test]
    fn should_approx_eq_rel_not_match_abs_tolerance() {
        for ApproxEqTestCase {
            left,
            right,
            absolute_tolerance: _,
            relative_percentage,
        } in APPROX_EQ_ABS_MATCH_CASES
        {
            let left = Fixed::from_bits(*left);
            let right = Fixed::from_bits(*right);
            let relative_percentage = Fixed::from_bits(*relative_percentage);
            assert!(
                !are_approx_eq_rel(left, right, relative_percentage).unwrap(),
                "Expected {} != {} with relative tolerance (%) {}, but got '='",
                left,
                right,
                relative_percentage
            );
            assert!(
                !are_approx_eq_rel(right, left, relative_percentage).unwrap(),
                "Expected approx eq to be symmetrical; {} != {}, but {} == {} for rel tolerance (%) {}",
                left, right, right, left, relative_percentage
            );
        }
    }

    #[test]
    fn should_fail_incorrect_relative_percentage() {
        let percentage = Fixed::from_bits(-1234);
        assert_eq!(
            are_approx_eq(Fixed::ZERO, Fixed::ZERO, Fixed::ZERO, percentage,),
            Err(ApproxEqError::IncorrectRelativePercentage(percentage))
        );
        let percentage = Fixed::from_bits(balance!(1) as FixedInner + 1);
        assert_eq!(
            are_approx_eq(Fixed::ZERO, Fixed::ZERO, Fixed::ZERO, percentage,),
            Err(ApproxEqError::IncorrectRelativePercentage(percentage))
        );
    }

    #[test]
    fn should_fail_incorrect_absolute_percentage() {
        let abs_tolerance = Fixed::from_bits(-1);
        assert_eq!(
            are_approx_eq(Fixed::ZERO, Fixed::ZERO, abs_tolerance, Fixed::ZERO,),
            Err(ApproxEqError::NegativeAbsoluteTolerance(abs_tolerance))
        );
        let abs_tolerance = Fixed::from_bits(i128::MIN);
        assert_eq!(
            are_approx_eq(Fixed::ZERO, Fixed::ZERO, abs_tolerance, Fixed::ZERO,),
            Err(ApproxEqError::NegativeAbsoluteTolerance(abs_tolerance))
        );
    }
}
