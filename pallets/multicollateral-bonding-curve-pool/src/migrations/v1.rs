use crate::Pallet;
use common::XST;
use frame_support::traits::Get;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::{
    log::{error, info},
    pallet_prelude::StorageVersion,
    traits::GetStorageVersion as _,
};

pub struct InitializeXSTPool<T>(core::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for InitializeXSTPool<T>
where
    T: crate::Config,
{
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        if Pallet::<T>::on_chain_storage_version() == 0 {
            info!("Applying migration to version 1: Initialize XST pool");
            match Pallet::<T>::initialize_pool_unchecked(XST.into(), false) {
                Ok(()) => StorageVersion::new(1).put::<Pallet<T>>(),
                // We can't return an error here, so we just log it
                Err(err) => error!(
                    "An error occurred during XST pool initialization: {:?}",
                    err
                ),
            }
            <T as frame_system::Config>::BlockWeights::get().max_block
        } else {
            error!(
                "Runtime upgrade executed with wrong storage version, expected 0, got {:?}",
                Pallet::<T>::on_chain_storage_version()
            );
            <T as frame_system::Config>::DbWeight::get().reads(1)
        }
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<(), &'static str> {
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 0,
            "must upgrade linearly"
        );
        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade() -> Result<(), &'static str> {
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 1,
            "should be upgraded to version 1"
        );
        Ok(())
    }
}
