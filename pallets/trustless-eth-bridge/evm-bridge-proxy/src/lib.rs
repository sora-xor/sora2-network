#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod test;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use bridge_types::{traits::MessageStatusNotifier, types::MessageStatus, EthNetworkId, H160, H256};
use codec::{Decode, Encode};
use common::{prelude::constants::EXTRINSIC_FIXED_WEIGHT, Balance};
use frame_support::{dispatch::Weight, RuntimeDebug};
use scale_info::TypeInfo;
use sp_std::prelude::*;

pub trait WeightInfo {
    fn burn() -> Weight;
}

impl WeightInfo for () {
    fn burn() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}

pub use pallet::*;

#[derive(Clone, RuntimeDebug, Encode, Decode, PartialEq, Eq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub enum BridgeRequest<T: frame_system::Config + assets::Config + pallet_timestamp::Config> {
    IncomingTransfer {
        source: H160,
        dest: T::AccountId,
        asset_id: T::AssetId,
        amount: Balance,
        status: MessageStatus,
        start_timestamp: u64,
        end_timestamp: T::Moment,
    },
    OutgoingTransfer {
        source: T::AccountId,
        dest: H160,
        asset_id: T::AssetId,
        amount: Balance,
        status: MessageStatus,
        start_timestamp: T::Moment,
        end_timestamp: Option<u64>,
    },
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::{
        traits::EvmBridgeApp,
        types::{BridgeAppInfo, BridgeAssetInfo},
    };
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use traits::MultiCurrency;

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type BalanceOf<T> = <<T as assets::Config>::Currency as MultiCurrency<AccountIdOf<T>>>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + pallet_timestamp::Config {
        type Event: From<Event> + IsType<<Self as frame_system::Config>::Event>;

        type EthApp: EvmBridgeApp<Self::AccountId, Self::AssetId, Balance>;

        type ERC20App: EvmBridgeApp<Self::AccountId, Self::AssetId, Balance>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn transactions)]
    pub(super) type Transactions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        AccountIdOf<T>,
        Blake2_128Concat,
        (EthNetworkId, H256),
        BridgeRequest<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn sender)]
    pub(super) type Senders<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        EthNetworkId,
        Blake2_128Concat,
        H256,
        AccountIdOf<T>,
        OptionQuery,
    >;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            Transactions::kill_prefix();
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    /// Events for the ETH module.
    pub enum Event {
        RequestStatusUpdate(H256, MessageStatus),
    }

    #[pallet::error]
    pub enum Error<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn burn(
            origin: OriginFor<T>,
            network_id: EthNetworkId,
            asset_id: T::AssetId,
            recipient: H160,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            if T::EthApp::is_asset_supported(network_id, asset_id) {
                T::EthApp::transfer(network_id, asset_id, sender, recipient, amount)?;
            } else {
                T::ERC20App::transfer(network_id, asset_id, sender, recipient, amount)?;
            }
            Ok(())
        }
    }
    impl<T: Config> Pallet<T> {
        pub fn list_apps(network_id: EthNetworkId) -> Vec<BridgeAppInfo> {
            let mut res = vec![];
            res.extend(T::EthApp::list_apps(network_id));
            res.extend(T::ERC20App::list_apps(network_id));
            res
        }

        pub fn list_supported_assets(network_id: EthNetworkId) -> Vec<BridgeAssetInfo<T::AssetId>> {
            let mut res = vec![];
            res.extend(T::EthApp::list_supported_assets(network_id));
            res.extend(T::ERC20App::list_supported_assets(network_id));
            res
        }
    }
}

impl<T: Config> MessageStatusNotifier<T::AssetId, T::AccountId> for Pallet<T> {
    fn update_status(
        network_id: EthNetworkId,
        id: H256,
        new_status: MessageStatus,
        new_end_timestamp: Option<u64>,
    ) {
        let sender = match Senders::<T>::get(network_id, id) {
            Some(sender) => sender,
            None => return,
        };
        Transactions::<T>::mutate(sender, (network_id, id), |req| {
            if let Some(req) = req {
                Self::deposit_event(Event::RequestStatusUpdate(id, new_status));
                match req {
                    BridgeRequest::IncomingTransfer { status, .. }
                    | BridgeRequest::OutgoingTransfer { status, .. } => *status = new_status,
                }
                match req {
                    BridgeRequest::OutgoingTransfer { end_timestamp, .. } => {
                        if let Some(timestamp) = new_end_timestamp {
                            *end_timestamp = Some(timestamp);
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    fn inbound_request(
        network_id: EthNetworkId,
        message_id: H256,
        source: H160,
        dest: T::AccountId,
        asset_id: T::AssetId,
        amount: Balance,
        start_timestamp: u64,
    ) {
        Self::deposit_event(Event::RequestStatusUpdate(message_id, MessageStatus::Done));
        Senders::<T>::insert(&network_id, &message_id, &dest);
        let timestamp = pallet_timestamp::Pallet::<T>::now();
        Transactions::<T>::insert(
            &dest,
            (&network_id, &message_id),
            BridgeRequest::IncomingTransfer {
                source,
                dest: dest.clone(),
                asset_id,
                amount,
                status: MessageStatus::Done,
                start_timestamp,
                end_timestamp: timestamp,
            },
        );
    }

    fn outbound_request(
        network_id: EthNetworkId,
        message_id: H256,
        source: T::AccountId,
        dest: H160,
        asset_id: T::AssetId,
        amount: Balance,
    ) {
        Self::deposit_event(Event::RequestStatusUpdate(
            message_id,
            MessageStatus::InQueue,
        ));
        Senders::<T>::insert(&network_id, &message_id, &source);
        let timestamp = pallet_timestamp::Pallet::<T>::now();
        Transactions::<T>::insert(
            &source,
            (&network_id, &message_id),
            BridgeRequest::OutgoingTransfer {
                source: source.clone(),
                dest,
                asset_id,
                amount,
                status: MessageStatus::InQueue,
                start_timestamp: timestamp,
                end_timestamp: None,
            },
        );
    }
}
