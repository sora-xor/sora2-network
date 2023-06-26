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

// #[cfg(feature = "std")]
// use serde::{Deserialize, Serialize};

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
    use sp_std::prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + order_book::Config + trading_pair::Config {
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

    #[derive(
        Encode,
        Decode,
        Eq,
        PartialEq,
        Copy,
        Clone,
        PartialOrd,
        Ord,
        RuntimeDebug,
        Hash,
        scale_info::TypeInfo,
        MaxEncodedLen,
    )]
    // #[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
    pub struct OrderBookFillSettings {
        best_bid_price: order_book::types::OrderPrice,
        best_ask_price: order_book::types::OrderPrice,
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
        pub fn order_book_create_empty_many(
            _origin: OriginFor<T>,
            dex_id: T::DEXId,
            order_book_ids: Vec<OrderBookId<T::AssetId>>,
        ) -> DispatchResult {
            // Extrinsic is only for testing, so any origin is allowed.
            // It also allows not to worry about fees.

            Self::create_multiple_empty_unchecked(dex_id, order_book_ids)?;
            Ok(())
        }

        /// An example dispatchable that may throw a custom error.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::cause_error())]
        pub fn order_book_create_and_fill_many(
            _origin: OriginFor<T>,
            dex_id: T::DEXId,
            bids_owner: T::AccountId,
            asks_owner: T::AccountId,
            fill_settings: Vec<(OrderBookId<T::AssetId>, OrderBookFillSettings)>,
        ) -> DispatchResult {
            // Extrinsic is only for testing, so any origin is allowed.
            // It also allows not to worry about fees.

            let order_book_ids: Vec<_> = fill_settings.iter().map(|(id, _)| id).cloned().collect();
            Self::create_multiple_empty_unchecked(dex_id, order_book_ids)?;
            Self::fill_multiple_empty_unchecked(bids_owner, asks_owner, fill_settings)?;
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn create_multiple_empty_unchecked(
            dex_id: T::DEXId,
            order_book_ids: Vec<OrderBookId<T::AssetId>>,
        ) -> Result<(), DispatchError> {
            for order_book_id in &order_book_ids {
                if !trading_pair::Pallet::<T>::is_trading_pair_enabled(
                    &dex_id,
                    &order_book_id.quote.into(),
                    &order_book_id.base.into(),
                )? {
                    trading_pair::Pallet::<T>::register_pair(
                        dex_id,
                        order_book_id.quote.into(),
                        order_book_id.base.into(),
                    )?;
                }
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

        fn fill_multiple_empty_unchecked(
            bids_owner: T::AccountId,
            asks_owner: T::AccountId,
            fill_settings: Vec<(OrderBookId<T::AssetId>, OrderBookFillSettings)>,
        ) -> Result<(), DispatchError> {
            // assets::Pallet::<T>::update_balance(
            //     RuntimeOrigin::root(),
            //     account.clone(),
            //     order_book_id.base,
            //     INIT_BALANCE.try_into().unwrap()
            // )?;

            // let lifespan = Some(100000);

            // // prices
            // let bp1 = balance!(10);
            // let bp2 = balance!(9.8);
            // let bp3 = balance!(9.5);
            // let sp1 = balance!(11);
            // let sp2 = balance!(11.2);
            // let sp3 = balance!(11.5);

            // // buy amounts
            // let amount1 = balance!(168.5);
            // let amount2 = balance!(95.2);
            // let amount3 = balance!(44.7);
            // let amount4 = balance!(56.4);
            // let amount5 = balance!(89.9);
            // let amount6 = balance!(115);

            // // sell amounts
            // let amount7 = balance!(176.3);
            // let amount8 = balance!(85.4);
            // let amount9 = balance!(93.2);
            // let amount10 = balance!(36.6);
            // let amount11 = balance!(205.5);
            // let amount12 = balance!(13.7);
            Ok(())
        }
    }
}
