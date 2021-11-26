//! Ceres liquidity locker module benchmarking.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Decode;
use common::prelude::Balance;
use common::{balance, AssetName, AssetSymbol, DEXId, DEFAULT_BALANCE_PRECISION, XOR};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;

use assets::Module as Assets;
use permissions::Module as Permissions;
use pool_xyk::Module as XYKPool;
use trading_pair::Module as TradingPair;

#[cfg(test)]
mod mock;

pub struct Module<T: Config>(ceres_liquidity_locker::Module<T>);
pub trait Config: ceres_liquidity_locker::Config + trading_pair::Config + pool_xyk::Config {}

pub const DEX: DEXId = DEXId::Polkaswap;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn setup_benchmark_assets_only<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Module::<T>::inc_providers(&owner);
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();
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

    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        XOR.into(),
        AssetSymbol(b"XOR".to_vec()),
        AssetName(b"SORA".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        ceres_asset_id.into(),
        AssetSymbol(b"CERES".to_vec()),
        AssetName(b"Ceres".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    )
    .unwrap();

    TradingPair::<T>::register(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        ceres_asset_id.into(),
    )
    .unwrap();

    Assets::<T>::mint_to(&XOR.into(), &owner.clone(), &owner.clone(), balance!(50000))?;
    Assets::<T>::mint_to(
        &ceres_asset_id.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000),
    )?;

    Ok(())
}

fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Module::<T>::inc_providers(&owner);
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();
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
        balance!(0),
        balance!(0),
    )?;

    Ok(())
}

benchmarks! {
    lock_liquidity {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let block_number = frame_system::Pallet::<T>::block_number();
        let lp_percentage = balance!(0.5);
        let ceres_asset_id: T::AssetId = common::AssetId32::from_bytes(hex!(
            "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
        )).into();
    }: {
        let _ = ceres_liquidity_locker::Module::<T>::lock_liquidity(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            ceres_asset_id,
            block_number,
            lp_percentage,
            true
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(test_benchmark_lock_liquidity::<Runtime>());
        });
    }
}
