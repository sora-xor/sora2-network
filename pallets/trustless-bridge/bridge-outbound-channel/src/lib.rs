//! Channel for passing messages from substrate to ethereum.

#![cfg_attr(not(feature = "std"), no_std)]

use bridge_types::{H160, H256, U256};
use codec::{Decode, Encode};
use ethabi::{self, Token};
use frame_support::ensure;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_core::RuntimeDebug;
use sp_io::offchain_index;
use sp_runtime::traits::Hash;
use sp_std::prelude::*;
use sp_std::vec;
use traits::MultiCurrency;

use bridge_types::types::MessageNonce;
use bridge_types::EVMChainId;

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
    pub network_id: EVMChainId,
    /// Target application on the Ethereum side.
    pub target: H160,
    /// A nonce for replay protection and ordering.
    pub nonce: u64,
    /// Fee for accepting message on this channel.
    pub fee: U256,
    /// Maximum gas this message can use on the Ethereum.
    pub max_gas: U256,
    /// Payload for target application.
    pub payload: Vec<u8>,
}

/// Wire-format for commitment
#[derive(Encode, Decode, Clone, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct Commitment {
    /// Total maximum gas that can be used by all messages in the commit.
    /// Should be equal to sum of `max_gas`es of `messages`
    pub total_max_gas: U256,
    /// Messages passed through the channel in the current commit.
    pub messages: Vec<Message>,
}

type BalanceOf<T> = <<T as assets::Config>::Currency as MultiCurrency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::traits::AuxiliaryDigestHandler;
    use bridge_types::traits::MessageStatusNotifier;
    use bridge_types::traits::OutboundChannel;
    use bridge_types::types::AdditionalEVMOutboundData;
    use bridge_types::types::AuxiliaryDigestItem;
    use bridge_types::types::MessageId;
    use bridge_types::types::MessageStatus;
    use bridge_types::GenericNetworkId;
    use frame_support::log::debug;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + technical::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Prefix for offchain storage keys.
        const INDEXING_PREFIX: &'static [u8];

        type Hashing: Hash<Output = H256>;

        /// Max bytes in a message payload
        type MaxMessagePayloadSize: Get<u64>;

        /// Max number of messages that can be queued and committed in one go for a given channel.
        type MaxMessagesPerCommit: Get<u64>;

        /// Maximum gas limit for one message batch sent to Ethereum.
        type MaxTotalGasLimit: Get<u64>;

        type FeeCurrency: Get<Self::AssetId>;

        type FeeTechAccountId: Get<Self::TechAccountId>;

        type AuxiliaryDigestHandler: AuxiliaryDigestHandler;

        type MessageStatusNotifier: MessageStatusNotifier<
            Self::AssetId,
            Self::AccountId,
            BalanceOf<Self>,
        >;

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

    /// Messages waiting to be committed. To update the queue, use `append_message_queue` and `take_message_queue` methods
    /// (to keep correct value in [QueuesTotalGas]).
    #[pallet::storage]
    pub(crate) type MessageQueues<T: Config> =
        StorageMap<_, Identity, EVMChainId, Vec<Message>, ValueQuery>;

    /// Total gas for each queue. Updated by mutating the queues with methods `append_message_queue` and `take_message_queue`.
    #[pallet::storage]
    pub(crate) type QueuesTotalGas<T: Config> =
        StorageMap<_, Identity, EVMChainId, U256, ValueQuery>;

    /// Add message to queue and accumulate total maximum gas value    
    pub(crate) fn append_message_queue<T: Config>(network: EVMChainId, msg: Message) {
        QueuesTotalGas::<T>::mutate(network, |sum| *sum = sum.saturating_add(msg.max_gas));
        MessageQueues::<T>::append(network, msg);
    }

    /// Take the queue together with accumulated total maximum gas value.
    pub(crate) fn take_message_queue<T: Config>(network: EVMChainId) -> (Vec<Message>, U256) {
        (
            MessageQueues::<T>::take(network),
            QueuesTotalGas::<T>::take(network),
        )
    }

    #[pallet::storage]
    pub type ChannelNonces<T: Config> = StorageMap<_, Identity, EVMChainId, u64, ValueQuery>;

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
        MessageAccepted(EVMChainId, MessageNonce),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The message payload exceeds byte limit.
        PayloadTooLarge,
        /// No more messages can be queued for the channel during this commit cycle.
        QueueSizeLimitReached,
        /// Maximum gas for queued batch exceeds limit.
        MaxGasTooBig,
        /// Cannot pay the fee to submit a message.
        NoFunds,
        /// Cannot increment nonce
        Overflow,
        /// This channel already exists
        ChannelExists,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::set_fee())]
        pub fn set_fee(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Fee::<T>::set(amount);
            Ok(().into())
        }
    }
    impl<T: Config> Pallet<T> {
        pub fn make_message_id(nonce: u64) -> H256 {
            MessageId::outbound(nonce).using_encoded(|v| <T as Config>::Hashing::hash(v))
        }

        fn commit(network_id: EVMChainId) -> Weight {
            debug!("Commit messages");
            let (messages, total_max_gas) = take_message_queue::<T>(network_id);
            if messages.is_empty() {
                return <T as Config>::WeightInfo::on_initialize_no_messages();
            }

            for message in messages.iter() {
                T::MessageStatusNotifier::update_status(
                    GenericNetworkId::EVM(network_id),
                    Self::make_message_id(message.nonce),
                    MessageStatus::Committed,
                    None,
                );
            }

            let commitment = Commitment {
                total_max_gas,
                messages,
            };

            let average_payload_size = Self::average_payload_size(&commitment.messages);
            let messages_count = commitment.messages.len();
            let commitment_hash = Self::make_commitment_hash(&commitment);
            let digest_item = AuxiliaryDigestItem::Commitment(
                GenericNetworkId::EVM(network_id),
                commitment_hash.clone(),
            );
            T::AuxiliaryDigestHandler::add_item(digest_item);

            let key = Self::make_offchain_key(commitment_hash);
            offchain_index::set(&*key, &commitment.encode());

            <T as Config>::WeightInfo::on_initialize(
                messages_count as u32,
                average_payload_size as u32,
            )
        }
        fn make_commitment_hash(commitment: &Commitment) -> H256 {
            // Batch(uint256,(address,uint64,uint256,uint256,bytes)[])
            let messages: Vec<Token> = commitment
                .messages
                .iter()
                .map(|message| {
                    Token::Tuple(vec![
                        Token::Address(message.target),
                        Token::Uint(message.nonce.into()),
                        Token::Uint(message.fee.into()),
                        Token::Uint(message.max_gas.into()),
                        Token::Bytes(message.payload.clone()),
                    ])
                })
                .collect();
            let commitment: Vec<Token> = vec![
                Token::Uint(commitment.total_max_gas),
                Token::Array(messages),
            ];
            // Structs are represented as tuples in ABI
            // https://docs.soliditylang.org/en/v0.8.15/abi-spec.html#mapping-solidity-to-abi-types
            let input = ethabi::encode(&vec![Token::Tuple(commitment)]);
            <T as Config>::Hashing::hash(&input)
        }

        fn average_payload_size(messages: &[Message]) -> usize {
            let sum: usize = messages.iter().fold(0, |acc, x| acc + x.payload.len());
            // We overestimate message payload size rather than underestimate.
            // So add 1 here to account for integer division truncation.
            (sum / messages.len()).saturating_add(1)
        }

        pub fn make_offchain_key(hash: H256) -> Vec<u8> {
            (T::INDEXING_PREFIX, hash).encode()
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub fee: BalanceOf<T>,
        pub interval: T::BlockNumber,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                fee: Default::default(),
                interval: 10u32.into(),
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

    impl<T: Config> OutboundChannel<EVMChainId, T::AccountId, AdditionalEVMOutboundData> for Pallet<T> {
        /// Submit message on the outbound channel
        fn submit(
            network_id: EVMChainId,
            who: &RawOrigin<T::AccountId>,
            payload: &[u8],
            additional: AdditionalEVMOutboundData,
        ) -> Result<H256, DispatchError> {
            let AdditionalEVMOutboundData { target, max_gas } = additional;
            debug!("Send message from {:?} to {:?}", who, target);
            let current_total_gas = QueuesTotalGas::<T>::get(network_id);
            ensure!(
                current_total_gas.saturating_add(max_gas) <= T::MaxTotalGasLimit::get().into(),
                Error::<T>::MaxGasTooBig,
            );
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

                append_message_queue::<T>(
                    network_id,
                    Message {
                        network_id: network_id,
                        target,
                        nonce: *nonce,
                        fee: fee.into(),
                        max_gas,
                        payload: payload.to_vec(),
                    },
                );
                Self::deposit_event(Event::MessageAccepted(network_id, *nonce));
                Ok(Self::make_message_id(*nonce))
            })
        }
    }
}
