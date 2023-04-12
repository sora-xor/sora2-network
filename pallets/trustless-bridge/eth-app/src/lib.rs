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

pub const TRANSFER_MAX_GAS: u64 = 100_000;

use common::prelude::constants::EXTRINSIC_FIXED_WEIGHT;
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::traits::EnsureOrigin;
use frame_support::weights::Weight;
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

/// Weight functions needed for this pallet.
pub trait WeightInfo {
    fn burn() -> Weight;
    fn mint() -> Weight;
    fn register_network() -> Weight;
    fn register_network_with_existing_asset() -> Weight;
}

impl WeightInfo for () {
    fn burn() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn mint() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn register_network() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
    fn register_network_with_existing_asset() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use assets::AssetIdOf;
    use bridge_types::traits::{BridgeApp, MessageStatusNotifier};
    use bridge_types::types::{
        AdditionalEVMInboundData, AdditionalEVMOutboundData, AppKind, BridgeAppInfo,
        BridgeAssetInfo, CallOriginOutput,
    };
    use bridge_types::{GenericAccount, GenericNetworkId, H256};
    use bridge_types::{H160, U256};
    use common::{AssetName, AssetSymbol, Balance};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_support::transactional;
    use frame_system::pallet_prelude::{OriginFor, *};
    use frame_system::RawOrigin;
    use traits::MultiCurrency;

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type BalanceOf<T> = <<T as assets::Config>::Currency as MultiCurrency<AccountIdOf<T>>>::Balance;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + assets::Config + technical::Config + permissions::Config
    {
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
            Self::AssetId,
            Self::AccountId,
            BalanceOf<Self>,
        >;

        type BridgeTechAccountId: Get<Self::TechAccountId>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn address_and_asset)]
    pub(super) type Addresses<T: Config> =
        StorageMap<_, Identity, EVMChainId, (H160, AssetIdOf<T>), OptionQuery>;

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
                timestamp,
                additional,
            } = T::CallOrigin::ensure_origin(origin)?;
            let (registered_contract, asset_id) =
                Addresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            ensure!(
                additional.source == registered_contract,
                Error::<T>::InvalidAppAddress
            );

            let amount: BalanceOf<T> = amount.as_u128().into();
            ensure!(amount > 0, Error::<T>::WrongAmount);
            let recipient = T::Lookup::lookup(recipient)?;
            T::Currency::deposit(asset_id, &recipient, amount)?;
            T::MessageStatusNotifier::inbound_request(
                GenericNetworkId::EVM(network_id),
                message_id,
                GenericAccount::EVM(sender),
                recipient.clone(),
                asset_id,
                amount,
                timestamp,
            );
            Self::deposit_event(Event::Minted(network_id, sender, recipient.clone(), amount));

            Ok(())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::register_network())]
        pub fn register_network(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            name: AssetName,
            symbol: AssetSymbol,
            decimals: u8,
            contract: H160,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !Addresses::<T>::contains_key(network_id),
                Error::<T>::AppAlreadyExists
            );
            let bridge_account = Self::bridge_account()?;
            let asset_id = assets::Pallet::<T>::register_from(
                &bridge_account,
                symbol,
                name,
                decimals,
                Balance::from(0u32),
                true,
                None,
                None,
            )?;
            Self::register_network_inner(network_id, asset_id, contract)?;
            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::register_network())]
        pub fn register_network_with_existing_asset(
            origin: OriginFor<T>,
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
            contract: H160,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                !Addresses::<T>::contains_key(network_id),
                Error::<T>::AppAlreadyExists
            );
            Self::register_network_inner(network_id, asset_id, contract)?;
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        fn bridge_account() -> Result<T::AccountId, DispatchError> {
            Ok(technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::BridgeTechAccountId::get(),
            )?)
        }

        fn register_network_inner(
            network_id: EVMChainId,
            asset_id: AssetIdOf<T>,
            contract: H160,
        ) -> DispatchResult {
            Addresses::<T>::insert(network_id, (contract, asset_id));
            let bridge_account = Self::bridge_account()?;
            let scope = permissions::Scope::Limited(common::hash(&asset_id));
            for permission_id in [permissions::BURN, permissions::MINT] {
                if permissions::Pallet::<T>::check_permission_with_scope(
                    bridge_account.clone(),
                    permission_id,
                    &scope,
                )
                .is_err()
                {
                    permissions::Pallet::<T>::assign_permission(
                        bridge_account.clone(),
                        &bridge_account,
                        permission_id,
                        scope,
                    )?;
                }
            }
            Ok(())
        }

        pub fn burn_inner(
            who: T::AccountId,
            network_id: EVMChainId,
            recipient: H160,
            amount: BalanceOf<T>,
        ) -> Result<H256, DispatchError> {
            ensure!(amount > 0, Error::<T>::WrongAmount);

            let (target, asset_id) =
                Addresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;

            T::Currency::withdraw(asset_id, &who, amount)?;

            let message = OutboundPayload::<T> {
                sender: who.clone(),
                recipient: recipient.clone(),
                amount: amount.into(),
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
                amount,
            );
            Self::deposit_event(Event::Burned(network_id, who, recipient, amount.into()));

            Ok(message_id)
        }

        pub fn refund_inner(
            network_id: EVMChainId,
            recipient: T::AccountId,
            asset_id: T::AssetId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            ensure!(amount > 0, Error::<T>::WrongAmount);

            let (_, ether_asset_id) =
                Addresses::<T>::get(network_id).ok_or(Error::<T>::AppIsNotRegistered)?;
            ensure!(asset_id == ether_asset_id, Error::<T>::WrongRequest);

            T::Currency::deposit(asset_id, &recipient, amount)?;

            Self::deposit_event(Event::Refunded(network_id, recipient, amount));

            Ok(())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub networks: Vec<(EVMChainId, H160, T::AssetId)>,
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
            for (network_id, contract, asset_id) in &self.networks {
                Pallet::<T>::register_network_inner(*network_id, *asset_id, *contract).unwrap();
            }
        }
    }

    impl<T: Config> BridgeApp<EVMChainId, T::AccountId, H160, T::AssetId, Balance> for Pallet<T> {
        fn is_asset_supported(network_id: EVMChainId, asset_id: T::AssetId) -> bool {
            Addresses::<T>::get(network_id)
                .map(|(_contract, native_asset_id)| native_asset_id == asset_id)
                .unwrap_or(false)
        }

        fn transfer(
            network_id: EVMChainId,
            asset_id: T::AssetId,
            sender: T::AccountId,
            recipient: H160,
            amount: Balance,
        ) -> Result<H256, DispatchError> {
            if Self::is_asset_supported(network_id, asset_id) {
                Pallet::<T>::burn_inner(sender, network_id, recipient, amount)
            } else {
                Err(Error::<T>::AppIsNotRegistered.into())
            }
        }

        fn refund(
            network_id: EVMChainId,
            _message_id: H256,
            recipient: T::AccountId,
            asset_id: AssetIdOf<T>,
            amount: Balance,
        ) -> DispatchResult {
            Pallet::<T>::refund_inner(network_id, recipient, asset_id, amount)
        }

        fn list_supported_assets(network_id: EVMChainId) -> Vec<BridgeAssetInfo<T::AssetId>> {
            Addresses::<T>::get(network_id)
                .map(|(_app_address, asset_id)| {
                    vec![BridgeAssetInfo {
                        app_kind: AppKind::EthApp,
                        asset_id,
                        evm_address: None,
                    }]
                })
                .unwrap_or_default()
        }

        fn list_apps(network_id: EVMChainId) -> Vec<BridgeAppInfo> {
            Addresses::<T>::get(network_id)
                .map(|(evm_address, _asset_id)| {
                    vec![BridgeAppInfo {
                        app_kind: AppKind::EthApp,
                        evm_address,
                    }]
                })
                .unwrap_or_default()
        }
    }
}
