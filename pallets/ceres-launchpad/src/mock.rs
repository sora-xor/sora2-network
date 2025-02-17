use crate::{self as ceres_launchpad, Config};
use common::mock::ExistentialDeposits;
use common::prelude::Balance;
pub use common::TechAssetId as Tas;
pub use common::TechPurpose::*;
#[cfg(feature = "runtime-benchmarks")]
use common::PSWAP;
use common::{
    balance, hash, mock_assets_config, mock_ceres_liquidity_locker_config,
    mock_ceres_token_locker_config, mock_common_config, mock_currencies_config,
    mock_demeter_farming_platform_config, mock_dex_manager_config, mock_frame_system_config,
    mock_multicollateral_bonding_curve_pool_config, mock_pallet_balances_config,
    mock_pallet_timestamp_config, mock_permissions_config, mock_pool_xyk_config,
    mock_pswap_distribution_config, mock_technical_config, mock_tokens_config,
    mock_trading_pair_config, mock_vested_rewards_config, AssetName, AssetSymbol, BalancePrecision,
    ContentSource, DEXId, DEXInfo, Description, CERES_ASSET_ID, VXOR, XOR, XST, XSTUSD,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{GenesisBuild, Hooks};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system::pallet_prelude::BlockNumberFor;
use permissions::{Scope, MANAGE_DEX};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::{MultiSignature, Perbill};

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

mock_assets_config!(Runtime);
mock_ceres_liquidity_locker_config!(Runtime, PoolXYK, CeresAssetId);
mock_ceres_token_locker_config!(Runtime);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_demeter_farming_platform_config!(Runtime);
mock_dex_manager_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_multicollateral_bonding_curve_pool_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);
mock_permissions_config!(Runtime);
mock_pool_xyk_config!(Runtime);
mock_pswap_distribution_config!(Runtime, PoolXYK);
mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);
mock_tokens_config!(Runtime);
mock_trading_pair_config!(Runtime);
mock_vested_rewards_config!(Runtime);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::new([100u8; 32]);
    pub GetPswapDistributionAccountId: AccountId = AccountId32::new([101u8; 32]);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::new([102u8; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::new([103u8; 32]);
    pub GetFarmingRewardsAccountId: AccountId = AccountId32::new([104u8; 32]);
    pub GetCrowdloanRewardsAccountId: AccountId = AccountId32::new([105u8; 32]);
}

impl Config for Runtime {
    const MILLISECONDS_PER_DAY: Self::Moment = 86_400_000;
    type RuntimeEvent = RuntimeEvent;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = VXOR;
}

parameter_types! {
    pub const CeresAssetId: AssetId = CERES_ASSET_ID;
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
            regulated_assets: Default::default(),
            sbt_assets: Default::default(),
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
