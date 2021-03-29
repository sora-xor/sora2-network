//! Assets module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use common::XOR;

use crate::Pallet as Assets;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

// Adds `n` assets to the Assets Pallet
fn add_assets<T: Config>(n: u32) -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();
    for _i in 0..n {
        Assets::<T>::register(
            owner_origin.clone(),
            AssetSymbol(b"TOKEN".to_vec()),
            AssetName(b"TOKEN".to_vec()),
            Balance::zero(),
            true,
        )?;
    }

    Ok(())
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    register {
        let n in 1 .. 1000 => add_assets::<T>(n)?;
        let caller = alice::<T>();
        let asset_id = Assets::<T>::gen_asset_id(&caller);
    }:
    {
        Assets::<T>::register_asset_id(
            caller.clone(),
            asset_id.clone(),
            AssetSymbol(b"NEWT".to_vec()),
            AssetName(b"NEWT".to_vec()),
            18,
            Balance::zero(),
            true,
        )?;
    }
    verify {
        assert_last_event::<T>(Event::AssetRegistered(asset_id, caller).into())
    }

    transfer {
        let n in 1 .. 1000 => add_assets::<T>(n)?;
        let caller = alice::<T>();
        let _ = Assets::<T>::register_asset_id(
            caller.clone(),
            XOR.into(),
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"XOR".to_vec()),
            18,
            Balance::zero(),
            true,
        );
    }: _(
        RawOrigin::Signed(caller.clone()),
        XOR.into(),
        caller.clone(),
        100_u32.into()
    )
    verify {
        assert_last_event::<T>(Event::Transfer(caller.clone(), caller, XOR.into(), 100_u32.into()).into())
    }

    mint {
        let n in 1 .. 1000 => add_assets::<T>(n)?;
        let caller = alice::<T>();
        let _ = Assets::<T>::register_asset_id(
            caller.clone(),
            XOR.into(),
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"XOR".to_vec()),
            18,
            Balance::zero(),
            true,
        );
    }: _(
        RawOrigin::Signed(caller.clone()),
        XOR.into(),
        caller.clone(),
        100_u32.into()
    )
    verify {
        assert_last_event::<T>(Event::Mint(caller.clone(), caller, XOR.into(), 100_u32.into()).into())
    }

    burn {
        let n in 1 .. 1000 => add_assets::<T>(n)?;
        let caller = alice::<T>();
        let _ = Assets::<T>::register_asset_id(
            caller.clone(),
            XOR.into(),
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"Sora Token".to_vec()),
            18,
            Balance::zero(),
            true,
        );
        Assets::<T>::mint(
            RawOrigin::Signed(caller.clone()).into(),
            XOR.into(),
            caller.clone(),
            1000_u32.into()
        )?;
    }: _(
        RawOrigin::Signed(caller.clone()),
        XOR.into(),
        100_u32.into()
    )
    verify {
        assert_last_event::<T>(Event::Burn(caller, XOR.into(), 100_u32.into()).into())
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
            assert_ok!(test_benchmark_register::<Runtime>());
            assert_ok!(test_benchmark_transfer::<Runtime>());
            assert_ok!(test_benchmark_mint::<Runtime>());
            assert_ok!(test_benchmark_burn::<Runtime>());
        });
    }
}
