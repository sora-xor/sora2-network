//! # ETH
//!
//! An application that implements a bridged ETH asset.
//!
//! ## Overview
//!
//! ETH balances are stored in the tightly-coupled [`asset`] runtime module. When an account holder burns
//! some of their balance, a `Transfer` event is emit for this event
//! and relay it to the other chain.
//!
//! ## Interface
//!
//! ### Dispatchable Calls
//!
//! - `burn`: Burn an ETH balance.
//!
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::DispatchResult;
use frame_support::traits::EnsureOrigin;
use frame_support::transactional;
use frame_support::weights::Weight;
use frame_system::ensure_signed;
use snowbridge_ethereum::EthNetworkId;
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
    fn register_new_asset() -> Weight;
}

impl WeightInfo for () {
    fn burn() -> Weight {
        0
    }
    fn mint() -> Weight {
        0
    }

    fn register_new_asset() -> Weight {
        0
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use assets::AssetIdOf;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::{OriginFor, *};
    use traits::MultiCurrency;

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type BalanceOf<T> = <<T as assets::Config>::Currency as MultiCurrency<AccountIdOf<T>>>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        type OutboundRouter: OutboundRouter<Self::AccountId>;

        type CallOrigin: EnsureOrigin<Self::Origin, Success = (EthNetworkId, H160)>;

        type FeeCurrency: Get<Self::AssetId>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn address)]
    pub(super) type Address<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Identity, H160, AssetIdOf<T>, OptionQuery>;

    /// Destination account for bridge funds
    #[pallet::storage]
    pub type DestAccount<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Identity, H160, T::AccountId, OptionQuery>;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
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
        BridgeNotFound,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // Users should burn their holdings to release funds on the Ethereum side
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        #[transactional]
        pub fn burn(
            origin: OriginFor<T>,
            channel_id: ChannelId,
            network_id: EthNetworkId,
            channel: H160,
            bridge: H160,
            recipient: H160,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let dest =
                DestAccount::<T>::get(network_id, bridge).ok_or(Error::<T>::BridgeNotFound)?;
            let asset_id =
                <Address<T>>::get(network_id, bridge).ok_or(Error::<T>::BridgeNotFound)?;

            T::Currency::transfer(asset_id, &who, &dest, amount)?;

            let message = OutboundPayload {
                sender: who.clone(),
                recipient: recipient.clone(),
                amount: amount.into(),
            };

            T::OutboundRouter::submit(
                network_id,
                channel,
                channel_id,
                &who,
                bridge,
                &message.encode(),
            )?;
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
            let (network_id, who) = T::CallOrigin::ensure_origin(origin)?;
            let asset = Address::<T>::get(network_id, who).ok_or(Error::<T>::BridgeNotFound)?;

            let recipient = T::Lookup::lookup(recipient)?;
            T::Currency::deposit(asset, &recipient, amount)?;
            Self::deposit_event(Event::Minted(sender, recipient.clone(), amount.into()));

            Ok(())
        }

        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn register_new_asset(
            origin: OriginFor<T>,
            dest_account: T::AccountId,
            network_id: EthNetworkId,
            asset_id: AssetIdOf<T>,
            channel: H160,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            <DestAccount<T>>::insert(network_id, channel, dest_account);
            <Address<T>>::insert(network_id, channel, asset_id);
            Ok(().into())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub networks: Vec<(EthNetworkId, Vec<(H160, T::AccountId, T::AssetId)>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                networks: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (network_id, channels) in &self.networks {
                for (channel, dest_account, asset_id) in channels {
                    <DestAccount<T>>::insert(network_id, channel, dest_account.clone());
                    <Address<T>>::insert(network_id, channel, asset_id);
                }
            }
        }
    }
}
