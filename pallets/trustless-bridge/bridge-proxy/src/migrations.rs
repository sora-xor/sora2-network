pub mod init {
    use core::marker::PhantomData;

    use assets::AssetIdOf;
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

pub mod generic_account_v2 {
    use core::marker::PhantomData;

    use assets::AssetIdOf;
    use frame_support::traits::OnRuntimeUpgrade;
    use frame_support::traits::StorageVersion;
    use sp_core::Get;

    use crate::*;

    pub struct LiberlandGenericAccount<T>(PhantomData<T>);

    impl<T: Config> OnRuntimeUpgrade for LiberlandGenericAccount<T> {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            if StorageVersion::get::<Pallet<T>>() >= StorageVersion::new(2) {
                frame_support::log::error!(
                    "Expected storage version less than 2, found {:?}, skipping migration",
                    StorageVersion::get::<Pallet<T>>()
                );
                return frame_support::weights::Weight::zero();
            }

            frame_support::log::info!("Migrating BridgeProxy to v2");

            let mut reads_writes = 0;

            Transactions::<T>::translate(
                |(_, _), _, bridge_request: OldBridgeRequest<AssetIdOf<T>>| {
                    reads_writes += 1;
                    Some(bridge_request.into())
                },
            );

            frame_support::log::info!(
                "BridgeProxy Migration to v2: {:?} BridgeRequests translated",
                reads_writes
            );

            StorageVersion::new(2).put::<Pallet<T>>();

            T::DbWeight::get().reads_writes(reads_writes, reads_writes)
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1),
                "Wrong storage version before upgrade"
            );
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
                "Wrong storage version after upgrade"
            );
            Ok(())
        }
    }

    #[derive(Clone, RuntimeDebug, Encode, Decode, PartialEq, Eq, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct OldBridgeRequest<AssetId> {
        source: OldGenericAccount,
        dest: OldGenericAccount,
        asset_id: AssetId,
        amount: Balance,
        status: MessageStatus,
        start_timepoint: GenericTimepoint,
        end_timepoint: GenericTimepoint,
        direction: MessageDirection,
    }

    impl<AssetId> Into<BridgeRequest<AssetId>> for OldBridgeRequest<AssetId> {
        fn into(self) -> BridgeRequest<AssetId> {
            BridgeRequest {
                source: self.source.into(),
                dest: self.dest.into(),
                asset_id: self.asset_id,
                amount: self.amount,
                status: self.status,
                start_timepoint: self.start_timepoint,
                end_timepoint: self.end_timepoint,
                direction: self.direction,
            }
        }
    }

    #[allow(clippy::large_enum_variant)]
    #[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
    pub enum OldGenericAccount {
        EVM(H160),
        Sora(MainnetAccountId),
        Parachain(xcm::VersionedMultiLocation),
        Unknown,
        Root,
    }

    impl Into<GenericAccount> for OldGenericAccount {
        fn into(self) -> GenericAccount {
            match self {
                OldGenericAccount::EVM(account) => GenericAccount::EVM(account),
                OldGenericAccount::Sora(account) => GenericAccount::Sora(account),
                OldGenericAccount::Parachain(account) => GenericAccount::Parachain(account),
                OldGenericAccount::Unknown => GenericAccount::Unknown,
                OldGenericAccount::Root => GenericAccount::Root,
            }
        }
    }
}
