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

use super::*;
use bridge_types::GenericNetworkId;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::{self, RawOrigin};
use sp_std::prelude::*;

const BASE_NETWORK_ID: GenericNetworkId = GenericNetworkId::Sub(SubNetworkId::Mainnet);

#[allow(unused_imports)]
use crate::inbound::Pallet as BridgeInboundChannel;

// This collection of benchmarks should include a benchmark for each
// call dispatched by the channel, i.e. each "app" pallet function
// that can be invoked by MessageDispatch. The most expensive call
// should be used in the `submit` benchmark.
//
// We rely on configuration via chain spec of the app pallets because
// we don't have access to their storage here.
benchmarks! {
    // Benchmark `submit` extrinsic under worst case conditions:
    // * `submit` dispatches the DotApp::unlock call
    // * `unlock` call successfully unlocks DOT
    submit {
        let messages = vec![];
        let commitment = bridge_types::GenericCommitment::Sub(
            bridge_types::substrate::Commitment {
                messages: messages.try_into().unwrap(),
                nonce: 1u64,
            }
        );
        let proof = T::Verifier::valid_proof().unwrap();
    }: _(RawOrigin::None, BASE_NETWORK_ID, commitment, proof)
    verify {
        assert_eq!(1, <ChannelNonces<T>>::get(BASE_NETWORK_ID));
    }
}

impl_benchmark_test_suite!(
    BridgeInboundChannel,
    crate::inbound::test::new_tester(),
    crate::inbound::test::Test,
);
