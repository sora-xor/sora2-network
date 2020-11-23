use crate::{GenesisConfig, Module, Scope, Trait, INIT_DEX, MINT, TRANSFER};
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use sp_core::{H256, H512};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

pub type AccountId = u128;

impl_outer_origin! {
    pub enum Origin for Test {}
}

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const JOHN: AccountId = 3;

#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl system::Trait for Test {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = ();
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type ModuleToIndex = ();
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
}

impl Trait for Test {
    type Event = ();
}

pub type PermissionsModule = Module<Test>;

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
        let mut t = system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        GenesisConfig::<Test> {
            initial_permission_owners: self.initial_permission_owners,
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
