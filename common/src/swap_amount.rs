use codec::{Decode, Encode};
use core::ops::{Div, DivAssign, Mul, MulAssign};
use frame_support::RuntimeDebug;
use sp_std::mem;

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
