use crate::{Module, Trait};
use common::{fixed_from_basis_points, Amount, AssetId, Fixed};
use currencies::BasicCurrencyAdapter;

use common::prelude::Balance;
use frame_support::sp_runtime::AccountId32;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

type TechAssetId = common::TechAssetId<AssetId, DEXId>;
type TechAccountIdPrimitive = common::TechAccountId<AccountId, AssetId, DEXId>;
// type TechAccountId = crate::TechAccountIdReprCompat<Testtime, TechAccountIdPrimitive>;
type TechAmount = Amount;
type TechBalance = Balance;

pub fn alice() -> AccountId {
    AccountId32::from([1u8; 32])
}

pub const DOT: AssetId = AssetId::DOT;
pub const KSM: AssetId = AssetId::KSM;
pub const DEX_A_ID: DEXId = 1;
pub const DEX_B_ID: DEXId = 2;

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
    pub const GetDefaultFee: u16 = 30;
    pub const GetDefaultProtocolFee: u16 = 0;
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
    type ModuleToIndex = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
}

parameter_types! {
    pub GetFee: Fixed = fixed_from_basis_points(30u16);
}

impl Trait<crate::Instance1> for Runtime {
    type Event = ();
    type GetFee = GetFee;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl Trait<crate::Instance2> for Runtime {
    type Event = ();
    type GetFee = GetFee;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl technical::Trait for Runtime {
    type Event = ();
    type TechAssetId = TechAssetId;
    type TechAccountIdPrimitive = TechAccountIdPrimitive;
    type TechAmount = TechAmount;
    type TechBalance = TechBalance;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
}

impl tokens::Trait for Runtime {
    type Event = ();
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Trait>::AssetId;
    type OnReceived = ();
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = AssetId::XOR;
}

impl currencies::Trait for Runtime {
    type Event = ();
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Balances, Balance, Balance, Amount, BlockNumber>;
    type GetNativeCurrencyId = GetBaseAssetId;
}

type DEXId = u32;

impl assets::Trait for Runtime {
    type Event = ();
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
}

impl common::Trait for Runtime {
    type DEXId = DEXId;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
}

impl pallet_balances::Trait for Runtime {
    type Balance = Balance;
    type DustRemoval = ();
    type Event = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

impl permissions::Trait for Runtime {
    type Event = ();
}

impl dex_manager::Trait for Runtime {
    type Event = ();
    type GetDefaultFee = GetDefaultFee;
    type GetDefaultProtocolFee = GetDefaultProtocolFee;
}

impl trading_pair::Trait for Runtime {
    type Event = ();
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
}

pub type System = frame_system::Module<Runtime>;
pub type Balances = pallet_balances::Module<Runtime>;
pub type Tokens = tokens::Module<Runtime>;
pub type MockLiquiditySource = Module<Runtime, crate::Instance1>;
pub type MockLiquiditySource2 = Module<Runtime, crate::Instance2>;

pub struct ExtBuilder {
    // add additional fields for other pallets genesis
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            // add values for mock genesis
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let t = system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        // Add additional pallets genesis

        t.into()
    }
}
