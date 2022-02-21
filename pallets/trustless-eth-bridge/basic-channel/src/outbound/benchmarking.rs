//! BasicOutboundChannel pallet benchmarking
use super::*;

use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_support::traits::OnInitialize;

#[allow(unused_imports)]
use crate::outbound::Pallet as BasicOutboundChannel;

const BASE_NETWORK_ID: EthNetworkId = 0;

benchmarks! {
    // Benchmark `on_initialize` under worst case conditions, i.e. messages
    // in queue are committed.
    on_initialize {
        let m in 1 .. T::MaxMessagesPerCommit::get() as u32;
        let p in 0 .. T::MaxMessagePayloadSize::get() as u32;

        for _ in 0 .. m {
            let payload: Vec<u8> = (0..).take(p as usize).collect();
            <MessageQueue<T>>::append(BASE_NETWORK_ID, Message {
                network_id: BASE_NETWORK_ID,
                target: H160::zero(),
                nonce: 0u64,
                payload,
            });
        }

        let block_number = Interval::<T>::get();

    }: { BasicOutboundChannel::<T>::on_initialize(block_number) }
    verify {
        assert_eq!(<MessageQueue<T>>::get(BASE_NETWORK_ID).len(), 0);
    }

    // Benchmark 'on_initialize` for the best case, i.e. nothing is done
    // because it's not a commitment interval.
    on_initialize_non_interval {
        <MessageQueue<T>>::take(BASE_NETWORK_ID);
        <MessageQueue<T>>::append(BASE_NETWORK_ID, Message {
            network_id: BASE_NETWORK_ID,
            target: H160::zero(),
            nonce: 0u64,
            payload: vec![1u8; T::MaxMessagePayloadSize::get() as usize],
        });

        Interval::<T>::put::<T::BlockNumber>(10u32.into());
        let block_number: T::BlockNumber = 12u32.into();

    }: { BasicOutboundChannel::<T>::on_initialize(block_number) }
    verify {
        assert_eq!(<MessageQueue<T>>::get(BASE_NETWORK_ID).len(), 1);
    }

    // Benchmark 'on_initialize` for the case where it is a commitment interval
    // but there are no messages in the queue.
    on_initialize_no_messages {
        <MessageQueue<T>>::take(BASE_NETWORK_ID);

        let block_number = Interval::<T>::get();

    }: { BasicOutboundChannel::<T>::on_initialize(block_number) }
}

impl_benchmark_test_suite!(
    BasicOutboundChannel,
    crate::outbound::test::new_tester(),
    crate::outbound::test::Test,
);
