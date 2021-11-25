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

use sp_runtime::traits::Hash;

use pallet_mmr::primitives::LeafDataProvider;

use codec::Encode;

pub use pallet::*;

type MerkleRootOf<T> = <T as pallet_mmr::Config>::Hash;

/// A type that is able to return current list of parachain heads that end up in the MMR leaf.

#[frame_support::pallet]
pub mod pallet {
    #![allow(missing_docs)]

    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    /// BEEFY-MMR pallet.
    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_n: T::BlockNumber) {
            let digest = frame_system::Pallet::<T>::digest();
            LatestDigest::<T>::put(digest);
        }
    }

    /// Fee for accepting a message
    #[pallet::storage]
    #[pallet::getter(fn latest_digest)]
    pub(super) type LatestDigest<T: Config> =
        StorageValue<_, sp_runtime::generic::Digest, ValueQuery>;

    /// The module's configuration trait.
    #[pallet::config]
    #[pallet::disable_frame_system_supertrait_check]
    pub trait Config: pallet_beefy_mmr::Config {
        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
    }

    #[pallet::event]
    pub enum Event<T: Config> {}
}

impl<T: pallet_beefy_mmr::Config + Config + pallet_mmr::Config> LeafDataProvider for Pallet<T>
where
    beefy_merkle_tree::Hash: From<<T as pallet_mmr::Config>::Hash>,
    <T as pallet_mmr::Config>::Hash: From<beefy_merkle_tree::Hash>,
{
    type LeafData = (
        <pallet_beefy_mmr::Pallet<T> as LeafDataProvider>::LeafData,
        beefy_merkle_tree::Hash,
    );

    fn leaf_data() -> Self::LeafData {
        let digest = Pallet::<T>::latest_digest();
        let digest_encoded = digest.encode();
        let digest_hash =
            <pallet_beefy_mmr::Pallet<T> as beefy_merkle_tree::Hasher>::hash(&digest_encoded);
        frame_support::log::warn!(
            "get leaf data: block number: {:?}, digest hash: {:?}, digest {:?}",
            frame_system::Pallet::<T>::block_number(),
            digest_hash,
            digest
        );
        (
            <pallet_beefy_mmr::Pallet<T> as LeafDataProvider>::leaf_data(),
            digest_hash,
        )
    }
}

impl<T: Config> beefy_merkle_tree::Hasher for Pallet<T>
where
    MerkleRootOf<T>: Into<beefy_merkle_tree::Hash>,
{
    fn hash(data: &[u8]) -> beefy_merkle_tree::Hash {
        <T as pallet_mmr::Config>::Hashing::hash(data).into()
    }
}

impl<T: Config> Pallet<T> where
    MerkleRootOf<T>: From<beefy_merkle_tree::Hash> + Into<beefy_merkle_tree::Hash>
{
}
