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
#![allow(dead_code)]
use crate as referrals;
use common::mock::ExistentialDeposits;
use common::prelude::Balance;
use common::{
    mock_assets_config, mock_common_config, mock_currencies_config, mock_frame_system_config,
    mock_pallet_balances_config, mock_permissions_config, mock_referrals_config,
    mock_tokens_config, Amount, AssetId32, AssetName, AssetSymbol, PredefinedAssetId,
    DEFAULT_BALANCE_PRECISION, VAL, XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use sp_core::crypto::AccountId32;
use sp_runtime::{self, Perbill};

type DEXId = common::DEXId;
type AccountId = AccountId32;
type AssetId = AssetId32<PredefinedAssetId>;
type BlockNumber = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub const ALICE: AccountId32 = AccountId32::new([1; 32]);
pub const BOB: AccountId32 = AccountId32::new([2; 32]);
pub const MINTING_ACCOUNT: AccountId = AccountId32::new([4; 32]);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::XOR);
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 4;
}

construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        Tokens: tokens::{Pallet, Call, Storage, Config<T>, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Storage, Config<T>, Event<T>},
        Permissions: permissions::{Pallet, Call, Storage, Config<T>, Event<T>},
        Referrals: referrals::{Pallet, Call, Storage, Config<T>},
    }
);

mock_assets_config!(Runtime);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_permissions_config!(Runtime);
mock_referrals_config!(Runtime);
mock_tokens_config!(Runtime);

parameter_types! {
    pub const GetBuyBackAssetId: common::AssetId32<PredefinedAssetId> = XST;
}

pub fn test_ext() -> sp_io::TestExternalities {
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

    assets::GenesisConfig::<Runtime> {
        endowed_assets: vec![
            (
                VAL,
                ALICE,
                AssetSymbol(b"VAL".to_vec()),
                AssetName(b"SORA Validator Token".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                None,
                None,
            ),
            (
                XOR,
                ALICE,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"XOR".to_vec()),
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

    tokens::GenesisConfig::<Runtime> {
        balances: vec![
            (ALICE, VAL, 100000u128.into()),
            (BOB, VAL, 1000000u128.into()),
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
