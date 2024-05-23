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

use crate::{self as vested_rewards, Config};
use common::mock::{ExistentialDeposits, GetTradingPairRestrictedFlag};
use common::prelude::{Balance, DEXInfo};
use common::prelude::{LiquiditySourceType, QuoteAmount, SwapAmount, SwapOutcome};
use common::{
    balance, fixed, hash, AssetId32, AssetName, AssetSymbol, BalancePrecision, ContentSource,
    Description, Fixed, LiquidityProxyTrait, LiquiditySourceFilter, DEFAULT_BALANCE_PRECISION, DOT,
    KSM, PSWAP, TBCD, VAL, XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{Everything, GenesisBuild};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup, Zero};
use sp_runtime::{DispatchError, Perbill, Percent};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
        VestedRewards: vested_rewards::{Pallet, Call, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Storage, Event<T>},
        PoolXyk: pool_xyk::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Storage, Event<T>},
        MBCPool: multicollateral_bonding_curve_pool::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
    }
}

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type Amount = i128;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;

pub fn alice() -> AccountId {
    AccountId32::from([1u8; 32])
}
pub fn bob() -> AccountId {
    AccountId32::from([2u8; 32])
}
pub fn eve() -> AccountId {
    AccountId32::from([3u8; 32])
}
pub fn initial_assets_owner() -> AccountId {
    AccountId32::from([4u8; 32])
}
pub const DEX_ID: DEXId = 0;
type AssetId = AssetId32<common::PredefinedAssetId>;

pub struct MockLiquidityProxy;

impl LiquidityProxyTrait<DEXId, AccountId, AssetId> for MockLiquidityProxy {
    fn quote(
        _dex_id: DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: QuoteAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        match _amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => Ok(SwapOutcome {
                amount: desired_amount_in * 2,
                fee: Default::default(),
            }),
            _ => unimplemented!(),
        }
    }

    fn exchange(
        _dex_id: DEXId,
        _sender: &AccountId,
        _receiver: &AccountId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: SwapAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        unimplemented!()
    }
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([151; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::from([152; 32]);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::from([153; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::from([154; 32]);
    pub GetFarmingRewardsAccountId: AccountId = AccountId32::from([155; 32]);
    pub GetCrowdloanRewardsAccountId: AccountId = AccountId32::from([156; 32]);
    pub GetXykFee: Fixed = fixed!(0.003);
    pub const MinimumPeriod: u64 = 5;
    pub const CrowdloanVestingPeriod: u64 = 14400;
    pub GetXykIrreducibleReservePercent: Percent = Percent::from_percent(1);
    pub GetTbcIrreducibleReservePercent: Percent = Percent::from_percent(1);
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
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = PalletInfo;
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<65536>;
}

impl Config for Runtime {
    const BLOCKS_PER_DAY: BlockNumber = 14400;
    type RuntimeEvent = RuntimeEvent;
    type GetBondingCurveRewardsAccountId = GetBondingCurveRewardsAccountId;
    type GetMarketMakerRewardsAccountId = GetMarketMakerRewardsAccountId;
    type GetFarmingRewardsAccountId = GetFarmingRewardsAccountId;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl tokens::Config for Runtime {
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
    pub const GetBaseAssetId: AssetId = XOR;
}

impl currencies::Config for Runtime {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

type DEXId = u32;

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
    type AssetManager = assets::Pallet<Runtime>;
    type MultiCurrency = currencies::Pallet<Runtime>;
}

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = TBCD;
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![VAL, PSWAP];
    pub const GetBuyBackPercentage: u8 = 10;
    pub const GetBuyBackAccountId: AccountId = AccountId::new(hex!(
            "0000000000000000000000000000000000000000000000000000000000000023"
    ));
    pub const GetBuyBackDexId: DEXId = 0;
    pub GetTBCBuyBackTBCDPercent: Fixed = fixed!(0.025);
}

impl assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<DEXId, common::LiquiditySourceType, [u8; 32]>;
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

impl technical::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl pswap_distribution::Config for Runtime {
    const PSWAP_BURN_PERCENT: Percent = Percent::from_percent(3);
    type RuntimeEvent = RuntimeEvent;
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type GetTBCDAssetId = GetBuyBackAssetId;
    type LiquidityProxy = MockLiquidityProxy;
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetBurnUpdateFrequency = GetBurnUpdateFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = ();
    type OnPswapBurnedAggregator = ();
    type WeightInfo = ();
    type PoolXykPallet = pool_xyk::Pallet<Runtime>;
    type GetParliamentAccountId = GetParliamentAccountId;
    type BuyBackHandler = ();
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
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
    const MIN_XOR: Balance = balance!(0.007);
    type RuntimeEvent = RuntimeEvent;
    type PairSwapAction = pool_xyk::PairSwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type TradingPairSourceManager = ();
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = ();
    type EnabledSourcesManager = ();
    type GetFee = GetXykFee;
    type OnPoolCreated = pswap_distribution::Pallet<Runtime>;
    type OnPoolReservesChanged = ();
    type XSTMarketInfo = ();
    type GetTradingPairRestrictedFlag = GetTradingPairRestrictedFlag;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    type IrreducibleReserve = GetXykIrreducibleReservePercent;
    type WeightInfo = ();
}
impl multicollateral_bonding_curve_pool::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = MockLiquidityProxy;
    type EnsureTradingPairExists = ();
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type VestedRewardsPallet = VestedRewards;
    type TradingPairSourceManager = ();
    type PriceToolsPallet = ();
    type BuyBackHandler = ();
    type BuyBackTBCDPercent = GetTBCBuyBackTBCDPercent;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    type IrreducibleReserve = GetTbcIrreducibleReservePercent;
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
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
}

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl dex_manager::Config for Runtime {}

impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl ceres_liquidity_locker::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self> = 14_440;
    type RuntimeEvent = RuntimeEvent;
    type XYKPool = PoolXyk;
    type DemeterFarmingPlatform = DemeterFarmingPlatform;
    type CeresAssetId = ();
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
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_assets: vec![
                (
                    XOR,
                    initial_assets_owner(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    DOT,
                    initial_assets_owner(),
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"Polkadot".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    KSM,
                    initial_assets_owner(),
                    AssetSymbol(b"KSM".to_vec()),
                    AssetName(b"Kusama".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    PSWAP,
                    initial_assets_owner(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"Polkaswap".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
            ],
            endowed_accounts: vec![],
            dex_list: vec![(
                DEX_ID,
                DEXInfo {
                    base_asset_id: XOR,
                    synthetic_base_asset_id: XST,
                    is_public: true,
                },
            )],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_ID)), vec![alice()]),
            ],
            initial_permissions: vec![
                (alice(), Scope::Unlimited, vec![INIT_DEX]),
                (alice(), Scope::Limited(hash(&DEX_ID)), vec![MANAGE_DEX]),
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
                (alice(), 0),
                (bob(), 0),
                (eve(), 0),
                (initial_assets_owner(), 0),
            ],
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

        tokens::GenesisConfig::<Runtime> {
            balances: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
