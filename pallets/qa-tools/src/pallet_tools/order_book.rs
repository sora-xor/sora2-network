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

use crate::Config;
use codec::{Decode, Encode};
use common::{balance, AssetIdOf, AssetManager, Balance, PriceVariant, TradingPairSourceManager};
use frame_support::pallet_prelude::*;
use frame_support::traits::Time;
use frame_system::pallet_prelude::*;
use order_book::DataLayer;
use order_book::{MomentOf, OrderBook, OrderBookId, OrderPrice, OrderVolume};
use rand::{Rng, RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use sp_std::iter::repeat;
use sp_std::ops::{Range, RangeInclusive};
use sp_std::prelude::*;

/// Parameters for filling one order book side
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, scale_info::TypeInfo)]
pub struct SideFillInput<Moment> {
    /// the best price for bids; the worst for asks
    pub highest_price: Balance,
    /// the worst price for bids; the best for asks
    pub lowest_price: Balance,
    pub price_step: Balance,
    pub orders_per_price: u32,
    /// Lifespan of inserted orders, max by default.
    pub lifespan: Option<Moment>,
    /// Default: `min_lot_size..=max_lot_size`
    pub amount_range_inclusive: Option<RandomAmount>,
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct OrderBookAttributes {
    pub tick_size: Balance,
    pub step_lot_size: Balance,
    pub min_lot_size: Balance,
    pub max_lot_size: Balance,
}

// default attributes for regular assets (not NFT)
impl Default for OrderBookAttributes {
    fn default() -> Self {
        Self {
            tick_size: balance!(0.00001),
            step_lot_size: balance!(0.00001),
            min_lot_size: balance!(1),
            max_lot_size: balance!(1000),
        }
    }
}

/// Parameters for orders amount generation
#[derive(Encode, Decode, Clone, Copy, Debug, PartialEq, Eq, scale_info::TypeInfo)]
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
pub struct FillInput<Moment, BlockNumber> {
    /// Best price = lowest, worst = highest.
    pub asks: Option<SideFillInput<Moment>>,
    /// Best price = highest, worst = lowest.
    pub bids: Option<SideFillInput<Moment>>,
    /// Seed for producing random values during the fill process. If `None`,
    /// current block is chosen
    pub random_seed: Option<BlockNumber>,
}

/// Does not create order books that already exist
///
/// `who` is just some account. Used to mint non-divisible assets for creating corresponding
/// order book(-s).
pub fn create_empty_batch_unchecked<T: Config>(
    order_book_settings: Vec<(OrderBookId<AssetIdOf<T>, T::DEXId>, OrderBookAttributes)>,
) -> Result<(), DispatchError> {
    let to_create_ids: Vec<_> = order_book_settings
        .into_iter()
        .filter(|(id, _)| !<order_book::OrderBooks<T>>::contains_key(id))
        .collect();
    for (order_book_id, _) in &to_create_ids {
        if !<T as Config>::TradingPairSourceManager::is_trading_pair_enabled(
            &order_book_id.dex_id,
            &order_book_id.quote,
            &order_book_id.base,
        )? {
            <T as Config>::TradingPairSourceManager::register_pair(
                order_book_id.dex_id,
                order_book_id.quote,
                order_book_id.base,
            )?;
        }
        order_book::Pallet::<T>::verify_create_orderbook_params(order_book_id)?;
    }

    for (order_book_id, attributes) in to_create_ids {
        order_book::Pallet::<T>::create_orderbook_unchecked(
            &order_book_id,
            attributes.tick_size,
            attributes.step_lot_size,
            attributes.min_lot_size,
            attributes.max_lot_size,
        )?;

        #[cfg(feature = "private-net")]
        order_book::Pallet::<T>::deposit_event_exposed(order_book::Event::<T>::OrderBookCreated {
            order_book_id,
            creator: None,
        });
    }
    Ok(())
}

/// Place orders into the order books.
pub fn fill_batch_unchecked<T: Config>(
    bids_owner: T::AccountId,
    asks_owner: T::AccountId,
    settings: Vec<(
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        FillInput<MomentOf<T>, BlockNumberFor<T>>,
    )>,
) -> Result<(), DispatchError> {
    let now = <T as order_book::Config>::Time::now();
    let current_block = frame_system::Pallet::<T>::block_number();
    let mut data = order_book::cache_data_layer::CacheDataLayer::<T>::new();

    for (order_book_id, fill_settings) in settings {
        fill_order_book(
            &mut data,
            order_book_id,
            asks_owner.clone(),
            bids_owner.clone(),
            fill_settings,
            now,
            current_block,
        )?;
    }
    data.commit();
    Ok(())
}

fn verify_fill_side_price_params<T: Config>(
    params: &SideFillInput<MomentOf<T>>,
    tick_size: OrderPrice,
) -> Result<(), DispatchError> {
    let tick = tick_size.balance();
    let prices_count = params
        .highest_price
        .saturating_sub(params.lowest_price)
        .checked_div(params.price_step)
        .ok_or(crate::Error::<T>::IncorrectPrice)?
        + 1;

    ensure!(
        params.price_step % tick == 0
            && params.price_step != 0
            && params.highest_price % tick == 0
            && params.lowest_price % tick == 0
            && params.highest_price != 0
            && params.lowest_price != 0
            && params.highest_price >= params.lowest_price
            && params.orders_per_price != 0
            && prices_count != 0,
        crate::Error::<T>::IncorrectPrice
    );

    ensure!(
        prices_count <= <T as order_book::Config>::MaxSidePriceCount::get().into(),
        crate::Error::<T>::TooManyPrices
    );

    ensure!(
        params.orders_per_price <= <T as order_book::Config>::MaxLimitOrdersForPrice::get()
            && prices_count.saturating_mul(params.orders_per_price as u128)
                <= <T as order_book::Config>::SOFT_MIN_MAX_RATIO as u128,
        crate::Error::<T>::TooManyOrders
    );
    Ok(())
}

fn verify_amount_range_within_bounds<T: Config>(
    amount_range: &RangeInclusive<Balance>,
    accepted_range: &RangeInclusive<Balance>,
) -> Result<(), DispatchError> {
    ensure!(
        accepted_range.contains(amount_range.start())
            && accepted_range.contains(amount_range.end()),
        crate::Error::<T>::OutOfBoundsRandomRange
    );
    Ok(())
}

fn max_amount_range<T: Config>(order_book: &OrderBook<T>) -> RandomAmount {
    RandomAmount::new(
        *order_book.min_lot_size.balance(),
        *order_book.max_lot_size.balance(),
    )
}

/// Fill a single order book.
fn fill_order_book<T: Config>(
    data: &mut impl DataLayer<T>,
    book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
    asks_owner: T::AccountId,
    bids_owner: T::AccountId,
    settings: FillInput<MomentOf<T>, BlockNumberFor<T>>,
    now: MomentOf<T>,
    current_block: BlockNumberFor<T>,
) -> Result<(), DispatchError> {
    let mut order_book = <order_book::OrderBooks<T>>::get(book_id)
        .ok_or(crate::Error::<T>::CannotFillUnknownOrderBook)?;
    let order_book_lot_size_range =
        *order_book.min_lot_size.balance()..=*order_book.max_lot_size.balance();

    let seed = settings.random_seed.unwrap_or(current_block);
    let seed = <BlockNumberFor<T> as TryInto<u64>>::try_into(seed).unwrap_or(0);
    let mut seed_generator = ChaCha8Rng::seed_from_u64(seed);
    // RNGs for each value are seeded separately.
    // This allows to have random as independent from other values as possible.
    // E.g. choosing to generate bids should not affect amounts of asks.
    let mut buy_amount_generator = ChaCha8Rng::seed_from_u64(seed_generator.next_u64());
    let mut sell_amount_generator = ChaCha8Rng::seed_from_u64(seed_generator.next_u64());

    if let Some(bids_settings) = settings.bids {
        let bids_lifespan = bids_settings
            .lifespan
            .unwrap_or(<T as order_book::Config>::MAX_ORDER_LIFESPAN);
        verify_fill_side_price_params::<T>(&bids_settings, order_book.tick_size)?;
        // price_step is checked to be non-zero in `verify_fill_side_params`
        let buy_prices: Vec<_> = (0..)
            .map(|step| bids_settings.highest_price - step * bids_settings.price_step)
            .take_while(|price| *price >= bids_settings.lowest_price)
            .collect();
        let buy_prices = buy_prices.into_iter().rev();
        let buy_amount_non_empty_range = bids_settings
            .amount_range_inclusive
            .unwrap_or_else(|| max_amount_range(&order_book))
            .as_non_empty_inclusive_range()
            .ok_or(crate::Error::<T>::EmptyRandomRange)?;
        verify_amount_range_within_bounds::<T>(
            &buy_amount_non_empty_range,
            &order_book_lot_size_range,
        )?;
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

        // total amount of assets to be locked
        let buy_quote_locked: Balance = buy_orders
            .iter()
            .map(|(quote, base)| *(*quote * (*base)).balance())
            .sum();
        // mint required amount to make this extrinsic self-sufficient
        T::AssetManager::mint_unchecked(&book_id.quote, &bids_owner, buy_quote_locked)?;

        // place buy orders
        place_multiple_orders(
            data,
            &mut order_book,
            bids_owner,
            PriceVariant::Buy,
            buy_orders.into_iter(),
            now,
            bids_lifespan,
            current_block,
        )?;
    }

    if let Some(asks_settings) = settings.asks {
        let asks_lifespan = asks_settings
            .lifespan
            .unwrap_or(<T as order_book::Config>::MAX_ORDER_LIFESPAN);
        verify_fill_side_price_params::<T>(&asks_settings, order_book.tick_size)?;
        // price_step is checked to be non-zero in `verify_fill_side_params`
        let sell_prices = (0..)
            .map(|step| asks_settings.lowest_price + step * asks_settings.price_step)
            .take_while(|price| *price <= asks_settings.highest_price);

        let sell_amount_non_empty_range = asks_settings
            .amount_range_inclusive
            .unwrap_or_else(|| max_amount_range(&order_book))
            .as_non_empty_inclusive_range()
            .ok_or(crate::Error::<T>::EmptyRandomRange)?;
        verify_amount_range_within_bounds::<T>(
            &sell_amount_non_empty_range,
            &order_book_lot_size_range,
        )?;
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

        // total amount of assets to be locked
        let sell_base_locked: Balance = sell_orders.iter().map(|(_, base)| *base.balance()).sum();
        // mint required amount to make this extrinsic self-sufficient
        T::AssetManager::mint_unchecked(&book_id.base, &asks_owner, sell_base_locked)?;

        // place sell orders
        place_multiple_orders(
            data,
            &mut order_book,
            asks_owner,
            PriceVariant::Sell,
            sell_orders.into_iter(),
            now,
            asks_lifespan,
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
