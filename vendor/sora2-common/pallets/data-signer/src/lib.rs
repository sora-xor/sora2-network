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

use bridge_types::substrate::DataSignerCall;
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::WeightInfo;

pub(crate) const LOG_TARGET: &str = "runtime::data-signer";

// syntactic sugar for logging.
#[macro_export]
macro_rules! log {
	($level:tt, $patter:expr $(, $values:expr)* $(,)?) => {
		log::$level!(
			target: $crate::LOG_TARGET,
			concat!("[{:?}] 💸 ", $patter), <frame_system::Pallet<T>>::block_number() $(, $values)*
		)
	};
}

impl<T: Config> From<DataSignerCall> for Call<T> {
    fn from(value: DataSignerCall) -> Self {
        match value {
            DataSignerCall::AddPeer { peer } => Call::finish_add_peer { peer },
            DataSignerCall::RemovePeer { peer } => Call::finish_remove_peer { peer },
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    #![allow(missing_docs)]

    use super::WeightInfo;
    use bridge_types::substrate::MultisigVerifierCall;
    use bridge_types::substrate::SubstrateBridgeMessageEncode;
    use bridge_types::traits::OutboundChannel;
    use bridge_types::types::CallOriginOutput;
    use bridge_types::{GenericNetworkId, SubNetworkId, H256};
    use frame_support::dispatch::Pays;
    use frame_support::fail;
    use frame_support::transactional;
    use frame_support::BoundedBTreeMap;
    use frame_support::{pallet_prelude::*, BoundedBTreeSet, BoundedVec};
    use frame_system::ensure_root;
    use frame_system::pallet_prelude::*;
    use frame_system::RawOrigin;
    use sp_core::ecdsa;
    use sp_core::Get;
    use sp_core::TryCollect;
    use sp_runtime::DispatchError;
    use sp_std::collections::{btree_map::BTreeMap, btree_set::BTreeSet};
    use sp_std::iter;
    use sp_std::vec::Vec;

    /// Data Signer Pallet
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The module's configuration trait.
    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        type OutboundChannel: OutboundChannel<SubNetworkId, Self::AccountId, ()>;

        type CallOrigin: EnsureOrigin<
            Self::RuntimeOrigin,
            Success = CallOriginOutput<SubNetworkId, H256, ()>,
        >;

        /// A configuration for base priority of unsigned transactions.
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;

        /// A configuration for longevity of unsigned transactions.
        #[pallet::constant]
        type UnsignedLongevity: Get<u64>;

        #[pallet::constant]
        type MaxPeers: Get<u32>;

        #[pallet::constant]
        type MinPeers: Get<u32>;

        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        Initialized {
            network_id: GenericNetworkId,
            peers: BoundedVec<ecdsa::Public, T::MaxPeers>,
        },
        AddedPeer {
            network_id: GenericNetworkId,
            peer: ecdsa::Public,
        },
        RemovedPeer {
            network_id: GenericNetworkId,
            peer: ecdsa::Public,
        },
        ApprovalAccepted {
            network_id: GenericNetworkId,
            data: H256,
            signature: ecdsa::Signature,
        },
        Approved {
            network_id: GenericNetworkId,
            data: H256,
            signatures: BoundedVec<ecdsa::Signature, T::MaxPeers>,
        },
        PendingApprovalRegistered {
            network_id: GenericNetworkId,
            data: H256,
        },
        PendingApprovalCleared {
            network_id: GenericNetworkId,
            data: H256,
        },
        PeerSetForced {
            network_id: GenericNetworkId,
            peer_count: u32,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        PalletInitialized,
        PalletNotInitialized,
        PeerExists,
        PeerNotExists,
        TooMuchPeers,
        FailedToVerifySignature,
        PeerNotFound,
        TooMuchApprovals,
        ApprovalsNotFound,
        SignaturesNotFound,
        HasPendingPeerUpdate,
        DontHavePendingPeerUpdates,
        NetworkNotSupported,
        SignatureAlreadyExists,
        NotEnoughPeers,
        ApprovalNotPending,
        PendingApprovalAlreadyExists,
        PeerRegisteredInOtherNetwork,
    }

    /// Peers
    #[pallet::storage]
    #[pallet::getter(fn peers)]
    pub(super) type Peers<T: Config> = StorageMap<
        _,
        Identity,
        GenericNetworkId,
        BoundedBTreeSet<ecdsa::Public, T::MaxPeers>,
        OptionQuery,
    >;

    /// Pending peers
    #[pallet::storage]
    #[pallet::getter(fn pending_peer_update)]
    pub(super) type PendingPeerUpdate<T: Config> =
        StorageMap<_, Identity, GenericNetworkId, bool, ValueQuery>;

    /// Pending approval hashes that may be signed by bridge peers.
    #[pallet::storage]
    #[pallet::getter(fn pending_approval)]
    pub(super) type PendingApprovals<T: Config> =
        StorageDoubleMap<_, Identity, GenericNetworkId, Identity, H256, bool, ValueQuery>;

    /// Approvals
    #[pallet::storage]
    #[pallet::getter(fn approvals)]
    pub(super) type Approvals<T: Config> = StorageDoubleMap<
        _,
        Identity,
        GenericNetworkId,
        Identity,
        H256,
        BoundedBTreeMap<ecdsa::Public, ecdsa::Signature, T::MaxPeers>,
        ValueQuery,
    >;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::register_network())]
        pub fn register_network(
            origin: OriginFor<T>,
            network_id: GenericNetworkId,
            peers: BoundedVec<ecdsa::Public, T::MaxPeers>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let peers = Self::canonicalize_peers(peers, true)?;
            Self::ensure_no_substrate_peer_overlap(network_id, peers.iter())?;
            Peers::<T>::try_mutate(network_id, |storage_peers| {
                if storage_peers.is_some() {
                    return Err(Error::<T>::PalletInitialized);
                } else {
                    *storage_peers = Some(peers.clone());
                }
                Ok(())
            })?;
            Self::deposit_event(Event::<T>::Initialized {
                network_id,
                peers: Self::peers_to_vec(&peers)?,
            });
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::approve())]
        pub fn approve(
            origin: OriginFor<T>,
            network_id: GenericNetworkId,
            data: H256,
            signature: ecdsa::Signature,
        ) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;
            let peers = Peers::<T>::get(network_id).ok_or(Error::<T>::PalletNotInitialized)?;
            ensure!(
                PendingApprovals::<T>::get(network_id, data),
                Error::<T>::ApprovalNotPending
            );
            let public = sp_io::crypto::secp256k1_ecdsa_recover_compressed(&signature.0, &data.0)
                .map_err(|_| Error::<T>::FailedToVerifySignature)?;
            let public = ecdsa::Public::from_raw(public);
            ensure!(peers.contains(&public), Error::<T>::PeerNotFound);
            let mut approvals = Approvals::<T>::get(network_id, data);
            approvals.retain(|approved_peer, _| peers.contains(approved_peer));
            if approvals.contains_key(&public) {
                fail!(Error::<T>::SignatureAlreadyExists);
            }
            approvals
                .try_insert(public, signature.clone())
                .map_err(|_| Error::<T>::TooMuchApprovals)?;
            Approvals::<T>::insert(network_id, data, &approvals);
            let peers_len = peers.len() as u32;
            Self::deposit_event(Event::<T>::ApprovalAccepted {
                network_id,
                data,
                signature,
            });
            if (approvals.len() as u32) >= bridge_types::utils::threshold(peers_len) {
                let signatures = approvals
                    .values()
                    .cloned()
                    .try_collect()
                    .map_err(|_| Error::<T>::TooMuchApprovals)?;
                Self::deposit_event(Event::<T>::Approved {
                    network_id,
                    data,
                    signatures,
                });
            }
            Ok(Pays::No.into())
        }

        #[pallet::call_index(2)]
        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::add_peer())]
        pub fn add_peer(
            origin: OriginFor<T>,
            network_id: GenericNetworkId,
            peer: ecdsa::Public,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ensure!(
                !PendingPeerUpdate::<T>::get(network_id),
                Error::<T>::HasPendingPeerUpdate
            );
            Self::ensure_no_substrate_peer_overlap(network_id, iter::once(&peer))?;
            Peers::<T>::try_mutate(network_id, |peers| {
                if let Some(peers) = peers {
                    if peers.contains(&peer) {
                        return Err(Error::<T>::PeerExists);
                    } else {
                        peers
                            .try_insert(peer)
                            .map_err(|_| Error::<T>::TooMuchPeers)?;
                    }
                } else {
                    return Err(Error::<T>::PalletNotInitialized);
                }
                Ok(())
            })?;
            PendingPeerUpdate::<T>::insert(network_id, true);
            let network_id = network_id.sub().ok_or(Error::<T>::NetworkNotSupported)?;
            T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &MultisigVerifierCall::AddPeer { peer }.prepare_message(),
                (),
            )?;
            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::remove_peer())]
        pub fn remove_peer(
            origin: OriginFor<T>,
            network_id: GenericNetworkId,
            peer: ecdsa::Public,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ensure!(
                !PendingPeerUpdate::<T>::get(network_id),
                Error::<T>::HasPendingPeerUpdate
            );
            let peers = Peers::<T>::get(network_id).ok_or(Error::<T>::PalletNotInitialized)?;
            ensure!(peers.contains(&peer), Error::<T>::PeerNotExists);
            ensure!(
                peers.len() as u32 > T::MinPeers::get(),
                Error::<T>::NotEnoughPeers
            );
            // Do nothing to ensure we have enough approvals for remove peer request
            // Will be actually removed after request from sidechain
            PendingPeerUpdate::<T>::insert(network_id, true);
            let network_id = network_id.sub().ok_or(Error::<T>::NetworkNotSupported)?;
            T::OutboundChannel::submit(
                network_id,
                &RawOrigin::Root,
                &MultisigVerifierCall::RemovePeer { peer }.prepare_message(),
                (),
            )?;
            Ok(().into())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::finish_remove_peer())]
        pub fn finish_remove_peer(
            origin: OriginFor<T>,
            peer: ecdsa::Public,
        ) -> DispatchResultWithPostInfo {
            let CallOriginOutput { network_id, .. } = T::CallOrigin::ensure_origin(origin)?;
            let network_id: GenericNetworkId = network_id.into();
            ensure!(
                PendingPeerUpdate::<T>::get(network_id),
                Error::<T>::DontHavePendingPeerUpdates
            );
            Peers::<T>::try_mutate(network_id, |peers| {
                if let Some(peers) = peers {
                    if !peers.contains(&peer) {
                        return Err(Error::<T>::PeerNotExists);
                    } else if peers.len() as u32 <= T::MinPeers::get() {
                        return Err(Error::<T>::NotEnoughPeers);
                    } else {
                        peers.remove(&peer);
                    }
                } else {
                    return Err(Error::<T>::PalletNotInitialized);
                }
                Ok(())
            })?;
            PendingPeerUpdate::<T>::insert(network_id, false);
            Ok(().into())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(<T as Config>::WeightInfo::finish_add_peer())]
        pub fn finish_add_peer(
            origin: OriginFor<T>,
            _peer: ecdsa::Public,
        ) -> DispatchResultWithPostInfo {
            let CallOriginOutput { network_id, .. } = T::CallOrigin::ensure_origin(origin)?;
            let network_id: GenericNetworkId = network_id.into();
            ensure!(
                PendingPeerUpdate::<T>::get(network_id),
                Error::<T>::DontHavePendingPeerUpdates
            );
            PendingPeerUpdate::<T>::insert(network_id, false);
            Ok(().into())
        }

        #[pallet::call_index(6)]
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
            Peers::<T>::insert(network_id, peers);
            PendingPeerUpdate::<T>::insert(network_id, false);
            Self::deposit_event(Event::<T>::PeerSetForced {
                network_id,
                peer_count,
            });
            Ok(().into())
        }

        #[pallet::call_index(7)]
        #[pallet::weight(<T as Config>::WeightInfo::register_pending_approval())]
        pub fn register_pending_approval(
            origin: OriginFor<T>,
            network_id: GenericNetworkId,
            data: H256,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ensure!(
                Peers::<T>::contains_key(network_id),
                Error::<T>::PalletNotInitialized
            );
            ensure!(
                !PendingApprovals::<T>::get(network_id, data),
                Error::<T>::PendingApprovalAlreadyExists
            );
            // Start every approval window from a clean slate, including legacy leftovers.
            Approvals::<T>::remove(network_id, data);
            PendingApprovals::<T>::insert(network_id, data, true);
            Self::deposit_event(Event::<T>::PendingApprovalRegistered { network_id, data });
            Ok(().into())
        }

        #[pallet::call_index(8)]
        #[pallet::weight(<T as Config>::WeightInfo::clear_pending_approval())]
        pub fn clear_pending_approval(
            origin: OriginFor<T>,
            network_id: GenericNetworkId,
            data: H256,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ensure!(
                PendingApprovals::<T>::get(network_id, data),
                Error::<T>::ApprovalNotPending
            );
            PendingApprovals::<T>::remove(network_id, data);
            Approvals::<T>::remove(network_id, data);
            Self::deposit_event(Event::<T>::PendingApprovalCleared { network_id, data });
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

        fn peers_to_vec(
            peers: &BoundedBTreeSet<ecdsa::Public, T::MaxPeers>,
        ) -> Result<BoundedVec<ecdsa::Public, T::MaxPeers>, DispatchError> {
            peers
                .iter()
                .cloned()
                .collect::<Vec<_>>()
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

            let has_overlap = Peers::<T>::iter()
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
            for (network_id, peers) in Peers::<T>::iter()
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
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        // mb add prefetch with validate_ancestors=true to not include unneccessary stuff
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            if let Call::approve {
                network_id,
                data,
                signature,
            } = call
            {
                let peers = Peers::<T>::get(network_id).ok_or(InvalidTransaction::BadSigner)?;
                ensure!(
                    PendingApprovals::<T>::get(network_id, *data),
                    InvalidTransaction::Stale
                );
                let public =
                    sp_io::crypto::secp256k1_ecdsa_recover_compressed(&signature.0, &data.0)
                        .map_err(|_| InvalidTransaction::BadProof)?;
                let public = ecdsa::Public::from_raw(public);
                ensure!(peers.contains(&public), InvalidTransaction::BadSigner);
                let has_current_approval =
                    Approvals::<T>::get(network_id, data)
                        .into_iter()
                        .any(|(approved_peer, _)| {
                            approved_peer == public && peers.contains(&approved_peer)
                        });
                if has_current_approval {
                    fail!(InvalidTransaction::Stale);
                }
                ValidTransaction::with_tag_prefix("DataSignerApprove")
                    .priority(T::UnsignedPriority::get())
                    .longevity(T::UnsignedLongevity::get())
                    .and_provides((network_id, data, public))
                    .propagate(true)
                    .build()
            } else {
                log!(warn, "Unknown unsigned call, can't validate");
                InvalidTransaction::Call.into()
            }
        }
    }
}
