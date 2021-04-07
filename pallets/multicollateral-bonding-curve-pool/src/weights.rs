use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn initialize_pool() -> Weight {
        (65_100_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(6 as Weight))
            .saturating_add(T::DbWeight::get().writes(3 as Weight))
    }
    fn set_reference_asset() -> Weight {
        (36_800_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(3 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn set_optional_reward_multiplier() -> Weight {
        (44_700_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(5 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
    fn claim_incentives() -> Weight {
        (100_300_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(10 as Weight))
            .saturating_add(T::DbWeight::get().writes(5 as Weight))
    }
}

impl crate::WeightInfo for () {
    fn initialize_pool() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn set_reference_asset() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn set_optional_reward_multiplier() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn claim_incentives() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
