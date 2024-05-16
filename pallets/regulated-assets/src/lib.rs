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

//! Regulated Assets pallet provides an ability to configure an access to regulated assets.

#![cfg_attr(not(feature = "std"), no_std)]

use common::{
    AssetInfoProvider, AssetName, AssetRegulator, AssetSymbol, BalancePrecision, ContentSource,
    Description,
};
use frame_support::sp_runtime::DispatchError;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type AssetIdOf<T> = <T as assets::Config>::AssetId;

impl<T: Config> Pallet<T> {}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type AssetInfoProvider: AssetInfoProvider<
            Self::AssetId,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {}

    #[pallet::type_value]
    pub fn DefaultAssetRegulated<T: Config>() -> bool {
        false
    }

    #[pallet::storage]
    #[pallet::getter(fn asset_regulated)]
    pub type AssetRegulated<T: Config> =
        StorageMap<_, Blake2_256, AssetIdOf<T>, bool, ValueQuery, DefaultAssetRegulated<T>>;
}

impl<T: Config> AssetRegulator<AccountIdOf<T>, AssetIdOf<T>> for Pallet<T> {
    fn mint(
        issuer: &AccountIdOf<T>,
        _to: Option<&AccountIdOf<T>>,
        asset_id: &AssetIdOf<T>,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn transfer(
        _from: &AccountIdOf<T>,
        _to: &AccountIdOf<T>,
        _asset_id: &AssetIdOf<T>,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn burn(
        issuer: &AccountIdOf<T>,
        from: Option<&AccountIdOf<T>>,
        asset_id: &AssetIdOf<T>,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn assign_permissions_on_register(
        owner: &AccountIdOf<T>,
        asset_id: &AssetIdOf<T>,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}
