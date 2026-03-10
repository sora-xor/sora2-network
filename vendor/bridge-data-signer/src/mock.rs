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

use crate as data_signer;
use bridge_types::{traits::OutboundChannel, SubNetworkId};
use frame_support::weights::Weight;
use frame_support::{parameter_types, traits::Everything};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    transaction_validity::TransactionPriority,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        DataSigner: data_signer::{Pallet, Call, Storage, Event<T>},
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const SS58Prefix: u8 = 42;
    pub const TestUnsignedPriority: TransactionPriority = 100;
    pub const TestUnsignedLongevity: u64 = 100;
    pub const BridgeMaxPeers: u32 = 50;
}

pub type AccountId = u64;

impl system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl data_signer::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type OutboundChannel = TestOutboundChannel;
    type CallOrigin = TestCallOrigin;
    type UnsignedPriority = TestUnsignedPriority;
    type UnsignedLongevity = TestUnsignedLongevity;
    type MaxPeers = BridgeMaxPeers;
    type WeightInfo = ();
}

pub struct TestOutboundChannel;
impl OutboundChannel<SubNetworkId, AccountId, ()> for TestOutboundChannel {
    fn submit(
        _network_id: SubNetworkId,
        _who: &system::RawOrigin<AccountId>,
        _payload: &[u8],
        _additional: (),
    ) -> Result<H256, sp_runtime::DispatchError> {
        Ok([1; 32].into())
    }

    fn submit_weight() -> Weight {
        Default::default()
    }
}

impl Default for RuntimeOrigin {
    fn default() -> Self {
        RuntimeOrigin::root()
    }
}

pub struct TestCallOrigin;
impl<OuterOrigin: Default> frame_support::traits::EnsureOrigin<OuterOrigin> for TestCallOrigin {
    type Success = bridge_types::types::CallOriginOutput<SubNetworkId, H256, ()>;

    fn try_origin(_o: OuterOrigin) -> Result<Self::Success, OuterOrigin> {
        Ok(bridge_types::types::CallOriginOutput {
            network_id: SubNetworkId::Mainnet,
            message_id: [1; 32].into(),
            timepoint: Default::default(),
            additional: (),
        })
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<OuterOrigin, ()> {
        Ok(Default::default())
    }
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut ext: sp_io::TestExternalities = system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into();
    ext.register_extension(sp_keystore::KeystoreExt(std::sync::Arc::new(
        sp_keystore::testing::KeyStore::new(),
    )));

    ext
}
