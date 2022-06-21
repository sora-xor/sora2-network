use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

/// Weight functions for ceres_token_locker
pub struct WeightInfo<T>(PhantomData<T>);

impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn lock_tokens() -> Weight {
        (129_400_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(9 as Weight))
            .saturating_add(T::DbWeight::get().writes(6 as Weight))
    }
    fn withdraw_tokens() -> Weight {
        (86_900_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(5 as Weight))
            .saturating_add(T::DbWeight::get().writes(4 as Weight))
    }
    fn change_fee() -> Weight {
        (28_000_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(1 as Weight))
            .saturating_add(T::DbWeight::get().writes(1 as Weight))
    }
}

impl crate::WeightInfo for () {
    fn lock_tokens() -> Weight {
        2 * EXTRINSIC_FIXED_WEIGHT
    }
    fn withdraw_tokens() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn change_fee() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
