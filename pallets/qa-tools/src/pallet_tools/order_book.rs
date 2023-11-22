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

use crate::settings::{RandomAmount, SideFill};
use crate::Config;
use common::{AssetInfoProvider, Balance, PriceVariant};
use frame_support::pallet_prelude::*;
use frame_support::sp_runtime::traits::Zero;
use frame_support::traits::Time;
use frame_system::pallet_prelude::*;
use order_book::DataLayer;
use order_book::{MomentOf, OrderBook, OrderBookId};
use order_book::{OrderPrice, OrderVolume};
use rand::{Rng, RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sp_std::iter::repeat;
use sp_std::prelude::*;

pub mod settings {
    use codec::{Decode, Encode};
    use common::Balance;
    use std::ops::{Range, RangeInclusive};

    /// Parameters for filling one order book side
    #[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, scale_info::TypeInfo)]
    pub struct SideFill {
        /// highest price for bids; lowest for asks
        pub best_price: Balance,
        /// lowest price for bids; highest for asks
        pub worst_price: Balance,
        pub price_step: Balance,
        pub orders_per_price: u32,
        /// Default: `min_lot_size..=max_lot_size`
        pub amount_range_inclusive: Option<RandomAmount>,
    }

    /// Parameters for orders amount generation
    #[derive(Encode, Decode, Clone, PartialEq, Eq, scale_info::TypeInfo)]
    #[cfg_attr(feature = "std", derive(Debug))]
    pub struct RandomAmount {
        min: Balance,
        max: Balance,
    }

    impl RandomAmount {
        pub fn new(min: Balance, max: Balance) -> Self {
            Self { max, min }
        }

        pub fn as_non_empty_range(&self) -> Option<Range<Balance>> {
            if self.min < self.max {
                Some(self.min..self.max)
            } else {
                None
            }
        }

        pub fn as_non_empty_inclusive_range(&self) -> Option<RangeInclusive<Balance>> {
            if self.min <= self.max {
                Some(self.min..=self.max)
            } else {
                None
            }
        }
    }

    #[derive(Encode, Decode, Clone, PartialEq, Eq, scale_info::TypeInfo)]
    #[cfg_attr(feature = "std", derive(Debug))]
    pub struct OrderBookFill<Moment, BlockNumber> {
        /// Best price = highest, worst = lowest.
        pub bids: Option<SideFill>,
        /// Best price = lowest, worst = highest.
        pub asks: Option<SideFill>,
        /// Lifespan of inserted orders, max by default.
        pub lifespan: Option<Moment>,
        /// Seed for producing random values during the fill process. If `None`,
        /// current block is chosen
        pub random_seed: Option<BlockNumber>,
    }
}

/// Does not create order books that already exist
///
/// `who` is just some account. Used to mint non-divisible assets for creating corresponding
/// order book(-s).
pub fn create_multiple_empty_unchecked<T: Config>(
    who: &T::AccountId,
    order_book_ids: Vec<OrderBookId<T::AssetId, T::DEXId>>,
) -> Result<(), DispatchError> {
    let to_create_ids: Vec<_> = order_book_ids
        .into_iter()
        .filter(|id| !<order_book::OrderBooks<T>>::contains_key(id))
        .collect();
    for order_book_id in &to_create_ids {
        if !trading_pair::Pallet::<T>::is_trading_pair_enabled(
            &order_book_id.dex_id,
            &order_book_id.quote.into(),
            &order_book_id.base.into(),
        )? {
            trading_pair::Pallet::<T>::register_pair(
                order_book_id.dex_id,
                order_book_id.quote.into(),
                order_book_id.base.into(),
            )?;
        }
        if <T as Config>::AssetInfoProvider::is_non_divisible(&order_book_id.base)
            && <T as Config>::AssetInfoProvider::total_balance(&order_book_id.base, &who)?
                == Balance::zero()
        {
            assets::Pallet::<T>::mint_unchecked(&order_book_id.base, &who, 1)?;
        }
        order_book::Pallet::<T>::verify_create_orderbook_params(who, &order_book_id)?;
    }

    for order_book_id in to_create_ids {
        order_book::Pallet::<T>::create_orderbook_unchecked(&order_book_id)?;
        order_book::Pallet::<T>::deposit_event_exposed(order_book::Event::<T>::OrderBookCreated {
            order_book_id,
            creator: who.clone(),
        });
    }
    Ok(())
}

/// Place orders into the order books.
pub fn fill_multiple_empty_unchecked<T: Config>(
    bids_owner: T::AccountId,
    asks_owner: T::AccountId,
    fill_settings: Vec<(
        OrderBookId<T::AssetId, T::DEXId>,
        settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
    )>,
) -> Result<(), DispatchError> {
    let now = <T as order_book::Config>::Time::now();
    let current_block = frame_system::Pallet::<T>::block_number();
    let mut data = order_book::cache_data_layer::CacheDataLayer::<T>::new();

    for (order_book_id, settings) in fill_settings {
        fill_order_book(
            &mut data,
            order_book_id,
            asks_owner.clone(),
            bids_owner.clone(),
            settings,
            now,
            current_block,
        )?;
    }
    data.commit();
    Ok(())
}

fn verify_fill_side_price_params<T: Config>(
    params: &SideFill,
    tick_size: OrderPrice,
) -> Result<(), DispatchError> {
    let tick = tick_size.balance();
    ensure!(
        params.price_step % tick == 0
            && params.price_step != 0
            && params.best_price % tick == 0
            && params.worst_price % tick == 0,
        crate::Error::<T>::IncorrectPrice
    );
    Ok(())
}

fn default_amount_range<T: Config>(order_book: &OrderBook<T>) -> RandomAmount {
    RandomAmount::new(
        *order_book.min_lot_size.balance(),
        *order_book.max_lot_size.balance(),
    )
}

/// Fill a single order book.
fn fill_order_book<T: Config>(
    data: &mut impl DataLayer<T>,
    book_id: OrderBookId<T::AssetId, T::DEXId>,
    asks_owner: T::AccountId,
    bids_owner: T::AccountId,
    settings: settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
    now: MomentOf<T>,
    current_block: BlockNumberFor<T>,
) -> Result<(), DispatchError> {
    let lifespan = settings
        .lifespan
        .unwrap_or(<T as order_book::Config>::MAX_ORDER_LIFESPAN);
    let mut order_book = <order_book::OrderBooks<T>>::get(book_id)
        .ok_or(crate::Error::<T>::CannotFillUnknownOrderBook)?;

    let seed = settings.random_seed.unwrap_or(current_block);
    let seed = <BlockNumberFor<T> as TryInto<u64>>::try_into(seed).unwrap_or(0);
    let mut seed_generator = ChaCha8Rng::seed_from_u64(seed);
    // we create separate RNGs seeded for each value in order to have random as independent from
    // other values as possible.
    // E.g. choosing to generate bids should not affect amounts of asks.
    let mut buy_amount_generator = ChaCha8Rng::seed_from_u64(seed_generator.next_u64());
    let mut sell_amount_generator = ChaCha8Rng::seed_from_u64(seed_generator.next_u64());

    if let Some(bids_settings) = settings.bids {
        verify_fill_side_price_params::<T>(&bids_settings, order_book.tick_size)?;
        // price_step is checked to be non-zero in `verify_fill_side_params`
        let buy_prices = (0..)
            .map(|step| bids_settings.best_price - step * bids_settings.price_step)
            .take_while(|price| *price >= bids_settings.worst_price);
        let buy_amount_non_empty_range = bids_settings
            .amount_range_inclusive
            .clone()
            .unwrap_or_else(|| default_amount_range(&order_book))
            .as_non_empty_inclusive_range()
            .ok_or(crate::Error::<T>::EmptyRandomRange)?;
        let buy_orders: Vec<_> = buy_prices
            .flat_map(|price| {
                repeat(OrderPrice::divisible(price)).take(bids_settings.orders_per_price as usize)
            })
            .map(|price| {
                let random_amount = order_book.step_lot_size.copy_divisibility(
                    buy_amount_generator.gen_range(buy_amount_non_empty_range.clone()),
                );
                (price, order_book.align_amount(random_amount))
            })
            .collect();

        // Total amount of assets to be locked
        let buy_quote_locked: Balance = buy_orders
            .iter()
            .map(|(quote, base)| *(*quote * (*base)).balance())
            .sum();
        // mint required amount to make this extrinsic self-sufficient
        assets::Pallet::<T>::mint_unchecked(&book_id.quote, &bids_owner, buy_quote_locked)?;

        // place buy orders
        place_multiple_orders(
            data,
            &mut order_book,
            bids_owner.clone(),
            PriceVariant::Buy,
            buy_orders.into_iter(),
            now,
            lifespan,
            current_block,
        )?;
    }

    if let Some(asks_settings) = settings.asks {
        verify_fill_side_price_params::<T>(&asks_settings, order_book.tick_size)?;
        // price_step is checked to be non-zero in `verify_fill_side_params`
        let sell_prices = (0..)
            .map(|step| asks_settings.best_price + step * asks_settings.price_step)
            .take_while(|price| *price <= asks_settings.worst_price);

        let sell_amount_non_empty_range = asks_settings
            .amount_range_inclusive
            .clone()
            .unwrap_or_else(|| default_amount_range(&order_book))
            .as_non_empty_inclusive_range()
            .ok_or(crate::Error::<T>::EmptyRandomRange)?;
        let sell_orders: Vec<_> = sell_prices
            .flat_map(|price| {
                repeat(OrderPrice::divisible(price)).take(asks_settings.orders_per_price as usize)
            })
            .map(|price| {
                let random_amount = order_book.step_lot_size.copy_divisibility(
                    sell_amount_generator.gen_range(sell_amount_non_empty_range.clone()),
                );
                (price, order_book.align_amount(random_amount))
            })
            .collect();

        // Total amount of assets to be locked
        let sell_base_locked: Balance = sell_orders.iter().map(|(_, base)| *base.balance()).sum();
        // mint required amount to make this extrinsic self-sufficient
        assets::Pallet::<T>::mint_unchecked(&book_id.base, &asks_owner, sell_base_locked)?;

        // place sell orders
        place_multiple_orders(
            data,
            &mut order_book,
            asks_owner.clone(),
            PriceVariant::Sell,
            sell_orders.into_iter(),
            now,
            lifespan,
            current_block,
        )?;
    }

    <order_book::OrderBooks<T>>::insert(book_id, order_book);
    Ok(())
}

fn place_multiple_orders<T: Config>(
    data: &mut impl DataLayer<T>,
    book: &mut OrderBook<T>,
    owner: T::AccountId,
    side: PriceVariant,
    orders: impl Iterator<Item = (OrderPrice, OrderVolume)>,
    time: MomentOf<T>,
    lifespan: MomentOf<T>,
    current_block: BlockNumberFor<T>,
) -> Result<(), DispatchError> {
    for (price, amount) in orders {
        let order_id = book.next_order_id();
        let order = order_book::LimitOrder::<T>::new(
            order_id,
            owner.clone(),
            side,
            price,
            amount,
            time,
            lifespan,
            current_block,
        );
        book.place_limit_order(order, data)?;
    }
    Ok(())
}
