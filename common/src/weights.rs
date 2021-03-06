use frame_support::weights::{
    Weight, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use smallvec::smallvec;
use sp_arithmetic::Perbill;

use crate::fixed_wrapper::FixedWrapper;
use crate::primitives::Balance;
use crate::{balance, Fixed, FixedInner};

pub struct WeightToFixedFee(Balance);

impl WeightToFeePolynomial for WeightToFixedFee {
    type Balance = Balance;

    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        smallvec!(WeightToFeeCoefficient {
            coeff_integer: 10_000_000,
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
        let result: FixedWrapper =
            Self::polynomial()
                .iter()
                .fold(FixedWrapper::from(0u128), |mut acc, args| {
                    let w = FixedWrapper::from(Fixed::from_bits(FixedInner::from(*weight)))
                        .pow(args.degree.into());

                    // The sum could get negative. Therefore we only sum with the accumulator.
                    // The Perbill Mul implementation is non overflowing.
                    let frac = w.clone()
                        * FixedWrapper::from(args.coeff_frac.deconstruct() as u128 * balance!(1));
                    let integer = w.clone() * FixedWrapper::from(args.coeff_integer * balance!(1));

                    if args.negative {
                        acc = acc - frac - integer;
                    } else {
                        acc = acc + frac + integer;
                    }
                    acc
                });
        result.into_balance()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::weights::Weight;

    type Fee = WeightToFixedFee;

    #[test]
    fn weight_to_fixed_fee_works() {
        assert_eq!(Fee::calc(&100_000_000_000), balance!(1));
        assert_eq!(Fee::calc(&500_000_000), balance!(0.005));
        assert_eq!(Fee::calc(&72_000_000), balance!(0.00072));
        assert_eq!(Fee::calc(&210_200_000_000), balance!(2.102));
    }

    #[test]
    fn weight_to_fixed_fee_does_not_underflow() {
        assert_eq!(Fee::calc(&0), 0);
    }

    #[test]
    fn weight_to_fixed_fee_does_not_overflow() {
        assert_eq!(Fee::calc(&Weight::max_value()), 184467440737095516150000000);
    }
}
