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

use crate::{AssetId32, Balance, PredefinedAssetId, TechAssetId};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::DispatchError;
use frame_support::weights::{
    WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use orml_traits::parameter_type_with_key;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use smallvec::smallvec;
use sp_arithmetic::Perbill;
use sp_runtime::AccountId32;
use sp_std::convert::TryFrom;

#[derive(
    Encode,
    Decode,
    Eq,
    PartialEq,
    Copy,
    Clone,
    PartialOrd,
    Ord,
    Debug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Hash))]
#[repr(u8)]
pub enum ComicAssetId {
    GoldenTicket,
    AppleTree,
    Apple,
    Teapot,
    Flower,
    RedPepper,
    BlackPepper,
    AcmeSpyKit,
    BatteryForMusicPlayer,
    MusicPlayer,
    Headphones,
    GreenPromise,
    BluePromise,
    Mango,
    MichaelJacksonCD,
    JesterMarotte,
    CrackedBrassBell,
    Tomato,
    Potato,
    Table,
    Future,
}

impl crate::traits::IsRepresentation for ComicAssetId {
    fn is_representation(&self) -> bool {
        false
    }
}

impl From<PredefinedAssetId> for AssetId32<ComicAssetId> {
    fn from(asset: PredefinedAssetId) -> Self {
        let comic = ComicAssetId::from(asset);
        AssetId32::<ComicAssetId>::from(comic)
    }
}

impl From<PredefinedAssetId> for ComicAssetId {
    fn from(asset_id: PredefinedAssetId) -> Self {
        use ComicAssetId::*;
        // only conversion; the `asset_id`'s place of construction must receive the warnings
        #[allow(deprecated)]
        match asset_id {
            PredefinedAssetId::XOR => GoldenTicket,
            PredefinedAssetId::DOT => AppleTree,
            PredefinedAssetId::KSM => Apple,
            PredefinedAssetId::USDT => Teapot,
            PredefinedAssetId::VAL => Flower,
            PredefinedAssetId::PSWAP => RedPepper,
            PredefinedAssetId::DAI => BlackPepper,
            PredefinedAssetId::ETH => AcmeSpyKit,
            PredefinedAssetId::XSTUSD => Mango,
            PredefinedAssetId::XST => BatteryForMusicPlayer,
            PredefinedAssetId::KEN => JesterMarotte,
            PredefinedAssetId::TBCD => MichaelJacksonCD,
            PredefinedAssetId::KUSD => CrackedBrassBell,
            PredefinedAssetId::KGOLD => Tomato,
            PredefinedAssetId::KXOR => Potato,
            PredefinedAssetId::KARMA => Table,
        }
    }
}

impl Default for ComicAssetId {
    fn default() -> Self {
        Self::GoldenTicket
    }
}

// This is never used, and just makes some tests compatible.
impl From<AssetId32<PredefinedAssetId>> for AssetId32<ComicAssetId> {
    fn from(_asset: AssetId32<PredefinedAssetId>) -> Self {
        unreachable!()
    }
}

// This is never used, and just makes some tests compatible.
impl From<TechAssetId<PredefinedAssetId>> for PredefinedAssetId {
    fn from(_tech: TechAssetId<PredefinedAssetId>) -> Self {
        unimplemented!()
    }
}

// This is never used, and just makes some tests compatible.
impl TryFrom<PredefinedAssetId> for TechAssetId<TechAssetId<PredefinedAssetId>>
where
    TechAssetId<PredefinedAssetId>: Decode,
{
    type Error = DispatchError;
    fn try_from(_asset: PredefinedAssetId) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

impl From<PredefinedAssetId> for TechAssetId<ComicAssetId> {
    fn from(asset_id: PredefinedAssetId) -> Self {
        TechAssetId::Wrapped(ComicAssetId::from(asset_id))
    }
}

pub struct WeightToFixedFee;

impl WeightToFeePolynomial for WeightToFixedFee {
    type Balance = Balance;

    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        smallvec!(WeightToFeeCoefficient {
            coeff_integer: 7_000_000,
            coeff_frac: Perbill::zero(),
            negative: false,
            degree: 1,
        })
    }
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId32<PredefinedAssetId>| -> Balance {
        0
    };
}
pub struct GetTradingPairRestrictedFlag;

impl<T> orml_traits::get_by_key::GetByKey<T, bool> for GetTradingPairRestrictedFlag {
    fn get(_key: &T) -> bool {
        false
    }
}

parameter_type_with_key! {
    pub GetChameleonPools: |base: AssetId32<PredefinedAssetId>| -> Option<(AssetId32<PredefinedAssetId>, sp_std::collections::btree_set::BTreeSet<AssetId32<PredefinedAssetId>>)> {
        if *base == crate::XOR {
            Some((crate::KXOR, [crate::ETH].into_iter().collect()))
        } else {
            None
        }
    };
}

pub fn alice() -> AccountId32 {
    AccountId32::from([1; 32])
}

pub fn bob() -> AccountId32 {
    AccountId32::from([2; 32])
}

pub fn charlie() -> AccountId32 {
    AccountId32::from([3; 32])
}

/// Mock of pallet `assets::Config`.
#[macro_export]
macro_rules! mock_assets_config {
    ($runtime:ty, $asset_regulator:ty) => {
        frame_support::parameter_types! {
            pub GetBuyBackAccountId: AccountId = AccountId32::from([23; 32]);
            pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![];
            pub const GetBuyBackPercentage: u8 = 0;
            pub GetBuyBackDexId: DEXId = DEXId::from(common::DEXId::PolkaswapXSTUSD);
        }
        impl assets::Config for $runtime {
            type AssetId = AssetId;
            type AssetRegulator = $asset_regulator;
            type BuyBackLiquidityProxy = ();
            type Currency = currencies::Pallet<$runtime>;
            type ExtraAccountId = [u8; 32];
            type ExtraAssetRecordArg =
                common::AssetIdExtraAssetRecordArg<DEXId, common::LiquiditySourceType, [u8; 32]>;
            type GetBaseAssetId = GetBaseAssetId;
            type GetBuyBackAccountId = GetBuyBackAccountId;
            type GetBuyBackAssetId = GetBuyBackAssetId;
            type GetBuyBackDexId = GetBuyBackDexId;
            type GetBuyBackPercentage = GetBuyBackPercentage;
            type GetBuyBackSupplyAssets = GetBuyBackSupplyAssets;
            type GetTotalBalance = ();
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
    ($runtime:ty) => {
        mock_assets_config!($runtime, permissions::Pallet<$runtime>);
    };
}

/// Mock of pallet `band::Config`.
#[macro_export]
macro_rules! mock_band_config {
    ($runtime:ty, $on_new_symbol_relayed_hook:ty, $on_symbol_disabled_hook:ty) => {
        frame_support::parameter_types! {
            pub const GetBandRateStalePeriod: u64 = 60*10*1000; // 10 minutes
            pub const GetBandRateStaleBlockPeriod: u64 = 600;
        }
        impl band::Config for $runtime {
            type GetBandRateStaleBlockPeriod = GetBandRateStaleBlockPeriod;
            type GetBandRateStalePeriod = GetBandRateStalePeriod;
            type MaxRelaySymbols = frame_support::traits::ConstU32<100>;
            type OnNewSymbolsRelayedHook = $on_new_symbol_relayed_hook;
            type OnSymbolDisabledHook = $on_symbol_disabled_hook;
            type RuntimeEvent = RuntimeEvent;
            type Symbol = common::SymbolName;
            type Time = Timestamp;
            type WeightInfo = ();
        }
    };
    ($runtime:ty, $on_symbol_disabled_hook:ty) => {
        mock_band_config!(
            $runtime,
            oracle_proxy::Pallet<$runtime>,
            $on_symbol_disabled_hook
        );
    };
    ($runtime:ty) => {
        mock_band_config!($runtime, oracle_proxy::Pallet<$runtime>, ());
    };
}

/// Mock of pallet `common::Config`.
#[macro_export]
macro_rules! mock_common_config {
    ($runtime:ty) => {
        impl common::Config for $runtime {
            type AssetManager = assets::Pallet<$runtime>;
            type DEXId = DEXId;
            type LstId = common::LiquiditySourceType;
            type MultiCurrency = currencies::Pallet<$runtime>;
        }
    };
}

/// Mock of pallet `currencies::Config`.
#[macro_export]
macro_rules! mock_currencies_config {
    ($runtime:ty) => {
        impl currencies::Config for $runtime {
            type GetNativeCurrencyId = <$runtime as assets::Config>::GetBaseAssetId;
            type MultiCurrency = Tokens;
            type NativeCurrency = BasicCurrencyAdapter<$runtime, Balances, Amount, BlockNumber>;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `dex_api::Config`.
#[macro_export]
macro_rules! mock_dex_api_config {
    (
        $runtime:ty,
        $mcbc_pool:ty,
        $xyk_pool:ty,
        $xst_pool:ty,
        $liquidity_source:ty,
        $liquidity_source2:ty,
        $liquidity_source3:ty,
        $liquidity_source4:ty
    ) => {
        impl dex_api::Config for $runtime {
            type DexInfoProvider = dex_manager::Pallet<$runtime>;
            type MockLiquiditySource = $liquidity_source;
            type MockLiquiditySource2 = $liquidity_source2;
            type MockLiquiditySource3 = $liquidity_source3;
            type MockLiquiditySource4 = $liquidity_source4;
            type MulticollateralBondingCurvePool = $mcbc_pool;
            type OrderBook = ();
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
            type XSTPool = $xst_pool;
            type XYKPool = $xyk_pool;
        }
    };
    ($runtime:ty, $mcbc_pool:ty, $xyk_pool:ty, $xst_pool:ty) => {
        mock_dex_api_config!($runtime, $mcbc_pool, $xyk_pool, $xst_pool, (), (), (), ());
    };
    ($runtime:ty, $mcbc_pool:ty) => {
        mock_dex_api_config!($runtime, $mcbc_pool, pool_xyk::Pallet<$runtime>, ());
    };
    ($runtime:ty) => {
        mock_dex_api_config!($runtime, ());
    };
}

/// Mock of pallet `dex_manager::Config`.
#[macro_export]
macro_rules! mock_dex_manager_config {
    ($runtime:ty) => {
        impl dex_manager::Config for $runtime {}
    };
}

/// Mock of pallet `extended_assets::Config`.
#[macro_export]
macro_rules! mock_extended_assets_config {
    ($runtime:ty) => {
        impl extended_assets::Config for $runtime {
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type MaxRegulatedAssetsPerSBT = ConstU32<10000>;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `frame_system::Config`.
#[macro_export]
macro_rules! mock_frame_system_config {
    ($runtime:ty, $ss58_prefix:ty, $max_consumers:ty, $account_data:ty) => {
        impl frame_system::Config for $runtime {
            type AccountData = $account_data;
            type AccountId = AccountId;
            type BaseCallFilter = frame_support::traits::Everything;
            type BlockHashCount = frame_support::traits::ConstU64<250>;
            type BlockLength = ();
            type BlockNumber = u64;
            type BlockWeights = ();
            type DbWeight = ();
            type Hash = sp_core::H256;
            type Hashing = sp_runtime::traits::BlakeTwo256;
            type Header = sp_runtime::testing::Header;
            type Index = u64;
            type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
            type MaxConsumers = $max_consumers;
            type OnKilledAccount = ();
            type OnNewAccount = ();
            type OnSetCode = ();
            type PalletInfo = PalletInfo;
            type RuntimeCall = RuntimeCall;
            type RuntimeEvent = RuntimeEvent;
            type RuntimeOrigin = RuntimeOrigin;
            type SS58Prefix = $ss58_prefix;
            type SystemWeightInfo = ();
            type Version = ();
        }
    };
    ($runtime:ty, $ss58_prefix:ty, $max_consumers:ty) => {
        mock_frame_system_config!(
            $runtime,
            $ss58_prefix,
            $max_consumers,
            pallet_balances::AccountData<Balance>
        );
    };
    ($runtime:ty, $ss58_prefix:ty) => {
        mock_frame_system_config!(
            $runtime,
            $ss58_prefix,
            frame_support::traits::ConstU32<65536>
        );
    };
    ($runtime:ty) => {
        mock_frame_system_config!($runtime, ());
    };
}

/// Mock of pallet `liquidity_proxy::Config`.
#[macro_export]
macro_rules! mock_liquidity_proxy_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
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
            pub GetInternalSlippageTolerancePercent: sp_runtime::Permill = sp_runtime::Permill::from_rational(1u32, 1000); // 0.1%
        }
        impl liquidity_proxy::Config for $runtime {
            type ADARCommissionRatioUpdateOrigin =
                frame_system::EnsureRoot<sp_runtime::AccountId32>;
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type DexInfoProvider = dex_manager::Pallet<$runtime>;
            type GetADARAccountId = GetADARAccountId;
            type GetChameleonPools = common::mock::GetChameleonPools;
            type GetNumSamples = GetNumSamples;
            type GetTechnicalAccountId = GetLiquidityProxyAccountId;
            type InternalSlippageTolerance = GetInternalSlippageTolerancePercent;
            type LiquidityRegistry = dex_api::Pallet<$runtime>;
            type LockedLiquiditySourcesManager = trading_pair::Pallet<$runtime>;
            type MaxAdditionalDataLengthSwapTransferBatch = ConstU32<2000>;
            type MaxAdditionalDataLengthXorlessTransfer = ConstU32<128>;
            type PrimaryMarketTBC = ();
            type PrimaryMarketXST = ();
            type RuntimeEvent = RuntimeEvent;
            type SecondaryMarket = ();
            type TradingPairSourceManager = trading_pair::Pallet<$runtime>;
            type VestedRewardsPallet = vested_rewards::Pallet<$runtime>;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `multicollateral_bonding_curve_pool::Config`.
#[macro_export]
macro_rules! mock_multicollateral_bonding_curve_pool_config {
    ($runtime:ty, $liquidity_proxy:ty, $buy_back_handler:ty, $price_tool:ty, $trading_pair:ty) => {
        frame_support::parameter_types! {
            pub GetTBCBuyBackTBCDPercent: common::Fixed = common::fixed!(0.025);
            pub GetTbcIrreducibleReservePercent: sp_runtime::Percent = sp_runtime::Percent::from_percent(1);
        }
        impl multicollateral_bonding_curve_pool::Config for $runtime {
            const RETRY_DISTRIBUTION_FREQUENCY: BlockNumber = 1000;
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type BuyBackHandler = $buy_back_handler;
            type BuyBackTBCDPercent = GetTBCBuyBackTBCDPercent;
            type EnsureDEXManager = dex_manager::Pallet<$runtime>;
            type EnsureTradingPairExists = $trading_pair;
            type IrreducibleReserve = GetTbcIrreducibleReservePercent;
            type LiquidityProxy = $liquidity_proxy;
            type PriceToolsPallet = $price_tool;
            type RuntimeEvent = RuntimeEvent;
            type TradingPairSourceManager = $trading_pair;
            type VestedRewardsPallet = VestedRewards;
            type WeightInfo = ();
        }
    };
    ($runtime:ty, $liquidity_proxy:ty, $buy_back_handler:ty, $price_tool:ty) => {
        mock_multicollateral_bonding_curve_pool_config!($runtime, $liquidity_proxy, $buy_back_handler, $price_tool, trading_pair::Pallet<$runtime>);
    };
    ($runtime:ty, $liquidity_proxy:ty, $buy_back_handler:ty) => {
        mock_multicollateral_bonding_curve_pool_config!($runtime, $liquidity_proxy, $buy_back_handler, ());
    };
    ($runtime:ty, $liquidity_proxy:ty) => {
        mock_multicollateral_bonding_curve_pool_config!($runtime, $liquidity_proxy, ());
    };
    ($runtime:ty) => {
        mock_multicollateral_bonding_curve_pool_config!($runtime, ());
    };
}

/// Mock of pallet `oracle_proxy::Config`.
#[macro_export]
macro_rules! mock_oracle_proxy_config {
    ($runtime:ty, $band_chain_oracle:ty) => {
        impl oracle_proxy::Config for $runtime {
            type BandChainOracle = $band_chain_oracle;
            type RuntimeEvent = RuntimeEvent;
            type Symbol = <$runtime as band::Config>::Symbol;
            type WeightInfo = ();
        }
    };
    ($runtime:ty) => {
        mock_oracle_proxy_config!($runtime, band::Pallet<$runtime>);
    };
}

/// Mock of pallet `orml_tokens::Config`.
#[macro_export]
macro_rules! mock_orml_tokens_config {
    ($runtime:ty) => {
        impl orml_tokens::Config for $runtime {
            type Amount = Amount;
            type Balance = Balance;
            type CurrencyHooks = ();
            type CurrencyId = <$runtime as assets::Config>::AssetId;
            type DustRemovalWhitelist = Everything;
            type ExistentialDeposits = ExistentialDeposits;
            type MaxLocks = ();
            type MaxReserves = ();
            type ReserveIdentifier = ();
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `pallet_balances::Config`.
#[macro_export]
macro_rules! mock_pallet_balances_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const ExistentialDeposit: u128 = 0;
        }
        impl pallet_balances::Config for $runtime {
            type AccountStore = System;
            type Balance = Balance;
            type DustRemoval = ();
            type ExistentialDeposit = ExistentialDeposit;
            type MaxLocks = ();
            type MaxReserves = ();
            type ReserveIdentifier = ();
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `pallet_timestamp::Config`.
#[macro_export]
macro_rules! mock_pallet_timestamp_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const MinimumPeriod: u64 = 5;
        }
        impl pallet_timestamp::Config for $runtime {
            type MinimumPeriod = MinimumPeriod;
            type Moment = u64;
            type OnTimestampSet = ();
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `permissions::Config`.
#[macro_export]
macro_rules! mock_permissions_config {
    ($runtime:ty) => {
        impl permissions::Config for $runtime {
            type RuntimeEvent = RuntimeEvent;
        }
    };
}

/// Mock of pallet `pool_xyk::Config`.
#[macro_export]
macro_rules! mock_pool_xyk_config {
    ($runtime:ty, $trading_pair:ty, $enabled_sources:ty, $on_pool_created:ty) => {
        frame_support::parameter_types! {
            pub GetXykFee: common::Fixed = common::fixed!(0.003);
            pub GetXykIrreducibleReservePercent: sp_runtime::Percent = sp_runtime::Percent::from_percent(1);
            pub GetXykMaxIssuanceRatio: common::Fixed = common::fixed!(1.5);
        }
        impl pool_xyk::Config for $runtime {
            const MIN_XOR: Balance = balance!(0.0007);
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type AssetRegulator = ();
            type DepositLiquidityAction =
                pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
            type DexInfoProvider = dex_manager::Pallet<$runtime>;
            type EnabledSourcesManager = $enabled_sources;
            type EnsureDEXManager = dex_manager::Pallet<$runtime>;
            type EnsureTradingPairExists = $enabled_sources;
            type GetChameleonPools = common::mock::GetChameleonPools;
            type GetFee = GetXykFee;
            type GetMaxIssuanceRatio = GetXykMaxIssuanceRatio;
            type GetTradingPairRestrictedFlag = common::mock::GetTradingPairRestrictedFlag;
            type IrreducibleReserve = GetXykIrreducibleReservePercent;
            type OnPoolCreated = $on_pool_created;
            type OnPoolReservesChanged = ();
            type PairSwapAction =
                pool_xyk::PairSwapAction<DEXId, AssetId, AccountId, TechAccountId>;
            type PolySwapAction =
                pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
            type PoolAdjustPeriod = sp_runtime::traits::ConstU64<1>;
            type RuntimeEvent = RuntimeEvent;
            type TradingPairSourceManager = $trading_pair;
            type WeightInfo = ();
            type WithdrawLiquidityAction =
                pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
            type XSTMarketInfo = ();
        }
    };
    ($runtime:ty, $trading_pair:ty, $enabled_trading_pair:ty) => {
        mock_pool_xyk_config!(
            $runtime,
            $trading_pair,
            $enabled_trading_pair,
            PswapDistribution
        );
    };
    ($runtime:ty, $trading_pair:ty) => {
        mock_pool_xyk_config!($runtime, $trading_pair, trading_pair::Pallet<$runtime>);
    };
    ($runtime:ty) => {
        mock_pool_xyk_config!($runtime, trading_pair::Pallet<$runtime>);
    };
}

/// Mock of pallet `price_tools::Config`.
#[macro_export]
macro_rules! mock_price_tools_config {
    ($runtime:ty, $liquidity_proxy:ty) => {
        impl price_tools::Config for $runtime {
            type LiquidityProxy = $liquidity_proxy;
            type RuntimeEvent = RuntimeEvent;
            type TradingPairSourceManager = trading_pair::Pallet<$runtime>;
            type WeightInfo = price_tools::weights::SubstrateWeight<$runtime>;
        }
    };
    ($runtime:ty) => {
        mock_price_tools_config!($runtime, ());
    };
}

/// Mock of pallet `pswap_distribution::Config`.
#[macro_export]
macro_rules! mock_pswap_distribution_config {
    (
        $runtime:ty,
        $xyk_pool:ty,
        $chameleon_pools:ty,
        $liquidity_proxy:ty,
        $buy_back_handler:ty
    ) => {
        frame_support::parameter_types! {
            pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
        }
        impl pswap_distribution::Config for $runtime {
            const PSWAP_BURN_PERCENT: sp_runtime::Percent = sp_runtime::Percent::from_percent(3);
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type BuyBackHandler = $buy_back_handler;
            type CompatBalance = Balance;
            type DexInfoProvider = dex_manager::Pallet<$runtime>;
            type EnsureDEXManager = ();
            type GetBurnUpdateFrequency = GetBurnUpdateFrequency;
            type GetBuyBackAssetId = GetBuyBackAssetId;
            type GetChameleonPools = $chameleon_pools;
            type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
            type GetIncentiveAssetId = GetIncentiveAssetId;
            type GetParliamentAccountId = GetParliamentAccountId;
            type GetTechnicalAccountId = GetPswapDistributionAccountId;
            type LiquidityProxy = $liquidity_proxy;
            type OnPswapBurnedAggregator = ();
            type PoolXykPallet = $xyk_pool;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
    ($runtime:ty, $xyk_pool:ty, $chameleon_pools:ty, $liquidity_proxy:ty) => {
        mock_pswap_distribution_config!(
            $runtime,
            $xyk_pool,
            $chameleon_pools,
            $liquidity_proxy,
            ()
        );
    };
    ($runtime:ty, $xyk_pool:ty, $chameleon_pools:ty) => {
        mock_pswap_distribution_config!($runtime, $xyk_pool, $chameleon_pools, ());
    };
    ($runtime:ty, $xyk_pool:ty) => {
        mock_pswap_distribution_config!($runtime, $xyk_pool, common::mock::GetChameleonPools);
    };
    ($runtime:ty) => {
        mock_pswap_distribution_config!($runtime, pool_xyk::Pallet<$runtime>);
    };
}

/// Mock of pallet `technical::Config`.
#[macro_export]
macro_rules! mock_technical_config {
    ($runtime:ty, $swap_action:ty) => {
        impl technical::Config for $runtime {
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type Condition = ();
            type RuntimeEvent = RuntimeEvent;
            type SwapAction = $swap_action;
            type TechAccountId = TechAccountId;
            type TechAssetId = TechAssetId;
            type Trigger = ();
        }
    };
    ($runtime:ty) => {
        mock_technical_config!($runtime, ());
    };
}

/// Mock of pallet `tokens::Config`.
#[macro_export]
macro_rules! mock_tokens_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const MaxLocks: u32 = 1;
        }
        impl tokens::Config for $runtime {
            type Amount = Amount;
            type Balance = Balance;
            type CurrencyHooks = ();
            type CurrencyId = <$runtime as assets::Config>::AssetId;
            type DustRemovalWhitelist = Everything;
            type ExistentialDeposits = ExistentialDeposits;
            type MaxLocks = MaxLocks;
            type MaxReserves = ();
            type ReserveIdentifier = ();
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `trading_pair::Config`.
#[macro_export]
macro_rules! mock_trading_pair_config {
    ($runtime:ty) => {
        impl trading_pair::Config for $runtime {
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type DexInfoProvider = dex_manager::Pallet<$runtime>;
            type EnsureDEXManager = dex_manager::Pallet<$runtime>;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `vested-rewards::Config`
#[macro_export]
macro_rules! mock_vested_rewards_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const MaxVestingSchedules: u32 = 0;
            pub const MinVestedTransfer: common::prelude::Balance = 0;
        }
        impl vested_rewards::Config for $runtime {
            const BLOCKS_PER_DAY: BlockNumberFor<Self> = 14400;
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type Currency = Tokens;
            type GetBondingCurveRewardsAccountId = GetBondingCurveRewardsAccountId;
            type GetFarmingRewardsAccountId = GetFarmingRewardsAccountId;
            type GetMarketMakerRewardsAccountId = GetMarketMakerRewardsAccountId;
            type MaxVestingSchedules = MaxVestingSchedules;
            type MaxWeightForAutoClaim = ();
            type MinVestedTransfer = MinVestedTransfer;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `xst::Config`.
#[macro_export]
macro_rules! mock_xst_config {
    ($runtime:ty, $price_tool:ty) => {
        impl xst::Config for $runtime {
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type EnsureDEXManager = dex_manager::Pallet<$runtime>;
            type GetSyntheticBaseAssetId = GetSyntheticBaseAssetId;
            type GetSyntheticBaseBuySellLimit = GetSyntheticBaseBuySellLimit;
            type GetXSTPoolPermissionedTechAccountId = GetXSTPoolPermissionedTechAccountId;
            type Oracle = OracleProxy;
            type PriceToolsPallet = $price_tool;
            type RuntimeEvent = RuntimeEvent;
            type Symbol = common::SymbolName;
            type TradingPairSourceManager = trading_pair::Pallet<$runtime>;
            type WeightInfo = ();
        }
    };
    ($runtime:ty) => {
        mock_xst_config!($runtime, ());
    };
}
