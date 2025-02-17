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

use common::mock::ExistentialDeposits;
use common::prelude::Balance;
use common::{
    balance, fixed, mock_assets_config, mock_ceres_liquidity_locker_config, mock_common_config,
    mock_currencies_config, mock_demeter_farming_platform_config, mock_dex_manager_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_pallet_timestamp_config,
    mock_permissions_config, mock_pool_xyk_config, mock_pswap_distribution_config,
    mock_technical_config, mock_tokens_config, mock_trading_pair_config, AssetName, AssetSymbol,
    BalancePrecision, ContentSource, Description, Fixed, FromGenericPair,
    DEFAULT_BALANCE_PRECISION, PSWAP, VXOR, XOR,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use hex_literal::hex;
use permissions::Scope;
use sp_runtime::traits::Zero;
use sp_runtime::{AccountId32, Perbill};
use sp_std::vec;

use crate::Config;
use frame_system::pallet_prelude::BlockNumberFor;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type Amount = i128;
pub type AssetId = common::AssetId32<common::PredefinedAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
type DEXId = common::DEXId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub fn alice() -> AccountId {
    AccountId32::from([1u8; 32])
}

pub fn fees_account_a() -> AccountId {
    AccountId32::from([2u8; 32])
}

pub fn fees_account_b() -> AccountId {
    AccountId32::from([3u8; 32])
}

pub fn pool_account_a() -> AccountId {
    AccountId32::from([11u8; 32])
}

pub fn pool_account_b() -> AccountId {
    AccountId32::from([12u8; 32])
}

pub const DEX_A_ID: DEXId = common::DEXId::Polkaswap;

parameter_types! {
    pub GetBaseAssetId: AssetId = XOR.into();
    pub const PoolTokenAId: AssetId = common::AssetId32::from_bytes(hex!("0211110000000000000000000000000000000000000000000000000000000000"));
    pub const PoolTokenBId: AssetId = common::AssetId32::from_bytes(hex!("0222220000000000000000000000000000000000000000000000000000000000"));
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 1);
    pub const MaximumBlockLength: u32 = 1024 * 2;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetDefaultFee: u16 = 30;
    pub const GetDefaultProtocolFee: u16 = 0;
    pub GetPswapDistributionTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            pswap_distribution::TECH_ACCOUNT_PREFIX.to_vec(),
            pswap_distribution::TECH_ACCOUNT_MAIN.to_vec(),
        );
        tech_account_id
    };
    pub GetPswapDistributionAccountId: AccountId = {
        let tech_account_id = GetPswapDistributionTechAccountId::get();
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 3;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
    pub GetParliamentAccountId: AccountId = AccountId32::from([7u8; 32]);
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Storage},
        TradingPair: trading_pair::{Pallet, Call, Config<T>, Storage, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
    }
}

impl Config for Runtime {}

mock_assets_config!(Runtime);
mock_ceres_liquidity_locker_config!(Runtime, PoolXYK);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_demeter_farming_platform_config!(Runtime);
mock_dex_manager_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);
mock_permissions_config!(Runtime);
mock_pswap_distribution_config!(Runtime, PoolXYK);
mock_pool_xyk_config!(Runtime);
mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);
mock_tokens_config!(Runtime);
mock_trading_pair_config!(Runtime);

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = VXOR;
}

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
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
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    subscribed_accounts: Vec<(AccountId, (DEXId, AccountId, BlockNumber, BlockNumber))>,
    burn_info: (Fixed, Fixed, Fixed),
}

impl ExtBuilder {
    pub fn with_accounts(accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
        let permissioned_account_id = GetPswapDistributionAccountId::get();
        Self {
            endowed_accounts: accounts,
            endowed_assets: vec![
                (
                    XOR.into(),
                    alice(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    PSWAP.into(),
                    alice(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"Polkaswap".to_vec()),
                    10,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    PoolTokenAId::get(),
                    alice(),
                    AssetSymbol(b"POOLA".to_vec()),
                    AssetName(b"Pool A".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    PoolTokenBId::get(),
                    alice(),
                    AssetSymbol(b"POOLB".to_vec()),
                    AssetName(b"Pool B".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
            ],
            initial_permission_owners: vec![],
            initial_permissions: vec![(
                permissioned_account_id,
                Scope::Unlimited,
                vec![permissions::MINT, permissions::BURN],
            )],
            subscribed_accounts: vec![
                (fees_account_a(), (DEX_A_ID, pool_account_a(), 5, 0)),
                (fees_account_b(), (DEX_A_ID, pool_account_b(), 7, 0)),
            ],
            burn_info: (fixed!(0.1), fixed!(0.10), fixed!(0.40)),
        }
    }
}

impl Default for ExtBuilder {
    fn default() -> Self {
        ExtBuilder::with_accounts(vec![
            (fees_account_a(), XOR.into(), balance!(1)),
            (fees_account_a(), PSWAP.into(), balance!(6)),
        ])
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = SystemConfig::default().build_storage::<Runtime>().unwrap();

        let mut vec = self
            .endowed_accounts
            .iter()
            .map(|(acc, ..)| (acc.clone(), 0))
            .chain(vec![
                (alice(), 0),
                (fees_account_a(), 0),
                (fees_account_b(), 0),
                (GetPswapDistributionAccountId::get(), 0),
                (GetParliamentAccountId::get(), 0),
            ])
            .collect::<Vec<_>>();

        vec.sort_by_key(|x| x.0.clone());
        vec.dedup_by(|x, y| x.0 == y.0);
        BalancesConfig { balances: vec }
            .assimilate_storage(&mut t)
            .unwrap();

        PermissionsConfig {
            initial_permissions: self.initial_permissions,
            initial_permission_owners: self.initial_permission_owners,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        TokensConfig {
            balances: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        AssetsConfig {
            endowed_assets: self.endowed_assets,
            regulated_assets: Default::default(),
            sbt_assets: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        PswapDistributionConfig {
            subscribed_accounts: self.subscribed_accounts,
            burn_info: self.burn_info,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
