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

use crate as iroha_migration; // for construct_runtime
use crate::{Config, TECH_ACCOUNT_MAIN, TECH_ACCOUNT_PREFIX};
use common::mock::ExistentialDeposits;
use common::prelude::Balance;
use common::{
    balance, mock_assets_config, mock_common_config, mock_currencies_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_permissions_config,
    mock_technical_config, mock_tokens_config, Amount, AssetId32, AssetName, AssetSymbol,
    PredefinedAssetId, DEFAULT_BALANCE_PRECISION, VAL, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{Everything, GenesisBuild};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use permissions::{Scope, MINT};
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::{self, Perbill};

type DEXId = common::DEXId;
type AccountId = AccountId32;
type AssetId = AssetId32<PredefinedAssetId>;
type BlockNumber = u64;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<PredefinedAssetId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub const XOR: PredefinedAssetId = PredefinedAssetId::XOR;
pub const ALICE: AccountId = AccountId::new([1; 32]);
pub const BOB: AccountId = AccountId::new([2; 32]);
pub const CHARLIE: AccountId = AccountId::new([3; 32]);
pub const MINTING_ACCOUNT: AccountId = AccountId::new([4; 32]);
pub const REFERRALS_RESERVES_ACC: AccountId = AccountId::new([22; 32]);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(XOR);
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 4;
    pub const ReferralsReservesAcc: AccountId = REFERRALS_RESERVES_ACC;
}

construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Storage, Config<T>, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Storage, Config<T>, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Event<T>},
        Permissions: permissions::{Pallet, Call, Storage, Config<T>, Event<T>},
        Referrals: referrals::{Pallet, Call, Storage, Config<T>},
        IrohaMigration: iroha_migration::{Pallet, Call, Storage, Config<T>, Event<T>}
    }
);

mock_technical_config!(Runtime);
// Required by assets::Config
mock_currencies_config!(Runtime);
// Required by currencies::Config
mock_pallet_balances_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_common_config!(Runtime);
mock_tokens_config!(Runtime);
mock_assets_config!(Runtime);
mock_permissions_config!(Runtime);

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = XST;
}

impl referrals::Config for Runtime {
    type ReservesAcc = ReferralsReservesAcc;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl pallet_multisig::Config for Runtime {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

impl Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

pub fn test_ext(add_iroha_accounts: bool) -> sp_io::TestExternalities {
    let tech_account_id =
        TechAccountId::Generic(TECH_ACCOUNT_PREFIX.to_vec(), TECH_ACCOUNT_MAIN.to_vec());

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    pallet_balances::GenesisConfig::<Runtime> {
        balances: vec![
            (ALICE, 0u128.into()),
            (BOB, 0u128.into()),
            (MINTING_ACCOUNT, 0u128.into()),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    permissions::GenesisConfig::<Runtime> {
        initial_permission_owners: vec![(MINT, Scope::Unlimited, vec![MINTING_ACCOUNT])],
        initial_permissions: vec![(MINTING_ACCOUNT, Scope::Unlimited, vec![MINT])],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    assets::GenesisConfig::<Runtime> {
        endowed_assets: vec![(
            VAL,
            ALICE,
            AssetSymbol(b"VAL".to_vec()),
            AssetName(b"SORA Validator Token".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        )],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let eth_bridge_tech_account_id = TechAccountId::Generic(
        eth_bridge::TECH_ACCOUNT_PREFIX.to_vec(),
        eth_bridge::TECH_ACCOUNT_MAIN.to_vec(),
    );
    let eth_bridge_account_id =
        Technical::tech_account_id_to_account_id(&eth_bridge_tech_account_id).unwrap();

    tokens::GenesisConfig::<Runtime> {
        balances: vec![
            (ALICE, VAL, 0u128.into()),
            (eth_bridge_account_id.clone(), VAL, balance!(1000)),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    technical::GenesisConfig::<Runtime> {
        register_tech_accounts: vec![
            (MINTING_ACCOUNT, tech_account_id.clone()),
            (eth_bridge_account_id, eth_bridge_tech_account_id),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let iroha_accounts = if add_iroha_accounts {
        vec![
            (
                "did_sora_d9bda3688c6f608ab15c@sora".to_string(),
                Balance::from(0u128),
                None,
                1,
                vec![
                    "D9BDA3688c6f608ab15c03a55b171da0413788a40a25722b4ae4d3672890bcd7".to_string(),
                ],
            ),
            (
                "did_sora_balance@sora".to_string(),
                Balance::from(300u128),
                None,
                1,
                vec![
                    "9A685d77BCd3f60e6cc1e91eedc7a48e11bbcf1a036b920f3bae0372a78a5432".to_string(),
                ],
            ),
            (
                "did_sora_referral@sora".to_string(),
                Balance::from(0u128),
                Some("did_sora_referrer@sora".to_string()),
                1,
                vec![
                    "cba1c8c2eeaf287d734bd167b10d762e89c0ee8327a29e04f064ae94086ef1e9".to_string(),
                ],
            ),
            (
                "did_sora_referrer@sora".to_string(),
                Balance::from(0u128),
                None,
                1,
                vec![
                    "dd54e9efb95531154316cf3e28e2232abab349296dde94353febc9ebbb3ff283".to_string(),
                ],
            ),
            (
                "did_sora_multi_sig@sora".to_string(),
                Balance::from(1000u128),
                None,
                2,
                vec![
                    "f7d89d39d48a67e4741a612de10650234f9148e84fe9e8b2a9fad322b0d8e5bc".to_string(),
                    "f56b4880ed91a25b257144acab749f615855c4b1b6a5d7891e1a6cdd9fd695e9".to_string(),
                    "57571ec82cff710143eba60c05d88de14a22799048137162d63c534a8b02dc20".to_string(),
                ],
            ),
        ]
    } else {
        Vec::new()
    };

    IrohaMigrationConfig {
        iroha_accounts,
        account_id: Some(MINTING_ACCOUNT),
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    test_ext(true)
}
