#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_benchmarking::BenchmarkError;
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::{One, Zero};

fn default_condition_input<BlockNumber>() -> ConditionInput<BlockNumber>
where
    BlockNumber: Default,
{
    ConditionInput {
        question: b"Will Hydra succeed?".to_vec(),
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
    fn create_condition() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        let metadata = default_condition_input::<BlockNumberFor<T>>();

        #[extrinsic_call]
        create_condition(RawOrigin::Signed(caller), metadata);
        Ok(())
    }

    #[benchmark]
    fn create_market() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        let metadata = default_condition_input::<BlockNumberFor<T>>();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)?;

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
        Ok(())
    }

    #[benchmark]
    fn commit_order() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        GovernanceBonds::<T>::insert(&caller, T::GovernanceBondMinimum::get());
        let metadata = default_condition_input::<BlockNumberFor<T>>();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)?;
        let close = <frame_system::Pallet<T>>::block_number()
            + T::MinMarketDuration::get()
            + BlockNumberFor::<T>::one();
        Pallet::<T>::create_market(
            RawOrigin::Signed(caller.clone()).into(),
            0,
            close,
            T::Balance::zero(),
            None,
        )?;

        let commitment = [1u8; 32];

        #[extrinsic_call]
        commit_order(RawOrigin::Signed(caller), 0, commitment);
        Ok(())
    }

    #[benchmark]
    fn reveal_order() -> Result<(), BenchmarkError> {
        let caller: T::AccountId = whitelisted_caller();
        let metadata = default_condition_input::<BlockNumberFor<T>>();
        Pallet::<T>::create_condition(RawOrigin::Signed(caller.clone()).into(), metadata)?;
        let close = <frame_system::Pallet<T>>::block_number()
            + T::MinMarketDuration::get()
            + BlockNumberFor::<T>::one();
        Pallet::<T>::create_market(
            RawOrigin::Signed(caller.clone()).into(),
            0,
            close,
            T::Balance::zero(),
            None,
        )?;

        let payload = b"BUY:10@50".to_vec();
        let salt = b"benchmark".to_vec();
        let commitment = Pallet::<T>::compute_commitment_hash(&caller, 0, &payload, &salt);

        Pallet::<T>::commit_order(RawOrigin::Signed(caller.clone()).into(), 0, commitment)?;

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
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
    }
}
