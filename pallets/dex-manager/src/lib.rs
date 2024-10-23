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
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use common::prelude::EnsureDexManager;
use common::AssetIdOf;
use common::{hash, DexInfoProvider, ManagementMode};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_system::RawOrigin;
use permissions::{Scope, MANAGE_DEX};
use sp_std::vec::Vec;

pub mod migrations;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type DexInfo<T> = common::prelude::DexInfo<AssetIdOf<T>>;

impl<T: Config> EnsureDexManager<T::DexId, T::AccountId, DispatchError> for Pallet<T> {
    fn ensure_can_manage<OuterOrigin>(
        dex_id: &T::DexId,
        origin: OuterOrigin,
        mode: ManagementMode,
    ) -> Result<Option<T::AccountId>, DispatchError>
    where
        OuterOrigin: Into<Result<RawOrigin<T::AccountId>, OuterOrigin>>,
    {
        match origin.into() {
            Ok(RawOrigin::Signed(who)) => {
                let dex_info = Self::get_dex_info(&dex_id)?;
                // If DEX is public, anyone can manage it, otherwise confirm ownership.
                if !dex_info.is_public || mode != ManagementMode::Public {
                    Self::ensure_direct_manager(&dex_id, &who)?;
                }
                Ok(Some(who))
            }
            _ => Err(Error::<T>::InvalidAccountId.into()),
        }
    }
}

impl<T: Config> DexInfoProvider<T::DexId, DexInfo<T>> for Pallet<T> {
    fn get_dex_info(dex_id: &T::DexId) -> Result<DexInfo<T>, DispatchError> {
        Ok(DexInfos::<T>::get(&dex_id).ok_or(Error::<T>::DEXDoesNotExist)?)
    }

    fn ensure_dex_exists(dex_id: &T::DexId) -> DispatchResult {
        ensure!(
            DexInfos::<T>::contains_key(&dex_id),
            Error::<T>::DEXDoesNotExist
        );
        Ok(())
    }

    fn list_dex_ids() -> Vec<T::DexId> {
        DexInfos::<T>::iter().map(|(k, _)| k).collect()
    }
}

impl<T: Config> Pallet<T> {
    fn ensure_direct_manager(dex_id: &T::DexId, who: &T::AccountId) -> DispatchResult {
        permissions::Pallet::<T>::check_permission_with_scope(
            who.clone(),
            MANAGE_DEX,
            &Scope::Limited(hash(&dex_id)),
        )
        .map_err(|e| e.into())
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;

    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config + permissions::Config {}

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::error]
    pub enum Error<T> {
        /// DEX with given id is already registered.
        DexIdAlreadyExists,
        /// DEX with given Id is not registered.
        DEXDoesNotExist,
        /// Numeric value provided as fee is not valid, e.g. out of basis-point range.
        InvalidFeeValue,
        /// Account with given Id is not registered.
        InvalidAccountId,
    }

    #[pallet::storage]
    #[pallet::getter(fn dex_id)]
    pub type DexInfos<T: Config> = StorageMap<_, Twox64Concat, T::DexId, DexInfo<T>>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub dex_list: Vec<(T::DexId, DexInfo<T>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                dex_list: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.dex_list.iter().for_each(|(dex_id, dex_info)| {
                DexInfos::<T>::insert(dex_id.clone(), dex_info);
            })
        }
    }
}
