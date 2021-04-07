use codec::Decode;
use common::eth::EthereumAddress;
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::{Config, Event, Module, Pallet, PswapFarmOwners, PswapWaifuOwners, ValOwners};

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("f08879dab4530529153a1bdb63e27cd3be45f1574a122b7e88579b6e5e60bd43");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

// Adds `n` of unaccessible rewards and after adds 1 reward that will be claimed
fn add_rewards<T: Config>(n: u32) {
    let unaccessible_eth_addr: EthereumAddress =
        hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A635").into();
    for _i in 0..n {
        ValOwners::<T>::insert(&unaccessible_eth_addr, 1);
        PswapFarmOwners::<T>::insert(&unaccessible_eth_addr, 1);
        PswapWaifuOwners::<T>::insert(&unaccessible_eth_addr, 1);
    }
    let eth_addr: EthereumAddress = hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636").into();
    ValOwners::<T>::insert(&eth_addr, 300);
    PswapFarmOwners::<T>::insert(&eth_addr, 300);
    PswapWaifuOwners::<T>::insert(&eth_addr, 300);
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    claim {
        let n in 1 .. 1000 => add_rewards::<T>(n);
        let caller = alice::<T>();
        let caller_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(caller.clone()).into();
        let signature = hex!("eb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb60e018c0bf560b2de013078f1c").into();
    }: {
        Pallet::<T>::claim(caller_origin, signature)?;
    }
    verify {
        assert_last_event::<T>(Event::Claimed(caller).into())
    }
}

#[cfg(test)]
mod tests {
    use frame_support::assert_ok;

    use crate::mock::{ExtBuilder, Runtime};

    #[test]
    fn migrate() {
        ExtBuilder::with_rewards(false).build().execute_with(|| {
            assert_ok!(super::test_benchmark_claim::<Runtime>());
        });
    }
}
