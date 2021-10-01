use codec::{Decode, Encode};
use ethabi::{self, Token};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_core::{RuntimeDebug, H160, H256};
use sp_io::offchain_index;
use sp_runtime::traits::{Hash, Zero};
use sp_std::prelude::*;

use snowbridge_core::types::AuxiliaryDigestItem;
use snowbridge_core::{ChannelId, MessageNonce};

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod test;

/// Wire-format for committed messages
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug)]
pub struct Message {
    /// Target application on the Ethereum side.
    target: H160,
    /// A nonce for replay protection and ordering.
    nonce: u64,
    /// Payload for target application.
    payload: Vec<u8>,
}

/// Weight functions needed for this pallet.
pub trait WeightInfo {
    fn on_initialize(num_messages: u32, avg_payload_bytes: u32) -> Weight;
    fn on_initialize_non_interval() -> Weight;
    fn on_initialize_no_messages() -> Weight;
}

impl WeightInfo for () {
    fn on_initialize(_: u32, _: u32) -> Weight {
        0
    }
    fn on_initialize_non_interval() -> Weight {
        0
    }
    fn on_initialize_no_messages() -> Weight {
        0
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Prefix for offchain storage keys.
        const INDEXING_PREFIX: &'static [u8];

        type Hashing: Hash<Output = H256>;

        // Max bytes in a message payload
        type MaxMessagePayloadSize: Get<usize>;

        /// Max number of messages that can be queued and committed in one go for a given channel.
        type MaxMessagesPerCommit: Get<usize>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    /// Interval between committing messages.
    #[pallet::storage]
    #[pallet::getter(fn interval)]
    type Interval<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

    /// Messages waiting to be committed.
    #[pallet::storage]
    type MessageQueue<T: Config> = StorageValue<_, Vec<Message>, ValueQuery>;

    #[pallet::storage]
    type Nonce<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::pallet]
    #[pallet::generate_store(trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

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

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
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
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    impl<T: Config> Pallet<T> {
        /// Submit message on the outbound channel
        pub fn submit(_who: &T::AccountId, target: H160, payload: &[u8]) -> DispatchResult {
            ensure!(
                MessageQueue::<T>::decode_len().unwrap_or(0) < T::MaxMessagesPerCommit::get(),
                Error::<T>::QueueSizeLimitReached,
            );
            ensure!(
                payload.len() <= T::MaxMessagePayloadSize::get(),
                Error::<T>::PayloadTooLarge,
            );

            Nonce::<T>::try_mutate(|nonce| -> DispatchResult {
                if let Some(v) = nonce.checked_add(1) {
                    *nonce = v;
                } else {
                    return Err(Error::<T>::Overflow.into());
                }

                MessageQueue::<T>::append(Message {
                    target,
                    nonce: *nonce,
                    payload: payload.to_vec(),
                });
                <Pallet<T>>::deposit_event(Event::MessageAccepted(*nonce));
                Ok(())
            })
        }

        fn commit() -> Weight {
            let messages: Vec<Message> = MessageQueue::<T>::take();
            if messages.is_empty() {
                return T::WeightInfo::on_initialize_no_messages();
            }

            let commitment_hash = Self::make_commitment_hash(&messages);
            let average_payload_size = Self::average_payload_size(&messages);

            let digest_item =
                AuxiliaryDigestItem::Commitment(ChannelId::Basic, commitment_hash.clone()).into();
            <frame_system::Pallet<T>>::deposit_log(digest_item);

            let key = Self::make_offchain_key(commitment_hash);
            offchain_index::set(&*key, &messages.encode());

            T::WeightInfo::on_initialize(messages.len() as u32, average_payload_size as u32)
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
