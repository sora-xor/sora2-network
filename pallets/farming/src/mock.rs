use crate::{Module, Trait};
use common::prelude::{AssetId, Balance};
use common::Amount;
use currencies::BasicCurrencyAdapter;
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use sp_core::crypto::AccountId32;
use sp_core::crypto::UncheckedInto;
use sp_core::{H256, H512};
use sp_runtime::traits::Zero;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

pub type AccountId = u128;
pub type BlockNumber = u64;
type TechAccountIdPrimitive = common::TechAccountId<AccountId, AssetId, DEXId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<AssetId, DEXId>;
type DEXId = common::DEXId;
pub type FarmsModule = Module<Test>;
pub type System = frame_system::Module<Test>;
pub type Balances = pallet_balances::Module<Test>;
pub type Tokens = tokens::Module<Test>;
pub type Currencies = currencies::Module<Test>;
pub type BondingCurvePool = Module<Test>;
pub type Technical = technical::Module<Test>;
pub type MockLiquiditySource =
    mock_liquidity_source::ReservesAcc<Test, mock_liquidity_source::Instance1>;
pub type Assets = assets::Module<Test>;

pub const XOR: AssetId = AssetId::XOR;
pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const NICK: AccountId = 3;

impl_outer_origin! {
    pub enum Origin for Test {}
}

impl_outer_event! {
    pub enum Event for Test {
        frame_system<T>,
        pallet_balances<T>,
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct Test;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId = AssetId::XOR;
    pub const ExistentialDeposit: u128 = 0;
    pub const MinimumPeriod: u64 = 5;
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
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
}

impl common::Trait for Test {
    type DEXId = DEXId;
}

impl technical::Trait for Test {
    type Event = ();
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
}

impl currencies::Trait for Test {
    type Event = ();
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Balances, Balance, Balance, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Test as assets::Trait>::GetBaseAssetId;
}

impl assets::Trait for Test {
    type Event = ();
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Test>;
}

impl permissions::Trait for Test {
    type Event = ();
}

impl pallet_balances::Trait for Test {
    type Balance = Balance;
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

impl tokens::Trait for Test {
    type Event = ();
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Test as assets::Trait>::AssetId;
    type OnReceived = ();
}

impl pallet_timestamp::Trait for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl Trait for Test {
    type Event = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    ExtBuilder::default().build()
}

pub struct ExtBuilder {
    initial_permissions: Vec<(u32, AccountId, AccountId, Option<H512>)>,
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            initial_permissions: vec![
                (permissions::CREATE_FARM, ALICE, ALICE, None),
                (permissions::TRANSFER, BOB, BOB, None),
                (permissions::TRANSFER, NICK, NICK, None),
                (permissions::INVEST_TO_FARM, BOB, BOB, None),
                (permissions::INVEST_TO_FARM, NICK, NICK, None),
                (permissions::CLAIM_FROM_FARM, BOB, BOB, None),
            ],
            endowed_accounts: vec![
                (ALICE, XOR, 1_000_000_u128.into()),
                (BOB, XOR, 1_000_000_u128.into()),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        permissions::GenesisConfig::<Test> {
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Test> {
            endowed_accounts: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
