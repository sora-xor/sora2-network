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

use crate::{self as faucet, Config};
use common::mock::ExistentialDeposits;
use common::prelude::{Balance, FixedWrapper};
use common::{
    self, balance, mock_assets_config, mock_common_config, mock_currencies_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_permissions_config,
    mock_rewards_config, mock_technical_config, mock_tokens_config, Amount, AssetId32, AssetName,
    AssetSymbol, TechPurpose, DEFAULT_BALANCE_PRECISION, USDT, VAL, XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use permissions::{Scope, BURN, MINT};
use sp_core::crypto::AccountId32;
use sp_runtime::Perbill;

type DEXId = common::DEXId;
type AccountId = AccountId32;
type BlockNumber = u64;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
type AssetId = AssetId32<common::PredefinedAssetId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub fn alice() -> AccountId32 {
    AccountId32::from([1u8; 32])
}

pub fn bob() -> AccountId32 {
    AccountId32::from([2u8; 32])
}

pub fn tech_account_id() -> TechAccountId {
    TechAccountId::Pure(
        DEXId::Polkaswap,
        TechPurpose::Identifier(b"faucet_tech_account_id".to_vec()),
    )
}

pub fn account_id() -> AccountId {
    Technical::tech_account_id_to_account_id(&tech_account_id()).unwrap()
}

pub const NOT_SUPPORTED_ASSET_ID: AssetId = USDT;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId = XOR;
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        Faucet: faucet::{Pallet, Call, Config<T>, Storage, Event<T>},
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Rewards: rewards::{Pallet, Event<T>},
    }
}

mock_assets_config!(Runtime);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_permissions_config!(Runtime);
mock_rewards_config!(Runtime);
mock_technical_config!(Runtime);
mock_tokens_config!(Runtime);

impl Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = XST;
}

pub struct ExtBuilder {}

impl ExtBuilder {
    pub fn build() -> sp_io::TestExternalities {
        let mut t = SystemConfig::default().build_storage::<Runtime>().unwrap();

        let tech_account_id = tech_account_id();
        let account_id: AccountId = account_id();

        BalancesConfig {
            balances: vec![
                (
                    account_id.clone(),
                    (faucet::DEFAULT_LIMIT * FixedWrapper::from(1.5)).into_balance(),
                ),
                (alice(), balance!(0)),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        PermissionsConfig {
            initial_permission_owners: vec![
                (MINT, Scope::Unlimited, vec![account_id.clone()]),
                (BURN, Scope::Unlimited, vec![account_id.clone()]),
            ],
            initial_permissions: vec![(account_id.clone(), Scope::Unlimited, vec![MINT, BURN])],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        AssetsConfig {
            endowed_assets: vec![
                (
                    XOR,
                    alice(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    VAL.into(),
                    alice(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"SORA Validator Token".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
            ],
            regulated_assets: Default::default(),
            sbt_assets: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        TokensConfig {
            balances: vec![(
                account_id.clone(),
                VAL.into(),
                (faucet::DEFAULT_LIMIT * FixedWrapper::from(1.5)).into_balance(),
            )],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        TechnicalConfig {
            register_tech_accounts: vec![(account_id, tech_account_id.clone())],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        FaucetConfig {
            reserves_account_id: tech_account_id,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
