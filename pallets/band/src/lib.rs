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

use common::prelude::FixedWrapper;
use common::{Balance, DataFeed, Fixed, OnNewSymbolsRelayed, Oracle, Rate};
use frame_support::pallet_prelude::*;
use frame_support::sp_runtime::SaturatedConversion;
use frame_support::traits::UnixTime;
use frame_support::weights::Weight;
use frame_system::pallet_prelude::*;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::prelude::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

pub mod weights;

/// Multiplier to convert rates from precision = 9 (which band team use)
/// to precision = 18 (which we use)
pub const RATE_MULTIPLIER: i128 = 1_000_000_000;

pub trait WeightInfo {
    fn relay() -> Weight;
    fn force_relay() -> Weight;
    fn add_relayers() -> Weight;
    fn remove_relayers() -> Weight;
}

/// Symbol rate
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Copy, Clone, PartialEq, Eq)]
pub struct BandRate {
    /// Rate value in USD.
    pub value: Balance,
    /// Last updated timestamp.
    pub last_updated: u64,
    /// Request identifier in the *Band* protocol.
    /// Useful for debugging and in emergency cases.
    pub request_id: u64,
}

impl BandRate {
    pub fn update_if_outdated(&mut self, new: BandRate) {
        if self.last_updated <= new.last_updated {
            *self = new;
        }
    }
}

impl From<BandRate> for Rate {
    fn from(value: BandRate) -> Rate {
        Rate {
            value: value.value,
            last_updated: value.last_updated,
        }
    }
}

pub use pallet::*;

impl<T: Config<I>, I: 'static> DataFeed<T::Symbol, Rate, u64> for Pallet<T, I> {
    fn quote(symbol: &T::Symbol) -> Result<Option<Rate>, DispatchError> {
        let rate = if let Some(rate) = Self::rates(symbol) {
            rate
        } else {
            return Ok(None);
        };

        let current_time = T::UnixTime::now().as_millis().saturated_into::<u64>();
        let stale_period = T::GetBandRateStalePeriod::get();
        let current_period = current_time
            .checked_sub(rate.last_updated)
            .ok_or(Error::<T, I>::RateHasInvalidTimestamp)?;

        ensure!(current_period < stale_period, Error::<T, I>::RateExpired);

        Ok(Some(rate.into()))
    }

    fn list_enabled_symbols() -> Result<Vec<(T::Symbol, u64)>, DispatchError> {
        Ok(Vec::from_iter(SymbolRates::<T, I>::iter().filter_map(
            |item| match item {
                (symbol, Some(rate)) => Some((symbol, rate.last_updated)),
                _ => None,
            },
        )))
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use sp_std::collections::btree_set::BTreeSet;

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
        type Symbol: Parameter + Ord;
        /// Event type of this pallet.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
        /// Hook which is being executed when some new symbols were relayed
        type OnNewSymbolsRelayedHook: OnNewSymbolsRelayed<Self::Symbol>;
        /// Rate expiration period in seconds.
        #[pallet::constant]
        type GetBandRateStalePeriod: Get<u64>;
        /// Time used for checking if rate expired
        type UnixTime: UnixTime;
    }

    #[pallet::storage]
    #[pallet::getter(fn trusted_relayers)]
    pub type TrustedRelayers<T: Config<I>, I: 'static = ()> =
        StorageValue<_, BTreeSet<T::AccountId>>;

    #[pallet::storage]
    #[pallet::getter(fn rates)]
    pub type SymbolRates<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::Symbol, Option<BandRate>, ValueQuery>;

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
        UnauthorizedRelayer,
        /// A request to add an account, which is already a trusted relayer, was supplied.
        AlreadyATrustedRelayer,
        /// A request to remove an account, which is not a trusted relayer, was supplied.
        NoSuchRelayer,
        /// Relayed rate is too big to be stored in the pallet.
        RateConversionOverflow,
        /// Rate has invalid timestamp.
        RateHasInvalidTimestamp,
        /// Rate is expired and can't be used until next update.
        RateExpired,
    }

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {
        /// Relay a list of symbols and their associated rates along with the resolve time and request id on `BandChain`.
        ///
        /// Checks if:
        /// - The caller is a relayer;
        /// - The `resolve_time` for a particular symbol is not lower than previous saved value, ignores this rate if so;
        ///
        /// If `rates` contains duplicated symbols, then the last rate will be stored.
        ///
        /// - `origin`: the relayer account on whose behalf the transaction is being executed,
        /// - `rates`: symbols with rates in USD represented as fixed point with precision = 9,
        /// - `resolve_time`: symbols which rates are provided,
        /// - `request_id`: id of the request sent to the *BandChain* to retrieve this data.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::relay())]
        pub fn relay(
            origin: OriginFor<T>,
            rates: Vec<(T::Symbol, u64)>,
            resolve_time: u64,
            request_id: u64,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_relayer(origin)?;

            let symbols = Self::update_rates(
                rates,
                resolve_time,
                request_id,
                |option_old_rate, new_rate| match option_old_rate {
                    Some(rate) => rate.update_if_outdated(new_rate),
                    None => _ = option_old_rate.insert(new_rate),
                },
            )?;

            Self::deposit_event(Event::SymbolsRelayed(symbols));
            Ok(().into())
        }

        /// Similar to [`relay()`] but without the resolve time guard.
        ///
        /// Should be used in emergency situations i.e. then previous value was
        /// relayed by a faulty/malicious actor.
        ///
        /// - `origin`: the relayer account on whose behalf the transaction is being executed,
        /// - `rates`: symbols with rates in USD represented as fixed point with precision = 9,
        /// - `resolve_time`: symbols which rates are provided,
        /// - `request_id`: id of the request sent to the *BandChain* to retrieve this data.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::force_relay())]
        pub fn force_relay(
            origin: OriginFor<T>,
            rates: Vec<(T::Symbol, u64)>,
            resolve_time: u64,
            request_id: u64,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_relayer(origin)?;

            let symbols: Vec<_> = Self::update_rates(
                rates,
                resolve_time,
                request_id,
                |option_old_rate, new_rate| {
                    let _ = option_old_rate.insert(new_rate);
                },
            )?;

            Self::deposit_event(Event::SymbolsRelayed(symbols));
            Ok(().into())
        }

        /// Add `account_ids` to the list of trusted relayers.
        ///
        /// Ignores repeated accounts in `account_ids`.
        /// If one of account is already a trusted relayer an [`Error::AlreadyATrustedRelayer`] will
        /// be returned.
        ///
        /// - `origin`: the sudo account on whose behalf the transaction is being executed,
        /// - `account_ids`: list of new trusted relayers to add.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::add_relayers())]
        pub fn add_relayers(
            origin: OriginFor<T>,
            account_ids: Vec<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let added_accounts =
                TrustedRelayers::<T, I>::mutate(|option_relayers| match option_relayers {
                    Some(relayers) => {
                        let to_add = BTreeSet::from_iter(account_ids);

                        if relayers.is_disjoint(&to_add) {
                            relayers.append(&mut to_add.clone());
                            Ok(to_add)
                        } else {
                            Err(Error::<T, I>::AlreadyATrustedRelayer)
                        }
                    }
                    None => {
                        let to_add = BTreeSet::from_iter(account_ids);
                        let _ = option_relayers.insert(to_add.clone());
                        Ok(to_add)
                    }
                })?;

            Self::deposit_event(Event::RelayersAdded(added_accounts.into_iter().collect()));
            Ok(().into())
        }

        /// Remove `account_ids` from the list of trusted relayers.
        ///
        /// Ignores repeated accounts in `account_ids`.
        /// If one of account is not a trusted relayer an [`Error::AlreadyATrustedRelayer`] will
        /// be returned.
        ///
        /// - `origin`: the sudo account on whose behalf the transaction is being executed,
        /// - `account_ids`: list of relayers to remove.
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::remove_relayers())]
        pub fn remove_relayers(
            origin: OriginFor<T>,
            account_ids: Vec<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let removed_accounts =
                TrustedRelayers::<T, I>::mutate(|option_relayers| match option_relayers {
                    Some(relayers) => {
                        let to_remove = BTreeSet::from_iter(account_ids);
                        if to_remove.is_subset(&relayers) {
                            for account in &to_remove {
                                relayers.remove(account);
                            }
                            Ok(to_remove)
                        } else {
                            Err(Error::<T, I>::NoSuchRelayer)
                        }
                    }
                    None => {
                        let _ = option_relayers.insert(BTreeSet::new());
                        if account_ids.is_empty() {
                            Ok(BTreeSet::new())
                        } else {
                            Err(Error::<T, I>::NoSuchRelayer)
                        }
                    }
                })?;

            Self::deposit_event(Event::RelayersRemoved(
                removed_accounts.into_iter().collect(),
            ));
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
            .ok_or_else(|| Error::<T, I>::UnauthorizedRelayer.into())
    }

    /// Update rates in the storage with the new ones.
    ///
    /// `f` - mutation function which defines the way values should be updated.
    fn update_rates(
        rates: Vec<(T::Symbol, u64)>,
        resolve_time: u64,
        request_id: u64,
        f: impl Fn(&mut Option<BandRate>, BandRate),
    ) -> Result<Vec<T::Symbol>, DispatchError> {
        let mut symbols = Vec::with_capacity(rates.len());
        let mut new_symbols = BTreeSet::new();
        for (symbol, rate_value) in rates {
            let new_rate = BandRate {
                value: Self::raw_rate_into_balance(rate_value)?,
                last_updated: resolve_time,
                request_id,
            };

            SymbolRates::<T, I>::mutate(&symbol, |option_old_rate| {
                if option_old_rate.is_none() {
                    new_symbols.insert(symbol.clone());
                }
                f(option_old_rate, new_rate);
            });
            symbols.push(symbol);
        }
        T::OnNewSymbolsRelayedHook::on_new_symbols_relayed(Oracle::BandChainFeed, new_symbols)?;

        Ok(symbols)
    }

    pub fn raw_rate_into_balance(raw_rate: u64) -> Result<Balance, DispatchError> {
        i128::from(raw_rate)
            .checked_mul(RATE_MULTIPLIER)
            .and_then(|value| {
                let fixed = Fixed::from_bits(value);
                FixedWrapper::from(fixed).try_into_balance().ok()
            })
            .ok_or_else(|| Error::<T, I>::RateConversionOverflow.into())
    }
}
