//! Trading Pair module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::{Decode, Encode};
use common::{AssetName, AssetSymbol, Balance, DEXId, DOT, XOR};
use frame_benchmarking::{benchmarks, Zero};
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_std::prelude::*;

use crate::Pallet as TradingPairModule;
use assets::Pallet as Assets;

pub const DEX: DEXId = DEXId::Polkaswap;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn asset<T: Config>(name: &'static str, index: u32) -> T::AssetId {
    let entropy: [u8; 32] = (name, index).using_encoded(blake2_256);
    T::AssetId::from(H256(entropy))
}

fn setup_benchmark<T: Config>(n: u32) -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();
    for i in 0..n {
        let asset_id = asset::<T>("token", i);
        Assets::<T>::register_asset_id(
            owner.clone(),
            asset_id.clone(),
            AssetSymbol(b"TOKEN".to_vec()),
            AssetName(b"Token".to_vec()),
            18,
            Balance::zero(),
            true,
        )?;
        TradingPairModule::<T>::register(
            owner_origin.clone(),
            DEX.into(),
            XOR.into(),
            asset_id.clone(),
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
        let n in 1 .. 1000 => setup_benchmark::<T>(n)?;
        let caller = alice::<T>();
        let trading_pair = TradingPair::<T> {
            base_asset_id: XOR.into(),
            target_asset_id: DOT.into(),
        };
    }: _(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        XOR.into(),
        DOT.into()
    )
    verify {
        assert_last_event::<T>(
            Event::TradingPairStored(
                DEX.into(),
                trading_pair.clone()
            ).into()
        )
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
        });
    }
}
