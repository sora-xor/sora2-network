use common::{
    generate_storage_instance, AssetIdOf, AssetManager, AssetName, AssetSymbol, AssetType,
    BalancePrecision, ContentSource, Description,
};
use frame_support::pallet_prelude::Weight;
use frame_support::pallet_prelude::{StorageMap, StorageVersion, ValueQuery};
use frame_support::traits::Get;
use frame_support::Identity;
use sp_std::collections::btree_set::BTreeSet;

use crate::aliases::AccountIdOf;
use crate::{AccountPools, Config, Pallet};

generate_storage_instance!(PoolXYK, AccountPools);
type OldAccountPools<T> = StorageMap<
    AccountPoolsOldInstance,
    Identity,
    AccountIdOf<T>,
    BTreeSet<AssetIdOf<T>>,
    ValueQuery,
>;

pub fn migrate<T: Config>() -> Weight {
    for (account, target_assets) in OldAccountPools::<T>::drain() {
        #[cfg(feature = "std")]
        {
            println!("{account:?}, {target_assets:?}");
        }
        AccountPools::<T>::insert(
            account,
            <<T as common::Config>::AssetManager as AssetManager<
                T,
                AssetSymbol,
                AssetName,
                BalancePrecision,
                AssetType,
                ContentSource,
                Description,
            >>::GetBaseAssetId::get(),
            target_assets,
        );
    }
    StorageVersion::new(2).put::<Pallet<T>>();
    T::BlockWeights::get().max_block
}

#[cfg(test)]
mod tests {
    use frame_support::traits::GetStorageVersion;
    use hex_literal::hex;
    use sp_std::collections::btree_set::BTreeSet;

    use crate::mock::*;
    use crate::{AccountPools, Pallet};

    use super::OldAccountPools;

    #[test]
    fn test() {
        ExtBuilder::default().build().execute_with(|| {
            let target_asset_a = AssetId::from_bytes(
                hex!("0200000700000000000000000000000000000000000000000000000000000000").into(),
            );
            let target_asset_b = AssetId::from_bytes(
                hex!("0200010700000000000000000000000000000000000000000000000000000000").into(),
            );
            let target_asset_c = AssetId::from_bytes(
                hex!("0200020700000000000000000000000000000000000000000000000000000000").into(),
            );
            OldAccountPools::<Runtime>::insert(
                ALICE(),
                BTreeSet::from([target_asset_a, target_asset_b, target_asset_c]),
            );

            super::migrate::<Runtime>();

            assert_eq!(
                AccountPools::<Runtime>::iter().collect::<Vec<_>>(),
                vec![(
                    ALICE(),
                    GetBaseAssetId::get(),
                    BTreeSet::from([target_asset_a, target_asset_b, target_asset_c])
                )]
            );
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 2);
        });
    }
}
