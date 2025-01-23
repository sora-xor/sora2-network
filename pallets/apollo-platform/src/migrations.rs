use crate::{Config, Pallet, UserBorrowingInfo, UserTotalCollateral};
use common::prelude::Balance;
use frame_support::log::{error, info};
use frame_support::pallet_prelude::*;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::traits::StorageVersion;
use sp_runtime::traits::Zero;

pub struct MigrateToV1<T>(core::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for MigrateToV1<T>
where
    T: Config,
{
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        if Pallet::<T>::on_chain_storage_version() != StorageVersion::new(0) {
            error!(
                "Runtime upgrade executed with wrong storage version, expected 0, got {:?}",
                Pallet::<T>::on_chain_storage_version()
            );
            return <T as frame_system::Config>::DbWeight::get().reads(1);
        }

        info!("Applying migration to version 2: Convert borrowing info to total collateral");

        // Perform migration
        <UserBorrowingInfo<T>>::iter().for_each(|(_, user, old_borrowing_map)| {
            old_borrowing_map
                .iter()
                .for_each(|(collateral_asset, borrow_info)| {
                    let additional_collateral = borrow_info.collateral_amount;

                    if additional_collateral > Balance::zero() {
                        let current_total_collateral =
                            <UserTotalCollateral<T>>::get(&user, collateral_asset)
                                .unwrap_or_else(Zero::zero);

                        let updated_total_collateral =
                            current_total_collateral.saturating_add(additional_collateral);

                        <UserTotalCollateral<T>>::insert(
                            user.clone(),
                            collateral_asset,
                            updated_total_collateral,
                        );
                    }
                });
        });

        let total_migrated_entries = <UserTotalCollateral<T>>::iter().count();
        info!(
            "Migrated {} user total collateral entries",
            total_migrated_entries
        );

        // Update storage version
        StorageVersion::new(2).put::<Pallet<T>>();

        // Calculate and return weight
        <T as frame_system::Config>::DbWeight::get().reads_writes(
            total_migrated_entries as u64 * 2, // read old and new storage
            total_migrated_entries as u64,     // write new storage
        )
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        ensure!(
            Pallet::<T>::on_chain_storage_version() == 1,
            "must upgrade linearly"
        );
        Ok(Vec::new()) // No state needed for pre-upgrade check
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        let total_migrated_entries = <UserTotalCollateral<T>>::iter().count();
        ensure!(
            total_migrated_entries > 0,
            "No entries migrated during upgrade"
        );

        ensure!(
            Pallet::<T>::on_chain_storage_version() == 2,
            "should be upgraded to version 2"
        );
        Ok(())
    }
}
