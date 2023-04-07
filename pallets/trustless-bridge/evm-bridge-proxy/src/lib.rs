#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod test;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use bridge_types::{
    traits::MessageStatusNotifier,
    types::{MessageDirection, MessageStatus},
    EVMChainId, GenericAccount, GenericNetworkId, H160, H256,
};
use codec::{Decode, Encode};
use common::{prelude::constants::EXTRINSIC_FIXED_WEIGHT, Balance};
use frame_support::dispatch::{DispatchResult, RuntimeDebug, Weight};
use frame_support::log;
use scale_info::TypeInfo;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::prelude::*;

pub trait WeightInfo {
    fn burn() -> Weight;
}

impl WeightInfo for () {
    fn burn() -> Weight {
        EXTRINSIC_FIXED_WEIGHT
    }
}

#[derive(Clone, RuntimeDebug, Encode, Decode, PartialEq, Eq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct BridgeRequest<AccountId, AssetId> {
    source: GenericAccount<AccountId>,
    dest: GenericAccount<AccountId>,
    asset_id: AssetId,
    amount: Balance,
    status: MessageStatus,
    start_timestamp: u64,
    end_timestamp: Option<u64>,
    direction: MessageDirection,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::{
        traits::BridgeApp,
        types::{BridgeAppInfo, BridgeAssetInfo},
    };
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use traits::MultiCurrency;

    type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
    type BalanceOf<T> = <<T as assets::Config>::Currency as MultiCurrency<AccountIdOf<T>>>::Balance;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + pallet_timestamp::Config {
        type RuntimeEvent: From<Event> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type EthApp: BridgeApp<EVMChainId, Self::AccountId, H160, Self::AssetId, Balance>;

        type ERC20App: BridgeApp<EVMChainId, Self::AccountId, H160, Self::AssetId, Balance>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    #[pallet::getter(fn transactions)]
    pub(super) type Transactions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        (GenericNetworkId, H256),
        BridgeRequest<T::AccountId, T::AssetId>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn sender)]
    pub(super) type Senders<T: Config> = StorageDoubleMap<
        _,
        Twox64Concat,
        GenericNetworkId,
        Blake2_128Concat,
        H256,
        T::AccountId,
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
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    /// Events for the ETH module.
    pub enum Event {
        RequestStatusUpdate(H256, MessageStatus),
        RefundFailed(H256),
    }

    #[pallet::error]
    pub enum Error<T> {
        PathIsNotAvailable,
        WrongAccountKind,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::burn())]
        pub fn burn(
            origin: OriginFor<T>,
            network_id: GenericNetworkId,
            asset_id: T::AssetId,
            recipient: GenericAccount<T::AccountId>,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            match network_id {
                GenericNetworkId::EVM(network_id) => {
                    let recipient = match recipient {
                        GenericAccount::EVM(address) => address,
                        _ => return Err(Error::<T>::WrongAccountKind.into()),
                    };
                    if T::EthApp::is_asset_supported(network_id, asset_id) {
                        T::EthApp::transfer(network_id, asset_id, sender, recipient, amount)?;
                    } else {
                        T::ERC20App::transfer(network_id, asset_id, sender, recipient, amount)?;
                    }
                }
                _ => return Err(Error::<T>::PathIsNotAvailable.into()),
            }
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn list_apps(network_id: EVMChainId) -> Vec<BridgeAppInfo> {
            let mut res = vec![];
            res.extend(T::EthApp::list_apps(network_id));
            res.extend(T::ERC20App::list_apps(network_id));
            res
        }

        pub fn list_supported_assets(network_id: EVMChainId) -> Vec<BridgeAssetInfo<T::AssetId>> {
            let mut res = vec![];
            res.extend(T::EthApp::list_supported_assets(network_id));
            res.extend(T::ERC20App::list_supported_assets(network_id));
            res
        }

        pub fn refund(
            network_id: GenericNetworkId,
            message_id: H256,
            beneficiary: GenericAccount<T::AccountId>,
            asset_id: T::AssetId,
            amount: Balance,
        ) -> DispatchResult {
            let beneficiary = match beneficiary {
                GenericAccount::Sora(account) => account,
                _ => return Err(Error::<T>::WrongAccountKind.into()),
            };
            match network_id {
                GenericNetworkId::EVM(chain_id) => {
                    if T::EthApp::is_asset_supported(chain_id, asset_id) {
                        T::EthApp::refund(chain_id, message_id, beneficiary, asset_id, amount)
                    } else {
                        T::ERC20App::refund(chain_id, message_id, beneficiary, asset_id, amount)
                    }
                }
                GenericNetworkId::Sub(_) => Err(Error::<T>::PathIsNotAvailable.into()),
            }
        }
    }
}

impl<T: Config> MessageStatusNotifier<T::AssetId, T::AccountId, Balance> for Pallet<T> {
    fn update_status(
        network_id: GenericNetworkId,
        message_id: H256,
        mut new_status: MessageStatus,
        new_end_timestamp: Option<u64>,
    ) {
        let sender = match Senders::<T>::get(network_id, message_id) {
            Some(sender) => sender,
            None => {
                log::warn!(
                    "Message status update called for unknown message: {:?} {:?}",
                    network_id,
                    message_id
                );
                return;
            }
        };
        Transactions::<T>::mutate(sender, (network_id, message_id), |req| {
            if let Some(req) = req {
                if new_status == MessageStatus::Failed
                    && req.direction == MessageDirection::Outbound
                {
                    match Pallet::<T>::refund(
                        network_id,
                        message_id,
                        req.source.clone(),
                        req.asset_id,
                        req.amount,
                    ) {
                        Ok(_) => {
                            new_status = MessageStatus::Refunded;
                        }
                        Err(_) => {
                            Self::deposit_event(Event::RefundFailed(message_id));
                        }
                    }
                }
                req.status = new_status;

                if let Some(timestamp) = new_end_timestamp {
                    req.end_timestamp = Some(timestamp);
                }

                Self::deposit_event(Event::RequestStatusUpdate(message_id, new_status));
            }
        })
    }

    fn inbound_request(
        network_id: GenericNetworkId,
        message_id: H256,
        source: GenericAccount<T::AccountId>,
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
            BridgeRequest {
                source,
                dest: GenericAccount::Sora(dest.clone()),
                asset_id,
                amount,
                status: MessageStatus::Done,
                start_timestamp,
                end_timestamp: Some(timestamp.unique_saturated_into()),
                direction: MessageDirection::Inbound,
            },
        );
    }

    fn outbound_request(
        network_id: GenericNetworkId,
        message_id: H256,
        source: T::AccountId,
        dest: GenericAccount<T::AccountId>,
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
            BridgeRequest {
                source: GenericAccount::Sora(source.clone()),
                dest,
                asset_id,
                amount,
                status: MessageStatus::InQueue,
                start_timestamp: timestamp.unique_saturated_into(),
                end_timestamp: None,
                direction: MessageDirection::Outbound,
            },
        );
    }
}
