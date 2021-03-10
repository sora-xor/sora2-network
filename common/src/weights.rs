use frame_support::weights::{
    WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use smallvec::smallvec;
use sp_arithmetic::Perbill;

use crate::primitives::Balance;
pub mod constants {
    use frame_support::weights::Weight;

    pub const EXTRINSIC_FIXED_WEIGHT: Weight = 100_000_000;
}

pub struct PresetWeightInfo;

pub struct WeightToFixedFee;

impl WeightToFeePolynomial for WeightToFixedFee {
    type Balance = Balance;

    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        smallvec!(WeightToFeeCoefficient {
            coeff_integer: 7_000_000,
            coeff_frac: Perbill::zero(),
            negative: false,
            degree: 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{super::balance, *};
    use frame_support::weights::Weight;

    type Fee = WeightToFixedFee;

    #[test]
    fn weight_to_fixed_fee_works() {
        assert_eq!(Fee::calc(&100_000_000_000), balance!(0.7));
        assert_eq!(Fee::calc(&500_000_000), balance!(0.0035));
        assert_eq!(Fee::calc(&72_000_000), balance!(0.000504));
        assert_eq!(Fee::calc(&210_200_000_000), balance!(1.4714));
    }

    #[test]
    fn weight_to_fixed_fee_does_not_underflow() {
        assert_eq!(Fee::calc(&0), 0);
    }

    #[test]
    fn weight_to_fixed_fee_does_not_overflow() {
        assert_eq!(Fee::calc(&Weight::max_value()), 129127208515966861305000000);
    }
}
