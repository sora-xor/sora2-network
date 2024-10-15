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

use core::marker::PhantomData;

use crate::{AggregatedPriceInfo, Config, FastPriceInfos, Pallet, PriceInfos};
use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
use sp_core::Get;
#[cfg(feature = "try-runtime")]
use sp_std::prelude::*;

pub struct AddFastPriceInfos<T>(PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for AddFastPriceInfos<T> {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        if Pallet::<T>::on_chain_storage_version() == StorageVersion::new(2) {
            for asset_id in PriceInfos::<T>::iter_keys() {
                FastPriceInfos::<T>::insert(asset_id, AggregatedPriceInfo::default());
            }
            StorageVersion::new(3).put::<Pallet<T>>()
        } else {
            log::error!(
                "Current version {:?}, expected 2",
                Pallet::<T>::on_chain_storage_version()
            );
        }
        T::BlockWeights::get().max_block
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        if FastPriceInfos::<T>::iter().count() > 0 {
            return Err("FastPriceInfos storage should not have values");
        }
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        for key in PriceInfos::<T>::iter_keys() {
            if FastPriceInfos::<T>::get(key).ok_or("Expected key not found")?
                != AggregatedPriceInfo::default()
            {
                return Err("Unexpected storage value");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::migrations::v3::AddFastPriceInfos;
    use crate::mock::{ExtBuilder, Runtime};
    use crate::{AggregatedPriceInfo, FastPriceInfos, Pallet, PriceInfos};
    use core::default::Default;
    use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};

    #[test]
    fn test() {
        ExtBuilder::default().build().execute_with(|| {
            StorageVersion::new(2).put::<Pallet<Runtime>>();

            PriceInfos::<Runtime>::insert(common::DAI, AggregatedPriceInfo::default());
            PriceInfos::<Runtime>::insert(common::ETH, AggregatedPriceInfo::default());

            assert!(
                FastPriceInfos::<Runtime>::iter().count() == 0,
                "FastPriceInfos should not have any values"
            );

            AddFastPriceInfos::<Runtime>::on_runtime_upgrade();

            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 3);

            assert!(
                FastPriceInfos::<Runtime>::iter().count() == PriceInfos::<Runtime>::iter().count(),
            );

            for key in PriceInfos::<Runtime>::iter_keys() {
                assert!(
                    FastPriceInfos::<Runtime>::get(key)
                        .expect("FastPriceInfos should have the same keys as PriceInfos")
                        == AggregatedPriceInfo::default(),
                );
            }
        });
    }
}
