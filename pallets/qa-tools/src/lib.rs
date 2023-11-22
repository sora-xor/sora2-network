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

pub use pallet::*;

// private-net to make circular dependencies work
#[cfg(all(test, feature = "private-net", feature = "wip"))] // order-book
mod tests;
pub mod weights;
pub use weights::*;
mod pallet_tools;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{
        AssetInfoProvider, AssetName, AssetSymbol, BalancePrecision, ContentSource, Description,
    };
    use frame_support::dispatch::DispatchErrorWithPostInfo;
    use frame_support::sp_runtime::{traits::BadOrigin, BoundedBTreeSet};
    use frame_support::{dispatch::PostDispatchInfo, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;
    use order_book::{MomentOf, OrderBookId};
    pub use pallet_tools::order_book::settings;
    use sp_std::prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + order_book::Config + trading_pair::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type WeightInfo: WeightInfo;
        type AssetInfoProvider: AssetInfoProvider<
            Self::AssetId,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;
        type QaToolsWhitelistCapacity: Get<u32>;
    }

    /// In order to prevent breaking testnets/staging with such zero-weight
    /// extrinsics from this pallet, we restrict `origin`s to root and trusted
    /// list of accounts (added by root).
    #[pallet::storage]
    #[pallet::getter(fn whitelisted_callers)]
    pub type WhitelistedCallers<T: Config> =
        StorageValue<_, BoundedBTreeSet<T::AccountId, T::QaToolsWhitelistCapacity>>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AddedToWhitelist { user: T::AccountId },
        RemovedFromWhitelist { user: T::AccountId },
    }

    #[pallet::error]
    pub enum Error<T> {
        // this pallet errors
        /// Cannot add an account to the whitelist: it's full
        WhitelistFull,
        /// The account is already in the whitelist
        AlreadyInWhitelist,
        /// The account intended for removal is not in whitelist
        NotInWhitelist,

        // order_book pallet errors
        /// Did not find an order book with given id to fill. Likely an error with
        /// order book creation.
        CannotFillUnknownOrderBook,
        /// Price step, best price, and worst price must be a multiple of
        /// order book's tick size. Price step must also be non-zero.
        IncorrectPrice,
        /// Provided range is incorrect, check that lower bound is less or equal than the upper one.
        EmptyRandomRange,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Add the account to the list of allowed callers.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::order_book_create_empty_batch())]
        pub fn add_to_whitelist(
            origin: OriginFor<T>,
            account: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            WhitelistedCallers::<T>::mutate(|option_whitelist| {
                let whitelist = match option_whitelist {
                    Some(w) => w,
                    None => option_whitelist.insert(BoundedBTreeSet::new()),
                };
                whitelist
                    .try_insert(account.clone())
                    .map_err(|_| Error::<T>::WhitelistFull)?
                    .then_some(())
                    .ok_or(Error::<T>::AlreadyInWhitelist)?;
                Ok::<(), Error<T>>(())
            })?;
            Self::deposit_event(Event::<T>::AddedToWhitelist { user: account });
            Ok(().into())
        }

        /// Remove the account from the list of allowed callers.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::order_book_create_empty_batch())]
        pub fn remove_from_whitelist(
            origin: OriginFor<T>,
            account: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            WhitelistedCallers::<T>::mutate(|option_whitelist| {
                let whitelist = match option_whitelist {
                    Some(w) => w,
                    None => option_whitelist.insert(BoundedBTreeSet::new()),
                };
                let was_not_whitelisted = !whitelist.remove(&account);
                if was_not_whitelisted {
                    Err(Error::<T>::NotInWhitelist)
                } else {
                    Ok::<(), Error<T>>(())
                }
            })?;
            Self::deposit_event(Event::<T>::RemovedFromWhitelist { user: account });
            Ok(().into())
        }

        /// Create multiple order books with default parameters (if do not exist yet).
        ///
        /// Parameters:
        /// - `origin`: caller, should be account because error messages for unsigned txs are unclear,
        /// - `order_book_ids`: ids of the created order books; trading pairs are created
        /// if necessary,
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::order_book_create_empty_batch())]
        pub fn order_book_create_empty_batch(
            origin: OriginFor<T>,
            order_book_ids: Vec<OrderBookId<T::AssetId, T::DEXId>>,
        ) -> DispatchResultWithPostInfo {
            let who = Self::ensure_in_whitelist(origin)?;

            // replace with more convenient `with_pays_fee` when/if available
            // https://github.com/paritytech/substrate/pull/14470
            pallet_tools::order_book::create_multiple_empty_unchecked::<T>(&who, order_book_ids)
                .map_err(|e| DispatchErrorWithPostInfo {
                    post_info: PostDispatchInfo {
                        actual_weight: None,
                        pays_fee: Pays::No,
                    },
                    error: e,
                })?;

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Initialize order book liquidity source. Create multiple many order books with default
        /// parameters if do not exist and fill them according to given parameters.
        ///
        /// Balance for placing the orders is minted automatically, trading pairs are
        /// created if needed.
        ///
        /// Parameters:
        /// - `origin`: account to mint non-divisible assets (for creating an order book)
        /// - `bids_owner`: Creator of the buy orders placed on the order books,
        /// - `asks_owner`: Creator of the sell orders placed on the order books,
        /// - `fill_settings`: Parameters for placing the orders in each order book.
        /// `best_bid_price` should be at least 3 price steps from the lowest accepted price,
        /// and `best_ask_price` - at least 3 steps below maximum price,
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::order_book_create_and_fill_batch())]
        pub fn order_book_create_and_fill_batch(
            origin: OriginFor<T>,
            bids_owner: T::AccountId,
            asks_owner: T::AccountId,
            fill_settings: Vec<(
                OrderBookId<T::AssetId, T::DEXId>,
                settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
            )>,
        ) -> DispatchResultWithPostInfo {
            // error messages for unsigned calls are non-informative
            let who = Self::ensure_in_whitelist(origin)?;

            pallet_tools::liquidity_proxy::source_initializers::order_book::<T>(
                who,
                bids_owner,
                asks_owner,
                fill_settings,
            )
            .map_err(|e| DispatchErrorWithPostInfo {
                post_info: PostDispatchInfo {
                    actual_weight: None,
                    pays_fee: Pays::No,
                },
                error: e,
            })?;

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn ensure_in_whitelist<OuterOrigin>(
            origin: OuterOrigin,
        ) -> Result<T::AccountId, BadOrigin>
        where
            OuterOrigin: Into<Result<RawOrigin<T::AccountId>, OuterOrigin>>,
        {
            let who = match origin.into() {
                Ok(RawOrigin::Signed(w)) => w,
                _ => return Err(BadOrigin),
            };
            let Some(whitelist) = WhitelistedCallers::<T>::get() else {
                return Err(BadOrigin)
            };
            if whitelist.contains(&who) {
                Ok(who)
            } else {
                Err(BadOrigin)
            }
        }
    }
}
