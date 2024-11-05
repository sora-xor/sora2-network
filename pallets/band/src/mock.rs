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

use crate::{self as band, Config};
use common::{mock_frame_system_config, mock_oracle_proxy_config, mock_pallet_timestamp_config};
use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstU16, ConstU32},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;
type AccountId = u64;
type Moment = u64;

// Configure a mock runtime to test the pallet.
construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system,
        Band: band,
        OracleProxy: oracle_proxy,
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
    }
);

parameter_types! {
    pub const GetRateStalePeriod: Moment = 60*5*1000; // 5 minutes
    pub const GetRateStaleBlockPeriod: u64 = 600;
}

impl Config for Runtime {
    type Symbol = String;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type OnNewSymbolsRelayedHook = oracle_proxy::Pallet<Runtime>;
    type Time = Timestamp;
    type GetBandRateStalePeriod = GetRateStalePeriod;
    type OnSymbolDisabledHook = ();
    type GetBandRateStaleBlockPeriod = GetRateStaleBlockPeriod;
    type MaxRelaySymbols = frame_support::traits::ConstU32<100>;
}

mock_frame_system_config!(Runtime, ConstU16<42>, ConstU32<16>, ());
mock_oracle_proxy_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap()
        .into()
}
