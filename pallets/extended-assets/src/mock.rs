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

use crate as extended_assets;
use common::mock::ExistentialDeposits;
use common::{
    mock_assets_config, mock_common_config, mock_currencies_config, mock_extended_assets_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_pallet_timestamp_config,
    mock_permissions_config, mock_technical_config, mock_tokens_config, Amount, AssetId32, DEXId,
    PredefinedAssetId, XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::ConstU32;
use frame_support::{construct_runtime, parameter_types};
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::MultiSignature;

type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
type AssetId = AssetId32<PredefinedAssetId>;
type Balance = u128;
type Signature = MultiSignature;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type BlockNumber = u64;

mock_assets_config!(
    TestRuntime,
    (
        extended_assets::Pallet<TestRuntime>,
        permissions::Pallet<TestRuntime>,
    )
);
mock_common_config!(TestRuntime);
mock_currencies_config!(TestRuntime);
mock_extended_assets_config!(TestRuntime);
mock_frame_system_config!(TestRuntime);
mock_pallet_balances_config!(TestRuntime);
mock_pallet_timestamp_config!(TestRuntime);
mock_permissions_config!(TestRuntime);
mock_technical_config!(TestRuntime);
mock_tokens_config!(TestRuntime);

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = XST;
}

construct_runtime! {
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Storage, Config<T>, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        ExtendedAssets: extended_assets::{Pallet, Storage, Event<T>, Call},
        Technical: technical::{Pallet, Call, Config<T>, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
    }
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap()
        .into()
}
