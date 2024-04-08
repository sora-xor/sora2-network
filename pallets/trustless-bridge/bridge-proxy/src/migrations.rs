pub mod init {
    use core::marker::PhantomData;

    use common::AssetIdOf;
    use frame_support::traits::OnRuntimeUpgrade;
    use frame_support::traits::StorageVersion;
    use sp_core::Get;

    use crate::*;

    pub struct InitLockedAssets<T, AssetsList, NetworkId>(PhantomData<(T, AssetsList, NetworkId)>);

    impl<
            T: Config,
            ListAssets: Get<Vec<(AssetIdOf<T>, Balance)>>,
            NetworkId: Get<GenericNetworkId>,
        > OnRuntimeUpgrade for InitLockedAssets<T, ListAssets, NetworkId>
    {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(0) {
                frame_support::log::error!(
                    "Expected storage version 0, found {:?}, skipping migration",
                    StorageVersion::get::<Pallet<T>>()
                );
                return frame_support::weights::Weight::zero();
            }

            frame_support::log::info!("Migrating PswapDistribution to v2");

            let assets = ListAssets::get();
            let network_id = NetworkId::get();
            let mut reads_writes = 0;
            for (asset_id, locked) in assets {
                reads_writes += 1;
                crate::LockedAssets::<T>::insert(network_id, asset_id, locked);
                frame_support::log::debug!("Add locked asset {asset_id:?}: {locked:?}");
            }

            StorageVersion::new(1).put::<Pallet<T>>();

            T::DbWeight::get().reads_writes(reads_writes, reads_writes + 1)
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(0),
                "Wrong storage version before upgrade"
            );
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1),
                "Wrong storage version after upgrade"
            );
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::mock::{new_tester, AssetId, Test};
        use common::{balance, DAI, XOR};

        frame_support::parameter_types! {
            pub const HashiBridgeNetworkId: GenericNetworkId = GenericNetworkId::EVMLegacy(0);

            pub AssetsList: Vec<(AssetId, Balance)> = vec![
                (DAI, balance!(100)),
                (XOR, balance!(1000)),
            ];
        }

        #[test]
        fn test() {
            new_tester().execute_with(|| {
                assert_eq!(StorageVersion::get::<crate::Pallet<Test>>(), 0);
                InitLockedAssets::<Test, AssetsList, HashiBridgeNetworkId>::on_runtime_upgrade();
                assert_eq!(
                    crate::LockedAssets::<Test>::get(GenericNetworkId::EVMLegacy(0), DAI),
                    balance!(100)
                );
                assert_eq!(
                    crate::LockedAssets::<Test>::get(GenericNetworkId::EVMLegacy(0), XOR),
                    balance!(1000)
                );
                assert_eq!(StorageVersion::get::<crate::Pallet<Test>>(), 1);
            });
        }
    }
}
