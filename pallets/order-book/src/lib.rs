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
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![feature(int_roundings)]

use common::alt::{DiscreteQuotation, SideAmount, SwapChunk};
use common::prelude::{
    BalanceUnit, EnsureTradingPairExists, FixedWrapper, OutcomeFee, QuoteAmount, SwapAmount,
    SwapOutcome, TradingPair,
};
use common::AssetIdOf;
use common::LiquiditySourceType;
use common::{
    AssetInfoProvider, AssetName, AssetSymbol, Balance, BalancePrecision, ContentSource,
    Description, DexInfoProvider, LiquiditySource, OrderBookId, OrderBookManager, PriceVariant,
    RewardReason, SyntheticInfoProvider, ToOrderTechUnitFromDEXAndTradingPair,
    TradingPairSourceManager,
};
use core::fmt::Debug;
use frame_support::dispatch::{DispatchResultWithPostInfo, PostDispatchInfo};
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::{Get, Time};
use frame_support::weights::{Weight, WeightMeter};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::{
    AtLeast32BitUnsigned, CheckedAdd, CheckedDiv, CheckedMul, MaybeDisplay, Zero,
};
use sp_runtime::BoundedVec;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

pub mod weights;

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test", feature = "runtime-benchmarks"))]
pub mod test_utils;

pub mod cache_data_layer;
pub mod fee_calculator;
mod limit_order;
mod market_order;
mod order_book;
mod scheduler;
pub mod storage_data_layer;
pub mod traits;
pub mod types;

pub use crate::order_book::OrderBook;
use cache_data_layer::CacheDataLayer;
pub use limit_order::LimitOrder;
pub use market_order::MarketOrder;
pub use traits::{
    AlignmentScheduler, CurrencyLocker, CurrencyUnlocker, DataLayer, Delegate, ExpirationScheduler,
};
pub use types::{
    CancelReason, DealInfo, MarketChange, MarketRole, MarketSide, OrderAmount, OrderBookEvent,
    OrderBookStatus, OrderBookTechStatus, OrderPrice, OrderVolume, Payment, PriceOrders,
    UserOrders,
};
pub use weights::WeightInfo;

pub use pallet::*;

pub type MomentOf<T> = <<T as Config>::Time as Time>::Moment;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::DEXInfo;
    use frame_support::{
        pallet_prelude::{OptionQuery, *},
        traits::Hooks,
        Blake2_128Concat,
    };
    use frame_system::pallet_prelude::*;
    use sp_runtime::Either;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config + technical::Config {
        const MAX_ORDER_LIFESPAN: MomentOf<Self>;
        const MIN_ORDER_LIFESPAN: MomentOf<Self>;
        const MILLISECS_PER_BLOCK: MomentOf<Self>;
        /// The soft ratio between min & max order amounts.
        /// In particular, it defines the optimal number of limit orders that could be executed by one big market order in one block.
        const SOFT_MIN_MAX_RATIO: usize;
        /// The hard ratio between min & max order amounts.
        /// In particular, it defines the max number of limit orders that could be executed by one big market order in one block.
        /// During update of parameters, the limits must satisfy this ratio.
        const HARD_MIN_MAX_RATIO: usize;
        /// The number of executed limit orders by one market orders.
        /// It is used to estimate the weight of `exchange` before calling.
        const REGULAR_NUBMER_OF_EXECUTED_ORDERS: usize;

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
        type Locker: CurrencyLocker<Self::AccountId, AssetIdOf<Self>, Self::DEXId, DispatchError>;
        type Unlocker: CurrencyUnlocker<
            Self::AccountId,
            AssetIdOf<Self>,
            Self::DEXId,
            DispatchError,
        >;
        type Scheduler: AlignmentScheduler
            + ExpirationScheduler<
                Self::BlockNumber,
                OrderBookId<AssetIdOf<Self>, Self::DEXId>,
                Self::DEXId,
                Self::OrderId,
                DispatchError,
            >;
        type Delegate: Delegate<
            Self::AccountId,
            AssetIdOf<Self>,
            Self::OrderId,
            Self::DEXId,
            MomentOf<Self>,
        >;
        type MaxOpenedLimitOrdersPerUser: Get<u32>;
        type MaxLimitOrdersForPrice: Get<u32>;
        type MaxSidePriceCount: Get<u32>;
        type MaxExpiringOrdersPerBlock: Get<u32>;
        type MaxExpirationWeightPerBlock: Get<Weight>;
        type MaxAlignmentWeightPerBlock: Get<Weight>;
        type EnsureTradingPairExists: EnsureTradingPairExists<
            Self::DEXId,
            AssetIdOf<Self>,
            DispatchError,
        >;
        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, AssetIdOf<Self>>;
        type AssetInfoProvider: AssetInfoProvider<
            AssetIdOf<Self>,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;
        type SyntheticInfoProvider: SyntheticInfoProvider<AssetIdOf<Self>>;
        type DexInfoProvider: DexInfoProvider<Self::DEXId, DEXInfo<AssetIdOf<Self>>>;
        type Time: Time;
        type PermittedCreateOrigin: EnsureOrigin<
            Self::RuntimeOrigin,
            Success = Either<Self::AccountId, ()>,
        >;
        type PermittedEditOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = ()>;
        type WeightInfo: WeightInfo;
    }

    /// The storage contains the information about order book, it's parameters and statuses.
    #[pallet::storage]
    #[pallet::getter(fn order_books)]
    pub type OrderBooks<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        OrderBook<T>,
        OptionQuery,
    >;

    /// The storage contains the information about all limit orders in all order books.
    #[pallet::storage]
    #[pallet::getter(fn limit_orders)]
    pub type LimitOrders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        Blake2_128Concat,
        T::OrderId,
        LimitOrder<T>,
        OptionQuery,
    >;

    /// The index contains the list with the bid order `id` for each price.
    #[pallet::storage]
    #[pallet::getter(fn bids)]
    pub type Bids<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        Blake2_128Concat,
        OrderPrice,
        PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>,
        OptionQuery,
    >;

    /// The index contains the list with the ask order `id` for each price.
    #[pallet::storage]
    #[pallet::getter(fn asks)]
    pub type Asks<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        Blake2_128Concat,
        OrderPrice,
        PriceOrders<T::OrderId, T::MaxLimitOrdersForPrice>,
        OptionQuery,
    >;

    /// The index contains the aggregated information about bids with total volume of orders for each price.
    #[pallet::storage]
    #[pallet::getter(fn aggregated_bids)]
    pub type AggregatedBids<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        MarketSide<T::MaxSidePriceCount>,
        ValueQuery,
    >;

    /// The index contains the aggregated information about asks with total volume of orders for each price.
    #[pallet::storage]
    #[pallet::getter(fn aggregated_asks)]
    pub type AggregatedAsks<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        MarketSide<T::MaxSidePriceCount>,
        ValueQuery,
    >;

    /// The index contains the list with the order `id` for the user in different order books.
    #[pallet::storage]
    #[pallet::getter(fn user_limit_orders)]
    pub type UserLimitOrders<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        UserOrders<T::OrderId, T::MaxOpenedLimitOrdersPerUser>,
        OptionQuery,
    >;

    /// The tech storage that is used in the order expiration mechanism.
    #[pallet::storage]
    #[pallet::getter(fn expired_orders_at)]
    pub type ExpirationsAgenda<T: Config> = StorageMap<
        _,
        Identity,
        T::BlockNumber,
        BoundedVec<(OrderBookId<AssetIdOf<T>, T::DEXId>, T::OrderId), T::MaxExpiringOrdersPerBlock>,
        ValueQuery,
    >;

    /// The tech storage that is used during the process of updating the order book.
    #[pallet::storage]
    #[pallet::getter(fn alignment_cursor)]
    pub type AlignmentCursor<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        OrderBookId<AssetIdOf<T>, T::DEXId>,
        T::OrderId,
        OptionQuery,
    >;

    /// Earliest block with incomplete expirations;
    /// Weight limit might not allow to finish all expirations for a block
    /// so they might be operated later.
    #[pallet::storage]
    #[pallet::getter(fn incomplete_expirations_since)]
    pub type IncompleteExpirationsSince<T: Config> = StorageValue<_, T::BlockNumber>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// New order book is created by user
        OrderBookCreated {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            /// `creator` contains an address if the order book is created for an indivisible asset by the asset holder, or `None` if it is created by root / tech committee
            creator: Option<T::AccountId>,
        },

        /// Order book is deleted
        OrderBookDeleted {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        },

        /// Order book status is changed
        OrderBookStatusChanged {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            new_status: OrderBookStatus,
        },

        /// Order book attributes are updated
        OrderBookUpdated {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        },

        /// User placed new limit order
        LimitOrderPlaced {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            order_id: T::OrderId,
            owner_id: T::AccountId,
            side: PriceVariant,
            price: OrderPrice,
            amount: OrderVolume,
            lifetime: MomentOf<T>,
        },

        /// User tried to place the limit order out of the spread. The limit order is converted into a market order.
        LimitOrderConvertedToMarketOrder {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            owner_id: T::AccountId,
            direction: PriceVariant,
            amount: OrderAmount,
            average_price: OrderPrice,
        },

        /// User tried to place the limit order out of the spread.
        /// One part of the liquidity of the limit order is converted into a market order, and the other part is placed as a limit order.
        LimitOrderIsSplitIntoMarketOrderAndLimitOrder {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            owner_id: T::AccountId,
            market_order_direction: PriceVariant,
            market_order_amount: OrderAmount,
            market_order_average_price: OrderPrice,
            limit_order_id: T::OrderId,
        },

        /// User canceled their limit order or the limit order has reached the end of its lifespan
        LimitOrderCanceled {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            order_id: T::OrderId,
            owner_id: T::AccountId,
            reason: CancelReason,
        },

        /// Some amount of the limit order is executed
        LimitOrderExecuted {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            order_id: T::OrderId,
            owner_id: T::AccountId,
            side: PriceVariant,
            price: OrderPrice,
            amount: OrderAmount,
        },

        /// All amount of the limit order is executed
        LimitOrderFilled {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            order_id: T::OrderId,
            owner_id: T::AccountId,
        },

        /// The limit order is updated
        LimitOrderUpdated {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            order_id: T::OrderId,
            owner_id: T::AccountId,
            new_amount: OrderVolume,
        },

        /// User executes a deal by the market order
        MarketOrderExecuted {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            owner_id: T::AccountId,
            direction: PriceVariant,
            amount: OrderAmount,
            average_price: OrderPrice,
            to: Option<T::AccountId>,
        },

        /// Failed to cancel expired order
        ExpirationFailure {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            order_id: T::OrderId,
            error: DispatchError,
        },

        /// Failed to cancel expired order
        AlignmentFailure {
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            error: DispatchError,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Order book does not exist for this trading pair
        UnknownOrderBook,
        /// Invalid order book id
        InvalidOrderBookId,
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
        /// Expiration schedule for expiration block is full
        BlockScheduleFull,
        /// Could not find expiration in given block schedule
        ExpirationNotFound,
        /// There are no bids/asks for the price
        NoDataForPrice,
        /// There are no aggregated bids/asks for the order book
        NoAggregatedData,
        /// There is not enough liquidity in the order book to cover the deal
        NotEnoughLiquidityInOrderBook,
        /// Cannot create order book with equal base and target assets
        ForbiddenToCreateOrderBookWithSameAssets,
        /// The asset is not allowed to be quote. Only the dex base asset can be a quote asset for order book
        NotAllowedQuoteAsset,
        /// Orderbooks cannot be created with given dex id.
        NotAllowedDEXId,
        /// Synthetic assets are forbidden for order book.
        SyntheticAssetIsForbidden,
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
        OrderBookReachedMaxCountOfPricesForSide,
        /// An error occurred while calculating the amount
        AmountCalculationFailed,
        /// An error occurred while calculating the price
        PriceCalculationFailed,
        /// Unauthorized action
        Unauthorized,
        /// Invalid asset
        InvalidAsset,
        /// Invalid tick size
        InvalidTickSize,
        /// Invalid step lot size
        InvalidStepLotSize,
        /// Invalid min lot size
        InvalidMinLotSize,
        /// Invalid max lot size
        InvalidMaxLotSize,
        /// Tick size & step lot size are too big and their multiplication overflows Balance
        TickSizeAndStepLotSizeAreTooBig,
        /// Product of tick and step lot sizes goes out of precision. It must be accurately represented by fixed-precision float to prevent rounding errors. I.e. the product should not have more than 18 digits after the comma.
        TickSizeAndStepLotSizeLosePrecision,
        /// Max lot size cannot be more that total supply of base asset
        MaxLotSizeIsMoreThanTotalSupply,
        /// Indicated limit for slippage has not been met during transaction execution.
        SlippageLimitExceeded,
        /// Market orders are allowed only for indivisible assets
        MarketOrdersAllowedOnlyForIndivisibleAssets,
        /// It is possible to delete an order-book only with the statuses: OnlyCancel or Stop
        ForbiddenStatusToDeleteOrderBook,
        /// It is possible to delete only empty order-book
        OrderBookIsNotEmpty,
        /// It is possible to update an order-book only with the statuses: OnlyCancel or Stop
        ForbiddenStatusToUpdateOrderBook,
        /// Order Book is locked for technical maintenance. Try again later.
        OrderBookIsLocked,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Perform scheduled expirations
        fn on_initialize(current_block: T::BlockNumber) -> Weight {
            let mut expiration_weight_counter =
                WeightMeter::from_limit(T::MaxExpirationWeightPerBlock::get());
            Self::service_expiration(current_block, &mut expiration_weight_counter);

            let mut alignment_weight_counter =
                WeightMeter::from_limit(T::MaxAlignmentWeightPerBlock::get());
            Self::service_alignment(&mut alignment_weight_counter);

            expiration_weight_counter
                .consumed
                .saturating_add(alignment_weight_counter.consumed)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Creates a new order book for the pair of assets.
        ///
        /// # Parameters:
        /// - `origin`: caller account who must have permissions to create the order book
        /// - `order_book_id`: [order book identifier](OrderBookId) that contains: `DexId`, `base asset` & `quote asset`
        /// - `tick_size`: price step
        /// - `step_lot_size`: amount step
        /// - `min_lot_size`: minimal order amount
        /// - `max_lot_size`: maximal order amount
        ///
        /// # Rules:
        /// - root & tech committee can create any order book
        /// - a regular user can create an order book only for indivisible base assets (most likely NFT) and only if they have this asset on their balance
        /// - trading pair for the assets must be registered before the creating an order book
        ///
        /// # Attribute rules (for `tick_size`, `step_lot_size`, `min_lot_size` & `max_lot_size`):
        /// - all attributes must be non-zero
        /// - `min_lot_size` <= `max_lot_size`
        /// - `step_lot_size` <= `min_lot_size`
        /// - `min_lot_size` & `max_lot_size` must be a multiple of `step_lot_size`
        /// - `max_lot_size` <= `min_lot_size` * `SOFT_MIN_MAX_RATIO`, now `SOFT_MIN_MAX_RATIO` = 1 000
        /// - `max_lot_size` <= total supply of `base` asset
        /// - precision of `tick_size` * `step_lot_size` must not overflow **18 digits**
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::create_orderbook())]
        pub fn create_orderbook(
            origin: OriginFor<T>,
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            tick_size: Balance,
            step_lot_size: Balance,
            min_lot_size: Balance,
            max_lot_size: Balance,
        ) -> DispatchResult {
            let origin_check_result = T::PermittedCreateOrigin::ensure_origin(origin)?;
            let maybe_who = match origin_check_result {
                Either::Left(who) => {
                    if <T as Config>::AssetInfoProvider::is_non_divisible(&order_book_id.base) {
                        // nft
                        // ensure the user has nft
                        ensure!(
                            <T as Config>::AssetInfoProvider::total_balance(
                                &order_book_id.base,
                                &who
                            )? > Balance::zero(),
                            Error::<T>::UserHasNoNft
                        );
                        Some(who)
                    } else {
                        return Err(DispatchError::BadOrigin);
                    }
                }
                Either::Right(()) => None,
            };

            Self::inner_create_orderbook(
                &order_book_id,
                tick_size,
                step_lot_size,
                min_lot_size,
                max_lot_size,
            )?;

            Self::deposit_event(Event::<T>::OrderBookCreated {
                order_book_id,
                creator: maybe_who,
            });
            Ok(())
        }

        /// Deletes the order book
        ///
        /// # Parameters:
        /// - `origin`: caller account who must have permissions to delete the order book
        /// - `order_book_id`: [order book identifier](OrderBookId) that contains: `DexId`, `base asset` & `quote asset`
        ///
        /// # Rules:
        /// - only root & tech committee can delete the order book
        /// - status of the order book must be [`OnlyCancel`](OrderBookStatus::OnlyCancel) or [`Stop`](OrderBookStatus::Stop)
        /// - the order book must be empty - doesn't contain any orders
        ///
        /// # Real life delete process:
        /// 1. Announce that the order book will be deleted.
        /// 2. Stop the order book by changing the status to [`OnlyCancel`](OrderBookStatus::OnlyCancel) or [`Stop`](OrderBookStatus::Stop)
        /// 3. Wait until users cancel their orders or their lifetime just expires (maximum 1 month).
        /// 4. Delete the empty order book.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::delete_orderbook())]
        pub fn delete_orderbook(
            origin: OriginFor<T>,
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        ) -> DispatchResult {
            T::PermittedEditOrigin::ensure_origin(origin)?;
            let order_book =
                <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;

            ensure!(
                order_book.status == OrderBookStatus::OnlyCancel
                    || order_book.status == OrderBookStatus::Stop,
                Error::<T>::ForbiddenStatusToDeleteOrderBook
            );

            let is_empty = <LimitOrders<T>>::iter_prefix_values(order_book_id)
                .next()
                .is_none();
            ensure!(is_empty, Error::<T>::OrderBookIsNotEmpty);

            T::TradingPairSourceManager::disable_source_for_trading_pair(
                &order_book_id.dex_id,
                &order_book_id.quote,
                &order_book_id.base,
                LiquiditySourceType::OrderBook,
            )?;

            Self::deregister_tech_account(&order_book_id)?;
            <OrderBooks<T>>::remove(order_book_id);

            Self::deposit_event(Event::<T>::OrderBookDeleted { order_book_id });
            Ok(())
        }

        /// Updates the attributes of the order book
        ///
        /// # Parameters:
        /// - `origin`: caller account who must have permissions to update the order book
        /// - `order_book_id`: [order book identifier](OrderBookId) that contains: `DexId`, `base asset` & `quote asset`
        /// - `tick_size`: price step
        /// - `step_lot_size`: amount step
        /// - `min_lot_size`: minimal order amount
        /// - `max_lot_size`: maximal order amount
        ///
        /// # Rules:
        /// - only root & tech committee can update the order book
        /// - status of the order book must be [`OnlyCancel`](OrderBookStatus::OnlyCancel) or [`Stop`](OrderBookStatus::Stop)
        /// - inernal tech status of the order book must be [`Ready`](OrderBookTechStatus::Ready), that means the previos update is completed
        ///
        /// # Attribute rules (for `tick_size`, `step_lot_size`, `min_lot_size` & `max_lot_size`):
        /// - all attributes must be non-zero
        /// - `min_lot_size` <= `max_lot_size`
        /// - `step_lot_size` <= `min_lot_size`
        /// - `min_lot_size` & `max_lot_size` must be a multiple of `step_lot_size`
        /// - `max_lot_size` <= total supply of `base` asset
        /// - precision of `tick_size` * `step_lot_size` must not overflow 18 digits
        /// - `max_lot_size` <= `min_lot_size` * `SOFT_MIN_MAX_RATIO`, now `SOFT_MIN_MAX_RATIO` = 1 000
        /// - `max_lot_size` <= **old** `min_lot_size` * `HARD_MIN_MAX_RATIO`, now `HARD_MIN_MAX_RATIO` = 4 000
        ///
        /// # Real life update process:
        /// 1. Announce that the order book will be updated.
        /// 2. Stop the order book by changing the status to [`OnlyCancel`](OrderBookStatus::OnlyCancel) or [`Stop`](OrderBookStatus::Stop)
        /// 3. Update the order book attributes according to the rules[^note].
        /// 4. Wait the orders alignment if it is necessary - the order book tech status must become [`Ready`](OrderBookTechStatus::Ready).
        /// 5. Change the order book status back to [`Trade`](OrderBookStatus::Trade) or other necessary status.
        /// 6. Announce that the order book update is completed.
        ///
        /// [^note]: according to tech reasons it is forbidden to update `max_lot_size` with too large a value (see last 2 rules).
        /// For example, if the current values `min_lot_size` = 1 & `max_lot_size` = 1 000,
        /// we cannot change it to `min_lot_size` = 1 000 & `max_lot_size` = 1 000 000.
        /// In this case it is necessary to do several update rounds:
        /// 1. `min_lot_size`: 1 --> 1 000, `max_lot_size`: 1 000 --> 4 000
        /// 2. `max_lot_size`: 4 000 --> 1 000 000
        ///
        /// It is also not recommended to batch these updates, because the tech status of the order book can be changed after the 1st update and the 2nd update will be declined in this case.
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::update_orderbook())]
        pub fn update_orderbook(
            origin: OriginFor<T>,
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            tick_size: Balance,
            step_lot_size: Balance,
            min_lot_size: Balance,
            max_lot_size: Balance,
        ) -> DispatchResult {
            T::PermittedEditOrigin::ensure_origin(origin)?;
            let mut order_book =
                <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;

            ensure!(
                order_book.tech_status == OrderBookTechStatus::Ready,
                Error::<T>::OrderBookIsLocked
            );

            ensure!(
                order_book.status == OrderBookStatus::OnlyCancel
                    || order_book.status == OrderBookStatus::Stop,
                Error::<T>::ForbiddenStatusToUpdateOrderBook
            );

            Self::verify_orderbook_attributes(
                &order_book_id,
                tick_size,
                step_lot_size,
                min_lot_size,
                max_lot_size,
            )?;

            // check the ratio between old min & new max
            ensure!(
                max_lot_size
                    <= order_book
                        .min_lot_size
                        .balance()
                        .saturating_mul(T::HARD_MIN_MAX_RATIO as Balance),
                Error::<T>::InvalidMaxLotSize
            );

            let prev_step_lot_size = order_book.step_lot_size;

            order_book.tick_size.set(tick_size);
            order_book.step_lot_size.set(step_lot_size);
            order_book.min_lot_size.set(min_lot_size);
            order_book.max_lot_size.set(max_lot_size);

            // Note:
            // The amounts of already existed limit orders are aligned if they don't meet the requirements of new `step_lot_size` value.
            // All new limit orders must meet the requirements of new attributes.

            if prev_step_lot_size.balance() % step_lot_size != 0 {
                order_book.tech_status = OrderBookTechStatus::Updating;

                // schedule alignment
                <AlignmentCursor<T>>::set(order_book_id, Some(T::OrderId::zero()));
            }
            <OrderBooks<T>>::set(order_book_id, Some(order_book));
            Self::deposit_event(Event::<T>::OrderBookUpdated { order_book_id });
            Ok(())
        }

        /// Sets the order book status
        ///
        /// # Parameters:
        /// - `origin`: caller account who must have permissions to change the order book status
        /// - `order_book_id`: [order book identifier](OrderBookId) that contains: `DexId`, `base asset` & `quote asset`
        /// - `status`: one of the statuses from [OrderBookStatus]
        ///
        /// # Rules:
        /// - only root & tech committee can set the order book status
        /// - if the order book is locked by updating (tech status is [`Updating`](OrderBookTechStatus::Updating)), the allowed statues to set:
        ///     - [`OnlyCancel`](OrderBookStatus::OnlyCancel)
        ///     - [`Stop`](OrderBookStatus::Stop)
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::change_orderbook_status())]
        pub fn change_orderbook_status(
            origin: OriginFor<T>,
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            status: OrderBookStatus,
        ) -> DispatchResult {
            T::PermittedEditOrigin::ensure_origin(origin)?;
            <OrderBooks<T>>::mutate(order_book_id, |order_book| {
                let order_book = order_book.as_mut().ok_or(Error::<T>::UnknownOrderBook)?;

                if order_book.tech_status == OrderBookTechStatus::Updating
                    && status != OrderBookStatus::OnlyCancel
                    && status != OrderBookStatus::Stop
                {
                    return Err(Error::<T>::OrderBookIsLocked);
                }

                order_book.status = status;
                Ok::<_, Error<T>>(())
            })?;
            Self::deposit_event(Event::<T>::OrderBookStatusChanged {
                order_book_id,
                new_status: status,
            });
            Ok(())
        }

        /// Places the limit order into the order book
        ///
        /// # Parameters:
        /// - `origin`: caller account, the limit order owner
        /// - `order_book_id`: [order book identifier](OrderBookId) that contains: `DexId`, `base asset` & `quote asset`
        /// - `price`: price in the `quote asset`
        /// - `amount`: volume of the limit order in the `base asset`
        /// - `side`: [side](PriceVariant) where to place the limit order
        /// - `lifespan`: life duration of the limit order in millisecs, if not defined the default value 30 days is set
        ///
        /// # Rules:
        /// - `price` must be a multiple of [`OrderBook::tick_size`]
        /// - `amount` >= [`OrderBook::min_lot_size`]
        /// - `amount` <= [`OrderBook::max_lot_size`]
        /// - `amount` must be a multiple of [`OrderBook::step_lot_size`]
        /// - if the `price` crosses the spread (the opposite `side`):
        ///     - if [`OrderBook::status`] allows to trade - the limit order is converted into market order and the exchange occurs
        ///     - if [`OrderBook::status`] doesn't allow to trade - transaction fails
        #[pallet::call_index(4)]
        // in the worst case the limit order is converted into market order and the exchange occurs
        #[pallet::weight(Pallet::<T>::exchange_weight())]
        pub fn place_limit_order(
            origin: OriginFor<T>,
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            price: Balance,
            amount: Balance,
            side: PriceVariant,
            lifespan: Option<MomentOf<T>>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let executed_orders_count =
                Self::inner_place_limit_order(who, order_book_id, price, amount, side, lifespan)?;

            // Note: be careful with changing the weight. The fee depends on it,
            // the market-maker fee is charged for some weight, and the regular fee for none weight
            let actual_weight = if executed_orders_count == 0 {
                // if the extrinsic just places the limit order, the weight of the placing is returned
                Some(<T as Config>::WeightInfo::place_limit_order_without_cross_spread())
            } else {
                // if the limit order was converted into market order, then None weight is returned
                // this weight will be replaced with worst case weight:
                // exchange_weight() + place_limit_order_without_cross_spread()
                None
            };

            Ok(PostDispatchInfo {
                actual_weight,
                pays_fee: Pays::Yes,
            })
        }

        /// Cancels the limit order
        ///
        /// # Parameters:
        /// - `origin`: caller account who owns the limit order
        /// - `order_book_id`: [order book identifier](OrderBookId) that contains: `DexId`, `base asset` & `quote asset`
        /// - `order_id`: `id` of the limit order
        ///
        /// # Rules:
        /// - only the order owner can cancel the limit order
        ///
        /// # Note:
        /// Network fee isn't charged if the order is successfully cancelled by the owner
        #[pallet::call_index(5)]
        #[pallet::weight(
            <T as Config>::WeightInfo::cancel_limit_order_first_expiration()
                .max(<T as Config>::WeightInfo::cancel_limit_order_last_expiration()))
        ]
        pub fn cancel_limit_order(
            origin: OriginFor<T>,
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            order_id: T::OrderId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let mut data = CacheDataLayer::<T>::new();
            let order = data.get_limit_order(&order_book_id, order_id)?;

            ensure!(order.owner == who, Error::<T>::Unauthorized);

            let order_book =
                <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;

            order_book.cancel_limit_order(order, &mut data)?;
            data.commit();

            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Cancels the list of limit orders
        ///
        /// # Parameters:
        /// - `origin`: caller account who owns the limit orders
        /// - `limit_orders_to_cancel`: the list with [`order_book_id`](OrderBookId) & `order_id` pairs to cancel
        ///
        /// # Rules:
        /// - only the owner of **all** orders can cancel all limit orders from the list
        ///
        /// # Note:
        /// Network fee isn't charged if orders are successfully cancelled by the owner
        #[pallet::call_index(6)]
        #[pallet::weight({
            let cancel_limit_order = <T as Config>::WeightInfo::cancel_limit_order_first_expiration()
                .max(<T as Config>::WeightInfo::cancel_limit_order_last_expiration());
            let limit_orders_count: u64 = limit_orders_to_cancel
                .iter()
                .fold(0, |count, (_, order_ids)| count.saturating_add(order_ids.len() as u64));

            cancel_limit_order.saturating_mul(limit_orders_count)
        })]
        pub fn cancel_limit_orders_batch(
            origin: OriginFor<T>,
            limit_orders_to_cancel: Vec<(OrderBookId<AssetIdOf<T>, T::DEXId>, Vec<T::OrderId>)>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let mut data = CacheDataLayer::<T>::new();

            for (order_book_id, order_ids) in limit_orders_to_cancel {
                for order_id in order_ids {
                    let order = data.get_limit_order(&order_book_id, order_id)?;

                    ensure!(order.owner == who, Error::<T>::Unauthorized);

                    let order_book =
                        <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;

                    order_book.cancel_limit_order(order, &mut data)?;
                }
            }

            data.commit();

            Ok(PostDispatchInfo {
                actual_weight: None,
                pays_fee: Pays::No,
            })
        }

        /// Executes the market order
        ///
        /// # Parameters:
        /// - `origin`: caller account
        /// - `order_book_id`: [order book identifier](OrderBookId) that contains: `DexId`, `base asset` & `quote asset`
        /// - `direction`: [direction](PriceVariant) of the market order
        /// - `amount`: volume of the `base asset` to trade
        ///
        /// # Rules:
        /// - works only for order books with indivisible `base asset`, because there is no other ability to trade such assets. All other divisible assets must be traded by `liquidity_proxy::swap`
        /// - `amount` >= [`OrderBook::min_lot_size`]
        /// - `amount` <= [`OrderBook::max_lot_size`]
        /// - `amount` must be a multiple of [`OrderBook::step_lot_size`]
        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::execute_market_order())]
        pub fn execute_market_order(
            origin: OriginFor<T>,
            order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
            direction: PriceVariant,
            amount: Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                <T as Config>::AssetInfoProvider::is_non_divisible(&order_book_id.base),
                Error::<T>::MarketOrdersAllowedOnlyForIndivisibleAssets
            );
            let order_book =
                <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;

            let amount = OrderVolume::indivisible(amount);
            let mut data = CacheDataLayer::<T>::new();

            let market_order = MarketOrder::<T>::new(who, direction, order_book_id, amount, None);
            order_book.execute_market_order(market_order, &mut data)?;

            data.commit();
            Ok(())
        }
    }
}

impl<T: Config> CurrencyLocker<T::AccountId, AssetIdOf<T>, T::DEXId, DispatchError> for Pallet<T> {
    fn lock_liquidity(
        account: &T::AccountId,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
        asset_id: &AssetIdOf<T>,
        amount: OrderVolume,
    ) -> Result<(), DispatchError> {
        let tech_account = Self::tech_account_for_order_book(order_book_id);
        technical::Pallet::<T>::transfer_in(asset_id, account, &tech_account, *amount.balance())
    }
}

impl<T: Config> CurrencyUnlocker<T::AccountId, AssetIdOf<T>, T::DEXId, DispatchError>
    for Pallet<T>
{
    fn unlock_liquidity(
        account: &T::AccountId,
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
        asset_id: &AssetIdOf<T>,
        amount: OrderVolume,
    ) -> Result<(), DispatchError> {
        let tech_account = Self::tech_account_for_order_book(order_book_id);
        technical::Pallet::<T>::transfer_out(asset_id, &tech_account, account, *amount.balance())
    }

    fn unlock_liquidity_batch(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
        asset_id: &AssetIdOf<T>,
        receivers: &BTreeMap<T::AccountId, OrderVolume>,
    ) -> Result<(), DispatchError> {
        let tech_account = Self::tech_account_for_order_book(order_book_id);
        for (account, amount) in receivers.iter() {
            technical::Pallet::<T>::transfer_out(
                asset_id,
                &tech_account,
                account,
                *amount.balance(),
            )?;
        }
        Ok(())
    }
}

impl<T: Config> Delegate<T::AccountId, AssetIdOf<T>, T::OrderId, T::DEXId, MomentOf<T>>
    for Pallet<T>
{
    fn emit_event(
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        event: OrderBookEvent<T::AccountId, T::OrderId, MomentOf<T>>,
    ) {
        let event = match event {
            OrderBookEvent::LimitOrderPlaced {
                order_id,
                owner_id,
                side,
                price,
                amount,
                lifetime,
            } => Event::<T>::LimitOrderPlaced {
                order_book_id,
                order_id,
                owner_id,
                side,
                price,
                amount,
                lifetime,
            },

            OrderBookEvent::LimitOrderConvertedToMarketOrder {
                owner_id,
                direction,
                amount,
                average_price,
            } => Event::<T>::LimitOrderConvertedToMarketOrder {
                order_book_id,
                owner_id,
                direction,
                amount,
                average_price,
            },

            OrderBookEvent::LimitOrderIsSplitIntoMarketOrderAndLimitOrder {
                owner_id,
                market_order_direction,
                market_order_amount,
                market_order_average_price,
                limit_order_id,
            } => Event::<T>::LimitOrderIsSplitIntoMarketOrderAndLimitOrder {
                order_book_id,
                owner_id,
                market_order_direction,
                market_order_amount,
                market_order_average_price,
                limit_order_id,
            },

            OrderBookEvent::LimitOrderCanceled {
                order_id,
                owner_id,
                reason,
            } => Event::<T>::LimitOrderCanceled {
                order_book_id,
                order_id,
                owner_id,
                reason,
            },

            OrderBookEvent::LimitOrderExecuted {
                order_id,
                owner_id,
                side,
                price,
                amount,
            } => Event::<T>::LimitOrderExecuted {
                order_book_id,
                order_id,
                owner_id,
                side,
                price,
                amount,
            },

            OrderBookEvent::LimitOrderFilled { order_id, owner_id } => {
                Event::<T>::LimitOrderFilled {
                    order_book_id,
                    order_id,
                    owner_id,
                }
            }

            OrderBookEvent::LimitOrderUpdated {
                order_id,
                owner_id,
                new_amount,
            } => Event::<T>::LimitOrderUpdated {
                order_book_id,
                order_id,
                owner_id,
                new_amount,
            },

            OrderBookEvent::MarketOrderExecuted {
                owner_id,
                direction,
                amount,
                average_price,
                to,
            } => Event::<T>::MarketOrderExecuted {
                order_book_id,
                owner_id,
                direction,
                amount,
                average_price,
                to,
            },
        };

        Self::deposit_event(event);
    }
}

impl<T: Config> Pallet<T> {
    pub fn tech_account_for_order_book(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
    ) -> <T as technical::Config>::TechAccountId {
        let trading_pair: TradingPair<AssetIdOf<T>> = (*order_book_id).into();
        // Same as in xyk accounts
        let tech_pair = trading_pair.map(|a| a.into());
        <T as technical::Config>::TechAccountId::to_order_tech_unit_from_dex_and_trading_pair(
            order_book_id.dex_id,
            tech_pair,
        )
    }

    /// Validity of asset ids (for example, to have the same base asset
    /// for dex and pair) should be done beforehand
    pub fn register_tech_account(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
    ) -> Result<(), DispatchError> {
        let tech_account = Self::tech_account_for_order_book(order_book_id);
        technical::Pallet::<T>::register_tech_account_id(tech_account)
    }

    /// Validity of asset ids (for example, to have the same base asset
    /// for dex and pair) should be done beforehand
    pub fn deregister_tech_account(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
    ) -> Result<(), DispatchError> {
        let tech_account = Self::tech_account_for_order_book(order_book_id);
        technical::Pallet::<T>::deregister_tech_account_id(tech_account)
    }

    pub fn assemble_order_book_id(
        dex_id: T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
    ) -> Option<OrderBookId<AssetIdOf<T>, T::DEXId>> {
        if input_asset_id == output_asset_id {
            return None;
        }

        let Ok(dex_info) = T::DexInfoProvider::get_dex_info(&dex_id) else {
            return None;
        };

        let order_book_id = match dex_info.base_asset_id {
            input if input == *input_asset_id => OrderBookId::<AssetIdOf<T>, T::DEXId> {
                dex_id,
                base: *output_asset_id,
                quote: input,
            },
            output if output == *output_asset_id => OrderBookId::<AssetIdOf<T>, T::DEXId> {
                dex_id,
                base: *input_asset_id,
                quote: output,
            },
            _ => {
                return None;
            }
        };

        Some(order_book_id)
    }

    pub fn verify_create_orderbook_params(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
    ) -> Result<(), DispatchError> {
        ensure!(
            order_book_id.base != order_book_id.quote,
            Error::<T>::ForbiddenToCreateOrderBookWithSameAssets
        );

        ensure!(
            order_book_id.dex_id == common::DEXId::Polkaswap.into()
                || order_book_id.dex_id == common::DEXId::PolkaswapKUSD.into()
                || order_book_id.dex_id == common::DEXId::PolkaswapVXOR.into()
                || order_book_id.dex_id == common::DEXId::PolkaswapPresto.into(),
            Error::<T>::NotAllowedDEXId
        );

        // a quote asset of order book must be the base asset of DEX
        let dex_info = T::DexInfoProvider::get_dex_info(&order_book_id.dex_id)?;
        ensure!(
            order_book_id.quote == dex_info.base_asset_id,
            Error::<T>::NotAllowedQuoteAsset
        );

        <T as Config>::AssetInfoProvider::ensure_asset_exists(&order_book_id.base)?;
        T::EnsureTradingPairExists::ensure_trading_pair_exists(
            &order_book_id.dex_id,
            &order_book_id.quote,
            &order_book_id.base,
        )?;

        ensure!(
            !<OrderBooks<T>>::contains_key(order_book_id),
            Error::<T>::OrderBookAlreadyExists
        );
        Ok(())
    }

    fn verify_orderbook_attributes(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
        tick_size: Balance,
        step_lot_size: Balance,
        min_lot_size: Balance,
        max_lot_size: Balance,
    ) -> Result<(), DispatchError> {
        // Check that values are non-zero
        ensure!(tick_size > Balance::zero(), Error::<T>::InvalidTickSize);
        ensure!(
            step_lot_size > Balance::zero(),
            Error::<T>::InvalidStepLotSize
        );
        ensure!(
            min_lot_size > Balance::zero(),
            Error::<T>::InvalidMinLotSize
        );
        ensure!(
            max_lot_size > Balance::zero(),
            Error::<T>::InvalidMaxLotSize
        );

        // min <= max
        // It is possible to set min == max if it necessary, e.g. some NFTs
        ensure!(min_lot_size <= max_lot_size, Error::<T>::InvalidMaxLotSize);

        // min & max couldn't be less then `step_lot_size`
        ensure!(min_lot_size >= step_lot_size, Error::<T>::InvalidMinLotSize);
        ensure!(max_lot_size >= step_lot_size, Error::<T>::InvalidMaxLotSize);

        // min & max must be a multiple of `step_lot_size`
        ensure!(
            min_lot_size % step_lot_size == 0,
            Error::<T>::InvalidMinLotSize
        );
        ensure!(
            max_lot_size % step_lot_size == 0,
            Error::<T>::InvalidMaxLotSize
        );

        // check the ratio between min & max
        ensure!(
            max_lot_size <= min_lot_size.saturating_mul(T::SOFT_MIN_MAX_RATIO as Balance),
            Error::<T>::InvalidMaxLotSize
        );

        if !<T as Config>::AssetInfoProvider::is_non_divisible(&order_book_id.base) {
            // Even if `tick_size` & `step_lot_size` meet precision conditions the min possible deal amount could not match.
            // The min possible deal amount = `tick_size` * `step_lot_size`.
            // We need to be sure that the value doesn't overflow Balance if `tick_size` & `step_lot_size` are too big
            // and doesn't go out of precision.
            let _min_possible_deal_amount = (FixedWrapper::from(tick_size)
                .lossless_mul(FixedWrapper::from(step_lot_size))
                .ok_or(Error::<T>::TickSizeAndStepLotSizeLosePrecision)?)
            .try_into_balance() // Returns error if value overflows.
            .map_err(|_| Error::<T>::TickSizeAndStepLotSizeAreTooBig)?;
        }

        Ok(())
    }

    pub fn create_orderbook_unchecked(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
        tick_size: Balance,
        step_lot_size: Balance,
        min_lot_size: Balance,
        max_lot_size: Balance,
    ) -> Result<(), DispatchError> {
        let order_book = if <T as Config>::AssetInfoProvider::is_non_divisible(&order_book_id.base)
        {
            OrderBook::<T>::new(
                *order_book_id,
                OrderPrice::divisible(tick_size),
                OrderVolume::indivisible(step_lot_size),
                OrderVolume::indivisible(min_lot_size),
                OrderVolume::indivisible(max_lot_size),
            )
        } else {
            // regular asset
            OrderBook::<T>::new(
                *order_book_id,
                OrderPrice::divisible(tick_size),
                OrderVolume::divisible(step_lot_size),
                OrderVolume::divisible(min_lot_size),
                OrderVolume::divisible(max_lot_size),
            )
        };
        <OrderBooks<T>>::insert(order_book_id, order_book);
        Self::register_tech_account(order_book_id)
    }

    pub fn inner_create_orderbook(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
        tick_size: Balance,
        step_lot_size: Balance,
        min_lot_size: Balance,
        max_lot_size: Balance,
    ) -> Result<(), DispatchError> {
        Self::verify_create_orderbook_params(order_book_id)?;
        Self::verify_orderbook_attributes(
            order_book_id,
            tick_size,
            step_lot_size,
            min_lot_size,
            max_lot_size,
        )?;

        T::TradingPairSourceManager::enable_source_for_trading_pair(
            &order_book_id.dex_id,
            &order_book_id.quote,
            &order_book_id.base,
            LiquiditySourceType::OrderBook,
        )?;

        Self::create_orderbook_unchecked(
            order_book_id,
            tick_size,
            step_lot_size,
            min_lot_size,
            max_lot_size,
        )
    }

    pub fn inner_place_limit_order(
        owner: T::AccountId,
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        price: Balance,
        amount: Balance,
        side: PriceVariant,
        lifespan: Option<MomentOf<T>>,
    ) -> Result<usize, DispatchError> {
        let mut order_book =
            <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;
        let order_id = order_book.next_order_id();
        let now = T::Time::now();
        let current_block = frame_system::Pallet::<T>::block_number();
        let lifespan = lifespan.unwrap_or(T::MAX_ORDER_LIFESPAN);
        let amount = if <T as Config>::AssetInfoProvider::is_non_divisible(&order_book_id.base) {
            OrderVolume::indivisible(amount)
        } else {
            OrderVolume::divisible(amount)
        };
        let order = LimitOrder::<T>::new(
            order_id,
            owner,
            side,
            OrderPrice::divisible(price),
            amount,
            now,
            lifespan,
            current_block,
        );

        let mut data = CacheDataLayer::<T>::new();

        let executed_orders_count = order_book.place_limit_order(order, &mut data)?;

        data.commit();
        <OrderBooks<T>>::insert(order_book_id, order_book);

        Ok(executed_orders_count)
    }
}

#[cfg(feature = "private-net")]
impl<T: Config> Pallet<T> {
    /// Specifically created for `qa-tools` pallet
    pub fn deposit_event_exposed(event: Event<T>) {
        Self::deposit_event(event)
    }
}

impl<T: Config> OrderBookManager<T::AccountId, AssetIdOf<T>, T::DEXId, MomentOf<T>> for Pallet<T> {
    fn assemble_order_book_id(
        dex_id: T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
    ) -> Option<OrderBookId<AssetIdOf<T>, T::DEXId>> {
        Self::assemble_order_book_id(dex_id, input_asset_id, output_asset_id)
    }

    fn tech_account_id_for_order_book(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
    ) -> Result<T::AccountId, DispatchError> {
        let tech_account = Self::tech_account_for_order_book(order_book_id);
        let account_id = technical::Pallet::<T>::tech_account_id_to_account_id(&tech_account)?;
        Ok(account_id)
    }

    fn initialize_orderbook(
        order_book_id: &OrderBookId<AssetIdOf<T>, T::DEXId>,
        tick_size: Balance,
        step_lot_size: Balance,
        min_lot_size: Balance,
        max_lot_size: Balance,
    ) -> Result<(), DispatchError> {
        Self::inner_create_orderbook(
            order_book_id,
            tick_size,
            step_lot_size,
            min_lot_size,
            max_lot_size,
        )
    }

    fn place_limit_order(
        owner: T::AccountId,
        order_book_id: OrderBookId<AssetIdOf<T>, T::DEXId>,
        price: Balance,
        amount: Balance,
        side: PriceVariant,
        lifespan: Option<MomentOf<T>>,
    ) -> Result<(), DispatchError> {
        let _ = Self::inner_place_limit_order(owner, order_book_id, price, amount, side, lifespan)?;
        Ok(())
    }
}

impl<T: Config> LiquiditySource<T::DEXId, T::AccountId, AssetIdOf<T>, Balance, DispatchError>
    for Pallet<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
    ) -> bool {
        let Some(order_book_id) = Self::assemble_order_book_id(*dex_id, input_asset_id, output_asset_id) else {
            return false;
        };

        let Some(order_book) = <OrderBooks<T>>::get(order_book_id) else {
            return false;
        };

        order_book.status == OrderBookStatus::Trade
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        amount: QuoteAmount<Balance>,
        _deduce_fee: bool,
    ) -> Result<(SwapOutcome<Balance, AssetIdOf<T>>, Weight), DispatchError> {
        let Some(order_book_id) = Self::assemble_order_book_id(*dex_id, input_asset_id, output_asset_id) else {
            return Err(Error::<T>::UnknownOrderBook.into());
        };

        let order_book = <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;
        let mut data = CacheDataLayer::<T>::new();

        let deal_info =
            order_book.calculate_deal(input_asset_id, output_asset_id, amount, &mut data)?;

        ensure!(deal_info.is_valid(), Error::<T>::PriceCalculationFailed);

        let base_amount = deal_info.base_amount();
        ensure!(
            order_book.min_lot_size <= base_amount && base_amount <= order_book.max_lot_size,
            Error::<T>::InvalidOrderAmount
        );

        // order-book doesn't take fee
        let fee = OutcomeFee::new();

        match amount {
            QuoteAmount::WithDesiredInput { .. } => Ok((
                SwapOutcome::new(*deal_info.output_amount.value().balance(), fee),
                Self::quote_weight(),
            )),
            QuoteAmount::WithDesiredOutput { .. } => Ok((
                SwapOutcome::new(*deal_info.input_amount.value().balance(), fee),
                Self::quote_weight(),
            )),
        }
    }

    fn step_quote(
        dex_id: &T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        amount: QuoteAmount<Balance>,
        recommended_samples_count: usize,
        _deduce_fee: bool,
    ) -> Result<(DiscreteQuotation<AssetIdOf<T>, Balance>, Weight), DispatchError> {
        let Some(order_book_id) = Self::assemble_order_book_id(*dex_id, input_asset_id, output_asset_id) else {
            return Err(Error::<T>::UnknownOrderBook.into());
        };

        if amount.amount().is_zero() {
            return Ok((DiscreteQuotation::new(), Weight::zero()));
        }

        let order_book = <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;
        let mut data = CacheDataLayer::<T>::new();

        let direction = order_book.get_direction(input_asset_id, output_asset_id)?;

        let market_depth = order_book.market_depth(
            direction.switched(),
            Some(OrderAmount::Base(order_book.max_lot_size)),
            &mut data,
        );

        let (base_min_amount, quote_min_amount) = order_book.sum_market(
            market_depth
                .iter()
                .map(|(ref price, ref volume)| (price, volume)),
            Some(OrderAmount::Base(order_book.min_lot_size)),
            false,
        )?;

        if *base_min_amount.value() < order_book.min_lot_size {
            // order-book has not ehough liquidity even for min amount
            return Ok((
                DiscreteQuotation::new(),
                Self::step_quote_weight(recommended_samples_count),
            ));
        }

        let (base_max_amount, quote_max_amount) = order_book.sum_market(
            market_depth
                .iter()
                .map(|(ref price, ref volume)| (price, volume)),
            Some(OrderAmount::Base(order_book.max_lot_size)),
            false,
        )?;

        let mut quotation = DiscreteQuotation::new();

        let target = match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => match direction {
                PriceVariant::Buy => {
                    quotation.limits.min_amount =
                        Some(SideAmount::Input(*quote_min_amount.value().balance()));
                    quotation.limits.max_amount =
                        Some(SideAmount::Input(*quote_max_amount.value().balance()));
                    quotation.limits.amount_precision =
                        Some(SideAmount::Output(*order_book.step_lot_size.balance()));

                    if desired_amount_in < *quote_min_amount.value().balance() {
                        return Ok((
                            quotation,
                            Self::step_quote_weight(recommended_samples_count),
                        ));
                    }

                    let target = desired_amount_in.min(*quote_max_amount.value().balance());

                    OrderAmount::Quote(order_book.tick_size.copy_divisibility(target))
                }
                PriceVariant::Sell => {
                    quotation.limits.min_amount =
                        Some(SideAmount::Input(*base_min_amount.value().balance()));
                    quotation.limits.max_amount =
                        Some(SideAmount::Input(*base_max_amount.value().balance()));
                    quotation.limits.amount_precision =
                        Some(SideAmount::Input(*order_book.step_lot_size.balance()));

                    if desired_amount_in < *base_min_amount.value().balance() {
                        return Ok((
                            quotation,
                            Self::step_quote_weight(recommended_samples_count),
                        ));
                    }

                    let target = desired_amount_in.min(*base_max_amount.value().balance());

                    OrderAmount::Base(order_book.step_lot_size.copy_divisibility(target))
                }
            },
            QuoteAmount::WithDesiredOutput { desired_amount_out } => match direction {
                PriceVariant::Buy => {
                    quotation.limits.min_amount =
                        Some(SideAmount::Output(*base_min_amount.value().balance()));
                    quotation.limits.max_amount =
                        Some(SideAmount::Output(*base_max_amount.value().balance()));
                    quotation.limits.amount_precision =
                        Some(SideAmount::Output(*order_book.step_lot_size.balance()));

                    if desired_amount_out < *base_min_amount.value().balance() {
                        return Ok((
                            quotation,
                            Self::step_quote_weight(recommended_samples_count),
                        ));
                    }

                    let target = desired_amount_out.min(*base_max_amount.value().balance());

                    OrderAmount::Base(order_book.step_lot_size.copy_divisibility(target))
                }
                PriceVariant::Sell => {
                    quotation.limits.min_amount =
                        Some(SideAmount::Output(*quote_min_amount.value().balance()));
                    quotation.limits.max_amount =
                        Some(SideAmount::Output(*quote_max_amount.value().balance()));
                    quotation.limits.amount_precision =
                        Some(SideAmount::Input(*order_book.step_lot_size.balance()));

                    if desired_amount_out < *quote_min_amount.value().balance() {
                        return Ok((
                            quotation,
                            Self::step_quote_weight(recommended_samples_count),
                        ));
                    }

                    let target = desired_amount_out.min(*quote_max_amount.value().balance());

                    OrderAmount::Quote(order_book.tick_size.copy_divisibility(target))
                }
            },
        };

        let mut target_sum = BalanceUnit::zero();
        for (price, base_volume) in market_depth.iter() {
            let quote_volume = price
                .checked_mul(base_volume)
                .ok_or(Error::<T>::AmountCalculationFailed)?;

            match direction {
                PriceVariant::Buy => quotation.chunks.push_back(SwapChunk::new(
                    *quote_volume.balance(),
                    *base_volume.balance(),
                    Default::default(),
                )),
                PriceVariant::Sell => quotation.chunks.push_back(SwapChunk::new(
                    *base_volume.balance(),
                    *quote_volume.balance(),
                    Default::default(),
                )),
            }

            target_sum = match target {
                OrderAmount::Base(..) => target_sum
                    .checked_add(base_volume)
                    .ok_or(Error::<T>::AmountCalculationFailed)?,
                OrderAmount::Quote(..) => target_sum
                    .checked_add(&quote_volume)
                    .ok_or(Error::<T>::AmountCalculationFailed)?,
            };
            if target_sum >= *target.value() {
                break;
            }
        }

        Ok((
            quotation,
            Self::step_quote_weight(recommended_samples_count),
        ))
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        desired_amount: SwapAmount<Balance>,
    ) -> Result<(SwapOutcome<Balance, AssetIdOf<T>>, Weight), DispatchError> {
        let Some(order_book_id) = Self::assemble_order_book_id(*dex_id, input_asset_id, output_asset_id) else {
            return Err(Error::<T>::UnknownOrderBook.into());
        };

        let order_book = <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;
        let mut data = CacheDataLayer::<T>::new();

        let deal_info = order_book.calculate_deal(
            input_asset_id,
            output_asset_id,
            desired_amount.into(),
            &mut data,
        )?;

        ensure!(deal_info.is_valid(), Error::<T>::PriceCalculationFailed);

        match desired_amount {
            SwapAmount::WithDesiredInput { min_amount_out, .. } => {
                ensure!(
                    *deal_info.output_amount.value().balance() >= min_amount_out,
                    Error::<T>::SlippageLimitExceeded
                );
            }
            SwapAmount::WithDesiredOutput { max_amount_in, .. } => {
                ensure!(
                    *deal_info.input_amount.value().balance() <= max_amount_in,
                    Error::<T>::SlippageLimitExceeded
                );
            }
        }

        let to = if sender == receiver {
            None
        } else {
            Some(receiver.clone())
        };

        let direction = deal_info.direction;
        let amount = deal_info.base_amount();

        let market_order =
            MarketOrder::<T>::new(sender.clone(), direction, order_book_id, amount, to);

        let (input_amount, output_amount, executed_orders_count) =
            order_book.execute_market_order(market_order, &mut data)?;

        // order-book doesn't take fee
        let fee = OutcomeFee::new();

        let result = match desired_amount {
            SwapAmount::WithDesiredInput { min_amount_out, .. } => {
                ensure!(
                    *output_amount.value().balance() >= min_amount_out,
                    Error::<T>::SlippageLimitExceeded
                );
                SwapOutcome::new(*output_amount.value().balance(), fee)
            }
            SwapAmount::WithDesiredOutput { max_amount_in, .. } => {
                ensure!(
                    *input_amount.value().balance() <= max_amount_in,
                    Error::<T>::SlippageLimitExceeded
                );
                SwapOutcome::new(*input_amount.value().balance(), fee)
            }
        };

        data.commit();

        let weight = <T as Config>::WeightInfo::exchange(executed_orders_count as u32);

        Ok((result, weight))
    }

    fn check_rewards(
        _dex_id: &T::DEXId,
        _input_asset_id: &AssetIdOf<T>,
        _output_asset_id: &AssetIdOf<T>,
        _input_amount: Balance,
        _output_amount: Balance,
    ) -> Result<(Vec<(Balance, AssetIdOf<T>, RewardReason)>, Weight), DispatchError> {
        Ok((Vec::new(), Weight::zero())) // no rewards for Order Book
    }

    fn quote_without_impact(
        dex_id: &T::DEXId,
        input_asset_id: &AssetIdOf<T>,
        output_asset_id: &AssetIdOf<T>,
        amount: QuoteAmount<Balance>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetIdOf<T>>, DispatchError> {
        let Some(order_book_id) = Self::assemble_order_book_id(*dex_id, input_asset_id, output_asset_id) else {
            return Err(Error::<T>::UnknownOrderBook.into());
        };

        let order_book = <OrderBooks<T>>::get(order_book_id).ok_or(Error::<T>::UnknownOrderBook)?;
        let mut data = CacheDataLayer::<T>::new();

        let direction = order_book.get_direction(input_asset_id, output_asset_id)?;

        let Some((price, _)) = (match direction {
            PriceVariant::Buy => order_book.best_ask(&mut data),
            PriceVariant::Sell => order_book.best_bid(&mut data),
        }) else {
            return Err(Error::<T>::NotEnoughLiquidityInOrderBook.into());
        };

        let (target_amount, base_amount) = match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => match direction {
                // User wants to swap a known amount of the `quote` asset for the `base` asset.
                // Necessary to return `base` amount.
                // Divide the `quote` amount by the price and align the `base` amount.
                PriceVariant::Buy => {
                    let base_amount = order_book.align_amount(
                        order_book
                            .tick_size
                            .copy_divisibility(desired_amount_in)
                            .checked_div(&price)
                            .ok_or(Error::<T>::AmountCalculationFailed)?,
                    );
                    (base_amount, base_amount)
                }

                // User wants to swap a known amount of the `base` asset for the `quote` asset.
                // Necessary to return `quote` amount.
                // Align the `base` amount and then multiply by the price.
                PriceVariant::Sell => {
                    let base_amount = order_book.align_amount(
                        order_book
                            .step_lot_size
                            .copy_divisibility(desired_amount_in),
                    );
                    let quote_amount = base_amount
                        .checked_mul(&price)
                        .ok_or(Error::<T>::AmountCalculationFailed)?;
                    (quote_amount, base_amount)
                }
            },

            QuoteAmount::WithDesiredOutput { desired_amount_out } => match direction {
                // User wants to swap the `quote` asset for a known amount of the `base` asset.
                // Necessary to return `quote` amount.
                // Align the `base` amount and then multiply by the price.
                PriceVariant::Buy => {
                    let base_amount = order_book.align_amount(
                        order_book
                            .step_lot_size
                            .copy_divisibility(desired_amount_out),
                    );
                    let quote_amount = base_amount
                        .checked_mul(&price)
                        .ok_or(Error::<T>::AmountCalculationFailed)?;
                    (quote_amount, base_amount)
                }

                // User wants to swap the `base` asset for a known amount of the `quote` asset.
                // Necessary to return `base` amount.
                PriceVariant::Sell => {
                    let base_amount = order_book.align_amount(
                        order_book
                            .tick_size
                            .copy_divisibility(desired_amount_out)
                            .checked_div(&price)
                            .ok_or(Error::<T>::AmountCalculationFailed)?,
                    );
                    (base_amount, base_amount)
                }
            },
        };

        ensure!(
            target_amount > OrderVolume::zero(),
            Error::<T>::InvalidOrderAmount
        );

        ensure!(
            order_book.min_lot_size <= base_amount && base_amount <= order_book.max_lot_size,
            Error::<T>::InvalidOrderAmount
        );

        // order-book doesn't take fee
        let fee = OutcomeFee::new();

        Ok(SwapOutcome::new(*target_amount.balance(), fee))
    }

    fn quote_weight() -> Weight {
        <T as Config>::WeightInfo::quote()
    }

    fn step_quote_weight(_samples_count: usize) -> Weight {
        <T as Config>::WeightInfo::step_quote()
    }

    fn exchange_weight() -> Weight {
        <T as Config>::WeightInfo::exchange(
            T::REGULAR_NUBMER_OF_EXECUTED_ORDERS.try_into().unwrap(),
        )
    }

    fn check_rewards_weight() -> Weight {
        Weight::zero()
    }
}
