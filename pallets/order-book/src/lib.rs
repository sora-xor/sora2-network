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

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
pub enum LimitOrder<Timestamp> {
    Buy(OrderData<Timestamp>),
    Sell(OrderData<Timestamp>),
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug)]
pub struct OrderId(u128);

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug)]
pub struct OrderData<Timestamp> {
    id: OrderId,
    expires_at: Timestamp,
    amount: Balance,
}

// pub struct

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use common::fixnum::typenum::Integer;
    use frame_support::{
        pallet_prelude::{OptionQuery, *},
        Twox64Concat,
    };
    use frame_system::pallet_prelude::*;

    use super::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type PricePrecision: Integer;
    }

    pub type Timestamp<T: Config> = T::BlockNumber;

    // TODO: figure out how to store orders in an optimal way, maybe implement b-tree on top of storage.
    #[pallet::storage]
    #[pallet::getter(fn order)]
    pub type Orders<T: Config> = StorageMap<_, Twox64Concat, FixedInner, LimitOrder<Timestamp<T>>>;

    /// Asset Id -> Owner Account Id
    #[pallet::storage]
    #[pallet::getter(fn asset_owner)]
    pub type AssetOwners<T: Config> =
        StorageMap<_, Twox64Concat, T::AssetId, T::AccountId, OptionQuery>;

    // #[pallet::storage]
    // #[pallet::getter(fn trade_pair)]
    // pub type TradingPairs<T: Config> = StorageDoubleMap<_, Twox64Concat, T::AssetId, Twox64Concat, T::AssetId, LimitOrder<Timestamp<T>>, OptionQuery>;

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
        pub fn place_order(origin: OriginFor<T>, something: u32) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer.
            // This function will return an error if the extrinsic is not signed.
            // https://docs.substrate.io/main-docs/build/origins/
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
