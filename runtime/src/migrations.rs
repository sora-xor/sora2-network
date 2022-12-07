use crate::*;
use frame_support::traits::OnRuntimeUpgrade;
use vested_rewards::migrations::MoveMarketMakerRewardPoolToLiquidityProviderPool;

pub type Migrations = (
    EthBridgeMigration,
    MoveMarketMakerRewardPoolToLiquidityProviderPool<Runtime>,
);

pub struct EthBridgeMigration;

impl OnRuntimeUpgrade for EthBridgeMigration {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Run migration EthBridgeMigration");
        eth_bridge::migration::migrate::<Runtime>();
        <Runtime as frame_system::Config>::BlockWeights::get().max_block
    }
}
