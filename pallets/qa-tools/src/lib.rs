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
    use common::{balance, Balance, PriceVariant};
    use frame_support::traits::{Get, Time};
    use frame_support::{dispatch::PostDispatchInfo, pallet_prelude::*};
    use frame_system::pallet_prelude::*;
    use sp_std::prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + order_book::Config + trading_pair::Config {
        type WeightInfo: WeightInfo;
        type OrderBookOrderLifespan: Get<<Self::Time as Time>::Moment>;
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        /// Error names should be descriptive.
        NoneValue,
        /// Errors should have helpful documentation associated with them.
        StorageOverflow,
        /// Order book does not exist for this trading pair
        OrderBookUnkonwnBook,
        /// Could not place limit order
        OrderBookFailedToPlaceOrders,
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
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            order_book_ids: Vec<OrderBookId<T::AssetId>>,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_signed(origin)?;

            Self::create_multiple_empty_unchecked(dex_id, order_book_ids)?;

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: Some(Weight::zero()),
                pays_fee: Pays::No,
            })
        }

        /// An example dispatchable that may throw a custom error.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::cause_error())]
        pub fn order_book_create_and_fill_many(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            bids_owner: T::AccountId,
            asks_owner: T::AccountId,
            fill_settings: Vec<(OrderBookId<T::AssetId>, OrderBookFillSettings)>,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_signed(origin)?;

            let order_book_ids: Vec<_> = fill_settings.iter().map(|(id, _)| id).cloned().collect();
            Self::create_multiple_empty_unchecked(dex_id, order_book_ids)?;
            Self::fill_multiple_empty_unchecked(bids_owner, asks_owner, fill_settings)?;

            // Extrinsic is only for testing, so we return all fees
            // for simplicity.
            Ok(PostDispatchInfo {
                actual_weight: Some(Weight::zero()),
                pays_fee: Pays::No,
            })
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
            let now = T::Time::now();
            let current_block = frame_system::Pallet::<T>::block_number();

            // (price, amount)
            let buy_orders = [
                (balance!(10), balance!(168.5)),
                (balance!(9.8), balance!(95.2)),
                (balance!(9.8), balance!(44.7)),
                (balance!(9.5), balance!(56.4)),
                (balance!(9.5), balance!(89.9)),
                (balance!(9.5), balance!(115)),
            ];

            // (price, amount)
            let sell_orders = [
                (balance!(11), balance!(176.3)),
                (balance!(11.2), balance!(85.4)),
                (balance!(11.2), balance!(93.2)),
                (balance!(11.5), balance!(36.6)),
                (balance!(11.5), balance!(205.5)),
                (balance!(11.5), balance!(13.7)),
            ];

            let base_amount_give: Balance = sell_orders.iter().map(|(_, base)| base).sum();
            let quote_amount_give: Balance =
                buy_orders.iter().map(|(quote, base)| quote * base).sum();

            let mut data = order_book::cache_data_layer::CacheDataLayer::<T>::new();

            for (order_book_id, settings) in fill_settings {
                let mut order_book = <order_book::OrderBooks<T>>::get(order_book_id)
                    .ok_or(Error::<T>::OrderBookUnkonwnBook)?;
                assets::Pallet::<T>::mint_unchecked(
                    &order_book_id.base,
                    &bids_owner,
                    base_amount_give,
                )?;
                assets::Pallet::<T>::mint_unchecked(
                    &order_book_id.quote,
                    &asks_owner,
                    quote_amount_give,
                )?;

                for (buy_price, buy_amount) in buy_orders {
                    let order_id = order_book.next_order_id();
                    let order = order_book::LimitOrder::<T>::new(
                        order_id,
                        asks_owner.clone(),
                        PriceVariant::Buy,
                        buy_price,
                        buy_amount,
                        now,
                        T::OrderBookOrderLifespan::get(),
                        current_block,
                    );
                    let (market_input, deal_input) =
                        order_book.place_limit_order::<order_book::Pallet<T>, order_book::Pallet<T>, order_book::Pallet<T>>(order, &mut data)?;
                    if let (None, None) = (market_input, deal_input) {
                        // should never happen
                        return Err(Error::<T>::OrderBookFailedToPlaceOrders.into());
                    }
                }
            }
            data.commit();
            Ok(())
        }
    }
}
