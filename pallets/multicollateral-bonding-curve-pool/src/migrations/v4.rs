use crate::Config;
use crate::Pallet;
use common::AssetIdOf;
use common::Balance;
use frame_support::pallet_prelude::*;
use frame_support::traits::Get;
use frame_support::traits::OnRuntimeUpgrade;
use frame_system::pallet_prelude::BlockNumberFor;
use log::{error, info};
use sp_runtime::traits::One;
#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::prelude::Vec;

pub mod old_storage {
    use super::*;
    #[frame_support::storage_alias]
    pub type PendingFreeReserves<T: Config> =
        StorageValue<Pallet<T>, Vec<(AssetIdOf<T>, Balance)>, ValueQuery>;
}

pub struct MigrateToV4<T>(core::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for MigrateToV4<T>
where
    T: crate::Config,
{
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        if Pallet::<T>::on_chain_storage_version() != 3 {
            error!(
                "Runtime upgrade executed with wrong storage version, expected 3, got {:?}",
                Pallet::<T>::on_chain_storage_version()
            );
            return <T as frame_system::Config>::DbWeight::get().reads(1);
        }
        info!("Applying migration to version 4: Move free reserves distribution to Hooks");
        let pending_free_reserves = old_storage::PendingFreeReserves::<T>::take()
            .into_iter()
            .collect::<BTreeMap<AssetIdOf<T>, Balance>>();
        if !pending_free_reserves.is_empty() {
            info!("Migrate pending free reserves: {:?}", pending_free_reserves);
            crate::PendingFreeReserves::<T>::insert(
                frame_system::Pallet::<T>::block_number() + BlockNumberFor::<T>::one(),
                pending_free_reserves,
            );
        } else {
            info!("No pending free reserves to migrate");
        }
        StorageVersion::new(4).put::<Pallet<T>>();
        <T as frame_system::Config>::DbWeight::get().reads_writes(2, 2)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
        let state = old_storage::PendingFreeReserves::<T>::get().encode();
        ensure!(
            Pallet::<T>::on_chain_storage_version() == 3,
            TryRuntimeError::Other("must upgrade linearly")
        );
        Ok(state)
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
        ensure!(
            !old_storage::PendingFreeReserves::<T>::exists(),
            TryRuntimeError::Other("Old storage value still present")
        );
        let pending_free_reserves = Vec::<(AssetIdOf<T>, Balance)>::decode(&mut &state[..])
            .map_err(|_| "Failed to decode state")?;
        if pending_free_reserves.is_empty() {
            ensure!(
                crate::PendingFreeReserves::<T>::iter().count() == 0,
                TryRuntimeError::Other("Pending free reserves not empty")
            );
        } else {
            ensure!(
                crate::PendingFreeReserves::<T>::iter().count() == 1,
                TryRuntimeError::Other("Pending free reserves have more than one entry")
            );
            let (block_number, value) = crate::PendingFreeReserves::<T>::iter()
                .next()
                .ok_or("Empty pending free reserves")?;
            ensure!(
                block_number
                    == frame_system::Pallet::<T>::block_number() + BlockNumberFor::<T>::one(),
                TryRuntimeError::Other("Pending free reserves have wrong block number")
            );

            ensure!(
                value
                    == pending_free_reserves
                        .into_iter()
                        .collect::<BTreeMap<AssetIdOf<T>, Balance>>(),
                TryRuntimeError::Other("Pending free reserves have wrong value")
            );
        }
        ensure!(
            Pallet::<T>::on_chain_storage_version() == 4,
            TryRuntimeError::Other("should be upgraded to version 4")
        );
        Ok(())
    }
}
