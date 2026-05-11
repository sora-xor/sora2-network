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

mod benchmarking;

pub mod weights;

pub use pallet::*;

/// Soratopia game pallet
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use crate::weights::WeightInfo;
    use common::{AccountIdOf, AssetManager, Balance, XOR};
    use frame_support::{pallet_prelude::*, transactional};
    use frame_system::pallet_prelude::*;
    use sp_runtime::Saturating;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        #[pallet::constant]
        type CheckInTransferAmount: Get<Balance>;

        /// Minimum number of blocks between successful check-ins for one account.
        #[pallet::constant]
        type CheckInInterval: Get<BlockNumberFor<Self>>;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        CheckIn(AccountIdOf<T>),
    }

    #[pallet::storage]
    #[pallet::getter(fn last_check_in)]
    pub type LastCheckIn<T: Config> =
        StorageMap<_, Blake2_128Concat, AccountIdOf<T>, BlockNumberFor<T>, OptionQuery>;

    #[pallet::error]
    pub enum Error<T> {
        /// The account has already checked in within the configured interval.
        CheckInTooSoon,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Soratopia on-chain check in.
        /// Burns XOR from caller.
        #[transactional]
        #[pallet::call_index(0)]
        #[pallet::weight(< T as Config>::WeightInfo::check_in())]
        pub fn check_in(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let current_block = frame_system::Pallet::<T>::block_number();

            if let Some(last_check_in) = LastCheckIn::<T>::get(&who) {
                ensure!(
                    current_block.saturating_sub(last_check_in) >= T::CheckInInterval::get(),
                    Error::<T>::CheckInTooSoon
                );
            }

            T::AssetManager::burn_from(&XOR.into(), &who, &who, T::CheckInTransferAmount::get())?;
            LastCheckIn::<T>::insert(&who, current_block);
            Self::deposit_event(Event::CheckIn(who));
            Ok(().into())
        }
    }
}
