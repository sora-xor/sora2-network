use super::*;
use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::types;
use bridge_types::types::GenericAdditionalInboundData;
use bridge_types::GenericNetworkId;
use codec::Encode;
use frame_support::parameter_types;
use frame_support::traits::{ConstU32, Everything};
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup, Keccak256};
use sp_runtime::BuildStorage;

use crate as dispatch;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test
    {
        System: frame_system,
        Dispatch: dispatch,
    }
);

type AccountId = u64;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type BaseCallFilter = Everything;
    type SystemWeightInfo = ();
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<65536>;
    type Nonce = u64;
    type Block = Block;
}

pub struct CallFilter;
impl Contains<RuntimeCall> for CallFilter {
    fn contains(call: &RuntimeCall) -> bool {
        matches!(
            call,
            RuntimeCall::System(frame_system::pallet::Call::<Test>::remark { .. })
        )
    }
}

#[cfg(feature = "runtime-benchmarks")]
pub struct DispatchBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl crate::BenchmarkHelper<Test> for DispatchBenchmarkHelper {
    fn successful_dispatch_context() -> (GenericNetworkId, GenericAdditionalInboundData, Vec<u8>) {
        let payload =
            RuntimeCall::System(frame_system::pallet::Call::<Test>::remark { remark: vec![] })
                .encode();
        (
            GenericNetworkId::EVM([1u8; 32].into()),
            AdditionalEVMInboundData {
                source: Default::default(),
            }
            .into(),
            payload,
        )
    }
}

impl dispatch::Config for Test {
    type OriginOutput =
        types::CallOriginOutput<GenericNetworkId, H256, GenericAdditionalInboundData>;
    type Origin = RuntimeOrigin;
    type MessageId = types::MessageId;
    type Hashing = Keccak256;
    type Call = RuntimeCall;
    type CallFilter = CallFilter;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = DispatchBenchmarkHelper;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    let t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    sp_io::TestExternalities::new(t)
}
