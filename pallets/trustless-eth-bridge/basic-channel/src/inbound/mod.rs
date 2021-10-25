mod envelope;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

#[cfg(test)]
mod test;

use frame_system::ensure_signed;
use snowbridge_core::{ChannelId, Message, MessageDispatch, MessageId, Verifier};
use sp_core::H160;
use sp_std::convert::TryFrom;

use envelope::Envelope;
use snowbridge_ethereum::EthNetworkId;
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Verifier module for message verification.
        type Verifier: Verifier;

        /// Verifier module for message verification.
        type MessageDispatch: MessageDispatch<Self, MessageId>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    pub enum Event<T> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Message came from an invalid outbound channel on the Ethereum side.
        InvalidSourceChannel,
        /// Message has an invalid envelope.
        InvalidEnvelope,
        /// Message has an unexpected nonce.
        InvalidNonce,
        /// This channel already exists
        ChannelExists,
    }

    /// Source channel on the ethereum side
    #[pallet::storage]
    #[pallet::getter(fn source_channel)]
    pub type ChannelOwners<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Identity, H160, T::AccountId, OptionQuery>;

    #[pallet::storage]
    pub type ChannelNonces<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Identity, H160, u64, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub networks: Vec<(EthNetworkId, Vec<(H160, T::AccountId)>)>,
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
                for (channel, owner) in channels {
                    <ChannelOwners<T>>::insert(network_id, channel, owner);
                }
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
            let log = T::Verifier::verify(network_id, &message)?;

            // Decode log into an Envelope
            let envelope = Envelope::try_from(log).map_err(|_| Error::<T>::InvalidEnvelope)?;

            ensure!(
                <ChannelOwners<T>>::contains_key(network_id, envelope.channel) == false,
                Error::<T>::ChannelExists
            );

            // Verify message nonce
            <ChannelNonces<T>>::try_mutate(
                network_id,
                envelope.channel,
                |nonce| -> DispatchResult {
                    if envelope.nonce != *nonce + 1 {
                        Err(Error::<T>::InvalidNonce.into())
                    } else {
                        *nonce += 1;
                        Ok(())
                    }
                },
            )?;

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
            let owner = ensure_signed(origin)?;
            ensure!(
                <ChannelOwners<T>>::contains_key(network_id, channel) == false,
                Error::<T>::ChannelExists
            );

            <ChannelOwners<T>>::insert(network_id, channel, owner);
            Ok(())
        }
    }
}
