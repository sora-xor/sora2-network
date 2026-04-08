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

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mock;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

use frame_support::dispatch::{DispatchResult, Parameter};
use frame_support::traits::{Contains, EnsureOrigin};
use sp_runtime::traits::Dispatchable;

use sp_core::RuntimeDebug;

use sp_std::prelude::*;

use bridge_types::traits;

use bridge_types::H256;
use codec::{Decode, Encode};

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<T: pallet::Config<I>, I: 'static = ()> {
    fn successful_dispatch_context() -> (
        <<T as pallet::Config<I>>::OriginOutput as traits::BridgeOriginOutput>::NetworkId,
        <<T as pallet::Config<I>>::OriginOutput as traits::BridgeOriginOutput>::Additional,
        Vec<u8>,
    );
}

#[cfg(feature = "runtime-benchmarks")]
impl<T: pallet::Config<I>, I: 'static> BenchmarkHelper<T, I> for () {
    fn successful_dispatch_context() -> (
        <<T as pallet::Config<I>>::OriginOutput as traits::BridgeOriginOutput>::NetworkId,
        <<T as pallet::Config<I>>::OriginOutput as traits::BridgeOriginOutput>::Additional,
        Vec<u8>,
    ) {
        unimplemented!("benchmark helper is not configured for this mock runtime")
    }
}

#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    RuntimeDebug,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
pub struct RawOrigin<OriginOutput: traits::BridgeOriginOutput> {
    pub origin: OriginOutput,
}

impl<OriginOutput> codec::DecodeWithMemTracking for RawOrigin<OriginOutput>
where
    OriginOutput: traits::BridgeOriginOutput + codec::Decode,
{
}

impl<OriginOutput: traits::BridgeOriginOutput> RawOrigin<OriginOutput> {
    pub fn new(origin: OriginOutput) -> Self {
        Self { origin }
    }
}

#[derive(Default)]
pub struct EnsureAccount<OriginOutput: traits::BridgeOriginOutput>(
    sp_std::marker::PhantomData<OriginOutput>,
);

impl<OuterOrigin, OriginOutput> EnsureOrigin<OuterOrigin> for EnsureAccount<OriginOutput>
where
    OuterOrigin: Into<Result<RawOrigin<OriginOutput>, OuterOrigin>> + From<RawOrigin<OriginOutput>>,
    OriginOutput: Default + traits::BridgeOriginOutput,
{
    type Success = OriginOutput;

    fn try_origin(o: OuterOrigin) -> Result<Self::Success, OuterOrigin> {
        o.into().map(|o| o.origin)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<OuterOrigin, ()>
    where
        OriginOutput: Default,
    {
        Ok(RawOrigin {
            origin: Default::default(),
        }
        .into())
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    use super::*;
    use crate::weights::WeightInfo;
    use bridge_types::GenericTimepoint;
    use frame_support::dispatch::GetDispatchInfo;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_support::weights::Weight;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Hash;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    type NetworkIdOf<T, I> =
        <<T as Config<I>>::OriginOutput as traits::BridgeOriginOutput>::NetworkId;
    type AdditionalOf<T, I> =
        <<T as Config<I>>::OriginOutput as traits::BridgeOriginOutput>::Additional;

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::config]
    pub trait Config<I: 'static = ()>:
        frame_system::Config<RuntimeEvent: From<Event<Self, I>>>
    {

        type OriginOutput: traits::BridgeOriginOutput;

        /// The overarching origin type.
        type Origin: From<RawOrigin<Self::OriginOutput>>;

        /// Id of the message. Whenever message is passed to the dispatch module, it emits
        /// event with this id + dispatch result.
        type MessageId: Parameter;

        type Hashing: Hash<Output = H256>;

        /// The overarching dispatch call type.
        type Call: Parameter
            + Dispatchable<
                RuntimeOrigin = <Self as Config<I>>::Origin,
                PostInfo = frame_support::dispatch::PostDispatchInfo,
            > + GetDispatchInfo;

        /// The pallet will filter all incoming calls right before they're dispatched. If this filter
        /// rejects the call, special event (`Event::MessageRejected`) is emitted.
        type CallFilter: Contains<<Self as Config<I>>::Call>;

        type WeightInfo: WeightInfo;

        #[cfg(feature = "runtime-benchmarks")]
        type BenchmarkHelper: crate::BenchmarkHelper<Self, I>;
    }

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {}

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// Message has been dispatched with given result.
        MessageDispatched(T::MessageId, DispatchResult),
        /// Message has been rejected
        MessageRejected(T::MessageId),
        /// We have failed to decode a Call from the message.
        MessageDecodeFailed(T::MessageId),
    }

    #[pallet::origin]
    #[allow(type_alias_bounds)]
    pub type Origin<T: Config<I>, I: 'static = ()> = RawOrigin<<T as Config<I>>::OriginOutput>;

    impl<T: Config<I>, I: 'static>
        traits::MessageDispatch<T, NetworkIdOf<T, I>, T::MessageId, AdditionalOf<T, I>>
        for Pallet<T, I>
    {
        fn dispatch(
            network_id: NetworkIdOf<T, I>,
            message_id: T::MessageId,
            timepoint: GenericTimepoint,
            payload: &[u8],
            additional: AdditionalOf<T, I>,
        ) {
            let call = match <T as Config<I>>::Call::decode(&mut &payload[..]) {
                Ok(call) => call,
                Err(_) => {
                    Self::deposit_event(Event::MessageDecodeFailed(message_id));
                    return;
                }
            };

            if !T::CallFilter::contains(&call) {
                Self::deposit_event(Event::MessageRejected(message_id));
                return;
            }

            let origin = RawOrigin::new(<T::OriginOutput as traits::BridgeOriginOutput>::new(
                network_id,
                message_id.using_encoded(<T as Config<I>>::Hashing::hash),
                timepoint,
                additional,
            ))
            .into();
            let result = call.dispatch(origin);

            Self::deposit_event(Event::MessageDispatched(
                message_id,
                result.map(drop).map_err(|e| e.error),
            ));
        }

        fn dispatch_weight(payload: &[u8]) -> Weight {
            let call = match <T as Config<I>>::Call::decode(&mut &payload[..]) {
                Ok(call) => call,
                Err(_) => {
                    return <T as Config<I>>::WeightInfo::dispatch_decode_failed();
                }
            };
            let dispatch_info = call.get_dispatch_info();
            dispatch_info
                .call_weight
                .saturating_add(<T as Config<I>>::WeightInfo::dispatch_success())
        }

        #[cfg(feature = "runtime-benchmarks")]
        fn successful_dispatch_event(
            id: T::MessageId,
        ) -> Option<<T as frame_system::Config>::RuntimeEvent> {
            let event: <T as frame_system::Config>::RuntimeEvent =
                Event::<T, I>::MessageDispatched(id, Ok(())).into();
            Some(event)
        }
    }
}
