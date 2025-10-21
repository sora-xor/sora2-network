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

use core::convert::{TryFrom, TryInto};
use core::ops::{Mul, MulAssign};
use core::result::Result;

use codec::{Decode, Encode, MaxEncodedLen};
use fixnum::ops::RoundMode::*;
use fixnum::ops::RoundingMul;
use frame_support::RuntimeDebug;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{CheckedAdd, CheckedSub, UniqueSaturatedFrom, UniqueSaturatedInto};
use sp_std::mem;
use sp_std::ops::{Add, Sub};

use crate::outcome_fee::OutcomeFee;
use crate::primitives::Balance;
use crate::Fixed;

#[derive(
    Encode, Decode, Copy, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum QuoteAmount<AmountType> {
    WithDesiredInput { desired_amount_in: AmountType },
    WithDesiredOutput { desired_amount_out: AmountType },
}

impl<T> QuoteAmount<T> {
    pub fn with_desired_input(desired_amount_in: T) -> Self {
        Self::WithDesiredInput { desired_amount_in }
    }

    pub fn with_desired_output(desired_amount_out: T) -> Self {
        Self::WithDesiredOutput { desired_amount_out }
    }

    pub fn with_variant(variant: SwapVariant, amount: T) -> Self {
        match variant {
            SwapVariant::WithDesiredInput => Self::WithDesiredInput {
                desired_amount_in: amount,
            },
            SwapVariant::WithDesiredOutput => Self::WithDesiredOutput {
                desired_amount_out: amount,
            },
        }
    }

    /// Return inner amount value of either desired_amount_in or desired_amount_out to put away enum variant.
    pub fn amount(self) -> T {
        match self {
            QuoteAmount::WithDesiredInput {
                desired_amount_in: amount,
                ..
            }
            | QuoteAmount::WithDesiredOutput {
                desired_amount_out: amount,
                ..
            } => amount,
        }
    }

    /// Returns desired input/output variant of the value
    pub fn variant(&self) -> SwapVariant {
        match self {
            QuoteAmount::WithDesiredInput { .. } => SwapVariant::WithDesiredInput,
            QuoteAmount::WithDesiredOutput { .. } => SwapVariant::WithDesiredOutput,
        }
    }

    /// Position desired amount with outcome such that input and output values are aligned.
    pub fn place_input_and_output<AssetId: Ord>(self, outcome: SwapOutcome<T, AssetId>) -> (T, T) {
        match self {
            Self::WithDesiredInput { .. } => (self.amount(), outcome.amount),
            Self::WithDesiredOutput { .. } => (outcome.amount, self.amount()),
        }
    }

    /// Create new value with same direction as `self`.
    pub fn copy_direction(&self, amount: T) -> Self {
        match self {
            Self::WithDesiredInput { .. } => Self::with_desired_input(amount),
            Self::WithDesiredOutput { .. } => Self::with_desired_output(amount),
        }
    }
}

/// Provide addition for QuoteAmount type. Only values of same enum variant can be added,
/// otherwise panic. Arithmetic failures, e.g. overflow, underflow will panic.
impl<T: Add<Output = T>> Add for QuoteAmount<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (
                Self::WithDesiredInput {
                    desired_amount_in: in_a,
                },
                Self::WithDesiredInput {
                    desired_amount_in: in_b,
                },
            ) => Self::with_desired_input(in_a + in_b),
            (
                Self::WithDesiredOutput {
                    desired_amount_out: out_a,
                },
                Self::WithDesiredOutput {
                    desired_amount_out: out_b,
                },
            ) => Self::with_desired_output(out_a + out_b),
            (_, _) => panic!("cannot add non-uniform variants"),
        }
    }
}

/// Provide subtraction for QuoteAmount type. Only values of same enum variant can be subtracted,
/// otherwise panic. Arithmetic failures, e.g. overflow, underflow will panic.
impl<T: Sub<Output = T>> Sub for QuoteAmount<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (
                Self::WithDesiredInput {
                    desired_amount_in: in_a,
                },
                Self::WithDesiredInput {
                    desired_amount_in: in_b,
                },
            ) => Self::with_desired_input(in_a - in_b),
            (
                Self::WithDesiredOutput {
                    desired_amount_out: out_a,
                },
                Self::WithDesiredOutput {
                    desired_amount_out: out_b,
                },
            ) => Self::with_desired_output(out_a - out_b),
            (_, _) => panic!("cannot subtract non-uniform variants"),
        }
    }
}

/// Provide checked addition for QuoteAmount type. Only values of same enum variant can be added,
/// otherwise return `None`. Arithmetic failures, e.g. overflow, underflow will return `None`.
impl<T: CheckedAdd> CheckedAdd for QuoteAmount<T> {
    fn checked_add(&self, rhs: &QuoteAmount<T>) -> Option<Self::Output> {
        match (self, rhs) {
            (
                Self::WithDesiredInput {
                    desired_amount_in: in_a,
                },
                Self::WithDesiredInput {
                    desired_amount_in: in_b,
                },
            ) => Some(Self::with_desired_input(in_a.checked_add(in_b)?)),
            (
                Self::WithDesiredOutput {
                    desired_amount_out: out_a,
                },
                Self::WithDesiredOutput {
                    desired_amount_out: out_b,
                },
            ) => Some(Self::with_desired_output(out_a.checked_add(out_b)?)),
            (_, _) => None,
        }
    }
}

/// Provide checked subtraction for QuoteAmount type. Only values of same enum variant can be subtracted,
/// otherwise return `None`. Arithmetic failures, e.g. overflow, underflow will return `None`.
impl<T: CheckedSub<Output = T>> CheckedSub for QuoteAmount<T> {
    fn checked_sub(&self, rhs: &QuoteAmount<T>) -> Option<Self::Output> {
        match (self, rhs) {
            (
                Self::WithDesiredInput {
                    desired_amount_in: in_a,
                },
                Self::WithDesiredInput {
                    desired_amount_in: in_b,
                },
            ) => Some(Self::with_desired_input(in_a.checked_sub(in_b)?)),
            (
                Self::WithDesiredOutput {
                    desired_amount_out: out_a,
                },
                Self::WithDesiredOutput {
                    desired_amount_out: out_b,
                },
            ) => Some(Self::with_desired_output(out_a.checked_sub(out_b)?)),
            (_, _) => None,
        }
    }
}

impl<T> From<SwapAmount<T>> for QuoteAmount<T> {
    fn from(swap_amount: SwapAmount<T>) -> Self {
        match swap_amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in, ..
            } => Self::with_desired_input(desired_amount_in),
            SwapAmount::WithDesiredOutput {
                desired_amount_out, ..
            } => Self::with_desired_output(desired_amount_out),
        }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TryFromQuoteAmountError;

impl TryFrom<QuoteAmount<Fixed>> for QuoteAmount<Balance> {
    type Error = TryFromQuoteAmountError;

    fn try_from(v: QuoteAmount<Fixed>) -> Result<Self, Self::Error> {
        Ok(match v {
            QuoteAmount::WithDesiredInput { desired_amount_in } => Self::WithDesiredInput {
                desired_amount_in: desired_amount_in
                    .into_bits()
                    .try_into()
                    .map_err(|_| TryFromQuoteAmountError)?,
            },
            QuoteAmount::WithDesiredOutput { desired_amount_out } => Self::WithDesiredOutput {
                desired_amount_out: desired_amount_out
                    .into_bits()
                    .try_into()
                    .map_err(|_| TryFromQuoteAmountError)?,
            },
        })
    }
}

impl TryFrom<QuoteAmount<Balance>> for QuoteAmount<Fixed> {
    type Error = TryFromQuoteAmountError;

    fn try_from(v: QuoteAmount<Balance>) -> Result<Self, Self::Error> {
        Ok(match v {
            QuoteAmount::WithDesiredInput { desired_amount_in } => Self::WithDesiredInput {
                desired_amount_in: Fixed::from_bits(
                    desired_amount_in
                        .try_into()
                        .map_err(|_| TryFromQuoteAmountError)?,
                ),
            },
            QuoteAmount::WithDesiredOutput { desired_amount_out } => Self::WithDesiredOutput {
                desired_amount_out: Fixed::from_bits(
                    desired_amount_out
                        .try_into()
                        .map_err(|_| TryFromQuoteAmountError)?,
                ),
            },
        })
    }
}

/// Used to identify intention of caller to indicate desired input amount or desired output amount.
/// Similar to SwapAmount, does not hold value in order to be used in external API.
#[derive(
    Encode, Decode, Copy, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum SwapVariant {
    WithDesiredInput,
    WithDesiredOutput,
}

/// Used to identify intention of caller either to transfer tokens based on exact input amount or
/// exact output amount.
#[derive(
    Encode,
    Decode,
    Copy,
    Clone,
    RuntimeDebug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
pub enum SwapAmount<AmountType> {
    WithDesiredInput {
        desired_amount_in: AmountType,
        min_amount_out: AmountType,
    },
    WithDesiredOutput {
        desired_amount_out: AmountType,
        max_amount_in: AmountType,
    },
}

impl<T> SwapAmount<T> {
    pub fn with_desired_input(desired_amount_in: T, min_amount_out: T) -> Self {
        Self::WithDesiredInput {
            desired_amount_in,
            min_amount_out,
        }
    }

    pub fn with_desired_output(desired_amount_out: T, max_amount_in: T) -> Self {
        Self::WithDesiredOutput {
            desired_amount_out,
            max_amount_in,
        }
    }

    pub fn with_variant(variant: SwapVariant, amount: T, limit: T) -> Self {
        match variant {
            SwapVariant::WithDesiredInput => Self::WithDesiredInput {
                desired_amount_in: amount,
                min_amount_out: limit,
            },
            SwapVariant::WithDesiredOutput => Self::WithDesiredOutput {
                desired_amount_out: amount,
                max_amount_in: limit,
            },
        }
    }

    /// Return inner amount value of either desired_amount_in or desired_amount_out to put away enum variant.
    pub fn amount(self) -> T {
        match self {
            SwapAmount::WithDesiredInput {
                desired_amount_in: amount,
                ..
            }
            | SwapAmount::WithDesiredOutput {
                desired_amount_out: amount,
                ..
            } => amount,
        }
    }

    /// Return inner limit value of either min_amount_out or max_amount_in to put away enum variant.
    pub fn limit(self) -> T {
        match self {
            SwapAmount::WithDesiredInput {
                min_amount_out: amount,
                ..
            }
            | SwapAmount::WithDesiredOutput {
                max_amount_in: amount,
                ..
            } => amount,
        }
    }

    /// Returns desired input/output variant of the value
    pub fn variant(&self) -> SwapVariant {
        match self {
            SwapAmount::WithDesiredInput { .. } => SwapVariant::WithDesiredInput,
            SwapAmount::WithDesiredOutput { .. } => SwapVariant::WithDesiredOutput,
        }
    }

    // Position desired amount with outcome such that input and output values are aligned.
    pub fn place_input_and_output<AssetId: Ord>(self, outcome: SwapOutcome<T, AssetId>) -> (T, T) {
        match self {
            Self::WithDesiredInput { .. } => (self.amount(), outcome.amount),
            Self::WithDesiredOutput { .. } => (outcome.amount, self.amount()),
        }
    }

    // Create new value with same direction as `self`.
    pub fn copy_direction(&self, amount: T, limit: T) -> Self {
        match self {
            Self::WithDesiredInput { .. } => Self::with_desired_input(amount, limit),
            Self::WithDesiredOutput { .. } => Self::with_desired_output(amount, limit),
        }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TryFromSwapAmountError;

impl TryFrom<SwapAmount<Fixed>> for SwapAmount<Balance> {
    type Error = TryFromSwapAmountError;

    fn try_from(v: SwapAmount<Fixed>) -> Result<Self, Self::Error> {
        Ok(match v {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::WithDesiredInput {
                desired_amount_in: desired_amount_in
                    .into_bits()
                    .try_into()
                    .map_err(|_| TryFromSwapAmountError)?,
                min_amount_out: min_amount_out
                    .into_bits()
                    .try_into()
                    .map_err(|_| TryFromSwapAmountError)?,
            },
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::WithDesiredOutput {
                desired_amount_out: desired_amount_out
                    .into_bits()
                    .try_into()
                    .map_err(|_| TryFromSwapAmountError)?,
                max_amount_in: max_amount_in
                    .into_bits()
                    .try_into()
                    .map_err(|_| TryFromSwapAmountError)?,
            },
        })
    }
}

impl UniqueSaturatedFrom<SwapAmount<Fixed>> for SwapAmount<Balance> {
    fn unique_saturated_from(v: SwapAmount<Fixed>) -> Self {
        fn fixed_to_balance_saturating(value: Fixed) -> Balance {
            Balance::try_from(value.into_bits()).unwrap_or(0)
        }

        match v {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::WithDesiredInput {
                desired_amount_in: fixed_to_balance_saturating(desired_amount_in),
                min_amount_out: fixed_to_balance_saturating(min_amount_out),
            },
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::WithDesiredOutput {
                desired_amount_out: fixed_to_balance_saturating(desired_amount_out),
                max_amount_in: fixed_to_balance_saturating(max_amount_in),
            },
        }
    }
}

impl TryFrom<SwapAmount<Balance>> for SwapAmount<Fixed> {
    type Error = TryFromSwapAmountError;

    fn try_from(v: SwapAmount<Balance>) -> Result<Self, Self::Error> {
        Ok(match v {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::WithDesiredInput {
                desired_amount_in: Fixed::from_bits(
                    desired_amount_in
                        .try_into()
                        .map_err(|_| TryFromSwapAmountError)?,
                ),
                min_amount_out: Fixed::from_bits(
                    min_amount_out
                        .try_into()
                        .map_err(|_| TryFromSwapAmountError)?,
                ),
            },
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::WithDesiredOutput {
                desired_amount_out: Fixed::from_bits(
                    desired_amount_out
                        .try_into()
                        .map_err(|_| TryFromSwapAmountError)?,
                ),
                max_amount_in: Fixed::from_bits(
                    max_amount_in
                        .try_into()
                        .map_err(|_| TryFromSwapAmountError)?,
                ),
            },
        })
    }
}

impl UniqueSaturatedFrom<SwapAmount<Balance>> for SwapAmount<Fixed> {
    fn unique_saturated_from(v: SwapAmount<Balance>) -> Self {
        match v {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::WithDesiredInput {
                desired_amount_in: Fixed::from_bits(desired_amount_in.unique_saturated_into()),
                min_amount_out: Fixed::from_bits(min_amount_out.unique_saturated_into()),
            },
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::WithDesiredOutput {
                desired_amount_out: Fixed::from_bits(desired_amount_out.unique_saturated_into()),
                max_amount_in: Fixed::from_bits(max_amount_in.unique_saturated_into()),
            },
        }
    }
}

impl<T> From<SwapAmount<T>> for SwapVariant {
    fn from(v: SwapAmount<T>) -> Self {
        match v {
            SwapAmount::WithDesiredInput { .. } => SwapVariant::WithDesiredInput,
            _ => SwapVariant::WithDesiredOutput,
        }
    }
}

// TODO: use macros for impl generation
impl<T> Mul<Fixed> for SwapAmount<T>
where
    T: Copy + Into<Fixed> + From<Fixed>,
{
    type Output = Self;

    fn mul(self, rhs: Fixed) -> Self::Output {
        match self {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::with_desired_input(
                rhs.rmul(desired_amount_in.into(), Floor).unwrap().into(),
                rhs.rmul(min_amount_out.into(), Floor).unwrap().into(),
            ),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::with_desired_output(
                rhs.rmul(desired_amount_out.into(), Floor).unwrap().into(),
                rhs.rmul(max_amount_in.into(), Floor).unwrap().into(),
            ),
        }
    }
}

impl<T> MulAssign<Fixed> for SwapAmount<T>
where
    T: Copy + Into<Fixed> + From<Fixed>,
{
    fn mul_assign(&mut self, rhs: Fixed) {
        match *self {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => mem::replace(
                self,
                SwapAmount::with_desired_input(
                    rhs.rmul(desired_amount_in.into(), Floor).unwrap().into(),
                    rhs.rmul(min_amount_out.into(), Floor).unwrap().into(),
                ),
            ),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => mem::replace(
                self,
                SwapAmount::with_desired_output(
                    rhs.rmul(desired_amount_out.into(), Floor).unwrap().into(),
                    rhs.rmul(max_amount_in.into(), Floor).unwrap().into(),
                ),
            ),
        };
    }
}

impl<T> Mul<SwapAmount<T>> for Fixed
where
    T: Copy + RoundingMul<Output = T> + Into<Fixed> + From<Fixed>,
{
    type Output = SwapAmount<T>;

    fn mul(self, rhs: SwapAmount<T>) -> Self::Output {
        match rhs {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::with_desired_input(
                self.rmul(desired_amount_in.into(), Floor).unwrap().into(),
                self.rmul(min_amount_out.into(), Floor).unwrap().into(),
            ),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::with_desired_output(
                self.rmul(desired_amount_out.into(), Floor).unwrap().into(),
                self.rmul(max_amount_in.into(), Floor).unwrap().into(),
            ),
        }
    }
}

/// Amount of output tokens from either price request or actual exchange.
#[derive(
    Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct SwapOutcome<AmountType, AssetId: Ord> {
    /// Actual swap output/input amount including deduced fee.
    pub amount: AmountType,
    /// Accumulated fee amount.
    pub fee: OutcomeFee<AssetId, AmountType>,
}

impl<AmountType, AssetId: Ord> SwapOutcome<AmountType, AssetId> {
    pub fn new(amount: AmountType, fee: OutcomeFee<AssetId, AmountType>) -> Self {
        Self { amount, fee }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TryFromSwapOutcomeError;

impl<AssetId: Ord> TryFrom<SwapOutcome<Balance, AssetId>> for SwapOutcome<Fixed, AssetId> {
    type Error = TryFromSwapOutcomeError;

    fn try_from(value: SwapOutcome<Balance, AssetId>) -> Result<Self, Self::Error> {
        let amount = Fixed::from_bits(
            value
                .amount
                .try_into()
                .map_err(|_| TryFromSwapOutcomeError)?,
        );

        let mut fee = OutcomeFee::new();
        for (asset, fee_amount) in value.fee.0 {
            let fee_fixed =
                Fixed::from_bits(fee_amount.try_into().map_err(|_| TryFromSwapOutcomeError)?);
            fee.0.insert(asset, fee_fixed);
        }

        Ok(Self { amount, fee })
    }
}

impl<AssetId: Ord> TryFrom<SwapOutcome<Fixed, AssetId>> for SwapOutcome<Balance, AssetId> {
    type Error = TryFromSwapOutcomeError;

    fn try_from(value: SwapOutcome<Fixed, AssetId>) -> Result<Self, Self::Error> {
        let amount = value
            .amount
            .into_bits()
            .try_into()
            .map_err(|_| TryFromSwapOutcomeError)?;

        let mut fee = OutcomeFee::new();
        for (asset, fee_amount) in value.fee.0 {
            let fee_balance = fee_amount
                .into_bits()
                .try_into()
                .map_err(|_| TryFromSwapOutcomeError)?;
            fee.0.insert(asset, fee_balance);
        }

        Ok(Self { amount, fee })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::balance;
    use crate::fixed;
    use sp_std::convert::TryFrom;

    #[test]
    fn test_quote_amount_direction_helpers() {
        let qa_in = QuoteAmount::with_desired_input(10u32);
        assert_eq!(qa_in.variant(), SwapVariant::WithDesiredInput);
        assert_eq!(qa_in.amount(), 10);

        let qa_out = QuoteAmount::with_desired_output(25u32);
        assert_eq!(qa_out.variant(), SwapVariant::WithDesiredOutput);
        assert_eq!(qa_out.amount(), 25);

        assert_eq!(
            QuoteAmount::with_variant(SwapVariant::WithDesiredInput, 7u32),
            QuoteAmount::with_desired_input(7u32)
        );
        assert_eq!(
            QuoteAmount::with_variant(SwapVariant::WithDesiredOutput, 9u32),
            QuoteAmount::with_desired_output(9u32)
        );

        assert_eq!(qa_in.copy_direction(3), QuoteAmount::with_desired_input(3));
        assert_eq!(
            qa_out.copy_direction(4),
            QuoteAmount::with_desired_output(4)
        );
    }

    #[test]
    fn test_quote_amount_place_input_output_should_align() {
        let qa_in = QuoteAmount::with_desired_input(11u32);
        let qa_out = QuoteAmount::with_desired_output(13u32);
        let empty_fee = OutcomeFee::<u8, u32>::new();
        let outcome = SwapOutcome::new(17u32, empty_fee);

        assert_eq!(qa_in.place_input_and_output(outcome.clone()), (11, 17));
        assert_eq!(qa_out.place_input_and_output(outcome), (17, 13));
    }

    #[test]
    fn test_quote_amount_from_swap_amount_should_match_direction() {
        let swap_in = SwapAmount::with_desired_input(20u32, 5u32);
        let swap_out = SwapAmount::with_desired_output(30u32, 40u32);

        assert_eq!(
            QuoteAmount::from(swap_in),
            QuoteAmount::with_desired_input(20u32)
        );
        assert_eq!(
            QuoteAmount::from(swap_out),
            QuoteAmount::with_desired_output(30u32)
        );
    }

    #[test]
    fn test_quote_amount_arithmetic_should_follow_variant() {
        let sum_in = QuoteAmount::with_desired_input(2u32) + QuoteAmount::with_desired_input(3u32);
        assert_eq!(sum_in, QuoteAmount::with_desired_input(5u32));

        let sum_out =
            QuoteAmount::with_desired_output(4u32) + QuoteAmount::with_desired_output(6u32);
        assert_eq!(sum_out, QuoteAmount::with_desired_output(10u32));

        let diff_in = QuoteAmount::with_desired_input(9u32) - QuoteAmount::with_desired_input(4u32);
        assert_eq!(diff_in, QuoteAmount::with_desired_input(5u32));

        let diff_out =
            QuoteAmount::with_desired_output(12u32) - QuoteAmount::with_desired_output(2u32);
        assert_eq!(diff_out, QuoteAmount::with_desired_output(10u32));
    }

    #[test]
    fn test_quote_amount_checked_ops_should_handle_edge_cases() {
        let a = QuoteAmount::with_desired_input(u8::MAX);
        let b = QuoteAmount::with_desired_input(1u8);
        assert!(a.checked_add(&b).is_none());

        let add_in_lhs = QuoteAmount::with_desired_input(10u8);
        let add_in_rhs = QuoteAmount::with_desired_input(5u8);
        assert_eq!(
            add_in_lhs.checked_add(&add_in_rhs),
            Some(QuoteAmount::with_desired_input(15u8))
        );

        let c = QuoteAmount::with_desired_output(0u8);
        let d = QuoteAmount::with_desired_output(1u8);
        assert!(c.checked_sub(&d).is_none());

        let mixed_in = QuoteAmount::with_desired_input(5u8);
        let mixed_out = QuoteAmount::with_desired_output(5u8);
        assert!(mixed_in.checked_add(&mixed_out).is_none());
        assert!(mixed_in.checked_sub(&mixed_out).is_none());

        let ok_add = QuoteAmount::with_desired_output(10u8);
        let ok_add_rhs = QuoteAmount::with_desired_output(5u8);
        assert_eq!(
            ok_add.checked_add(&ok_add_rhs),
            Some(QuoteAmount::with_desired_output(15u8))
        );

        let ok_sub = QuoteAmount::with_desired_input(10u8);
        let ok_sub_rhs = QuoteAmount::with_desired_input(5u8);
        assert_eq!(
            ok_sub.checked_sub(&ok_sub_rhs),
            Some(QuoteAmount::with_desired_input(5u8))
        );

        let ok_sub_out = QuoteAmount::with_desired_output(9u8);
        let ok_sub_out_rhs = QuoteAmount::with_desired_output(4u8);
        assert_eq!(
            ok_sub_out.checked_sub(&ok_sub_out_rhs),
            Some(QuoteAmount::with_desired_output(5u8))
        );
    }

    #[test]
    fn test_quote_amount_conversions_between_fixed_and_balance() {
        let qa_fixed: QuoteAmount<Fixed> = QuoteAmount::with_desired_input(fixed!(1.5));
        let converted: QuoteAmount<Balance> =
            QuoteAmount::try_from(qa_fixed).expect("should convert to balance");
        assert_eq!(converted, QuoteAmount::with_desired_input(balance!(1.5)));

        let qa_fixed_out: QuoteAmount<Fixed> = QuoteAmount::with_desired_output(fixed!(2));
        let converted_out: QuoteAmount<Balance> =
            QuoteAmount::try_from(qa_fixed_out).expect("should convert to balance");
        assert_eq!(converted_out, QuoteAmount::with_desired_output(balance!(2)));

        let negative_fixed: QuoteAmount<Fixed> = QuoteAmount::with_desired_input(fixed!(-1));
        assert!(QuoteAmount::<Balance>::try_from(negative_fixed).is_err());

        let overflowing_balance = QuoteAmount::with_desired_output(Balance::MAX);
        assert!(QuoteAmount::<Fixed>::try_from(overflowing_balance).is_err());
    }

    #[test]
    fn test_swap_amount_direction_helpers() {
        let sa_in = SwapAmount::with_desired_input(50u32, 10u32);
        assert_eq!(sa_in.variant(), SwapVariant::WithDesiredInput);
        assert_eq!(sa_in.amount(), 50);
        assert_eq!(sa_in.limit(), 10);
        assert_eq!(
            SwapAmount::with_variant(SwapVariant::WithDesiredInput, 7u32, 3u32),
            SwapAmount::with_desired_input(7u32, 3u32)
        );
        assert_eq!(SwapVariant::from(sa_in), SwapVariant::WithDesiredInput);

        let sa_out = SwapAmount::with_desired_output(80u32, 120u32);
        assert_eq!(sa_out.variant(), SwapVariant::WithDesiredOutput);
        assert_eq!(sa_out.amount(), 80);
        assert_eq!(sa_out.limit(), 120);
        assert_eq!(
            SwapAmount::with_variant(SwapVariant::WithDesiredOutput, 9u32, 11u32),
            SwapAmount::with_desired_output(9u32, 11u32)
        );
        assert_eq!(SwapVariant::from(sa_out), SwapVariant::WithDesiredOutput);
    }

    #[test]
    fn test_swap_amount_place_input_output_should_align() {
        let sa_in = SwapAmount::with_desired_input(40u32, 30u32);
        let sa_out = SwapAmount::with_desired_output(15u32, 70u32);
        let fee = OutcomeFee::<u8, u32>::new();
        let outcome = SwapOutcome::new(90u32, fee);

        assert_eq!(sa_in.place_input_and_output(outcome.clone()), (40, 90));
        assert_eq!(sa_out.place_input_and_output(outcome), (90, 15));
    }

    #[test]
    fn test_swap_amount_copy_direction_should_preserve_variant() {
        let sa_in = SwapAmount::with_desired_input(10u32, 5u32);
        assert_eq!(
            sa_in.copy_direction(3, 2),
            SwapAmount::with_desired_input(3u32, 2u32)
        );

        let sa_out = SwapAmount::with_desired_output(9u32, 12u32);
        assert_eq!(
            sa_out.copy_direction(7, 6),
            SwapAmount::with_desired_output(7u32, 6u32)
        );
    }

    #[test]
    fn test_swap_amount_multiplication_variants_should_pass() {
        let swap_input: SwapAmount<Fixed> = SwapAmount::with_desired_input(fixed!(25), fixed!(10));
        assert_eq!(
            swap_input * fixed!(3),
            SwapAmount::with_desired_input(fixed!(75), fixed!(30))
        );

        let swap_output: SwapAmount<Fixed> =
            SwapAmount::with_desired_output(fixed!(40), fixed!(120));
        assert_eq!(
            swap_output * fixed!(2),
            SwapAmount::with_desired_output(fixed!(80), fixed!(240))
        );

        let factor_in: Fixed = fixed!(4);
        assert_eq!(
            factor_in * swap_input,
            SwapAmount::with_desired_input(fixed!(100), fixed!(40))
        );
        let factor_out: Fixed = fixed!(5);
        assert_eq!(
            factor_out * swap_output,
            SwapAmount::with_desired_output(fixed!(200), fixed!(600))
        );
    }

    #[test]
    fn test_swap_amount_mul_assign_output_variant_should_pass() {
        let mut swap_amount: SwapAmount<Fixed> =
            SwapAmount::with_desired_output(fixed!(30), fixed!(80));
        swap_amount *= fixed!(3);
        assert_eq!(
            swap_amount,
            SwapAmount::with_desired_output(fixed!(90), fixed!(240))
        );
    }

    #[test]
    fn test_swap_amount_try_from_fixed_to_balance_should_handle_errors() {
        let swap_fixed_in: SwapAmount<Fixed> =
            SwapAmount::with_desired_input(fixed!(1.25), fixed!(0.75));
        let converted_in: SwapAmount<Balance> =
            SwapAmount::try_from(swap_fixed_in).expect("convert input variant");
        assert_eq!(
            converted_in,
            SwapAmount::with_desired_input(balance!(1.25), balance!(0.75))
        );

        let swap_fixed_out: SwapAmount<Fixed> =
            SwapAmount::with_desired_output(fixed!(2), fixed!(3));
        let converted_out: SwapAmount<Balance> =
            SwapAmount::try_from(swap_fixed_out).expect("convert output variant");
        assert_eq!(
            converted_out,
            SwapAmount::with_desired_output(balance!(2), balance!(3))
        );

        let negative_fixed: SwapAmount<Fixed> =
            SwapAmount::with_desired_output(fixed!(-1), fixed!(1));
        assert!(SwapAmount::<Balance>::try_from(negative_fixed).is_err());
    }

    #[test]
    fn test_swap_amount_try_from_balance_to_fixed_should_handle_edges() {
        let swap_balance_in = SwapAmount::with_desired_input(balance!(1.5), balance!(0.25));
        let converted_in: SwapAmount<Fixed> =
            SwapAmount::try_from(swap_balance_in).expect("convert balance input");
        assert_eq!(
            converted_in,
            SwapAmount::with_desired_input(fixed!(1.5), fixed!(0.25))
        );

        let swap_balance_out = SwapAmount::with_desired_output(balance!(2.5), balance!(4));
        let converted_out: SwapAmount<Fixed> =
            SwapAmount::try_from(swap_balance_out).expect("convert balance output");
        assert_eq!(
            converted_out,
            SwapAmount::with_desired_output(fixed!(2.5), fixed!(4))
        );

        let overflow_balance = SwapAmount::with_desired_input(Balance::MAX, Balance::MAX);
        assert!(SwapAmount::<Fixed>::try_from(overflow_balance).is_err());
    }

    #[test]
    fn test_swap_amount_unique_saturated_from_fixed_to_balance() {
        let swap_negative = SwapAmount::with_desired_input(fixed!(-5), fixed!(2));
        let saturated_negative: SwapAmount<Balance> =
            SwapAmount::unique_saturated_from(swap_negative);
        match saturated_negative {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => {
                assert_eq!(desired_amount_in, 0);
                assert_eq!(min_amount_out, balance!(2));
            }
            _ => panic!("expected input variant"),
        }

        let swap_mixed = SwapAmount::with_desired_output(fixed!(3), fixed!(-1));
        let saturated_mixed: SwapAmount<Balance> = SwapAmount::unique_saturated_from(swap_mixed);
        match saturated_mixed {
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => {
                assert_eq!(desired_amount_out, balance!(3));
                assert_eq!(max_amount_in, 0);
            }
            _ => panic!("expected input variant"),
        }
    }

    #[test]
    fn test_swap_amount_unique_saturated_from_balance_to_fixed() {
        let swap_large = SwapAmount::with_desired_output(Balance::MAX, Balance::MAX);
        let saturated: SwapAmount<Fixed> = SwapAmount::unique_saturated_from(swap_large);
        match saturated {
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => {
                assert_eq!(desired_amount_out.into_bits(), i128::MAX);
                assert_eq!(max_amount_in.into_bits(), i128::MAX);
            }
            _ => panic!("expected output variant"),
        }
    }

    #[test]
    fn test_swap_outcome_try_from_balance_should_convert() {
        let mut fee = OutcomeFee::<u8, Balance>::new();
        fee.0.insert(1, balance!(0.5));
        fee.0.insert(2, balance!(1.25));
        let outcome = SwapOutcome::new(balance!(10), fee);

        let converted: SwapOutcome<Fixed, u8> =
            SwapOutcome::try_from(outcome).expect("should convert outcome");
        assert_eq!(converted.amount, fixed!(10));
        assert_eq!(converted.fee.0.get(&1), Some(&fixed!(0.5)));
        assert_eq!(converted.fee.0.get(&2), Some(&fixed!(1.25)));
        assert_eq!(converted.fee.0.len(), 2);
    }

    #[test]
    fn test_swap_outcome_try_from_balance_should_fail_on_large_values() {
        let mut fee = OutcomeFee::<u8, Balance>::new();
        fee.0.insert(3, Balance::MAX);
        let outcome = SwapOutcome::new(Balance::MAX, fee);
        assert!(SwapOutcome::<Fixed, u8>::try_from(outcome).is_err());
    }

    #[test]
    fn test_swap_outcome_try_from_fixed_should_convert() {
        let mut fee = OutcomeFee::<u8, Fixed>::new();
        fee.0.insert(4, fixed!(0.75));
        let outcome = SwapOutcome::new(fixed!(5.5), fee);

        let converted: SwapOutcome<Balance, u8> =
            SwapOutcome::try_from(outcome).expect("should convert to balance");
        assert_eq!(converted.amount, balance!(5.5));
        assert_eq!(converted.fee.0.get(&4), Some(&balance!(0.75)));
        assert_eq!(converted.fee.0.len(), 1);
    }

    #[test]
    fn test_swap_outcome_try_from_fixed_should_fail_on_negative_values() {
        let mut fee = OutcomeFee::<u8, Fixed>::new();
        fee.0.insert(5, fixed!(-0.1));
        let outcome = SwapOutcome::new(fixed!(-1), fee);
        assert!(SwapOutcome::<Balance, u8>::try_from(outcome).is_err());
    }

    #[test]
    fn test_mul_amount_should_pass() {
        let swap_amount: SwapAmount<Fixed> =
            SwapAmount::with_desired_input(fixed!(100), fixed!(50));
        assert_eq!(
            swap_amount * fixed!(2),
            SwapAmount::with_desired_input(fixed!(200), fixed!(100))
        );
    }

    #[test]
    fn test_mul_assign_amount_should_pass() {
        let mut swap_amount: SwapAmount<Fixed> =
            SwapAmount::with_desired_input(fixed!(100), fixed!(50));
        swap_amount *= fixed!(2);
        assert_eq!(
            swap_amount,
            SwapAmount::with_desired_input(fixed!(200), fixed!(100))
        );
    }
}
