// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

//! Channel for passing messages from substrate to ethereum.

use bridge_types::substrate::BridgeMessage;
use frame_support::ensure;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use bridge_types::H256;

use bridge_types::types::MessageNonce;
use bridge_types::SubNetworkId;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod test;

#[cfg(test)]
mod mock;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::traits::AuxiliaryDigestHandler;
    use bridge_types::traits::MessageStatusNotifier;
    use bridge_types::traits::OutboundChannel;
    use bridge_types::traits::TimepointProvider;
    use bridge_types::types::AuxiliaryDigestItem;
    use bridge_types::types::GenericCommitmentWithBlock;
    use bridge_types::types::MessageId;
    use bridge_types::types::MessageStatus;
    use bridge_types::GenericNetworkId;
    use bridge_types::GenericTimepoint;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::BuildGenesisConfig;
    use frame_support::traits::StorageVersion;
    use frame_support::Parameter;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;
    use log::debug;
    use sp_runtime::traits::Zero;
    use sp_runtime::DispatchError;

    #[pallet::config]
    pub trait Config:
        frame_system::Config<RuntimeEvent: From<Event<Self>>> + pallet_timestamp::Config
    {

        /// Max bytes in a message payload
        type MaxMessagePayloadSize: Get<u32>;

        /// Max number of messages that can be queued and committed in one go for a given channel.
        type MaxMessagesPerCommit: Get<u32>;

        type AssetId: Parameter;

        type Balance: Parameter;

        type MessageStatusNotifier: MessageStatusNotifier<
            Self::AssetId,
            Self::AccountId,
            Self::Balance,
        >;

        type AuxiliaryDigestHandler: AuxiliaryDigestHandler;

        type TimepointProvider: TimepointProvider;

        #[pallet::constant]
        type ThisNetworkId: Get<GenericNetworkId>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    /// Interval between committing messages.
    #[pallet::storage]
    #[pallet::getter(fn interval)]
    pub(crate) type Interval<T: Config> =
        StorageValue<_, BlockNumberFor<T>, ValueQuery, DefaultInterval<T>>;

    #[pallet::type_value]
    pub(crate) fn DefaultInterval<T: Config>() -> BlockNumberFor<T> {
        // TODO: Select interval
        10u32.into()
    }

    /// Messages waiting to be committed. To update the queue, use `append_message_queue` and `take_message_queue` methods
    /// (to keep correct value in [QueuesTotalGas]).
    #[pallet::storage]
    pub(crate) type MessageQueues<T: Config> = StorageMap<
        _,
        Identity,
        SubNetworkId,
        BoundedVec<BridgeMessage<T::MaxMessagePayloadSize>, T::MaxMessagesPerCommit>,
        ValueQuery,
    >;

    #[pallet::storage]
    pub type ChannelNonces<T: Config> = StorageMap<_, Identity, SubNetworkId, u64, ValueQuery>;

    #[pallet::storage]
    pub type LatestCommitment<T: Config> = StorageMap<
        _,
        Identity,
        SubNetworkId,
        GenericCommitmentWithBlock<
            BlockNumberFor<T>,
            T::MaxMessagesPerCommit,
            T::MaxMessagePayloadSize,
        >,
        OptionQuery,
    >;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Generate a message commitment every [`Interval`] blocks.
        //
        // The commitment hash is included in an [`AuxiliaryDigestItem`] in the block header,
        // with the corresponding commitment is persisted offchain.
        //
        // Use `on_finalize` instead of `on_idle` to ensure that the commitment is always sent,
        // because `on_idle` not guaranteed to be called.
        fn on_finalize(now: BlockNumberFor<T>) {
            let interval = Self::interval();
            if now % interval == Zero::zero() {
                for chain_id in MessageQueues::<T>::iter_keys() {
                    Self::commit(chain_id)
                }
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        MessageAccepted {
            network_id: SubNetworkId,
            batch_nonce: u64,
            message_nonce: MessageNonce,
        },
        IntervalUpdated {
            interval: BlockNumberFor<T>,
        },
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
        /// Interval cannot be zero.
        ZeroInterval,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::update_interval())]
        pub fn update_interval(
            origin: OriginFor<T>,
            new_interval: BlockNumberFor<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ensure!(new_interval > Zero::zero(), Error::<T>::ZeroInterval);
            Interval::<T>::put(new_interval);
            Self::deposit_event(Event::IntervalUpdated {
                interval: new_interval,
            });
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        pub(crate) fn commit(network_id: SubNetworkId) {
            debug!("Commit substrate messages");
            let messages = MessageQueues::<T>::take(network_id);

            let batch_nonce = ChannelNonces::<T>::mutate(network_id, |nonce| {
                *nonce += 1;
                *nonce
            });

            for idx in 0..messages.len() as u64 {
                T::MessageStatusNotifier::update_status(
                    GenericNetworkId::Sub(network_id),
                    MessageId::batched(
                        T::ThisNetworkId::get(),
                        network_id.into(),
                        batch_nonce,
                        idx,
                    )
                    .hash(),
                    MessageStatus::Committed,
                    GenericTimepoint::Pending,
                );
            }

            let commitment =
                bridge_types::GenericCommitment::Sub(bridge_types::substrate::Commitment {
                    messages,
                    nonce: batch_nonce,
                });

            let commitment_hash = commitment.hash();
            let digest_item =
                AuxiliaryDigestItem::Commitment(GenericNetworkId::Sub(network_id), commitment_hash);
            T::AuxiliaryDigestHandler::add_item(digest_item);

            let commitment = bridge_types::types::GenericCommitmentWithBlock {
                commitment,
                block_number: <frame_system::Pallet<T>>::block_number(),
            };
            LatestCommitment::<T>::insert(network_id, commitment);
        }
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub interval: BlockNumberFor<T>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                interval: 10u32.into(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            Interval::<T>::set(self.interval);
        }
    }

    impl<T: Config> OutboundChannel<SubNetworkId, T::AccountId, ()> for Pallet<T> {
        /// Submit message on the outbound channel
        fn submit(
            network_id: SubNetworkId,
            who: &RawOrigin<T::AccountId>,
            payload: &[u8],
            _: (),
        ) -> Result<H256, DispatchError> {
            debug!("Send message from {:?} to network {:?}", who, network_id);
            let messages_count = MessageQueues::<T>::decode_len(network_id).unwrap_or(0) as u64;
            ensure!(
                messages_count < T::MaxMessagesPerCommit::get() as u64,
                Error::<T>::QueueSizeLimitReached,
            );
            ensure!(
                payload.len() <= T::MaxMessagePayloadSize::get() as usize,
                Error::<T>::PayloadTooLarge,
            );

            let batch_nonce = ChannelNonces::<T>::get(network_id)
                .checked_add(1)
                .ok_or(Error::<T>::Overflow)?;

            MessageQueues::<T>::try_append(
                network_id,
                BridgeMessage {
                    payload: payload
                        .to_vec()
                        .try_into()
                        .map_err(|_| Error::<T>::PayloadTooLarge)?,
                    timepoint: T::TimepointProvider::get_timepoint(),
                },
            )
            .map_err(|_| Error::<T>::QueueSizeLimitReached)?;
            Self::deposit_event(Event::MessageAccepted {
                network_id,
                batch_nonce,
                message_nonce: messages_count,
            });
            Ok(MessageId::batched(
                T::ThisNetworkId::get(),
                network_id.into(),
                batch_nonce,
                messages_count,
            )
            .hash())
        }

        fn submit_weight() -> Weight {
            <T as Config>::WeightInfo::submit()
        }
    }
}
