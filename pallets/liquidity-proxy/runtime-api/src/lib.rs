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
use common::prelude::OutcomeFee;
#[cfg(feature = "std")]
use common::utils::{fee_serialization, string_serialization};
use common::{BalanceWrapper, RewardReason};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr, Zero};
use sp_std::prelude::*;

#[derive(Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct SwapOutcomeInfo<Balance, AssetId>
where
    AssetId: MaybeDisplay + MaybeFromStr + Ord,
{
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
    pub amount_without_impact: Balance,
    #[cfg_attr(
        feature = "std",
        serde(
            bound(
                serialize = "Balance: std::fmt::Display",
                deserialize = "Balance: std::str::FromStr"
            ),
            with = "fee_serialization"
        )
    )]
    pub fee: OutcomeFee<AssetId, Balance>,
    pub rewards: Vec<RewardsInfo<Balance, AssetId>>,
    pub route: Vec<AssetId>,
}

impl<Balance, AssetId> From<SwapOutcomeInfoV2<Balance, AssetId>>
    for SwapOutcomeInfo<Balance, AssetId>
where
    Balance: Zero + Copy,
    AssetId: MaybeDisplay + MaybeFromStr + Ord + From<common::AssetId32<common::PredefinedAssetId>>,
{
    fn from(value: SwapOutcomeInfoV2<Balance, AssetId>) -> Self {
        Self {
            amount: value.amount,
            amount_without_impact: value.amount_without_impact,
            fee: OutcomeFee::xor(value.fee),
            rewards: value.rewards,
            route: value.route,
        }
    }
}

#[derive(Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct SwapOutcomeInfoV2<Balance, AssetId: MaybeDisplay + MaybeFromStr> {
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
    pub amount_without_impact: Balance,
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
    pub route: Vec<AssetId>,
}

impl<Balance, AssetId> From<SwapOutcomeInfoV1<Balance, AssetId>>
    for SwapOutcomeInfo<Balance, AssetId>
where
    Balance: Default + Zero + Copy,
    AssetId: MaybeDisplay + MaybeFromStr + Ord + From<common::AssetId32<common::PredefinedAssetId>>,
{
    fn from(value: SwapOutcomeInfoV1<Balance, AssetId>) -> Self {
        Self {
            amount: value.amount,
            amount_without_impact: Default::default(),
            fee: OutcomeFee::xor(value.fee),
            rewards: value.rewards,
            route: Default::default(),
        }
    }
}

#[derive(Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct SwapOutcomeInfoV1<Balance, AssetId: MaybeDisplay + MaybeFromStr> {
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
}

#[derive(Eq, PartialEq, Encode, Decode, Default, TypeInfo)]
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
    #[api_version(3)]
    pub trait LiquidityProxyAPI<DEXId, AssetId, Balance, SwapVariant, LiquiditySourceType, FilterMode> where
        DEXId: Codec,
        AssetId: Codec + MaybeFromStr + MaybeDisplay + Ord,
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

        #[changed_in(3)]
        fn quote(
            dex_id: DEXId,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
            amount: BalanceWrapper,
            swap_variant: SwapVariant,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
        ) -> Option<SwapOutcomeInfoV2<Balance, AssetId>>;

        #[changed_in(2)]
        fn quote(
            dex_id: DEXId,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
            amount: BalanceWrapper,
            swap_variant: SwapVariant,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
        ) -> Option<SwapOutcomeInfoV1<Balance, AssetId>>;

        fn is_path_available(
            dex_id: DEXId,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
        ) -> bool;

        fn list_enabled_sources_for_path(
            dex_id: DEXId,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
        ) -> Vec<LiquiditySourceType>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{balance, Balance};

    type AssetId = common::AssetId32<common::PredefinedAssetId>;

    #[test]
    fn should_serialize_and_deserialize_outcome_fee() {
        let info = SwapOutcomeInfo::<Balance, AssetId> {
            fee: OutcomeFee::xor(balance!(0.003)),
            ..Default::default()
        };
        let json_str = r#"{"amount":"0","amount_without_impact":"0","fee":{"0x0200000000000000000000000000000000000000000000000000000000000000":"3000000000000000"},"rewards":[],"route":[]}"#;

        let parsed: SwapOutcomeInfo<Balance, AssetId> = serde_json::from_str(json_str).unwrap();
        assert_eq!(serde_json::to_string(&info).unwrap(), json_str);
        assert_eq!(info, parsed);
        // should not panic
        serde_json::to_value(info).unwrap();

        let info = SwapOutcomeInfo::<Balance, AssetId> {
            fee: OutcomeFee::xor(balance!(0.003)).merge(OutcomeFee::xst(balance!(0.04))),
            ..Default::default()
        };
        let json_str = r#"{"amount":"0","amount_without_impact":"0","fee":{"0x0200000000000000000000000000000000000000000000000000000000000000":"3000000000000000","0x0200090000000000000000000000000000000000000000000000000000000000":"40000000000000000"},"rewards":[],"route":[]}"#;

        let parsed: SwapOutcomeInfo<Balance, AssetId> = serde_json::from_str(json_str).unwrap();
        assert_eq!(serde_json::to_string(&info).unwrap(), json_str);
        assert_eq!(info, parsed);
        // should not panic
        serde_json::to_value(info).unwrap();
    }
}
