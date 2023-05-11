use crate::*;
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

pub struct FarmingPoolBlocksToInspect;

impl Get<Vec<BlockNumber>> for FarmingPoolBlocksToInspect {
    fn get() -> Vec<BlockNumber> {
        vec![230, 362, 501, 550, 632, 832, 1025]
    }
}

pub type Migrations = (
    pallet_staking::migrations::v10::MigrateToV10<Runtime>,
    pallet_staking::migrations::v11::MigrateToV11<Runtime, BagsList, StakingMigrationV11OldPallet>,
    pallet_staking::migrations::v12::MigrateToV12<Runtime>,
    pallet_preimage::migration::v1::Migration<Runtime>,
    pallet_scheduler::migration::v3::MigrateToV4<Runtime>,
    pallet_democracy::migrations::v1::Migration<Runtime>,
    pallet_multisig::migrations::v1::MigrateToV1<Runtime>,
    pallet_scheduler::migration::v4::CleanupAgendas<Runtime>,
    pallet_grandpa::migrations::CleanupSetIdSessionMap<Runtime>,
    pallet_staking::migrations::v13::MigrateToV13<Runtime>,
    pallet_election_provider_multi_phase::migrations::v1::MigrateToV1<Runtime>,
    // We don't need this migration, so pass empty account list
    pallet_balances::migration::MigrateManyToTrackInactive<Runtime, EmptyAccountList>,
    multicollateral_bonding_curve_pool::migrations::v3::MigrateToV3<Runtime>,
    farming::migrations::v3::Migrate<
        Runtime,
        XYKSyntheticPoolAccountList<Runtime>,
        FarmingPoolBlocksToInspect,
    >,
    pswap_distribution::migrations::v2::Migrate<Runtime, XYKSyntheticPoolAccountList<Runtime>>,
    pool_xyk::migrations::v3::XYKPoolUpgrade<Runtime, XYKSyntheticPoolPairs<Runtime>>,
);
