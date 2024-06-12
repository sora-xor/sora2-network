use {
    crate as apollo_platform,
    common::{
        balance, fixed, hash,
        mock::{ExistentialDeposits, GetTradingPairRestrictedFlag},
        mock_currencies_config, mock_frame_system_config, mock_pallet_balances_config,
        mock_technical_config,
        prelude::{Balance, SwapOutcome},
        AssetId32, AssetName, AssetSymbol, BalancePrecision, ContentSource,
        DEXId::Polkaswap,
        DEXInfo, Description, Fixed, FromGenericPair, LiquidityProxyTrait, PriceToolsProvider,
        PriceVariant, APOLLO_ASSET_ID, CERES_ASSET_ID, DAI, DOT, KSM, PSWAP, TBCD, VAL, XOR, XST,
    },
    currencies::BasicCurrencyAdapter,
    frame_support::{
        construct_runtime,
        pallet_prelude::Weight,
        parameter_types,
        traits::{ConstU64, Everything, GenesisBuild, Hooks},
    },
    frame_system::{
        self, offchain::SendTransactionTypes, pallet_prelude::BlockNumberFor, EnsureRoot, RawOrigin,
    },
    permissions::{Scope, MANAGE_DEX},
    sp_core::{ConstU32, H256},
    sp_runtime::{
        testing::{Header, TestXt},
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

pub fn exchange_account() -> AccountId32 {
    AccountId32::from([4; 32])
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
        ApolloPlatform: apollo_platform::{Pallet, Call, Storage, Event<T>, ValidateUnsigned},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
        PriceTools: price_tools::{Pallet, Storage, Event<T>},
    }
}

pub type MockExtrinsic = TestXt<RuntimeCall, ()>;

mock_pallet_balances_config!(Runtime);
mock_currencies_config!(Runtime);

impl<LocalCall> SendTransactionTypes<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    type Extrinsic = MockExtrinsic;
    type OverarchingCall = RuntimeCall;
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub GetXykFee: Fixed = fixed!(0.003);
    pub GetIncentiveAssetId: AssetId = common::PSWAP;
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::from([100; 32]);
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([101; 32]);
    pub const MinimumPeriod: u64 = 5;
}

mock_frame_system_config!(Runtime);

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
    pub GetXykIrreducibleReservePercent: Percent = Percent::from_percent(1);
    pub GetTbcIrreducibleReservePercent: Percent = Percent::from_percent(1);
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
    type AssetRegulator = permissions::Pallet<Runtime>;
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
    type LockedLiquiditySourcesManager = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type GetADARAccountId = GetADARAccountId;
    type ADARCommissionRatioUpdateOrigin = EnsureRoot<AccountId>;
    type MaxAdditionalDataLengthXorlessTransfer = ConstU32<128>;
    type MaxAdditionalDataLengthSwapTransferBatch = ConstU32<2000>;
    type GetChameleonPool = common::mock::GetChameleonPool;
    type GetChameleonPoolBaseAssetId = common::mock::GetChameleonPoolBaseAssetId;
    type AssetInfoProvider = assets::Pallet<Runtime>;
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
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
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
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl trading_pair::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl multicollateral_bonding_curve_pool::Config for Runtime {
    const RETRY_DISTRIBUTION_FREQUENCY: BlockNumber = 1000;
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = ();
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type PriceToolsPallet = ();
    type VestedRewardsPallet = VestedRewards;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type BuyBackHandler = ();
    type BuyBackTBCDPercent = GetTBCBuyBackTBCDPercent;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    type IrreducibleReserve = GetTbcIrreducibleReservePercent;
    type WeightInfo = ();
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
    type XSTMarketInfo = ();
    type GetTradingPairRestrictedFlag = GetTradingPairRestrictedFlag;
    type GetChameleonPool = common::mock::GetChameleonPool;
    type GetChameleonPoolBaseAssetId = common::mock::GetChameleonPoolBaseAssetId;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    type IrreducibleReserve = GetXykIrreducibleReservePercent;
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
    type GetChameleonPoolBaseAssetId = common::mock::GetChameleonPoolBaseAssetId;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl pallet_timestamp::Config for Runtime {
    type Moment = Moment;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl dex_manager::Config for Runtime {}

mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);

impl common::Config for Runtime {
    type DEXId = common::DEXId;
    type LstId = common::LiquiditySourceType;
    type AssetManager = assets::Pallet<Runtime>;
    type MultiCurrency = currencies::Pallet<Runtime>;
}

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl price_tools::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = LiquidityProxy;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type WeightInfo = price_tools::weights::SubstrateWeight<Runtime>;
}

pub struct MockPriceTools;

impl PriceToolsProvider<AssetId> for MockPriceTools {
    fn is_asset_registered(_asset_id: &AssetId) -> bool {
        unimplemented!()
    }

    fn get_average_price(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        price_variant: common::prelude::PriceVariant,
    ) -> Result<Balance, sp_runtime::DispatchError> {
        match price_variant {
            PriceVariant::Buy => match (input_asset_id, output_asset_id) {
                (&DAI, &DAI) => Ok(balance!(0.1)), // needed for liquidation test
                (&XOR, &DAI) => Ok(balance!(1)),
                (&DAI, &XOR) => Ok(balance!(1)),
                (&XOR, &DOT) => Ok(balance!(1)),
                (&DOT, &XOR) => Ok(balance!(1)),
                (&XOR, &KSM) => Ok(balance!(1)),
                (&KSM, &XOR) => Ok(balance!(1)),
                (&DAI, &DOT) => Ok(balance!(1)),
                (&DOT, &DAI) => Ok(balance!(1)),
                (&DAI, &KSM) => Ok(balance!(1)),
                (&KSM, &DAI) => Ok(balance!(1)),
                (&DOT, &KSM) => Ok(balance!(1)),
                (&KSM, &DOT) => Ok(balance!(1)),
                _ => Ok(balance!(0)),
            },
            PriceVariant::Sell => match (input_asset_id, output_asset_id) {
                (&DAI, &DAI) => Ok(balance!(0.1)), // needed for liquidation test
                (&XOR, &DAI) => Ok(balance!(1)),
                (&DAI, &XOR) => Ok(balance!(1)),
                (&XOR, &DOT) => Ok(balance!(1)),
                (&DOT, &XOR) => Ok(balance!(1)),
                (&XOR, &KSM) => Ok(balance!(1)),
                (&KSM, &XOR) => Ok(balance!(1)),
                (&DAI, &DOT) => Ok(balance!(1)),
                (&DOT, &DAI) => Ok(balance!(1)),
                (&DAI, &KSM) => Ok(balance!(1)),
                (&KSM, &DAI) => Ok(balance!(1)),
                (&DOT, &KSM) => Ok(balance!(1)),
                (&KSM, &DOT) => Ok(balance!(1)),
                _ => Ok(balance!(0)),
            },
        }
    }

    fn register_asset(_asset_id: &AssetId) -> frame_support::pallet_prelude::DispatchResult {
        todo!()
    }
}

impl demeter_farming_platform::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DemeterAssetId = ();
    const BLOCKS_PER_HOUR_AND_A_HALF: BlockNumberFor<Self> = 900;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

pub struct MockLiquidityProxy;

impl LiquidityProxyTrait<DEXId, AccountId, AssetId> for MockLiquidityProxy {
    fn quote(
        _dex_id: DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        amount: common::prelude::QuoteAmount<Balance>,
        _filter: common::LiquiditySourceFilter<DEXId, common::prelude::LiquiditySourceType>,
        _deduce_fee: bool,
    ) -> Result<common::prelude::SwapOutcome<Balance, AssetId>, sp_runtime::DispatchError> {
        Ok(SwapOutcome::new(amount.amount(), Default::default()))
    }

    fn exchange(
        _dex_id: DEXId,
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: common::prelude::SwapAmount<Balance>,
        _filter: common::LiquiditySourceFilter<DEXId, common::prelude::LiquiditySourceType>,
    ) -> Result<common::prelude::SwapOutcome<Balance, AssetId>, sp_runtime::DispatchError> {
        // Transfer to exchange account (input asset)
        let _ = Assets::transfer(
            RawOrigin::Signed(sender.clone()).into(),
            *input_asset_id,
            exchange_account(),
            amount.amount(),
        );

        if input_asset_id == &DAI && output_asset_id != &APOLLO_ASSET_ID {
            let _ = Assets::transfer(
                RawOrigin::Signed(exchange_account()).into(),
                *output_asset_id,
                receiver.clone(),
                amount.amount() * balance!(0.1) / balance!(1),
            );
        } else {
            // Transfer from exchange account (output asset)
            let _ = Assets::transfer(
                RawOrigin::Signed(exchange_account()).into(),
                *output_asset_id,
                receiver.clone(),
                amount.amount(),
            );
        }

        Ok(SwapOutcome::new(amount.amount(), Default::default()))
    }
}

impl crate::Config for Runtime {
    const BLOCKS_PER_FIFTEEN_MINUTES: BlockNumberFor<Self> = 150;
    type RuntimeEvent = RuntimeEvent;
    type PriceTools = MockPriceTools;
    type LiquidityProxyPallet = MockLiquidityProxy;
    type UnsignedPriority = ConstU64<100>;
    type UnsignedLongevity = ConstU64<100>;
    type WeightInfo = ();
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
                (
                    CERES_ASSET_ID,
                    alice(),
                    AssetSymbol(b"CERES".to_vec()),
                    AssetName(b"Ceres".to_vec()),
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
        common::test_utils::init_logger();
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
        ApolloPlatform::on_initialize(System::block_number());
        ApolloPlatform::offchain_worker(System::block_number());
    }
}
