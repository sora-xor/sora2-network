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

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

pub mod weights;

use common::Balance;
use common::ReferrerAccountProvider;
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;

pub use weights::WeightInfo;

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
            Referrals::<T>::append(&referrer, referral);
            *r = Some(referrer);
            Ok(())
        })
    }

    pub fn can_set_referrer(referral: &T::AccountId) -> bool {
        !Referrers::<T>::contains_key(referral)
    }

    pub fn withdraw_fee(referrer: &T::AccountId, fee: Balance) -> Result<(), DispatchError> {
        ReferrerBalances::<T>::mutate(referrer, |b| {
            let balance = b
                .unwrap_or(0)
                .checked_sub(fee)
                .ok_or(DispatchError::from(Error::<T>::ReferrerInsufficientBalance))?;
            *b = Some(balance);
            Ok(())
        })
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use common::{
        AssetIdOf, AssetInfoProvider, AssetManager, AssetName, AssetSymbol, Balance,
        BalancePrecision, ContentSource, Description, XOR,
    };
    use frame_support::pallet_prelude::*;
    use frame_support::sp_runtime;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use sp_std::prelude::*;

    use crate::WeightInfo;

    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config {
        type ReservesAcc: Get<Self::AccountId>;
        type WeightInfo: WeightInfo;
        type AssetInfoProvider: AssetInfoProvider<
            AssetIdOf<Self>,
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
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Reserves the balance from the account for a special balance that can be used to pay referrals' fees
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::reserve())]
        pub fn reserve(origin: OriginFor<T>, balance: Balance) -> DispatchResultWithPostInfo {
            let referrer = ensure_signed(origin)?;

            if balance == 0 {
                return Ok(().into());
            }

            common::with_transaction(|| {
                T::AssetManager::transfer_from(
                    &XOR.into(),
                    &referrer,
                    &T::ReservesAcc::get(),
                    balance,
                )?;

                ReferrerBalances::<T>::mutate(referrer, |b| {
                    *b = Some(b.unwrap_or(0).saturating_add(balance))
                });

                Ok(().into())
            })
        }

        /// Unreserves the balance and transfers it back to the account
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::unreserve())]
        pub fn unreserve(origin: OriginFor<T>, balance: Balance) -> DispatchResultWithPostInfo {
            let referrer = ensure_signed(origin)?;

            if balance == 0 {
                return Ok(().into());
            }

            common::with_transaction(|| {
                ReferrerBalances::<T>::mutate(&referrer, |b| {
                    if let Some(balance) = b.unwrap_or(0).checked_sub(balance) {
                        *b = (balance != 0).then(|| balance);
                        Ok(())
                    } else {
                        Err(Error::<T>::ReferrerInsufficientBalance)
                    }
                })?;

                T::AssetManager::transfer_from(
                    &XOR.into(),
                    &T::ReservesAcc::get(),
                    &referrer,
                    balance,
                )?;

                Ok(().into())
            })
        }

        /// Sets the referrer for the account
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::set_referrer())]
        pub fn set_referrer(
            origin: OriginFor<T>,
            referrer: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let referree = ensure_signed(origin)?;
            Self::set_referrer_to(&referree, referrer)?;
            Ok(().into())
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Account already has a referrer.
        AlreadyHasReferrer,
        /// Increment account reference error.
        IncRefError,
        /// Referrer doesn't have enough of reserved balance
        ReferrerInsufficientBalance,
    }

    #[pallet::storage]
    #[pallet::getter(fn referrer_account)]
    pub type Referrers<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    #[pallet::storage]
    #[pallet::getter(fn referrer_balance)]
    pub type ReferrerBalances<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance>;

    #[pallet::storage]
    #[pallet::getter(fn referrals)]
    pub type Referrals<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<T::AccountId>, ValueQuery>;

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
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            self.referrers.iter().for_each(|(k, v)| {
                frame_system::Pallet::<T>::inc_consumers(k).unwrap();
                frame_system::Pallet::<T>::inc_consumers(v).unwrap();
                Referrers::<T>::insert(k, v);
                Referrals::<T>::append(v, k);
            });
        }
    }
}

impl<T: Config> ReferrerAccountProvider<T::AccountId> for Pallet<T> {
    fn get_referrer_account(who: &T::AccountId) -> Option<T::AccountId> {
        Self::referrer_account(who)
    }
}
