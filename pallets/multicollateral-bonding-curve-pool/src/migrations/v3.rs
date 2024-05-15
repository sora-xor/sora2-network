use crate::DistributionAccount;
use crate::DistributionAccountData;
use crate::DistributionAccounts;
use crate::Pallet;
use codec::{Decode, Encode};
use frame_support::traits::Get;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::{pallet_prelude::StorageVersion, traits::GetStorageVersion as _};
use log::{error, info};

#[cfg(feature = "try-runtime")]
use sp_std::prelude::Vec;

#[derive(Debug, Encode, Decode, Clone, scale_info::TypeInfo, Default)]
pub struct OldDistributionAccounts<DistributionAccountData> {
    pub xor_allocation: DistributionAccountData,
    pub val_holders: DistributionAccountData,
    pub sora_citizens: DistributionAccountData,
    pub stores_and_shops: DistributionAccountData,
    pub parliament_and_development: DistributionAccountData,
    pub projects: DistributionAccountData,
}

pub struct MigrateToV3<T>(core::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for MigrateToV3<T>
where
    T: crate::Config,
{
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        if Pallet::<T>::on_chain_storage_version() != 2 {
            error!(
                "Runtime upgrade executed with wrong storage version, expected 2, got {:?}",
                Pallet::<T>::on_chain_storage_version()
            );
            return <T as frame_system::Config>::DbWeight::get().reads(1);
        }
        info!("Applying migration to version 3: Move parliament and development distribution to buy back XST");
        let result = crate::DistributionAccountsEntry::<T>::translate::<
            OldDistributionAccounts<
                DistributionAccountData<DistributionAccount<T::AccountId, T::TechAccountId>>,
            >,
            _,
        >(|value| {
            if let Some(value) = value {
                Some(DistributionAccounts {
                    xor_allocation: value.xor_allocation,
                    val_holders: value.val_holders,
                    sora_citizens: value.sora_citizens,
                    stores_and_shops: value.stores_and_shops,
                    projects: value.projects,
                })
            } else {
                None
            }
        });
        if let Err(err) = result {
            error!("Failed to decode DistributionAccounts, skipping migration: {err:?}");
            return <T as frame_system::Config>::DbWeight::get().reads(1);
        }
        StorageVersion::new(3).put::<Pallet<T>>();
        <T as frame_system::Config>::BlockWeights::get().max_block
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 2,
            "must upgrade linearly"
        );
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 3,
            "should be upgraded to version 3"
        );
        Ok(())
    }
}
