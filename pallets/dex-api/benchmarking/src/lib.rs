//! DEX-API module benchmarking.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use dex_api::*;

use codec::Decode;
use common::prelude::{Balance, SwapVariant};
use common::{
    balance, AssetName, AssetSymbol, DEXId, LiquiditySourceType, DOT, PSWAP, USDT, VAL, XOR,
};
use frame_benchmarking::benchmarks;
use frame_support::traits::Get;
use frame_system::{EventRecord, RawOrigin};

use frame_benchmarking::Zero;
use hex_literal::hex;
use sp_std::prelude::*;

use assets::Pallet as Assets;
use multicollateral_bonding_curve_pool::Pallet as MBCPool;
use permissions::Pallet as Permissions;
use pool_xyk::Pallet as XYKPool;
use trading_pair::Pallet as TradingPair;

pub const DEX: DEXId = DEXId::Polkaswap;

#[cfg(test)]
mod mock;

pub struct Module<T: Config>(dex_api::Module<T>);
pub trait Config:
    dex_api::Config + pool_xyk::Config + technical::Config + multicollateral_bonding_curve_pool::Config
{
}

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

// Prepare Runtime for running benchmarks
fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();
    let dex_id: T::DEXId = DEX.into();

    // Grant permissions to self in case they haven't been explicitly given in genesis config
    Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::MANAGE_DEX,
        permissions::Scope::Limited(common::hash(&dex_id)),
    )
    .unwrap();
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
        AssetName(b"XOR".to_vec()),
        18,
        Balance::zero(),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        VAL.into(),
        AssetSymbol(b"VAL".to_vec()),
        AssetName(b"VAL".to_vec()),
        18,
        Balance::zero(),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        PSWAP.into(),
        AssetSymbol(b"PSWAP".to_vec()),
        AssetName(b"PSWAP".to_vec()),
        18,
        Balance::zero(),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        USDT.into(),
        AssetSymbol(b"USDT".to_vec()),
        AssetName(b"USDT".to_vec()),
        18,
        Balance::zero(),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"DOT".to_vec()),
        AssetName(b"DOT".to_vec()),
        18,
        Balance::zero(),
        true,
    );
    Assets::<T>::mint_to(&XOR.into(), &owner.clone(), &owner.clone(), balance!(50000)).unwrap();
    Assets::<T>::mint_to(
        &DOT.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &USDT.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &VAL.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &PSWAP.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();

    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into()).unwrap();
    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), USDT.into()).unwrap();
    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), VAL.into()).unwrap();
    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), PSWAP.into()).unwrap();

    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())?;
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), VAL.into())?;
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), PSWAP.into())?;
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), USDT.into())?;

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        DOT.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )?;
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        VAL.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )?;
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        PSWAP.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )?;
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        USDT.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )?;

    MBCPool::<T>::initialize_pool(owner_origin.clone(), USDT.into())?;
    MBCPool::<T>::initialize_pool(owner_origin.clone(), VAL.into())?;

    Ok(())
}

#[allow(dead_code)]
fn assert_last_event<T: Config>(generic_event: <T as dex_api::Config>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    swap {
        let n in 1 .. 1000 => setup_benchmark::<T>()?;

        let caller = alice::<T>();
        let base_asset: T::AssetId = <T as assets::Config>::GetBaseAssetId::get();
        let target_asset: T::AssetId = DOT.into();
    }: _(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        LiquiditySourceType::XYKPool,
        base_asset.clone(),
        target_asset.clone(),
        balance!(1000),
        0,
        SwapVariant::WithDesiredInput,
        None
    )
    verify {
        // TODO: implement proper verification method
        // assert_last_event::<T>(Event::DirectExchange(
        //     caller.clone(),
        //     caller.clone(),
        //     DEX.into(),
        //     LiquiditySourceType::XYKPool,
        //     base_asset.clone(),
        //     target_asset.clone(),
        //     fixed!(1000),
        //     fixed!(667),
        //     fixed!(3)
        // ).into())
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
            assert_ok!(test_benchmark_swap::<Runtime>());
        });
    }
}
