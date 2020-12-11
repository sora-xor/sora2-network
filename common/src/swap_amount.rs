use crate::balance::Balance;
use crate::Fixed;
use codec::{Decode, Encode};
use core::ops::{Div, DivAssign, Mul, MulAssign};
use frame_support::RuntimeDebug;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::mem;

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
#[derive(Encode, Decode, Clone, Copy, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
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

impl From<SwapAmount<Fixed>> for SwapAmount<Balance> {
    fn from(v: SwapAmount<Fixed>) -> Self {
        match v {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::WithDesiredInput {
                desired_amount_in: desired_amount_in.into(),
                min_amount_out: min_amount_out.into(),
            },
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::WithDesiredOutput {
                desired_amount_out: desired_amount_out.into(),
                max_amount_in: max_amount_in.into(),
            },
        }
    }
}

impl From<SwapAmount<Balance>> for SwapAmount<Fixed> {
    fn from(v: SwapAmount<Balance>) -> Self {
        match v {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::WithDesiredInput {
                desired_amount_in: desired_amount_in.0,
                min_amount_out: min_amount_out.0,
            },
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::WithDesiredOutput {
                desired_amount_out: desired_amount_out.0,
                max_amount_in: max_amount_in.0,
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
impl<T: Mul<Output = T> + Clone + Copy> Mul<T> for SwapAmount<T> {
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        match self {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::with_desired_input(desired_amount_in * rhs, min_amount_out * rhs),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::with_desired_output(desired_amount_out * rhs, max_amount_in * rhs),
        }
    }
}

impl<T: Div<Output = T> + Clone + Copy> Div<T> for SwapAmount<T> {
    type Output = Self;

    fn div(self, rhs: T) -> Self::Output {
        match self {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::with_desired_input(desired_amount_in / rhs, min_amount_out / rhs),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::with_desired_output(desired_amount_out / rhs, max_amount_in / rhs),
        }
    }
}

impl<T: Mul<Output = T> + Clone + Copy> MulAssign<T> for SwapAmount<T> {
    fn mul_assign(&mut self, rhs: T) {
        match self.clone() {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => mem::replace(
                self,
                SwapAmount::with_desired_input(desired_amount_in * rhs, min_amount_out * rhs),
            ),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => mem::replace(
                self,
                SwapAmount::with_desired_output(desired_amount_out * rhs, max_amount_in * rhs),
            ),
        };
    }
}

impl<T: Div<Output = T> + Clone + Copy> DivAssign<T> for SwapAmount<T> {
    fn div_assign(&mut self, rhs: T) {
        match self.clone() {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => mem::replace(
                self,
                SwapAmount::with_desired_input(desired_amount_in / rhs, min_amount_out / rhs),
            ),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => mem::replace(
                self,
                SwapAmount::with_desired_output(desired_amount_out / rhs, max_amount_in / rhs),
            ),
        };
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

impl From<SwapOutcome<Balance>> for SwapOutcome<Fixed> {
    fn from(v: SwapOutcome<Balance>) -> Self {
        match v {
            SwapOutcome { amount, fee } => SwapOutcome {
                amount: amount.0,
                fee: fee.0,
            },
        }
    }
}

impl From<SwapOutcome<Fixed>> for SwapOutcome<Balance> {
    fn from(v: SwapOutcome<Fixed>) -> Self {
        match v {
            SwapOutcome { amount, fee } => SwapOutcome {
                amount: amount.into(),
                fee: fee.into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::Balance;

    #[test]
    fn test_mul_amount_should_pass() {
        let swap_amount =
            SwapAmount::with_desired_input(Balance::from(100u128), Balance::from(50u128));
        assert_eq!(
            swap_amount * Balance::from(2u128),
            SwapAmount::with_desired_input(Balance::from(200u128), Balance::from(100u128))
        );
    }

    #[test]
    fn test_mul_assign_amount_should_pass() {
        let swap_amount =
            SwapAmount::with_desired_input(Balance::from(100u128), Balance::from(50u128));
        assert_eq!(
            swap_amount / Balance::from(2u128),
            SwapAmount::with_desired_input(Balance::from(50u128), Balance::from(25u128))
        );
    }

    #[test]
    fn test_div_amount_should_pass() {
        let mut swap_amount =
            SwapAmount::with_desired_input(Balance::from(100u128), Balance::from(50u128));
        swap_amount *= Balance::from(2u128);
        assert_eq!(
            swap_amount,
            SwapAmount::with_desired_input(Balance::from(200u128), Balance::from(100u128))
        );
    }

    #[test]
    fn test_div_assign_amount_should_pass() {
        let mut swap_amount =
            SwapAmount::with_desired_input(Balance::from(100u128), Balance::from(50u128));
        swap_amount /= Balance::from(2u128);
        assert_eq!(
            swap_amount,
            SwapAmount::with_desired_input(Balance::from(50u128), Balance::from(25u128))
        );
    }
}
