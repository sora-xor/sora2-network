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
use sp_runtime::Perbill;
use sp_std::vec::Vec;

pub mod weights;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod cache_data_layer;
mod limit_order;
mod market_order;
mod order_book;
pub mod storage_data_layer;
pub mod traits;
pub mod types;

pub use crate::order_book::{OrderBook, OrderBookStatus};
use cache_data_layer::CacheDataLayer;
pub use limit_order::LimitOrder;
pub use market_order::MarketOrder;
pub use traits::DataLayer;
pub use types::{MarketSide, OrderBookId, OrderPrice, OrderVolume, PriceOrders, UserOrders};

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
    use common::DEXInfo;
    use frame_support::{
        pallet_prelude::{OptionQuery, *},
        Blake2_128Concat,
    };
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + pallet_timestamp::Config {
        const MAX_ORDER_LIFETIME: Self::Moment;
        const MIN_ORDER_LIFETIME: Self::Moment;
        const MAX_PRICE_SHIFT: Perbill;

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
        type MaxOpenedLimitOrdersPerUser: Get<u32>;
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

    #[pallet::storage]
    #[pallet::getter(fn order_books)]
    pub type OrderBooks<T: Config> =
        StorageMap<_, Blake2_128Concat, OrderBookId<AssetIdOf<T>>, OrderBook<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn limit_orders)]
    pub type LimitOrders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>>,
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
        OrderBookId<AssetIdOf<T>>,
        Blake2_128Concat,
        OrderPrice,
        PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn asks)]
    pub type Asks<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>>,
        Blake2_128Concat,
        OrderPrice,
        PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn aggregated_bids)]
    pub type AggregatedBids<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>>,
        MarketSide<T::MaxSidePrices>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn aggregated_asks)]
    pub type AggregatedAsks<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>>,
        MarketSide<T::MaxSidePrices>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn user_limit_orders)]
    pub type UserLimitOrders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>>,
        UserOrders<T::OrderId, T::MaxOpenedLimitOrdersPerUser>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New order book is created by user
        OrderBookCreated {
            order_book_id: OrderBookId<AssetIdOf<T>>,
            dex_id: T::DEXId,
            creator: T::AccountId,
        },

        /// Order book is deleted by Council
        OrderBookDeleted {
            order_book_id: OrderBookId<AssetIdOf<T>>,
            dex_id: T::DEXId,
        },

        /// Order book attributes are updated by Council
        OrderBookUpdated {
            order_book_id: OrderBookId<AssetIdOf<T>>,
            dex_id: T::DEXId,
        },

        /// User placed new limit order
        OrderPlaced {
            order_book_id: OrderBookId<AssetIdOf<T>>,
            dex_id: T::DEXId,
            order_id: T::OrderId,
            owner_id: T::AccountId,
        },

        /// User canceled their limit order
        OrderCanceled {
            order_book_id: OrderBookId<AssetIdOf<T>>,
            dex_id: T::DEXId,
            order_id: T::OrderId,
            owner_id: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Order book does not exist for this trading pair
        UnknownOrderBook,
        /// Order book already exists for this trading pair
        OrderBookAlreadyExists,
        /// Limit order does not exist for this trading pair and order id
        UnknownLimitOrder,
        /// Limit order already exists for this trading pair and order id
        LimitOrderAlreadyExists,
        /// It is impossible to insert the limit order because the bounds have been reached
        LimitOrderStorageOverflow,
        /// It is impossible to update the limit order
        UpdateLimitOrderError,
        /// It is impossible to delete the limit order
        DeleteLimitOrderError,
        /// There are no bids/asks for the price
        NoDataForPrice,
        /// There are no aggregated bids/asks for the order book
        NoAggregatedData,
        /// There is not enough liquidity in the order book to cover the deal
        NotEnoughLiquidity,
        /// Cannot create order book with equal base and target assets
        ForbiddenToCreateOrderBookWithSameAssets,
        /// The asset is not allowed to be base. Only dex base asset can be a base asset for order book
        NotAllowedBaseAsset,
        /// User cannot create an order book with NFT if they don't have NFT
        UserHasNoNft,
        /// Lifespan exceeds defined limits
        InvalidLifespan,
        /// The order amount (limit or market) does not meet the requirements
        InvalidOrderAmount,
        /// The limit order price does not meet the requirements
        InvalidLimitOrderPrice,
        /// User cannot set the price of limit order too far from actual market price
        LimitOrderPriceIsTooFarFromSpread,
        /// At the moment, Trading is forbidden in the current order book
        TradingIsForbidden,
        /// At the moment, Users cannot place new limit orders in the current order book
        PlacementOfLimitOrdersIsForbidden,
        /// At the moment, Users cannot cancel their limit orders in the current order book
        CancellationOfLimitOrdersIsForbidden,
        /// User has the max available count of open limit orders in the current order book
        UserHasMaxCountOfOpenedOrders,
        /// It is impossible to place the limit order because bounds of the max count of orders at the current price have been reached
        PriceReachedMaxCountOfLimitOrders,
        /// It is impossible to place the limit order because bounds of the max count of prices for the side have been reached
        OrderBookReachedMaxCoundOfPricesForSide,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::create_orderbook())]
        pub fn create_orderbook(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            order_book_id: OrderBookId<AssetIdOf<T>>,
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
                        Error::<T>::UserHasNoNft
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
            _order_book_id: OrderBookId<AssetIdOf<T>>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            // todo (m.tagirov)
            todo!()
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::update_orderbook())]
        pub fn update_orderbook(
            origin: OriginFor<T>,
            order_book_id: OrderBookId<AssetIdOf<T>>,
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
            _order_book_id: OrderBookId<AssetIdOf<T>>,
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
            order_book_id: OrderBookId<AssetIdOf<T>>,
            price: OrderPrice,
            amount: OrderVolume,
            side: PriceVariant,
            lifespan: T::Moment,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let mut order_book =
                <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;
            let dex_id = order_book.dex_id;
            let order_id = order_book.next_order_id();
            let now = pallet_timestamp::Pallet::<T>::now();
            let order =
                LimitOrder::<T>::new(order_id, who.clone(), side, price, amount, now, lifespan);

            let mut data = CacheDataLayer::<T>::new();
            order_book.place_limit_order(order, &mut data)?;

            data.commit();
            <OrderBooks<T>>::insert(order_book_id, order_book);
            Self::deposit_event(Event::<T>::OrderPlaced {
                order_book_id: order_book_id,
                dex_id: dex_id,
                order_id: order_id,
                owner_id: who,
            });
            Ok(().into())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::cancel_limit_order())]
        pub fn cancel_limit_order(
            origin: OriginFor<T>,
            _order_book_id: OrderBookId<AssetIdOf<T>>,
            _order_id: T::OrderId,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            // todo (m.tagirov)
            todo!()
        }
    }
}

impl<T: Config> Pallet<T> {
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
