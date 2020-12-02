use core::convert::TryInto;
use core::ops::{Shl, Shr};

use codec::{Compact, CompactAs, Decode, Encode, Input, WrapperTypeEncode};
use derive_more::From;
use fixnum::ops::{CheckedAdd, CheckedSub, Numeric, RoundMode::*, RoundingDiv, RoundingMul};
use num_traits::{CheckedNeg, Num, One, Unsigned, Zero};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::{
    Bounded, CheckedDiv, CheckedMul, CheckedShl, CheckedShr, IntegerSquareRoot, Saturating,
};
use sp_core::{crypto::AccountId32, U256};
use sp_runtime::FixedPointOperand;
use sp_std::convert::TryFrom;
use sp_std::fmt::Display;
use sp_std::ops::*;
use sp_std::str::FromStr;

use static_assertions::_core::fmt::Formatter;
use static_assertions::{assert_eq_align, assert_eq_size};

use crate::{Amount, Fixed, FixedInner};

/// Fixed-point balance type.
#[derive(Encode, Debug, Clone, Copy, Decode, Default, From, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Balance(pub Fixed);

/// Error type for conversion.
#[derive(Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// Overflow encountered.
    Overflow,
    /// Input is not acceptable for conversion.
    Invalid,
}

type Inner = i128;

#[cfg(feature = "std")]
impl FromStr for Balance {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Balance(s.parse()?))
    }
}

#[cfg(feature = "std")]
impl Display for Balance {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Rem for Balance {
    type Output = Balance;

    /// Division always occurs without a remainder.
    fn rem(self, _: Self) -> Self::Output {
        Balance::zero()
    }
}

impl Add for Balance {
    type Output = Balance;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.cadd(rhs.0).unwrap())
    }
}

impl Mul for Balance {
    type Output = Balance;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0.rmul(rhs.0, Floor).unwrap())
    }
}

impl Div for Balance {
    type Output = Balance;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0.rdiv(rhs.0, Floor).unwrap())
    }
}

impl Sub for Balance {
    type Output = Balance;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.csub(rhs.0).unwrap())
    }
}

impl Shl<u32> for Balance {
    type Output = Balance;

    fn shl(self, _rhs: u32) -> Self::Output {
        // TODO: implement `Shl` for `Balance`
        self
    }
}

impl Shr<u32> for Balance {
    type Output = Balance;

    fn shr(self, _rhs: u32) -> Self::Output {
        // TODO: implement `Shr` for `Balance`
        self
    }
}

impl AddAssign for Balance {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self(self.0.cadd(rhs.0).unwrap());
    }
}

impl SubAssign for Balance {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self(self.0.csub(rhs.0).unwrap());
    }
}

impl MulAssign for Balance {
    fn mul_assign(&mut self, rhs: Self) {
        *self = Self(self.0.rmul(rhs.0, Floor).unwrap());
    }
}

impl DivAssign for Balance {
    fn div_assign(&mut self, rhs: Self) {
        *self = Self(self.0.rdiv(rhs.0, Floor).unwrap());
    }
}

impl RemAssign for Balance {
    fn rem_assign(&mut self, rhs: Self) {
        *self = *self % rhs;
    }
}

impl Bounded for Balance {
    fn min_value() -> Self {
        Self(Fixed::MIN)
    }

    fn max_value() -> Self {
        Self(Fixed::MAX)
    }
}

impl Zero for Balance {
    fn zero() -> Self {
        const ZERO: Fixed = Fixed::from_bits(0);
        Self(ZERO)
    }

    fn is_zero(&self) -> bool {
        const ZERO: Fixed = Fixed::from_bits(0);
        self.0 == ZERO
    }
}

impl One for Balance {
    fn one() -> Self {
        Self(1u32.try_into().unwrap())
    }

    fn is_one(&self) -> bool {
        self.0 == 1u32.try_into().unwrap()
    }
}

impl IntegerSquareRoot for Balance {
    fn integer_sqrt_checked(&self) -> Option<Self>
    where
        Self: Sized,
    {
        // TODO: implement `IntegerSquareRoot` for `Balance`
        None
    }
}

impl sp_arithmetic::traits::CheckedAdd for Balance {
    fn checked_add(&self, rhs: &Self) -> Option<Self> {
        self.0.cadd(rhs.0).map(Self).ok()
    }
}

impl sp_arithmetic::traits::CheckedSub for Balance {
    fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        self.0.csub(rhs.0).map(Self).ok()
    }
}

impl CheckedMul for Balance {
    fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        self.0.rmul(rhs.0, Floor).map(Self).ok()
    }
}

impl CheckedDiv for Balance {
    fn checked_div(&self, rhs: &Self) -> Option<Self> {
        self.0.rdiv(rhs.0, Floor).map(Self).ok()
    }
}

impl CheckedShl for Balance {
    fn checked_shl(&self, _rhs: u32) -> Option<Self> {
        // TODO: implement `CheckedShl` for Balance
        None
    }
}

impl CheckedShr for Balance {
    fn checked_shr(&self, _rhs: u32) -> Option<Self> {
        // TODO: implement `CheckedShr` for Balance
        None
    }
}

impl Saturating for Balance {
    fn saturating_add(self, rhs: Self) -> Self {
        unimplemented!() // TODO
    }

    fn saturating_sub(self, rhs: Self) -> Self {
        unimplemented!() // TODO
    }

    fn saturating_mul(self, rhs: Self) -> Self {
        unimplemented!() // TODO
    }

    fn saturating_pow(self, exp: usize) -> Self {
        unimplemented!() // TODO
    }
}

impl Num for Balance {
    type FromStrRadixErr = ();

    fn from_str_radix(_str: &str, _radix: u32) -> Result<Self, Self::FromStrRadixErr> {
        // TODO: implement `Num` for `Balance`
        Err(())
    }
}

impl Unsigned for Balance {}

macro_rules! impl_primitive_conversions {
    ($($t:ty)+) => ($(
        impl_primitive_conversions!{@single $t}
    )*);
    (@single $t:ty) => {
        impl From<$t> for Balance {
            fn from(v: $t) -> Balance {
                Balance(Fixed::from_bits(v as FixedInner))
            }
        }

        impl Into<$t> for Balance {
            fn into(self) -> $t {
                *self.0.as_bits() as $t
            }
        }
    };
}

impl_primitive_conversions!(u8 u16 u32 u64 u128);

impl From<usize> for Balance {
    fn from(x: usize) -> Balance {
        Balance(Fixed::try_from(x as FixedInner).unwrap())
    }
}

impl Into<usize> for Balance {
    fn into(self) -> usize {
        *self.0.as_bits() as usize // TODO
    }
}

impl Into<Amount> for Balance {
    fn into(self) -> Amount {
        <Self as Into<u64>>::into(self) as i128
    }
}

impl TryFrom<Amount> for Balance {
    type Error = ();

    fn try_from(amount: Amount) -> Result<Self, Self::Error> {
        if amount < 0 {
            Err(())
        } else {
            Ok(Self::from(amount as u128))
        }
    }
}

impl CheckedNeg for Balance {
    fn checked_neg(&self) -> Option<Self> {
        None
    }
}

impl FixedPointOperand for Balance {}

impl From<Compact<Balance>> for Balance {
    fn from(v: Compact<Balance>) -> Self {
        v.0
    }
}

impl CompactAs for Balance {
    type As = Inner;

    fn encode_as(&self) -> &Self::As {
        // This statically (at compile time) guarantees memory layout
        // equality for `Fixed` and its inner type `Fixed::Inner`.
        assert_eq_size!(Fixed, Inner);
        assert_eq_align!(Fixed, Inner);

        // FIXME: create a pull request for adding something like
        // `FixedPointNumber::inner_as_ref` to substrate
        //
        // Safety: `Fixed` is a newtype (`FixedU128(u128)`), so it has memory layout
        //         same as its inner type - `u128`.
        unsafe { sp_std::mem::transmute::<&Fixed, &Inner>(&self.0) }
    }

    fn decode_from(v: Self::As) -> Self {
        Balance(Fixed::from_bits(v))
    }
}

#[cfg(test)]
mod tests {
    use codec::{Compact, CompactAs, Decode, Encode, Input, WrapperTypeEncode};

    use crate::fixed;

    use super::Balance;

    #[test]
    fn balance_encode_as_should_equal_fixed_inner() {
        let balance: Balance = fixed!(1).into();
        assert_eq!(balance.0.as_bits(), balance.encode_as());
    }
}
