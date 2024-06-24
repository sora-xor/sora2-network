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

pub mod storages {
    use super::*;

    mod v1 {
        use super::*;

        #[frame_support::storage_alias]
        pub type AssetInfos<T: Config> = StorageMap<
            Pallet<T>,
            Twox64Concat,
            <T as Config>::AssetId,
            (
                AssetSymbol,
                AssetName,
                BalancePrecision,
                bool,
                Option<ContentSource>,
                Option<Description>,
            ),
            ValueQuery,
        >;
    }

    pub use v1::AssetInfos as AssetInfosV1;

    mod v2 {
        use super::*;

        #[frame_support::storage_alias]
        pub type AssetInfos<T: Config> = StorageMap<
            Pallet<T>,
            Twox64Concat,
            <T as Config>::AssetId,
            common::AssetInfo,
            ValueQuery,
        >;
    }

    pub use v2::AssetInfos as AssetInfosV2;
}

use crate::{Config, Pallet};
use common::{AssetName, AssetSymbol, BalancePrecision, ContentSource, Description};
use frame_support::pallet_prelude::*;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use storages::*;

#[cfg(feature = "try-runtime")]
use sp_std::prelude::Vec;

pub struct AssetsUpdateV2<T>(core::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for AssetsUpdateV2<T>
where
    T: Config,
{
    fn on_runtime_upgrade() -> Weight {
        if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(1) {
            frame_support::log::error!(
                "Expected storage version 1, found {:?}, skipping migration",
                StorageVersion::get::<Pallet<T>>()
            );
        }
        let mut weight = Weight::zero();

        AssetInfosV2::<T>::translate::<
            (
                AssetSymbol,
                AssetName,
                BalancePrecision,
                bool,
                Option<ContentSource>,
                Option<Description>,
            ),
            _,
        >(
            |_, (symbol, name, precision, is_mintable, content_source, description)| {
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 2));
                let asset_info = common::AssetInfo {
                    symbol,
                    name,
                    precision,
                    is_mintable,
                    asset_type: common::AssetType::Regular,
                    content_source,
                    description,
                };
                Some(asset_info)
            },
        );

        StorageVersion::new(2).put::<Pallet<T>>();
        weight.saturating_add(T::DbWeight::get().reads_writes(1, 1))
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        frame_support::ensure!(
            StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1),
            "must upgrade linearly"
        );
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        frame_support::ensure!(
            StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
            "should be upgraded to version 1"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::storages::*;
    use super::AssetsUpdateV2;
    use crate::mock::*;
    use crate::pallet::Pallet;

    use common::AssetInfo;
    use common::AssetName;
    use common::AssetSymbol;
    use common::BalancePrecision;
    use common::ContentSource;
    use common::Description;
    use common::USDT;
    use common::XOR;
    use frame_support::traits::{GetStorageVersion as _, OnRuntimeUpgrade, StorageVersion};
    #[test]
    fn test() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            StorageVersion::new(1).put::<Pallet<Runtime>>();
            let assets = [
                (
                    XOR,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"XOR".to_vec()),
                ),
                (
                    USDT,
                    AssetSymbol(b"USDT".to_vec()),
                    AssetName(b"USDT".to_vec()),
                ),
            ];

            assets
                .iter()
                .cloned()
                .for_each(|(asset_id, asset_symbol, asset_name)| {
                    AssetInfosV1::<Runtime>::insert::<
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
                        asset_id, (asset_symbol, asset_name, 18, true, None, None)
                    );
                });

            System::set_block_number(1);
            AssetsUpdateV2::<Runtime>::on_runtime_upgrade();

            for (asset_id, asset_symbol, asset_name) in assets.into_iter() {
                let asset_info = AssetInfosV2::<Runtime>::get(asset_id);

                assert_eq!(
                    asset_info,
                    AssetInfo {
                        name: asset_name,
                        symbol: asset_symbol,
                        precision: 18,
                        is_mintable: true,
                        asset_type: common::AssetType::Regular,
                        content_source: None,
                        description: None,
                    }
                );
            }
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 2);
        });
    }
}
