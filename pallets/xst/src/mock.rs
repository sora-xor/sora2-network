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

use crate::{self as xst, Config};
use common::mock::ExistentialDeposits;
use common::prelude::{Balance, PriceToolsProvider};
use common::{
    self, balance, hash, mock_assets_config, mock_band_config, mock_ceres_liquidity_locker_config,
    mock_common_config, mock_currencies_config, mock_demeter_farming_platform_config,
    mock_dex_api_config, mock_dex_manager_config, mock_frame_system_config,
    mock_liquidity_source_config, mock_oracle_proxy_config, mock_pallet_balances_config,
    mock_pallet_timestamp_config, mock_permissions_config, mock_pool_xyk_config,
    mock_price_tools_config, mock_pswap_distribution_config, mock_technical_config,
    mock_tokens_config, mock_trading_pair_config, mock_xst_config, Amount, AssetId32, AssetIdOf,
    AssetName, AssetSymbol, DEXInfo, FromGenericPair, PriceVariant, DAI, DEFAULT_BALANCE_PRECISION,
    PSWAP, USDT, VAL, VXOR, XOR, XST, XSTUSD,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system::pallet_prelude::BlockNumberFor;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::Zero;
use sp_runtime::Perbill;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
type DEXId = common::DEXId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub fn alice() -> AccountId {
    AccountId32::from([1u8; 32])
}

pub fn bob() -> AccountId {
    AccountId32::from([2u8; 32])
}

pub fn assets_owner() -> AccountId {
    AccountId32::from([3u8; 32])
}

pub const DEX_A_ID: DEXId = DEXId::Polkaswap;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetDefaultFee: u16 = 30;
    pub const GetDefaultProtocolFee: u16 = 0;
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetSyntheticBaseAssetId: AssetId = XST;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
    pub const GetNumSamples: usize = 40;
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([151; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::from([152; 32]);
    pub GetXSTPoolPermissionedTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            crate::TECH_ACCOUNT_PREFIX.to_vec(),
            crate::TECH_ACCOUNT_PERMISSIONED.to_vec(),
        );
        tech_account_id
    };
    pub GetXSTPoolPermissionedAccountId: AccountId = {
        let tech_account_id = GetXSTPoolPermissionedTechAccountId::get();
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub const GetSyntheticBaseBuySellLimit: Balance = balance!(10000000);
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Storage},
        TradingPair: trading_pair::{Pallet, Call, Storage, Event<T>},
        MockLiquiditySource: mock_liquidity_source::<Instance1>::{Pallet, Call, Config<T>, Storage},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        XSTPool: xst::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Storage, Event<T>},
        DEXApi: dex_api::{Pallet, Call, Storage, Config, Event<T>},
        Band: band::{Pallet, Call, Storage, Event<T>},
        OracleProxy: oracle_proxy::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
        PriceTools: price_tools::{Pallet, Storage, Event<T>},
    }
}

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = VXOR;
}

mock_assets_config!(Runtime);
mock_band_config!(
    Runtime,
    oracle_proxy::Pallet<Runtime>,
    crate::Pallet<Runtime>
);
mock_ceres_liquidity_locker_config!(Runtime, PoolXYK);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_demeter_farming_platform_config!(Runtime);
mock_dex_api_config!(Runtime, (), MockLiquiditySource, XSTPool);
mock_dex_manager_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_liquidity_source_config!(Runtime, mock_liquidity_source::Instance1);
mock_oracle_proxy_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);
mock_permissions_config!(Runtime);
mock_pool_xyk_config!(Runtime);
mock_price_tools_config!(Runtime);
mock_pswap_distribution_config!(Runtime, PoolXYK);
mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);
mock_tokens_config!(Runtime);
mock_trading_pair_config!(Runtime);
mock_xst_config!(
    Runtime,
    price_tools::Pallet<Runtime>,
    oracle_proxy::Pallet<Runtime>,
    <Runtime as band::Config>::Symbol
);

pub fn set_mock_price<T: Config>(asset_id: &AssetId, price_buy: Balance, price_sell: Balance)
where
    T: price_tools::Config,
{
    let asset_id_to_register = AssetIdOf::<T>::from(*asset_id);
    let _ = price_tools::Pallet::<T>::register_asset(&asset_id_to_register);

    for _ in 0..31 {
        price_tools::PriceInfos::<T>::mutate(&asset_id_to_register, |opt_val| {
            let val = opt_val.as_mut().unwrap();
            val.price_mut_of(PriceVariant::Buy)
                .incoming_spot_price(
                    price_buy,
                    PriceVariant::Buy,
                    &price_tools::DEFAULT_PARAMETERS,
                )
                .expect("Failed to relay spot price");
            val.price_mut_of(PriceVariant::Sell)
                .incoming_spot_price(
                    price_sell,
                    PriceVariant::Sell,
                    &price_tools::DEFAULT_PARAMETERS,
                )
                .expect("Failed to relay spot price");
        })
    }
}

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance, AssetSymbol, AssetName, u8)>,
    endowed_accounts_with_synthetics:
        Vec<(AccountId, AssetId, Balance, AssetSymbol, AssetName, u8)>,
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    // Vec<(AssetId, Price, Buy fee ratio, Sell fee ratio)>
    initial_prices: Vec<(AssetId, Balance, Balance)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![
                (
                    alice(),
                    USDT,
                    0,
                    AssetSymbol(b"USDT".to_vec()),
                    AssetName(b"Tether USD".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    XOR,
                    balance!(350000),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    VAL,
                    balance!(500000),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"SORA Validator Token".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    PSWAP,
                    balance!(0),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"Polkaswap Token".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    XST,
                    balance!(250000),
                    AssetSymbol(b"XST".to_vec()),
                    AssetName(b"Sora Synthetics".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
                (
                    alice(),
                    DAI,
                    balance!(100000),
                    AssetSymbol(b"DAI".to_vec()),
                    AssetName(b"DAI".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                ),
            ],
            endowed_accounts_with_synthetics: vec![(
                alice(),
                XSTUSD,
                balance!(100000),
                AssetSymbol(b"XSTUSD".to_vec()),
                AssetName(b"SORA Synthetic USD".to_vec()),
                DEFAULT_BALANCE_PRECISION,
            )],
            dex_list: vec![(
                DEX_A_ID,
                DEXInfo {
                    base_asset_id: GetBaseAssetId::get(),
                    synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
                    is_public: true,
                },
            )],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![alice()]),
            ],
            initial_permissions: vec![
                (alice(), Scope::Unlimited, vec![INIT_DEX]),
                (alice(), Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
                (
                    assets_owner(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
                (
                    GetXSTPoolPermissionedAccountId::get(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
            ],
            initial_prices: vec![
                (XST, balance!(0.6), balance!(0.5)),
                (DAI, balance!(110), balance!(90)),
                (USDT, balance!(200), balance!(190)),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn new(
        endowed_accounts: Vec<(AccountId, AssetId, Balance, AssetSymbol, AssetName, u8)>,
        endowed_accounts_with_synthetics: Vec<(
            AccountId,
            AssetId,
            Balance,
            AssetSymbol,
            AssetName,
            u8,
        )>,
    ) -> Self {
        Self {
            endowed_accounts,
            endowed_accounts_with_synthetics,
            ..Default::default()
        }
    }

    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: self
                .endowed_accounts
                .iter()
                .cloned()
                .filter_map(|(account_id, asset_id, balance, ..)| {
                    if asset_id == GetBaseAssetId::get() {
                        Some((account_id, balance))
                    } else {
                        None
                    }
                })
                .chain(vec![(bob(), 0), (assets_owner(), 0)])
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        technical::GenesisConfig::<Runtime> {
            register_tech_accounts: vec![(
                GetXSTPoolPermissionedAccountId::get(),
                GetXSTPoolPermissionedTechAccountId::get(),
            )],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.dex_list,
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
                .endowed_accounts
                .iter()
                .cloned()
                .chain(self.endowed_accounts_with_synthetics.iter().cloned())
                .map(|(account_id, asset_id, _, symbol, name, precision)| {
                    (
                        asset_id,
                        account_id,
                        symbol,
                        name,
                        precision,
                        Balance::zero(),
                        true,
                        None,
                        None,
                    )
                })
                .collect(),
            regulated_assets: Default::default(),
            sbt_assets: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        crate::GenesisConfig::<Runtime>::default()
            .assimilate_storage(&mut t)
            .unwrap();

        tokens::GenesisConfig::<Runtime> {
            balances: self
                .endowed_accounts
                .into_iter()
                .chain(self.endowed_accounts_with_synthetics.into_iter())
                .map(|(account_id, asset_id, balance, ..)| (account_id, asset_id, balance))
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let mut ext: sp_io::TestExternalities = t.into();
        ext.execute_with(|| {
            self.initial_prices
                .into_iter()
                .for_each(|(asset_id, price_buy, price_sell)| {
                    set_mock_price::<Runtime>(&asset_id, price_buy, price_sell)
                })
        });
        ext
    }
}
