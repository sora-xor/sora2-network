use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn swap_pair() -> Weight {
        (2_785_805_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(15 as Weight))
            .saturating_add(T::DbWeight::get().writes(6 as Weight))
    }
    fn deposit_liquidity() -> Weight {
        (2_532_754_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(18 as Weight))
            .saturating_add(T::DbWeight::get().writes(4 as Weight))
    }
    fn withdraw_liquidity() -> Weight {
        (2_114_113_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(15 as Weight))
            .saturating_add(T::DbWeight::get().writes(4 as Weight))
    }
    fn initialize_pool() -> Weight {
        (1_828_872_000 as Weight)
            .saturating_add(T::DbWeight::get().reads(15 as Weight))
            .saturating_add(T::DbWeight::get().writes(15 as Weight))
    }
}

impl crate::WeightInfo for () {
    fn swap_pair() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn deposit_liquidity() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn withdraw_liquidity() -> Weight {
        10 * EXTRINSIC_FIXED_WEIGHT
    }
    fn initialize_pool() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
