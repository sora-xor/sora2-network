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

use crate::*;

#[cfg(feature = "wip")] // dex-kusd
pub type Migrations = (
    assets::migration::register_asset::RegisterAsset<
        Runtime,
        KGOLDAssetId,
        KGOLDAssetName,
        KGOLDAssetSymbol,
        PredefinedAssetOwnerAccountId,
    >,
    assets::migration::register_asset::RegisterAsset<
        Runtime,
        KXORAssetId,
        KXORAssetName,
        KXORAssetSymbol,
        PredefinedAssetOwnerAccountId,
    >,
    assets::migration::register_asset::RegisterAsset<
        Runtime,
        KARMAAssetId,
        KARMAAssetName,
        KARMAAssetSymbol,
        PredefinedAssetOwnerAccountId,
    >,
    kensetsu::migrations::v1_to_v2::UpgradeToV2<Runtime>,
    dex_manager::migrations::kusd_dex::AddKusdBasedDex<Runtime>,
);

#[cfg(not(feature = "wip"))] // dex-kusd
pub type Migrations = (
    assets::migration::register_asset::RegisterAsset<
        Runtime,
        KGOLDAssetId,
        KGOLDAssetName,
        KGOLDAssetSymbol,
        PredefinedAssetOwnerAccountId,
    >,
    assets::migration::register_asset::RegisterAsset<
        Runtime,
        KXORAssetId,
        KXORAssetName,
        KXORAssetSymbol,
        PredefinedAssetOwnerAccountId,
    >,
    assets::migration::register_asset::RegisterAsset<
        Runtime,
        KARMAAssetId,
        KARMAAssetName,
        KARMAAssetSymbol,
        PredefinedAssetOwnerAccountId,
    >,
    kensetsu::migrations::v1_to_v2::UpgradeToV2<Runtime>,
);

parameter_types! {
    pub const MaxMigrations: u32 = 100;
    pub KGOLDAssetId: AssetId = common::KGOLD;
    pub KGOLDAssetSymbol: AssetSymbol = AssetSymbol(b"KGOLD".to_vec());
    pub KGOLDAssetName: AssetName = AssetName(b"Kensetsu ounce of gold".to_vec());
    pub KXORAssetId: AssetId = common::KXOR;
    pub KXORAssetSymbol: AssetSymbol = AssetSymbol(b"KXOR".to_vec());
    pub KXORAssetName: AssetName = AssetName(b"Kensetsu XOR".to_vec());
    pub KARMAAssetId: AssetId = common::KARMA;
    pub KARMAAssetSymbol: AssetSymbol = AssetSymbol(b"KARMA".to_vec());
    pub KARMAAssetName: AssetName = AssetName(b"Chameleon".to_vec());
    pub PredefinedAssetOwnerAccountId: AccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            b"SYSTEM_ACCOUNT".to_vec(),
            b"ASSETS_PERMISSIONS".to_vec(),
        );
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
}
