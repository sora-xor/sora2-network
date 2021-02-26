use crate::{Module, Trait};
use common::prelude::Balance;
use common::{AssetId32, XOR};
use currencies::BasicCurrencyAdapter;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use sp_core::H256;
use sp_runtime::traits::Zero;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type Amount = i128;
type AssetId = AssetId32<common::AssetId>;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

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
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = ();
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
}

impl crate::Trait for Runtime {
    type Event = ();
    type ExtraDEXId = common::DEXId;
    type ExtraLstId = common::LiquiditySourceType;
    type ExtraAccountId = AccountId;
    type ExtraTupleArg =
        common::AssetIdExtraTupleArg<common::DEXId, common::LiquiditySourceType, AccountId>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
    type WeightInfo = ();
}

impl permissions::Trait for Runtime {
    type Event = ();
}

impl tokens::Trait for Runtime {
    type Event = ();
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as Trait>::AssetId;
    type OnReceived = ();
    type WeightInfo = ();
}

impl currencies::Trait for Runtime {
    type Event = ();
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as Trait>::GetBaseAssetId;
    type WeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
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

pub type System = frame_system::Module<Runtime>;
pub type Balances = pallet_balances::Module<Runtime>;
pub type Tokens = tokens::Module<Runtime>;
pub type Assets = Module<Runtime>;

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![(ALICE, XOR, Balance::zero())],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: vec![],
            initial_permissions: vec![],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            endowed_accounts: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
