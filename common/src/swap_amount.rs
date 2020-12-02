use core::fmt::Debug;
use core::ops::{Div, DivAssign, Mul, MulAssign};
use core::result::Result;

use codec::{Decode, Encode};
use fixnum::{
    impl_op,
    ops::{RoundMode::*, RoundingDiv, RoundingMul},
};
use frame_support::RuntimeDebug;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::mem;

use crate::balance::Balance;
use crate::Fixed;

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
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
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

// TODO: use macros for impl generation
impl<T> Mul<T> for SwapAmount<T>
where
    T: Copy + RoundingMul<Output = T>,
    <T as RoundingMul>::Error: Debug,
{
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        match self {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::with_desired_input(
                desired_amount_in.rmul(rhs, Floor).unwrap(),
                min_amount_out.rmul(rhs, Floor).unwrap(),
            ),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::with_desired_output(
                desired_amount_out.rmul(rhs, Floor).unwrap(),
                max_amount_in.rmul(rhs, Floor).unwrap(),
            ),
        }
    }
}

impl<T> Div<T> for SwapAmount<T>
where
    T: Copy + RoundingDiv<Output = T>,
    <T as RoundingDiv>::Error: Debug,
{
    type Output = Self;

    fn div(self, rhs: T) -> Self::Output {
        match self {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => SwapAmount::with_desired_input(
                desired_amount_in.rdiv(rhs, Floor).unwrap(),
                min_amount_out.rdiv(rhs, Floor).unwrap(),
            ),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => SwapAmount::with_desired_output(
                desired_amount_out.rdiv(rhs, Floor).unwrap(),
                max_amount_in.rdiv(rhs, Floor).unwrap(),
            ),
        }
    }
}

impl<T> MulAssign<T> for SwapAmount<T>
where
    T: Copy + RoundingMul<Output = T>,
    <T as RoundingMul>::Error: Debug,
{
    fn mul_assign(&mut self, rhs: T) {
        match self.clone() {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => mem::replace(
                self,
                SwapAmount::with_desired_input(
                    desired_amount_in.rmul(rhs, Floor).unwrap(),
                    min_amount_out.rmul(rhs, Floor).unwrap(),
                ),
            ),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => mem::replace(
                self,
                SwapAmount::with_desired_output(
                    desired_amount_out.rmul(rhs, Floor).unwrap(),
                    max_amount_in.rmul(rhs, Floor).unwrap(),
                ),
            ),
        };
    }
}

impl<T> DivAssign<T> for SwapAmount<T>
where
    T: Copy + RoundingDiv<Output = T>,
    <T as RoundingDiv>::Error: Debug,
{
    fn div_assign(&mut self, rhs: T) {
        match self.clone() {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => mem::replace(
                self,
                SwapAmount::with_desired_input(
                    desired_amount_in.rdiv(rhs, Floor).unwrap(),
                    min_amount_out.rdiv(rhs, Floor).unwrap(),
                ),
            ),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => mem::replace(
                self,
                SwapAmount::with_desired_output(
                    desired_amount_out.rdiv(rhs, Floor).unwrap(),
                    max_amount_in.rdiv(rhs, Floor).unwrap(),
                ),
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

impl_op!(Balance [rmul] Balance = Balance);
impl_op!(Balance [rdiv] Balance = Balance);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{prelude::Balance, FixedInner};
    use std::convert::TryFrom;

    fn balance(x: FixedInner) -> Balance {
        Balance::from(Fixed::try_from(x).unwrap())
    }

    #[test]
    fn test_mul_amount_should_pass() {
        let swap_amount = SwapAmount::with_desired_input(balance(100), balance(50));
        assert_eq!(
            swap_amount * balance(2),
            SwapAmount::with_desired_input(balance(200), balance(100))
        );
    }

    #[test]
    fn test_mul_assign_amount_should_pass() {
        let swap_amount = SwapAmount::with_desired_input(balance(100), balance(50));
        assert_eq!(
            swap_amount / balance(2),
            SwapAmount::with_desired_input(balance(50), balance(25))
        );
    }

    #[test]
    fn test_div_amount_should_pass() {
        let mut swap_amount = SwapAmount::with_desired_input(balance(100), balance(50));
        swap_amount *= balance(2);
        assert_eq!(
            swap_amount,
            SwapAmount::with_desired_input(balance(200), balance(100))
        );
    }

    #[test]
    fn test_div_assign_amount_should_pass() {
        let mut swap_amount = SwapAmount::with_desired_input(balance(100), balance(50));
        swap_amount /= balance(2);
        assert_eq!(
            swap_amount,
            SwapAmount::with_desired_input(balance(50), balance(25))
        );
    }
}
