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

use super::pallet::{Config, Pallet};
use codec::Decode;
use common::{AssetIdOf, DEXInfo, XST};
use frame_support::pallet_prelude::{Get, StorageVersion};
use frame_support::{log::info, traits::GetStorageVersion as _, weights::Weight};

use crate::DEXInfos;

pub mod kusd_dex;

#[derive(Decode)]
struct DEXInfoV0<T: Config> {
    base_asset_id: AssetIdOf<T>,
    is_public: bool,
}

/// Migration which adds `XST` as a *synthetic base asset*
pub fn migrate<T: Config>() -> Weight {
    if Pallet::<T>::on_chain_storage_version() >= 2 {
        info!("Migration to version 2 has already been applied");
        return Weight::zero();
    }

    let mut weight = 0;
    DEXInfos::<T>::translate::<DEXInfoV0<T>, _>(|_, dex_info| {
        weight += 1;
        Some(DEXInfo {
            base_asset_id: dex_info.base_asset_id,
            synthetic_base_asset_id: XST.into(),
            is_public: dex_info.is_public,
        })
    });

    StorageVersion::new(2).put::<Pallet<T>>();
    T::DbWeight::get().reads_writes(weight, weight)
}

#[cfg(test)]
mod tests;
