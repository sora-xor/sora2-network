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

use super::{
    pallet::{Config, Pallet},
    BandRate,
};
use codec::Decode;
use common::{fixed, Balance};
use frame_support::pallet_prelude::{Get, StorageVersion};
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;

#[cfg(feature = "try-runtime")]
use sp_std::prelude::Vec;

use crate::SymbolRates;

#[derive(Decode)]
pub struct BandRateV0 {
    pub value: Balance,
    pub last_updated: u64,
    pub request_id: u64,
}

pub struct BandUpdate<T>(core::marker::PhantomData<T>);

/// Migration which migrates `XSTUSD` synthetic to the new format.
impl<T> OnRuntimeUpgrade for BandUpdate<T>
where
    T: Config,
{
    fn on_runtime_upgrade() -> Weight {
        if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(0) {
            frame_support::log::error!(
                "Expected storage version 0, found {:?}, skipping migration",
                StorageVersion::get::<Pallet<T>>()
            );
        }
        let mut weight = Weight::zero();

        SymbolRates::<T>::translate::<Option<BandRateV0>, _>(|_, band_rate| {
            weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
            match band_rate {
                Some(band_rate) => Some(Some(BandRate {
                    value: band_rate.value,
                    last_updated: band_rate.last_updated,
                    request_id: band_rate.request_id,
                    dynamic_fee: fixed!(0),
                })),
                None => None,
            }
        });

        StorageVersion::new(1).put::<Pallet<T>>();
        weight.saturating_add(T::DbWeight::get().reads_writes(1, 1))
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        frame_support::ensure!(
            StorageVersion::get::<Pallet<T>>() == StorageVersion::new(0),
            "must upgrade linearly"
        );
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        frame_support::ensure!(
            StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1),
            "should be upgraded to version 1"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{mock::*, pallet::*};
    use common::fixed;
    use frame_support::traits::{GetStorageVersion as _, OnRuntimeUpgrade, StorageVersion};
    #[test]
    fn test() {
        new_test_ext().execute_with(|| {
            StorageVersion::new(0).put::<Pallet<Runtime>>();

            super::BandUpdate::<Runtime>::on_runtime_upgrade();

            for band_rate in SymbolRates::<Runtime>::iter_values() {
                if let Some(band_rate) = band_rate {
                    assert_eq!(band_rate.dynamic_fee, fixed!(0));
                }
            }
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 1);
        });
    }
}
