use crate::*;
use frame_support::traits::OnRuntimeUpgrade;

pub type Migrations = (EthBridgeMigration, XSTPoolMigration);

pub struct EthBridgeMigration;

impl OnRuntimeUpgrade for EthBridgeMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration EthBridgeMigration");
        eth_bridge::migration::migrate::<Runtime>();
        <Runtime as frame_system::Config>::BlockWeights::get().max_block
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
