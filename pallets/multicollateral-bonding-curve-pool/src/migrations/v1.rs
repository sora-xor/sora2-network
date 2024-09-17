use crate::Pallet;
use common::XST;
use frame_support::traits::Get;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::{pallet_prelude::StorageVersion, traits::GetStorageVersion as _};
use log::{error, info};
#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;
#[cfg(feature = "try-runtime")]
use sp_std::prelude::*;

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
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 0,
            "must upgrade linearly"
        );
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), TryRuntimeError> {
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 1,
            TryRuntimeError::Other("should be upgraded to version 1")
        );
        Ok(())
    }
}
