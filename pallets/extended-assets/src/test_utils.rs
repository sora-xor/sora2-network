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

pub fn register_regular_asset<T: Config>(owner: &T::AccountId) -> AssetIdOf<T> {
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

pub fn register_regulated_asset<T: Config>(owner: &T::AccountId) -> AssetIdOf<T> {
    frame_system::Pallet::<T>::inc_providers(owner);
    let regulated_asset_id = T::AssetManager::gen_asset_id(owner);

    assert_ok!(crate::Pallet::<T>::register_regulated_asset(
        RawOrigin::Signed(owner.clone()).into(),
        AssetSymbol(b"TOKEN".to_vec()),
        AssetName(b"TOKEN".to_vec()),
        common::Balance::from(0u32),
        true,
        true,
        None,
        None,
    ));

    regulated_asset_id
}

pub fn register_sbt_asset<T: Config>(owner: &T::AccountId) -> AssetIdOf<T> {
    frame_system::Pallet::<T>::inc_providers(owner);
    let sbt_asset_id = T::AssetManager::gen_asset_id(owner);

    // Issue SBT
    assert_ok!(crate::Pallet::<T>::issue_sbt(
        RawOrigin::Signed(owner.clone()).into(),
        AssetSymbol(b"SBT".to_vec()),
        AssetName(b"Soulbound Token".to_vec()),
        None,
        None,
        None,
    ));

    sbt_asset_id
}
