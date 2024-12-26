use crate::{Config, UserBorrowingInfo, UserTotalCollateral};
use common::prelude::Balance;
use frame_support::log;
use sp_runtime::traits::Zero;

/// Migration to convert existing borrowing information to the new UserTotalCollateral storage
pub fn migrate<T: Config>() -> Result<(), &'static str> {
    // Create a new storage for total collateral
    <UserBorrowingInfo<T>>::iter().for_each(|(_, user, old_borrowing_map)| {
        old_borrowing_map
            .iter()
            .for_each(|(collateral_asset, borrow_info)| {
                // Calculate total collateral for this user and asset
                let additional_collateral = borrow_info.collateral_amount;

                // Skip if there's no additional collateral amount
                if additional_collateral > Balance::zero() {
                    // Retrieve the current total collateral if it exists
                    let current_total_collateral =
                        <UserTotalCollateral<T>>::get(&user, collateral_asset)
                            .unwrap_or_else(Zero::zero);

                    // Add the additional collateral to the current total
                    let updated_total_collateral =
                        current_total_collateral.saturating_add(additional_collateral);

                    // Update or insert the new total collateral amount
                    <UserTotalCollateral<T>>::insert(
                        user.clone(),
                        collateral_asset,
                        updated_total_collateral,
                    );
                }
            });
    });

    let total_migrated_entries = <UserTotalCollateral<T>>::iter().count();
    log::info!(
        "Migrated {} user total collateral entries",
        total_migrated_entries
    );

    Ok(())
}
