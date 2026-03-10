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
use crate::Pallet as BridgeDataSigner;
use bridge_types::GenericNetworkId;
use core::fmt::Write;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_support::traits::EnsureOrigin;
use frame_system::{self, RawOrigin};
use sp_core::bounded::BoundedBTreeMap;
use sp_core::{bounded::BoundedVec, ecdsa, Get, H256};
use sp_std::prelude::*;

fn initial_peers<T: Config>(n: usize) -> BoundedVec<ecdsa::Public, <T as Config>::MaxPeers> {
    let mut keys = Vec::new();
    for i in 0..n {
        let key = generate_key(i);
        keys.push(key);
    }

    keys.try_into().unwrap()
}

fn generate_key(i: usize) -> ecdsa::Public {
    let mut seed = sp_std::Writer::default();
    core::write!(seed, "//TestPeer//p-{}", i).unwrap();
    sp_io::crypto::ecdsa_generate(sp_core::crypto::key_types::DUMMY, Some(seed.into_inner()))
}

fn initialize_network<T: Config>(
    network_id: GenericNetworkId,
    n: usize,
) -> BoundedVec<ecdsa::Public, <T as Config>::MaxPeers> {
    let keys = initial_peers::<T>(n);
    assert_ok!(BridgeDataSigner::<T>::register_network(
        RawOrigin::Root.into(),
        network_id,
        keys.clone()
    ));
    keys
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
    register_network {
        let n = <T as Config>::MaxPeers::get();
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);
        let peers = initial_peers::<T>(n as usize);
    }: _(RawOrigin::Root, network_id, peers.clone())
    verify {
        assert_last_event::<T>(Event::Initialized{network_id, peers}.into());
    }

    add_peer {
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);

        initialize_network::<T>(network_id, 3);
        let key = generate_key(3);
    }: _(RawOrigin::Root, network_id, key)
    verify {
        assert!(PendingPeerUpdate::<T>::get(network_id));
    }

    remove_peer {
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);

        let peers = initialize_network::<T>(network_id, 3);
        let key = peers[0];
    }: _(RawOrigin::Root, network_id, key)
    verify {
        assert!(PendingPeerUpdate::<T>::get(network_id));
    }

    finish_add_peer {
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);

        let peers = initialize_network::<T>(network_id, 3);
        let key = generate_key(3);
        BridgeDataSigner::<T>::add_peer(RawOrigin::Root.into(), network_id, key).expect("remove_peer: Error adding peer");
    }: {
        BridgeDataSigner::<T>::finish_add_peer(T::CallOrigin::try_successful_origin().unwrap(), key)?;
    }
    verify {
        assert!(!PendingPeerUpdate::<T>::get(network_id));
        assert!(BridgeDataSigner::<T>::peers(network_id).expect("add_peer: key found").contains(&key));
    }

    finish_remove_peer {
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);

        let peers = initialize_network::<T>(network_id, 3);
        let key = peers[0];
        BridgeDataSigner::<T>::remove_peer(RawOrigin::Root.into(), network_id, key).expect("remove_peer: Error removing peer");
    }: {
        BridgeDataSigner::<T>::finish_remove_peer(T::CallOrigin::try_successful_origin().unwrap(), key)?;
    }
    verify {
        assert!(!PendingPeerUpdate::<T>::get(network_id));
        assert!(!BridgeDataSigner::<T>::peers(network_id).expect("remove_peer: No key found").contains(&key));
    }

    approve {
        let network_id = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);
        let peers = initialize_network::<T>(network_id, 3);
        let key = peers[0];
        let data = [3u8; 32];
        let signature = sp_io::crypto::ecdsa_sign_prehashed(sp_core::crypto::key_types::DUMMY, &key, &data).unwrap();
        let mut expected = BoundedBTreeMap::<ecdsa::Public, ecdsa::Signature, T::MaxPeers>::new();
        expected.try_insert(key, signature.clone()).unwrap();
    }: _(RawOrigin::None, network_id, data.into(), signature)
    verify {
        assert_eq!(Approvals::<T>::get(network_id, H256::from(data)), expected);
    }

    impl_benchmark_test_suite!(BridgeDataSigner, crate::mock::new_test_ext(), mock::Test)
}
