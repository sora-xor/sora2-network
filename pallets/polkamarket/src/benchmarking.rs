#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::{One, Zero};

fn default_condition_input<BlockNumber>() -> ConditionInput<BlockNumber>
where
    BlockNumber: Default,
{
    ConditionInput {
        question: b"Will the Polkamarket beta succeed in 2025?".to_vec(),
        oracle: b"Chainlink".to_vec(),
        resolution_source: b"https://oracle.example.com".to_vec(),
        submission_deadline: BlockNumber::default(),
    }
}

#[benchmarks(where
    T::AccountId: From<<T as frame_system::Config>::AccountId>,
)]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn create_condition() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        let metadata = default_condition_input::<BlockNumberFor<T>>();

        #[extrinsic_call]
        create_condition(RawOrigin::Signed(caller), metadata);
    }

    #[benchmark]
    fn create_market() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        let metadata = default_condition_input::<BlockNumberFor<T>>();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata).unwrap();

        let close = <frame_system::Pallet<T>>::block_number()
            + T::MinMarketDuration::get()
            + BlockNumberFor::<T>::one();

        #[extrinsic_call]
        create_market(
            RawOrigin::Signed(caller),
            0,
            close,
            T::Balance::zero(),
            None,
        );
    }

    #[benchmark]
    fn commit_order() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        let metadata = default_condition_input::<BlockNumberFor<T>>();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata).unwrap();
        let close = <frame_system::Pallet<T>>::block_number()
            + T::MinMarketDuration::get()
            + BlockNumberFor::<T>::one();
        Pallet::<T>::create_market(
            RawOrigin::Signed(caller.clone()).into(),
            0,
            close,
            T::Balance::zero(),
            None,
        )
        .unwrap();

        let commitment = [1u8; 32];

        #[extrinsic_call]
        commit_order(RawOrigin::Signed(caller), 0, commitment);
    }

    #[benchmark]
    fn reveal_order() {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        let metadata = default_condition_input::<BlockNumberFor<T>>();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata).unwrap();
        let close = <frame_system::Pallet<T>>::block_number()
            + T::MinMarketDuration::get()
            + BlockNumberFor::<T>::one();
        Pallet::<T>::create_market(
            RawOrigin::Signed(caller.clone()).into(),
            0,
            close,
            T::Balance::zero(),
            None,
        )
        .unwrap();

        let payload = b"BUY:10@50".to_vec();
        let salt = b"benchmark".to_vec();
        let commitment = Pallet::<T>::compute_commitment_hash(&caller, 0, &payload, &salt);

        Pallet::<T>::commit_order(RawOrigin::Signed(caller.clone()).into(), 0, commitment).unwrap();

        let now = <frame_system::Pallet<T>>::block_number()
            + T::CommitmentRevealDelay::get()
            + BlockNumberFor::<T>::one();
        <frame_system::Pallet<T>>::set_block_number(now);

        #[extrinsic_call]
        reveal_order(
            RawOrigin::Signed(caller),
            0,
            payload,
            salt,
            T::Balance::one(),
        );
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
    }
}
