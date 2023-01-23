use crate::*;
use frame_support::traits::OnRuntimeUpgrade;

pub type Migrations = ();

pub struct PriceToolsMigration;

impl OnRuntimeUpgrade for PriceToolsMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration PriceToolsMigration");
        price_tools::migration::migrate::<Runtime>();
        <Runtime as frame_system::Config>::BlockWeights::get().max_block
    }
}

pub struct DexManagerMigration;

impl OnRuntimeUpgrade for DexManagerMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration DexManagerMigration");
        dex_manager::migrations::migrate::<Runtime>()
    }
}
