use crate::{self as liquidity_proxy, Config};
use common::{
    self, fixed, fixed_from_basis_points, hash, mock::ExistentialDeposits, Amount, AssetId32,
    DEXInfo, Fixed, FromGenericPair, LiquiditySourceType, DOT, KSM, XOR,
};
use currencies::BasicCurrencyAdapter;

use frame_support::{construct_runtime, parameter_types, traits::GenesisBuild, weights::Weight};
use frame_system;

use common::prelude::Balance;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    AccountId32, Perbill,
};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type DEXId = u32;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::AssetId>;
type AssetId = AssetId32<common::AssetId>;
type ReservesInit = Vec<(DEXId, AssetId, (Fixed, Fixed))>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub fn alice() -> AccountId {
    AccountId32::from([1u8; 32])
}

pub const DEX_A_ID: DEXId = 1;
pub const DEX_B_ID: DEXId = 2;
pub const DEX_C_ID: DEXId = 3;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub GetLiquidityProxyTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            crate::TECH_ACCOUNT_PREFIX.to_vec(),
            crate::TECH_ACCOUNT_MAIN.to_vec(),
        );
        tech_account_id
    };
    pub GetLiquidityProxyAccountId: AccountId = {
        let tech_account_id = GetLiquidityProxyTechAccountId::get();
        let account_id =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub const GetNumSamples: usize = 40;
    pub const GetBaseAssetId: AssetId = XOR;
    pub const ExistentialDeposit: u128 = 1;
    pub GetFee0: Fixed = fixed_from_basis_points(0u16);
    pub GetFee10: Fixed = fixed_from_basis_points(10u16);
    pub GetFee20: Fixed = fixed_from_basis_points(20u16);
    pub GetFee30: Fixed = fixed_from_basis_points(30u16);
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        LiquidityProxy: liquidity_proxy::{Module, Call, Event<T>},
        Tokens: tokens::{Module, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Module, Call, Storage, Event<T>},
        Assets: assets::{Module, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Event<T>},
        DexManager: dex_manager::{Module, Call, Storage, Event<T>},
        MockLiquiditySource: mock_liquidity_source::<Instance1>::{Module, Call, Config<T>, Storage},
        MockLiquiditySource2: mock_liquidity_source::<Instance2>::{Module, Call, Config<T>, Storage},
        MockLiquiditySource3: mock_liquidity_source::<Instance3>::{Module, Call, Config<T>, Storage},
        MockLiquiditySource4: mock_liquidity_source::<Instance4>::{Module, Call, Config<T>, Storage},
        Technical: technical::{Module, Call, Storage, Event<T>},
        Permissions: permissions::{Module, Call, Config<T>, Storage, Event<T>},
        DexApi: dex_api::{Module, Call, Config, Storage, Event<T>},
        TradingPair: trading_pair::{Module, Call, Storage, Event<T>},
    }
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
    type LiquidityRegistry = dex_api::Module<Runtime>;
    type GetNumSamples = GetNumSamples;
    type GetTechnicalAccountId = GetLiquidityProxyAccountId;
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

impl currencies::Config for Runtime {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = GetBaseAssetId;
    type WeightInfo = ();
}

impl assets::Config for Runtime {
    type Event = Event;
    type ExtraAccountId = [u8; 32];
    type ExtraTupleArg = common::AssetIdExtraTupleArg<DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
    type WeightInfo = ();
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl dex_manager::Config for Runtime {
    type Event = Event;
    type WeightInfo = ();
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance1> for Runtime {
    type GetFee = GetFee0;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance2> for Runtime {
    type GetFee = GetFee10;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance3> for Runtime {
    type GetFee = GetFee20;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance4> for Runtime {
    type GetFee = GetFee30;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl technical::Config for Runtime {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
    type WeightInfo = ();
}

impl permissions::Config for Runtime {
    type Event = Event;
}

impl dex_api::Config for Runtime {
    type Event = Event;
    type MockLiquiditySource =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance1>;
    type MockLiquiditySource2 =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance2>;
    type MockLiquiditySource3 =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance3>;
    type MockLiquiditySource4 =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance4>;
    type BondingCurvePool = ();
    type XYKPool = ();
    type MulticollateralBondingCurvePool = ();
    type WeightInfo = ();
}

impl trading_pair::Config for Runtime {
    type Event = Event;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

pub struct ExtBuilder {
    pub reserves: ReservesInit,
    pub reserves_2: ReservesInit,
    pub reserves_3: ReservesInit,
    pub reserves_4: ReservesInit,
    pub dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    pub initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    pub initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    pub source_types: Vec<LiquiditySourceType>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            reserves: vec![
                (DEX_A_ID, DOT, (fixed!(5000), fixed!(7000))),
                (DEX_A_ID, KSM, (fixed!(5500), fixed!(4000))),
                (DEX_B_ID, DOT, (fixed!(100), fixed!(45))),
                (DEX_C_ID, DOT, (fixed!(520), fixed!(550))),
            ],
            reserves_2: vec![
                (DEX_A_ID, DOT, (fixed!(6000), fixed!(6000))),
                (DEX_A_ID, KSM, (fixed!(6500), fixed!(3000))),
                (DEX_B_ID, DOT, (fixed!(200), fixed!(45))),
                (DEX_C_ID, DOT, (fixed!(550), fixed!(700))),
            ],
            reserves_3: vec![
                (DEX_A_ID, DOT, (fixed!(7000), fixed!(5000))),
                (DEX_A_ID, KSM, (fixed!(7500), fixed!(2000))),
                (DEX_B_ID, DOT, (fixed!(300), fixed!(45))),
                (DEX_C_ID, DOT, (fixed!(400), fixed!(380))),
            ],
            reserves_4: vec![
                (DEX_A_ID, DOT, (fixed!(8000), fixed!(4000))),
                (DEX_A_ID, KSM, (fixed!(8500), fixed!(1000))),
                (DEX_B_ID, DOT, (fixed!(400), fixed!(45))),
                (DEX_C_ID, DOT, (fixed!(1300), fixed!(1800))),
            ],
            dex_list: vec![
                (
                    DEX_A_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        is_public: true,
                    },
                ),
                (
                    DEX_B_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        is_public: true,
                    },
                ),
                (
                    DEX_C_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        is_public: true,
                    },
                ),
            ],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_B_ID)), vec![alice()]),
            ],
            initial_permissions: vec![
                (alice(), Scope::Unlimited, vec![INIT_DEX]),
                (alice(), Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
                (alice(), Scope::Limited(hash(&DEX_B_ID)), vec![MANAGE_DEX]),
            ],
            source_types: vec![
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4,
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: self.initial_permission_owners,
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance1> {
            reserves: self.reserves,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance2> {
            reserves: self.reserves_2,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance3> {
            reserves: self.reserves_3,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance4> {
            reserves: self.reserves_4,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        <dex_api::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
            &dex_api::GenesisConfig {
                source_types: self.source_types,
            },
            &mut t,
        )
        .unwrap();

        t.into()
    }
}
