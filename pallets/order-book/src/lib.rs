#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use common::fixnum::FixedPoint;
use common::{Balance, FixedInner};
use scale_info::TypeInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

type FixedPrice<P> = FixedPoint<FixedInner, P>;

mod book {
    use codec::{Decode, Encode, MaxEncodedLen};
    use common::{Balance, FixedInner};
    use frame_support::{traits::Get, BoundedBTreeMap};
    use scale_info::TypeInfo;
    use std::collections::VecDeque;

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug)]
    pub struct OrderId(u128);

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug)]
    pub struct OrderData<Timestamp> {
        id: OrderId,
        expires_at: Timestamp,
        amount: Balance,
    }

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Debug)]
    pub struct BuyLimitOrder<Timestamp>(OrderData<Timestamp>);

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Debug)]
    pub struct SellLimitOrder<Timestamp>(OrderData<Timestamp>);

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Debug)]
    pub enum OrderList<Timestamp> {
        Buy(VecDeque<BuyLimitOrder<Timestamp>>),
        Sell(VecDeque<SellLimitOrder<Timestamp>>),
    }

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Debug)]
    pub struct PricePoint<Timestamp> {
        cumulative_amount: Balance,
        orders: OrderList<Timestamp>,
    }

    #[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, Debug)]
    pub struct OrderBook<Timestamp, PriceCountLimit: Get<u32>> {
        pub prices: BoundedBTreeMap<FixedInner, PricePoint<Timestamp>, PriceCountLimit>,
        some_price_limit: FixedInner,
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use common::{fixnum::typenum::Integer, TradingPair};
    use frame_support::{
        pallet_prelude::{OptionQuery, *},
        Blake2_128Concat, Twox64Concat,
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
        type PricePrecision: Integer;
        type PriceCountLimitPerBook: Get<u32> + TypeInfo;
    }

    pub type Timestamp<T: Config> = T::BlockNumber;

    // TODO: figure out how to store orders in an optimal way
    #[pallet::storage]
    #[pallet::getter(fn order_book)]
    pub type OrderBooks<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        TradingPair<T::AssetId>,
        OrderBook<Timestamp<T>, T::PriceCountLimitPerBook>,
        OptionQuery,
    >;

    // to find order in `OrderBooks` by id (e.g. for revoking the order).
    #[pallet::storage]
    #[pallet::getter(fn order_lookup)]
    pub type OrderInfo<T: Config> =
        StorageMap<_, Blake2_128Concat, OrderId, (TradingPair<T::AssetId>, FixedInner)>;

    // Pallets use events to inform users when important changes are made.
    // https://docs.substrate.io/main-docs/build/events-errors/
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Event documentation should end with an array that provides descriptive names for event
        /// parameters. [something, who]
        OrderPlaced { order: OrderData<Timestamp<T>> },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Trading pair currently reached its capacity
        OrderLimitReached,
        /// Price in given order exceeds allowed limits for the trading pair
        PriceExceedsLimits,
        // ...
    }

    // Dispatchable functions allows users to interact with the pallet and invoke state changes.
    // These functions materialize as "extrinsics", which are often compared to transactions.
    // Dispatchable functions must be annotated with a weight and must return a DispatchResult.
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(10_000 + T::DbWeight::get().writes(1).ref_time())]
        pub fn place_limit_order(
            origin: OriginFor<T>,
            order: OrderData<Timestamp<T>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Update storage.
            <Something<T>>::put(something);

            // Emit an event.
            Self::deposit_event(Event::SomethingStored { something, who });
            // Return a successful DispatchResultWithPostInfo
            Ok(())
        }

        /// An example dispatchable that may throw a custom error.
        #[pallet::call_index(1)]
        #[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1).ref_time())]
        pub fn cause_error(origin: OriginFor<T>) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // Read a value from storage.
            match <Something<T>>::get() {
                // Return an error if the value has not been set.
                None => return Err(Error::<T>::NoneValue.into()),
                Some(old) => {
                    // Increment the value read from storage; will error in the event of overflow.
                    let new = old.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
                    // Update the value in storage with the incremented result.
                    <Something<T>>::put(new);
                    Ok(())
                }
            }
        }
    }
}
