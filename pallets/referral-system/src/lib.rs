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

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;

impl<T: Config> Pallet<T> {
    pub fn set_referrer_to(
        referral: &T::AccountId,
        referrer: T::AccountId,
    ) -> Result<(), DispatchError> {
        Referrers::<T>::mutate(&referral, |r| {
            ensure!(r.is_none(), Error::<T>::AlreadyHasReferrer);
            frame_system::Pallet::<T>::inc_providers(referral);
            frame_system::Pallet::<T>::inc_providers(&referrer);
            frame_system::Pallet::<T>::inc_consumers(referral)
                .map_err(|_| Error::<T>::IncRefError)?;
            frame_system::Pallet::<T>::inc_consumers(&referrer)
                .map_err(|_| Error::<T>::IncRefError)?;
            *r = Some(referrer);
            Ok(())
        })
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {}

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Account already has a referrer.
        AlreadyHasReferrer,
        /// Increment account reference error.
        IncRefError,
    }

    #[pallet::storage]
    #[pallet::getter(fn referrer_account)]
    pub type Referrers<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub referrers: Vec<(T::AccountId, T::AccountId)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                referrers: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.referrers.iter().for_each(|(k, v)| {
                frame_system::Pallet::<T>::inc_consumers(k).unwrap();
                frame_system::Pallet::<T>::inc_consumers(v).unwrap();
                Referrers::<T>::insert(k, v);
            });
        }
    }
}
