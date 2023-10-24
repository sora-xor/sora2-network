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
use bridge_types::GenericNetworkId;
use sp_runtime::traits::Zero;

pub struct HashiBridgeLockedAssets;

impl Get<Vec<(AssetId, Balance)>> for HashiBridgeLockedAssets {
    fn get() -> Vec<(AssetId, Balance)> {
        let Ok(assets) = EthBridge::get_registered_assets(Some(GetEthNetworkId::get())) else {
            frame_support::log::warn!("Failed to get registered assets, skipping migration");
            return vec![];
        };
        let Some(bridge_account) = eth_bridge::BridgeAccount::<Runtime>::get(GetEthNetworkId::get()) else {
            frame_support::log::warn!("Failed to get Hashi bridge account, skipping migration");
            return vec![];
        };
        let mut result = vec![];
        for (kind, (asset_id, _precision), _) in assets {
            let reserved = if kind.is_owned() {
                Assets::total_balance(&asset_id, &bridge_account)
            } else {
                Assets::total_issuance(&asset_id)
            };
            let reserved = reserved.unwrap_or_default();
            if !reserved.is_zero() {
                result.push((asset_id, reserved));
            }
        }
        result
    }
}

parameter_types! {
    pub const HashiBridgeNetworkId: GenericNetworkId = GenericNetworkId::EVMLegacy(GetEthNetworkId::get());
}

pub type Migrations = (
    bridge_proxy::migrations::init::InitLockedAssets<
        Runtime,
        HashiBridgeLockedAssets,
        HashiBridgeNetworkId,
    >,
);
