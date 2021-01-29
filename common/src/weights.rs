use frame_support::weights::{
    Weight, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use smallvec::smallvec;
use sp_arithmetic::Perbill;

use crate::balance::Balance;
use crate::fixed_wrapper::FixedWrapper;
use crate::{fixed, Fixed, FixedInner};

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
        let result: FixedWrapper = Self::polynomial().iter().fold(fixed!(0), |mut acc, args| {
            let w = FixedWrapper::from(Fixed::from_bits(FixedInner::from(*weight)))
                .pow(args.degree.into());

            // The sum could get negative. Therefore we only sum with the accumulator.
            // The Perbill Mul implementation is non overflowing.
            let frac = w.clone() * FixedWrapper::from(args.coeff_frac.deconstruct());
            let integer = w * args.coeff_integer;

            if args.negative {
                acc = acc - frac - integer;
            } else {
                acc = acc + frac + integer;
            }

            acc
        });
        result.get().unwrap().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed;
    use frame_support::weights::Weight;

    type Fee = WeightToFixedFee;

    #[test]
    fn weight_to_fixed_fee_works() {
        assert_eq!(Fee::calc(&100_000_000_000), fixed!(1));
        assert_eq!(Fee::calc(&500_000_000), fixed!(0.005));
        assert_eq!(Fee::calc(&72_000_000), fixed!(0.00072));
        assert_eq!(Fee::calc(&210_200_000_000), fixed!(2.102));
    }

    #[test]
    fn weight_to_fixed_fee_does_not_underflow() {
        assert_eq!(Fee::calc(&0), fixed!(0));
    }

    #[test]
    fn weight_to_fixed_fee_does_not_overflow() {
        assert_eq!(
            Fee::calc(&Weight::max_value()),
            fixed!(184467440.73709551615),
        );
    }
}
