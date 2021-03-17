use crate::{self as trading_pair, Config};
use common::{
    hash,
    mock::ExistentialDeposits,
    prelude::{Balance, DEXInfo},
    AssetId32, AssetSymbol, BalancePrecision, DOT, KSM, XOR,
};
use currencies::BasicCurrencyAdapter;
use frame_support::{construct_runtime, parameter_types, traits::GenesisBuild, weights::Weight};
use frame_system;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup, Zero},
    Perbill,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        TradingPair: trading_pair::{Module, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Module, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Module, Call, Storage, Event<T>},
        Assets: assets::{Module, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Event<T>},
        Permissions: permissions::{Module, Call, Config<T>, Storage, Event<T>},
        DexManager: dex_manager::{Module, Call, Config<T>, Storage, Event<T>},
    }
}

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type Amount = i128;

pub const ALICE: AccountId = 1;
pub const DEX_ID: DEXId = 1;
type AssetId = AssetId32<common::AssetId>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
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
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = PalletInfo;
    type SS58Prefix = ();
}

impl Config for Runtime {
    type Event = Event;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

impl tokens::Config for Runtime {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type OnDust = ();
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
}

impl currencies::Config for Runtime {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

type DEXId = u32;

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

impl assets::Config for Runtime {
    type Event = Event;
    type ExtraAccountId = AccountId;
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<DEXId, common::LiquiditySourceType, AccountId>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
    type WeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl permissions::Config for Runtime {
    type Event = Event;
}

impl dex_manager::Config for Runtime {
    type Event = Event;
    type WeightInfo = ();
}

pub struct ExtBuilder {
    endowed_assets: Vec<(
        AssetId,
        AccountId,
        AssetSymbol,
        BalancePrecision,
        Balance,
        bool,
    )>,
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
}

impl ExtBuilder {
    pub fn without_initialized_dex() -> Self {
        Self {
            endowed_assets: vec![
                (
                    XOR,
                    ALICE,
                    AssetSymbol(b"XOR".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                ),
                (
                    DOT,
                    ALICE,
                    AssetSymbol(b"DOT".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                ),
                (
                    KSM,
                    ALICE,
                    AssetSymbol(b"DOT".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                ),
            ],
            endowed_accounts: vec![],
            dex_list: vec![],
            initial_permission_owners: vec![],
            initial_permissions: vec![],
        }
    }
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_assets: vec![
                (
                    XOR,
                    ALICE,
                    AssetSymbol(b"XOR".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                (
                    DOT,
                    ALICE,
                    AssetSymbol(b"DOT".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                (
                    KSM,
                    ALICE,
                    AssetSymbol(b"KSM".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
            ],
            endowed_accounts: vec![],
            dex_list: vec![(
                DEX_ID,
                DEXInfo {
                    base_asset_id: XOR,
                    is_public: true,
                },
            )],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![ALICE]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_ID)), vec![ALICE]),
            ],
            initial_permissions: vec![
                (ALICE, Scope::Unlimited, vec![INIT_DEX]),
                (ALICE, Scope::Limited(hash(&DEX_ID)), vec![MANAGE_DEX]),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: self.initial_permission_owners,
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Runtime> {
            endowed_assets: self.endowed_assets,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            endowed_accounts: self.endowed_accounts,
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
