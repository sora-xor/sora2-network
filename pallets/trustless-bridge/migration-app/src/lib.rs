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
use frame_support::weights::Weight;
use sp_std::prelude::*;

use bridge_types::traits::OutboundChannel;
use bridge_types::EVMChainId;
use bridge_types::H160;

mod payload;
use payload::MigrateErc20Payload;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// Weight functions needed for this pallet.
pub trait WeightInfo {
    fn burn() -> Weight;
    fn mint() -> Weight;
    fn register_network() -> Weight;
}

impl WeightInfo for () {
    fn burn() -> Weight {
        Weight::zero()
    }
    fn mint() -> Weight {
        Weight::zero()
    }
    fn register_network() -> Weight {
        Weight::zero()
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::evm::AdditionalEVMOutboundData;
    use bridge_types::types::AssetKind;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::{OriginFor, *};
    use frame_system::RawOrigin;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + assets::Config
        + technical::Config
        + permissions::Config
        + erc20_app::Config
        + eth_app::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type OutboundChannel: OutboundChannel<
            EVMChainId,
            Self::AccountId,
            AdditionalEVMOutboundData,
        >;

        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn address_and_asset)]
    pub(super) type Addresses<T: Config> = StorageMap<_, Identity, EVMChainId, H160, OptionQuery>;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    /// Events for the ETH module.
    pub enum Event<T: Config> {
        Erc20Migrated(EVMChainId, H160),
        SidechainMigrated(EVMChainId, H160),
        EthMigrated(EVMChainId, H160),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The submitted payload could not be decoded.
        InvalidPayload,
        /// App for given network is not registered.
        AppIsNotRegistered,
        /// Message came from wrong address.
        InvalidAppAddress,
        /// App for given network exists.
        AppAlreadyExists,
        /// Token already registered with another address.
        TokenRegisteredWithAnotherAddress,
        /// Call encoding failed.
        CallEncodeFailed,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // Send ERC20 tokens to ERC20 App address and register tokens.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn migrate_erc20(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            erc20_assets: Vec<(erc20_app::AssetIdOf<T>, H160, u8)>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let target = Addresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            let contract_address =
                erc20_app::Pallet::<T>::app_address(network_id, AssetKind::Sidechain)
                    .ok_or(Error::<T>::AppIsNotRegistered)?;

            let mut erc20_tokens = vec![];
            for (asset_id, address, precision) in erc20_assets {
                erc20_tokens.push(address);
                if let Some(registered_token) =
                    erc20_app::Pallet::<T>::token_address(network_id, &asset_id)
                {
                    if registered_token != address {
                        return Err(Error::<T>::TokenRegisteredWithAnotherAddress.into());
                    }
                } else {
                    erc20_app::Pallet::<T>::register_asset_inner(
                        network_id,
                        asset_id,
                        address,
                        AssetKind::Sidechain,
                        precision,
                    )?;
                }
            }

            let message = MigrateErc20Payload {
                contract_address,
                erc20_tokens,
            };

            <T as Config>::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &message.encode().map_err(|_| Error::<T>::CallEncodeFailed)?,
                AdditionalEVMOutboundData {
                    target,
                    max_gas: 2000000u64.into(),
                },
            )?;
            Self::deposit_event(Event::Erc20Migrated(network_id, contract_address));

            Ok(())
        }

        // Transfer ownership of tokens to Sidechain App and register tokens.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn migrate_sidechain(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            sidechain_assets: Vec<(erc20_app::AssetIdOf<T>, H160, u8)>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let target = Addresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            let contract_address =
                erc20_app::Pallet::<T>::app_address(network_id, AssetKind::Thischain)
                    .ok_or(Error::<T>::AppIsNotRegistered)?;

            let mut sidechain_tokens = vec![];
            for (asset_id, address, precision) in sidechain_assets {
                sidechain_tokens.push(address);
                if let Some(_) = erc20_app::Pallet::<T>::token_address(network_id, &asset_id) {
                    return Err(Error::<T>::TokenRegisteredWithAnotherAddress.into());
                } else {
                    erc20_app::Pallet::<T>::register_asset_inner(
                        network_id,
                        asset_id,
                        address,
                        AssetKind::Thischain,
                        precision,
                    )?;
                }
            }

            let message = payload::MigrateSidechainPayload {
                contract_address,
                sidechain_tokens,
            };

            <T as Config>::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &message.encode().map_err(|_| Error::<T>::CallEncodeFailed)?,
                AdditionalEVMOutboundData {
                    target,
                    max_gas: 2000000u64.into(),
                },
            )?;
            Self::deposit_event(Event::SidechainMigrated(network_id, contract_address));

            Ok(())
        }

        // Transfer Eth tokens to Eth App contract
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn migrate_eth(origin: OriginFor<T>, network_id: EVMChainId) -> DispatchResult {
            ensure_root(origin)?;
            let target = Addresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            let (contract_address, _, _) = eth_app::Pallet::<T>::address_and_asset(network_id)
                .ok_or(Error::<T>::AppIsNotRegistered)?;

            let message = payload::MigrateEthPayload { contract_address };

            <T as Config>::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &message.encode().map_err(|_| Error::<T>::CallEncodeFailed)?,
                AdditionalEVMOutboundData {
                    target,
                    max_gas: 2000000u64.into(),
                },
            )?;
            Self::deposit_event(Event::SidechainMigrated(network_id, contract_address));

            Ok(())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::register_network())]
        pub fn register_network(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            contract: H160,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !Addresses::<T>::contains_key(network_id),
                Error::<T>::AppAlreadyExists
            );
            Self::register_network_inner(network_id, contract)?;
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        fn register_network_inner(network_id: EVMChainId, contract: H160) -> DispatchResult {
            Addresses::<T>::insert(network_id, contract);
            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub networks: Vec<(EVMChainId, H160)>,
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {
                networks: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            for (network_id, contract) in &self.networks {
                Pallet::<T>::register_network_inner(*network_id, *contract).unwrap();
            }
        }
    }
}
