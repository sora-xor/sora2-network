// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::{self as farming, Config};
use common::mock::{ExistentialDeposits, GetTradingPairRestrictedFlag};
use common::prelude::Balance;
use common::{
    balance, fixed, hash, mock_assets_config, mock_common_config, mock_currencies_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_technical_config,
    mock_tokens_config, mock_vested_rewards_config, AssetName, AssetSymbol, DEXId, DEXInfo, Fixed,
    DEFAULT_BALANCE_PRECISION, DOT, PSWAP, VAL, VXOR, XOR, XST, XSTUSD,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{Everything, GenesisBuild, OnFinalize, OnInitialize, PrivilegeCmp};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::EnsureRoot;
use permissions::*;
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::{Perbill, Percent};
use sp_std::cmp::Ordering;
use sp_std::marker::PhantomData;

pub use common::mock::*;
pub use common::TechAssetId as Tas;
pub use common::TechPurpose::*;

pub type BlockNumber = u64;
pub type AccountId = AccountId32;
pub type Amount = i128;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type AssetId = common::AssetId32<common::PredefinedAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub const PSWAP_PER_DAY: Balance = balance!(2500000);
pub const REFRESH_FREQUENCY: BlockNumberFor<Runtime> = 1200;
pub const VESTING_COEFF: u32 = 3;
pub const VESTING_FREQUENCY: BlockNumberFor<Runtime> = 3600;
pub const BLOCKS_PER_DAY: BlockNumberFor<Runtime> = 14_440;

#[allow(non_snake_case)]
pub fn ALICE() -> AccountId {
    AccountId32::from([1; 32])
}

#[allow(non_snake_case)]
pub fn BOB() -> AccountId {
    AccountId32::from([2; 32])
}

#[allow(non_snake_case)]
pub fn CHARLIE() -> AccountId {
    AccountId32::from([3; 32])
}

#[allow(non_snake_case)]
pub fn DAVE() -> AccountId {
    AccountId32::from([4; 32])
}

#[allow(non_snake_case)]
pub fn EVE() -> AccountId {
    AccountId32::from([5; 32])
}

#[allow(non_snake_case)]
pub fn FERDIE() -> AccountId {
    AccountId32::from([6; 32])
}

pub const DEX_A_ID: DEXId = DEXId::Polkaswap;
pub const DEX_B_ID: DEXId = DEXId::PolkaswapXSTUSD;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId = common::AssetId32 { code: [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], phantom: PhantomData };
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
    pub GetParliamentAccountId: AccountId = AccountId32::from([8; 32]);
    pub RewardDoublingAssets: Vec<AssetId> = vec![VAL.into(), PSWAP.into(), DOT.into()];
    pub GetXykFee: Fixed = fixed!(0.003);
    pub GetXykMaxIssuanceRatio: Fixed = fixed!(1.5);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::from([12; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::from([13; 32]);
    pub GetFarmingRewardsAccountId: AccountId = AccountId32::from([14; 32]);
    pub GetCrowdloanRewardsAccountId: AccountId = AccountId32::from([15; 32]);
    pub const SchedulerMaxWeight: Weight = Weight::from_parts(1024, 0);
    pub const MinimumPeriod: u64 = 5;
    pub GetXykIrreducibleReservePercent: Percent = Percent::from_percent(1);
    pub GetTbcIrreducibleReservePercent: Percent = Percent::from_percent(1);
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
        TradingPair: trading_pair::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Config<T>, Storage, Event<T>},
        MBCPool: multicollateral_bonding_curve_pool::{Pallet, Call, Storage, Event<T>},
        VestedRewards: vested_rewards::{Pallet, Storage, Event<T>},
        Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},
        Farming: farming::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
    }
}

mock_pallet_balances_config!(Runtime);
mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);
mock_currencies_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_common_config!(Runtime);
mock_tokens_config!(Runtime);
mock_assets_config!(Runtime);
mock_vested_rewards_config!(Runtime);

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

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = VXOR;
    pub GetTBCBuyBackTBCDPercent: Fixed = fixed!(0.025);
}

impl demeter_farming_platform::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DemeterAssetId = ();
    const BLOCKS_PER_HOUR_AND_A_HALF: BlockNumberFor<Self> = 900;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl pool_xyk::Config for Runtime {
    const MIN_XOR: Balance = balance!(0.007);
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
    type GetMaxIssuanceRatio = GetXykMaxIssuanceRatio;
    type OnPoolCreated = (PswapDistribution, Farming);
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
    type GetVXORAssetId = GetBuyBackAssetId;
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

impl multicollateral_bonding_curve_pool::Config for Runtime {
    const RETRY_DISTRIBUTION_FREQUENCY: BlockNumber = 1000;
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = ();
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type PriceToolsPallet = ();
    type VestedRewardsPallet = VestedRewards;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type BuyBackHandler = ();
    type BuyBackTBCDPercent = GetTBCBuyBackTBCDPercent;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    type IrreducibleReserve = GetTbcIrreducibleReservePercent;
    type WeightInfo = ();
}

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;

impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
    fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
        if left == right {
            return Some(Ordering::Equal);
        }

        match (left, right) {
            // Root is greater than anything.
            (OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),
            // For every other origin we don't care, as they are not used for `ScheduleOrigin`.
            _ => None,
        }
    }
}

impl pallet_scheduler::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = SchedulerMaxWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = ();
    type WeightInfo = ();
    type OriginPrivilegeCmp = OriginPrivilegeCmp;
    type Preimages = ();
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
    type CeresAssetId = ();
    type WeightInfo = ();
}

impl Config for Runtime {
    const PSWAP_PER_DAY: Balance = PSWAP_PER_DAY;
    const REFRESH_FREQUENCY: BlockNumberFor<Self> = REFRESH_FREQUENCY;
    const VESTING_COEFF: u32 = VESTING_COEFF;
    const VESTING_FREQUENCY: BlockNumberFor<Self> = VESTING_FREQUENCY;
    const BLOCKS_PER_DAY: BlockNumberFor<Self> = BLOCKS_PER_DAY;
    type RuntimeCall = RuntimeCall;
    type SchedulerOriginCaller = OriginCaller;
    type Scheduler = Scheduler;
    type RewardDoublingAssets = RewardDoublingAssets;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type WeightInfo = ();
    type RuntimeEvent = RuntimeEvent;
}

pub struct ExtBuilder {
    initial_dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        let preset01 = vec![
            INIT_DEX,
            CREATE_FARM,
            LOCK_TO_FARM,
            UNLOCK_FROM_FARM,
            CLAIM_FROM_FARM,
        ];
        Self {
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
            endowed_accounts: [DOT, PSWAP, VAL, XSTUSD]
                .into_iter()
                .flat_map(|asset| {
                    [ALICE(), BOB(), CHARLIE(), DAVE(), EVE(), FERDIE()]
                        .into_iter()
                        .map(move |account| (account, asset, balance!(2000000)))
                })
                .collect(),
            initial_permission_owners: vec![
                (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![BOB()]),
                (CREATE_FARM, Scope::Unlimited, vec![ALICE()]),
                (LOCK_TO_FARM, Scope::Unlimited, vec![ALICE()]),
                (UNLOCK_FROM_FARM, Scope::Unlimited, vec![ALICE()]),
                (CLAIM_FROM_FARM, Scope::Unlimited, vec![ALICE()]),
            ],
            initial_permissions: vec![
                (BOB(), Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
                (ALICE(), Scope::Unlimited, preset01.clone()),
                (BOB(), Scope::Unlimited, preset01.clone()),
                (CHARLIE(), Scope::Unlimited, preset01.clone()),
                (DAVE(), Scope::Unlimited, preset01.clone()),
                (EVE(), Scope::Unlimited, preset01.clone()),
                (FERDIE(), Scope::Unlimited, preset01.clone()),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (ALICE(), balance!(99000)),
                (BOB(), balance!(99000)),
                (CHARLIE(), balance!(99000)),
                (DAVE(), balance!(99000)),
                (EVE(), balance!(99000)),
                (FERDIE(), balance!(99000)),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
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
            endowed_assets: vec![
                (
                    XOR.into(),
                    ALICE(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    0,
                    true,
                    None,
                    None,
                ),
                (
                    DOT.into(),
                    ALICE(),
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"DOT".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    0,
                    true,
                    None,
                    None,
                ),
                (
                    PSWAP.into(),
                    ALICE(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"PSWAP".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    0,
                    true,
                    None,
                    None,
                ),
                (
                    VAL.into(),
                    ALICE(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"VAL".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    0,
                    true,
                    None,
                    None,
                ),
                (
                    XSTUSD.into(),
                    ALICE(),
                    AssetSymbol(b"XSTUSD".to_vec()),
                    AssetName(b"XSTUSD".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    0,
                    true,
                    None,
                    None,
                ),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.initial_dex_list,
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
        Scheduler::on_initialize(System::block_number());
        Farming::on_initialize(System::block_number());
    }
}
