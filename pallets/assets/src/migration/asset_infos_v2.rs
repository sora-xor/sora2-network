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

use crate::{AssetInfos, AssetInfosV2, Config};
use frame_support::pallet_prelude::*;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;

pub struct AssetInfosUpdate<T>(core::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for AssetInfosUpdate<T>
where
    T: Config,
{
    fn on_runtime_upgrade() -> Weight {
        let mut weight = Weight::zero();

        AssetInfos::<T>::iter().for_each(
            |(asset_id, (symbol, name, precision, is_mintable, content_source, description))| {
                AssetInfosV2::<T>::insert(
                    asset_id,
                    common::AssetInfo {
                        symbol,
                        name,
                        precision,
                        is_mintable,
                        asset_type: if precision == 0 {
                            common::AssetType::NFT
                        } else {
                            common::AssetType::Regular
                        },
                        content_source,
                        description,
                    },
                );
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1))
            },
        );

        weight.saturating_add(T::DbWeight::get().reads_writes(1, 1))
    }
}

#[cfg(test)]
mod tests {

    use super::{AssetInfos, AssetInfosUpdate, AssetInfosV2};
    use crate::mock::*;

    use common::{
        AssetInfo, AssetName, AssetSymbol, BalancePrecision, ContentSource, Description, USDT, XOR,
    };
    use frame_support::traits::OnRuntimeUpgrade;

    #[test]
    fn test() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let assets = [
                (
                    XOR,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"XOR".to_vec()),
                    18,
                    true,
                ),
                (
                    USDT,
                    AssetSymbol(b"USDT".to_vec()),
                    AssetName(b"USDT".to_vec()),
                    0,
                    false,
                ),
            ];

            assets.iter().cloned().for_each(
                |(asset_id, asset_symbol, asset_name, precision, is_mintable)| {
                    AssetInfos::<Runtime>::insert::<
                        _,
                        (
                            AssetSymbol,
                            AssetName,
                            BalancePrecision,
                            bool,
                            Option<ContentSource>,
                            Option<Description>,
                        ),
                    >(
                        asset_id,
                        (asset_symbol, asset_name, precision, is_mintable, None, None),
                    );
                },
            );

            AssetInfosUpdate::<Runtime>::on_runtime_upgrade();

            for (asset_id, asset_symbol, asset_name, precision, is_mintable) in assets.into_iter() {
                let asset_info_v1 = AssetInfos::<Runtime>::get(asset_id);
                let asset_info_v2 = AssetInfosV2::<Runtime>::get(asset_id);
                let asset_type = if precision == 0 {
                    common::AssetType::NFT
                } else {
                    common::AssetType::Regular
                };
                assert_eq!(
                    asset_info_v1,
                    (
                        asset_symbol.clone(),
                        asset_name.clone(),
                        precision,
                        is_mintable,
                        None,
                        None
                    )
                );
                assert_eq!(
                    asset_info_v2,
                    AssetInfo {
                        name: asset_name,
                        symbol: asset_symbol,
                        precision,
                        is_mintable,
                        asset_type,
                        content_source: None,
                        description: None,
                    }
                );
            }
        });
    }
}
