#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]

use codec::{Codec, Decode, Encode};
#[cfg(feature = "std")]
use common::utils::string_serialization;
use common::{BalanceWrapper, RewardReason};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_std::prelude::*;

#[derive(Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct SwapOutcomeInfo<Balance, AssetId: MaybeDisplay + MaybeFromStr> {
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
    pub amount: Balance,
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
    pub fee: Balance,
    pub rewards: Vec<RewardsInfo<Balance, AssetId>>,
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
    pub amount_without_impact: Balance,
}

#[derive(Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct RewardsInfo<Balance, AssetId> {
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
    pub amount: Balance,
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
    pub currency: AssetId,
    pub reason: RewardReason,
}

sp_api::decl_runtime_apis! {
    pub trait LiquidityProxyAPI<DEXId, AssetId, Balance, SwapVariant, LiquiditySourceType, FilterMode> where
        DEXId: Codec,
        AssetId: Codec + MaybeFromStr + MaybeDisplay,
        Balance: Codec + MaybeFromStr + MaybeDisplay,
        SwapVariant: Codec,
        LiquiditySourceType: Codec,
        FilterMode: Codec,
    {
        fn quote(
            dex_id: DEXId,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
            amount: BalanceWrapper,
            swap_variant: SwapVariant,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
        ) -> Option<SwapOutcomeInfo<Balance, AssetId>>;

        fn is_path_available(
            dex_id: DEXId,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
        ) -> bool;
    }
}
