use crate::{self as ceres_launchpad};
pub use common::mock::*;
use common::mock::{ExistentialDeposits, GetTradingPairRestrictedFlag};
use common::prelude::Balance;
use common::ContentSource;
use common::Description;
pub use common::TechAssetId as Tas;
pub use common::TechPurpose::*;
use common::{balance, fixed, hash, DEXId, DEXInfo, Fixed, CERES_ASSET_ID, TBCD, XOR, XST};
use common::{mock_assets_config, AssetSymbol};
use common::{AssetName, XSTUSD};
use common::{BalancePrecision, PSWAP};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{Everything, GenesisBuild, Hooks};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system::pallet_prelude::BlockNumberFor;
use permissions::{Scope, MANAGE_DEX};
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify};
use sp_runtime::{MultiSignature, Perbill, Percent};

pub type BlockNumber = u64;

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
pub type Amount = i128;
pub type AssetId = common::AssetId32<common::PredefinedAssetId>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Currencies: currencies::{Pallet, Call, Storage},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
        TradingPair: trading_pair::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Config<T>, Storage, Event<T>},
        MBCPool: multicollateral_bonding_curve_pool::{Pallet, Call, Config<T>, Storage, Event<T>},
        VestedRewards: vested_rewards::{Pallet, Call, Storage, Event<T>},
        CeresTokenLocker: ceres_token_locker::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        CeresLaunchpad: ceres_launchpad::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
    }
}

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const CHARLES: AccountId = AccountId32::new([3u8; 32]);
pub const DAN: AccountId = AccountId32::new([4; 32]);
pub const EMILY: AccountId = AccountId32::new([5u8; 32]);
pub const DEX_A_ID: DEXId = DEXId::Polkaswap;
pub const DEX_B_ID: DEXId = DEXId::PolkaswapXSTUSD;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub GetXykFee: Fixed = fixed!(0.003);
    pub GetIncentiveAssetId: AssetId = PSWAP;
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::new([100u8; 32]);
    pub GetPswapDistributionAccountId: AccountId = AccountId32::new([101u8; 32]);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::new([102u8; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::new([103u8; 32]);
    pub GetFarmingRewardsAccountId: AccountId = AccountId32::new([104u8; 32]);
    pub GetCrowdloanRewardsAccountId: AccountId = AccountId32::new([105u8; 32]);
    pub const MinimumPeriod: u64 = 5;
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<65536>;
}

impl crate::Config for Runtime {
    const MILLISECONDS_PER_DAY: Self::Moment = 86_400_000;
    type RuntimeEvent = RuntimeEvent;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = TBCD;
    pub GetTBCBuyBackTBCDPercent: Fixed = fixed!(0.025);
}

mock_assets_config!(Runtime);

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
    type AssetManager = assets::Pallet<Runtime>;
    type MultiCurrency = currencies::Pallet<Runtime>;
}

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl dex_manager::Config for Runtime {}

impl trading_pair::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl demeter_farming_platform::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DemeterAssetId = ();
    const BLOCKS_PER_HOUR_AND_A_HALF: BlockNumberFor<Self> = 900;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl pool_xyk::Config for Runtime {
    const MIN_XOR: Balance = balance!(0.0007);
    type RuntimeEvent = RuntimeEvent;
    type PairSwapAction = pool_xyk::PairSwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type EnabledSourcesManager = trading_pair::Pallet<Runtime>;
    type GetFee = GetXykFee;
    type OnPoolCreated = PswapDistribution;
    type OnPoolReservesChanged = ();
    type WeightInfo = ();
    type XSTMarketInfo = ();
    type GetTradingPairRestrictedFlag = GetTradingPairRestrictedFlag;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl multicollateral_bonding_curve_pool::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = ();
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type PriceToolsPallet = ();
    type VestedRewardsPallet = VestedRewards;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type BuyBackHandler = ();
    type BuyBackTBCDPercent = GetTBCBuyBackTBCDPercent;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl vested_rewards::Config for Runtime {
    const BLOCKS_PER_DAY: BlockNumberFor<Self> = 14400;
    type RuntimeEvent = RuntimeEvent;
    type GetMarketMakerRewardsAccountId = GetMarketMakerRewardsAccountId;
    type GetBondingCurveRewardsAccountId = GetBondingCurveRewardsAccountId;
    type GetFarmingRewardsAccountId = GetFarmingRewardsAccountId;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

parameter_types! {
    pub const CeresAssetId: AssetId = CERES_ASSET_ID;
}

impl ceres_token_locker::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CeresAssetId = CeresAssetId;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl ceres_liquidity_locker::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self> = 14_440;
    type RuntimeEvent = RuntimeEvent;
    type XYKPool = PoolXYK;
    type DemeterFarmingPlatform = DemeterFarmingPlatform;
    type CeresAssetId = CeresAssetId;
    type WeightInfo = ();
}

impl pswap_distribution::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    const PSWAP_BURN_PERCENT: Percent = Percent::from_percent(3);
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type GetTBCDAssetId = GetBuyBackAssetId;
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
    type BuyBackHandler = ();
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl technical::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type CurrencyHooks = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type DustRemovalWhitelist = Everything;
}

impl currencies::Config for Runtime {
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
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
}

#[allow(clippy::type_complexity)]
pub struct ExtBuilder {
    pub endowed_assets: Vec<(
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
    initial_dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_assets: vec![],
            initial_dex_list: vec![
                (
                    DEX_A_ID,
                    DEXInfo {
                        base_asset_id: XOR,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
                (
                    DEX_B_ID,
                    DEXInfo {
                        base_asset_id: XSTUSD,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
            ],
            endowed_accounts: vec![
                (ALICE, CERES_ASSET_ID, balance!(15000)),
                (BOB, CERES_ASSET_ID, balance!(5)),
                (CHARLES, CERES_ASSET_ID, balance!(3000)),
            ],
            initial_permission_owners: vec![
                (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![BOB]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_B_ID)), vec![BOB]),
            ],
            initial_permissions: vec![
                (ALICE, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
                (ALICE, Scope::Limited(hash(&DEX_B_ID)), vec![MANAGE_DEX]),
            ],
        }
    }
}

impl ExtBuilder {
    #[cfg(feature = "runtime-benchmarks")]
    pub fn benchmarking() -> Self {
        Self {
            endowed_assets: vec![
                (
                    CERES_ASSET_ID,
                    ALICE,
                    AssetSymbol(b"CERES".to_vec()),
                    AssetName(b"Ceres".to_vec()),
                    18,
                    0,
                    true,
                    None,
                    None,
                ),
                (
                    XOR,
                    ALICE,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"XOR".to_vec()),
                    18,
                    0,
                    true,
                    None,
                    None,
                ),
                (
                    PSWAP,
                    ALICE,
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"PSWAP".to_vec()),
                    18,
                    0,
                    true,
                    None,
                    None,
                ),
            ],
            ..Default::default()
        }
    }

    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = SystemConfig::default().build_storage::<Runtime>().unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.initial_dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        TokensConfig {
            balances: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
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

        t.into()
    }
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
        CeresLaunchpad::on_initialize(System::block_number());
    }
}
