use core::convert::{TryFrom, TryInto};
use core::ops::{Mul, MulAssign};
use core::result::Result;

use codec::{Decode, Encode};
use fixnum::ops::{RoundMode::*, RoundingMul};
use frame_support::RuntimeDebug;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{UniqueSaturatedFrom, UniqueSaturatedInto};
use sp_std::mem;

use crate::primitives::Balance;
use crate::Fixed;

#[derive(Encode, Decode, Copy, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
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
}

/// Used to identify intention of caller to indicate desired input amount or desired output amount.
/// Similar to SwapAmount, does not hold value in order to be used in external API.
#[derive(Encode, Decode, Copy, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum SwapVariant {
    WithDesiredInput,
    WithDesiredOutput,
}

/// Used to identify intention of caller either to transfer tokens based on exact input amount or
/// exact output amount.
#[derive(Encode, Decode, Copy, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
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
        match v {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::WithDesiredInput {
                desired_amount_in: desired_amount_in.into_bits().unique_saturated_into(),
                min_amount_out: min_amount_out.into_bits().unique_saturated_into(),
            },
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::WithDesiredOutput {
                desired_amount_out: desired_amount_out.into_bits().unique_saturated_into(),
                max_amount_in: max_amount_in.into_bits().unique_saturated_into(),
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
        match self.clone() {
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
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct SwapOutcome<AmountType> {
    /// Actual swap output/input amount including deduced fee.
    pub amount: AmountType,
    /// Accumulated fee amount, assumed to be in XOR.
    pub fee: AmountType,
}

impl<AmountType> SwapOutcome<AmountType> {
    pub fn new(amount: AmountType, fee: AmountType) -> Self {
        Self { amount, fee }
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TryFromSwapOutcomeError;

impl TryFrom<SwapOutcome<Balance>> for SwapOutcome<Fixed> {
    type Error = TryFromSwapOutcomeError;

    fn try_from(value: SwapOutcome<Balance>) -> Result<Self, Self::Error> {
        let amount = Fixed::from_bits(
            value
                .amount
                .try_into()
                .map_err(|_| TryFromSwapOutcomeError)?,
        );
        let fee = Fixed::from_bits(value.fee.try_into().map_err(|_| TryFromSwapOutcomeError)?);
        Ok(Self { amount, fee })
    }
}

impl TryFrom<SwapOutcome<Fixed>> for SwapOutcome<Balance> {
    type Error = TryFromSwapOutcomeError;

    fn try_from(value: SwapOutcome<Fixed>) -> Result<Self, Self::Error> {
        let amount = value
            .amount
            .into_bits()
            .try_into()
            .map_err(|_| TryFromSwapOutcomeError)?;
        let fee = value
            .fee
            .into_bits()
            .try_into()
            .map_err(|_| TryFromSwapOutcomeError)?;
        Ok(Self { amount, fee })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed;

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
