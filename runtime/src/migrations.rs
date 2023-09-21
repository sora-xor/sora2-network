use crate::*;
use bridge_types::{traits::BridgeApp, GenericNetworkId};
use common::{DEXId, XOR, XST, XSTUSD};

pub struct StakingMigrationV11OldPallet;
impl Get<&'static str> for StakingMigrationV11OldPallet {
    fn get() -> &'static str {
        "BagsList"
    }
}
pub struct EmptyAccountList;

impl Get<Vec<AccountId>> for EmptyAccountList {
    fn get() -> Vec<AccountId> {
        Default::default()
    }
}

pub struct XYKSyntheticPoolPairs<T>(core::marker::PhantomData<T>);

impl<T> Get<Vec<(T::AssetId, T::AssetId, T::DEXId)>> for XYKSyntheticPoolPairs<T>
where
    T: assets::Config + common::Config,
{
    fn get() -> Vec<(T::AssetId, T::AssetId, T::DEXId)> {
        vec![
            (XSTUSD.into(), XST.into(), DEXId::PolkaswapXSTUSD.into()),
            (XSTUSD.into(), XOR.into(), DEXId::PolkaswapXSTUSD.into()),
            (XOR.into(), XSTUSD.into(), DEXId::Polkaswap.into()),
        ]
    }
}

pub struct XYKSyntheticPoolAccountList<T>(core::marker::PhantomData<T>);

impl<T> Get<Vec<(T::AccountId, T::AccountId)>> for XYKSyntheticPoolAccountList<T>
where
    T: pool_xyk::Config,
{
    fn get() -> Vec<(T::AccountId, T::AccountId)> {
        XYKSyntheticPoolPairs::<T>::get()
            .iter()
            .map(|(base_asset, target_asset, _)| {
                pool_xyk::Properties::<T>::get(base_asset, target_asset)
            })
            .filter(|v| v.is_some())
            .map(|v| v.unwrap())
            .collect()
    }
}
/// List of block numbers binded to pools with synthetic assets in Farming pallet
/// This list contains block numbers for main, staging, and test environments
pub struct FarmingPoolBlocksToInspect;

impl Get<Vec<BlockNumber>> for FarmingPoolBlocksToInspect {
    fn get() -> Vec<BlockNumber> {
        vec![230, 362, 501, 550, 632, 832, 1025]
    }
}

pub struct HashiBridgeLockedAssets;

impl Get<Vec<(AssetId, Balance)>> for HashiBridgeLockedAssets {
    fn get() -> Vec<(AssetId, Balance)> {
        let Ok(assets) = EthBridge::get_registered_assets(Some(GetEthNetworkId::get())) else {
            return Weight::zero();
        };
        let Some(bridge_account) = eth_bridge::BridgeAccount::<Runtime>::get(GetEthNetworkId::get()) else {
            return Weight::zero();
        };
        let mut result = vec![];
        for (kind, (asset_id, precision), _) in assets {
            let reserved = if kind.is_owned() {
                Assets::total_issuance(&asset_id)
            } else {
                Assets::total_balance(&asset_id, &bridge_account)
            };
            result.push((asset_id, reserved.unwrap_or_default()));
        }
    }
}

parameter_types! {
    pub const HashiBridgeNetworkId: GenericNetworkId = GenericNetworkId::EVMLegacy(GetEthNetworkId::get());
}

pub type Migrations = (
    farming::migrations::v3::Migrate<
        Runtime,
        XYKSyntheticPoolAccountList<Runtime>,
        FarmingPoolBlocksToInspect,
    >,
    pswap_distribution::migrations::v2::Migrate<Runtime, XYKSyntheticPoolAccountList<Runtime>>,
    pool_xyk::migrations::v3::XYKPoolUpgrade<Runtime, XYKSyntheticPoolPairs<Runtime>>,
    band::migrations::v2::BandUpdateV2<Runtime>,
    bridge_proxy::migrations::init::InitLockedAssets<
        Runtime,
        HashiBridgeLockedAssets,
        HashiBridgeNetworkId,
    >,
);
