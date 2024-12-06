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
    Pan,
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
            PredefinedAssetId::PRUSD => Pan,
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

/// Mock of pallet `apollo_platform::Config`.
#[macro_export]
macro_rules! mock_apollo_platform_config {
    ($runtime:ty) => {
        impl apollo_platform::Config for $runtime {
            const BLOCKS_PER_FIFTEEN_MINUTES: BlockNumberFor<Self> = 150;
            type LiquidityProxyPallet = MockLiquidityProxy;
            type PriceTools = MockPriceTools;
            type RuntimeEvent = RuntimeEvent;
            type UnsignedLongevity = frame_support::traits::ConstU64<100>;
            type UnsignedPriority = frame_support::traits::ConstU64<100>;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `assets::Config`.
#[macro_export]
macro_rules! mock_assets_config {
    (
        $runtime:ty,
        $asset_regulator:ty,
        $extra_account:ty,
        $buy_back_liquidity_proxy:ty,
        $buy_back_percentage:expr,
        $buy_back_account_id:expr,
        $buy_back_dex_id:expr,
        $buy_back_supply_assets:expr
    ) => {
        frame_support::parameter_types! {
            pub const GetBuyBackPercentage: u8 = $buy_back_percentage;
            pub GetBuyBackAccountId: AccountId = $buy_back_account_id;
            pub GetBuyBackDexId: DEXId = $buy_back_dex_id;
            pub GetBuyBackSupplyAssets: Vec<AssetId> = $buy_back_supply_assets;
        }
        impl assets::Config for $runtime {
            type AssetId = AssetId;
            type AssetRegulator = $asset_regulator;
            type BuyBackLiquidityProxy = $buy_back_liquidity_proxy;
            type Currency = currencies::Pallet<$runtime>;
            type ExtraAccountId = $extra_account;
            type ExtraAssetRecordArg = common::AssetIdExtraAssetRecordArg<
                DEXId,
                common::LiquiditySourceType,
                $extra_account,
            >;
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
    ($runtime:ty, $asset_regulator:ty) => {
        mock_assets_config!(
            $runtime,
            $asset_regulator,
            [u8; 32],
            (),
            0,
            sp_core::crypto::AccountId32::from([23; 32]),
            DEXId::from(common::DEXId::PolkaswapXSTUSD),
            vec![]
        );
    };
    ($runtime:ty) => {
        mock_assets_config!($runtime, permissions::Pallet<$runtime>);
    };
}

/// Mock of pallet `band::Config`.
#[macro_export]
macro_rules! mock_band_config {
    (
        $runtime:ty,
        $on_new_symbol_relayed_hook:ty,
        $on_symbol_disabled_hook:ty,
        $symbol:ty
    ) => {
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
            type Symbol = $symbol;
            type Time = Timestamp;
            type WeightInfo = ();
        }
    };
    ($runtime:ty, $on_new_symbol_relayed_hook:ty, $on_symbol_disabled_hook:ty) => {
        mock_band_config!(
            $runtime,
            $on_new_symbol_relayed_hook,
            $on_symbol_disabled_hook,
            common::SymbolName
        );
    };
    ($runtime:ty, $on_new_symbol_relayed_hook:ty) => {
        mock_band_config!($runtime, $on_new_symbol_relayed_hook, ());
    };
    ($runtime:ty) => {
        mock_band_config!($runtime, oracle_proxy::Pallet<$runtime>);
    };
}

/// Mock of pallet `bridge_channel::outbound::Config`.
#[macro_export]
macro_rules! mock_bridge_channel_outbound_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const MaxTotalGasLimit: u64 = 5_000_000;
            pub const ThisNetworkId: bridge_types::GenericNetworkId = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);
        }
        impl bridge_channel::outbound::Config for $runtime {
            type RuntimeEvent = RuntimeEvent;
            type MaxMessagePayloadSize = MaxMessagePayloadSize;
            type MaxMessagesPerCommit = MaxMessagesPerCommit;
            type MessageStatusNotifier = BridgeProxy;
            type AuxiliaryDigestHandler = ();
            type ThisNetworkId = ThisNetworkId;
            type AssetId = AssetId;
            type Balance = Balance;
            type MaxGasPerCommit = MaxTotalGasLimit;
            type MaxGasPerMessage = MaxTotalGasLimit;
            type TimepointProvider = GenericTimepointProvider;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `bridge_multisig::Config`.
#[macro_export]
macro_rules! mock_bridge_multisig_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const DepositBase: u64 = 1;
            pub const DepositFactor: u64 = 1;
            pub const MaxSignatories: u16 = 4;
        }
        impl bridge_multisig::Config for $runtime {
            type Currency = Balances;
            type DepositBase = DepositBase;
            type DepositFactor = DepositFactor;
            type MaxSignatories = MaxSignatories;
            type RuntimeCall = RuntimeCall;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `ceres_governance_platform::Config`.
#[macro_export]
macro_rules! mock_ceres_governance_platform_config {
    ($runtime:ty) => {
        impl ceres_governance_platform::Config for $runtime {
            type DescriptionLimit = DescriptionLimit;
            type OptionsLimit = OptionsLimit;
            type RuntimeEvent = RuntimeEvent;
            type StringLimit = StringLimit;
            type TitleLimit = TitleLimit;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `ceres_liquidity_locker::Config`.
#[macro_export]
macro_rules! mock_ceres_liquidity_locker_config {
    ($runtime:ty, $pool_xyk:ty, $ceres_asset_id:ty) => {
        impl ceres_liquidity_locker::Config for $runtime {
            const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self> = 14_440;
            type CeresAssetId = $ceres_asset_id;
            type DemeterFarmingPlatform = DemeterFarmingPlatform;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
            type XYKPool = $pool_xyk;
        }
    };
    ($runtime:ty, $pool_xyk:ty) => {
        mock_ceres_liquidity_locker_config!($runtime, $pool_xyk, ());
    };
    ($runtime:ty) => {
        mock_ceres_liquidity_locker_config!($runtime, ());
    };
}

/// Mock of pallet `ceres_token_locker::Config`.
#[macro_export]
macro_rules! mock_ceres_token_locker_config {
    ($runtime:ty) => {
        impl ceres_token_locker::Config for $runtime {
            type AssetInfoProvider = assets::Pallet<Runtime>;
            type CeresAssetId = CeresAssetId;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
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

/// Mock of pallet `demeter_farming_platform::Config`.
#[macro_export]
macro_rules! mock_demeter_farming_platform_config {
    ($runtime:ty, $demeter_asset_id:ty) => {
        impl demeter_farming_platform::Config for $runtime {
            const BLOCKS_PER_HOUR_AND_A_HALF: frame_system::pallet_prelude::BlockNumberFor<Self> =
                900;
            type AssetInfoProvider = assets::Pallet<Runtime>;
            type DemeterAssetId = $demeter_asset_id;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
    ($runtime:ty) => {
        mock_demeter_farming_platform_config!($runtime, ());
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
        $liquidity_source1:ty,
        $liquidity_source2:ty,
        $liquidity_source3:ty,
        $liquidity_source4:ty,
        $order_book:ty
    ) => {
        impl dex_api::Config for $runtime {
            type DexInfoProvider = dex_manager::Pallet<$runtime>;
            type MockLiquiditySource = mock_liquidity_source::Pallet<$runtime, $liquidity_source1>;
            type MockLiquiditySource2 = mock_liquidity_source::Pallet<$runtime, $liquidity_source2>;
            type MockLiquiditySource3 = mock_liquidity_source::Pallet<$runtime, $liquidity_source3>;
            type MockLiquiditySource4 = mock_liquidity_source::Pallet<$runtime, $liquidity_source4>;
            type MulticollateralBondingCurvePool = $mcbc_pool;
            type OrderBook = $order_book;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
            type XSTPool = $xst_pool;
            type XYKPool = $xyk_pool;
        }
    };
    (
        $runtime:ty,
        $mcbc_pool:ty,
        $xyk_pool:ty,
        $xst_pool:ty,
        $liquidity_source1:ty,
        $liquidity_source2:ty,
        $liquidity_source3:ty,
        $liquidity_source4:ty
    ) => {
        mock_dex_api_config!(
            $runtime,
            $mcbc_pool,
            $xyk_pool,
            $xst_pool,
            $liquidity_source1,
            $liquidity_source2,
            $liquidity_source3,
            $liquidity_source4,
            ()
        );
    };
    ($runtime:ty, $mcbc_pool:ty, $xyk_pool:ty, $xst_pool:ty) => {
        impl dex_api::Config for $runtime {
            type DexInfoProvider = dex_manager::Pallet<$runtime>;
            type MockLiquiditySource = ();
            type MockLiquiditySource2 = ();
            type MockLiquiditySource3 = ();
            type MockLiquiditySource4 = ();
            type MulticollateralBondingCurvePool = $mcbc_pool;
            type OrderBook = ();
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
            type XSTPool = $xst_pool;
            type XYKPool = $xyk_pool;
        }
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

/// Mock of pallet `dispatch::Config`.
#[macro_export]
macro_rules! mock_dispatch_config {
    ($runtime:ty) => {
        impl dispatch::Config for $runtime {
            type Call = RuntimeCall;
            type CallFilter = frame_support::traits::Everything;
            type Hashing = sp_runtime::traits::Keccak256;
            type MessageId = MessageId;
            type Origin = RuntimeOrigin;
            type OriginOutput = OriginOutput;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `evm_fungible_app::Config`.
#[macro_export]
macro_rules! mock_evm_fungible_app_config {
    ($runtime:ty) => {
        impl evm_fungible_app::Config for $runtime {
            type AppRegistry = AppRegistryImpl;
            type AssetIdConverter = sp_runtime::traits::ConvertInto;
            type AssetRegistry = BridgeProxy;
            type BalancePrecisionConverter = BalancePrecisionConverterImpl;
            type BaseFeeLifetime = frame_support::traits::ConstU64<100>;
            type BridgeAssetLocker = BridgeProxy;
            type CallOrigin = dispatch::EnsureAccount<OriginOutput>;
            type MessageStatusNotifier = BridgeProxy;
            type OutboundChannel = BridgeOutboundChannel;
            type PriorityFee = frame_support::traits::ConstU128<100>;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
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

/// Mock of pallet `kensetsu::Config`.
#[macro_export]
macro_rules! mock_kensetsu_config {
    ($runtime:ty) => {
        impl kensetsu::Config for TestRuntime {
            type AssetInfoProvider = Assets;
            type DepositoryTechAccount = KensetsuDepositoryTechAccountId;
            type KarmaAssetId = KarmaAssetId;
            type KarmaIncentiveRemintPercent = GetKarmaIncentiveRemintPercent;
            type KenAssetId = KenAssetId;
            type KenIncentiveRemintPercent = GetKenIncentiveRemintPercent;
            type LiquidityProxy = MockLiquidityProxy;
            type MaxCdpsPerOwner = frame_support::traits::ConstU32<10000>;
            type MinimalStabilityFeeAccrue = MinimalStabilityFeeAccrue;
            type Oracle = MockOracle;
            type PriceTools = MockPriceTools;
            type Randomness = MockRandomness;
            type RuntimeEvent = RuntimeEvent;
            type TbcdAssetId = TbcdAssetId;
            type TradingPairSourceManager = MockTradingPairSourceManager;
            type TreasuryTechAccount = KensetsuTreasuryTechAccountId;
            type UnsignedLongevity = frame_support::traits::ConstU64<100>;
            type UnsignedPriority = frame_support::traits::ConstU64<100>;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `liquidity_proxy::Config`.
#[macro_export]
macro_rules! mock_liquidity_proxy_config {
    ($runtime:ty, $primaty_market_tbc:ty, $primaty_market_xst:ty, $secondary_market:ty) => {
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
            type MaxAdditionalDataLengthSwapTransferBatch = frame_support::traits::ConstU32<2000>;
            type MaxAdditionalDataLengthXorlessTransfer = frame_support::traits::ConstU32<128>;
            type PrimaryMarketTBC = $primaty_market_tbc;
            type PrimaryMarketXST = $primaty_market_xst;
            type RuntimeEvent = RuntimeEvent;
            type SecondaryMarket = $secondary_market;
            type TradingPairSourceManager = trading_pair::Pallet<$runtime>;
            type VestedRewardsPallet = vested_rewards::Pallet<$runtime>;
            type WeightInfo = ();
        }
    };
    ($runtime:ty) => {
        mock_liquidity_proxy_config!($runtime, (), (), ());
    };
}

/// Mock of pallet `mock_liquidity_source::Config`.
#[macro_export]
macro_rules! mock_liquidity_source_config {
    (
        $runtime:ty,
        $instance:ty,
        $ensure_dex_manager:ty,
        $get_fee:ty,
        $ensure_trading_pair:ty
    ) => {
        impl mock_liquidity_source::Config<$instance> for $runtime {
            type DexInfoProvider = dex_manager::Pallet<$runtime>;
            type EnsureDEXManager = $ensure_dex_manager;
            type EnsureTradingPairExists = $ensure_trading_pair;
            type GetFee = $get_fee;
        }
    };
    ($runtime:ty, $instance:ty, $ensure_dex_manager:ty, $get_fee:ty) => {
        mock_liquidity_source_config!($runtime, $instance, $ensure_dex_manager, $get_fee, ());
    };
    ($runtime:ty, $instance:ty, $ensure_dex_manager:ty) => {
        mock_liquidity_source_config!($runtime, $instance, $ensure_dex_manager, ());
    };
    ($runtime:ty, $instance:ty) => {
        mock_liquidity_source_config!($runtime, $instance, ());
    };
}

/// Mock of pallet `multicollateral_bonding_curve_pool::Config`.
#[macro_export]
macro_rules! mock_multicollateral_bonding_curve_pool_config {
    (
        $runtime:ty,
        $liquidity_proxy:ty,
        $buy_back_handler:ty,
        $price_tool:ty,
        $trading_pair:ty,
        $vested_rewards:ty
    ) => {
        frame_support::parameter_types! {
            pub GetTBCBuyBackAssetId: AssetId = $crate::KUSD;
            pub GetTbcIrreducibleReservePercent: sp_runtime::Percent = sp_runtime::Percent::from_percent(1);
        }
        impl multicollateral_bonding_curve_pool::Config for $runtime {
            const RETRY_DISTRIBUTION_FREQUENCY: BlockNumber = 1000;
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type BuyBackHandler = $buy_back_handler;
            type GetBuyBackAssetId = GetTBCBuyBackAssetId;
            type EnsureDEXManager = dex_manager::Pallet<$runtime>;
            type EnsureTradingPairExists = $trading_pair;
            type IrreducibleReserve = GetTbcIrreducibleReservePercent;
            type LiquidityProxy = $liquidity_proxy;
            type PriceToolsPallet = $price_tool;
            type RuntimeEvent = RuntimeEvent;
            type TradingPairSourceManager = $trading_pair;
            type VestedRewardsPallet = $vested_rewards;
            type WeightInfo = ();
        }
    };
    ($runtime:ty, $liquidity_proxy:ty, $buy_back_handler:ty, $price_tool:ty, $trading_pair:ty) => {
        mock_multicollateral_bonding_curve_pool_config!($runtime, $liquidity_proxy, $buy_back_handler, $price_tool, $trading_pair, VestedRewards);
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
    ($runtime:ty, $band_chain_oracle:ty, $symbol:ty) => {
        impl oracle_proxy::Config for $runtime {
            type BandChainOracle = $band_chain_oracle;
            type RuntimeEvent = RuntimeEvent;
            type Symbol = $symbol;
            type WeightInfo = ();
        }
    };
    ($runtime:ty, $band_chain_oracle:ty) => {
        mock_oracle_proxy_config!(
            $runtime,
            $band_chain_oracle,
            <$runtime as band::Config>::Symbol
        );
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
            type DustRemovalWhitelist = frame_support::traits::Everything;
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

/// Mock of pallet `pallet_multisig::Config`.
#[macro_export]
macro_rules! mock_pallet_multisig_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const DepositBase: u64 = 1;
            pub const DepositFactor: u64 = 1;
            pub const MaxSignatories: u16 = 4;
        }
        impl pallet_multisig::Config for $runtime {
            type Currency = Balances;
            type DepositBase = DepositBase;
            type DepositFactor = DepositFactor;
            type MaxSignatories = MaxSignatories;
            type RuntimeCall = RuntimeCall;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `pallet_scheduler::Config`.
#[macro_export]
macro_rules! mock_pallet_scheduler_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const SchedulerMaxWeight: Weight = Weight::from_parts(1024, 0);
        }
        impl pallet_scheduler::Config for $runtime {
            type MaxScheduledPerBlock = ();
            type MaximumWeight = SchedulerMaxWeight;
            type OriginPrivilegeCmp = OriginPrivilegeCmp;
            type PalletsOrigin = OriginCaller;
            type Preimages = ();
            type RuntimeCall = RuntimeCall;
            type RuntimeEvent = RuntimeEvent;
            type RuntimeOrigin = RuntimeOrigin;
            type ScheduleOrigin = frame_system::EnsureRoot<AccountId>;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `pallet_sudo::Config`.
#[macro_export]
macro_rules! mock_pallet_sudo_config {
    ($runtime:ty) => {
        impl pallet_sudo::Config for $runtime {
            type RuntimeCall = RuntimeCall;
            type RuntimeEvent = RuntimeEvent;
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

/// Mock of pallet `pallet_transaction_payment::Config`.
#[macro_export]
macro_rules! mock_pallet_transaction_payment_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const OperationalFeeMultiplier: u8 = 5;
        }
        impl pallet_transaction_payment::Config for $runtime {
            type RuntimeEvent = RuntimeEvent;
            type OnChargeTransaction = XorFee;
            type WeightToFee = frame_support::weights::IdentityFee<Balance>;
            type FeeMultiplierUpdate = ();
            type LengthToFee = frame_support::weights::ConstantMultiplier<
                Balance,
                frame_support::traits::ConstU128<0>,
            >;
            type OperationalFeeMultiplier = OperationalFeeMultiplier;
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
    (
        $runtime:ty,
        $trading_pair:ty,
        $enabled_sources:ty,
        $on_pool_created:ty,
        $asset_regulator:ty,
        $chameleon_pools:ty,
        $trading_pair_restricted_flag:ty,
        $xst_market_info:ty,
        $min_xor:expr
    ) => {
        frame_support::parameter_types! {
            pub GetXykFee: common::Fixed = common::fixed!(0.006);
            pub GetXykIrreducibleReservePercent: sp_runtime::Percent = sp_runtime::Percent::from_percent(1);
            pub GetXykMaxIssuanceRatio: common::Fixed = common::fixed!(1.5);
        }
        impl pool_xyk::Config for $runtime {
            const MIN_XOR: Balance = $min_xor;
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type AssetRegulator = $asset_regulator;
            type DepositLiquidityAction =
                pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
            type DexInfoProvider = dex_manager::Pallet<$runtime>;
            type EnabledSourcesManager = $enabled_sources;
            type EnsureDEXManager = dex_manager::Pallet<$runtime>;
            type EnsureTradingPairExists = $enabled_sources;
            type GetChameleonPools = $chameleon_pools;
            type GetFee = GetXykFee;
            type GetMaxIssuanceRatio = GetXykMaxIssuanceRatio;
            type GetTradingPairRestrictedFlag = $trading_pair_restricted_flag;
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
            type XSTMarketInfo = $xst_market_info;
        }
    };
    ($runtime:ty, $trading_pair:ty, $enabled_sources:ty, $on_pool_created:ty) => {
        mock_pool_xyk_config!(
            $runtime,
            $trading_pair,
            $enabled_sources,
            $on_pool_created,
            (),
            common::mock::GetChameleonPools,
            common::mock::GetTradingPairRestrictedFlag,
            (),
            balance!(0.0007)
        );
    };
    ($runtime:ty, $trading_pair:ty, $enabled_sources:ty) => {
        mock_pool_xyk_config!(
            $runtime,
            $trading_pair,
            $enabled_sources,
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
    ($runtime:ty, $liquidity_proxy:ty, $trading_pair:ty, $weight:ty) => {
        impl price_tools::Config for $runtime {
            type LiquidityProxy = $liquidity_proxy;
            type RuntimeEvent = RuntimeEvent;
            type TradingPairSourceManager = $trading_pair;
            type WeightInfo = $weight;
        }
    };
    ($runtime:ty, $liquidity_proxy:ty) => {
        mock_price_tools_config!(
            $runtime,
            $liquidity_proxy,
            trading_pair::Pallet<$runtime>,
            price_tools::weights::SubstrateWeight<$runtime>
        );
    };
    ($runtime:ty) => {
        mock_price_tools_config!($runtime, ());
    };
}

/// Mock of pallet `proxy::Config`.
#[macro_export]
macro_rules! mock_proxy_config {
    ($runtime:ty) => {
        impl proxy::Config for $runtime {
            type AccountIdConverter = sp_runtime::traits::Identity;
            type FAApp = FungibleApp;
            type HashiBridge = ();
            type LiberlandApp = ();
            type ManagerOrigin = frame_system::EnsureRoot<AccountId>;
            type ParachainApp = ();
            type ReferencePriceProvider = ReferencePriceProvider;
            type RuntimeEvent = RuntimeEvent;
            type TimepointProvider = GenericTimepointProvider;
            type WeightInfo = ();
        }
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
        $buy_back_handler:ty,
        $ensure_dex_manager:ty
    ) => {
        use sp_runtime::Permill;
        frame_support::parameter_types! {
            pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
            pub GetBuyBackFractions: Vec<(AssetId, Permill)> = vec![(common::KUSD.into(), Permill::from_rational(39u32, 100u32)), (common::TBCD.into(), Permill::from_rational(1u32, 100u32))];
        }
        impl pswap_distribution::Config for $runtime {
            const PSWAP_BURN_PERCENT: sp_runtime::Percent = sp_runtime::Percent::from_percent(3);
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type BuyBackHandler = $buy_back_handler;
            type CompatBalance = Balance;
            type DexInfoProvider = dex_manager::Pallet<$runtime>;
            type EnsureDEXManager = $ensure_dex_manager;
            type GetBurnUpdateFrequency = GetBurnUpdateFrequency;
            type GetBuyBackFractions = GetBuyBackFractions;
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
    ($runtime:ty, $xyk_pool:ty, $chameleon_pools:ty, $liquidity_proxy:ty, $buy_back_handler:ty) => {
        mock_pswap_distribution_config!(
            $runtime,
            $xyk_pool,
            $chameleon_pools,
            $liquidity_proxy,
            $buy_back_handler,
            ()
        );
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

/// Mock of pallet `referrals::Config`.
#[macro_export]
macro_rules! mock_referrals_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const ReferralsReservesAcc: AccountId = AccountId::new([22; 32]);
        }
        impl referrals::Config for $runtime {
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type ReservesAcc = ReferralsReservesAcc;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `rewards::Config`.
#[macro_export]
macro_rules! mock_rewards_config {
    ($runtime:ty) => {
        impl rewards::Config for $runtime {
            const BLOCKS_PER_DAY: BlockNumber = 20;
            const MAX_CHUNK_SIZE: usize = 1;
            const MAX_VESTING_RATIO: sp_runtime::Percent = sp_runtime::Percent::from_percent(55);
            const TIME_TO_SATURATION: BlockNumber = 100;
            const UPDATE_FREQUENCY: BlockNumber = 5;
            const VAL_BURN_PERCENT: sp_runtime::Percent = sp_runtime::Percent::from_percent(3);
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `soratopia::Config`.
#[macro_export]
macro_rules! mock_soratopia_config {
    ($runtime:ty) => {
        frame_support::parameter_types! {
            pub const CheckInTransferAmount: Balance = 1_000;
            pub AdminAccount: AccountId = hex_literal::hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48").into();
        }
        impl soratopia::Config for $runtime {
            type AdminAccount = AdminAccount;
            type CheckInTransferAmount = CheckInTransferAmount;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
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
            type DustRemovalWhitelist = frame_support::traits::Everything;
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
    (
        $runtime:ty,
        $max_weight_for_auto_claim:ty,
        $max_vesting_schedules:expr,
        $mix_vested_transfer:expr
    ) => {
        frame_support::parameter_types! {
            pub const MaxVestingSchedules: u32 = $max_vesting_schedules;
            pub const MinVestedTransfer: common::prelude::Balance = $mix_vested_transfer;
        }
        impl vested_rewards::Config for $runtime {
            const BLOCKS_PER_DAY: BlockNumberFor<Self> = 14400;
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type Currency = Currencies;
            type GetBondingCurveRewardsAccountId = GetBondingCurveRewardsAccountId;
            type GetFarmingRewardsAccountId = GetFarmingRewardsAccountId;
            type GetMarketMakerRewardsAccountId = GetMarketMakerRewardsAccountId;
            type MaxVestingSchedules = MaxVestingSchedules;
            type MaxWeightForAutoClaim = $max_weight_for_auto_claim;
            type MinVestedTransfer = MinVestedTransfer;
            type RuntimeEvent = RuntimeEvent;
            type WeightInfo = ();
        }
    };
    ($runtime:ty) => {
        mock_vested_rewards_config!($runtime, (), 0, 0);
    };
}

/// Mock of pallet `xst::Config`.
#[macro_export]
macro_rules! mock_xst_config {
    ($runtime:ty, $price_tool:ty, $oracle:ty, $symbol:ty) => {
        impl xst::Config for $runtime {
            type AssetInfoProvider = assets::Pallet<$runtime>;
            type EnsureDEXManager = dex_manager::Pallet<$runtime>;
            type GetSyntheticBaseAssetId = GetSyntheticBaseAssetId;
            type GetSyntheticBaseBuySellLimit = GetSyntheticBaseBuySellLimit;
            type GetXSTPoolPermissionedTechAccountId = GetXSTPoolPermissionedTechAccountId;
            type Oracle = $oracle;
            type PriceToolsPallet = $price_tool;
            type RuntimeEvent = RuntimeEvent;
            type Symbol = $symbol;
            type TradingPairSourceManager = trading_pair::Pallet<$runtime>;
            type WeightInfo = ();
        }
    };
    ($runtime:ty, $price_tool:ty, $oracle:ty) => {
        mock_xst_config!($runtime, $price_tool, $oracle, common::SymbolName);
    };
    ($runtime:ty, $price_tool:ty) => {
        mock_xst_config!($runtime, $price_tool, OracleProxy);
    };
    ($runtime:ty) => {
        mock_xst_config!($runtime, ());
    };
}
