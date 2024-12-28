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

use crate as trading_pair;
use common::mock::ExistentialDeposits;
use common::prelude::{Balance, DEXInfo};
use common::{
    hash, mock_assets_config, mock_common_config, mock_currencies_config, mock_dex_manager_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_permissions_config,
    mock_tokens_config, mock_trading_pair_config, AssetId32, AssetName, AssetSymbol,
    BalancePrecision, ContentSource, DEXId, Description, DEFAULT_BALANCE_PRECISION, DOT, KSM, KUSD,
    PRUSD, VXOR, XOR, XST, XSTUSD,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::Zero;
use sp_runtime::Perbill;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        TradingPair: trading_pair::{Pallet, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
    }
}

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type Amount = i128;

pub const ALICE: AccountId = AccountId32::new([1; 32]);
pub const DEX_ID: DEXId = DEXId::Polkaswap;
type AssetId = AssetId32<common::PredefinedAssetId>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

mock_assets_config!(Runtime);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_dex_manager_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_permissions_config!(Runtime);
mock_tokens_config!(Runtime);
mock_trading_pair_config!(Runtime);

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = XST;
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

impl ExtBuilder {
    pub fn without_initialized_dex() -> Self {
        Self {
            endowed_assets: vec![
                (
                    XOR,
                    ALICE,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    DOT,
                    ALICE,
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"Polkadot".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    KSM,
                    ALICE,
                    AssetSymbol(b"KSM".to_vec()),
                    AssetName(b"Kusama".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
            ],
            endowed_accounts: vec![],
            dex_list: vec![],
            initial_permission_owners: vec![],
            initial_permissions: vec![],
        }
    }
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_assets: vec![
                (
                    XOR,
                    ALICE,
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
                    ALICE,
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
                    ALICE,
                    AssetSymbol(b"KSM".to_vec()),
                    AssetName(b"Kusama".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    XSTUSD,
                    ALICE,
                    AssetSymbol(b"XSTUSD".to_vec()),
                    AssetName(b"XSTUSD".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    KUSD.into(),
                    ALICE,
                    AssetSymbol(b"KUSD".to_vec()),
                    AssetName(b"Kensetsu Stable Dollar".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    VXOR,
                    ALICE,
                    AssetSymbol(b"VXOR".to_vec()),
                    AssetName(b"Vested XOR".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    PRUSD,
                    ALICE,
                    AssetSymbol(b"PRUSD".to_vec()),
                    AssetName(b"Presto USD".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
            ],
            endowed_accounts: vec![],
            dex_list: vec![
                (
                    DEX_ID,
                    DEXInfo {
                        base_asset_id: XOR,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
                (
                    DEXId::PolkaswapXSTUSD,
                    DEXInfo {
                        base_asset_id: XSTUSD,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
                (
                    DEXId::PolkaswapKUSD,
                    DEXInfo {
                        base_asset_id: KUSD,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
                (
                    DEXId::PolkaswapVXOR,
                    DEXInfo {
                        base_asset_id: VXOR,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
                (
                    DEXId::PolkaswapPresto,
                    DEXInfo {
                        base_asset_id: PRUSD,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
            ],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![ALICE]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_ID)), vec![ALICE]),
            ],
            initial_permissions: vec![
                (ALICE, Scope::Unlimited, vec![INIT_DEX]),
                (ALICE, Scope::Limited(hash(&DEX_ID)), vec![MANAGE_DEX]),
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
            balances: vec![(ALICE, 0)],
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
