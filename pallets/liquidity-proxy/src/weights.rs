use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

use common::prelude::SwapVariant;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn swap(amount: SwapVariant) -> Weight {
        match amount {
            SwapVariant::WithDesiredInput => (26_026_726_000 as Weight)
                .saturating_add(T::DbWeight::get().reads(45 as Weight))
                .saturating_add(T::DbWeight::get().writes(11 as Weight)),
            _ => (45_803_872_000 as Weight)
                .saturating_add(T::DbWeight::get().reads(45 as Weight))
                .saturating_add(T::DbWeight::get().writes(11 as Weight)),
        }
    }
}

impl crate::WeightInfo for () {
    fn swap(_amount: SwapVariant) -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
