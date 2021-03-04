use crate::{GenesisConfig, Trait};
use common::{
    self, balance, prelude::Balance, Amount, AssetId32, AssetSymbol, TechPurpose, USDT, VAL, XOR,
};
use currencies::BasicCurrencyAdapter;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use permissions::{Scope, BURN, MINT};
use sp_core::{crypto::AccountId32, H256};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

type DEXId = common::DEXId;
type AccountId = AccountId32;
type BlockNumber = u64;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::AssetId>;
type Balances = pallet_balances::Module<Test>;
type Tokens = tokens::Module<Test>;
type System = frame_system::Module<Test>;
type Technical = technical::Module<Test>;
type AssetId = AssetId32<common::AssetId>;

pub fn alice() -> AccountId32 {
    AccountId32::from([1u8; 32])
}

pub fn bob() -> AccountId32 {
    AccountId32::from([2u8; 32])
}

pub fn tech_account_id() -> TechAccountId {
    TechAccountId::Pure(
        DEXId::Polkaswap,
        TechPurpose::Identifier(b"faucet_tech_account_id".to_vec()),
    )
}

pub fn account_id() -> AccountId {
    Technical::tech_account_id_to_account_id(&tech_account_id()).unwrap()
}

pub const NOT_SUPPORTED_ASSET_ID: AssetId = USDT;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId = XOR;
    pub const ExistentialDeposit: u128 = 0;
}

#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct Test;

impl_outer_origin! {
    pub enum Origin for Test {}
}

impl Trait for Test {
    type Event = ();
}

impl frame_system::Trait for Test {
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

impl technical::Trait for Test {
    type Event = ();
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
    type WeightInfo = ();
}

impl assets::Trait for Test {
    type Event = ();
    type ExtraLstId = common::LiquiditySourceType;
    type ExtraAccountId = [u8; 32];
    type ExtraTupleArg =
        common::AssetIdExtraTupleArg<common::DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Test>;
    type WeightInfo = ();
}

impl common::Trait for Test {
    type DEXId = DEXId;
}

// Required by assets::Trait
impl permissions::Trait for Test {
    type Event = ();
}

// Required by assets::Trait
impl currencies::Trait for Test {
    type Event = ();
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Test as assets::Trait>::GetBaseAssetId;
    type WeightInfo = ();
}

// Required by currencies::Trait
impl pallet_balances::Trait for Test {
    type Balance = Balance;
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

// Required by assets::Trait
impl tokens::Trait for Test {
    type Event = ();
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Test as assets::Trait>::AssetId;
    type OnReceived = ();
    type WeightInfo = ();
}

pub struct ExtBuilder {}

impl ExtBuilder {
    pub fn build() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        let tech_account_id = tech_account_id();
        let account_id: AccountId = account_id();

        pallet_balances::GenesisConfig::<Test> {
            balances: vec![(account_id.clone(), balance!(150))],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Test> {
            initial_permission_owners: vec![
                (MINT, Scope::Unlimited, vec![account_id.clone()]),
                (BURN, Scope::Unlimited, vec![account_id.clone()]),
            ],
            initial_permissions: vec![(account_id.clone(), Scope::Unlimited, vec![MINT, BURN])],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Test> {
            endowed_assets: vec![
                (
                    XOR,
                    alice(),
                    AssetSymbol(b"XOR".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                ),
                (
                    VAL.into(),
                    alice(),
                    AssetSymbol(b"VAL".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                ),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Test> {
            endowed_accounts: vec![(account_id.clone(), VAL.into(), balance!(150))],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        technical::GenesisConfig::<Test> {
            account_ids_to_tech_account_ids: vec![(account_id, tech_account_id.clone())],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        GenesisConfig::<Test> {
            reserves_account_id: tech_account_id,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
