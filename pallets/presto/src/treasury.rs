use crate::Config;
use common::{AccountIdOf, AssetManager, Balance};
use core::marker::PhantomData;
use sp_runtime::traits::Get;
use sp_runtime::DispatchResult;

pub struct Treasury<T: Config>(PhantomData<T>);

impl<T: Config> Treasury<T> {
    pub fn mint_presto_usd(amount: Balance) -> DispatchResult {
        let presto_tech_account_id =
            technical::Pallet::<T>::tech_account_id_to_account_id(&T::PrestoTechAccount::get())?;

        T::AssetManager::mint_to(
            &T::PrestoUsdAssetId::get(),
            &presto_tech_account_id,
            &presto_tech_account_id,
            amount,
        )?;

        Ok(())
    }

    pub fn burn_presto_usd(amount: Balance) -> DispatchResult {
        let presto_tech_account_id =
            technical::Pallet::<T>::tech_account_id_to_account_id(&T::PrestoTechAccount::get())?;

        T::AssetManager::burn_from(
            &T::PrestoUsdAssetId::get(),
            &presto_tech_account_id,
            &presto_tech_account_id,
            amount,
        )?;

        Ok(())
    }

    pub fn send_presto_usd(amount: Balance, to: &AccountIdOf<T>) -> DispatchResult {
        let presto_tech_account_id =
            technical::Pallet::<T>::tech_account_id_to_account_id(&T::PrestoTechAccount::get())?;

        T::AssetManager::transfer_from(
            &T::PrestoUsdAssetId::get(),
            &presto_tech_account_id,
            to,
            amount,
        )?;

        Ok(())
    }

    pub fn transfer_from_buffer_to_main(amount: Balance) -> DispatchResult {
        let presto_tech_account_id =
            technical::Pallet::<T>::tech_account_id_to_account_id(&T::PrestoTechAccount::get())?;

        let presto_buffer_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
            &T::PrestoBufferTechAccount::get(),
        )?;

        T::AssetManager::transfer_from(
            &T::PrestoUsdAssetId::get(),
            &presto_buffer_tech_account_id,
            &presto_tech_account_id,
            amount,
        )?;

        Ok(())
    }

    pub fn return_from_buffer(amount: Balance, to: &AccountIdOf<T>) -> DispatchResult {
        let presto_buffer_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
            &T::PrestoBufferTechAccount::get(),
        )?;

        T::AssetManager::transfer_from(
            &T::PrestoUsdAssetId::get(),
            &presto_buffer_tech_account_id,
            to,
            amount,
        )?;

        Ok(())
    }

    pub fn collect_to_buffer(amount: Balance, from: &AccountIdOf<T>) -> DispatchResult {
        let presto_buffer_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
            &T::PrestoBufferTechAccount::get(),
        )?;

        T::AssetManager::transfer_from(
            &T::PrestoUsdAssetId::get(),
            from,
            &presto_buffer_tech_account_id,
            amount,
        )?;

        Ok(())
    }
}
