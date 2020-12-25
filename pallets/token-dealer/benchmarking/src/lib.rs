//! Liquidity Proxy module benchmarking.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use cumulus_token_dealer::*;

use codec::{Decode, Encode};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

#[cfg(test)]
mod mock;
pub struct Module<T: Trait>(cumulus_token_dealer::Module<T>);
pub trait Trait: cumulus_token_dealer::Trait {}

// Support Functions
fn alice<T: Trait>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn bob<T: Trait>() -> T::AccountId {
    let bytes = hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn convert_hack<O: Decode>(input: &impl Encode) -> O {
    input.using_encoded(|e| Decode::decode(&mut &e[..]).expect("Must be compatible; qed"))
}

fn assert_last_event<T: Trait>(generic_event: <T as cumulus_token_dealer::Trait>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Trait>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    _ {}

    transfer_tokens_to_relay_chain {
        let n in 1 .. 1000 => ();
        let source = alice::<T>();
        let dest = bob::<T>();
    }: _(
        RawOrigin::Signed(source.clone()),
        dest.clone(),
        convert_hack(&1_000_u128)
    )
    verify {
        assert_last_event::<T>(
            RawEvent::TransferredTokensToRelayChain(
                dest.clone(),
                convert_hack(&1_000_u128),
            ).into()
        )
    }

    transfer_tokens_to_parachain_chain {
        let n in 1 .. 1000 => ();
        let source = alice::<T>();
        let dest = bob::<T>();
        let para_id = 200u32;
    }: _(
        RawOrigin::Signed(source.clone()),
        para_id,
        dest.clone(),
        convert_hack(&1_000_u128)
    )
    verify {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::build().execute_with(|| {
            assert_ok!(test_benchmark_transfer_tokens_to_relay_chain::<Runtime>());
            assert_ok!(test_benchmark_transfer_tokens_to_parachain_chain::<Runtime>());
        });
    }
}
