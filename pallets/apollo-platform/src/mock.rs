use {
    crate as apollo_platform,
    common::{
        balance, hash,
        mock::ExistentialDeposits,
        mock_apollo_platform_config, mock_assets_config, mock_ceres_liquidity_locker_config,
        mock_common_config, mock_currencies_config, mock_demeter_farming_platform_config,
        mock_dex_api_config, mock_dex_manager_config, mock_frame_system_config,
        mock_liquidity_proxy_config, mock_multicollateral_bonding_curve_pool_config,
        mock_pallet_balances_config, mock_pallet_timestamp_config, mock_permissions_config,
        mock_pool_xyk_config, mock_price_tools_config, mock_pswap_distribution_config,
        mock_technical_config, mock_tokens_config, mock_trading_pair_config,
        mock_vested_rewards_config,
        prelude::{Balance, SwapOutcome},
        AssetId32, AssetName, AssetSymbol, BalancePrecision, ContentSource,
        DEXId::Polkaswap,
        DEXInfo, Description, FromGenericPair, LiquidityProxyTrait, PriceToolsProvider,
        PriceVariant, APOLLO_ASSET_ID, CERES_ASSET_ID, DAI, DOT, KSM, KUSD, VXOR, XOR, XST,
    },
    currencies::BasicCurrencyAdapter,
    frame_support::{
        construct_runtime,
        pallet_prelude::Weight,
        parameter_types,
        traits::{GenesisBuild, Hooks},
    },
    frame_system::{
        self, offchain::SendTransactionTypes, pallet_prelude::BlockNumberFor, RawOrigin,
    },
    permissions::{Scope, MANAGE_DEX},
    sp_runtime::{testing::TestXt, traits::Zero, AccountId32, Perbill},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

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

mock_apollo_platform_config!(Runtime);
mock_assets_config!(Runtime);
mock_ceres_liquidity_locker_config!(Runtime, PoolXYK);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_demeter_farming_platform_config!(Runtime);
mock_dex_api_config!(Runtime, multicollateral_bonding_curve_pool::Pallet<Runtime>);
mock_dex_manager_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_liquidity_proxy_config!(Runtime);
mock_multicollateral_bonding_curve_pool_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);
mock_permissions_config!(Runtime);
mock_pool_xyk_config!(Runtime);
mock_price_tools_config!(Runtime);
mock_pswap_distribution_config!(Runtime, PoolXYK);
mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);
mock_tokens_config!(Runtime);
mock_trading_pair_config!(Runtime);
mock_vested_rewards_config!(Runtime);

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
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::from([100; 32]);
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([101; 32]);
}

parameter_types! {
    pub const GetNumSamples: usize = 40;
    pub const GetBaseAssetId: AssetId = APOLLO_ASSET_ID;
    pub const GetBuyBackAssetId: AssetId = VXOR;
    pub GetADARAccountId: AccountId = AccountId32::from([14; 32]);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::from([9; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::from([10; 32]);
    pub GetFarmingRewardsAccountId: AccountId = AccountId32::from([12; 32]);
}

pub struct MockPriceTools;

impl PriceToolsProvider<AssetId> for MockPriceTools {
    fn is_asset_registered(_asset_id: &AssetId) -> bool {
        false
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
                (&KUSD, &DOT) => Ok(balance!(1)),
                (&DOT, &KUSD) => Ok(balance!(1)),
                (&XOR, &KUSD) => Ok(balance!(1)),
                (&KUSD, &XOR) => Ok(balance!(1)),
                (&DAI, &KUSD) => Ok(balance!(1)),
                (&KUSD, &DAI) => Ok(balance!(1)),
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
                (&KUSD, &DOT) => Ok(balance!(1)),
                (&DOT, &KUSD) => Ok(balance!(1)),
                (&XOR, &KUSD) => Ok(balance!(1)),
                (&KUSD, &XOR) => Ok(balance!(1)),
                (&DAI, &KUSD) => Ok(balance!(1)),
                (&KUSD, &DAI) => Ok(balance!(1)),
                _ => Ok(balance!(0)),
            },
        }
    }

    fn register_asset(_asset_id: &AssetId) -> frame_support::pallet_prelude::DispatchResult {
        Ok(())
    }
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
                (
                    KUSD,
                    alice(),
                    AssetSymbol(b"KUSD".to_vec()),
                    AssetName(b"Kensetsu Stable Dollar".to_vec()),
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
