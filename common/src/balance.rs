use crate::Amount;
use crate::Fixed;
use codec::{Compact, CompactAs, Input, WrapperTypeEncode};
use codec::{Decode, Encode};
use cumulus_upward_message::{BalancesMessage, XCMPMessage};
use num_traits::{CheckedNeg, Num};
use polkadot_parachain::primitives::Id as ParaId;
use rococo_runtime::{BalancesCall, ParachainsCall};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::traits::{
    Bounded, CheckedAdd, CheckedDiv, CheckedMul, CheckedShl, CheckedShr, CheckedSub,
    IntegerSquareRoot, One, Saturating, Unsigned, Zero,
};
use sp_arithmetic::FixedPointNumber;
use sp_core::crypto::AccountId32;
use sp_runtime::FixedPointOperand;
use sp_std::convert::TryFrom;
use sp_std::ops::*;
use sp_std::vec::Vec;
use static_assertions::{assert_eq_align, assert_eq_size};

/// Fixed-point balance type.
///
/// Note: some operations like `Shl` and `integer_sqrt_checked` are not implemented yet.
#[derive(Debug, Clone, Copy, Encode, Decode, Default, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Balance(pub Fixed);

impl From<Fixed> for Balance {
    fn from(fixed: Fixed) -> Self {
        Self(fixed)
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
        Self(self.0 + rhs.0)
    }
}

impl Mul for Balance {
    type Output = Balance;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl Div for Balance {
    type Output = Balance;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl Sub for Balance {
    type Output = Balance;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
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
        *self = Self(self.0 + rhs.0);
    }
}

impl SubAssign for Balance {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self(self.0 - rhs.0);
    }
}

impl MulAssign for Balance {
    fn mul_assign(&mut self, rhs: Self) {
        *self = Self(self.0 * rhs.0);
    }
}

impl DivAssign for Balance {
    fn div_assign(&mut self, rhs: Self) {
        *self = Self(self.0 / rhs.0);
    }
}

impl RemAssign for Balance {
    fn rem_assign(&mut self, rhs: Self) {
        *self = *self % rhs;
    }
}

impl Bounded for Balance {
    fn min_value() -> Self {
        Self(Fixed::min_value())
    }

    fn max_value() -> Self {
        Self(Fixed::max_value())
    }
}

impl Zero for Balance {
    fn zero() -> Self {
        Self(Fixed::zero())
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl One for Balance {
    fn one() -> Self {
        Self(Fixed::one())
    }

    fn is_one(&self) -> bool {
        self.0.is_one()
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

impl CheckedAdd for Balance {
    fn checked_add(&self, rhs: &Self) -> Option<Self> {
        self.0.checked_add(&rhs.0).map(Self)
    }
}

impl CheckedSub for Balance {
    fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        self.0.checked_sub(&rhs.0).map(Self)
    }
}

impl CheckedMul for Balance {
    fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        self.0.checked_mul(&rhs.0).map(Self)
    }
}

impl CheckedDiv for Balance {
    fn checked_div(&self, rhs: &Self) -> Option<Self> {
        self.0.checked_div(&rhs.0).map(Self)
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
        Self(self.0.saturating_add(rhs.0))
    }

    fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    fn saturating_mul(self, rhs: Self) -> Self {
        Self(self.0.saturating_mul(rhs.0))
    }

    fn saturating_pow(self, exp: usize) -> Self {
        Self(self.0.saturating_pow(exp))
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

macro_rules! impl_primitive_conversion {
    ($t:ty) => {
        impl From<$t> for Balance {
            fn from(v: $t) -> Balance {
                Balance(Fixed::from(v as <Fixed as FixedPointNumber>::Inner))
            }
        }

        impl Into<$t> for Balance {
            fn into(self) -> $t {
                self.0.saturating_mul_int(1 as $t)
            }
        }
    };
}

macro_rules! impl_primitive_conversion_any {
        ($($t:ty)+) => ($(
            impl_primitive_conversion!($t);
        )+)
    }

impl_primitive_conversion_any!(u8 u16 u32 u64 u128);

impl From<usize> for Balance {
    fn from(v: usize) -> Balance {
        Balance(Fixed::from(v as <Fixed as FixedPointNumber>::Inner))
    }
}

impl Into<usize> for Balance {
    fn into(self) -> usize {
        self.0.saturating_mul_int(1u128) as usize
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

/// Custom implementation of `cumulus_upward_message`'s `UpwardMessage`.
/// Basically, it encodes our `Balance` type to `u128`.
pub struct RococoUpwardMessage(cumulus_upward_message::RococoUpwardMessage);

impl BalancesMessage<AccountId32, Balance> for RococoUpwardMessage {
    fn transfer(dest: AccountId32, amount: Balance) -> Self {
        Self(BalancesCall::transfer(dest, amount.0.into_inner()).into())
    }
}

impl XCMPMessage for RococoUpwardMessage {
    fn send_message(dest: ParaId, msg: Vec<u8>) -> Self {
        Self(ParachainsCall::send_xcmp_message(dest, msg).into())
    }
}

impl Deref for RococoUpwardMessage {
    type Target = cumulus_upward_message::RococoUpwardMessage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl WrapperTypeEncode for RococoUpwardMessage {}

impl Decode for RococoUpwardMessage {
    fn decode<I: Input>(value: &mut I) -> Result<Self, codec::Error> {
        Ok(Self(cumulus_upward_message::RococoUpwardMessage::decode(
            value,
        )?))
    }
}

impl From<Compact<Balance>> for Balance {
    fn from(v: Compact<Balance>) -> Self {
        v.0
    }
}

impl CompactAs for Balance {
    type As = <Fixed as FixedPointNumber>::Inner;

    fn encode_as(&self) -> &Self::As {
        // This statically (at compile time) guarantees memory layout
        // equality for `Fixed` and its inner type `Fixed::Inner`.
        assert_eq_size!(Fixed, <Fixed as FixedPointNumber>::Inner);
        assert_eq_align!(Fixed, <Fixed as FixedPointNumber>::Inner);

        // FIXME: create a pull request for adding something like
        // `FixedPointNumber::inner_as_ref` to substrate
        //
        // Safety: `Fixed` is a newtype (`FixedU128(u128)`), so it has memory layout
        //         same as its inner type - `u128`.
        unsafe { sp_std::mem::transmute::<&Fixed, &<Fixed as FixedPointNumber>::Inner>(&self.0) }
    }

    fn decode_from(v: Self::As) -> Self {
        Balance(Fixed::from_inner(v))
    }
}

#[cfg(test)]
mod tests {
    use super::{Balance, CompactAs, FixedPointNumber, One};

    #[test]
    fn balance_encode_as_should_equal_fixed_inner() {
        let balance = Balance::one();
        assert_eq!(balance.0.into_inner(), *balance.encode_as());
    }
}
