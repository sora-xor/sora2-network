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
                let total_collateral = borrow_info.collateral_amount;

                // Only insert if there's a non-zero collateral amount
                if total_collateral > Balance::zero() {
                    <UserTotalCollateral<T>>::insert(
                        user.clone(),
                        collateral_asset,
                        total_collateral,
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
