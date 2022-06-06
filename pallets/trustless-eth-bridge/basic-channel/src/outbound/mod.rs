pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod test;

use bridge_types::EthNetworkId;
use codec::{Decode, Encode};
use ethabi::{self, Token};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::traits::Get;
use sp_core::{RuntimeDebug, H160, H256};
use sp_io::offchain_index;
use sp_runtime::traits::Hash;

use sp_std::prelude::*;

use bridge_types::types::{ChannelId, MessageNonce};

pub use weights::WeightInfo;

/// Wire-format for committed messages
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct Message {
    pub network_id: EthNetworkId,
    /// Target application on the Ethereum side.
    pub target: H160,
    /// A nonce for replay protection and ordering.
    pub nonce: u64,
    /// Payload for target application.
    pub payload: Vec<u8>,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    use super::*;

    use bridge_types::types::AuxiliaryDigestItem;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;

    pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

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

        /// Prefix for offchain storage keys.
        const INDEXING_PREFIX: &'static [u8];

        type Hashing: Hash<Output = H256>;

        /// Max bytes in a message payload
        #[pallet::constant]
        type MaxMessagePayloadSize: Get<u64>;

        /// Max number of messages per commitment
        #[pallet::constant]
        type MaxMessagesPerCommit: Get<u64>;

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
        /// Target network not exists
        InvalidNetwork,
        /// This channel already exists
        ChannelExists,
    }

    /// Interval between commitments
    #[pallet::storage]
    #[pallet::getter(fn interval)]
    pub(super) type Interval<T: Config> =
        StorageValue<_, T::BlockNumber, ValueQuery, DefaultInterval<T>>;

    #[pallet::type_value]
    pub(crate) fn DefaultInterval<T: Config>() -> T::BlockNumber {
        // TODO: Select interval
        10u32.into()
    }

    /// Messages waiting to be committed.
    #[pallet::storage]
    pub(super) type MessageQueue<T: Config> =
        StorageMap<_, Identity, EthNetworkId, Vec<Message>, ValueQuery>;

    #[pallet::storage]
    pub type ChannelNonces<T: Config> = StorageMap<_, Identity, EthNetworkId, u64, ValueQuery>;

    #[pallet::storage]
    pub type ChannelOperators<T: Config> =
        StorageDoubleMap<_, Identity, EthNetworkId, Identity, AccountIdOf<T>, bool, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub networks: Vec<(EthNetworkId, Vec<AccountIdOf<T>>)>,
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
            for (network_id, operators) in &self.networks {
                for operator in operators {
                    <ChannelOperators<T>>::insert(network_id, operator, true);
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
            let mut scheduled_ids = vec![];
            let interval = Self::interval();
            let batch_id = now % interval;
            for key in MessageQueue::<T>::iter_keys() {
                if T::BlockNumber::from(key) % interval == batch_id {
                    scheduled_ids.push(key);
                }
            }
            let mut weight = Default::default();
            for id in scheduled_ids {
                weight += Self::commit(id);
            }
            weight
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(T::WeightInfo::register_operator())]
        pub fn register_operator(
            origin: OriginFor<T>,
            network_id: EthNetworkId,
            operator: AccountIdOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            <ChannelOperators<T>>::insert(network_id, operator, true);
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Submit message on the outbound channel
        pub fn submit(
            who: &RawOrigin<T::AccountId>,
            network_id: EthNetworkId,
            target: H160,
            payload: &[u8],
        ) -> DispatchResult {
            match who {
                RawOrigin::Signed(who) => {
                    if !ChannelOperators::<T>::get(network_id, who) {
                        return Err(Error::<T>::NotAuthorized.into());
                    }
                }
                RawOrigin::None => {
                    return Err(Error::<T>::NotAuthorized.into());
                }
                RawOrigin::Root => {}
            }
            ensure!(
                <MessageQueue<T>>::decode_len(network_id).unwrap_or(0)
                    < T::MaxMessagesPerCommit::get() as usize,
                Error::<T>::QueueSizeLimitReached,
            );
            ensure!(
                payload.len() <= T::MaxMessagePayloadSize::get() as usize,
                Error::<T>::PayloadTooLarge,
            );

            <ChannelNonces<T>>::try_mutate(network_id, |nonce| -> DispatchResult {
                if let Some(v) = nonce.checked_add(1) {
                    *nonce = v;
                } else {
                    return Err(Error::<T>::Overflow.into());
                }

                MessageQueue::<T>::append(
                    network_id,
                    Message {
                        network_id,
                        target,
                        nonce: *nonce,
                        payload: payload.to_vec(),
                    },
                );
                Self::deposit_event(Event::MessageAccepted(*nonce));
                Ok(())
            })
        }

        fn commit(network_id: EthNetworkId) -> Weight {
            let messages: Vec<Message> = <MessageQueue<T>>::take(network_id);
            if messages.is_empty() {
                return T::WeightInfo::on_initialize_no_messages();
            }

            let average_payload_size = Self::average_payload_size(&messages);
            let messages_count = messages.len();

            let commitment_hash = Self::make_commitment_hash(&messages);
            let digest_item = AuxiliaryDigestItem::Commitment(
                network_id,
                ChannelId::Basic,
                commitment_hash.clone(),
            )
            .into();
            <frame_system::Pallet<T>>::deposit_log(digest_item);

            let key = Self::make_offchain_key(commitment_hash);
            offchain_index::set(&*key, &messages.encode());

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

        pub fn make_offchain_key(hash: H256) -> Vec<u8> {
            (T::INDEXING_PREFIX, ChannelId::Basic, hash).encode()
        }
    }
}
