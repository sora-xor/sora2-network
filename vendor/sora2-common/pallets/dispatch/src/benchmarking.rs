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

//! BridgeInboundChannel pallet benchmarking

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::traits::MessageDispatch;
use bridge_types::types::GenericAdditionalInboundData;
use bridge_types::types::MessageId;
use bridge_types::GenericNetworkId;
use bridge_types::SubNetworkId;
use frame_benchmarking::benchmarks_instance_pallet;
use frame_system::EventRecord;
use frame_system::{self};
use sp_std::prelude::*;

fn assert_last_event<T: Config<I>, I: 'static>(
    system_event: <T as frame_system::Config>::RuntimeEvent,
) {
    let events = frame_system::Pallet::<T>::events();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks_instance_pallet! {
    where_clause {
        where
            <T as crate::Config<I>>::OriginOutput:
                bridge_types::traits::BridgeOriginOutput<
                    NetworkId = GenericNetworkId,
                    Additional = GenericAdditionalInboundData
                >,
            T: crate::Config<I, MessageId = MessageId>,
            crate::Event::<T, I>: Into<<T as frame_system::Config>::RuntimeEvent>
    }
    dispatch_success {
        let message_id = MessageId::basic(GenericNetworkId::EVM([1u8; 32].into()), GenericNetworkId::Sub(SubNetworkId::Mainnet), 1);
        let (network_id, additional, payload) =
            <T as crate::Config<I>>::BenchmarkHelper::successful_dispatch_context();
    }: {
        crate::Pallet::<T, I>::dispatch(
            network_id,
            message_id,
            Default::default(),
            &payload,
            additional,
        )
    }
    verify {
        assert_last_event::<T, I>(crate::Event::<T, I>::MessageDispatched(message_id, Ok(())).into());
    }

    dispatch_decode_failed {
        let message_id = MessageId::basic(GenericNetworkId::EVM([1u8; 32].into()), GenericNetworkId::Sub(SubNetworkId::Mainnet), 1);
    }: {
        crate::Pallet::<T, I>::dispatch(
            H256::repeat_byte(1).into(),
            message_id,
            Default::default(),
            &[],
            AdditionalEVMInboundData {
                source: Default::default()
            }.into()
        )
    }
    verify {
        assert_last_event::<T, I>(crate::Event::<T, I>::MessageDecodeFailed(message_id).into());
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test,);
}
