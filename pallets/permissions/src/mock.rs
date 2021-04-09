use crate::{self as permissions, Config, Scope, INIT_DEX, MINT, TRANSFER};
use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use sp_core::{H256, H512};
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::Perbill;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        Permissions: permissions::{Module, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
    }
}

pub type AccountId = u128;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const JOHN: AccountId = 3;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const ExistentialDeposit: u128 = 0;
}

impl pallet_balances::Config for Runtime {
    type Balance = u128;
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = ();
    type BlockWeights = ();
    type BlockLength = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type AccountData = pallet_balances::AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = PalletInfo;
    type SS58Prefix = ();
}

impl Config for Runtime {
    type Event = Event;
}

pub struct ExtBuilder {
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            initial_permission_owners: vec![
                (TRANSFER, Scope::Unlimited, vec![ALICE]),
                (INIT_DEX, Scope::Unlimited, vec![ALICE]),
                (MINT, Scope::Unlimited, vec![JOHN]),
            ],
            initial_permissions: vec![
                (ALICE, Scope::Unlimited, vec![TRANSFER]), // Alice is forbidden to transfer
                (BOB, Scope::Unlimited, vec![INIT_DEX]),
                (BOB, Scope::Limited(H512::repeat_byte(1)), vec![TRANSFER]), // Bob is forbidden to transfer
                (JOHN, Scope::Unlimited, vec![MINT]),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![(ALICE, 0), (BOB, 0), (JOHN, 0)],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        PermissionsConfig {
            initial_permission_owners: self.initial_permission_owners,
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
