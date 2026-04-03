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

use super::Call;
use crate::{mock::*, Error};
use bridge_types::{SubNetworkId, H256};
use frame_support::{assert_noop, assert_ok};
use sp_core::{
    bounded::BoundedVec,
    ecdsa::{self, Signature},
    Pair,
};
use sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
};

fn test_peers() -> (Vec<ecdsa::Public>, Vec<ecdsa::Pair>) {
    let pairs: Vec<ecdsa::Pair> = vec![
        ecdsa::Pair::generate_with_phrase(Some("password")),
        ecdsa::Pair::generate_with_phrase(Some("password1")),
        ecdsa::Pair::generate_with_phrase(Some("password2")),
        ecdsa::Pair::generate_with_phrase(Some("password3")),
        ecdsa::Pair::generate_with_phrase(Some("password4")),
        ecdsa::Pair::generate_with_phrase(Some("password5")),
    ]
    .into_iter()
    .map(|(x, _, _)| x)
    .collect();
    (pairs.clone().iter().map(|x| x.public()).collect(), pairs)
}

fn test_signer() -> ecdsa::Pair {
    ecdsa::Pair::generate_with_phrase(Some("something")).0
}

fn min_test_peers() -> Vec<ecdsa::Public> {
    test_peers()
        .0
        .into_iter()
        .take(BridgeMinPeers::get() as usize)
        .collect()
}

fn min_plus_one_test_peers() -> Vec<ecdsa::Public> {
    test_peers()
        .0
        .into_iter()
        .take(BridgeMinPeers::get() as usize + 1)
        .collect()
}

fn generated_public(seed: &str) -> ecdsa::Public {
    ecdsa::Pair::generate_with_phrase(Some(seed)).0.public()
}

fn open_pending_approval(network_id: bridge_types::GenericNetworkId, data: H256) {
    assert_ok!(DataSigner::register_pending_approval(
        RuntimeOrigin::root(),
        network_id,
        data,
    ));
}

#[test]
fn it_works_register_network() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = test_peers().0.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        assert!(DataSigner::peers(network_id).is_some());
        assert!(DataSigner::peers(network_id).unwrap().len() == peers.len());
    });
}

#[test]
fn it_fails_register_network_with_too_few_peers() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = vec![].try_into().unwrap();

        assert_noop!(
            DataSigner::register_network(RuntimeOrigin::root(), network_id, peers,),
            Error::<Test>::NotEnoughPeers
        );
        assert!(DataSigner::peers(network_id).is_none());
    });
}

#[test]
fn it_fails_register_network_when_deduped_peers_are_below_minimum() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers = min_test_peers();
        let duplicate_heavy: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            vec![peers[0], peers[1], peers[2], peers[0]]
                .try_into()
                .unwrap();

        assert_noop!(
            DataSigner::register_network(RuntimeOrigin::root(), network_id, duplicate_heavy,),
            Error::<Test>::NotEnoughPeers
        );
        assert!(DataSigner::peers(network_id).is_none());
    });
}

#[test]
fn it_fails_register_network_with_three_unique_peers() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = min_test_peers()
            .into_iter()
            .take(3)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        assert_noop!(
            DataSigner::register_network(RuntimeOrigin::root(), network_id, peers,),
            Error::<Test>::NotEnoughPeers
        );
        assert!(DataSigner::peers(network_id).is_none());
    });
}

#[test]
fn it_fails_register_network_alredy_initialized() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            test_peers().0.try_into().unwrap(),
        ));

        assert_noop!(
            DataSigner::register_network(
                RuntimeOrigin::root(),
                network_id,
                test_peers().0.try_into().unwrap(),
            ),
            Error::<Test>::PalletInitialized
        );
    });
}

#[test]
fn it_works_approve() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, pairs) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        let data = [1u8; 32];
        let signature = pairs[0].sign_prehashed(&data);
        assert!(DataSigner::peers(network_id).unwrap().contains(&peers[0]));
        assert!(DataSigner::peers(network_id)
            .unwrap()
            .contains(&pairs[0].public()));
        assert!(DataSigner::approvals(network_id, H256::from(data)).is_empty());
        open_pending_approval(network_id, H256::from(data));

        assert_ok!(DataSigner::approve(
            RuntimeOrigin::none(),
            network_id,
            H256::from(data),
            signature,
        ));

        assert!(DataSigner::approvals(network_id, H256::from(data)).len() == 1);
    });
}

#[test]
fn it_fails_approve_nonexisted_peer() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let data = [1u8; 32];
        let signature = test_signer().sign_prehashed(&data);
        assert!(DataSigner::approvals(network_id, H256::from(data)).is_empty());
        open_pending_approval(network_id, H256::from(data));

        assert_noop!(
            DataSigner::approve(
                RuntimeOrigin::none(),
                network_id,
                H256::from(data),
                signature,
            ),
            Error::<Test>::PeerNotFound
        );

        assert!(DataSigner::approvals(network_id, H256::from(data)).is_empty());
    });
}

#[test]
fn it_fails_approve_sign_already_exist() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, pairs) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let data = [1u8; 32];
        let signature = pairs[0].sign_prehashed(&data);
        assert!(DataSigner::approvals(network_id, H256::from(data)).is_empty());
        open_pending_approval(network_id, H256::from(data));

        assert_ok!(DataSigner::approve(
            RuntimeOrigin::none(),
            network_id,
            H256::from(data),
            signature.clone(),
        ));

        assert!(DataSigner::approvals(network_id, H256::from(data)).len() == 1);

        assert_noop!(
            DataSigner::approve(
                RuntimeOrigin::none(),
                network_id,
                H256::from(data),
                signature,
            ),
            Error::<Test>::SignatureAlreadyExists
        );

        assert!(DataSigner::approvals(network_id, H256::from(data)).len() == 1);
    });
}

#[test]
fn it_fails_approve_when_hash_is_not_pending() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, pairs) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let data = [1u8; 32];
        assert_noop!(
            DataSigner::approve(
                RuntimeOrigin::none(),
                network_id,
                H256::from(data),
                pairs[0].sign_prehashed(&data),
            ),
            Error::<Test>::ApprovalNotPending
        );
    });
}

#[test]
fn it_works_add_peer() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let new_peer = test_signer().public();
        assert_ok!(DataSigner::add_peer(
            RuntimeOrigin::root(),
            network_id,
            new_peer,
        ));

        assert!(DataSigner::pending_peer_update(network_id));
    });
}

#[test]
fn it_fails_register_network_when_substrate_peer_overlaps_another_network() {
    new_test_ext().execute_with(|| {
        let peers = test_peers().0;
        let mainnet: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            peers[..4].to_vec().try_into().unwrap();
        let kusama: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            peers[2..6].to_vec().try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
            mainnet,
        ));

        assert_noop!(
            DataSigner::register_network(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Kusama),
                kusama,
            ),
            Error::<Test>::PeerRegisteredInOtherNetwork
        );
    });
}

#[test]
fn it_fails_add_peer_when_substrate_peer_overlaps_another_network() {
    new_test_ext().execute_with(|| {
        let peers = test_peers().0;
        let mainnet: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            peers[..4].to_vec().try_into().unwrap();
        let kusama: BoundedVec<ecdsa::Public, BridgeMaxPeers> = vec![
            generated_public("kusama-peer-0"),
            generated_public("kusama-peer-1"),
            generated_public("kusama-peer-2"),
            generated_public("kusama-peer-3"),
        ]
        .try_into()
        .unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
            mainnet,
        ));
        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            bridge_types::GenericNetworkId::Sub(SubNetworkId::Kusama),
            kusama.clone(),
        ));

        assert_noop!(
            DataSigner::add_peer(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                kusama[0],
            ),
            Error::<Test>::PeerRegisteredInOtherNetwork
        );
    });
}

#[test]
fn it_works_register_and_clear_pending_approval() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = min_test_peers().try_into().unwrap();
        let data = H256::from([8u8; 32]);

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        assert_ok!(DataSigner::register_pending_approval(
            RuntimeOrigin::root(),
            network_id,
            data,
        ));
        assert!(DataSigner::pending_approval(network_id, data));

        assert_ok!(DataSigner::clear_pending_approval(
            RuntimeOrigin::root(),
            network_id,
            data,
        ));
        assert!(!DataSigner::pending_approval(network_id, data));
        assert!(DataSigner::approvals(network_id, data).is_empty());
    });
}

#[test]
fn it_fails_add_peer_pending_update() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let new_peer = test_signer().public();
        assert_ok!(DataSigner::add_peer(
            RuntimeOrigin::root(),
            network_id,
            new_peer,
        ));

        // cannot add another peer while pending peer update
        let new_peer = test_signer().public();
        assert_noop!(
            DataSigner::add_peer(RuntimeOrigin::root(), network_id, new_peer,),
            Error::<Test>::HasPendingPeerUpdate
        );

        assert!(DataSigner::pending_peer_update(network_id));
    });
}

#[test]
fn it_fails_add_peer_already_exists() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        let peer = peers[0];
        assert_noop!(
            DataSigner::add_peer(RuntimeOrigin::root(), network_id, peer,),
            Error::<Test>::PeerExists
        );

        assert!(!DataSigner::pending_peer_update(network_id));
    });
}

#[test]
fn it_fails_add_peer_evm_network_not_supported() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::EVM(H256::from_low_u64_be(1));
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let new_peer = test_signer().public();
        assert_noop!(
            DataSigner::add_peer(RuntimeOrigin::root(), network_id, new_peer,),
            Error::<Test>::NetworkNotSupported
        );
        assert!(!DataSigner::pending_peer_update(network_id));
        assert!(!DataSigner::peers(network_id).unwrap().contains(&new_peer));
    });
}

#[test]
fn it_rolls_back_add_peer_when_outbound_submit_fails() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = min_test_peers().try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let new_peer = test_signer().public();
        set_submit_should_fail(true);
        assert_noop!(
            DataSigner::add_peer(RuntimeOrigin::root(), network_id, new_peer,),
            sp_runtime::DispatchError::Other("mock submit failure")
        );
        set_submit_should_fail(false);

        assert!(!DataSigner::pending_peer_update(network_id));
        assert!(!DataSigner::peers(network_id).unwrap().contains(&new_peer));
    });
}

#[test]
fn it_works_remove_peer() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            min_plus_one_test_peers().try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        let peer = peers[0];
        assert_ok!(DataSigner::remove_peer(
            RuntimeOrigin::root(),
            network_id,
            peer,
        ));

        assert!(DataSigner::pending_peer_update(network_id));
    });
}

#[test]
fn it_fails_remove_peer_at_minimum_floor() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = min_test_peers().try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        assert_noop!(
            DataSigner::remove_peer(RuntimeOrigin::root(), network_id, peers[0],),
            Error::<Test>::NotEnoughPeers
        );
        assert!(!DataSigner::pending_peer_update(network_id));
    });
}

#[test]
fn it_fails_remove_peer_when_peer_is_not_in_set() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            min_plus_one_test_peers().try_into().unwrap();
        let unknown_peer = test_signer().public();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        assert_noop!(
            DataSigner::remove_peer(RuntimeOrigin::root(), network_id, unknown_peer,),
            Error::<Test>::PeerNotExists
        );
        assert!(!DataSigner::pending_peer_update(network_id));
        assert_eq!(DataSigner::peers(network_id).unwrap().len(), peers.len());
    });
}

#[test]
fn it_fails_remove_peer_when_peer_is_not_in_set_even_at_minimum_floor() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = min_test_peers().try_into().unwrap();
        let unknown_peer = test_signer().public();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        assert_noop!(
            DataSigner::remove_peer(RuntimeOrigin::root(), network_id, unknown_peer,),
            Error::<Test>::PeerNotExists
        );
        assert!(!DataSigner::pending_peer_update(network_id));
        assert_eq!(DataSigner::peers(network_id).unwrap().len(), peers.len());
    });
}

#[test]
fn it_fails_remove_peer_pending_update() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        let peer = peers[0];
        assert_ok!(DataSigner::remove_peer(
            RuntimeOrigin::root(),
            network_id,
            peer,
        ));

        // cannot remove another peer while pending peer update
        let peer = peers[1];
        assert_noop!(
            DataSigner::remove_peer(RuntimeOrigin::root(), network_id, peer,),
            Error::<Test>::HasPendingPeerUpdate
        );

        assert!(DataSigner::pending_peer_update(network_id));
    });
}

#[test]
fn it_fails_remove_peer_evm_network_not_supported() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::EVM(H256::from_low_u64_be(1));
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            min_plus_one_test_peers().try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        let peer = peers[0];
        assert_noop!(
            DataSigner::remove_peer(RuntimeOrigin::root(), network_id, peer,),
            Error::<Test>::NetworkNotSupported
        );
        assert!(!DataSigner::pending_peer_update(network_id));
    })
}

#[test]
fn it_rolls_back_remove_peer_when_outbound_submit_fails() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            min_plus_one_test_peers().try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        set_submit_should_fail(true);
        assert_noop!(
            DataSigner::remove_peer(RuntimeOrigin::root(), network_id, peers[0],),
            sp_runtime::DispatchError::Other("mock submit failure")
        );
        set_submit_should_fail(false);

        assert!(!DataSigner::pending_peer_update(network_id));
        assert_eq!(DataSigner::peers(network_id).unwrap().len(), peers.len());
    });
}

#[test]
fn it_works_finish_remove_peer() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            min_plus_one_test_peers().try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        let peer = peers[0];
        assert_ok!(DataSigner::remove_peer(
            RuntimeOrigin::root(),
            network_id,
            peer,
        ));

        assert!(DataSigner::pending_peer_update(network_id));

        assert_ok!(DataSigner::finish_remove_peer(RuntimeOrigin::root(), peer));

        assert!(!DataSigner::pending_peer_update(network_id));
        assert!(!DataSigner::peers(network_id).unwrap().contains(&peer));
    });
}

#[test]
fn it_fails_finish_remove_peer_at_minimum_floor() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = min_test_peers().try_into().unwrap();
        let peer = peers[0];

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));
        crate::pallet::PendingPeerUpdate::<Test>::insert(network_id, true);

        assert_noop!(
            DataSigner::finish_remove_peer(RuntimeOrigin::root(), peer),
            Error::<Test>::NotEnoughPeers
        );
        assert!(DataSigner::pending_peer_update(network_id));
        assert!(DataSigner::peers(network_id).unwrap().contains(&peer));
    });
}

#[test]
fn it_fails_finish_remove_peer_no_updates() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        let peer = peers[0];
        assert!(!DataSigner::pending_peer_update(network_id));

        assert_noop!(
            DataSigner::finish_remove_peer(RuntimeOrigin::root(), peer),
            Error::<Test>::DontHavePendingPeerUpdates
        );
    })
}

#[test]
fn it_fails_finish_remove_not_initialized() {
    new_test_ext().execute_with(|| {
        let peer = test_signer().public();

        assert_noop!(
            DataSigner::finish_remove_peer(RuntimeOrigin::root(), peer),
            Error::<Test>::DontHavePendingPeerUpdates
        );
    })
}

#[test]
fn it_fails_finish_remove_peer_not_exist() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        assert_ok!(DataSigner::remove_peer(
            RuntimeOrigin::root(),
            network_id,
            peers[0],
        ));

        assert!(DataSigner::pending_peer_update(network_id));
        let peer = test_signer().public();

        assert_noop!(
            DataSigner::finish_remove_peer(RuntimeOrigin::root(), peer),
            Error::<Test>::PeerNotExists
        );
    })
}

#[test]
fn it_works_finish_add_peer() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let new_peer = test_signer().public();
        assert_ok!(DataSigner::add_peer(
            RuntimeOrigin::root(),
            network_id,
            new_peer,
        ));

        assert!(DataSigner::pending_peer_update(network_id));

        assert_ok!(DataSigner::finish_add_peer(RuntimeOrigin::root(), new_peer));

        assert!(!DataSigner::pending_peer_update(network_id));
        assert!(DataSigner::peers(network_id).unwrap().contains(&new_peer));
    });
}

#[test]
fn it_fails_add_peer_no_pending_update() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let new_peer = test_signer().public();
        assert_noop!(
            DataSigner::finish_add_peer(RuntimeOrigin::root(), new_peer),
            Error::<Test>::DontHavePendingPeerUpdates
        );
    });
}

#[test]
fn it_works_force_set_peers_for_recovery() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = min_test_peers().try_into().unwrap();
        let recovery_peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            vec![test_signer().public()].try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));
        crate::pallet::PendingPeerUpdate::<Test>::insert(network_id, true);

        assert_ok!(DataSigner::force_set_peers(
            RuntimeOrigin::root(),
            network_id,
            recovery_peers.clone(),
        ));

        assert!(!DataSigner::pending_peer_update(network_id));
        let stored_peers = DataSigner::peers(network_id).unwrap();
        assert_eq!(stored_peers.len(), recovery_peers.len());
        for peer in recovery_peers {
            assert!(stored_peers.contains(&peer));
        }
    });
}

#[test]
fn it_fails_force_set_peers_when_substrate_peer_overlaps_another_network() {
    new_test_ext().execute_with(|| {
        let peers = test_peers().0;
        let mainnet: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            peers[..4].to_vec().try_into().unwrap();
        let kusama: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            peers[2..6].to_vec().try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
            mainnet,
        ));

        assert_noop!(
            DataSigner::force_set_peers(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Kusama),
                kusama,
            ),
            Error::<Test>::PeerRegisteredInOtherNetwork
        );
    });
}

#[test]
fn it_prunes_stale_approvals_after_force_set_peers() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, pairs) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers
            .into_iter()
            .take(4)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let data = H256::from([7u8; 32]);

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));
        open_pending_approval(network_id, data);

        assert_ok!(DataSigner::approve(
            RuntimeOrigin::none(),
            network_id,
            data,
            pairs[0].sign_prehashed(&data.0),
        ));
        assert_ok!(DataSigner::approve(
            RuntimeOrigin::none(),
            network_id,
            data,
            pairs[1].sign_prehashed(&data.0),
        ));
        assert_eq!(DataSigner::approvals(network_id, data).len(), 2);

        let rotated_peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            vec![pairs[1].public(), pairs[2].public(), pairs[3].public()]
                .try_into()
                .unwrap();
        assert_ok!(DataSigner::force_set_peers(
            RuntimeOrigin::root(),
            network_id,
            rotated_peers,
        ));

        assert_ok!(DataSigner::approve(
            RuntimeOrigin::none(),
            network_id,
            data,
            pairs[2].sign_prehashed(&data.0),
        ));

        let approvals = DataSigner::approvals(network_id, data);
        assert_eq!(approvals.len(), 2);
        assert!(!approvals.contains_key(&pairs[0].public()));
        assert!(approvals.contains_key(&pairs[1].public()));
        assert!(approvals.contains_key(&pairs[2].public()));
    });
}

#[test]
fn it_clears_pending_approval_and_stored_signatures() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, pairs) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();
        let data = H256::from([6u8; 32]);

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));
        open_pending_approval(network_id, data);
        assert_ok!(DataSigner::approve(
            RuntimeOrigin::none(),
            network_id,
            data,
            pairs[0].sign_prehashed(&data.0),
        ));

        assert_ok!(DataSigner::clear_pending_approval(
            RuntimeOrigin::root(),
            network_id,
            data,
        ));

        assert!(!DataSigner::pending_approval(network_id, data));
        assert!(DataSigner::approvals(network_id, data).is_empty());
    });
}

#[test]
fn it_works_validate_unsigned() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, pairs) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        let data = [1u8; 32];
        open_pending_approval(network_id, H256::from(data));
        let signature = pairs[0].sign_prehashed(&data);

        let call = Call::approve {
            network_id,
            data: H256::from(data),
            signature,
        };

        assert_eq!(
            <DataSigner as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::External,
                &call,
            ),
            TransactionValidity::Ok(
                ValidTransaction::with_tag_prefix("DataSignerApprove")
                    .priority(TestUnsignedPriority::get())
                    .longevity(TestUnsignedLongevity::get())
                    .and_provides((network_id, H256::from(data), peers[0]))
                    .propagate(true)
                    .build()
                    .unwrap()
            )
        );
    });
}

#[test]
fn it_fails_validate_unsigned_no_network() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, pairs) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let different_network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Kusama);

        let data = [1u8; 32];
        let signature = pairs[0].sign_prehashed(&data);

        let call = Call::approve {
            network_id: different_network_id,
            data: H256::from(data),
            signature,
        };

        assert_eq!(
            <DataSigner as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::External,
                &call,
            ),
            InvalidTransaction::BadSigner.into()
        );
    })
}

#[test]
fn it_fails_validate_unsigned_when_hash_is_not_pending() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, pairs) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let data = [1u8; 32];
        let call = Call::approve {
            network_id,
            data: H256::from(data),
            signature: pairs[0].sign_prehashed(&data),
        };

        assert_eq!(
            <DataSigner as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::External,
                &call,
            ),
            InvalidTransaction::Stale.into()
        );
    })
}

#[test]
fn it_fails_validate_unsigned_bad_proof() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let data = [1u8; 32];
        open_pending_approval(network_id, H256::from(data));
        let signature = Signature::from_raw([3u8; 65]);

        let call = Call::approve {
            network_id,
            data: H256::from(data),
            signature,
        };

        assert_eq!(
            <DataSigner as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::External,
                &call,
            ),
            InvalidTransaction::BadProof.into()
        );
    })
}

#[test]
fn it_fails_validate_unsigned_bad_signer() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let data = [1u8; 32];
        open_pending_approval(network_id, H256::from(data));
        let signature = test_signer().sign_prehashed(&data);

        let call = Call::approve {
            network_id,
            data: H256::from(data),
            signature,
        };

        assert_eq!(
            <DataSigner as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::External,
                &call,
            ),
            InvalidTransaction::BadSigner.into()
        );
    })
}

#[test]
fn it_fails_validate_unsigned_transaction_stale() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, pairs) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers,
        ));

        let data = [1u8; 32];
        let signature = pairs[0].sign_prehashed(&data);
        open_pending_approval(network_id, H256::from(data));

        assert_ok!(DataSigner::approve(
            RuntimeOrigin::none(),
            network_id,
            H256::from(data),
            signature.clone(),
        ));

        assert!(DataSigner::approvals(network_id, H256::from(data)).len() == 1);

        let call = Call::approve {
            network_id,
            data: H256::from(data),
            signature,
        };

        assert_eq!(
            <DataSigner as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::InBlock,
                &call,
            ),
            InvalidTransaction::Stale.into()
        );
    })
}

#[test]
fn it_fails_validate_unsigned_invalid_call() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let (peers, _) = test_peers();
        let peers: BoundedVec<ecdsa::Public, BridgeMaxPeers> = peers.try_into().unwrap();

        assert_ok!(DataSigner::register_network(
            RuntimeOrigin::root(),
            network_id,
            peers.clone(),
        ));

        let call = Call::register_network { network_id, peers };

        assert_eq!(
            <DataSigner as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(
                TransactionSource::External,
                &call,
            ),
            InvalidTransaction::Call.into()
        );
    })
}

#[test]
fn testing_signer() {
    let (peers, pairs) = test_peers();

    assert_eq!(peers[0], pairs[0].public());
}
