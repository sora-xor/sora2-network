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
use common::{fixed, Fixed, XSTUSD};
use frame_support::pallet_prelude::{Get, StorageVersion, ValueQuery};
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::{traits::GetStorageVersion as _, weights::Weight};
use log::info;
use sp_std::collections::btree_set::BTreeSet;

#[cfg(feature = "try-runtime")]
use sp_std::prelude::Vec;

use crate::{EnabledSymbols, EnabledSynthetics as NewEnabledSynthetics, SyntheticInfo};

#[frame_support::storage_alias]
type BaseFee<T: Config> = StorageValue<Pallet<T>, Fixed, ValueQuery>;

#[frame_support::storage_alias]
type PermissionedTechAccount<T: Config> =
    StorageValue<Pallet<T>, <T as technical::Config>::TechAccountId, ValueQuery>;

#[frame_support::storage_alias]
type EnabledSynthetics<T: Config> =
    StorageValue<Pallet<T>, BTreeSet<<T as assets::Config>::AssetId>, ValueQuery>;

pub struct CustomSyntheticsUpgrade<T>(core::marker::PhantomData<T>);

/// Migration which migrates `XSTUSD` synthetic to the new format.
impl<T> OnRuntimeUpgrade for CustomSyntheticsUpgrade<T>
where
    T: crate::Config,
    <T as frame_system::Config>::AccountId: From<[u8; 32]>,
{
    fn on_runtime_upgrade() -> Weight {
        if Pallet::<T>::on_chain_storage_version() >= 2 {
            info!("Migration to version 2 has already been applied");
            return Weight::zero();
        }

        if BaseFee::<T>::exists() {
            BaseFee::<T>::kill();
        }

        if PermissionedTechAccount::<T>::exists() {
            PermissionedTechAccount::<T>::kill();
        }

        if EnabledSynthetics::<T>::exists() {
            EnabledSynthetics::<T>::kill();
        }

        let xstusd_symbol = T::Symbol::from(common::SymbolName::usd());

        NewEnabledSynthetics::<T>::insert(
            T::AssetId::from(XSTUSD),
            SyntheticInfo {
                reference_symbol: xstusd_symbol.clone(),
                fee_ratio: fixed!(0.00666),
            },
        );
        EnabledSymbols::<T>::insert(xstusd_symbol, T::AssetId::from(XSTUSD));

        StorageVersion::new(2).put::<Pallet<T>>();
        T::DbWeight::get().reads_writes(0, 2)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 1,
            "must upgrade linearly"
        );
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 2,
            "should be upgraded to version 2"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests;
