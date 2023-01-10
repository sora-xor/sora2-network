use crate::*;
use frame_support::traits::OnRuntimeUpgrade;

pub type Migrations = (
    EthBridgeMigration,
    PriceToolsMigration,
    pallet_staking::migrations::lock_fix::LockFix<Runtime>,
);

pub struct EthBridgeMigration;

impl OnRuntimeUpgrade for EthBridgeMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration EthBridgeMigration");
        eth_bridge::migration::migrate::<Runtime>();
        <Runtime as frame_system::Config>::BlockWeights::get().max_block
    }
}

pub struct PriceToolsMigration;

impl OnRuntimeUpgrade for PriceToolsMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration PriceToolsMigration");
        price_tools::migration::migrate::<Runtime>();
        <Runtime as frame_system::Config>::BlockWeights::get().max_block
    }
}
