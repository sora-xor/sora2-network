use crate::*;
use bridge_types::GenericNetworkId;

pub struct HashiBridgeLockedAssets;

impl Get<Vec<(AssetId, Balance)>> for HashiBridgeLockedAssets {
    fn get() -> Vec<(AssetId, Balance)> {
        let Ok(assets) = EthBridge::get_registered_assets(Some(GetEthNetworkId::get())) else {
            frame_support::log::warn!("Failed to get registered assets, skipping migration");
            return vec![];
        };
        let Some(bridge_account) = eth_bridge::BridgeAccount::<Runtime>::get(GetEthNetworkId::get()) else {
            frame_support::log::warn!("Failed to get Hashi bridge account, skipping migration");
            return vec![];
        };
        let mut result = vec![];
        for (kind, (asset_id, _precision), _) in assets {
            let reserved = if kind.is_owned() {
                Assets::total_issuance(&asset_id)
            } else {
                Assets::total_balance(&asset_id, &bridge_account)
            };
            result.push((asset_id, reserved.unwrap_or_default()));
        }
        result
    }
}

parameter_types! {
    pub const HashiBridgeNetworkId: GenericNetworkId = GenericNetworkId::EVMLegacy(GetEthNetworkId::get());
}

pub type Migrations = (
    bridge_proxy::migrations::init::InitLockedAssets<
        Runtime,
        HashiBridgeLockedAssets,
        HashiBridgeNetworkId,
    >,
);
