#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::{Codec, Decode, Encode};
#[cfg(feature = "std")]
use common::utils::string_serialization;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_std::prelude::*;

#[derive(Eq, PartialEq, Encode, Decode, Default)]
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

#[derive(Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct AssetInfo<AssetId, AssetSymbol, AssetName, Precision> {
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
}

sp_api::decl_runtime_apis! {
    pub trait AssetsAPI<AccountId, AssetId, Balance, AssetSymbol, AssetName, Precision> where
        AccountId: Codec,
        AssetId: Codec,
        Balance: Codec + MaybeFromStr + MaybeDisplay,
        AssetSymbol: Codec + MaybeFromStr + MaybeDisplay,
        AssetName: Codec + MaybeFromStr + MaybeDisplay,
        Precision: Codec + MaybeFromStr + MaybeDisplay,
    {
        fn free_balance(account_id: AccountId, asset_id: AssetId) -> Option<BalanceInfo<Balance>>;

        fn usable_balance(account_id: AccountId, asset_id: AssetId) -> Option<BalanceInfo<Balance>>;

        fn total_balance(account_id: AccountId, asset_id: AssetId) -> Option<BalanceInfo<Balance>>;

        fn total_supply(asset_id: AssetId) -> Option<BalanceInfo<Balance>>;

        fn list_asset_ids() -> Vec<AssetId>;

        fn list_asset_infos() -> Vec<AssetInfo<AssetId, AssetSymbol, AssetName, Precision>>;

        fn get_asset_info(asset_id: AssetId) -> Option<AssetInfo<AssetId, AssetSymbol, AssetName, Precision>>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::prelude::{
        AssetId32 as ConcrAssetId, AssetName as ConcrAssetName, AssetSymbol as ConcrAssetSymbol,
        BalancePrecision as ConcrBalancePrecision, PredefinedAssetId as ConcrAssetIdUnderlying,
    };

    #[test]
    fn should_serialize_and_deserialize_asset_info_properly_with_string() {
        type AssetInfoTy = AssetInfo<
            ConcrAssetId<ConcrAssetIdUnderlying>,
            ConcrAssetSymbol,
            ConcrAssetName,
            ConcrBalancePrecision,
        >;
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
            precision: 18,
            is_mintable: true,
        };

        let json_str = r#"{"asset_id":"0x020003000400050006000700080009000a000b000c000d000e000f0001000200","symbol":"XOR","name":"SORA","precision":"18","is_mintable":"true"}"#;

        assert_eq!(serde_json::to_string(&asset_info).unwrap(), json_str);
        assert_eq!(
            serde_json::from_str::<AssetInfoTy>(json_str).unwrap(),
            asset_info
        );

        // should not panic
        serde_json::to_value(&asset_info).unwrap();
    }
}
