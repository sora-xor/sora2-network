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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::{Codec, Decode, Encode};
#[cfg(feature = "std")]
use common::utils::{string_serialization, string_serialization_opt};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_std::prelude::*;

#[derive(Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct BalanceInfo<Balance> {
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub balance: Balance,
}

#[derive(Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct AssetInfo<AssetId, AssetSymbol, AssetName, Precision, ContentSource, Description> {
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "AssetId: std::fmt::Display",
                deserialize = "AssetId: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub asset_id: AssetId,

    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "AssetSymbol: std::fmt::Display",
                deserialize = "AssetSymbol: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub symbol: AssetSymbol,

    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "AssetName: std::fmt::Display",
                deserialize = "AssetName: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub name: AssetName,

    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Precision: std::fmt::Display",
                deserialize = "Precision: std::str::FromStr"
            ),
            with = "string_serialization"
        )
    )]
    pub precision: Precision,

    #[cfg_attr(feature = "std", serde(with = "string_serialization"))]
    pub is_mintable: bool,

    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "ContentSource: std::fmt::Display",
                deserialize = "ContentSource: std::str::FromStr"
            ),
            with = "string_serialization_opt"
        ),
        serde(default)
    )]
    pub content_source: Option<ContentSource>,

    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Description: std::fmt::Display",
                deserialize = "Description: std::str::FromStr"
            ),
            with = "string_serialization_opt"
        ),
        serde(default)
    )]
    pub description: Option<Description>,
}

sp_api::decl_runtime_apis! {
    pub trait AssetsAPI<AccountId, AssetId, Balance, AssetSymbol, AssetName, Precision, ContentSource, Description> where
        AccountId: Codec,
        AssetId: Codec,
        Balance: Codec + MaybeFromStr + MaybeDisplay,
        AssetSymbol: Codec + MaybeFromStr + MaybeDisplay,
        AssetName: Codec + MaybeFromStr + MaybeDisplay,
        Precision: Codec + MaybeFromStr + MaybeDisplay,
        ContentSource: Codec + MaybeFromStr + MaybeDisplay,
        Description: Codec + MaybeFromStr + MaybeDisplay,
    {
        fn free_balance(account_id: AccountId, asset_id: AssetId) -> Option<BalanceInfo<Balance>>;

        fn usable_balance(account_id: AccountId, asset_id: AssetId) -> Option<BalanceInfo<Balance>>;

        fn total_balance(account_id: AccountId, asset_id: AssetId) -> Option<BalanceInfo<Balance>>;

        fn total_supply(asset_id: AssetId) -> Option<BalanceInfo<Balance>>;

        fn list_asset_ids() -> Vec<AssetId>;

        fn list_asset_infos() -> Vec<AssetInfo<AssetId, AssetSymbol, AssetName, Precision, ContentSource, Description>>;

        fn get_asset_info(asset_id: AssetId) -> Option<AssetInfo<AssetId, AssetSymbol, AssetName, Precision, ContentSource, Description>>;

        fn get_asset_content_src(asset_id: AssetId) -> Option<ContentSource>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::prelude::{
        AssetId32 as ConcrAssetId, AssetName as ConcrAssetName, AssetSymbol as ConcrAssetSymbol,
        BalancePrecision as ConcrBalancePrecision, ContentSource, Description,
        PredefinedAssetId as ConcrAssetIdUnderlying,
    };
    use common::DEFAULT_BALANCE_PRECISION;

    type AssetInfoTy = AssetInfo<
        ConcrAssetId<ConcrAssetIdUnderlying>,
        ConcrAssetSymbol,
        ConcrAssetName,
        ConcrBalancePrecision,
        ContentSource,
        Description,
    >;

    #[test]
    fn should_serialize_and_deserialize_asset_info_properly_with_string() {
        let asset_info = AssetInfoTy {
            asset_id: ConcrAssetId {
                code: [
                    2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 0, 10, 0, 11, 0, 12, 0, 13, 0, 14,
                    0, 15, 0, 1, 0, 2, 0,
                ],
                phantom: Default::default(),
            },
            symbol: ConcrAssetSymbol(b"XOR".to_vec()),
            name: ConcrAssetName(b"SORA".to_vec()),
            precision: DEFAULT_BALANCE_PRECISION,
            is_mintable: true,
            content_source: Some(ContentSource(b"none".to_vec())),
            description: Some(Description(b"none".to_vec())),
        };

        let json_str = r#"{"asset_id":"0x020003000400050006000700080009000a000b000c000d000e000f0001000200","symbol":"XOR","name":"SORA","precision":"18","is_mintable":"true","content_source":"none","description":"none"}"#;

        assert_eq!(serde_json::to_string(&asset_info).unwrap(), json_str);
        assert_eq!(
            serde_json::from_str::<AssetInfoTy>(json_str).unwrap(),
            asset_info
        );

        // should not panic
        serde_json::to_value(&asset_info).unwrap();
    }

    #[test]
    fn should_serialize_and_deserialize_asset_info_properly_with_string_2() {
        let asset_info = AssetInfoTy {
            asset_id: ConcrAssetId {
                code: [
                    2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 0, 10, 0, 11, 0, 12, 0, 13, 0, 14,
                    0, 15, 0, 1, 0, 2, 0,
                ],
                phantom: Default::default(),
            },
            symbol: ConcrAssetSymbol(b"XOR".to_vec()),
            name: ConcrAssetName(b"SORA".to_vec()),
            precision: DEFAULT_BALANCE_PRECISION,
            is_mintable: true,
            content_source: None,
            description: None,
        };

        let json_str = r#"{"asset_id":"0x020003000400050006000700080009000a000b000c000d000e000f0001000200","symbol":"XOR","name":"SORA","precision":"18","is_mintable":"true","content_source":null,"description":null}"#;

        assert_eq!(serde_json::to_string(&asset_info).unwrap(), json_str);
        assert_eq!(
            serde_json::from_str::<AssetInfoTy>(json_str).unwrap(),
            asset_info
        );

        // should not panic
        serde_json::to_value(&asset_info).unwrap();
    }
}
