//! DEX-API module benchmarking.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use dex_api::*;

use codec::Decode;
use common::prelude::{Balance, SwapVariant};
use common::{balance, AssetName, AssetSymbol, DEXId, LiquiditySourceType, DOT, XOR};
use frame_benchmarking::benchmarks;
use frame_support::traits::Get;
use frame_system::{EventRecord, RawOrigin};

use hex_literal::hex;
use permissions::{BURN, MINT};
use sp_std::prelude::*;

use assets::Module as Assets;
use permissions::Module as Permissions;
use pool_xyk::Module as XYKPool;
use technical::Module as Technical;
use trading_pair::Module as TradingPair;

pub const DEX: DEXId = DEXId::Polkaswap;

#[cfg(test)]
mod mock;

pub struct Module<T: Config>(dex_api::Module<T>);
pub trait Config: dex_api::Config + pool_xyk::Config + technical::Config {}

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();

    // Grant permissions to self in case they haven't been explicitly given in genesis config
    let _ = Permissions::<T>::grant_permission(owner.clone(), owner.clone(), MINT);
    let _ = Permissions::<T>::grant_permission(owner.clone(), owner.clone(), BURN);

    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        XOR.into(),
        AssetSymbol(b"XOR".to_vec()),
        AssetName(b"SORA".to_vec()),
        18,
        Balance::from(0u32),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"DOT".to_vec()),
        AssetName(b"Polkadot".to_vec()),
        18,
        Balance::from(0u32),
        true,
    );

    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())?;

    let (_, tech_acc_id, _fee_acc_id, mark_asset) =
        XYKPool::<T>::initialize_pool_unchecked(owner.clone(), DEX.into(), XOR.into(), DOT.into())?;

    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        mark_asset.clone().into(),
        AssetSymbol(b"PSWAP".to_vec()),
        AssetName(b"Polkaswap".to_vec()),
        18,
        Balance::from(0u32),
        true,
    );

    let repr: <T>::AccountId = Technical::<T>::tech_account_id_to_account_id(&tech_acc_id).unwrap();

    let _ = Permissions::<T>::grant_permission(owner.clone(), repr.clone(), MINT);
    let _ = Permissions::<T>::grant_permission(owner.clone(), repr.clone(), BURN);

    Assets::<T>::mint(
        owner_origin.clone(),
        XOR.into(),
        owner.clone(),
        balance!(10000),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        DOT.into(),
        owner.clone(),
        balance!(20000),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        XOR.into(),
        repr.clone(),
        balance!(1000000),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        DOT.into(),
        repr.clone(),
        balance!(1500000),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        mark_asset.into(),
        owner.clone(),
        balance!(1500000000000),
    )?;

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
