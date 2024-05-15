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

use common::{
    generate_storage_instance, AssetName, AssetSymbol, BalancePrecision, ContentSource, Description,
};
use frame_support::log::error;
use frame_support::pallet_prelude::{Get, OptionQuery, StorageMap};
use frame_support::Twox64Concat;

use crate::{AssetInfos, Config};

generate_storage_instance!(Assets, AssetContentSource);
type OldAssetContentSource<AssetId> =
    StorageMap<AssetContentSourceOldInstance, Twox64Concat, AssetId, ContentSource, OptionQuery>;

generate_storage_instance!(Assets, AssetDescription);
type OldAssetDescription<AssetId> =
    StorageMap<AssetDescriptionOldInstance, Twox64Concat, AssetId, Description, OptionQuery>;

pub fn migrate<T: Config>() -> Weight {
    let mut weight = 0;
    AssetInfos::<T>::translate::<(AssetSymbol, AssetName, BalancePrecision, bool), _>(
        |key, (symbol, name, precision, is_mintable)| {
            weight += T::DbWeight::get().reads_writes(3, 3);
            let content_source = OldAssetContentSource::<T::AssetId>::take(key);
            let description = OldAssetDescription::<T::AssetId>::take(key);
            Some((
                symbol,
                name,
                precision,
                is_mintable,
                content_source,
                description,
            ))
        },
    );
    let content_source_count = OldAssetContentSource::<T::AssetId>::drain().count();
    let description_count = OldAssetDescription::<T::AssetId>::drain().count();
    if content_source_count != 0 || description_count != 0 {
        error!(
            "remaining content sources: {}, remaining descriptions: {}",
            content_source_count, description_count
        );
    }
    weight
}

#[cfg(test)]
mod tests {
    use common::{
        generate_storage_instance, AssetName, AssetSymbol, BalancePrecision, ContentSource,
        Description, DEFAULT_BALANCE_PRECISION, ETH,
    };
    use frame_support::pallet_prelude::{StorageMap, ValueQuery};
    use frame_support::Twox64Concat;

    use crate::mock::{AssetId, ExtBuilder, Runtime};
    use crate::AssetInfos;

    use super::{OldAssetContentSource, OldAssetDescription};

    generate_storage_instance!(Assets, AssetInfos);
    type OldAssetInfos<AssetId> = StorageMap<
        AssetInfosOldInstance,
        Twox64Concat,
        AssetId,
        (AssetSymbol, AssetName, BalancePrecision, bool),
        ValueQuery,
    >;

    #[test]
    fn migrate() {
        ExtBuilder::default().build().execute_with(|| {
            OldAssetInfos::<AssetId>::insert(
                ETH,
                (
                    AssetSymbol(b"A".to_vec()),
                    AssetName(b"B".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    true,
                ),
            );
            OldAssetContentSource::<AssetId>::insert(ETH, ContentSource(b"C".to_vec()));
            OldAssetDescription::<AssetId>::insert(ETH, Description(b"D".to_vec()));
            super::migrate::<Runtime>();
            let (symbol, name, precision, is_mintable, content_source, description) =
                AssetInfos::<Runtime>::get(ETH);
            assert_eq!(symbol, AssetSymbol(b"A".to_vec()));
            assert_eq!(name, AssetName(b"B".to_vec()));
            assert_eq!(precision, DEFAULT_BALANCE_PRECISION);
            assert!(is_mintable);
            assert_eq!(content_source, Some(ContentSource(b"C".to_vec())));
            assert_eq!(description, Some(Description(b"D".to_vec())));
        });
    }
}
