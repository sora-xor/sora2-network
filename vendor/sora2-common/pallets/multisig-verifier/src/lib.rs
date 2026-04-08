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

use bridge_types::substrate::MultisigVerifierCall;
use bridge_types::substrate::SubstrateBridgeMessageEncode;
use bridge_types::traits::OutboundChannel;
use bridge_types::types::AuxiliaryDigest;
use bridge_types::types::AuxiliaryDigestItem;
use bridge_types::GenericNetworkId;
use frame_support::ensure;
use frame_support::pallet_prelude::*;
use frame_support::{BoundedBTreeSet, BoundedVec};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use scale_info::prelude::vec::Vec;
use sp_core::ecdsa;
use sp_core::RuntimeDebug;
use sp_core::H256;
use sp_runtime::traits::Hash;
use sp_runtime::traits::Keccak256;
use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet};
use sp_std::iter;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

#[derive(Clone, RuntimeDebug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo)]
pub struct Proof {
    pub digest: AuxiliaryDigest,
    pub proof: Vec<ecdsa::Signature>,
}

impl codec::DecodeWithMemTracking for Proof {}

impl<T: Config> From<MultisigVerifierCall> for Call<T> {
    fn from(value: MultisigVerifierCall) -> Self {
        match value {
            MultisigVerifierCall::AddPeer { peer } => Call::add_peer { peer },
            MultisigVerifierCall::RemovePeer { peer } => Call::remove_peer { peer },
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use bridge_types::SubNetworkId;
    use frame_support::dispatch::DispatchResultWithPostInfo;
    use frame_support::pallet_prelude::OptionQuery;
    use frame_support::transactional;
    use frame_support::{fail, Twox64Concat};
    use log::info;
    use sp_runtime::DispatchError;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type CallOrigin: EnsureOrigin<
            Self::RuntimeOrigin,
            Success = bridge_types::types::CallOriginOutput<SubNetworkId, H256, ()>,
        >;

        type OutboundChannel: OutboundChannel<SubNetworkId, Self::AccountId, ()>;

        #[pallet::constant]
        type MaxPeers: Get<u32>;

        #[pallet::constant]
        type MinPeers: Get<u32>;

        type WeightInfo: WeightInfo;

        #[pallet::constant]
        type ThisNetworkId: Get<GenericNetworkId>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn get_peer_keys)]
    pub type PeerKeys<T> = StorageMap<
        _,
        Twox64Concat,
        GenericNetworkId,
        BoundedBTreeSet<ecdsa::Public, <T as Config>::MaxPeers>,
        OptionQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        NetworkInitialized(GenericNetworkId),
        VerificationSuccessful(GenericNetworkId),
        PeerAdded(ecdsa::Public),
        PeerRemoved(ecdsa::Public),
        PeerSetForced(GenericNetworkId, u32),
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidInitParams,
        TooMuchPeers,
        NetworkNotInitialized,
        InvalidNumberOfSignatures,
        InvalidSignature,
        NotTrustedPeerSignature,
        PeerExists,
        NoSuchPeer,
        InvalidNetworkId,
        CommitmentNotFoundInDigest,
        DuplicatedPeer,
        NotEnoughPeers,
        PeerRegisteredInOtherNetwork,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::initialize())]
        pub fn initialize(
            origin: OriginFor<T>,
            network_id: GenericNetworkId,
            peers: BoundedVec<ecdsa::Public, T::MaxPeers>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let btree_peers = Self::canonicalize_peers(peers, true)?;
            Self::ensure_no_substrate_peer_overlap(network_id, btree_peers.iter())?;
            PeerKeys::<T>::set(network_id, Some(btree_peers));
            Self::deposit_event(Event::NetworkInitialized(network_id));
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::add_peer())]
        pub fn add_peer(origin: OriginFor<T>, peer: ecdsa::Public) -> DispatchResultWithPostInfo {
            let output = T::CallOrigin::ensure_origin(origin)?;
            info!("Call add_peer {:?} by {:?}", peer, output);
            let network_id = GenericNetworkId::from(output.network_id);
            Self::ensure_no_substrate_peer_overlap(network_id, iter::once(&peer))?;
            PeerKeys::<T>::try_mutate(network_id, |x| -> DispatchResult {
                let Some(peers) = x else {
                    fail!(Error::<T>::NetworkNotInitialized)
                };
                if peers.contains(&peer) {
                    fail!(Error::<T>::PeerExists);
                } else {
                    peers
                        .try_insert(peer)
                        .map_err(|_| Error::<T>::TooMuchPeers)?;
                }
                Ok(())
            })?;
            T::OutboundChannel::submit(
                output.network_id,
                &frame_system::RawOrigin::Root,
                &bridge_types::substrate::DataSignerCall::AddPeer { peer }.prepare_message(),
                (),
            )?;
            Self::deposit_event(Event::PeerAdded(peer));
            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::remove_peer())]
        pub fn remove_peer(
            origin: OriginFor<T>,
            peer: ecdsa::Public,
        ) -> DispatchResultWithPostInfo {
            let output = T::CallOrigin::ensure_origin(origin)?;
            info!("Call remove_peer {:?} by {:?}", peer, output);
            PeerKeys::<T>::try_mutate(
                GenericNetworkId::from(output.network_id),
                |x| -> DispatchResult {
                    let Some(keys) = x else {
                        fail!(Error::<T>::NetworkNotInitialized)
                    };
                    ensure!(keys.contains(&peer), {
                        log::error!("Call remove_peer: No such peer {:?}", peer);
                        Error::<T>::NoSuchPeer
                    });
                    ensure!(
                        keys.len() as u32 > T::MinPeers::get(),
                        Error::<T>::NotEnoughPeers
                    );
                    keys.remove(&peer);
                    Ok(())
                },
            )?;

            T::OutboundChannel::submit(
                output.network_id,
                &frame_system::RawOrigin::Root,
                &bridge_types::substrate::DataSignerCall::RemovePeer { peer }.prepare_message(),
                (),
            )?;

            Self::deposit_event(Event::PeerRemoved(peer));
            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::force_set_peers())]
        pub fn force_set_peers(
            origin: OriginFor<T>,
            network_id: GenericNetworkId,
            peers: BoundedVec<ecdsa::Public, T::MaxPeers>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let peers = Self::canonicalize_peers(peers, false)?;
            Self::ensure_no_substrate_peer_overlap(network_id, peers.iter())?;
            let peer_count = peers.len() as u32;
            PeerKeys::<T>::set(network_id, Some(peers));
            Self::deposit_event(Event::PeerSetForced(network_id, peer_count));
            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        fn canonicalize_peers(
            peers: BoundedVec<ecdsa::Public, T::MaxPeers>,
            require_minimum: bool,
        ) -> Result<BoundedBTreeSet<ecdsa::Public, T::MaxPeers>, DispatchError> {
            let unique_peers = peers.into_iter().collect::<BTreeSet<_>>();
            let minimum = if require_minimum {
                T::MinPeers::get()
            } else {
                1
            };
            ensure!(
                unique_peers.len() as u32 >= minimum,
                Error::<T>::NotEnoughPeers
            );
            unique_peers
                .try_into()
                .map_err(|_| Error::<T>::TooMuchPeers.into())
        }

        fn ensure_no_substrate_peer_overlap<'a>(
            network_id: GenericNetworkId,
            peers: impl IntoIterator<Item = &'a ecdsa::Public>,
        ) -> DispatchResult {
            if !matches!(network_id, GenericNetworkId::Sub(_)) {
                return Ok(());
            }

            let peers = peers.into_iter().cloned().collect::<BTreeSet<_>>();
            if peers.is_empty() {
                return Ok(());
            }

            let has_overlap = PeerKeys::<T>::iter()
                .filter(|(other_network_id, _)| {
                    *other_network_id != network_id
                        && matches!(other_network_id, GenericNetworkId::Sub(_))
                })
                .any(|(_, stored_peers)| stored_peers.iter().any(|peer| peers.contains(peer)));
            ensure!(!has_overlap, Error::<T>::PeerRegisteredInOtherNetwork);
            Ok(())
        }

        pub fn ensure_unique_substrate_peers() -> DispatchResult {
            let mut seen = BTreeMap::new();
            for (network_id, peers) in PeerKeys::<T>::iter()
                .filter(|(network_id, _)| matches!(network_id, GenericNetworkId::Sub(_)))
            {
                for peer in peers {
                    if let Some(existing) = seen.insert(peer, network_id) {
                        ensure!(
                            existing == network_id,
                            Error::<T>::PeerRegisteredInOtherNetwork
                        );
                    }
                }
            }
            Ok(())
        }

        pub fn verify_signatures(
            network_id: GenericNetworkId,
            hash: H256,
            signatures: &[ecdsa::Signature],
        ) -> DispatchResult {
            let Some(peers) = PeerKeys::<T>::get(network_id) else {
                log::error!(
                    "verify_signatures: Network {:?} not initialized",
                    network_id
                );
                fail!(Error::<T>::NetworkNotInitialized)
            };

            let mut unique_peers = BTreeSet::new();

            // Insure that every sighnature exists in the storage
            for sign in signatures {
                let Ok(rec_sign) =
                    sp_io::crypto::secp256k1_ecdsa_recover_compressed(&sign.0, &hash.0)
                else {
                    log::error!("verify_signatures: cannot recover: {:?}", sign);
                    fail!(Error::<T>::InvalidSignature)
                };
                let rec_sign = ecdsa::Public::from_raw(rec_sign);
                if !unique_peers.insert(rec_sign) {
                    fail!(Error::<T>::DuplicatedPeer);
                }
                ensure!(peers.contains(&rec_sign), {
                    log::error!("verify_signatures: not trusted signatures: {:?}", sign);
                    Error::<T>::NotTrustedPeerSignature
                });
            }

            let len = unique_peers.len() as u32;
            let treshold = bridge_types::utils::threshold(peers.len() as u32);
            ensure!(len >= treshold, {
                log::error!(
                    "verify_signatures: invalid number of signatures: {:?} < {:?}",
                    len,
                    treshold
                );
                Error::<T>::InvalidNumberOfSignatures
            });

            Self::deposit_event(Event::VerificationSuccessful(network_id));

            Ok(())
        }
    }
}

impl<T: Config> bridge_types::traits::Verifier for Pallet<T> {
    type Proof = Proof;

    fn verify(
        network_id: GenericNetworkId,
        commitment_hash: H256,
        proof: &Self::Proof,
    ) -> DispatchResult {
        let this_network_id = T::ThisNetworkId::get();
        let digest_hash = Keccak256::hash_of(&proof.digest);
        Self::verify_signatures(network_id, digest_hash, &proof.proof)?;
        let count = proof
            .digest
            .logs
            .iter()
            .filter(|x| {
                let AuxiliaryDigestItem::Commitment(log_network_id, log_commitment_hash) = x;
                // Digest proofs should only come from substrate networks
                if matches!(log_network_id, GenericNetworkId::Sub(_)) {
                    return *log_network_id == this_network_id
                        && commitment_hash == *log_commitment_hash;
                }
                false
            })
            .count();
        ensure!(count == 1, Error::<T>::CommitmentNotFoundInDigest);

        Ok(())
    }

    fn verify_weight(proof: &Self::Proof) -> Weight {
        <T as Config>::WeightInfo::verifier_verify(proof.proof.len() as u32)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn valid_proof() -> Option<Self::Proof> {
        None
    }
}

pub struct MultiEVMVerifier<T>(PhantomData<T>);

#[derive(Clone, RuntimeDebug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo)]
pub struct MultiEVMProof {
    pub proof: Vec<ecdsa::Signature>,
}

impl codec::DecodeWithMemTracking for MultiEVMProof {}

impl<T: Config> bridge_types::traits::Verifier for MultiEVMVerifier<T> {
    type Proof = MultiEVMProof;

    fn verify(
        network_id: GenericNetworkId,
        commitment_hash: H256,
        proof: &Self::Proof,
    ) -> DispatchResult {
        let this_network_id = T::ThisNetworkId::get();
        let approved_hash = Keccak256::hash_of(&(network_id, this_network_id, commitment_hash));
        Pallet::<T>::verify_signatures(network_id, approved_hash, &proof.proof)?;
        Ok(())
    }

    fn verify_weight(proof: &Self::Proof) -> Weight {
        <T as Config>::WeightInfo::verifier_verify(proof.proof.len() as u32)
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn valid_proof() -> Option<Self::Proof> {
        None
    }
}
