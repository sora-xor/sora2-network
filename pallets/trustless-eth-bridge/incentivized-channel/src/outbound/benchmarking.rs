//! IncentivizedOutboundChannel pallet benchmarking
use super::*;

use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_support::traits::OnInitialize;
use frame_system::RawOrigin;
use sp_core::U256;

const BASE_NETWORK_ID: EthNetworkId = 12123;

#[allow(unused_imports)]
use crate::outbound::Pallet as IncentivizedOutboundChannel;

benchmarks! {
    // Benchmark `on_initialize` under worst case conditions, i.e. messages
    // in queue are committed.
    on_initialize {
        let m in 1 .. T::MaxMessagesPerCommit::get() as u32;
        let p in 0 .. T::MaxMessagePayloadSize::get() as u32;

        for _ in 0 .. m {
            let payload: Vec<u8> = (0..).take(p as usize).collect();
            <MessageQueues<T>>::append(BASE_NETWORK_ID, Message {
                network_id: BASE_NETWORK_ID,
                target: H160::zero(),
                nonce: 0u64,
                fee: U256::zero(),
                payload,
            });
        }

        let block_number = T::BlockNumber::from(BASE_NETWORK_ID) % Interval::<T>::get();

    }: { IncentivizedOutboundChannel::<T>::on_initialize(block_number) }
    verify {
        assert_eq!(<MessageQueues<T>>::get(BASE_NETWORK_ID).len(), 0);
    }

    // Benchmark 'on_initialize` for the best case, i.e. nothing is done
    // because it's not a commitment interval.
    on_initialize_non_interval {
        <MessageQueues<T>>::take(BASE_NETWORK_ID);
        <MessageQueues<T>>::append(BASE_NETWORK_ID, Message {
            network_id: BASE_NETWORK_ID,
            target: H160::zero(),
            nonce: 0u64,
            fee: U256::zero(),
            payload: vec![1u8; T::MaxMessagePayloadSize::get() as usize],
        });

        Interval::<T>::put::<T::BlockNumber>(10u32.into());
        let block_number: T::BlockNumber = 12u32.into();

    }: { IncentivizedOutboundChannel::<T>::on_initialize(block_number) }
    verify {
        assert_eq!(<MessageQueues<T>>::get(BASE_NETWORK_ID).len(), 1);
    }

    // Benchmark 'on_initialize` for the case where it is a commitment interval
    // but there are no messages in the queue.
    on_initialize_no_messages {
        <MessageQueues<T>>::take(BASE_NETWORK_ID);

        let block_number = Interval::<T>::get();

    }: { IncentivizedOutboundChannel::<T>::on_initialize(block_number) }

    // Benchmark `set_fee` under worst case conditions:
    // * The origin is authorized, i.e. equals SetFeeOrigin
    set_fee {
        let new_fee: BalanceOf<T> = 32000000u128.into();
        assert!(<Fee<T>>::get() != new_fee);

    }: _(RawOrigin::Root, new_fee)
    verify {
        assert_eq!(<Fee<T>>::get(), new_fee);
    }
}

impl_benchmark_test_suite!(
    IncentivizedOutboundChannel,
    crate::outbound::test::new_tester(),
    crate::outbound::test::Test,
);
