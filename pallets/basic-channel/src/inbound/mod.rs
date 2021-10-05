use frame_support::dispatch::DispatchResult;
use frame_support::weights::Weight;
use frame_system::ensure_signed;
use snowbridge_core::{ChannelId, Message, MessageDispatch, MessageId, Verifier};
use sp_core::H160;
use sp_std::convert::TryFrom;

use envelope::Envelope;

mod benchmarking;

#[cfg(test)]
mod test;

mod envelope;

/// Weight functions needed for this pallet.
pub trait WeightInfo {
    fn submit() -> Weight;
    fn set_reward_fraction() -> Weight;
}

impl WeightInfo for () {
    fn submit() -> Weight {
        0
    }
    fn set_reward_fraction() -> Weight {
        0
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::log::{debug, warn};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

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

    #[pallet::storage]
    #[pallet::getter(fn source_channel)]
    pub(super) type SourceChannel<T: Config> = StorageValue<_, H160, ValueQuery>;

    #[pallet::storage]
    pub(super) type Nonce<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
    //#[pallet::generate_deposit(pub(super) fn deposit_event)]
    /// This module has no events
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Message came from an invalid outbound channel on the Ethereum side.
        InvalidSourceChannel,
        /// Message has an invalid envelope.
        InvalidEnvelope,
        /// Message has an unexpected nonce.
        InvalidNonce,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::submit())]
        pub fn submit(origin: OriginFor<T>, message: Message) -> DispatchResultWithPostInfo {
            let relayer = ensure_signed(origin)?;
            debug!("Recieved message from {:?}", relayer);
            // submit message to verifier for verification
            let log = T::Verifier::verify(&message)?;

            // Decode log into an Envelope
            let envelope: Envelope = Envelope::try_from(log).map_err(|_| {
                warn!("Invalid envelope");
                Error::<T>::InvalidEnvelope
            })?;

            // Verify that the message was submitted to us from a known
            // outbound channel on the ethereum side
            if envelope.channel != SourceChannel::<T>::get() {
                return Err(Error::<T>::InvalidSourceChannel.into());
            }

            // Verify message nonce
            Nonce::<T>::try_mutate(|nonce| -> DispatchResult {
                if envelope.nonce != *nonce + 1 {
                    Err(Error::<T>::InvalidNonce.into())
                } else {
                    *nonce += 1;
                    Ok(())
                }
            })?;

            let message_id = MessageId::new(ChannelId::Basic, envelope.nonce);
            T::MessageDispatch::dispatch(envelope.source, message_id, &envelope.payload);

            Ok(().into())
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub source_channel: H160,
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {
                source_channel: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            SourceChannel::<T>::set(self.source_channel);
        }
    }
}
