//! Faucet module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::{Call, Config, Event, Module};

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use common::{AssetSymbol, XOR};

use assets::Module as Assets;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn add_assets<T: Config>(n: u32) -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();
    for _i in 0..n {
        Assets::<T>::register(owner_origin.clone(), AssetSymbol(b"TOKEN".to_vec()), 18)?;
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
    _ {}

    transfer {
        let n in 1 .. 1000 => add_assets::<T>(n)?;
        // let n in 1 .. 1000 => (); //setup_benchmark()?;
        let caller = alice::<T>();
    }: _(
        RawOrigin::Signed(caller.clone()),
        XOR.into(),
        caller.clone(),
        100_u32.into()
    )
    verify {
        assert_last_event::<T>(Event::Transferred(caller.clone(), 100_u32.into()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Test};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::build().execute_with(|| {
            assert_ok!(test_benchmark_transfer::<Runtime>());
        });
    }
}
