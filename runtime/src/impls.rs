use core::marker::PhantomData;

use frame_support::traits::{Currency, OnUnbalanced};

pub type NegativeImbalanceOf<T> = <<T as pallet_staking::Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub struct OnUnbalancedDemocracySlash<T> {
    _marker: PhantomData<T>,
}

impl<T: frame_system::Config + pallet_staking::Config> OnUnbalanced<NegativeImbalanceOf<T>>
    for OnUnbalancedDemocracySlash<T>
{
    fn on_nonzero_unbalanced(_amount: NegativeImbalanceOf<T>) {}
}
