mod envelope;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

#[cfg(test)]
mod test;

use bridge_types::traits::{AppRegistry, MessageDispatch, Verifier};
use bridge_types::types::{ChannelId, Message, MessageId};
use bridge_types::EthNetworkId;
use frame_system::ensure_signed;
use sp_core::H160;
use sp_std::convert::TryFrom;

use envelope::Envelope;
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    use bridge_types::traits::OutboundRouter;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Verifier module for message verification.
        type Verifier: Verifier;

        /// Verifier module for message verification.
        type MessageDispatch: MessageDispatch<Self, MessageId>;

        type OutboundRouter: OutboundRouter<Self::AccountId>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    pub enum Event<T> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Message came from an invalid etherem network
        InvalidNetwork,
        /// Message came from an invalid outbound channel on the Ethereum side.
        InvalidSourceChannel,
        /// Message has an invalid envelope.
        InvalidEnvelope,
        /// Message has an unexpected nonce.
        InvalidNonce,
        /// This channel already exists
        ChannelExists,
        /// Call encoding failed.
        CallEncodeFailed,
    }

    #[pallet::storage]
    pub type ChannelNonces<T: Config> = StorageMap<_, Identity, EthNetworkId, u64, ValueQuery>;

    #[pallet::storage]
    pub type ChannelAddresses<T: Config> = StorageMap<_, Identity, EthNetworkId, H160, OptionQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub networks: Vec<(EthNetworkId, H160)>,
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
            for (network_id, channel) in &self.networks {
                <ChannelAddresses<T>>::insert(network_id, channel);
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::submit())]
        pub fn submit(
            origin: OriginFor<T>,
            network_id: EthNetworkId,
            message: Message,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            // submit message to verifier for verification
            let log = T::Verifier::verify(network_id, &message).map_err(|err| {
                frame_support::log::warn!("Failed to verify message: {:?}", err);
                err
            })?;

            // Decode log into an Envelope
            let envelope = Envelope::try_from(log).map_err(|_| Error::<T>::InvalidEnvelope)?;

            ensure!(
                <ChannelAddresses<T>>::get(network_id).ok_or(Error::<T>::InvalidNetwork)?
                    == envelope.channel,
                Error::<T>::InvalidSourceChannel
            );

            // Verify message nonce
            <ChannelNonces<T>>::try_mutate(network_id, |nonce| -> DispatchResult {
                if envelope.nonce != *nonce + 1 {
                    Err(Error::<T>::InvalidNonce.into())
                } else {
                    *nonce += 1;
                    Ok(())
                }
            })?;

            let message_id = MessageId::new(ChannelId::Basic, envelope.nonce);
            T::MessageDispatch::dispatch(
                network_id,
                envelope.source,
                message_id,
                &envelope.payload,
            );

            Ok(())
        }

        #[pallet::weight(<T as Config>::WeightInfo::register_channel())]
        pub fn register_channel(
            origin: OriginFor<T>,
            network_id: EthNetworkId,
            channel: H160,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(
                <ChannelAddresses<T>>::contains_key(network_id) == false,
                Error::<T>::ChannelExists
            );

            <ChannelAddresses<T>>::insert(network_id, channel);
            Ok(())
        }
    }

    impl<T: Config> AppRegistry for Pallet<T> {
        fn register_app(network_id: EthNetworkId, app: H160) -> DispatchResult {
            let target =
                ChannelAddresses::<T>::get(network_id).ok_or(Error::<T>::InvalidNetwork)?;

            let message = bridge_types::channel_abi::RegisterOperatorPayload { operator: app };

            T::OutboundRouter::submit(
                network_id,
                ChannelId::Basic,
                &RawOrigin::Root,
                target,
                message
                    .encode()
                    .map_err(|_| Error::<T>::CallEncodeFailed)?
                    .as_ref(),
            )?;
            Ok(())
        }

        fn deregister_app(network_id: EthNetworkId, app: H160) -> DispatchResult {
            let target =
                ChannelAddresses::<T>::get(network_id).ok_or(Error::<T>::InvalidNetwork)?;

            let message = bridge_types::channel_abi::DeregisterOperatorPayload { operator: app };

            T::OutboundRouter::submit(
                network_id,
                ChannelId::Basic,
                &RawOrigin::Root,
                target,
                message
                    .encode()
                    .map_err(|_| Error::<T>::CallEncodeFailed)?
                    .as_ref(),
            )?;
            Ok(())
        }
    }
}
