use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);

impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn lock_liquidity() -> Weight {
        (125_300_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn change_ceres_fee() -> Weight {
        (16_800_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
}

impl crate::WeightInfo for () {
    fn lock_liquidity() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn change_ceres_fee() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
