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

/// A type that is able to return current list of parachain heads that end up in the MMR leaf.

#[frame_support::pallet]
pub mod pallet {
    #![allow(missing_docs)]

    use beefy_primitives::mmr::BeefyDataProvider;
    use bridge_types::types::AuxiliaryDigest;
    use frame_support::pallet_prelude::*;
    use sp_runtime::traits;
    use sp_runtime::traits::Hash;

    type HashOf<T> = <T as Config>::Hash;

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
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
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
    }

    #[pallet::event]
    pub enum Event<T: Config> {}

    impl<T: Config> BeefyDataProvider<HashOf<T>> for Pallet<T> {
        fn extra_data() -> HashOf<T> {
            let digest = Pallet::<T>::latest_digest();
            let digest_encoded = digest.encode();
            <T as Config>::Hashing::hash(&digest_encoded)
        }
    }
}
