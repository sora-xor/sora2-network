// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate as bridge_halt;
use frame_support::{derive_impl, traits::ConstU32};
use frame_system::EnsureRoot;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        BridgeHalt: bridge_halt,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
}

pub type MaxReasonLength = ConstU32<64>;
pub const COOLDOWN: u64 = 10;
pub type HaltCooldown = frame_support::traits::ConstU64<COOLDOWN>;

impl bridge_halt::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type GuardianOrigin = EnsureRoot<u64>;
    type MaxGuardians = ConstU32<3>;
    type MaxReasonLength = MaxReasonLength;
    type HaltCooldown = HaltCooldown;
    type WeightInfo = ();
}

pub const GUARDIAN: u64 = 1;
pub const OTHER: u64 = 2;

pub fn new_test_ext() -> sp_io::TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    let mut ext = sp_io::TestExternalities::new(storage);
    // Events are not recorded in block zero.
    ext.execute_with(|| System::set_block_number(1));
    ext
}
