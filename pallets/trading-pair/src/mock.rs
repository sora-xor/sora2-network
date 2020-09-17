use crate::{Module, Trait};
use common::{
    hash,
    prelude::{AssetId, Balance, DEXInfo},
    BasisPoints,
};
use currencies::BasicCurrencyAdapter;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use permissions::{INIT_DEX, MANAGE_DEX};
use sp_core::{H256, H512};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type Amount = i128;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const XOR: AssetId = AssetId::XOR;
pub const DOT: AssetId = AssetId::DOT;
pub const DEX_ID: DEXId = 1;

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
    type ModuleToIndex = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
}

impl Trait for Runtime {
    type Event = ();
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
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
    type GetNativeCurrencyId = <Runtime as assets::Trait>::GetBaseAssetId;
}

type DEXId = u32;

impl common::Trait for Runtime {
    type DEXId = DEXId;
}

impl assets::Trait for Runtime {
    type Event = ();
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
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
}

parameter_types! {
    pub const GetDefaultFee: BasisPoints = 30;
    pub const GetDefaultProtocolFee: BasisPoints = 0;
}

impl permissions::Trait for Runtime {
    type Event = ();
}

impl dex_manager::Trait for Runtime {
    type Event = ();
    type GetDefaultFee = GetDefaultFee;
    type GetDefaultProtocolFee = GetDefaultProtocolFee;
}

pub type System = frame_system::Module<Runtime>;
pub type Balances = pallet_balances::Module<Runtime>;
pub type Tokens = tokens::Module<Runtime>;
pub type TradingPair = Module<Runtime>;

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permissions: Vec<(u32, AccountId, AccountId, Option<H512>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![
                (ALICE, XOR, 1_000_000_000_000_000_000u128.into()),
                (BOB, DOT, 1_000_000_000_000_000_000u128.into()),
            ],
            dex_list: vec![(
                DEX_ID,
                DEXInfo {
                    base_asset_id: XOR,
                    default_fee: 30,
                    default_protocol_fee: 0,
                },
            )],
            initial_permissions: vec![
                (INIT_DEX, ALICE, ALICE, None),
                (MANAGE_DEX, ALICE, ALICE, Some(hash(&DEX_ID))),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        tokens::GenesisConfig::<Runtime> {
            endowed_accounts: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
