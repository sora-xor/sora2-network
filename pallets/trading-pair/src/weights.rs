use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn register() -> Weight {
        (240_519_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(4 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
}

impl crate::WeightInfo for () {
    fn register() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
