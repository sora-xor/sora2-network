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
#[cfg(all(test, feature = "private-net", feature = "ready-to-test"))] // order-book
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
    use frame_support::{dispatch::PostDispatchInfo, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use order_book::{MomentOf, OrderBookId};
    pub use pallet_tools::order_book::settings;
    use sp_std::prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + order_book::Config {
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
        /// The range for generating order amounts must be within order book's accepted values.
        OutOfBoundsRandomRange,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Create multiple many order books with default parameters if do not exist and
        /// fill them according to given parameters.
        ///
        /// Balance for placing the orders is minted automatically, trading pairs are
        /// created if needed.
        ///
        /// In order to create empty order books, one can leave settings empty.
        ///
        /// Parameters:
        /// - `origin`: account to mint non-divisible assets (for creating an order book)
        /// - `bids_owner`: Creator of the buy orders placed on the order books,
        /// - `asks_owner`: Creator of the sell orders placed on the order books,
        /// - `settings`: Parameters for placing the orders in each order book.
        /// `best_bid_price` should be at least 3 price steps from the lowest accepted price,
        /// and `best_ask_price` - at least 3 steps below maximum price,
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::order_book_create_and_fill_batch())]
        pub fn order_book_create_and_fill_batch(
            origin: OriginFor<T>,
            bids_owner: T::AccountId,
            asks_owner: T::AccountId,
            settings: Vec<(
                OrderBookId<T::AssetId, T::DEXId>,
                settings::OrderBookAttributes,
                settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
            )>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            // Replace with more convenient `with_pays_fee` when/if available
            // https://github.com/paritytech/substrate/pull/14470
            pallet_tools::liquidity_proxy::source_initializers::order_book::<T>(
                bids_owner, asks_owner, settings,
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
}
