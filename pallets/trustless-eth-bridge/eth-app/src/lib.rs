//! # ETH
//!
//! An application that implements a bridged ETH asset.
//!
//! ## Overview
//!
//! ETH balances are stored in the tightly-coupled [`asset`] runtime module. When an account holder burns
//! some of their balance, a `Transfer` event is emitteframe_supportal{log::debug, pallet_prelude::*}_prelude::*} for this event
//! and relay it to the other chain.
//!
//! ## Interface
//!
//! ### Dispatchable Calls
//!
//! - `burn`: Burn an ETH balance.
//!
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::traits::EnsureOrigin;
use frame_support::transactional;
use frame_support::weights::Weight;
use frame_system::ensure_signed;
use sp_core::{H160, U256};
use sp_runtime::traits::StaticLookup;
use sp_std::prelude::*;

use snowbridge_core::{ChannelId, OutboundRouter};

mod payload;
use payload::OutboundPayload;

// mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

/// Weight functions needed for this pallet.
pub trait WeightInfo {
    fn burn() -> Weight;
    fn mint() -> Weight;
}

impl WeightInfo for () {
    fn burn() -> Weight {
        0
    }
    fn mint() -> Weight {
        0
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::log::{debug, warn};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::{OriginFor, *};
    use traits::MultiCurrency;

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type BalanceOf<T> = <<T as assets::Config>::Currency as MultiCurrency<AccountIdOf<T>>>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type OutboundRouter: OutboundRouter<Self::AccountId>;

        type CallOrigin: EnsureOrigin<Self::Origin, Success = H160>;

        type FeeCurrency: Get<Self::AssetId>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn address)]
    pub(super) type Address<T: Config> = StorageValue<_, H160, ValueQuery>;

    /// Destination account for bridge funds
    #[pallet::storage]
    pub type DestAccount<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    /// Events for the ETH module.
    pub enum Event<T: Config> {
        Burned(AccountIdOf<T>, H160, U256),
        Minted(H160, AccountIdOf<T>, U256),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The submitted payload could not be decoded.
        InvalidPayload,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // Users should burn their holdings to release funds on the Ethereum side
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        #[transactional]
        pub fn burn(
            origin: OriginFor<T>,
            channel_id: ChannelId,
            recipient: H160,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            T::Currency::transfer(
                T::FeeCurrency::get(),
                &who,
                &DestAccount::<T>::get(),
                amount,
            )?;

            let message = OutboundPayload {
                sender: who.clone(),
                recipient: recipient.clone(),
                amount: amount.into(),
            };

            T::OutboundRouter::submit(channel_id, &who, Address::<T>::get(), &message.encode())?;
            Self::deposit_event(Event::Burned(who.clone(), recipient, amount.into()));

            Ok(())
        }

        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        #[transactional]
        pub fn mint(
            origin: OriginFor<T>,
            sender: H160,
            recipient: <T::Lookup as StaticLookup>::Source,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = T::CallOrigin::ensure_origin(origin)?;
            if who != Address::<T>::get() {
                return Err(DispatchError::BadOrigin.into());
            }

            let recipient = T::Lookup::lookup(recipient)?;
            T::Currency::deposit(T::FeeCurrency::get(), &recipient, amount)?;
            Self::deposit_event(Event::Minted(sender, recipient.clone(), amount.into()));

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub address: H160,
        pub dest_account: T::AccountId,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                address: Default::default(),
                dest_account: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            Address::<T>::set(self.address);
            DestAccount::<T>::set(self.dest_account.clone());
        }
    }
}
