//! Channel for passing messages from substrate to ethereum.

#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use bridge_types::{H256, U256};
use frame_support::ensure;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use sp_std::vec;
use traits::MultiCurrency;

use bridge_types::types::{BatchNonce, MessageNonce};
use bridge_types::EVMChainId;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod test;

// We use U256 bitfield to represent message statuses, so
// we can store only 256 messages in single commitment.
pub const MAX_QUEUE_SIZE: usize = 256;

type BalanceOf<T> = <<T as assets::Config>::Currency as MultiCurrency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::evm::*;
    use bridge_types::traits::AuxiliaryDigestHandler;
    use bridge_types::traits::MessageStatusNotifier;
    use bridge_types::traits::OutboundChannel;
    use bridge_types::types::AuxiliaryDigestItem;
    use bridge_types::types::MessageId;
    use bridge_types::types::MessageStatus;
    use bridge_types::GenericNetworkId;
    use bridge_types::GenericTimepoint;
    use frame_support::log::{debug, warn};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config + technical::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Max bytes in a message payload
        type MaxMessagePayloadSize: Get<u32>;

        /// Max number of messages that can be queued and committed in one go for a given channel.
        /// Must be < 256
        type MaxMessagesPerCommit: Get<u32>;

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

        #[pallet::constant]
        type ThisNetworkId: Get<GenericNetworkId>;

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
    pub(crate) type MessageQueues<T: Config> = StorageMap<
        _,
        Identity,
        EVMChainId,
        BoundedVec<Message<T::MaxMessagePayloadSize>, T::MaxMessagesPerCommit>,
        ValueQuery,
    >;

    /// Total gas for each queue. Updated by mutating the queues with methods `append_message_queue` and `take_message_queue`.
    #[pallet::storage]
    pub(crate) type QueuesTotalGas<T: Config> =
        StorageMap<_, Identity, EVMChainId, U256, ValueQuery>;

    /// Add message to queue and accumulate total maximum gas value    
    pub(crate) fn append_message_queue<T: Config>(
        network: EVMChainId,
        msg: Message<T::MaxMessagePayloadSize>,
    ) -> DispatchResult {
        QueuesTotalGas::<T>::mutate(network, |sum| *sum = sum.saturating_add(msg.max_gas));
        MessageQueues::<T>::try_append(network, msg)
            .map_err(|_| Error::<T>::QueueSizeLimitReached)?;
        Ok(())
    }

    /// Take the queue together with accumulated total maximum gas value.
    pub(crate) fn take_message_queue<T: Config>(
        network: EVMChainId,
    ) -> (
        BoundedVec<Message<T::MaxMessagePayloadSize>, T::MaxMessagesPerCommit>,
        U256,
    ) {
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

    #[pallet::storage]
    pub type LatestCommitment<T: Config> = StorageMap<
        _,
        Identity,
        EVMChainId,
        bridge_types::types::GenericCommitmentWithBlock<
            BlockNumberFor<T>,
            T::MaxMessagesPerCommit,
            T::MaxMessagePayloadSize,
        >,
        OptionQuery,
    >;

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
        MessageAccepted(EVMChainId, BatchNonce, MessageNonce),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The message payload exceeds byte limit.
        PayloadTooLarge,
        /// No more messages can be queued for the channel during this commit cycle.
        QueueSizeLimitReached,
        /// Maximum gas for queued batch exceeds limit.
        MaxGasTooBig,
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
        fn commit(network_id: EVMChainId) -> Weight {
            debug!("Commit messages");
            let (messages, total_max_gas) = take_message_queue::<T>(network_id);
            if messages.is_empty() {
                return <T as Config>::WeightInfo::on_initialize_no_messages();
            }

            <ChannelNonces<T>>::mutate(network_id, |nonce| {
                if let Some(v) = nonce.checked_add(1) {
                    *nonce = v;
                    let batch_nonce = *nonce;
                    for i in 0..messages.len() {
                        T::MessageStatusNotifier::update_status(
                            GenericNetworkId::EVM(network_id),
                            MessageId::batched(
                                T::ThisNetworkId::get(),
                                network_id.into(),
                                batch_nonce,
                                i as u64,
                            )
                            .hash(),
                            MessageStatus::Committed,
                            GenericTimepoint::Pending,
                        );
                    }

                    let average_payload_size = Self::average_payload_size(&messages);
                    let messages_count = messages.len();

                    let commitment =
                        bridge_types::GenericCommitment::EVM(bridge_types::evm::Commitment {
                            nonce: batch_nonce,
                            total_max_gas,
                            messages,
                        });

                    let digest_item = AuxiliaryDigestItem::Commitment(
                        GenericNetworkId::EVM(network_id),
                        commitment.hash(),
                    );
                    T::AuxiliaryDigestHandler::add_item(digest_item);

                    let commitment = bridge_types::types::GenericCommitmentWithBlock {
                        commitment,
                        block_number: <frame_system::Pallet<T>>::block_number(),
                    };

                    LatestCommitment::<T>::insert(network_id, commitment);

                    <T as Config>::WeightInfo::on_initialize(
                        messages_count as u32,
                        average_payload_size as u32,
                    )
                } else {
                    warn!("Batch nonce overflow");
                    return <T as Config>::WeightInfo::on_initialize_no_messages();
                }
            })
        }

        fn average_payload_size(messages: &[Message<T::MaxMessagePayloadSize>]) -> usize {
            let sum: usize = messages.iter().fold(0, |acc, x| acc + x.payload.len());
            // We overestimate message payload size rather than underestimate.
            // So add 1 here to account for integer division truncation.
            (sum / messages.len()).saturating_add(1)
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
            let message_queue_len = MessageQueues::<T>::decode_len(network_id).unwrap_or(0);
            ensure!(
                message_queue_len < T::MaxMessagesPerCommit::get() as usize
                    && message_queue_len < MAX_QUEUE_SIZE,
                Error::<T>::QueueSizeLimitReached,
            );
            ensure!(
                payload.len() <= T::MaxMessagePayloadSize::get() as usize,
                Error::<T>::PayloadTooLarge,
            );

            // TODO compute fee and charge
            // Attempt to charge a fee for message submission
            // gas used - estimate - depends on message payload + batch submission + target call
            // base fee - from eth light client as EthereumGasOracle
            // priority fee - some const
            let _fee = match who {
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

            // batch nonce
            let batch_nonce = <ChannelNonces<T>>::get(network_id) + 1;
            let message_id =
                MessageQueues::<T>::decode_len(network_id).unwrap_or(0) as MessageNonce;

            append_message_queue::<T>(
                network_id,
                Message {
                    target,
                    max_gas,
                    payload: payload
                        .to_vec()
                        .try_into()
                        .map_err(|_| Error::<T>::PayloadTooLarge)?,
                },
            )?;
            Self::deposit_event(Event::MessageAccepted(network_id, batch_nonce, message_id));
            Ok(MessageId::batched(
                T::ThisNetworkId::get(),
                network_id.into(),
                batch_nonce,
                message_id,
            )
            .hash())
        }

        fn submit_weight() -> Weight {
            Default::default()
        }
    }
}
