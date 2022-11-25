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

use frame_support::pallet_prelude::*;
use frame_support::weights::Weight;
use frame_system::pallet_prelude::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

pub mod weights;

pub trait WeightInfo {
    fn relay() -> Weight;
    fn force_relay() -> Weight;
    fn add_relayers() -> Weight;
    fn remove_relayers() -> Weight;
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Copy, Clone, PartialEq, Eq)]
pub struct Rate {
    pub value: u64,
    pub last_updated: u64,
}

impl Rate {
    pub fn update_if_in_the_past(&mut self, new: Rate) {
        if self.last_updated < new.last_updated {
            *self = new;
        }
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use sp_std::collections::btree_set::BTreeSet;
    use sp_std::prelude::*;

    /// `Band` pallet is used to relay data from *BandChain* oracles to Polkaswap.
    /// This data contains information about some symbols rates, like price of some cryptocurrencies,
    /// stocks, fiat and etc.
    ///
    /// Some service will call [`relay`](Pallet::relay()) extrinsic every period of time using
    /// trusted account. Governance (aka *ROOT* user) can add or remove such trusted accounts.
    ///
    /// `I` generic argument is used to be able to instantiate this pallet multiple times. One per
    /// every asset category. This will prevent overlapping tickers.
    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        /// Type of the symbol to be relayed.
        type Symbol: Parameter;
        /// Event type of this pallet.
        type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn trusted_relayers)]
    pub type TrustedRelayers<T: Config<I>, I: 'static = ()> =
        StorageValue<_, BTreeSet<T::AccountId>>;

    #[pallet::storage]
    #[pallet::getter(fn rates)]
    pub type SymbolRates<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::Symbol, Option<Rate>, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// New symbol rates were successfully relayed. [symbols]
        SymbolsRelayed(Vec<T::Symbol>),
        /// Added new trusted relayer accounts. [relayers]
        RelayersAdded(Vec<T::AccountId>),
        /// Relayer accounts were removed from trusted list. [relayers]
        RelayersRemoved(Vec<T::AccountId>),
    }

    #[pallet::error]
    pub enum Error<T, I = ()> {
        /// An untrusted account tried to relay data.
        NotATrustedRelayer,
        /// `symbols` and `rates` provided to `relay` (or `force_relay`) extrinsic have different
        /// lengths.
        DivergedLengthsOfSymbolsAndRates,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Relay a list of symbols and their associated rates along with the resolve time and request id on `BandChain`.
        ///
        /// Checks if:
        /// - The caller is a relayer;
        /// - The `resolve_time` for a particular symbol is not lower than previous saved value, ignores this rate if so;
        ///
        /// - `origin`: the relayer account on whose behalf the transaction is being executed,
        /// - `symbols`: symbols which rates are provided,
        /// - `rates`: rates of symbols in the same order as `symbols`,
        /// - `resolve_time`: symbols which rates are provided,
        /// - `request_id`: id of the request sent to the *BandChain* to retrieve this data.
        #[pallet::weight(<T as Config<I>>::WeightInfo::relay())]
        pub fn relay(
            origin: OriginFor<T>,
            symbols: Vec<T::Symbol>,
            rates: Vec<u64>,
            resolve_time: u64,
            _request_id: u64,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_relayer(origin)?;
            ensure!(
                symbols.len() == rates.len(),
                Error::<T, I>::DivergedLengthsOfSymbolsAndRates
            );

            for (symbol, rate_value) in symbols.iter().zip(rates) {
                let new_rate = Rate {
                    value: rate_value,
                    last_updated: resolve_time,
                };

                SymbolRates::<T, I>::mutate(symbol, |option_rate| match option_rate {
                    Some(rate) => rate.update_if_in_the_past(new_rate),
                    None => _ = option_rate.insert(new_rate),
                });
            }

            Self::deposit_event(Event::SymbolsRelayed(symbols));
            Ok(().into())
        }

        /// Similar to [`relay()`] but without the resolve time guard.
        ///
        /// Should be used in emergency situations i.e. then previous value was
        /// relayed by a faulty/malicious actor.
        ///
        /// - `origin`: the relayer account on whose behalf the transaction is being executed,
        /// - `symbols`: symbols which rates are provided,
        /// - `rates`: rates of symbols in the same order as `symbols`,
        /// - `resolve_time`: symbols which rates are provided,
        /// - `request_id`: id of the request sent to the *BandChain* to retrieve this data.
        #[pallet::weight(<T as Config<I>>::WeightInfo::force_relay())]
        pub fn force_relay(
            origin: OriginFor<T>,
            symbols: Vec<T::Symbol>,
            rates: Vec<u64>,
            resolve_time: u64,
            _request_id: u64,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_relayer(origin)?;
            ensure!(
                symbols.len() == rates.len(),
                Error::<T, I>::DivergedLengthsOfSymbolsAndRates
            );

            for (symbol, rate_value) in symbols.iter().zip(rates) {
                let new_rate = Rate {
                    value: rate_value,
                    last_updated: resolve_time,
                };

                SymbolRates::<T, I>::mutate(symbol, |rate| {
                    _ = rate.insert(new_rate);
                });
            }

            Self::deposit_event(Event::SymbolsRelayed(symbols));
            Ok(().into())
        }

        /// Add `account_ids` to the list of trusted relayers.
        ///
        /// - `origin`: the sudo account on whose behalf the transaction is being executed,
        /// - `account_ids`: list of new trusted relayers to add.
        #[pallet::weight(<T as Config<I>>::WeightInfo::add_relayers())]
        pub fn add_relayers(
            origin: OriginFor<T>,
            account_ids: Vec<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            TrustedRelayers::<T, I>::mutate(|option_relayers| match option_relayers {
                Some(relayers) => relayers.extend(account_ids.clone()),
                None => _ = option_relayers.insert(BTreeSet::from_iter(account_ids.clone())),
            });

            Self::deposit_event(Event::RelayersAdded(account_ids));
            Ok(().into())
        }

        /// Remove `account_ids` from the list of trusted relayers.
        ///
        /// Ignores if some account is not presented in the list.
        ///
        /// - `origin`: the sudo account on whose behalf the transaction is being executed,
        /// - `account_ids`: list of relayers to remove.
        #[pallet::weight(<T as Config<I>>::WeightInfo::remove_relayers())]
        pub fn remove_relayers(
            origin: OriginFor<T>,
            account_ids: Vec<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            TrustedRelayers::<T, I>::mutate(|option_relayers| match option_relayers {
                Some(relayers) => {
                    for relayer in &account_ids {
                        relayers.remove(relayer);
                    }
                }
                None => _ = option_relayers.insert(BTreeSet::new()),
            });

            Self::deposit_event(Event::RelayersRemoved(account_ids));
            Ok(().into())
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    fn ensure_relayer(origin: OriginFor<T>) -> DispatchResult {
        let account_id = ensure_signed(origin)?;

        Self::trusted_relayers()
            // In Rust 1.62 can be replaced with
            // `and_then(|relayers| relayers.contains(account_id).then_some(()))`
            .and_then(|relayers| {
                if relayers.contains(&account_id) {
                    Some(())
                } else {
                    None
                }
            })
            .ok_or_else(|| Error::<T, I>::NotATrustedRelayer.into())
    }
}
