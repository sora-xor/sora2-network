use crate::*;
use frame_support::traits::OnRuntimeUpgrade;
use vested_rewards::migrations::MoveMarketMakerRewardPoolToLiquidityProviderPool;

pub type Migrations = (
    MoveMarketMakerRewardPoolToLiquidityProviderPool<Runtime>,
    PriceToolsMigration,
    pallet_staking::migrations::lock_fix::LockFix<Runtime>,
    DexManagerMigration,
    XSTPoolMigration,
);

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

pub struct XSTPoolMigration;

impl OnRuntimeUpgrade for XSTPoolMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration XSTPoolMigration");
        xst::migrations::migrate::<Runtime>();
        <Runtime as frame_system::Config>::BlockWeights::get().max_block
    }
}
