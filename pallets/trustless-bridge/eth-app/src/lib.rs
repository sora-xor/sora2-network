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
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

pub const TRANSFER_MAX_GAS: u64 = 100_000;

use bridge_types::types::AssetKind;
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::traits::EnsureOrigin;
use frame_system::ensure_signed;
use sp_runtime::traits::StaticLookup;
use sp_std::prelude::*;
use sp_std::vec;

use bridge_types::traits::OutboundChannel;
use bridge_types::EVMChainId;

mod payload;
use payload::OutboundPayload;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::evm::*;
    use bridge_types::traits::{
        BalancePrecisionConverter, BridgeApp, BridgeAssetLocker, BridgeAssetRegistry,
        MessageStatusNotifier,
    };
    use bridge_types::types::{BridgeAppInfo, BridgeAssetInfo, CallOriginOutput, MessageStatus};
    use bridge_types::MainnetAssetId;
    use bridge_types::{GenericAccount, GenericNetworkId, H256};
    use bridge_types::{H160, U256};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_support::transactional;
    use frame_system::pallet_prelude::{OriginFor, *};
    use frame_system::RawOrigin;
    use sp_runtime::traits::{Convert, Zero};

    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    pub type AssetIdOf<T> =
        <<T as Config>::BridgeAssetLocker as BridgeAssetLocker<AccountIdOf<T>>>::AssetId;

    pub type BalanceOf<T> =
        <<T as Config>::BridgeAssetLocker as BridgeAssetLocker<AccountIdOf<T>>>::Balance;
    pub type AssetNameOf<T> = <<T as Config>::AssetRegistry as BridgeAssetRegistry<
        AccountIdOf<T>,
        AssetIdOf<T>,
    >>::AssetName;
    pub type AssetSymbolOf<T> = <<T as Config>::AssetRegistry as BridgeAssetRegistry<
        AccountIdOf<T>,
        AssetIdOf<T>,
    >>::AssetSymbol;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type OutboundChannel: OutboundChannel<
            EVMChainId,
            Self::AccountId,
            AdditionalEVMOutboundData,
        >;

        type CallOrigin: EnsureOrigin<
            Self::RuntimeOrigin,
            Success = CallOriginOutput<EVMChainId, H256, AdditionalEVMInboundData>,
        >;

        type MessageStatusNotifier: MessageStatusNotifier<
            AssetIdOf<Self>,
            Self::AccountId,
            BalanceOf<Self>,
        >;

        type AssetRegistry: BridgeAssetRegistry<Self::AccountId, AssetIdOf<Self>>;

        type AssetIdConverter: Convert<AssetIdOf<Self>, MainnetAssetId>;

        type BalancePrecisionConverter: BalancePrecisionConverter<
            AssetIdOf<Self>,
            BalanceOf<Self>,
            U256,
        >;

        type BridgeAssetLocker: BridgeAssetLocker<Self::AccountId>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn address_and_asset)]
    pub(super) type Addresses<T: Config> =
        StorageMap<_, Identity, EVMChainId, (H160, AssetIdOf<T>, u8), OptionQuery>;

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
        Burned(EVMChainId, AccountIdOf<T>, H160, BalanceOf<T>),
        Minted(EVMChainId, H160, AccountIdOf<T>, BalanceOf<T>),
        Refunded(EVMChainId, AccountIdOf<T>, BalanceOf<T>),
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
        /// Destination account is not set.
        DestAccountIsNotSet,
        /// Call encoding failed.
        CallEncodeFailed,
        /// Amount must be > 0
        WrongAmount,
        /// Wrong bridge request for refund
        WrongRequest,
        /// Wrong bridge request status, must be Failed
        WrongRequestStatus,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // Users should burn their holdings to release funds on the Ethereum side
        #[transactional]
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn burn(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            recipient: H160,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Pallet::<T>::burn_inner(who, network_id, recipient, amount)?;
            Ok(())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::mint())]
        pub fn mint(
            origin: OriginFor<T>,
            sender: H160,
            recipient: <T::Lookup as StaticLookup>::Source,
            amount: U256,
        ) -> DispatchResult {
            let CallOriginOutput {
                network_id,
                message_id,
                timepoint,
                additional,
            } = T::CallOrigin::ensure_origin(origin)?;
            let (registered_contract, asset_id, sidechain_precision) =
                Addresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            ensure!(
                additional.source == registered_contract,
                Error::<T>::InvalidAppAddress
            );

            let thischain_amount = T::BalancePrecisionConverter::from_sidechain(
                &asset_id,
                sidechain_precision,
                amount,
            )
            .ok_or(Error::<T>::WrongAmount)?;
            ensure!(thischain_amount > Zero::zero(), Error::<T>::WrongAmount);

            let recipient = T::Lookup::lookup(recipient)?;
            T::BridgeAssetLocker::unlock_asset(
                network_id.into(),
                AssetKind::Sidechain,
                &recipient,
                &asset_id,
                &thischain_amount,
            )?;
            T::MessageStatusNotifier::inbound_request(
                GenericNetworkId::EVM(network_id),
                message_id,
                GenericAccount::EVM(sender),
                recipient.clone(),
                asset_id,
                thischain_amount.clone(),
                timepoint,
                MessageStatus::Done,
            );
            Self::deposit_event(Event::Minted(
                network_id,
                sender,
                recipient.clone(),
                thischain_amount,
            ));

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::register_network())]
        pub fn register_network(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            name: AssetNameOf<T>,
            symbol: AssetSymbolOf<T>,
            sidechain_precision: u8,
            contract: H160,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !Addresses::<T>::contains_key(network_id),
                Error::<T>::AppAlreadyExists
            );
            let asset_id = T::AssetRegistry::register_asset(network_id.into(), name, symbol)?;
            Self::register_network_inner(network_id, asset_id, contract, sidechain_precision)?;
            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::register_network())]
        pub fn register_network_with_existing_asset(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
            contract: H160,
            sidechain_precision: u8,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !Addresses::<T>::contains_key(network_id),
                Error::<T>::AppAlreadyExists
            );
            Self::register_network_inner(network_id, asset_id, contract, sidechain_precision)?;
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        fn register_network_inner(
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
            contract: H160,
            sidechain_precision: u8,
        ) -> DispatchResult {
            Addresses::<T>::insert(network_id, (contract, &asset_id, sidechain_precision));
            T::AssetRegistry::manage_asset(network_id.into(), asset_id)?;
            Ok(())
        }

        pub fn burn_inner(
            who: T::AccountId,
            network_id: EVMChainId,
            recipient: H160,
            amount: BalanceOf<T>,
        ) -> Result<H256, DispatchError> {
            let (target, asset_id, sidechain_precision) =
                Addresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;

            let sidechain_amount = T::BalancePrecisionConverter::to_sidechain(
                &asset_id,
                sidechain_precision,
                amount.clone(),
            )
            .ok_or(Error::<T>::WrongAmount)?;
            ensure!(sidechain_amount > 0.into(), Error::<T>::WrongAmount);

            T::BridgeAssetLocker::lock_asset(
                network_id.into(),
                AssetKind::Sidechain,
                &who,
                &asset_id,
                &amount,
            )?;

            let message = OutboundPayload::<T> {
                sender: who.clone(),
                recipient: recipient.clone(),
                amount: sidechain_amount,
            };

            let message_id = T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Signed(who.clone()),
                &message.encode().map_err(|_| Error::<T>::CallEncodeFailed)?,
                AdditionalEVMOutboundData {
                    target,
                    max_gas: TRANSFER_MAX_GAS.into(),
                },
            )?;
            T::MessageStatusNotifier::outbound_request(
                GenericNetworkId::EVM(network_id),
                message_id,
                who.clone(),
                GenericAccount::EVM(recipient),
                asset_id,
                amount.clone(),
                MessageStatus::InQueue,
            );
            Self::deposit_event(Event::Burned(network_id, who, recipient, amount.into()));

            Ok(message_id)
        }

        pub fn refund_inner(
            network_id: EVMChainId,
            recipient: T::AccountId,
            asset_id: AssetIdOf<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(amount > Zero::zero(), Error::<T>::WrongAmount);

            let (_, ether_asset_id, _sidechain_precision) =
                Addresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            ensure!(asset_id == ether_asset_id, Error::<T>::WrongRequest);

            T::BridgeAssetLocker::unlock_asset(
                network_id.into(),
                AssetKind::Sidechain,
                &recipient,
                &asset_id,
                &amount,
            )?;

            Self::deposit_event(Event::Refunded(network_id, recipient, amount));

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub networks: Vec<(EVMChainId, H160, AssetIdOf<T>, u8)>,
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
            for (network_id, contract, asset_id, precision) in &self.networks {
                Pallet::<T>::register_network_inner(
                    *network_id,
                    asset_id.clone(),
                    *contract,
                    *precision,
                )
                .unwrap();
            }
        }
    }

    impl<T: Config> BridgeApp<T::AccountId, H160, AssetIdOf<T>, BalanceOf<T>> for Pallet<T> {
        fn is_asset_supported(network_id: GenericNetworkId, asset_id: AssetIdOf<T>) -> bool {
            let GenericNetworkId::EVM(network_id) = network_id else {
                return false;
            };
            Addresses::<T>::get(network_id)
                .map(|(_contract, native_asset_id, _precision)| native_asset_id == asset_id)
                .unwrap_or(false)
        }

        fn transfer(
            network_id: GenericNetworkId,
            asset_id: AssetIdOf<T>,
            sender: T::AccountId,
            recipient: H160,
            amount: BalanceOf<T>,
        ) -> Result<H256, DispatchError> {
            if Self::is_asset_supported(network_id, asset_id) {
                let network_id = network_id.evm().ok_or(Error::<T>::AppIsNotRegistered)?;
                Pallet::<T>::burn_inner(sender, network_id, recipient, amount)
            } else {
                Err(Error::<T>::AppIsNotRegistered.into())
            }
        }

        fn refund(
            network_id: GenericNetworkId,
            _message_id: H256,
            recipient: T::AccountId,
            asset_id: AssetIdOf<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let network_id = network_id.evm().ok_or(Error::<T>::AppIsNotRegistered)?;
            Pallet::<T>::refund_inner(network_id, recipient, asset_id, amount)
        }

        fn list_supported_assets(network_id: GenericNetworkId) -> Vec<BridgeAssetInfo> {
            let GenericNetworkId::EVM(network_id) = network_id else {
                return vec![];
            };
            Addresses::<T>::get(network_id)
                .map(|(app_address, asset_id, precision)| {
                    vec![BridgeAssetInfo::EVM(EVMAssetInfo {
                        app_kind: EVMAppKind::EthApp,
                        asset_id: T::AssetIdConverter::convert(asset_id),
                        evm_address: app_address,
                        precision,
                    })]
                })
                .unwrap_or_default()
        }

        fn list_apps() -> Vec<BridgeAppInfo> {
            Addresses::<T>::iter()
                .map(|(network_id, (evm_address, _asset_id, _precision))| {
                    BridgeAppInfo::EVM(
                        network_id.into(),
                        EVMAppInfo {
                            app_kind: EVMAppKind::EthApp,
                            evm_address,
                        },
                    )
                })
                .collect()
        }
    }
}
