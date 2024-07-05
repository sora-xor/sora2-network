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

use crate::{self as market};
use common::mock::ExistentialDeposits;
use common::{
    mock_common_config, mock_currencies_config, mock_frame_system_config,
    mock_pallet_balances_config, mock_permissions_config, mock_technical_config,
    mock_tokens_config, Amount, AssetId32, DEXId, LiquiditySourceType, PredefinedAssetId, XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::parameter_types;
use frame_support::traits::{Everything, OnInitialize};
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};

use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::MultiSignature;

pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
pub type AssetId = AssetId32<PredefinedAssetId>;
pub type Balance = u128;
pub type Signature = MultiSignature;
pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
pub type Block = frame_system::mocking::MockBlock<Runtime>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type BlockNumber = u64;

mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_tokens_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_permissions_config!(Runtime);
mock_technical_config!(Runtime);

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system,
        Assets: assets,
        Balances: pallet_balances,
        Tokens: tokens,
        Permissions: permissions,
        Technical: technical,
        Market: market,
    }
);

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = XST;
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![];
    pub const GetBuyBackPercentage: u8 = 0;
    pub const GetBuyBackAccountId: AccountId = AccountId::new(hex!(
            "0000000000000000000000000000000000000000000000000000000000000023"
    ));
    pub const GetBuyBackDexId: DEXId = DEXId::Polkaswap;
}

impl assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<DEXId, LiquiditySourceType, [u8; 32]>;
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
    type AssetRegulator = market::Pallet<Runtime>;
}

impl crate::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut ext: sp_io::TestExternalities = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap()
        .into();
    ext.execute_with(|| {
        System::set_block_number(1); // No events in zero block
        System::inc_providers(&common::mock::alice());
        System::inc_providers(&common::mock::bob());
    });
    ext
}

pub fn run_to_block(n: BlockNumber) {
    while System::block_number() < n {
        let block_number = System::block_number() + 1;
        System::set_block_number(block_number);
        Market::on_initialize(block_number);
    }
}
