use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);

impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn lock_liquidity() -> Weight {
        Weight::zero()
    }
    fn change_ceres_fee() -> Weight {
        Weight::zero()
    }
}

impl crate::WeightInfo for () {
    fn lock_liquidity() -> Weight {
        EXTRINSIC_FIXED_WEIGHT.mul(2)
    }
    fn change_ceres_fee() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
