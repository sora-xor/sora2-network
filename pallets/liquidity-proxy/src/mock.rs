use crate::{Module, Trait};
use common::{fixed_from_basis_points, hash, Amount, AssetId, DEXInfo, Fixed};
use currencies::BasicCurrencyAdapter;

use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;

use common::prelude::Balance;
use permissions::{INIT_DEX, MANAGE_DEX};
use sp_core::{H256, H512};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    AccountId32, Perbill,
};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<AssetId, DEXId>;

pub fn alice() -> AccountId {
    AccountId32::from([1u8; 32])
}
pub const DOT: AssetId = AssetId::DOT;
pub const KSM: AssetId = AssetId::KSM;
pub const DEX_A_ID: DEXId = 1;
pub const DEX_B_ID: DEXId = 2;
pub const DEX_C_ID: DEXId = 3;

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

parameter_types! {
    pub const GetNumSamples: usize = 100;
}
impl Trait for Runtime {
    type Event = ();
    type LiquidityRegistry = dex_api::Module<Runtime>;
    type GetNumSamples = GetNumSamples;
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

impl assets::Trait for Runtime {
    type Event = ();
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
}

pub type DEXId = u32;

impl common::Trait for Runtime {
    type DEXId = DEXId;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
}

impl pallet_balances::Trait for Runtime {
    type Balance = Balance;
    type DustRemoval = ();
    type Event = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

parameter_types! {
    pub const GetDefaultFee: u16 = 30;
    pub const GetDefaultProtocolFee: u16 = 0;
}
impl dex_manager::Trait for Runtime {
    type Event = ();
    type GetDefaultFee = GetDefaultFee;
    type GetDefaultProtocolFee = GetDefaultProtocolFee;
}

parameter_types! {
    pub GetFee0: Fixed = fixed_from_basis_points(0u16);
    pub GetFee10: Fixed = fixed_from_basis_points(10u16);
    pub GetFee20: Fixed = fixed_from_basis_points(20u16);
    pub GetFee30: Fixed = fixed_from_basis_points(30u16);
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance1> for Runtime {
    type Event = ();
    type GetFee = GetFee0;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance2> for Runtime {
    type Event = ();
    type GetFee = GetFee10;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance3> for Runtime {
    type Event = ();
    type GetFee = GetFee20;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance4> for Runtime {
    type Event = ();
    type GetFee = GetFee30;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl technical::Trait for Runtime {
    type Event = ();
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
}

impl permissions::Trait for Runtime {
    type Event = ();
}

impl dex_api::Trait for Runtime {
    type Event = ();
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
}

impl trading_pair::Trait for Runtime {
    type Event = ();
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
}

pub type System = frame_system::Module<Runtime>;
pub type Balances = pallet_balances::Module<Runtime>;
pub type Tokens = tokens::Module<Runtime>;
pub type LiquidityProxy = Module<Runtime>;

type ReservesInit = Vec<(DEXId, AssetId, (Fixed, Fixed))>;

pub struct ExtBuilder {
    reserves: ReservesInit,
    reserves_2: ReservesInit,
    reserves_3: ReservesInit,
    reserves_4: ReservesInit,
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permissions: Vec<(u32, AccountId, AccountId, Option<H512>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            reserves: vec![
                (DEX_A_ID, DOT, (Fixed::from(5000), Fixed::from(7000))),
                (DEX_A_ID, KSM, (Fixed::from(5500), Fixed::from(4000))),
                (DEX_B_ID, DOT, (Fixed::from(100), Fixed::from(45))),
                (DEX_C_ID, DOT, (Fixed::from(520), Fixed::from(550))),
            ],
            reserves_2: vec![
                (DEX_A_ID, DOT, (Fixed::from(6000), Fixed::from(6000))),
                (DEX_A_ID, KSM, (Fixed::from(6500), Fixed::from(3000))),
                (DEX_B_ID, DOT, (Fixed::from(200), Fixed::from(45))),
                (DEX_C_ID, DOT, (Fixed::from(550), Fixed::from(700))),
            ],
            reserves_3: vec![
                (DEX_A_ID, DOT, (Fixed::from(7000), Fixed::from(5000))),
                (DEX_A_ID, KSM, (Fixed::from(7500), Fixed::from(2000))),
                (DEX_B_ID, DOT, (Fixed::from(300), Fixed::from(45))),
                (DEX_C_ID, DOT, (Fixed::from(400), Fixed::from(380))),
            ],
            reserves_4: vec![
                (DEX_A_ID, DOT, (Fixed::from(8000), Fixed::from(4000))),
                (DEX_A_ID, KSM, (Fixed::from(8500), Fixed::from(1000))),
                (DEX_B_ID, DOT, (Fixed::from(400), Fixed::from(45))),
                (DEX_C_ID, DOT, (Fixed::from(1300), Fixed::from(1800))),
            ],
            dex_list: vec![
                (
                    DEX_A_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        default_fee: GetDefaultFee::get(),
                        default_protocol_fee: GetDefaultProtocolFee::get(),
                    },
                ),
                (
                    DEX_B_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        default_fee: GetDefaultFee::get(),
                        default_protocol_fee: GetDefaultProtocolFee::get(),
                    },
                ),
                (
                    DEX_C_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        default_fee: GetDefaultFee::get(),
                        default_protocol_fee: GetDefaultProtocolFee::get(),
                    },
                ),
            ],
            initial_permissions: vec![
                (INIT_DEX, alice(), alice(), None),
                (MANAGE_DEX, alice(), alice(), Some(hash(&DEX_A_ID))),
                (MANAGE_DEX, alice(), alice(), Some(hash(&DEX_B_ID))),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
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

        t.into()
    }
}
