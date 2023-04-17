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
#![allow(dead_code)] // todo (m.tagirov) remove

use assets::AssetIdOf;
use common::prelude::{EnsureTradingPairExists, QuoteAmount, SwapAmount, SwapOutcome};
use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::{
    AssetInfoProvider, AssetName, AssetSymbol, Balance, BalancePrecision, ContentSource,
    Description, DexInfoProvider, LiquiditySource, PriceVariant, RewardReason,
};
use core::fmt::Debug;
use frame_support::sp_runtime::DispatchError;
use frame_support::weights::Weight;
use sp_runtime::traits::{AtLeast32BitUnsigned, MaybeDisplay, Zero};
use sp_std::vec::Vec;

pub mod weights;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod limit_order;
mod market_order;
mod order_book;

pub use crate::order_book::{OrderBook, OrderBookStatus};
pub use limit_order::LimitOrder;
pub use market_order::MarketOrder;

pub trait WeightInfo {
    fn create_orderbook() -> Weight;
    fn delete_orderbook() -> Weight;
    fn update_orderbook() -> Weight;
    fn change_orderbook_status() -> Weight;
    fn place_limit_order() -> Weight;
    fn cancel_limit_order() -> Weight;
    fn quote() -> Weight;
    fn exchange() -> Weight;
}

impl crate::WeightInfo for () {
    fn create_orderbook() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn delete_orderbook() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn update_orderbook() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn change_orderbook_status() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn place_limit_order() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn cancel_limit_order() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn quote() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn exchange() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{DEXInfo, TradingPair};
    use frame_support::{
        pallet_prelude::{OptionQuery, *},
        Blake2_128Concat, BoundedBTreeMap,
    };
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + pallet_timestamp::Config {
        const MAX_ORDER_LIFETIME: Self::Moment;
        const MAX_OPENED_LIMIT_ORDERS_COUNT: u32;

        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type OrderId: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + MaybeDisplay
            + AtLeast32BitUnsigned
            + Copy
            + Ord
            + PartialEq
            + Eq
            + MaxEncodedLen
            + scale_info::TypeInfo;
        type MaxOpenedLimitOrdersForAllOrderBooksPerUser: Get<u32>;
        type MaxLimitOrdersForPrice: Get<u32>;
        type MaxSidePrices: Get<u32>;
        type EnsureTradingPairExists: EnsureTradingPairExists<
            Self::DEXId,
            Self::AssetId,
            DispatchError,
        >;
        type AssetInfoProvider: AssetInfoProvider<
            Self::AssetId,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;
        type DexInfoProvider: DexInfoProvider<Self::DEXId, DEXInfo<Self::AssetId>>;
        type WeightInfo: WeightInfo;
    }

    pub type OrderBookId<T> = TradingPair<AssetIdOf<T>>;
    pub type OrderPrice = Balance;
    pub type OrderVolume = Balance;

    #[pallet::storage]
    #[pallet::getter(fn order_books)]
    pub type OrderBooks<T: Config> =
        StorageMap<_, Blake2_128Concat, OrderBookId<T>, OrderBook<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn limit_orders)]
    pub type LimitOrders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        OrderBookId<T>,
        Blake2_128Concat,
        T::OrderId,
        LimitOrder<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn bids)]
    pub type Bids<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        OrderBookId<T>,
        Blake2_128Concat,
        OrderPrice,
        BoundedVec<T::OrderId, T::MaxLimitOrdersForPrice>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn asks)]
    pub type Asks<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        OrderBookId<T>,
        Blake2_128Concat,
        OrderPrice,
        BoundedVec<T::OrderId, T::MaxLimitOrdersForPrice>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn aggregated_bids)]
    pub type AggregatedBids<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        OrderBookId<T>,
        BoundedBTreeMap<OrderPrice, OrderVolume, T::MaxSidePrices>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn aggregated_asks)]
    pub type AggregatedAsks<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        OrderBookId<T>,
        BoundedBTreeMap<OrderPrice, OrderVolume, T::MaxSidePrices>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn user_limit_orders)]
    pub type UserLimitOrders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        OrderBookId<T>,
        BoundedVec<T::OrderId, T::MaxOpenedLimitOrdersForAllOrderBooksPerUser>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New order book is created by user
        OrderBookCreated {
            order_book_id: OrderBookId<T>,
            dex_id: T::DEXId,
            creator: T::AccountId,
        },

        /// Order book is deleted by Council
        OrderBookDeleted {
            order_book_id: OrderBookId<T>,
            dex_id: T::DEXId,
        },

        /// Order book attributes are updated by Council
        OrderBookUpdated {
            order_book_id: OrderBookId<T>,
            dex_id: T::DEXId,
        },

        /// User placed new limit order
        OrderPlaced {
            order_book_id: OrderBookId<T>,
            dex_id: T::DEXId,
            order_id: T::OrderId,
            owner_id: T::AccountId,
        },

        /// User canceled their limit order
        OrderCanceled {
            order_book_id: OrderBookId<T>,
            dex_id: T::DEXId,
            order_id: T::OrderId,
            owner_id: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Trading pair currently reached its capacity
        OrderLimitReached,
        /// Price in given order exceeds allowed limits for the trading pair
        PriceExceedsLimits,
        /// Lifespan exceeds defined limits
        InvalidLifespan,
        /// Order book does not exist for this trading pair
        UnknownOrderBook,
        /// Order book already exists for this trading pair
        OrderBookAlreadyExists,
        /// Cannot insert the limit order because bounds are reached
        LimitOrderStorageOverflow,
        /// Cannot delete the limit order
        DeleteLimitOrderError,
        /// There is not enough liquidity in the order book to cover the deal
        NotEnoughLiquidity,
        /// Cannot create order book with equal base and target assets
        ForbiddenToCreateOrderBookWithSameAssets,
        /// The asset is not allowed to be base. Only dex base asset can be a base asset for order book
        NotAllowedBaseAsset,
        /// User cannot create an order book with NFT if they don't have NFT
        UserDoesntHaveNft,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::create_orderbook())]
        pub fn create_orderbook(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            order_book_id: OrderBookId<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                order_book_id.base_asset_id != order_book_id.target_asset_id,
                Error::<T>::ForbiddenToCreateOrderBookWithSameAssets
            );
            let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
            ensure!(
                order_book_id.base_asset_id == dex_info.base_asset_id,
                Error::<T>::NotAllowedBaseAsset
            );
            T::AssetInfoProvider::ensure_asset_exists(&order_book_id.target_asset_id)?;
            T::EnsureTradingPairExists::ensure_trading_pair_exists(
                &dex_id,
                &order_book_id.base_asset_id.into(),
                &order_book_id.target_asset_id.into(),
            )?;
            ensure!(
                !<OrderBooks<T>>::contains_key(order_book_id),
                Error::<T>::OrderBookAlreadyExists
            );

            let order_book =
                if T::AssetInfoProvider::get_asset_info(&order_book_id.target_asset_id).2 != 0 {
                    // regular asset
                    OrderBook::<T>::default(order_book_id, dex_id)
                } else {
                    // nft
                    // ensure the user has nft
                    ensure!(
                        T::AssetInfoProvider::total_balance(&order_book_id.target_asset_id, &who)?
                            > Balance::zero(),
                        Error::<T>::UserDoesntHaveNft
                    );
                    OrderBook::<T>::default_nft(order_book_id, dex_id)
                };

            <OrderBooks<T>>::insert(order_book_id, order_book);

            Self::deposit_event(Event::<T>::OrderBookCreated {
                order_book_id: order_book_id,
                dex_id: dex_id,
                creator: who,
            });
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::delete_orderbook())]
        pub fn delete_orderbook(
            origin: OriginFor<T>,
            _order_book_id: OrderBookId<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // todo (m.tagirov)
            todo!()
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::update_orderbook())]
        pub fn update_orderbook(
            origin: OriginFor<T>,
            order_book_id: OrderBookId<T>,
            _tick_size: OrderPrice,
            _step_lot_size: OrderVolume,
            _min_lot_size: OrderVolume,
            _max_lot_size: OrderVolume,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                <OrderBooks<T>>::contains_key(order_book_id),
                Error::<T>::UnknownOrderBook
            );
            // todo (m.tagirov)
            todo!()
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::change_orderbook_status())]
        pub fn change_orderbook_status(
            origin: OriginFor<T>,
            _order_book_id: OrderBookId<T>,
            _status: OrderBookStatus,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // todo (m.tagirov)
            todo!()
        }

        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::place_limit_order())]
        pub fn place_limit_order(
            origin: OriginFor<T>,
            order_book_id: OrderBookId<T>,
            _price: OrderPrice,
            _amount: OrderVolume,
            _side: PriceVariant,
            _lifespan: T::Moment,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            let _order_book =
                <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;
            // todo (m.tagirov)
            todo!()
        }

        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::cancel_limit_order())]
        pub fn cancel_limit_order(
            origin: OriginFor<T>,
            _order_book_id: OrderBookId<T>,
            _order_id: T::OrderId,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            // todo (m.tagirov)
            todo!()
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Inserts limit order consistently in all necessary storages.
    /// Must check before call. If returns error, it means we have problems with data consistency.
    pub fn insert_limit_order(
        order_book_id: &OrderBookId<T>,
        order: &LimitOrder<T>,
    ) -> Result<(), DispatchError> {
        <LimitOrders<T>>::insert(order_book_id, order.id, order);

        match order.side {
            PriceVariant::Buy => {
                <Bids<T>>::try_append(order_book_id, order.price, order.id)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;

                let mut bids = <AggregatedBids<T>>::get(order_book_id);
                let volume = bids
                    .get(&order.price)
                    .map(|x| *x)
                    .unwrap_or_default()
                    .checked_add(order.amount)
                    .ok_or(Error::<T>::LimitOrderStorageOverflow)?;
                bids.try_insert(order.price, volume)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
                <AggregatedBids<T>>::set(order_book_id, bids);
            }
            PriceVariant::Sell => {
                <Asks<T>>::try_append(order_book_id, order.price, order.id)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;

                let mut asks = <AggregatedAsks<T>>::get(order_book_id);
                let volume = asks
                    .get(&order.price)
                    .map(|x| *x)
                    .unwrap_or_default()
                    .checked_add(order.amount)
                    .ok_or(Error::<T>::LimitOrderStorageOverflow)?;
                asks.try_insert(order.price, volume)
                    .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;
                <AggregatedAsks<T>>::set(order_book_id, asks);
            }
        }

        <UserLimitOrders<T>>::try_append(&order.owner, order_book_id, order.id)
            .map_err(|_| Error::<T>::LimitOrderStorageOverflow)?;

        Ok(())
    }

    /// Delete limit order consistently from all necessary storages.
    /// Must check before call. If returns error, it means we have problems with data consistency.
    pub fn delete_limit_order(
        order_book_id: &OrderBookId<T>,
        order_id: T::OrderId,
    ) -> Result<(), DispatchError> {
        let order = <LimitOrders<T>>::take(order_book_id, order_id)
            .ok_or(Error::<T>::DeleteLimitOrderError)?;

        let mut user_orders = <UserLimitOrders<T>>::try_get(&order.owner, order_book_id)
            .map_err(|_| Error::<T>::DeleteLimitOrderError)?;
        user_orders.retain(|x| *x != order.id);
        if user_orders.is_empty() {
            <UserLimitOrders<T>>::remove(&order.owner, order_book_id)
        } else {
            <UserLimitOrders<T>>::set(&order.owner, order_book_id, Some(user_orders));
        }

        match order.side {
            PriceVariant::Buy => {
                let mut bids = <Bids<T>>::try_get(order_book_id, order.price)
                    .map_err(|_| Error::<T>::DeleteLimitOrderError)?;
                bids.retain(|x| *x != order.id);
                if bids.is_empty() {
                    <Bids<T>>::remove(order_book_id, order.price);
                } else {
                    <Bids<T>>::set(order_book_id, order.price, Some(bids));
                }

                let mut agg_bids = <AggregatedBids<T>>::try_get(order_book_id)
                    .map_err(|_| Error::<T>::DeleteLimitOrderError)?;
                let volume = agg_bids
                    .get(&order.price)
                    .map(|x| *x)
                    .ok_or(Error::<T>::DeleteLimitOrderError)?
                    .checked_sub(order.amount)
                    .ok_or(Error::<T>::DeleteLimitOrderError)?;
                if volume.is_zero() {
                    agg_bids
                        .remove(&order.price)
                        .ok_or(Error::<T>::DeleteLimitOrderError)?;
                } else {
                    agg_bids
                        .try_insert(order.price, volume)
                        .map_err(|_| Error::<T>::DeleteLimitOrderError)?;
                }
                <AggregatedBids<T>>::set(order_book_id, agg_bids);
            }
            PriceVariant::Sell => {
                let mut asks = <Asks<T>>::try_get(order_book_id, order.price)
                    .map_err(|_| Error::<T>::DeleteLimitOrderError)?;
                asks.retain(|x| *x != order.id);
                if asks.is_empty() {
                    <Asks<T>>::remove(order_book_id, order.price);
                } else {
                    <Asks<T>>::set(order_book_id, order.price, Some(asks));
                }

                let mut agg_asks = <AggregatedAsks<T>>::try_get(order_book_id)
                    .map_err(|_| Error::<T>::DeleteLimitOrderError)?;
                let volume = agg_asks
                    .get(&order.price)
                    .map(|x| *x)
                    .ok_or(Error::<T>::DeleteLimitOrderError)?
                    .checked_sub(order.amount)
                    .ok_or(Error::<T>::DeleteLimitOrderError)?;
                if volume.is_zero() {
                    agg_asks
                        .remove(&order.price)
                        .ok_or(Error::<T>::DeleteLimitOrderError)?;
                } else {
                    agg_asks
                        .try_insert(order.price, volume)
                        .map_err(|_| Error::<T>::DeleteLimitOrderError)?;
                }
                <AggregatedAsks<T>>::set(order_book_id, agg_asks);
            }
        }

        Ok(())
    }

    fn lock_liquidity(
        _account: &T::AccountId,
        _asset: &T::AssetId,
        _amount: Balance,
    ) -> Result<(), DispatchError> {
        // todo (m.tagirov)
        todo!()
    }

    fn unlock_liquidity(
        _account: &T::AccountId,
        _asset: &T::AssetId,
        _amount: Balance,
    ) -> Result<(), DispatchError> {
        // todo (m.tagirov)
        todo!()
    }
}

impl<T: Config> LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Balance, DispatchError>
    for Pallet<T>
{
    fn can_exchange(
        _dex_id: &T::DEXId,
        _input_asset_id: &T::AssetId,
        _output_asset_id: &T::AssetId,
    ) -> bool {
        // todo (m.tagirov)
        todo!()
    }

    fn quote(
        _dex_id: &T::DEXId,
        _input_asset_id: &T::AssetId,
        _output_asset_id: &T::AssetId,
        _amount: QuoteAmount<Balance>,
        _deduce_fee: bool,
    ) -> Result<(SwapOutcome<Balance>, Weight), DispatchError> {
        // todo (m.tagirov)
        todo!()
    }

    fn exchange(
        _sender: &T::AccountId,
        _receiver: &T::AccountId,
        _dex_id: &T::DEXId,
        _input_asset_id: &T::AssetId,
        _output_asset_id: &T::AssetId,
        _desired_amount: SwapAmount<Balance>,
    ) -> Result<(SwapOutcome<Balance>, Weight), DispatchError> {
        // todo (m.tagirov)
        todo!()
    }

    fn check_rewards(
        _dex_id: &T::DEXId,
        _input_asset_id: &T::AssetId,
        _output_asset_id: &T::AssetId,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<(Vec<(Balance, T::AssetId, RewardReason)>, Weight), DispatchError> {
        Ok((Vec::new(), Weight::zero())) // no rewards for Order Book
    }

    fn quote_without_impact(
        _dex_id: &T::DEXId,
        _input_asset_id: &T::AssetId,
        _output_asset_id: &T::AssetId,
        _amount: QuoteAmount<Balance>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        // todo (m.tagirov)
        todo!()
    }

    fn quote_weight() -> Weight {
        <T as Config>::WeightInfo::quote()
    }

    fn exchange_weight() -> Weight {
        <T as Config>::WeightInfo::exchange()
    }

    fn check_rewards_weight() -> Weight {
        Weight::zero()
    }
}
