use crate::*;
use frame_support::traits::OnRuntimeUpgrade;
#[cfg(not(feature = "private-net"))]
use vested_rewards::migrations::MoveMarketMakerRewardPoolToLiquidityProviderPool;

#[cfg(not(feature = "private-net"))]
pub type Migrations = (
    MoveMarketMakerRewardPoolToLiquidityProviderPool<Runtime>,
    PriceToolsMigration,
    pallet_staking::migrations::lock_fix::LockFix<Runtime>,
    DexManagerMigration,
    SetXSTFloorPrice,
);

// Test and Stage already have this migration applied
#[cfg(feature = "private-net")]
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

pub struct SetXSTFloorPrice;

impl OnRuntimeUpgrade for SetXSTFloorPrice {
    fn on_runtime_upgrade() -> Weight {
        frame_support::log::warn!("Set floor price for XST");
        if let Err(err) = xst::Pallet::<Runtime>::set_synthetic_base_asset_floor_price(
            frame_system::RawOrigin::Root.into(),
            balance!(0.001),
        ) {
            frame_support::log::error!("Failed to set floor price for XST: {:?}", err);
        }
        <Runtime as frame_system::Config>::BlockWeights::get().max_block
    }
}
