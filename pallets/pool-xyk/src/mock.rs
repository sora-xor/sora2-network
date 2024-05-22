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

use crate::{self as pool_xyk, Config};
use common::prelude::{AssetName, AssetSymbol, Balance, Fixed, FromGenericPair, SymbolName};
use common::GetMarketInfo;
use common::{balance, fixed, hash, DEXInfo, PSWAP, TBCD, VAL};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{Everything, GenesisBuild};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use hex_literal::hex;
use orml_traits::parameter_type_with_key;
use permissions::{Scope, MANAGE_DEX};
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::{Perbill, Percent};

pub use common::mock::ComicAssetId::*;
pub use common::mock::*;
pub use common::TechAssetId as Tas;
pub use common::TechPurpose::*;
use frame_system::pallet_prelude::BlockNumberFor;

pub type DEXId = u32;
pub type BlockNumber = u64;
pub type AccountId = AccountId32;
pub type Amount = i128;
pub type TechAssetId = common::TechAssetId<common::mock::ComicAssetId>;
pub type AssetId = common::AssetId32<common::mock::ComicAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type Moment = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub GetBaseAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200000000000000000000000000000000000000000000000000000000000000").into());
    pub GetIncentiveAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200050000000000000000000000000000000000000000000000000000000000").into());
    pub const ExistentialDeposit: u128 = 0;
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::from([8; 32]);
    pub GetFee: Fixed = fixed!(0.003);
    pub const MinimumPeriod: u64 = 5;
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
        0
    };
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
        Tokens: orml_tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Config<T>, Storage, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
        XSTPools: xst::{Pallet, Call, Storage, Event<T>},
        Band: band::{Pallet, Call, Storage, Event<T>},
        OracleProxy: oracle_proxy::{Pallet, Call, Storage, Event<T>},
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

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
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

impl currencies::Config for Runtime {
    type MultiCurrency = orml_tokens::Pallet<Runtime>;
    type NativeCurrency =
        BasicCurrencyAdapter<Runtime, pallet_balances::Pallet<Runtime>, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

parameter_types! {
    pub GetBuyBackAssetId: AssetId = TBCD.into();
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![VAL.into(), PSWAP.into()];
    pub const GetBuyBackPercentage: u8 = 10;
    pub const GetBuyBackAccountId: AccountId = AccountId::new(hex!(
            "0000000000000000000000000000000000000000000000000000000000000023"
    ));
    pub const GetBuyBackDexId: DEXId = 0;
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

impl technical::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = crate::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
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
    type GetChameleonPoolBaseAssetId = GetChameleonPoolBaseAssetId;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl pallet_timestamp::Config for Runtime {
    type Moment = Moment;
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

impl demeter_farming_platform::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DemeterAssetId = ();
    const BLOCKS_PER_HOUR_AND_A_HALF: BlockNumberFor<Self> = 900;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

parameter_types! {
    pub GetXSTPoolPermissionedTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            xst::TECH_ACCOUNT_PREFIX.to_vec(),
            xst::TECH_ACCOUNT_PERMISSIONED.to_vec(),
        );
        tech_account_id
    };
    pub GetSyntheticBaseAssetId: AssetId = BatteryForMusicPlayer.into();
    pub const GetSyntheticBaseBuySellLimit: Balance = balance!(10000000000);
    pub const GetBandRateStalePeriod: Moment = 60*5*1000; // 5 minutes
    pub const GetBandRateStaleBlockPeriod: u64 = 600; // 1 hour
}

impl band::Config for Runtime {
    type Symbol = common::SymbolName;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type OnNewSymbolsRelayedHook = oracle_proxy::Pallet<Runtime>;
    type Time = Timestamp;
    type GetBandRateStalePeriod = GetBandRateStalePeriod;
    type GetBandRateStaleBlockPeriod = GetBandRateStaleBlockPeriod;
    type OnSymbolDisabledHook = ();
    type MaxRelaySymbols = frame_support::traits::ConstU32<100>;
}

impl oracle_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type Symbol = <Runtime as band::Config>::Symbol;
    type BandChainOracle = band::Pallet<Runtime>;
}

impl xst::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type GetSyntheticBaseAssetId = GetSyntheticBaseAssetId;
    type GetXSTPoolPermissionedTechAccountId = GetXSTPoolPermissionedTechAccountId;
    type EnsureDEXManager = DexManager;
    type PriceToolsPallet = ();
    type WeightInfo = ();
    type Oracle = OracleProxy;
    type Symbol = SymbolName;
    type GetSyntheticBaseBuySellLimit = GetSyntheticBaseBuySellLimit;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
}

parameter_type_with_key! {
    pub GetTradingPairRestrictedFlag: |trading_pair: common::TradingPair<AssetId>| -> bool {
        let common::TradingPair {
            base_asset_id,
            target_asset_id
        } = trading_pair;
        <xst::Pallet::<Runtime> as GetMarketInfo<AssetId>>::enabled_target_assets()
            .contains(target_asset_id) ||
            (base_asset_id, target_asset_id) == (&Mango.into(), &GoldenTicket.into()) ||
            (base_asset_id, target_asset_id) == (&Mango.into(), &BatteryForMusicPlayer.into())
    };
}

parameter_type_with_key! {
    pub GetChameleonPoolBaseAssetId: |base_asset_id: AssetId| -> Option<AssetId> {
        if base_asset_id == &GoldenTicket.into() {
            Some(Potato.into())
        } else {
            None
        }
    };
}

parameter_type_with_key! {
    pub GetChameleonPool: |tpair: common::TradingPair<AssetId>| -> bool {
        tpair.base_asset_id == GoldenTicket.into() && tpair.target_asset_id == BlackPepper.into()
    };
}

impl Config for Runtime {
    const MIN_XOR: Balance = balance!(0.007);
    type RuntimeEvent = RuntimeEvent;
    type PairSwapAction = crate::PairSwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction = crate::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        crate::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = crate::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type EnabledSourcesManager = trading_pair::Pallet<Runtime>;
    type GetFee = GetFee;
    type OnPoolCreated = PswapDistribution;
    type OnPoolReservesChanged = ();
    type WeightInfo = ();
    type XSTMarketInfo = xst::Pallet<Runtime>;
    type GetTradingPairRestrictedFlag = GetTradingPairRestrictedFlag;
    type GetChameleonPool = GetChameleonPool;
    type GetChameleonPoolBaseAssetId = GetChameleonPoolBaseAssetId;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

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
    AccountId32::from([35; 32])
}

pub const DEX_A_ID: DEXId = 220;
pub const DEX_B_ID: DEXId = 221;
pub const DEX_C_ID: DEXId = 222;
// XSTPool uses hardcoded DEXId (DEXId::Polkaswap), without this
// DEX XSTPool initializes with error
pub const DEX_D_ID: DEXId = 0;

pub struct ExtBuilder {
    initial_dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    endowed_accounts_for_synthetics_platform:
        Vec<(AccountId, AssetId, Balance, AssetSymbol, AssetName, u8)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    initial_synthetics: Vec<(AssetId, SymbolName, Fixed)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            initial_dex_list: vec![
                (
                    DEX_A_ID,
                    DEXInfo {
                        base_asset_id: GoldenTicket.into(),
                        synthetic_base_asset_id: BatteryForMusicPlayer.into(),
                        is_public: true,
                    },
                ),
                (
                    DEX_B_ID,
                    DEXInfo {
                        base_asset_id: AppleTree.into(),
                        synthetic_base_asset_id: BatteryForMusicPlayer.into(),
                        is_public: true,
                    },
                ),
                (
                    DEX_C_ID,
                    DEXInfo {
                        base_asset_id: Mango.into(),
                        synthetic_base_asset_id: BatteryForMusicPlayer.into(),
                        is_public: true,
                    },
                ),
                (
                    DEX_D_ID,
                    DEXInfo {
                        base_asset_id: GoldenTicket.into(),
                        synthetic_base_asset_id: BatteryForMusicPlayer.into(),
                        is_public: true,
                    },
                ),
            ],
            endowed_accounts: vec![
                (ALICE(), RedPepper.into(), balance!(99000)),
                (ALICE(), BlackPepper.into(), balance!(2000000)),
                (ALICE(), Potato.into(), balance!(2000000)),
                (BOB(), RedPepper.into(), balance!(2000000)),
                (CHARLIE(), BlackPepper.into(), balance!(2000000)),
                (CHARLIE(), Potato.into(), balance!(2000000)),
            ],
            // some assets must be registered (synthetic assets and base synthetic asset)
            // before the initialization of XSTPool
            endowed_accounts_for_synthetics_platform: vec![
                (
                    ALICE(),
                    Mango.into(),
                    balance!(100000),
                    AssetSymbol(b"XSTUSD".to_vec()),
                    AssetName(b"SORA Synthetic USD".to_vec()),
                    common::DEFAULT_BALANCE_PRECISION,
                ),
                (
                    ALICE(),
                    BatteryForMusicPlayer.into(),
                    balance!(10000),
                    AssetSymbol(b"XST".to_vec()),
                    AssetName(b"Sora Synthetics".to_vec()),
                    common::DEFAULT_BALANCE_PRECISION,
                ),
            ],
            initial_permission_owners: vec![(
                MANAGE_DEX,
                Scope::Limited(hash(&DEX_A_ID)),
                vec![BOB()],
            )],
            initial_permissions: vec![(BOB(), Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX])],
            initial_synthetics: vec![(Mango.into(), SymbolName::usd(), fixed!(0))],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        common::test_utils::init_logger();
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
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

        assets::GenesisConfig::<Runtime> {
            endowed_assets: self
                .endowed_accounts_for_synthetics_platform
                .iter()
                .cloned()
                .map(|(account_id, asset_id, _, symbol, name, precision)| {
                    (
                        asset_id,
                        account_id,
                        symbol,
                        name,
                        precision,
                        balance!(0),
                        true,
                        None,
                        None,
                    )
                })
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        xst::GenesisConfig::<Runtime> {
            initial_synthetic_assets: self.initial_synthetics,
            reference_asset_id: Teapot.into(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
