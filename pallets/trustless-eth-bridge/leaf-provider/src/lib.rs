// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

//! A BEEFY+MMR pallet combo.
//!
//! While both BEEFY and Merkle Mountain Range (MMR) can be used separately,
//! these tools were designed to work together in unison.
//!
//! The pallet provides a standardized MMR Leaf format that is can be used
//! to bridge BEEFY+MMR-based networks (both standalone and polkadot-like).
//!
//! The MMR leaf contains:
//! 1. Block number and parent block hash.
//! 2. Merkle Tree Root Hash of next BEEFY validator set.
//! 3. Merkle Tree Root Hash of current parachain heads state.
//!
//! and thanks to versioning can be easily updated in the future.

pub use pallet::*;

/// Subject for randomness.
pub const RANDOMNESS_SUBJECT: &[u8] = b"beefy-leaf-extra";

/// A type that is able to return current list of parachain heads that end up in the MMR leaf.

#[frame_support::pallet]
pub mod pallet {
    #![allow(missing_docs)]

    use beefy_primitives::mmr::BeefyDataProvider;
    use bridge_types::types::{AuxiliaryDigest, LeafExtraData};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::Randomness;
    use sp_runtime::traits;
    use sp_runtime::traits::Hash;

    use crate::RANDOMNESS_SUBJECT;

    type HashOf<T> = <T as Config>::Hash;
    type RandomnessOutputOf<T> = <T as frame_system::Config>::Hash;

    /// BEEFY-MMR pallet.
    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
        fn on_finalize(_n: T::BlockNumber) {
            let digest: AuxiliaryDigest = frame_system::Pallet::<T>::digest().into();
            LatestDigest::<T>::put(digest);
        }
    }

    /// Latest digest
    #[pallet::storage]
    #[pallet::getter(fn latest_digest)]
    pub(super) type LatestDigest<T: Config> = StorageValue<_, AuxiliaryDigest, ValueQuery>;

    /// The module's configuration trait.
    #[pallet::config]
    #[pallet::disable_frame_system_supertrait_check]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Hashing: traits::Hash<Output = <Self as Config>::Hash>;
        type Hash: traits::Member
            + traits::MaybeSerializeDeserialize
            + sp_std::fmt::Debug
            + sp_std::hash::Hash
            + AsRef<[u8]>
            + AsMut<[u8]>
            + Copy
            + Default
            + codec::Codec
            + codec::EncodeLike
            + scale_info::TypeInfo
            + MaxEncodedLen;

        type Randomness: Randomness<RandomnessOutputOf<Self>, Self::BlockNumber>;
    }

    #[pallet::event]
    pub enum Event<T: Config> {}

    impl<T: Config> BeefyDataProvider<LeafExtraData<HashOf<T>, RandomnessOutputOf<T>>> for Pallet<T> {
        fn extra_data() -> LeafExtraData<HashOf<T>, RandomnessOutputOf<T>> {
            let digest = Pallet::<T>::latest_digest();
            let digest_encoded = digest.encode();
            let (random_seed, _) = T::Randomness::random(&RANDOMNESS_SUBJECT);
            let digest_hash = <T as Config>::Hashing::hash(&digest_encoded);
            LeafExtraData {
                random_seed,
                digest_hash,
            }
        }
    }
}
