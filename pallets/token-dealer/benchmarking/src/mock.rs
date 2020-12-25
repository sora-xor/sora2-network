use crate::{Module, Trait};
use common::prelude::Balance;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    AccountId32, Perbill,
};
use sp_std::prelude::*;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

pub fn alice() -> AccountId {
    AccountId32::from(hex!(
        "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
    ))
}

pub fn bob() -> AccountId {
    AccountId32::from(hex!(
        "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
    ))
}

impl_outer_origin! {
    pub enum Origin for Runtime {}
}

#[derive(Clone, Eq, PartialEq)]
pub struct Runtime;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl system::Trait for Runtime {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = ();
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = sp_runtime::testing::Header;
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
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = ();
}

impl cumulus_token_dealer::Trait for Runtime {
    type Event = ();
    type UpwardMessageSender = MessageBroker;
    type UpwardMessage = common::prelude::RococoUpwardMessage;
    type Currency = Balances;
    type XCMPMessageSender = MessageBroker;
    type WeightInfo = ();
}

impl cumulus_message_broker::Trait for Runtime {
    type Event = ();
    type DownwardMessageHandlers = TokenDealer;
    type UpwardMessage = common::prelude::RococoUpwardMessage;
    type ParachainId = ParachainInfo;
    type XCMPMessage = cumulus_token_dealer::XCMPMessage<AccountId, Balance>;
    type XCMPMessageHandlers = TokenDealer;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
}

impl pallet_balances::Trait for Runtime {
    type Balance = Balance;
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl parachain_info::Trait for Runtime {}

impl Trait for Runtime {}

pub type System = frame_system::Module<Runtime>;
pub type Balances = pallet_balances::Module<Runtime>;
pub type MessageBroker = cumulus_message_broker::Module<Runtime>;
pub type TokenDealer = cumulus_token_dealer::Module<Runtime>;
pub type ParachainInfo = parachain_info::Module<Runtime>;

pub struct ExtBuilder {
    balances: Vec<(AccountId, Balance)>,
}

impl ExtBuilder {
    pub fn build() -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (alice(), 1_000_000_000_000_000_000u128.into()),
                (bob(), 1_000_000_000_000_000_000u128.into()),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
