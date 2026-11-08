// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! Benchmarks for bridge-halt, with the guardian list filled to its bound.

use super::*;
use alloc::vec;
use common::recovery::BRIDGE_HALT_STORAGE_KEY;
use frame_benchmarking::v2::*;
use frame_support::{
    traits::{EnsureOrigin, Get},
    BoundedVec,
};
use frame_system::RawOrigin;

fn fill_guardians<T: Config>(count: u32) {
    let mut guardians = Guardians::<T>::get();
    for index in 0..count {
        let _ = guardians.try_insert(account("guardian", index, 0));
    }
    Guardians::<T>::put(guardians);
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn halt() {
        fill_guardians::<T>(T::MaxGuardians::get());
        let guardian: T::AccountId = account("guardian", 0, 0);
        sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);
        let reason: ReasonOf<T> =
            BoundedVec::truncate_from(vec![b'x'; T::MaxReasonLength::get() as usize]);

        #[extrinsic_call]
        _(RawOrigin::Signed(guardian), reason);

        assert!(Pallet::<T>::is_halted());
    }

    #[benchmark]
    fn resume() -> Result<(), BenchmarkError> {
        sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
        let origin =
            T::ResumeOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin);

        assert!(!Pallet::<T>::is_halted());
        Ok(())
    }

    #[benchmark]
    fn add_guardian() -> Result<(), BenchmarkError> {
        fill_guardians::<T>(T::MaxGuardians::get().saturating_sub(1));
        let new: T::AccountId = account("new-guardian", 0, 0);
        let origin =
            T::GuardianOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, new.clone());

        assert!(Pallet::<T>::is_guardian(&new));
        Ok(())
    }

    #[benchmark]
    fn remove_guardian() -> Result<(), BenchmarkError> {
        fill_guardians::<T>(T::MaxGuardians::get());
        let old: T::AccountId = account("guardian", 0, 0);
        let origin =
            T::GuardianOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;

        #[extrinsic_call]
        _(origin as T::RuntimeOrigin, old.clone());

        assert!(!Pallet::<T>::is_guardian(&old));
        Ok(())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
