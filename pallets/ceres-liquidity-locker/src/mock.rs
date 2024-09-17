use codec::Decode;
use common::mock::GetTradingPairRestrictedFlag;
use common::prelude::{Balance, Fixed};
use common::AccountIdOf;
use common::{
    balance, fixed, hash, mock_assets_config, mock_common_config, mock_currencies_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_pallet_timestamp_config,
    mock_permissions_config, mock_technical_config, DEXId, DEXInfo, TBCD, XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{Everything, Hooks};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use hex_literal::hex;
use orml_traits::parameter_type_with_key;
use permissions::{Scope, MANAGE_DEX};
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::{BuildStorage, Perbill, Percent};

use frame_system::pallet_prelude::BlockNumberFor;
use sp_core::crypto::AccountId32;

pub type BlockNumber = u64;
pub type AccountId = AccountId32;
pub type Amount = i128;
pub type AssetId = common::AssetId32<common::PredefinedAssetId>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type Block = frame_system::mocking::MockBlock<Runtime>;
type Moment = u64;

pub const BLOCKS_PER_DAY: BlockNumberFor<Runtime> = 14_440;

pub const ALICE: AccountId = AccountId::new([1; 32]);
pub const BOB: AccountId = AccountId::new([2; 32]);
pub const DEX_A_ID: DEXId = DEXId::PolkaswapXSTUSD;
pub const CERES_ASSET_ID: AssetId = common::AssetId32::from_bytes(hex!(
    "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
));

parameter_types! {
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub GetBaseAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200000000000000000000000000000000000000000000000000000000000000").into());
    pub GetIncentiveAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200050000000000000000000000000000000000000000000000000000000000").into());
    pub GetPswapDistributionAccountId: AccountId = AccountId::new([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId::new([8; 32]);
    pub GetFee: Fixed = fixed!(0.003);
    pub GetXykMaxIssuanceRatio: Fixed = fixed!(1.5);
    pub const CeresAssetId: AssetId = CERES_ASSET_ID;
    pub GetXykIrreducibleReservePercent: Percent = Percent::from_percent(1);
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
        0
    };
}

construct_runtime! {
    pub enum Runtime {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
        TradingPair: trading_pair::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Tokens: orml_tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Config<T>, Storage, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
    }
}

mock_pallet_balances_config!(Runtime);
mock_currencies_config!(Runtime);
mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);
mock_frame_system_config!(Runtime);
mock_common_config!(Runtime);
mock_assets_config!(Runtime);
mock_permissions_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);

impl dex_manager::Config for Runtime {}

impl trading_pair::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl orml_tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type CurrencyHooks = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type DustRemovalWhitelist = Everything;
}

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = TBCD;
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
    type GetFee = GetFee;
    type GetMaxIssuanceRatio = GetXykMaxIssuanceRatio;
    type OnPoolCreated = PswapDistribution;
    type OnPoolReservesChanged = ();
    type XSTMarketInfo = ();
    type GetTradingPairRestrictedFlag = GetTradingPairRestrictedFlag;
    type GetChameleonPools = common::mock::GetChameleonPools;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    type AssetRegulator = ();
    type IrreducibleReserve = GetXykIrreducibleReservePercent;
    type PoolAdjustPeriod = sp_runtime::traits::ConstU64<1>;
    type WeightInfo = ();
}

impl pswap_distribution::Config for Runtime {
    const PSWAP_BURN_PERCENT: Percent = Percent::from_percent(3);
    type RuntimeEvent = RuntimeEvent;
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
    type GetChameleonPools = common::mock::GetChameleonPools;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl ceres_liquidity_locker::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self> = BLOCKS_PER_DAY;
    type RuntimeEvent = RuntimeEvent;
    type XYKPool = PoolXYK;
    type DemeterFarmingPlatform = DemeterFarmingPlatform;
    type CeresAssetId = CeresAssetId;
    type WeightInfo = ();
}

#[allow(non_snake_case)]
pub fn AUTHORITY<T: frame_system::Config>() -> T::AccountId {
    let bytes = hex!("34a5b78f5fbcdc92a28767d63b579690a4b2f6a179931b3ecc87f09fc9366d47");
    AccountIdOf::<T>::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

pub struct ExtBuilder {
    initial_dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            initial_dex_list: vec![(
                DEX_A_ID,
                DEXInfo {
                    base_asset_id: XOR.into(),
                    synthetic_base_asset_id: XST.into(),
                    is_public: true,
                },
            )],
            endowed_accounts: vec![
                (ALICE, CERES_ASSET_ID.into(), balance!(2000)),
                (BOB, CERES_ASSET_ID.into(), balance!(1000)),
            ],
            initial_permission_owners: vec![(
                MANAGE_DEX,
                Scope::Limited(hash(&DEX_A_ID)),
                vec![BOB],
            )],
            initial_permissions: vec![(BOB, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX])],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Runtime>::default()
            .build_storage()
            .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.initial_dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        orml_tokens::GenesisConfig::<Runtime> {
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

        t.into()
    }
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
        CeresLiquidityLocker::on_initialize(System::block_number());
    }
}
