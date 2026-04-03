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

//! Channel for passing messages from ethereum to substrate.

use bridge_types::traits::{MessageDispatch, Verifier};
use bridge_types::types::MessageId;
use bridge_types::SubNetworkId;
use frame_support::dispatch::DispatchResult;
use frame_support::traits::Get;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod test;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::GenericNetworkId;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_support::weights::Weight;
    use frame_system::pallet_prelude::*;
    use log::warn;
    use sp_std::prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config<RuntimeEvent: From<Event<Self>>> + pallet_timestamp::Config
    {

        /// Verifier module for message verification.
        type Verifier: Verifier;

        /// Verifier module for message verification.
        type MessageDispatch: MessageDispatch<Self, SubNetworkId, MessageId, ()>;

        /// A configuration for base priority of unsigned transactions.
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;

        /// A configuration for longevity of unsigned transactions.
        #[pallet::constant]
        type UnsignedLongevity: Get<u64>;

        #[pallet::constant]
        type ThisNetworkId: Get<GenericNetworkId>;

        /// Max bytes in a message payload
        type MaxMessagePayloadSize: Get<u32>;

        /// Max number of messages that can be queued and committed in one go for a given channel.
        type MaxMessagesPerCommit: Get<u32>;

        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    #[pallet::storage]
    pub type ChannelNonces<T: Config> = StorageMap<_, Identity, SubNetworkId, u64, ValueQuery>;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::event]
    // #[pallet::generate_deposit(pub(super) fn deposit_event)]
    // This pallet don't have events
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Message came from an invalid network.
        InvalidNetwork,
        /// Message came from an invalid outbound channel on the Ethereum side.
        InvalidSourceChannel,
        /// Submitted invalid commitment type.
        InvalidCommitment,
        /// Message has an unexpected nonce.
        InvalidNonce,
        /// Incorrect reward fraction
        InvalidRewardFraction,
        /// This contract already exists
        ContractExists,
        /// Call encoding failed.
        CallEncodeFailed,
    }

    impl<T: Config> Pallet<T> {
        fn submit_weight(
            commitment: &bridge_types::GenericCommitment<
                T::MaxMessagesPerCommit,
                T::MaxMessagePayloadSize,
            >,
            proof: &<T::Verifier as Verifier>::Proof,
        ) -> Weight {
            let commitment_weight = match commitment {
                bridge_types::GenericCommitment::EVM(_)
                | bridge_types::GenericCommitment::TON(_) => {
                    <T as frame_system::Config>::BlockWeights::get().max_block
                }
                bridge_types::GenericCommitment::Sub(commitment) => commitment
                    .messages
                    .iter()
                    .map(|m| T::MessageDispatch::dispatch_weight(&m.payload))
                    .fold(Weight::zero(), |acc, w| acc.saturating_add(w)),
            };

            let proof_weight = T::Verifier::verify_weight(proof);

            <T as Config>::WeightInfo::submit()
                .saturating_add(commitment_weight)
                .saturating_add(proof_weight)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(Pallet::<T>::submit_weight(commitment, proof))]
        pub fn submit(
            origin: OriginFor<T>,
            network_id: SubNetworkId,
            commitment: bridge_types::GenericCommitment<
                T::MaxMessagesPerCommit,
                T::MaxMessagePayloadSize,
            >,
            proof: <T::Verifier as Verifier>::Proof,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;
            let commitment_hash = commitment.hash();
            let bridge_types::GenericCommitment::Sub(sub_commitment) = commitment else {
                frame_support::fail!(Error::<T>::InvalidCommitment);
            };
            // submit commitment to verifier for verification
            T::Verifier::verify(network_id.into(), commitment_hash, &proof)?;
            // Verify batch nonce
            <ChannelNonces<T>>::try_mutate(network_id, |nonce| -> DispatchResult {
                if sub_commitment.nonce != *nonce + 1 {
                    Err(Error::<T>::InvalidNonce.into())
                } else {
                    *nonce += 1;
                    Ok(())
                }
            })?;

            for (idx, message) in sub_commitment.messages.into_iter().enumerate() {
                let message_id = MessageId::batched(
                    network_id.into(),
                    T::ThisNetworkId::get(),
                    sub_commitment.nonce,
                    idx as u64,
                );
                T::MessageDispatch::dispatch(
                    network_id,
                    message_id,
                    message.timepoint,
                    &message.payload,
                    (),
                );
            }
            Ok(().into())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        // mb add prefetch with validate_ancestors=true to not include unneccessary stuff
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::submit {
                network_id,
                commitment,
                proof,
            } = call
            {
                let nonce = ChannelNonces::<T>::get(network_id);
                // If messages batch already submitted
                if commitment.nonce() != nonce + 1 {
                    return InvalidTransaction::BadProof.into();
                }
                let commitment_hash = commitment.hash();
                T::Verifier::verify((*network_id).into(), commitment_hash, proof).map_err(|e| {
                    warn!("Bad submit proof received: {:?}", e);
                    InvalidTransaction::BadProof
                })?;
                ValidTransaction::with_tag_prefix("SubstrateBridgeChannelSubmit")
                    .priority(T::UnsignedPriority::get())
                    .longevity(T::UnsignedLongevity::get())
                    .and_provides((network_id, commitment_hash))
                    .propagate(true)
                    .build()
            } else {
                warn!("Unknown unsigned call, can't validate");
                InvalidTransaction::Call.into()
            }
        }
    }
}
