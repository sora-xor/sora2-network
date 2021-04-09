use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::Weight;

pub struct WeightInfo<T>(PhantomData<T>);

impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn register() -> Weight {
        (783_115_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(9 as Weight))
            .saturating_add(T::DbWeight::get().writes(7 as Weight))
    }
    fn transfer() -> Weight {
        (360_872_000 as Weight).saturating_add(T::DbWeight::get().reads(4 as Weight))
    }
    fn mint() -> Weight {
        (604_771_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(7 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn burn() -> Weight {
        (465_776_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().writes(2 as Weight))
    }
    fn set_non_mintable() -> Weight {
        (177_752_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(2 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
}

impl crate::WeightInfo for () {
    fn register() -> Weight {
        10 * EXTRINSIC_FIXED_WEIGHT
    }
    fn transfer() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn mint() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn burn() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn set_non_mintable() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
