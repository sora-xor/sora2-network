use crate::*;
use core::marker::PhantomData;
use frame_support::traits::OnRuntimeUpgrade;

pub mod v2 {
    use frame_support::traits::StorageVersion;

    use super::*;

    // You need to provide list of pools with creation block number
    pub struct Migrate<T, G>(PhantomData<(T, G)>);

    impl<T, G> OnRuntimeUpgrade for Migrate<T, G>
    where
        T: Config,
        G: Get<Vec<(T::AccountId, T::BlockNumber)>>,
    {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(1) {
                frame_support::log::error!(
                    "Expected storage version 1, found {:?}, skipping migration",
                    StorageVersion::get::<Pallet<T>>()
                );
            }
            let pools = G::get();
            for (pool_account, block) in pools {
                Pools::<T>::mutate(block % T::REFRESH_FREQUENCY, |pools| {
                    if !pools.contains(&pool_account) {
                        frame_support::log::info!(
                            "Add pool {pool_account:?} at block {block:?} to farming"
                        );
                        pools.push(pool_account);
                    } else {
                        frame_support::log::info!(
                            "Skip {pool_account:?} at block {block:?}, already exist"
                        );
                    }
                });
            }
            StorageVersion::new(2).put::<Pallet<T>>();
            <T as frame_system::Config>::BlockWeights::get().max_block
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<(), &'static str> {
            ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1),
                "Wrong storage version before upgrade"
            );
            Ok(())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade() -> Result<(), &'static str> {
            ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
                "Wrong storage version after upgrade"
            );
            Ok(())
        }
    }
}
