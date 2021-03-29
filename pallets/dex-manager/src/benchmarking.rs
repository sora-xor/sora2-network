//! DEX Manager module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::{Decode, Encode};
use common::DEXId;
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
// use sp_io::hashing::blake2_256;
use sp_std::prelude::*;

use crate::Pallet as DEXManager;
use permissions::Pallet as Permissions;

use common::XOR;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

// fn dex<T: Config>(name: &'static str, index: u32) -> T::DEXId {
//     let entropy = (name, index).using_encoded(blake2_256);
//     T::DEXId::decode(&mut &entropy[..]).unwrap_or_default()
// }

// Adds `n` exchanges to the Dex-manager Pallet
fn setup_benchmark<T: Config>(n: u32) -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();

    Permissions::<T>::grant_permission(owner.clone(), owner.clone(), permissions::INIT_DEX)?;

    // for i in 0..n {
    //     DEXManager::<T>::initialize_dex(
    //         owner_origin.clone(),
    //         dex::<T>("dex", i),
    //         XOR.into(),
    //         owner.clone(),
    //         true,
    //     )?;
    // }

    Ok(())
}

// fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
//     let events = frame_system::Module::<T>::events();
//     let system_event: <T as frame_system::Config>::Event = generic_event.into();
//     // compare to the last event record
//     let EventRecord { event, .. } = &events[events.len() - 1];
//     assert_eq!(event, &system_event);
// }
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    // #[test]
    // fn test_benchmarks() {
    //     ExtBuilder::default().build().execute_with(|| {
    //         assert_ok!(test_benchmark_initialize_dex::<Runtime>());
    //         assert_ok!(test_benchmark_set_fee::<Runtime>());
    //         assert_ok!(test_benchmark_set_protocol_fee::<Runtime>());
    //     });
    // }
}
