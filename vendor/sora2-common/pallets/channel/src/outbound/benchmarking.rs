// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! BridgeOutboundChannel pallet benchmarking
use super::*;

use bridge_types::substrate::BridgeMessage;
use bridge_types::traits::OutboundChannel;
use bridge_types::GenericBridgeMessage;
use bridge_types::GenericNetworkId;
use frame_benchmarking::benchmarks;
use frame_system::EventRecord;
use frame_system::RawOrigin;
use sp_std::prelude::*;

const BASE_NETWORK_ID: GenericNetworkId = GenericNetworkId::Sub(SubNetworkId::Mainnet);

#[allow(unused_imports)]
use crate::outbound::Pallet as BridgeOutboundChannel;

fn assert_last_event<T: Config>(system_event: <T as frame_system::Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    where_clause {
        where crate::outbound::Event::<T>: Into<<T as frame_system::Config>::RuntimeEvent>
    }

    // Benchmark `on_initialize` under worst case conditions, i.e. messages
    // in queue are committed.
    on_initialize {
        let m in 1 .. T::MaxMessagesPerCommit::get();
        let p in 0 .. T::MaxMessagePayloadSize::get();

        for _ in 0 .. m {
            let payload: Vec<u8> = (0..).take(p as usize).collect();
            MessageQueues::<T>::try_append(
                BASE_NETWORK_ID, GenericBridgeMessage::Sub(BridgeMessage {
                payload: payload.try_into().unwrap(),
                timepoint: Default::default(),
            })).unwrap();
        }

        let block_number = 0u32.into();

    }: { BridgeOutboundChannel::<T>::on_initialize(block_number) }
    verify {
        assert_eq!(<MessageQueues<T>>::get(BASE_NETWORK_ID).len(), 0);
    }

    // Benchmark 'on_initialize` for the best case, i.e. nothing is done
    // because it's not a commitment interval.
    on_initialize_non_interval {
        MessageQueues::<T>::take(BASE_NETWORK_ID);
        let payload: Vec<u8> = (0..).take(10).collect();
        MessageQueues::<T>::try_append(
            BASE_NETWORK_ID, GenericBridgeMessage::Sub(BridgeMessage {
            payload: payload.try_into().unwrap(),
            timepoint: Default::default(),
        })).unwrap();

        let interval: BlockNumberFor<T> = 10u32.into();
        Interval::<T>::put(interval);
        let block_number: BlockNumberFor<T> = 12u32.into();

    }: { BridgeOutboundChannel::<T>::on_initialize(block_number) }
    verify {
        assert_eq!(<MessageQueues<T>>::get(BASE_NETWORK_ID).len(), 1);
    }

    // Benchmark 'on_initialize` for the case where it is a commitment interval
    // but there are no messages in the queue.
    on_initialize_no_messages {
        MessageQueues::<T>::take(BASE_NETWORK_ID);

        let block_number = Interval::<T>::get();

    }: { BridgeOutboundChannel::<T>::on_initialize(block_number) }

    submit {

    }: {
        BridgeOutboundChannel::<T>::submit(SubNetworkId::Rococo, &RawOrigin::Root, &[0u8; 128], ()).unwrap()
    }
    verify {
        assert_last_event::<T>(crate::outbound::Event::<T>::MessageAccepted {
            network_id: GenericNetworkId::Sub(SubNetworkId::Rococo),
            batch_nonce: 1,
            message_nonce: 0
        }.into());
    }

    impl_benchmark_test_suite!(
        BridgeOutboundChannel,
        crate::outbound::test::new_tester(),
        crate::outbound::test::Test,
    );
}
