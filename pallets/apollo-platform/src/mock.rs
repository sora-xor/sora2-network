use {
    crate as apollo_platform,
    common::{
        balance, fixed, hash,
        mock::{ExistentialDeposits, GetTradingPairRestrictedFlag},
        prelude::Balance,
        AssetId32, AssetName, AssetSymbol, BalancePrecision, ContentSource,
        DEXId::Polkaswap,
        DEXInfo, Description, Fixed, FromGenericPair, PriceToolsPallet, APOLLO_ASSET_ID, DAI, DOT,
        KSM, PSWAP, TBCD, VAL, XOR, XST,
    },
    currencies::BasicCurrencyAdapter,
    frame_support::{
        construct_runtime,
        pallet_prelude::Weight,
        parameter_types,
        traits::{Everything, GenesisBuild, Hooks},
    },
    frame_system::{self, pallet_prelude::BlockNumberFor, EnsureRoot},
    permissions::{Scope, MANAGE_DEX},
    sp_core::{ConstU32, H256},
    sp_runtime::{
        testing::Header,
        traits::{BlakeTwo256, IdentityLookup, Zero},
        AccountId32, Perbill, Percent,
    },
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;
type Moment = u64;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type Amount = i128;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
type DEXId = common::DEXId;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;

pub fn alice() -> AccountId32 {
    AccountId32::from([1; 32])
}

pub fn bob() -> AccountId32 {
    AccountId32::from([2; 32])
}

pub fn charles() -> AccountId32 {
    AccountId32::from([3; 32])
}
construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
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
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
        PriceTools: price_tools::{Pallet, Storage, Event<T>},
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
    pub GetParliamentAccountId: AccountId = AccountId32::from([100; 32]);
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([101; 32]);
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
    pub GetBuyBackAccountId: AccountId = AccountId32::from([23; 32]);
    pub const GetBuyBackDexId: DEXId = DEXId::Polkaswap;
    pub GetLiquidityProxyTechAccountId: TechAccountId = {

        TechAccountId::from_generic_pair(
            liquidity_proxy::TECH_ACCOUNT_PREFIX.to_vec(),
            liquidity_proxy::TECH_ACCOUNT_MAIN.to_vec(),
        )
    };
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
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<common::DEXId, common::LiquiditySourceType, [u8; 32]>;
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

impl ceres_liquidity_locker::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self> = 14_440;
    type RuntimeEvent = RuntimeEvent;
    type XYKPool = PoolXYK;
    type DemeterFarmingPlatform = DemeterFarmingPlatform;
    type CeresAssetId = ();
    type WeightInfo = ();
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

impl pallet_timestamp::Config for Runtime {
    type Moment = Moment;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl dex_manager::Config for Runtime {}

impl technical::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
}

impl common::Config for Runtime {
    type DEXId = common::DEXId;
    type LstId = common::LiquiditySourceType;
}

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl price_tools::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = LiquidityProxy;
    type WeightInfo = price_tools::weights::SubstrateWeight<Runtime>;
}

pub struct MockPriceTools;

impl PriceToolsPallet<AssetId> for MockPriceTools {
    fn get_average_price(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        price_variant: common::prelude::PriceVariant,
    ) -> Result<Balance, sp_runtime::DispatchError> {
        todo!()
    }

    fn register_asset(asset_id: &AssetId) -> frame_support::pallet_prelude::DispatchResult {
        todo!()
    }

    fn spot_price(asset_id: &AssetId) -> Result<Balance, sp_runtime::DispatchError> {
        todo!()
    }
}

impl demeter_farming_platform::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DemeterAssetId = ();
    const BLOCKS_PER_HOUR_AND_A_HALF: BlockNumberFor<Self> = 900;
    type WeightInfo = ();
}

impl crate::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PriceTools = MockPriceTools;
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
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    initial_dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            initial_dex_list: vec![(
                Polkaswap,
                DEXInfo {
                    base_asset_id: XOR,
                    synthetic_base_asset_id: XST,
                    is_public: true,
                },
            )],
            initial_permissions: vec![(
                charles(),
                Scope::Limited(hash(&Polkaswap)),
                vec![MANAGE_DEX],
            )],
            initial_permission_owners: vec![(
                MANAGE_DEX,
                Scope::Limited(hash(&Polkaswap)),
                vec![charles()],
            )],
            endowed_assets: vec![
                (
                    APOLLO_ASSET_ID,
                    alice(),
                    AssetSymbol(b"APOLLO".to_vec()),
                    AssetName(b"Apollo".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    XOR,
                    alice(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"Sora".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    DAI,
                    alice(),
                    AssetSymbol(b"DAI".to_vec()),
                    AssetName(b"Dai".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    DOT,
                    alice(),
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"Polkadot".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    KSM,
                    alice(),
                    AssetSymbol(b"KSM".to_vec()),
                    AssetName(b"Kusama".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
            ],
            endowed_accounts: vec![
                (alice(), APOLLO_ASSET_ID, balance!(300000)),
                (bob(), APOLLO_ASSET_ID, balance!(500)),
                (charles(), APOLLO_ASSET_ID, balance!(300000)),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = SystemConfig::default().build_storage::<Runtime>().unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (alice(), balance!(300000)),
                (bob(), balance!(500)),
                (charles(), balance!(300000)),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

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
    }
}
