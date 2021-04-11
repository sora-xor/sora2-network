use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);

impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn swap() -> Weight {
        (2_920_955_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(15 as Weight))
            .saturating_add(T::DbWeight::get().writes(6 as Weight))
    }
}

impl crate::WeightInfo for () {
    fn swap() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
