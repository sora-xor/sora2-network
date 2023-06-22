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

pub use pallet::*;

#[cfg(test)]
mod tests;

pub mod weights;
use common::AssetInfoProvider;
use order_book::OrderBookId;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config + order_book::Config {
        /// Type representing the weight of this pallet
        type WeightInfo: WeightInfo;
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        /// Error names should be descriptive.
        NoneValue,
        /// Errors should have helpful documentation associated with them.
        StorageOverflow,
    }

    // Dispatchable functions allows users to interact with the pallet and invoke state changes.
    // These functions materialize as "extrinsics", which are often compared to transactions.
    // Dispatchable functions must be annotated with a weight and must return a DispatchResult.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// An example dispatchable that takes a singles value as a parameter, writes the value to
        /// storage and emits an event. This function must be dispatched by a signed extrinsic.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::do_something())]
        pub fn create_empty_order_books(
            _origin: OriginFor<T>,
            dex_id: T::DEXId,
            order_book_ids: Vec<OrderBookId<T::AssetId>>,
        ) -> DispatchResult {
            // Extrinsic only for testing, so any origin is allowed.
            // It also allows not to worry about fees.

            for order_book_id in &order_book_ids {
                order_book::Pallet::<T>::verify_create_orderbook_params(&dex_id, order_book_id)?;
            }

            for order_book_id in order_book_ids {
                let order_book = if T::AssetInfoProvider::is_non_divisible(&order_book_id.base) {
                    order_book::OrderBook::<T>::default_nft(order_book_id, dex_id)
                } else {
                    order_book::OrderBook::<T>::default(order_book_id, dex_id)
                };

                #[cfg(feature = "wip")] // order-book
                {
                    T::TradingPairSourceManager::enable_source_for_trading_pair(
                        &dex_id,
                        &order_book_id.quote,
                        &order_book_id.base,
                        LiquiditySourceType::OrderBook,
                    )?;
                }

                <order_book::OrderBooks<T>>::insert(order_book_id, order_book);
                order_book::Pallet::<T>::register_tech_account(dex_id, order_book_id)?;
            }
            Ok(())
        }

        /// An example dispatchable that may throw a custom error.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::cause_error())]
        pub fn cause_error(origin: OriginFor<T>) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // // Read a value from storage.
            // match <Something<T>>::get() {
            //     // Return an error if the value has not been set.
            //     None => return Err(Error::<T>::NoneValue.into()),
            //     Some(old) => {
            //         // Increment the value read from storage; will error in the event of overflow.
            //         let new = old.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
            //         // Update the value in storage with the incremented result.
            //         <Something<T>>::put(new);
            Ok(())
            //     }
            // }
        }
    }
}
