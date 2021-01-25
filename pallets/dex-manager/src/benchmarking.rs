//! DEX Manager module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::{Decode, Encode};
use common::DEXId;
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_io::hashing::blake2_256;
use sp_std::prelude::*;

use crate::Module as DEXManager;
use permissions::Module as Permissions;

use common::XOR;

// Support Functions
fn alice<T: Trait>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn dex<T: Trait>(name: &'static str, index: u32) -> T::DEXId {
    let entropy = (name, index).using_encoded(blake2_256);
    T::DEXId::decode(&mut &entropy[..]).unwrap_or_default()
}

// Adds `n` exchanges to the Dex-manager Pallet
fn setup_benchmark<T: Trait>(n: u32) -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Trait>::Origin = RawOrigin::Signed(owner.clone()).into();

    Permissions::<T>::grant_permission(owner.clone(), owner.clone(), permissions::INIT_DEX)?;

    for i in 0..n {
        DEXManager::<T>::initialize_dex(
            owner_origin.clone(),
            dex::<T>("dex", i),
            XOR.into(),
            owner.clone(),
            true,
        )?;
    }

    Ok(())
}

fn assert_last_event<T: Trait>(generic_event: <T as Trait>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Trait>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    _ {}

    initialize_dex {
        let n in 1 .. 1000 => setup_benchmark::<T>(n)?;
        let caller = alice::<T>();
        let dex_id = dex::<T>("dex", n + 1);
    }: _(
        RawOrigin::Signed(caller.clone()),
        dex_id,
        XOR.into(),
        caller.clone(),
        None,
        None
    )
    verify {
        assert_last_event::<T>(RawEvent::DEXInitialized(dex_id).into())
    }

    set_fee {
        let n in 1 .. 1000 => setup_benchmark::<T>(n)?;
        let caller = alice::<T>();
    }: _(
        RawOrigin::Signed(caller.clone()),
        DEXId::Polkaswap.into(),
        100
    )
    verify {
        assert_last_event::<T>(RawEvent::FeeChanged(DEXId::Polkaswap.into(), 100).into())
    }

    set_protocol_fee {
        let n in 1 .. 1000 => setup_benchmark::<T>(n)?;
        let caller = alice::<T>();
    }: _(
        RawOrigin::Signed(caller.clone()),
        DEXId::Polkaswap.into(),
        100
    )
    verify {
        assert_last_event::<T>(RawEvent::ProtocolFeeChanged(DEXId::Polkaswap.into(), 100).into())
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
            assert_ok!(test_benchmark_initialize_dex::<Runtime>());
            assert_ok!(test_benchmark_set_fee::<Runtime>());
            assert_ok!(test_benchmark_set_protocol_fee::<Runtime>());
        });
    }
}
