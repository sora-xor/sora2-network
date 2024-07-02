#![cfg(feature = "wip")] // DEFI-R

use common::{AssetIdOf, AssetManager, AssetName, AssetSymbol, Balance, DEFAULT_BALANCE_PRECISION};
use frame_support::assert_ok;
use sp_core::crypto::AccountId32;

use frame_system::RawOrigin;

use crate::Config;

pub fn alice() -> AccountId32 {
    AccountId32::from([1; 32])
}

pub fn bob() -> AccountId32 {
    AccountId32::from([2; 32])
}

pub fn add_asset<T: Config>(owner: &T::AccountId) -> AssetIdOf<T> {
    frame_system::Pallet::<T>::inc_providers(owner);

    T::AssetManager::register_from(
        owner,
        AssetSymbol(b"TOKEN".to_vec()),
        AssetName(b"TOKEN".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        common::AssetType::Regular,
        None,
        None,
    )
    .expect("Failed to register asset")
}

pub fn register_sbt_asset<T: Config>(owner: &T::AccountId) -> AssetIdOf<T> {
    let asset_name = AssetName(b"Soulbound Token".to_vec());
    let asset_symbol = AssetSymbol(b"SBT".to_vec());
    let sbt_asset_id = T::AssetManager::gen_asset_id(&owner);

    // Issue SBT
    assert_ok!(crate::Pallet::<T>::issue_sbt(
        RawOrigin::Signed(owner.clone()).into(),
        asset_symbol,
        asset_name,
        None,
        None,
        None,
    ));

    sbt_asset_id.into()
}
