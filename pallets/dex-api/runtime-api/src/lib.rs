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
use common::BalanceWrapper;
#[cfg(feature = "std")]
use serde::de::DeserializeOwned;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr, Zero};
use sp_std::prelude::*;

#[derive(Eq, PartialEq, Encode, Decode, Default)]
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
                deserialize = "AssetId: DeserializeOwned, Balance: DeserializeOwned + std::str::FromStr"
            ),
            with = "fee_serialization"
        )
    )]
    pub fee: OutcomeFee<AssetId, Balance>,
}

impl<Balance, AssetId> From<SwapOutcomeInfoV1<Balance>> for SwapOutcomeInfo<Balance, AssetId>
where
    Balance: Copy + Zero,
    AssetId: MaybeDisplay + MaybeFromStr + Ord + From<common::AssetId32<common::PredefinedAssetId>>,
{
    fn from(value: SwapOutcomeInfoV1<Balance>) -> Self {
        Self {
            amount: value.amount,
            fee: OutcomeFee::xor(value.fee),
        }
    }
}

#[derive(Eq, PartialEq, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct SwapOutcomeInfoV1<Balance> {
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
}

sp_api::decl_runtime_apis! {
    #[api_version(2)]
    pub trait DEXAPI<AssetId, DEXId, Balance, LiquiditySourceType, SwapVariant> where
        AssetId: Codec + MaybeDisplay + MaybeFromStr + Ord,
        DEXId: Codec,
        LiquiditySourceType: Codec,
        Balance: Codec + MaybeFromStr + MaybeDisplay,
        SwapVariant: Codec,
    {
        fn quote(
            dex_id: DEXId,
            liquidity_source_type: LiquiditySourceType,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
            amount: BalanceWrapper,
            swap_variant: SwapVariant,
        ) -> Option<SwapOutcomeInfo<Balance, AssetId>>;

        #[changed_in(2)]
        fn quote(
            dex_id: DEXId,
            liquidity_source_type: LiquiditySourceType,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
            amount: BalanceWrapper,
            swap_variant: SwapVariant,
        ) -> Option<SwapOutcomeInfoV1<Balance>>;

        fn can_exchange(
            dex_id: DEXId,
            liquidity_source_type: LiquiditySourceType,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
        ) -> bool;

        fn list_supported_sources() -> Vec<LiquiditySourceType>;
    }
}
