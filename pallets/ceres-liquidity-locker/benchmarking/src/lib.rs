//! Ceres liquidity locker module benchmarking.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "runtime-benchmarks")]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use codec::Decode;
use common::prelude::Balance;
use common::{
    balance, AccountIdOf, AssetIdOf, AssetManager, AssetName, AssetSymbol, DEXId,
    TradingPairSourceManager, DEFAULT_BALANCE_PRECISION, XOR,
};
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;

use pallet_timestamp::Pallet as Timestamp;
use permissions::Pallet as Permissions;
use pool_xyk::Pallet as XYKPool;

#[cfg(test)]
mod mock;

pub struct Pallet<T: Config>(ceres_liquidity_locker::Pallet<T>);
pub trait Config: ceres_liquidity_locker::Config + pool_xyk::Config + permissions::Config {}

pub const DEX: DEXId = DEXId::Polkaswap;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

#[allow(non_snake_case)]
pub fn AUTHORITY<T: frame_system::Config>() -> T::AccountId {
    let bytes = hex!("34a5b78f5fbcdc92a28767d63b579690a4b2f6a179931b3ecc87f09fc9366d47");
    AccountIdOf::<T>::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn setup_benchmark_assets_only<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);
    let ceres_asset_id = common::AssetId32::from_bytes(hex!(
        "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
    ));

    // Grant permissions to self in case they haven't been explicitly given in genesis config
    let _ = Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::MINT,
        permissions::Scope::Unlimited,
    );
    let _ = Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::BURN,
        permissions::Scope::Unlimited,
    );

    let _ = T::AssetManager::register_asset_id(
        owner.clone(),
        XOR.into(),
        AssetSymbol(b"XOR".to_vec()),
        AssetName(b"SORA".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        common::AssetType::Regular,
        None,
        None,
    );
    let _ = T::AssetManager::register_asset_id(
        owner.clone(),
        ceres_asset_id.into(),
        AssetSymbol(b"CERES".to_vec()),
        AssetName(b"Ceres".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        common::AssetType::Regular,
        None,
        None,
    );

    T::TradingPairSourceManager::register_pair(DEX.into(), XOR.into(), ceres_asset_id.into())
        .unwrap();

    T::AssetManager::mint_to(&XOR.into(), &owner.clone(), &owner.clone(), balance!(50000))?;
    T::AssetManager::mint_to(
        &ceres_asset_id.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000),
    )?;

    Ok(())
}

fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);
    let owner_origin: <T as frame_system::Config>::RuntimeOrigin =
        RawOrigin::Signed(owner.clone()).into();
    let ceres_asset_id = common::AssetId32::from_bytes(hex!(
        "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
    ));

    setup_benchmark_assets_only::<T>()?;

    XYKPool::<T>::initialize_pool(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        ceres_asset_id.into(),
    )?;

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        ceres_asset_id.into(),
        balance!(2000),
        balance!(3000),
        balance!(2000),
        balance!(3000),
    )?;

    Ok(())
}

benchmarks! {
    lock_liquidity {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let timestamp = Timestamp::<T>::get() + 5u32.into();
        let lp_percentage = balance!(0.5);
        let ceres_asset_id: AssetIdOf<T> = common::AssetId32::from_bytes(hex!(
            "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
        )).into();
    }: {
        let _ = ceres_liquidity_locker::Pallet::<T>::lock_liquidity(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            ceres_asset_id,
            timestamp,
            lp_percentage,
            false
        );
    }
    verify {
        let lockups_alice = ceres_liquidity_locker::LockerData::<T>::get(caller.clone());
        assert_eq!(lockups_alice.len(), 1);
        assert_eq!(lockups_alice.get(0).unwrap().unlocking_timestamp, timestamp);
    }

    change_ceres_fee {
        setup_benchmark::<T>()?;
        let caller = AUTHORITY::<T>();
    }: {
        let _ = ceres_liquidity_locker::Pallet::<T>::change_ceres_fee(
            RawOrigin::Signed(caller.clone()).into(),
            balance!(69)
        );
    }
    verify {
        assert_eq!(ceres_liquidity_locker::FeesOptionTwoCeresAmount::<T>::get(), balance!(69));
    }
}

impl_benchmark_test_suite!(
    Pallet,
    crate::mock::ExtBuilder::default().build(),
    crate::mock::Runtime
);
