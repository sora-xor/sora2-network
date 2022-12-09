use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

/// Weight functions for ceres_token_locker
pub struct WeightInfo<T>(PhantomData<T>);

impl<T: frame_system::Config> crate::WeightInfo for WeightInfo<T> {
    fn lock_tokens() -> Weight {
        Weight::zero()
    }
    fn withdraw_tokens() -> Weight {
        Weight::zero()
    }
    fn change_fee() -> Weight {
        Weight::zero()
    }
}

impl crate::WeightInfo for () {
    fn lock_tokens() -> Weight {
        EXTRINSIC_FIXED_WEIGHT.mul(2)
    }
    fn withdraw_tokens() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn change_fee() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}
