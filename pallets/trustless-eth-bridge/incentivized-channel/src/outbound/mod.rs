use codec::{Decode, Encode};
use ethabi::{self, Token};
use frame_support::ensure;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_core::{RuntimeDebug, H160, H256, U256};
use sp_io::offchain_index;
use sp_runtime::traits::Hash;
use sp_std::prelude::*;
use traits::MultiCurrency;

use bridge_types::types::{ChannelId, MessageNonce};
use bridge_types::EthNetworkId;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod test;

/// Wire-format for committed messages
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct Message {
    pub network_id: EthNetworkId,
    /// Target application on the Ethereum side.
    pub target: H160,
    /// A nonce for replay protection and ordering.
    pub nonce: u64,
    /// Fee for accepting message on this channel.
    pub fee: U256,
    /// Payload for target application.
    pub payload: Vec<u8>,
}

type BalanceOf<T> = <<T as assets::Config>::Currency as MultiCurrency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::traits::MessageStatusNotifier;
    use bridge_types::types::AuxiliaryDigestItem;
    use bridge_types::types::MessageId;
    use bridge_types::types::MessageStatus;
    use frame_support::log::debug;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + technical::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Prefix for offchain storage keys.
        const INDEXING_PREFIX: &'static [u8];

        type Hashing: Hash<Output = H256>;

        // Max bytes in a message payload
        type MaxMessagePayloadSize: Get<u64>;

        /// Max number of messages that can be queued and committed in one go for a given channel.
        type MaxMessagesPerCommit: Get<u64>;

        type FeeCurrency: Get<Self::AssetId>;

        type FeeTechAccountId: Get<Self::TechAccountId>;

        type MessageStatusNotifier: MessageStatusNotifier<Self::AssetId, Self::AccountId>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    /// Interval between committing messages.
    #[pallet::storage]
    #[pallet::getter(fn interval)]
    pub(crate) type Interval<T: Config> =
        StorageValue<_, T::BlockNumber, ValueQuery, DefaultInterval<T>>;

    #[pallet::type_value]
    pub(crate) fn DefaultInterval<T: Config>() -> T::BlockNumber {
        // TODO: Select interval
        10u32.into()
    }

    /// Messages waiting to be committed.
    #[pallet::storage]
    pub(crate) type MessageQueues<T: Config> =
        StorageMap<_, Identity, EthNetworkId, Vec<Message>, ValueQuery>;

    #[pallet::storage]
    pub type ChannelNonces<T: Config> = StorageMap<_, Identity, EthNetworkId, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn fee)]
    pub type Fee<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery, DefaultFee<T>>;

    #[pallet::type_value]
    pub fn DefaultFee<T: Config>() -> BalanceOf<T> {
        // TODO: Select fee value
        10000
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

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
            for chain_id in MessageQueues::<T>::iter_keys() {
                let chain_id_rem: T::BlockNumber = chain_id
                    .checked_rem(u32::MAX.into())
                    .unwrap_or_default()
                    .as_u32()
                    .into();
                if chain_id_rem % interval == batch_id {
                    scheduled_ids.push(chain_id);
                }
            }
            let mut weight = Default::default();
            for id in scheduled_ids {
                weight += Self::commit(id);
            }
            weight
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        MessageAccepted(EthNetworkId, MessageNonce),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The message payload exceeds byte limit.
        PayloadTooLarge,
        /// No more messages can be queued for the channel during this commit cycle.
        QueueSizeLimitReached,
        /// Cannot pay the fee to submit a message.
        NoFunds,
        /// Cannot increment nonce
        Overflow,
        /// This channel already exists
        ChannelExists,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(<T as Config>::WeightInfo::set_fee())]
        pub fn set_fee(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Fee::<T>::set(amount);
            Ok(().into())
        }
    }
    impl<T: Config> Pallet<T> {
        /// Submit message on the outbound channel
        pub fn submit(
            who: &RawOrigin<T::AccountId>,
            network_id: EthNetworkId,
            target: H160,
            payload: &[u8],
        ) -> Result<H256, DispatchError> {
            debug!("Send message from {:?} to {:?}", who, target);
            ensure!(
                MessageQueues::<T>::decode_len(network_id).unwrap_or(0)
                    < T::MaxMessagesPerCommit::get() as usize,
                Error::<T>::QueueSizeLimitReached,
            );
            ensure!(
                payload.len() <= T::MaxMessagePayloadSize::get() as usize,
                Error::<T>::PayloadTooLarge,
            );

            <ChannelNonces<T>>::try_mutate(network_id, |nonce| -> Result<H256, DispatchError> {
                if let Some(v) = nonce.checked_add(1) {
                    *nonce = v;
                } else {
                    return Err(Error::<T>::Overflow.into());
                }

                // Attempt to charge a fee for message submission
                let fee = match who {
                    RawOrigin::Signed(who) => {
                        let fee = Self::fee();
                        technical::Pallet::<T>::transfer_in(
                            &T::FeeCurrency::get(),
                            who,
                            &T::FeeTechAccountId::get(),
                            fee,
                        )?;
                        fee
                    }
                    _ => 0u128.into(),
                };

                MessageQueues::<T>::append(
                    network_id,
                    Message {
                        network_id: network_id,
                        target,
                        nonce: *nonce,
                        fee: fee.into(),
                        payload: payload.to_vec(),
                    },
                );
                Self::deposit_event(Event::MessageAccepted(network_id, *nonce));
                Ok(Self::make_message_id(*nonce))
            })
        }

        fn make_message_id(nonce: u64) -> H256 {
            MessageId::outbound(ChannelId::Incentivized, nonce)
                .using_encoded(|v| <T as Config>::Hashing::hash(v))
        }

        fn commit(network_id: EthNetworkId) -> Weight {
            debug!("Commit messages");
            let messages: Vec<Message> = MessageQueues::<T>::take(network_id);
            if messages.is_empty() {
                return <T as Config>::WeightInfo::on_initialize_no_messages();
            }

            let average_payload_size = Self::average_payload_size(&messages);
            let messages_count = messages.len();
            let commitment_hash = Self::make_commitment_hash(&messages);
            let digest_item = AuxiliaryDigestItem::Commitment(
                network_id,
                ChannelId::Incentivized,
                commitment_hash.clone(),
            )
            .into();
            <frame_system::Pallet<T>>::deposit_log(digest_item);

            let key = Self::make_offchain_key(commitment_hash);
            offchain_index::set(&*key, &messages.encode());

            for message in messages.iter() {
                T::MessageStatusNotifier::update_status(
                    network_id,
                    Self::make_message_id(message.nonce),
                    MessageStatus::Committed,
                );
            }

            <T as Config>::WeightInfo::on_initialize(
                messages_count as u32,
                average_payload_size as u32,
            )
        }

        fn make_commitment_hash(messages: &[Message]) -> H256 {
            let messages: Vec<Token> = messages
                .iter()
                .map(|message| {
                    Token::Tuple(vec![
                        Token::Address(message.target),
                        Token::Uint(message.nonce.into()),
                        Token::Uint(message.fee.into()),
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
            (T::INDEXING_PREFIX, ChannelId::Incentivized, hash).encode()
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub fee: BalanceOf<T>,
        pub interval: T::BlockNumber,
        pub networks: Vec<(EthNetworkId, H160)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                fee: Default::default(),
                interval: 10u32.into(),
                networks: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            Fee::<T>::set(self.fee.clone());
            Interval::<T>::set(self.interval.clone());
        }
    }
}
