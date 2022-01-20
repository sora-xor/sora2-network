pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod test;

use codec::{Decode, Encode};
use ethabi::{self, Token};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::traits::{EnsureOrigin, Get};
use snowbridge_ethereum::EthNetworkId;
use sp_core::{RuntimeDebug, H160, H256};
use sp_io::offchain_index;
use sp_runtime::traits::{Hash, StaticLookup, Zero};

use sp_std::prelude::*;

use bridge_types::types::{AuxiliaryDigestItem, ChannelId, MessageNonce};

pub use weights::WeightInfo;

/// Wire-format for committed messages
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
pub struct Message {
    network_id: EthNetworkId,
    channel: H160,
    /// Target application on the Ethereum side.
    target: H160,
    /// A nonce for replay protection and ordering.
    nonce: u64,
    /// Payload for target application.
    payload: Vec<u8>,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Prefix for offchain storage keys.
        const INDEXING_PREFIX: &'static [u8];

        type Hashing: Hash<Output = H256>;

        /// Max bytes in a message payload
        #[pallet::constant]
        type MaxMessagePayloadSize: Get<u64>;

        /// Max number of messages per commitment
        #[pallet::constant]
        type MaxMessagesPerCommit: Get<u64>;

        type SetPrincipalOrigin: EnsureOrigin<Self::Origin>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T> {
        MessageAccepted(MessageNonce),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The message payload exceeds byte limit.
        PayloadTooLarge,
        /// No more messages can be queued for the channel during this commit cycle.
        QueueSizeLimitReached,
        /// Cannot increment nonce
        Overflow,
        /// Not authorized to send message
        NotAuthorized,
        /// Target channel not exists
        InvalidChannel,
        /// This channel already exists
        ChannelExists,
    }

    /// Interval between commitments
    #[pallet::storage]
    #[pallet::getter(fn interval)]
    pub(super) type Interval<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

    /// Messages waiting to be committed.
    #[pallet::storage]
    pub(super) type MessageQueue<T: Config> = StorageValue<_, Vec<Message>, ValueQuery>;

    /// Source channel on the ethereum side
    #[pallet::storage]
    pub type ChannelOwners<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Identity, H160, T::AccountId, OptionQuery>;

    #[pallet::storage]
    pub type ChannelNonces<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Identity, H160, u64, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub networks: Vec<(EthNetworkId, Vec<(H160, T::AccountId)>)>,
        pub interval: T::BlockNumber,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                interval: Default::default(),
                networks: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            Interval::<T>::set(self.interval.clone());
            for (network_id, channels) in &self.networks {
                for (channel, owner) in channels {
                    <ChannelOwners<T>>::insert(network_id, channel, owner);
                }
            }
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Generate a message commitment every [`Interval`] blocks.
        //
        // The commitment hash is included in an [`AuxiliaryDigestItem`] in the block header,
        // with the corresponding commitment is persisted offchain.
        fn on_initialize(now: T::BlockNumber) -> Weight {
            if (now % Self::interval()).is_zero() {
                Self::commit()
            } else {
                T::WeightInfo::on_initialize_non_interval()
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::set_principal())]
        pub fn set_principal(
            origin: OriginFor<T>,
            network_id: EthNetworkId,
            channel: H160,
            principal: <T::Lookup as StaticLookup>::Source,
        ) -> DispatchResult {
            T::SetPrincipalOrigin::ensure_origin(origin)?;
            let principal = T::Lookup::lookup(principal)?;
            <ChannelOwners<T>>::try_mutate(network_id, channel, |owner| {
                if let Some(owner) = owner {
                    *owner = principal;
                    Ok(())
                } else {
                    Err(Error::<T>::InvalidChannel)
                }
            })?;
            Ok(())
        }

        #[pallet::weight(T::WeightInfo::register_channel())]
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

    impl<T: Config> Pallet<T> {
        /// Submit message on the outbound channel
        pub fn submit(
            who: &T::AccountId,
            network_id: EthNetworkId,
            channel: H160,
            target: H160,
            payload: &[u8],
        ) -> DispatchResult {
            let owner =
                <ChannelOwners<T>>::get(network_id, channel).ok_or(Error::<T>::InvalidChannel)?;
            ensure!(*who == owner, Error::<T>::NotAuthorized,);
            ensure!(
                <MessageQueue<T>>::decode_len().unwrap_or(0)
                    < T::MaxMessagesPerCommit::get() as usize,
                Error::<T>::QueueSizeLimitReached,
            );
            ensure!(
                payload.len() <= T::MaxMessagePayloadSize::get() as usize,
                Error::<T>::PayloadTooLarge,
            );

            <ChannelNonces<T>>::try_mutate(network_id, channel, |nonce| -> DispatchResult {
                if let Some(v) = nonce.checked_add(1) {
                    *nonce = v;
                } else {
                    return Err(Error::<T>::Overflow.into());
                }

                <MessageQueue<T>>::try_mutate(|queue| -> DispatchResult {
                    queue.push(Message {
                        network_id,
                        channel,
                        target,
                        nonce: *nonce,
                        payload: payload.to_vec(),
                    });
                    Ok(())
                })?;
                Self::deposit_event(Event::MessageAccepted(*nonce));
                Ok(())
            })
        }

        fn commit() -> Weight {
            let messages: Vec<Message> = <MessageQueue<T>>::take();
            if messages.is_empty() {
                return T::WeightInfo::on_initialize_no_messages();
            }

            let average_payload_size = Self::average_payload_size(&messages);
            let messages_count = messages.len();
            let mut message_map = sp_std::collections::btree_map::BTreeMap::new();
            for message in messages {
                let key = (message.network_id.clone(), message.channel.clone());
                message_map.entry(key).or_insert(vec![]).push(message);
            }

            for ((network_id, channel), messages) in message_map {
                let commitment_hash = Self::make_commitment_hash(&messages);
                let digest_item = AuxiliaryDigestItem::Commitment(
                    ChannelId::Basic,
                    network_id,
                    channel,
                    commitment_hash.clone(),
                )
                .into();
                <frame_system::Pallet<T>>::deposit_log(digest_item);

                let key = Self::make_offchain_key(commitment_hash);
                offchain_index::set(&*key, &messages.encode());
            }

            T::WeightInfo::on_initialize(messages_count as u32, average_payload_size as u32)
        }

        fn make_commitment_hash(messages: &[Message]) -> H256 {
            let messages: Vec<Token> = messages
                .iter()
                .map(|message| {
                    Token::Tuple(vec![
                        Token::Address(message.target),
                        Token::Uint(message.nonce.into()),
                        Token::Bytes(message.payload.clone()),
                    ])
                })
                .collect();
            let input = ethabi::encode(&vec![Token::Array(messages)]);
            <T as Config>::Hashing::hash(&input)
        }

        fn average_payload_size(messages: &[Message]) -> usize {
            let sum: usize = messages.iter().fold(0, |acc, x| acc + x.payload.len());
            // We overestimate message payload size rather than underestimate.
            // So add 1 here to account for integer division truncation.
            (sum / messages.len()).saturating_add(1)
        }

        fn make_offchain_key(hash: H256) -> Vec<u8> {
            (T::INDEXING_PREFIX, ChannelId::Basic, hash).encode()
        }
    }
}
