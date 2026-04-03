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

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet as MultisigVerifier;
use bridge_types::traits::Verifier;
use bridge_types::{GenericNetworkId, SubNetworkId};
use core::fmt::Write;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_system::{self, RawOrigin};
use sp_core::ecdsa;
use sp_std::prelude::*;
use sp_std::Writer;

fn initial_keys<T: Config>(n: usize) -> BoundedVec<ecdsa::Public, <T as Config>::MaxPeers> {
    let mut keys = Vec::new();
    for i in 0..n {
        let key = generate_key(i);
        keys.push(key);
    }

    keys.try_into().unwrap()
}

fn generate_key(i: usize) -> ecdsa::Public {
    let mut seed = Writer::default();
    core::write!(seed, "//TestPeer//p-{}", i).unwrap();
    sp_io::crypto::ecdsa_generate(sp_core::crypto::key_types::DUMMY, Some(seed.into_inner()))
}

fn initialize_network<T: Config>(
    network_id: GenericNetworkId,
    n: usize,
) -> BoundedVec<ecdsa::Public, <T as Config>::MaxPeers> {
    let keys = initial_keys::<T>(n);
    assert_ok!(MultisigVerifier::<T>::initialize(
        RawOrigin::Root.into(),
        network_id,
        keys.clone()
    ));
    keys
}

fn assert_last_event<T: Config>(generic_event: <T as frame_system::Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event);
}

benchmarks! {
    initialize {
        let a = <T as Config>::MaxPeers::get();
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);
        let keys = initial_keys::<T>(a as usize);
    }: _(RawOrigin::Root, network_id, keys)
    verify {
        assert_last_event::<T>(Event::NetworkInitialized(network_id).into())
    }

    add_peer {
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);

        let min_peers = T::MinPeers::get() as usize;
        initialize_network::<T>(network_id, min_peers);
        let key = generate_key(min_peers);
    }: {
        MultisigVerifier::<T>::add_peer(T::CallOrigin::try_successful_origin().unwrap(), key)?;
    }
    verify {
        assert!(MultisigVerifier::<T>::get_peer_keys(GenericNetworkId::Sub(SubNetworkId::Mainnet)).expect("add_peer: No key found").contains(&key));
    }

    remove_peer {
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);

        let peers = initialize_network::<T>(network_id, T::MinPeers::get() as usize + 1);
        let key = peers[0];
    }: {
        MultisigVerifier::<T>::remove_peer(T::CallOrigin::try_successful_origin().unwrap(), key)?;
    }
    verify {
        assert!(!MultisigVerifier::<T>::get_peer_keys(GenericNetworkId::Sub(SubNetworkId::Mainnet)).expect("add_peer: No key found").contains(&key));
    }

    force_set_peers {
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);
        let keys = initial_keys::<T>(<T as Config>::MaxPeers::get() as usize);
    }: _(RawOrigin::Root, network_id, keys.clone())
    verify {
        assert_last_event::<T>(Event::PeerSetForced(network_id, keys.len() as u32).into())
    }

    verifier_verify {
        let a in 4..50;
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);
        let peers = initialize_network::<T>(network_id, a as usize);
        let data = [3u8; 32];
        let digest = bridge_types::types::AuxiliaryDigest {
            logs: vec![
                bridge_types::types::AuxiliaryDigestItem::Commitment(network_id, data.into()),
                bridge_types::types::AuxiliaryDigestItem::Commitment(bridge_types::SubNetworkId::Rococo.into(), [4u8; 32].into()),
            ]
        };
        let digest_hash = Keccak256::hash_of(&digest);
        let signatures = peers.iter().map(|k| {
            sp_io::crypto::ecdsa_sign_prehashed(sp_core::crypto::key_types::DUMMY, k, &digest_hash.0).unwrap()
        }).collect::<Vec<_>>();
        let key = peers[0];
    }: {
        MultisigVerifier::<T>::verify(network_id, data.into(), &Proof {
            digest,
            proof: signatures,
        })?;
    }

    impl_benchmark_test_suite!(MultisigVerifier, crate::mock::new_test_ext(), mock::Test)
}
