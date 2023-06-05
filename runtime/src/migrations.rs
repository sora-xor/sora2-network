use crate::*;
use common::{DEXId, XOR, XST, XSTUSD};

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

pub type Migrations = (
    xst::migrations::CustomSyntheticsUpgrade<Runtime>,
    farming::migrations::v3::Migrate<
        Runtime,
        XYKSyntheticPoolAccountList<Runtime>,
        FarmingPoolBlocksToInspect,
    >,
    pswap_distribution::migrations::v2::Migrate<Runtime, XYKSyntheticPoolAccountList<Runtime>>,
    pool_xyk::migrations::v3::XYKPoolUpgrade<Runtime, XYKSyntheticPoolPairs<Runtime>>,
);
