//! Assets module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;
use common::balance;

use crate::Pallet as CeresStaking;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
	deposit {
		let caller = alice::<T>();
		let amount = balance!(100);
	}: _(RawOrigin::Signed(caller.clone()), amount)
	verify {
		assert_last_event::<T>(Event::Deposited(caller.clone(), amount).into());
	}

	withdraw {
		let caller = alice::<T>();
		let amount = balance!(100);
		CeresStaking::<T>::deposit(RawOrigin::Signed(caller.clone()).into(), amount);
	}: _(RawOrigin::Signed(caller.clone()))
	verify {
		assert_last_event::<T>(Event::Withdrawn(caller, amount, balance!(0)).into());
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
			assert_ok!(test_benchmark_deposit::<Runtime>());
			assert_ok!(test_benchmark_withdraw::<Runtime>());
		});
	}
}
