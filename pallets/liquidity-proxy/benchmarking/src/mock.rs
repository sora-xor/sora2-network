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

#![cfg(test)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use crate::{Config, *};
use common::mock::{ExistentialDeposits, GetTradingPairRestrictedFlag};
use common::prelude::{Balance, QuoteAmount};
use common::{
    balance, fixed, fixed_from_basis_points, hash, Amount, AssetId32, AssetName, AssetSymbol,
    BalancePrecision, ContentSource, DEXInfo, Description, Fixed, FromGenericPair,
    LiquidityProxyTrait, LiquiditySourceFilter, LiquiditySourceType, PriceToolsProvider,
    PriceVariant, TechPurpose, DEFAULT_BALANCE_PRECISION, DOT, PSWAP, TBCD, USDT, VAL, XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use hex_literal::hex;

use frame_support::traits::{Everything, GenesisBuild};
use frame_support::{construct_runtime, parameter_types};
use frame_system::{pallet_prelude::BlockNumberFor, EnsureRoot};
use multicollateral_bonding_curve_pool::{
    DistributionAccount, DistributionAccountData, DistributionAccounts,
};
use permissions::{Scope, BURN, MANAGE_DEX, MINT};
use sp_core::{ConstU32, H256};
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::{AccountId32, DispatchError, DispatchResult, Percent};

pub type DEXId = u32;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type AccountId = AccountId32;
pub type BlockNumber = u64;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub fn alice() -> AccountId {
    AccountId32::from(hex!(
        "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
    ))
}

parameter_types! {
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
    pub const BlockHashCount: u64 = 250;
    pub const GetNumSamples: usize = 40;
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetSyntheticBaseAssetId: AssetId = XST;
    pub const ExistentialDeposit: u128 = 0;
    pub GetFee: Fixed = fixed_from_basis_points(0u16);
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 10;
    pub GetIncentiveAssetId: AssetId = PSWAP;
    pub GetParliamentAccountId: AccountId = AccountId32::from([8; 32]);
    pub GetMarketMakerRewardsAccountId: AccountId = AccountId32::from([9; 32]);
    pub GetBondingCurveRewardsAccountId: AccountId = AccountId32::from([10; 32]);
    pub GetFarmingRewardsAccountId: AccountId = AccountId32::from([12; 32]);
    pub GetCrowdloanRewardsAccountId: AccountId = AccountId32::from([13; 32]);
    pub GetADARAccountId: AccountId = AccountId32::from([14; 32]);
    pub GetXykFee: Fixed = fixed!(0.003);
    pub const MinimumPeriod: u64 = 5;
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        LiquidityProxy: liquidity_proxy::{Pallet, Call, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        DexApi: dex_api::{Pallet, Call, Config, Storage, Event<T>},
        TradingPair: trading_pair::{Pallet, Call, Config<T>, Storage, Event<T>},
        PriceTools: price_tools::{Pallet, Storage, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        MBCPool: multicollateral_bonding_curve_pool::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Config<T>, Storage, Event<T>},
        VestedRewards: vested_rewards::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
    }
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

impl currencies::Config for Runtime {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = GetBaseAssetId;
    type WeightInfo = ();
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
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
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

impl dex_manager::Config for Runtime {}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance1> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance2> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance3> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance4> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl technical::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
}

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl dex_api::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MockLiquiditySource = ();
    type MockLiquiditySource2 = ();
    type MockLiquiditySource3 = ();
    type MockLiquiditySource4 = ();
    type XYKPool = pool_xyk::Pallet<Runtime>;
    type XSTPool = ();
    type MulticollateralBondingCurvePool = multicollateral_bonding_curve_pool::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type OrderBook = ();
    type WeightInfo = ();
}

impl trading_pair::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type WeightInfo = ();
}

impl demeter_farming_platform::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DemeterAssetId = ();
    const BLOCKS_PER_HOUR_AND_A_HALF: BlockNumberFor<Self> = 900;
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
}
impl vested_rewards::Config for Runtime {
    const BLOCKS_PER_DAY: BlockNumberFor<Self> = 14400;
    type RuntimeEvent = RuntimeEvent;
    type GetMarketMakerRewardsAccountId = GetMarketMakerRewardsAccountId;
    type GetBondingCurveRewardsAccountId = GetBondingCurveRewardsAccountId;
    type GetFarmingRewardsAccountId = GetFarmingRewardsAccountId;
    type WeightInfo = ();
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

fn bonding_curve_distribution_accounts() -> DistributionAccounts<
    DistributionAccountData<
        DistributionAccount<
            <Runtime as frame_system::Config>::AccountId,
            <Runtime as technical::Config>::TechAccountId,
        >,
    >,
> {
    use common::fixed_wrapper;
    let val_holders_coefficient = fixed_wrapper!(0.5);
    let val_holders_xor_alloc_coeff = fixed_wrapper!(0.9) * val_holders_coefficient.clone();
    let val_holders_buy_back_coefficient =
        val_holders_coefficient.clone() * (fixed_wrapper!(1) - fixed_wrapper!(0.9));
    let projects_coefficient = fixed_wrapper!(1) - val_holders_coefficient;
    let projects_sora_citizens_coeff = projects_coefficient.clone() * fixed_wrapper!(0.01);
    let projects_stores_and_shops_coeff = projects_coefficient.clone() * fixed_wrapper!(0.04);
    let projects_other_coeff = projects_coefficient * fixed_wrapper!(0.9);

    let xor_allocation = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"xor_allocation".to_vec()),
        )),
        val_holders_xor_alloc_coeff.get().unwrap(),
    );
    let sora_citizens = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"sora_citizens".to_vec()),
        )),
        projects_sora_citizens_coeff.get().unwrap(),
    );
    let stores_and_shops = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"stores_and_shops".to_vec()),
        )),
        projects_stores_and_shops_coeff.get().unwrap(),
    );
    let projects = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"projects".to_vec()),
        )),
        projects_other_coeff.get().unwrap(),
    );
    let val_holders = DistributionAccountData::new(
        DistributionAccount::TechAccount(TechAccountId::Pure(
            0u32,
            TechPurpose::Identifier(b"val_holders".to_vec()),
        )),
        val_holders_buy_back_coefficient.get().unwrap(),
    );
    DistributionAccounts::<_> {
        xor_allocation,
        sora_citizens,
        stores_and_shops,
        projects,
        val_holders,
    }
}

parameter_types! {
    pub GetMbcReservesTechAccountId: TechAccountId = {

        TechAccountId::from_generic_pair(
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_PREFIX.to_vec(),
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_RESERVES.to_vec(),
        )
    };
    pub GetMbcReservesAccountId: AccountId = {
        let tech_account_id = GetMbcReservesTechAccountId::get();

        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.")
    };
    pub GetMbcRewardsTechAccountId: TechAccountId = {

        TechAccountId::from_generic_pair(
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_PREFIX.to_vec(),
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_REWARDS.to_vec(),
        )
    };
    pub GetMbcRewardsAccountId: AccountId = {
        let tech_account_id = GetMbcRewardsTechAccountId::get();

        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.")
    };
    pub GetMbcFreeReservesTechAccountId: TechAccountId = {

        TechAccountId::from_generic_pair(
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_PREFIX.to_vec(),
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_FREE_RESERVES.to_vec(),
        )
    };
    pub GetMbcFreeReservesAccountId: AccountId = {
        let tech_account_id = GetMbcFreeReservesTechAccountId::get();

        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.")
    };
}

pub struct MockPriceTools;

impl PriceToolsProvider<AssetId> for MockPriceTools {
    fn get_average_price(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        _price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        let res = <LiquidityProxy as LiquidityProxyTrait<DEXId, AccountId, AssetId>>::quote(
            0,
            input_asset_id,
            output_asset_id,
            QuoteAmount::with_desired_input(balance!(1)),
            LiquiditySourceFilter::with_allowed(0u32, [LiquiditySourceType::XYKPool].to_vec()),
            true,
        );
        Ok(res?.amount)
    }

    fn register_asset(_: &AssetId) -> DispatchResult {
        // do nothing
        Ok(())
    }
}

impl multicollateral_bonding_curve_pool::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = liquidity_proxy::Pallet<Runtime>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type PriceToolsPallet = MockPriceTools;
    type VestedRewardsPallet = VestedRewards;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type BuyBackHandler = liquidity_proxy::LiquidityProxyBuyBackHandler<Runtime, GetBuyBackDexId>;
    type BuyBackTBCDPercent = GetTBCBuyBackTBCDPercent;
    type WeightInfo = ();
}

impl pswap_distribution::Config for Runtime {
    const PSWAP_BURN_PERCENT: Percent = Percent::from_percent(3);
    type RuntimeEvent = RuntimeEvent;
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type GetTBCDAssetId = GetBuyBackAssetId;
    type LiquidityProxy = liquidity_proxy::Pallet<Runtime>;
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

impl price_tools::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = LiquidityProxy;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type WeightInfo = price_tools::weights::SubstrateWeight<Runtime>;
}

impl Config for Runtime {}

pub struct ExtBuilder {
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    source_types: Vec<LiquiditySourceType>,
    tech_accounts: Vec<(AccountId, TechAccountId)>,
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
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            dex_list: vec![(
                0_u32,
                DEXInfo {
                    base_asset_id: GetBaseAssetId::get(),
                    synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
                    is_public: true,
                },
            )],
            initial_permission_owners: vec![
                (MINT, Scope::Unlimited, vec![alice()]),
                (BURN, Scope::Unlimited, vec![alice()]),
                (MANAGE_DEX, Scope::Unlimited, vec![alice()]),
            ],
            initial_permissions: vec![
                (alice(), Scope::Unlimited, vec![MINT, BURN]),
                (alice(), Scope::Limited(hash(&0_u32)), vec![MANAGE_DEX]),
                (
                    GetMbcReservesAccountId::get(),
                    Scope::Unlimited,
                    vec![MINT, BURN],
                ),
            ],
            source_types: vec![
                LiquiditySourceType::MulticollateralBondingCurvePool,
                LiquiditySourceType::XYKPool,
            ],
            tech_accounts: vec![
                (
                    GetMbcReservesAccountId::get(),
                    GetMbcReservesTechAccountId::get(),
                ),
                (
                    GetMbcRewardsAccountId::get(),
                    GetMbcRewardsTechAccountId::get(),
                ),
                (
                    GetLiquidityProxyAccountId::get(),
                    GetLiquidityProxyTechAccountId::get(),
                ),
            ],
            endowed_assets: vec![
                (
                    XOR,
                    alice(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    balance!(350000),
                    true,
                    None,
                    None,
                ),
                (
                    DOT,
                    alice(),
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"DOT".to_vec()),
                    10,
                    balance!(0),
                    true,
                    None,
                    None,
                ),
                (
                    VAL,
                    alice(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"VAL".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    balance!(0),
                    true,
                    None,
                    None,
                ),
                (
                    USDT,
                    alice(),
                    AssetSymbol(b"USDT".to_vec()),
                    AssetName(b"USDT".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    balance!(0),
                    true,
                    None,
                    None,
                ),
                (
                    PSWAP,
                    alice(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"PSWAP".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    balance!(0),
                    true,
                    None,
                    None,
                ),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        let accounts = bonding_curve_distribution_accounts();
        let mut tech_accounts = self.tech_accounts.clone();
        for account in &accounts.accounts() {
            match account {
                DistributionAccount::Account(_) => continue,
                DistributionAccount::TechAccount(account_id) => {
                    tech_accounts.push((
                        Technical::tech_account_id_to_account_id(account_id).unwrap(),
                        account_id.clone(),
                    ));
                }
            }
        }

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (alice(), 0),
                (
                    if let DistributionAccount::TechAccount(account_id) =
                        &accounts.val_holders.account
                    {
                        Technical::tech_account_id_to_account_id(account_id).unwrap()
                    } else {
                        panic!("not a tech account")
                    },
                    0,
                ),
                (GetMbcReservesAccountId::get(), 0),
                (GetMbcRewardsAccountId::get(), 0),
                (GetLiquidityProxyAccountId::get(), 0),
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

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        AssetsConfig {
            endowed_assets: self.endowed_assets,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        technical::GenesisConfig::<Runtime> {
            register_tech_accounts: tech_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        <dex_api::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
            &dex_api::GenesisConfig {
                source_types: self.source_types,
            },
            &mut t,
        )
        .unwrap();

        trading_pair::GenesisConfig::<Runtime> {
            trading_pairs: vec![
                (
                    0,
                    trading_pair::TradingPair::<Runtime> {
                        base_asset_id: XOR,
                        target_asset_id: VAL,
                    },
                ),
                (
                    0,
                    trading_pair::TradingPair::<Runtime> {
                        base_asset_id: XOR,
                        target_asset_id: PSWAP,
                    },
                ),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        multicollateral_bonding_curve_pool::GenesisConfig::<Runtime> {
            distribution_accounts: accounts,
            reserves_account_id: GetMbcReservesTechAccountId::get(),
            reference_asset_id: USDT,
            incentives_account_id: Some(GetMbcRewardsAccountId::get()),
            initial_collateral_assets: vec![VAL],
            free_reserves_account_id: Some(GetMbcFreeReservesAccountId::get()),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
