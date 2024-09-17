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
use common::{fixed, fixed_wrapper, Balance, DataFeed, Fixed, OnNewSymbolsRelayed, Oracle, Rate};
use fallible_iterator::FallibleIterator;
use frame_support::pallet_prelude::*;
use frame_support::traits::Time;
use frame_system::pallet_prelude::*;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

pub mod migrations;

pub mod weights;

/// Multiplier to convert rates from precision = 9 (which band team use)
/// to precision = 18 (which we use)
pub const RATE_MULTIPLIER: i128 = 1_000_000_000;

/// Multiplier to convert rate last_update timestamp to Moment
pub const MILLISECS_MULTIPLIER: u64 = 1_000;

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq, Eq)]
pub struct FeeCalculationParameters {
    pub decay: Fixed,
    pub min_fee: Fixed,
    pub deviation: Fixed,
}

pub use pallet::*;

impl FeeCalculationParameters {
    pub fn validate<T: Config<I>, I: 'static>(&self) -> Result<(), DispatchError> {
        ensure!(
            self.decay >= fixed!(0) && self.decay < fixed!(1),
            Error::<T, I>::InvalidDynamicFeeParameters
        );
        ensure!(
            self.min_fee >= fixed!(0),
            Error::<T, I>::InvalidDynamicFeeParameters
        );
        ensure!(
            self.deviation >= fixed!(0),
            Error::<T, I>::InvalidDynamicFeeParameters
        );
        Ok(())
    }

    pub fn new(decay: Fixed, min_fee: Fixed, deviation: Fixed) -> Self {
        Self {
            decay,
            min_fee,
            deviation,
        }
    }
}

/// Symbol rate
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Copy, Clone, PartialEq, Eq)]
pub struct BandRate<BlockNumber> {
    /// Rate value in USD.
    pub value: Balance,
    /// Last updated timestamp.
    pub last_updated: u64,
    /// Last updated block number
    pub last_updated_block: BlockNumber,
    /// Request identifier in the *Band* protocol.
    /// Useful for debugging and in emergency cases.
    pub request_id: u64,
    /// Current dynamic fee for the symbol
    pub dynamic_fee: Fixed,
}

impl<BlockNumber> From<BandRate<BlockNumber>> for Rate {
    fn from(value: BandRate<BlockNumber>) -> Rate {
        Rate {
            value: value.value,
            last_updated: value.last_updated,
            dynamic_fee: value.dynamic_fee,
        }
    }
}

impl<T: Config<I>, I: 'static> DataFeed<T::Symbol, Rate, u64> for Pallet<T, I> {
    fn quote(symbol: &T::Symbol) -> Result<Option<Rate>, DispatchError> {
        let rate = if let Some(rate) = Self::rates(symbol) {
            rate
        } else {
            return Ok(None);
        };

        let current_time = T::Time::now();
        let stale_period = T::GetBandRateStalePeriod::get();
        // could not convert u64 to Moment directly, this workaround solves this
        let last_updated = rate
            .last_updated
            .saturating_mul(MILLISECS_MULTIPLIER)
            .try_into()
            .map_err(|_| "Can't cast u64 to <<T as Config<I>>::Time as Time>::Moment")
            .unwrap();

        ensure!(
            last_updated <= current_time,
            Error::<T, I>::RateHasInvalidTimestamp
        );

        let current_period = current_time - last_updated;

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

    fn quote_unchecked(symbol: &T::Symbol) -> Option<Rate> {
        Self::rates(symbol).map(|band_rate| band_rate.into())
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::OnSymbolDisabled;
    use sp_std::collections::btree_set::BTreeSet;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

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
    #[pallet::storage_version(STORAGE_VERSION)]
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
        type GetBandRateStalePeriod: Get<<<Self as pallet::Config<I>>::Time as Time>::Moment>;
        /// Rate expiration period in blocks
        #[pallet::constant]
        type GetBandRateStaleBlockPeriod: Get<BlockNumberFor<Self>>;
        /// Maximum number of symbols that can be relayed within a single call.
        #[pallet::constant]
        type MaxRelaySymbols: Get<u32>;
        /// Time used for checking if rate expired
        type Time: Time;
        /// Hook which is being executed when some symbol must be disabled
        type OnSymbolDisabledHook: OnSymbolDisabled<Self::Symbol>;
    }

    #[pallet::storage]
    #[pallet::getter(fn trusted_relayers)]
    pub type TrustedRelayers<T: Config<I>, I: 'static = ()> =
        StorageValue<_, BTreeSet<T::AccountId>>;

    #[pallet::storage]
    #[pallet::getter(fn rates)]
    pub type SymbolRates<T: Config<I>, I: 'static = ()> =
        StorageMap<_, Blake2_128Concat, T::Symbol, Option<BandRate<BlockNumberFor<T>>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn check_blocks)]
    pub type SymbolCheckBlock<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        BlockNumberFor<T>,
        Blake2_128Concat,
        T::Symbol,
        bool,
        ValueQuery,
    >;

    #[pallet::type_value]
    pub fn DefaultDynamicFeeParameters<T: Config<I>, I: 'static>() -> FeeCalculationParameters {
        FeeCalculationParameters {
            decay: fixed!(0),
            min_fee: fixed!(1),
            deviation: fixed!(0),
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn dynamic_fee_parameters)]
    pub type DynamicFeeParameters<T: Config<I>, I: 'static = ()> =
        StorageValue<_, FeeCalculationParameters, ValueQuery, DefaultDynamicFeeParameters<T, I>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// New symbol rates were successfully relayed. [symbols]
        SymbolsRelayed(Vec<(T::Symbol, Balance)>),
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
        /// Error during dynamic fee calculation
        DynamicFeeCalculationError,
        /// Dynamic fee parameters are invalid,
        InvalidDynamicFeeParameters,
    }

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            let mut weight = Weight::zero();
            let mut obsolete_symbols: Vec<T::Symbol> = Vec::new();
            for (symbol, _) in SymbolCheckBlock::<T, I>::iter_prefix(now) {
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
                T::OnSymbolDisabledHook::disable_symbol(&symbol);
                obsolete_symbols.push(symbol);
            }
            for symbol in obsolete_symbols.iter() {
                SymbolCheckBlock::<T, I>::remove(now, symbol);
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(0, 1));
            }
            weight
        }
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
            rates: BoundedVec<(T::Symbol, u64), T::MaxRelaySymbols>,
            resolve_time: u64,
            request_id: u64,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_relayer(origin)?;

            let symbol_rates = Self::update_rates(
                rates,
                resolve_time,
                request_id,
                |option_old_rate, new_rate, symbol| match option_old_rate {
                    Some(rate) => Self::update_rate_if_outdated(rate, new_rate, symbol),
                    None => {
                        let last_updated_block = new_rate.last_updated_block;
                        _ = option_old_rate.insert(new_rate);
                        SymbolCheckBlock::<T, I>::insert(
                            Self::calc_expiration_block(last_updated_block),
                            symbol,
                            true,
                        );
                        Ok(())
                    }
                },
            )?;

            Self::deposit_event(Event::SymbolsRelayed(symbol_rates));
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
            rates: BoundedVec<(T::Symbol, u64), T::MaxRelaySymbols>,
            resolve_time: u64,
            request_id: u64,
        ) -> DispatchResultWithPostInfo {
            Self::ensure_relayer(origin)?;

            let symbol_rates: Vec<_> = Self::update_rates(
                rates,
                resolve_time,
                request_id,
                |option_old_rate, new_rate, symbol| {
                    if let Some(rate) = option_old_rate {
                        SymbolCheckBlock::<T, I>::remove(
                            Self::calc_expiration_block(rate.last_updated_block),
                            symbol,
                        );
                    }
                    let last_updated_block = new_rate.last_updated_block;
                    _ = option_old_rate.insert(new_rate);
                    SymbolCheckBlock::<T, I>::insert(
                        Self::calc_expiration_block(last_updated_block),
                        symbol,
                        true,
                    );
                    Ok(())
                },
            )?;

            Self::deposit_event(Event::SymbolsRelayed(symbol_rates));
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
                        if to_remove.is_subset(relayers) {
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

        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config<I>>::WeightInfo::set_dynamic_fee_parameters())]
        pub fn set_dynamic_fee_parameters(
            origin: OriginFor<T>,
            fee_parameters: FeeCalculationParameters,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            fee_parameters.validate::<T, I>()?;
            DynamicFeeParameters::<T, I>::put(fee_parameters);
            Ok(().into())
        }
    }
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
    fn ensure_relayer(origin: OriginFor<T>) -> DispatchResult {
        let account_id = ensure_signed(origin)?;

        Self::trusted_relayers()
            .and_then(|relayers| relayers.contains(&account_id).then_some(()))
            .ok_or_else(|| Error::<T, I>::UnauthorizedRelayer.into())
    }

    /// Update rates in the storage with the new ones.
    ///
    /// `f` - mutation function which defines the way values should be updated.
    fn update_rates(
        rates: BoundedVec<(T::Symbol, u64), T::MaxRelaySymbols>,
        resolve_time: u64,
        request_id: u64,
        f: impl Fn(
            &mut Option<BandRate<BlockNumberFor<T>>>,
            BandRate<BlockNumberFor<T>>,
            &T::Symbol,
        ) -> Result<(), DispatchError>,
    ) -> Result<Vec<(T::Symbol, Balance)>, DispatchError> {
        let converted_rates: Vec<(T::Symbol, Balance)> =
            fallible_iterator::convert(rates.into_iter().map(
                |(symbol, rate_value)| -> Result<(T::Symbol, Balance), DispatchError> {
                    let converted_rate = Self::raw_rate_into_balance(rate_value)?;
                    Ok((symbol, converted_rate))
                },
            ))
            .collect()?;
        let now = frame_system::Pallet::<T>::block_number();
        let new_symbols =
            fallible_iterator::convert(converted_rates.iter().map(Ok::<_, DispatchError>)).fold(
                BTreeSet::new(),
                |mut new_symbols_acc, (symbol, rate_value)| {
                    let new_rate = BandRate {
                        value: *rate_value,
                        last_updated: resolve_time,
                        request_id,
                        dynamic_fee: fixed!(0),
                        last_updated_block: now,
                    };
                    SymbolRates::<T, I>::mutate(symbol, |option_old_rate| {
                        if option_old_rate.is_none() {
                            new_symbols_acc.insert(symbol.clone());
                        }
                        f(option_old_rate, new_rate, symbol)
                    })?;
                    Ok(new_symbols_acc)
                },
            )?;

        T::OnNewSymbolsRelayedHook::on_new_symbols_relayed(Oracle::BandChainFeed, new_symbols)?;

        Ok(converted_rates)
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

    pub fn calculate_dynamic_fee(
        prev_fee: Fixed,
        prev_rate: Balance,
        new_rate: Balance,
    ) -> Result<Fixed, DispatchError> {
        let FeeCalculationParameters {
            decay,
            min_fee,
            deviation,
        } = Self::dynamic_fee_parameters();

        let prev_fee = FixedWrapper::from(prev_fee);
        let prev_rate = FixedWrapper::from(prev_rate);
        let new_rate = FixedWrapper::from(new_rate);

        let decay = FixedWrapper::from(decay);
        let min_fee = FixedWrapper::from(min_fee);
        let doubled_deviation = FixedWrapper::from(deviation) * fixed_wrapper!(2);

        let decayed_fee = prev_fee * decay;

        let new_fee_part = new_rate / prev_rate - fixed_wrapper!(1) - doubled_deviation - min_fee;
        let new_fee = if new_fee_part > fixed_wrapper!(0) {
            decayed_fee + new_fee_part
        } else {
            decayed_fee
        };

        new_fee
            .get()
            .map_err(|_| Error::<T, I>::DynamicFeeCalculationError.into())
            .map(|fee| if fee >= fixed!(1) { fixed!(1) } else { fee })
    }

    pub fn update_rate_if_outdated(
        rate: &mut BandRate<BlockNumberFor<T>>,
        new_rate: BandRate<BlockNumberFor<T>>,
        symbol: &T::Symbol,
    ) -> Result<(), DispatchError> {
        if rate.last_updated <= new_rate.last_updated {
            SymbolCheckBlock::<T, I>::remove(
                Self::calc_expiration_block(rate.last_updated_block),
                symbol,
            );
            let dynamic_fee =
                Self::calculate_dynamic_fee(rate.dynamic_fee, rate.value, new_rate.value)?;
            *rate = new_rate;
            rate.dynamic_fee = dynamic_fee;
            SymbolCheckBlock::<T, I>::insert(
                Self::calc_expiration_block(rate.last_updated_block),
                symbol,
                true,
            );
        }
        Ok(())
    }

    pub fn calc_expiration_block(block_number: BlockNumberFor<T>) -> BlockNumberFor<T> {
        block_number + T::GetBandRateStaleBlockPeriod::get()
    }
}
