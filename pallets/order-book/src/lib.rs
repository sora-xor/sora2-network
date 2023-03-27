#![cfg_attr(not(feature = "std"), no_std)]

use book::{OrderData, OrderId};
use common::fixnum::FixedPoint;
use common::{Balance, FixedInner};
use frame_support::ensure;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::Get;
use sp_runtime::traits::Zero;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

type FixedPrice<P> = FixedPoint<FixedInner, P>;

mod book {
    use codec::{Decode, Encode, MaxEncodedLen};
    use common::{
        fixnum::{typenum::Unsigned, FixedPoint},
        Balance, FixedInner,
    };
    use core::marker::PhantomData;
    use frame_support::{traits::Get, BoundedBTreeMap};
    use scale_info::TypeInfo;
    use std::collections::VecDeque;

    use crate::OrderKind;

    // todo: need to consider how they're assigned
    // in special cases, such as partial order execution.
    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug)]
    pub struct OrderId(pub u128);

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug)]
    pub struct OrderData<Timestamp> {
        pub id: OrderId,
        pub expires_at: Timestamp,
        pub amount: Balance,
    }

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Debug)]
    pub struct BuyLimitOrder<Timestamp>(OrderData<Timestamp>);

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Debug)]
    pub struct SellLimitOrder<Timestamp>(OrderData<Timestamp>);

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Debug)]
    pub enum OrderList<Timestamp> {
        // todo: use something bounded
        Buy(VecDeque<BuyLimitOrder<Timestamp>>),
        Sell(VecDeque<SellLimitOrder<Timestamp>>),
    }

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Debug)]
    pub struct PricePoint<Timestamp> {
        cumulative_amount: Balance,
        orders: OrderList<Timestamp>,
    }

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Debug)]
    pub struct OrderBook<Timestamp, PriceCountLimit: Get<u32>, PricePrecision: Unsigned> {
        // todo: maybe it'll be cleaner to have FixedPoint<_, _> as parameter, then "extract" inner and precision from them
        prices: BoundedBTreeMap<FixedInner, PricePoint<Timestamp>, PriceCountLimit>,
        lowest_sell_price: Option<FixedInner>,
        highest_buy_price: Option<FixedInner>,
        some_price_limit: FixedInner,
        // so we can verify it statically when interacting
        price_precision: PhantomData<PricePrecision>,
    }

    pub struct CrossedOrder;

    impl<Timestamp, PriceCountLimit, PricePrecision>
        OrderBook<Timestamp, PriceCountLimit, PricePrecision>
    where
        PriceCountLimit: Get<u32>,
        PricePrecision: Unsigned,
    {
        pub fn new(some_price_limit: FixedPoint<FixedInner, PricePrecision>) -> Self {
            Self {
                prices: BoundedBTreeMap::new(),
                lowest_sell_price: None,
                highest_buy_price: None,
                some_price_limit: some_price_limit.into_bits(),
                price_precision: PhantomData,
            }
        }

        pub fn try_place(
            &mut self,
            price: FixedPoint<FixedInner, PricePrecision>,
            data: OrderData<Timestamp>,
        ) -> Result<(), CrossedOrder> {
            Ok(())
        }
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use common::{fixnum::typenum::Unsigned, TradingPair};
    use frame_support::{
        pallet_prelude::{OptionQuery, *},
        Blake2_128Concat,
    };
    use frame_system::pallet_prelude::*;

    use super::*;
    use book::{OrderBook, OrderData, OrderId};

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type PricePrecision: Unsigned + Eq + TypeInfo;
        type PriceCountLimitPerBook: Get<u32> + TypeInfo;
        type OrderLifetimeLimit: Get<Self::BlockNumber>;
    }

    pub type Timestamp<T: Config> = T::BlockNumber;

    // TODO: figure out how to store orders in an optimal way
    #[pallet::storage]
    // todo: remove unbounded
    #[pallet::unbounded]
    #[pallet::getter(fn order_book)]
    pub type OrderBooks<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        TradingPair<T::AssetId>,
        OrderBook<Timestamp<T>, T::PriceCountLimitPerBook, T::PricePrecision>,
        OptionQuery,
    >;

    // to find order in `OrderBooks` by id (e.g. for revoking the order).
    #[pallet::storage]
    #[pallet::getter(fn order_lookup)]
    pub type OrderInfo<T: Config> =
        StorageMap<_, Blake2_128Concat, OrderId, (TradingPair<T::AssetId>, FixedInner)>;

    #[pallet::type_value]
    pub(super) fn DefaultForNextOrderId<T: Config>() -> OrderId {
        OrderId(0)
    }

    #[pallet::storage]
    pub type NextOrderId<T: Config> =
        StorageValue<_, OrderId, ValueQuery, DefaultForNextOrderId<T>>;

    // Pallets use events to inform users when important changes are made.
    // https://docs.substrate.io/main-docs/build/events-errors/
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Event documentation should end with an array that provides descriptive names for event
        /// parameters. [something, who]
        OrderPlaced {
            trading_pair: TradingPair<T::AssetId>,
            order: OrderData<Timestamp<T>>,
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
        UnknownTradingPair,
        /// Price for the order crosses the order book
        CrossingOrder,
        // ...
    }

    #[derive(Encode, Decode, PartialEq, Eq, Debug, Clone, TypeInfo)]
    pub enum OrderKind {
        Buy,
        Sell,
    }

    // Dispatchable functions allows users to interact with the pallet and invoke state changes.
    // These functions materialize as "extrinsics", which are often compared to transactions.
    // Dispatchable functions must be annotated with a weight and must return a DispatchResult.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        // todo: benchmark
        #[pallet::weight(10_000 + T::DbWeight::get().writes(2).ref_time() + T::DbWeight::get().reads(2).ref_time())]
        pub fn place_limit_order(
            origin: OriginFor<T>,
            trading_pair: TradingPair<T::AssetId>,
            price: FixedPoint<FixedInner, T::PricePrecision>,
            amount: Balance,
            kind: OrderKind,
            lifespan: Timestamp<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let mut order_book =
                <OrderBooks<T>>::get(trading_pair).ok_or(Error::<T>::UnknownTradingPair)?;
            let order = Self::construct_order_data(amount, lifespan)?;
            order_book
                .try_place(price, order.clone())
                .map_err(|_| Error::<T>::CrossingOrder)?;
            <OrderInfo<T>>::insert(order.id, (trading_pair, price.into_bits()));
            Self::deposit_event(Event::OrderPlaced {
                trading_pair,
                order,
            });
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    fn construct_order_data(
        amount: Balance,
        lifespan: Timestamp<T>,
    ) -> Result<OrderData<T::BlockNumber>, DispatchError> {
        let id = <NextOrderId<T>>::get();
        <NextOrderId<T>>::put(OrderId(id.0 + 1));
        let current_block_number = <frame_system::Pallet<T>>::block_number();
        ensure!(
            lifespan > T::BlockNumber::zero() && lifespan < T::OrderLifetimeLimit::get(),
            Error::<T>::InvalidLifespan
        );
        let expires_at = current_block_number + lifespan;
        Ok(OrderData {
            id,
            expires_at,
            amount,
        })
    }
}
