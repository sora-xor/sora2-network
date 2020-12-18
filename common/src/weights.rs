use frame_support::weights::{
    Weight, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use smallvec::smallvec;
use sp_arithmetic::{
    traits::{Saturating, Zero},
    Perbill,
};

use crate::balance::Balance;
use crate::{Fixed, FixedInner};

pub struct WeightToFixedFee(Balance);

impl WeightToFeePolynomial for WeightToFixedFee {
    type Balance = Balance;

    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        smallvec!(WeightToFeeCoefficient {
            coeff_integer: 10_000_000_u32.into(),
            coeff_frac: Perbill::zero(),
            negative: false,
            degree: 1,
        })
    }

    /// Calculates the fee from the passed `weight` according to the `polynomial`.
    ///
    /// Calculation is done in the `Balance` type and never overflows. All evaluation is saturating.
    ///
    /// Specializaiton of the default trait method implementation for `Balance` being a `FixedU128` type.
    fn calc(weight: &Weight) -> Self::Balance {
        Self::polynomial()
            .iter()
            .fold(Self::Balance::zero(), |mut acc, args| {
                let w = Self::Balance::from(Fixed::from_bits(FixedInner::from(*weight)))
                    .saturating_pow(args.degree.into());

                // The sum could get negative. Therefore we only sum with the accumulator.
                // The Perbill Mul implementation is non overflowing.
                let frac = args.coeff_frac * w;
                let integer = args.coeff_integer.saturating_mul(w);

                if args.negative {
                    acc = acc.saturating_sub(frac);
                    acc = acc.saturating_sub(integer);
                } else {
                    acc = acc.saturating_add(frac);
                    acc = acc.saturating_add(integer);
                }

                acc
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{balance::Balance, fixed};
    use frame_support::weights::Weight;
    use sp_runtime::traits::SaturatedConversion;

    type Fee = WeightToFixedFee;

    #[test]
    fn weight_to_fixed_fee_works() {
        assert_eq!(Fee::calc(&100_000_000_000), Balance(fixed!(1)));
        assert_eq!(Fee::calc(&500_000_000), Balance(fixed!(5 e-3)));
        assert_eq!(Fee::calc(&72_000_000), Balance(fixed!(72 e-5)));
        assert_eq!(Fee::calc(&210_200_000_000), Balance(fixed!(2, 102)));
    }

    #[test]
    fn weight_to_fixed_fee_does_not_underflow() {
        assert_eq!(Fee::calc(&0), Balance::saturated_from(0_u32));
    }

    #[test]
    fn weight_to_fixed_fee_does_not_overflow() {
        assert_eq!(
            Fee::calc(&Weight::max_value()),
            Balance(fixed!(184467440, 73709551615)),
        );
    }
}
