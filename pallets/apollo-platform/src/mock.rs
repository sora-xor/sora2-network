use {
    crate::{self as apollo_platform},
    common::prelude::Balance,
    common::mock::{ExistentialDeposits, GetTradingPairRestrictedFlag},
    frame_system,
    frame_system::{EnsureRoot},
    frame_support::{construct_runtime, parameter_types},
    frame_support::traits::{Everything, GenesisBuild},
    common::{
        balance, fixed, AssetId32, AssetName, AssetSymbol, BalancePrecision, ContentSource,
        Description, Fixed, APOLLO_ASSET_ID, TBCD, PSWAP, VAL,
    },
    frame_support::pallet_prelude::Weight,
    sp_runtime::{Perbill, Percent, AccountId32},
    sp_core::{ConstU32, H256},
    sp_runtime::traits::{BlakeTwo256, Zero},
    sp_runtime::traits::IdentityLookup,
    sp_runtime::testing::Header,
    currencies::BasicCurrencyAdapter,
    frame_system::pallet_prelude::BlockNumberFor,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type Amount = i128;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
type DEXId = common::DEXId;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLES: AccountId = 3;
pub const BUY_BACK_ACCOUNT: AccountId = 23;

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        LiquidityProxy: liquidity_proxy::{Pallet, Call, Event<T>},
        DexApi: dex_api::{Pallet, Call, Config, Storage, Event<T>},
        VestedRewards: vested_rewards::{Pallet, Call, Storage, Event<T>},
        TradingPair: trading_pair::{Pallet, Call, Config<T>, Storage, Event<T>},
        MBCPool: multicollateral_bonding_curve_pool::{Pallet, Call, Config<T>, Storage, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Config<T>, Storage, Event<T>},
        ApolloPlatform: apollo_platform::{Pallet, Call, Storage, Event<T>},
    }
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub GetXykFee: Fixed = fixed!(0.003);
    pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = 100;
    pub GetPswapDistributionAccountId: AccountId = 101;
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

parameter_types! {
    pub const GetNumSamples: usize = 40;
    pub const GetBaseAssetId: AssetId = APOLLO_ASSET_ID;
    pub const GetBuyBackAssetId: AssetId = TBCD;
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![VAL, PSWAP];
    pub const GetBuyBackPercentage: u8 = 10;
    pub const GetBuyBackAccountId: AccountId = BUY_BACK_ACCOUNT;
    pub const GetBuyBackDexId: DEXId = DEXId::Polkaswap;
    pub GetLiquidityProxyAccountId: AccountId = {
        let tech_account_id = GetLiquidityProxyTechAccountId::get();

        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.")
    };
    pub GetADARAccountId: AccountId = AccountId32::from([14; 32]);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::from([9; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::from([10; 32]);
    pub GetFarmingRewardsAccountId: AccountId = AccountId32::from([12; 32]);
    pub GetTBCBuyBackTBCDPercent: Fixed = fixed!(0.025);
}

impl assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ExtraAccountId = AccountId;
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<common::DEXId, common::LiquiditySourceType, AccountId>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type GetBuyBackAssetId = GetBuyBackAssetId;
    type GetBuyBackSupplyAssets = GetBuyBackSupplyAssets;
    type GetBuyBackPercentage = GetBuyBackPercentage;
    type GetBuyBackAccountId = GetBuyBackAccountId;
    type GetBuyBackDexId = GetBuyBackDexId;
    type BuyBackLiquidityProxy = ();
    type Currency = currencies::Pallet<Runtime>;
    type GetTotalBalance = ();
    type WeightInfo = ();
}

impl currencies::Config for Runtime {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
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

impl liquidity_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityRegistry = dex_api::Pallet<Runtime>;
    type GetNumSamples = GetNumSamples;
    type GetTechnicalAccountId = GetLiquidityProxyAccountId;
    type WeightInfo = ();
    type PrimaryMarketTBC = ();
    type PrimaryMarketXST = ();
    type SecondaryMarket = ();
    type VestedRewardsPallet = vested_rewards::Pallet<Runtime>;
    type GetADARAccountId = GetADARAccountId;
    type ADARCommissionRatioUpdateOrigin = EnsureRoot<AccountId>;
    type MaxAdditionalDataLength = ConstU32<128>;
}

impl dex_api::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MockLiquiditySource = ();
    type MockLiquiditySource2 = ();
    type MockLiquiditySource3 = ();
    type MockLiquiditySource4 = ();
    type MulticollateralBondingCurvePool = ();
    type XYKPool = pool_xyk::Pallet<Runtime>;
    type XSTPool = ();

    #[cfg(feature = "ready-to-test")] // order-book
    type OrderBook = ();

    type WeightInfo = ();
}

impl vested_rewards::Config for Runtime {
    const BLOCKS_PER_DAY: BlockNumberFor<Self> = 14400;
    type RuntimeEvent = RuntimeEvent;
    type GetMarketMakerRewardsAccountId = GetMarketMakerRewardsAccountId;
    type GetBondingCurveRewardsAccountId = GetBondingCurveRewardsAccountId;
    type GetFarmingRewardsAccountId = GetFarmingRewardsAccountId;
    type WeightInfo = ();
}

impl trading_pair::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type WeightInfo = ();
}

impl multicollateral_bonding_curve_pool::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = ();
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type PriceToolsPallet = ();
    type VestedRewardsPallet = VestedRewards;
    type BuyBackHandler = ();
    type BuyBackTBCDPercent = GetTBCBuyBackTBCDPercent;
    type WeightInfo = ();
}

impl pool_xyk::Config for Runtime {
    const MIN_XOR: Balance = balance!(0.0007);
    type RuntimeEvent = RuntimeEvent;
    type PairSwapAction = pool_xyk::PairSwapAction<AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type GetFee = GetXykFee;
    type OnPoolCreated = PswapDistribution;
    type OnPoolReservesChanged = ();
    type WeightInfo = ();
    type XSTMarketInfo = ();
    type GetTradingPairRestrictedFlag = GetTradingPairRestrictedFlag;
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
}

impl common::Config for Runtime {
    type DEXId = common::DEXId;
    type LstId = common::LiquiditySourceType;
}

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl crate::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
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
                APOLLO_ASSET_ID,
                ALICE,
                AssetSymbol(b"APOLLO".to_vec()),
                AssetName(b"Apollo".to_vec()),
                18,
                Balance::zero(),
                true,
                None,
                None,
            )],
            endowed_accounts: vec![
                (ALICE, APOLLO_ASSET_ID, balance!(300000)),
                (BOB, APOLLO_ASSET_ID, balance!(500)),
                (CHARLES, APOLLO_ASSET_ID, balance!(300000)),
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

        t.into()
    }
}