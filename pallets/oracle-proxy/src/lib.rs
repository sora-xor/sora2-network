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

use common::{DataFeed, OnNewSymbolsRelayed, Oracle, Rate};
use frame_support;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use sp_std::{collections::btree_set::BTreeSet, vec::Vec};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

impl<T: Config> DataFeed<T::Symbol, Rate, u64> for Pallet<T> {
    fn quote(symbol: &T::Symbol) -> Result<Option<Rate>, DispatchError> {
        let enabled_oracles = Self::enabled_oracles();

        Self::enabled_symbols(symbol)
            .into_iter()
            .filter(|oracle| enabled_oracles.contains(&oracle))
            .map(|oracle| match oracle {
                Oracle::BandChainFeed => T::BandChainOracle::quote(symbol),
            })
            .next()
            .unwrap_or(Ok(None))
    }

    fn list_enabled_symbols() -> Result<Vec<(T::Symbol, u64)>, DispatchError> {
        let symbols_rates = Self::enabled_oracles()
            .into_iter()
            .flat_map(|oracle| match oracle {
                Oracle::BandChainFeed => match T::BandChainOracle::list_enabled_symbols() {
                    Ok(list) => list,
                    _ => Vec::new(),
                },
            })
            .collect();
        Ok(symbols_rates)
    }

    fn quote_unchecked(symbol: &T::Symbol) -> Option<Rate> {
        let enabled_oracles = Self::enabled_oracles();

        Self::enabled_symbols(symbol)
            .into_iter()
            .filter(|oracle| enabled_oracles.contains(&oracle))
            .map(|oracle| match oracle {
                Oracle::BandChainFeed => T::BandChainOracle::quote_unchecked(symbol),
            })
            .next()
            .unwrap_or(None)
    }
}

impl<T: Config> OnNewSymbolsRelayed<T::Symbol> for Pallet<T> {
    fn on_new_symbols_relayed(
        oracle_variant: Oracle,
        symbols: BTreeSet<T::Symbol>,
    ) -> Result<(), DispatchError> {
        symbols.into_iter().for_each(|symbol| {
            SymbolProviders::<T>::set(symbol, Some(oracle_variant));
        });
        Ok(())
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::traits::StorageVersion;
    use sp_std::collections::btree_set::BTreeSet;

    /// `OracleProxy` pallet is used to aggregate data from all supported oracles in Sora.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type WeightInfo: WeightInfo;
        type Symbol: Parameter + Ord;
        type BandChainOracle: DataFeed<Self::Symbol, Rate, u64>;
    }

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::storage]
    #[pallet::getter(fn enabled_oracles)]
    pub type EnabledOracles<T: Config> = StorageValue<_, BTreeSet<Oracle>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn enabled_symbols)]
    pub type SymbolProviders<T: Config> = StorageMap<_, Blake2_128Concat, T::Symbol, Oracle>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Oracle was successfully enabled. [oracle]
        OracleEnabled(Oracle),
        /// Oracle was successfully disabled. [oracle]
        OracleDisabled(Oracle),
    }

    #[pallet::error]
    pub enum Error<T> {
        OracleAlreadyEnabled,
        OracleAlreadyDisabled,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Enables a specified oracle
        ///
        /// Checks if the caller is root
        ///
        /// - `origin`: the sudo account
        /// - `oracle`: oracle variant which should be enabled
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::enable_oracle())]
        pub fn enable_oracle(origin: OriginFor<T>, oracle: Oracle) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ensure!(
                !Self::enabled_oracles().contains(&oracle),
                Error::<T>::OracleAlreadyEnabled
            );

            EnabledOracles::<T>::mutate(|enabled_oracles| enabled_oracles.insert(oracle.clone()));

            Self::deposit_event(Event::OracleEnabled(oracle));

            Ok(().into())
        }

        /// Disables a specified oracle
        ///
        /// Checks if the caller is root
        ///
        /// - `origin`: the sudo account
        /// - `oracle`: oracle variant which should be disabled
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::disable_oracle())]
        pub fn disable_oracle(origin: OriginFor<T>, oracle: Oracle) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ensure!(
                Self::enabled_oracles().contains(&oracle),
                Error::<T>::OracleAlreadyDisabled
            );

            EnabledOracles::<T>::mutate(|enabled_oracles| enabled_oracles.remove(&oracle));

            Self::deposit_event(Event::OracleDisabled(oracle));

            Ok(().into())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub enabled_oracles: BTreeSet<Oracle>,
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {
                enabled_oracles: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            EnabledOracles::<T>::put(&self.enabled_oracles);
        }
    }
}
