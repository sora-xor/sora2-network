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

    use bridge_types::traits::AuxiliaryDigestHandler;
    use bridge_types::types::{AuxiliaryDigest, AuxiliaryDigestItem, LeafExtraData};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::Randomness;
    use frame_system::pallet_prelude::*;
    use sp_consensus_beefy::mmr::BeefyDataProvider;
    use sp_runtime::traits;
    use sp_runtime::traits::Hash;
    use sp_std::prelude::*;

    use crate::RANDOMNESS_SUBJECT;

    type HashOf<T> = <T as Config>::Hash;
    type RandomnessOutputOf<T> = <T as frame_system::Config>::Hash;

    /// Leaf Provider pallet.
    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    /// Latest digest
    #[pallet::storage]
    #[pallet::getter(fn latest_digest)]
    pub(super) type LatestDigest<T: Config> =
        StorageValue<_, Vec<AuxiliaryDigestItem>, OptionQuery>;

    /// The module's configuration trait.
    #[pallet::config]
    #[pallet::disable_frame_system_supertrait_check]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
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

        type Randomness: Randomness<RandomnessOutputOf<Self>, BlockNumberFor<Self>>;
    }

    #[pallet::event]
    pub enum Event<T: Config> {}

    impl<T: Config> AuxiliaryDigestHandler for Pallet<T> {
        fn add_item(item: AuxiliaryDigestItem) {
            LatestDigest::<T>::append(item);
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Clear the latest digest. This pallet should be placed before any other pallets which is use AuxiliaryDigestHandler.
        fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
            LatestDigest::<T>::kill();
            <T as frame_system::Config>::DbWeight::get().writes(1)
        }
    }

    impl<T: Config> BeefyDataProvider<LeafExtraData<HashOf<T>, RandomnessOutputOf<T>>> for Pallet<T> {
        fn extra_data() -> LeafExtraData<HashOf<T>, RandomnessOutputOf<T>> {
            let digest = AuxiliaryDigest {
                logs: LatestDigest::<T>::get().unwrap_or_default(),
            };
            let digest_encoded = digest.encode();
            let (random_seed, _) = T::Randomness::random(RANDOMNESS_SUBJECT);
            let digest_hash = <T as Config>::Hashing::hash(&digest_encoded);
            LeafExtraData {
                random_seed,
                digest_hash,
            }
        }
    }
}
