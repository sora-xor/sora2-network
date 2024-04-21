use crate::pallet;
use crate::Config;
use crate::Pallet;
use common::balance;
use common::FromGenericPair;
use common::TBCD;
use common::{AssetManager, TradingPairSourceManager};
use frame_support::traits::Get;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::{
    log::{error, info},
    pallet_prelude::StorageVersion,
    traits::GetStorageVersion as _,
};
use sp_runtime::traits::Zero;

#[cfg(feature = "try-runtime")]
use common::AssetInfoProvider;
#[cfg(feature = "try-runtime")]
use sp_std::prelude::Vec;

pub const SORAMITSU_PAYMENT_ACCOUNT: [u8; 32] =
    hex_literal::hex!("34b9a44a2d3f681d8191815a6de986bf163d15f6d6b58d56aa1ab887313e1723");

pub struct InitializeTBCD<T>(core::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for InitializeTBCD<T>
where
    T: crate::Config,
    <T as frame_system::Config>::AccountId: From<[u8; 32]>,
{
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        if Pallet::<T>::on_chain_storage_version() == 1 {
            info!("Applying migration to version 2: Add TBCD token");
            let assets_and_permissions_tech_account_id = T::TechAccountId::from_generic_pair(
                b"SYSTEM_ACCOUNT".to_vec(),
                b"ASSETS_PERMISSIONS".to_vec(),
            );
            let assets_and_permissions_account_id =
                match technical::Pallet::<T>::tech_account_id_to_account_id(
                    &assets_and_permissions_tech_account_id,
                ) {
                    Ok(account) => account,
                    Err(err) => {
                        error!(
                            "Failed to get account id for assets and permissions technical account id: {:?}, error: {:?}",
                            assets_and_permissions_tech_account_id, err
                        );
                        return <T as frame_system::Config>::DbWeight::get().reads(1);
                    }
                };
            if let Err(err) = T::AssetManager::register_asset_id(
                assets_and_permissions_account_id.clone(),
                TBCD.into(),
                common::AssetSymbol(b"TBCD".to_vec()),
                common::AssetName(b"SORA TBC Dollar".to_vec()),
                common::DEFAULT_BALANCE_PRECISION,
                common::Balance::zero(),
                true,
                None,
                None,
            ) {
                error!("Failed to register TBCD asset, error: {:?}", err);
                return <T as frame_system::Config>::DbWeight::get().reads(1);
            }
            if let Err(err) = T::AssetManager::mint_to(
                TBCD.into(),
                &assets_and_permissions_account_id,
                &SORAMITSU_PAYMENT_ACCOUNT.into(),
                balance!(1688406),
            ) {
                error!(
                    "Failed to mint TBCD asset to Soramitsu payment account, error: {:?}",
                    err
                );
                return <T as frame_system::Config>::DbWeight::get().reads(1);
            }
            if let Err(err) = <T as pallet::Config>::TradingPairSourceManager::register_pair(
                common::DEXId::Polkaswap.into(),
                common::XOR.into(),
                common::TBCD.into(),
            ) {
                error!("Failed to register TBCD trading pair, error: {:?}", err);
                return <T as frame_system::Config>::DbWeight::get().reads(1);
            }
            if let Err(err) = Pallet::<T>::initialize_pool_unchecked(TBCD.into(), false) {
                error!("Failed to initialize TBCD pool: {:?}", err);
                return <T as frame_system::Config>::DbWeight::get().reads(1);
            }
            StorageVersion::new(2).put::<Pallet<T>>();
            <T as frame_system::Config>::BlockWeights::get().max_block
        } else {
            error!(
                "Runtime upgrade executed with wrong storage version, expected 1, got {:?}",
                Pallet::<T>::on_chain_storage_version()
            );
            <T as frame_system::Config>::DbWeight::get().reads(1)
        }
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        frame_support::ensure!(
            <T as Config>::AssetInfoProvider::ensure_asset_exists(&TBCD.into()).is_err(),
            "TBCD asset already registered"
        );
        frame_support::ensure!(
            !crate::EnabledTargets::<T>::get().contains(&TBCD.into()),
            "TBCD pool already initialized"
        );
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 1,
            "must upgrade linearly"
        );
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        <T as Config>::AssetInfoProvider::ensure_asset_exists(&TBCD.into())?;
        frame_support::ensure!(
            crate::EnabledTargets::<T>::get().contains(&TBCD.into()),
            "TBCD pool is not initialized"
        );
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 2,
            "should be upgraded to version 1"
        );
        Ok(())
    }
}
