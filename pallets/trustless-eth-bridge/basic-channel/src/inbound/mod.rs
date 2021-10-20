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
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::{AtLeast32BitUnsigned, MaybeDisplay, MaybeSerializeDeserialize};

    use core::fmt::Debug;

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

        /// Network id
        type NetworkId: Parameter
            + Member
            + MaybeSerializeDeserialize
            + Debug
            + Default
            + MaybeDisplay
            + AtLeast32BitUnsigned
            + Copy;
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
    }

    /// Source channel on the ethereum side
    #[pallet::storage]
    #[pallet::getter(fn source_channel)]
    pub type ChannelOwners<T: Config> =
        StorageDoubleMap<_, Identity, T::NetworkId, Identity, H160, T::AccountId, OptionQuery>;

    #[pallet::storage]
    pub type ChannelNonces<T: Config> =
        StorageDoubleMap<_, Identity, T::NetworkId, Identity, H160, u64, OptionQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub networks: Vec<(T::NetworkId, Vec<(H160, T::AccountId)>)>,
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
                    <ChannelNonces<T>>::insert(network_id, channel, 0);
                }
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::submit())]
        pub fn submit(origin: OriginFor<T>, message: Message) -> DispatchResult {
            ensure_signed(origin)?;
            // submit message to verifier for verification
            let log = T::Verifier::verify(&message)?;

            // Decode log into an Envelope
            let envelope = Envelope::try_from(log).map_err(|_| Error::<T>::InvalidEnvelope)?;

            let network_id: T::NetworkId = message.network_id.into();

            // Verify message nonce
            <ChannelNonces<T>>::try_mutate(
                network_id,
                envelope.channel,
                |nonce| -> DispatchResult {
                    match nonce {
                        Some(nonce) => {
                            if envelope.nonce != *nonce + 1 {
                                Err(Error::<T>::InvalidNonce.into())
                            } else {
                                *nonce += 1;
                                Ok(())
                            }
                        }
                        // Verify that the message was submitted to us from a known
                        // outbound channel on the ethereum side
                        _ => Err(Error::<T>::InvalidSourceChannel.into()),
                    }
                },
            )?;

            let message_id = MessageId::new(ChannelId::Basic, envelope.nonce);
            T::MessageDispatch::dispatch(
                message.network_id,
                envelope.source,
                message_id,
                &envelope.payload,
            );

            Ok(())
        }
        #[pallet::weight(T::WeightInfo::submit())]
        pub fn register_channel(
            origin: OriginFor<T>,
            network_id: T::NetworkId,
            channel: H160,
        ) -> DispatchResult {
            let owner = ensure_signed(origin)?;
            <ChannelOwners<T>>::insert(network_id, channel, owner);
            <ChannelNonces<T>>::insert(network_id, channel, 0);
            Ok(())
        }
    }
}
