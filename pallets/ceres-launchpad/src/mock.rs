use crate::{self as ceres_launchpad};
use common::mock::ExistentialDeposits;
use common::prelude::Balance;
use common::{
    balance, fixed, AssetId32, AssetName, AssetSymbol, BalancePrecision, ContentSource,
    Description, Fixed, XOR,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{GenesisBuild, Hooks};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup, Zero};
use sp_runtime::Perbill;

pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
type DEXId = common::DEXId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        Assets: assets::{Module, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Module, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Module, Call, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Event<T>},
        DexManager: dex_manager::{Module, Call, Config<T>, Storage},
        TradingPair: trading_pair::{Module, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Module, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Module, Call, Config<T>, Storage, Event<T>},
        PoolXYK: pool_xyk::{Module, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Module, Call, Config<T>, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Module, Call, Storage, Event<T>},
        CeresLaunchpad: ceres_launchpad::{Module, Call, Storage, Event<T>}
    }
}

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type Amount = i128;
pub type AssetId = AssetId32<common::PredefinedAssetId>;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLES: AccountId = 3;
pub const CERES_ASSET_ID: AssetId = common::AssetId32::from_bytes(hex!(
    "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
));

pub const BLOCKS_PER_DAY: BlockNumberFor<Runtime> = 14_440;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub GetXykFee: Fixed = fixed!(0.003);
    pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = 100;
    pub GetPswapDistributionAccountId: AccountId = 101;
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
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
}

impl crate::Config for Runtime {
    type Event = Event;
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetTeamReservesAccountId: AccountId = 3000u128;
}

impl assets::Config for Runtime {
    type Event = Event;
    type ExtraAccountId = AccountId;
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<common::DEXId, common::LiquiditySourceType, AccountId>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
    type GetTeamReservesAccountId = GetTeamReservesAccountId;
    type GetTotalBalance = ();
    type WeightInfo = ();
}

impl common::Config for Runtime {
    type DEXId = common::DEXId;
    type LstId = common::LiquiditySourceType;
}

impl permissions::Config for Runtime {
    type Event = Event;
}

impl dex_manager::Config for Runtime {}

impl trading_pair::Config for Runtime {
    type Event = Event;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

impl pool_xyk::Config for Runtime {
    const MIN_XOR: Balance = balance!(0.0007);
    type Event = Event;
    type PairSwapAction = pool_xyk::PairSwapAction<AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type GetFee = GetXykFee;
    type OnPoolCreated = PswapDistribution;
    type OnPoolReservesChanged = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const CeresAssetId: AssetId = CERES_ASSET_ID;
}

impl ceres_liquidity_locker::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self> = 14_440;
    type Event = Event;
    type XYKPool = PoolXYK;
    type CeresAssetId = CeresAssetId;
    type WeightInfo = ();
}

impl pswap_distribution::Config for Runtime {
    type Event = Event;
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type LiquidityProxy = ();
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetBurnUpdateFrequency = GetBurnUpdateFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = ();
    type OnPswapBurnedAggregator = ();
    type WeightInfo = ();
    type GetParliamentAccountId = GetParliamentAccountId;
    type PoolXykPallet = PoolXYK;
}

impl technical::Config for Runtime {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
}

impl tokens::Config for Runtime {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type OnDust = ();
}

impl currencies::Config for Runtime {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
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

pub struct ExtBuilder {
    endowed_assets: Vec<(
        AssetId,
        AccountId,
        AssetSymbol,
        AssetName,
        BalancePrecision,
        Balance,
        bool,
        Option<ContentSource>,
        Option<Description>,
    )>,
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_assets: vec![(
                CERES_ASSET_ID,
                ALICE,
                AssetSymbol(b"CERES".to_vec()),
                AssetName(b"Ceres".to_vec()),
                18,
                Balance::zero(),
                true,
                None,
                None,
            )],
            endowed_accounts: vec![
                (ALICE, CERES_ASSET_ID, balance!(15000)),
                (BOB, CERES_ASSET_ID, balance!(5)),
                (CHARLES, CERES_ASSET_ID, balance!(3000)),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = SystemConfig::default().build_storage::<Runtime>().unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: self
                .endowed_accounts
                .iter()
                .map(|(acc, _, balance)| (*acc, *balance))
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        PermissionsConfig {
            initial_permission_owners: vec![],
            initial_permissions: vec![],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Runtime> {
            endowed_assets: self.endowed_assets,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        TokensConfig {
            endowed_accounts: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
    }
}
