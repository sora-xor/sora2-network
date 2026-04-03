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

use crate::{mock::*, Error};
use bridge_types::{
    traits::Verifier,
    types::{AuxiliaryDigest, AuxiliaryDigestItem},
    SubNetworkId,
};

use codec::Decode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use sp_core::{ecdsa, Pair};
use sp_runtime::traits::{Hash, Keccak256};

fn alice<T: crate::Config>() -> T::AccountId {
    T::AccountId::decode(&mut [0u8; 32].as_slice()).unwrap()
}

fn test_pairs() -> Vec<ecdsa::Pair> {
    [
        Keccak256::hash_of(&"Password0").0,
        Keccak256::hash_of(&"Password1").0,
        Keccak256::hash_of(&"Password2").0,
        Keccak256::hash_of(&"Password3").0,
        Keccak256::hash_of(&"Password4").0,
    ]
    .into_iter()
    .map(|x| ecdsa::Pair::from_seed(&x))
    .collect()
}

fn test_peers() -> Vec<ecdsa::Public> {
    test_pairs().into_iter().map(|x| x.public()).collect()
}

fn min_test_peers() -> Vec<ecdsa::Public> {
    test_peers()
        .into_iter()
        .take(BridgeMinPeers::get() as usize)
        .collect()
}

fn min_plus_one_test_peers() -> Vec<ecdsa::Public> {
    test_peers()
        .into_iter()
        .take(BridgeMinPeers::get() as usize + 1)
        .collect()
}

fn generated_public(seed: &str) -> ecdsa::Public {
    ecdsa::Pair::generate_with_phrase(Some(seed)).0.public()
}

#[test]
fn it_works_initialize_pallet() {
    new_test_ext().execute_with(|| {
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                test_peers().try_into().unwrap(),
            ),
            ().into()
        )
    });
}

#[test]
fn it_fails_initialize_pallet_not_root() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            TrustedVerifier::initialize(
                RuntimeOrigin::signed(1),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                test_peers().try_into().unwrap(),
            ),
            frame_support::error::BadOrigin
        );
    });
}

#[test]
fn it_fails_initialize_pallet_empty_signatures() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                vec![].try_into().unwrap(),
            ),
            Error::<Test>::NotEnoughPeers
        );
    });
}

#[test]
fn it_fails_initialize_pallet_with_three_unique_peers() {
    new_test_ext().execute_with(|| {
        let peers = min_test_peers();
        assert_noop!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                peers
                    .into_iter()
                    .take(3)
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            ),
            Error::<Test>::NotEnoughPeers
        );
    });
}

#[test]
fn it_fails_initialize_pallet_when_deduped_peers_are_below_minimum() {
    new_test_ext().execute_with(|| {
        let peers = min_test_peers();
        assert_noop!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                vec![peers[0], peers[1], peers[2], peers[0]]
                    .try_into()
                    .unwrap(),
            ),
            Error::<Test>::NotEnoughPeers
        );
    });
}

#[test]
fn it_works_add_peer() {
    new_test_ext().execute_with(|| {
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                test_peers().try_into().unwrap(),
            ),
            ().into()
        );

        let key = ecdsa::Public::from_raw([
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            1, 1, 1, 6,
        ]);

        assert_ok!(
            TrustedVerifier::add_peer(RuntimeOrigin::signed(alice::<Test>()), key,),
            ().into()
        );

        assert!(
            TrustedVerifier::get_peer_keys(bridge_types::GenericNetworkId::Sub(
                SubNetworkId::Mainnet,
            ))
            .expect("it_works_add_peer: error reading pallet storage")
            .contains(&key)
        );
    });
}

#[test]
fn it_fails_initialize_when_substrate_peer_overlaps_another_network() {
    new_test_ext().execute_with(|| {
        let peers = test_peers();
        let mainnet: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            peers[..4].to_vec().try_into().unwrap();
        let kusama: BoundedVec<ecdsa::Public, BridgeMaxPeers> = vec![
            peers[2],
            peers[3],
            generated_public("kusama-verifier-0"),
            generated_public("kusama-verifier-1"),
        ]
        .try_into()
        .unwrap();

        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                mainnet,
            ),
            ().into()
        );

        assert_noop!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Kusama),
                kusama,
            ),
            Error::<Test>::PeerRegisteredInOtherNetwork
        );
    });
}

#[test]
fn it_fails_add_peer_not_initialized() {
    new_test_ext().execute_with(|| {
        let key = ecdsa::Public::from_raw([
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            1, 1, 1, 6,
        ]);

        assert_noop!(
            TrustedVerifier::add_peer(RuntimeOrigin::signed(alice::<Test>()), key,),
            Error::<Test>::NetworkNotInitialized
        );
    });
}

#[test]
fn it_rolls_back_add_peer_when_outbound_submit_fails() {
    new_test_ext().execute_with(|| {
        let peers = min_test_peers();
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                peers.clone().try_into().unwrap(),
            ),
            ().into()
        );

        let key = ecdsa::Public::from_raw([
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            1, 1, 1, 6,
        ]);

        set_submit_should_fail(true);
        assert_noop!(
            TrustedVerifier::add_peer(RuntimeOrigin::signed(alice::<Test>()), key,),
            sp_runtime::DispatchError::Other("mock submit failure")
        );
        set_submit_should_fail(false);

        let stored = TrustedVerifier::get_peer_keys(bridge_types::GenericNetworkId::Sub(
            SubNetworkId::Mainnet,
        ))
        .expect("network should remain initialized");
        assert!(!stored.contains(&key));
        assert_eq!(stored.len(), peers.len());
    });
}

#[test]
fn it_fails_add_peer_when_substrate_peer_overlaps_another_network() {
    new_test_ext().execute_with(|| {
        let mainnet = min_test_peers();
        let kusama: BoundedVec<ecdsa::Public, BridgeMaxPeers> = vec![
            generated_public("kusama-verifier-2"),
            generated_public("kusama-verifier-3"),
            generated_public("kusama-verifier-4"),
            generated_public("kusama-verifier-5"),
        ]
        .try_into()
        .unwrap();

        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                mainnet.clone().try_into().unwrap(),
            ),
            ().into()
        );
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Kusama),
                kusama.clone(),
            ),
            ().into()
        );

        assert_noop!(
            TrustedVerifier::add_peer(RuntimeOrigin::signed(alice::<Test>()), kusama[0],),
            Error::<Test>::PeerRegisteredInOtherNetwork
        );
    });
}

#[test]
fn it_works_delete_peer() {
    new_test_ext().execute_with(|| {
        let peers = min_plus_one_test_peers();
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                peers.clone().try_into().unwrap(),
            ),
            ().into()
        );

        let key = *peers.last().unwrap();

        assert_ok!(
            TrustedVerifier::remove_peer(RuntimeOrigin::signed(alice::<Test>()), key,),
            ().into()
        );

        // check if already deleted
        assert_noop!(
            TrustedVerifier::remove_peer(RuntimeOrigin::signed(alice::<Test>()), key,),
            Error::<Test>::NoSuchPeer
        );

        assert!(
            !TrustedVerifier::get_peer_keys(bridge_types::GenericNetworkId::Sub(
                SubNetworkId::Mainnet,
            ))
            .expect("it_works_add_peer: error reading pallet storage")
            .contains(&key)
        );
    });
}

#[test]
fn it_fails_delete_peer_at_minimum_floor() {
    new_test_ext().execute_with(|| {
        let peers = min_test_peers();
        let key = peers[0];
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                peers.try_into().unwrap(),
            ),
            ().into()
        );

        assert_noop!(
            TrustedVerifier::remove_peer(RuntimeOrigin::signed(alice::<Test>()), key,),
            Error::<Test>::NotEnoughPeers
        );
    });
}

#[test]
fn it_fails_delete_peer_not_initialized() {
    new_test_ext().execute_with(|| {
        let key = *test_peers().last().unwrap();

        assert_noop!(
            TrustedVerifier::remove_peer(RuntimeOrigin::signed(alice::<Test>()), key,),
            Error::<Test>::NetworkNotInitialized
        );
    });
}

#[test]
fn it_rolls_back_remove_peer_when_outbound_submit_fails() {
    new_test_ext().execute_with(|| {
        let peers = min_plus_one_test_peers();
        let key = peers[0];
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                peers.clone().try_into().unwrap(),
            ),
            ().into()
        );

        set_submit_should_fail(true);
        assert_noop!(
            TrustedVerifier::remove_peer(RuntimeOrigin::signed(alice::<Test>()), key,),
            sp_runtime::DispatchError::Other("mock submit failure")
        );
        set_submit_should_fail(false);

        let stored = TrustedVerifier::get_peer_keys(bridge_types::GenericNetworkId::Sub(
            SubNetworkId::Mainnet,
        ))
        .expect("network should remain initialized");
        assert!(stored.contains(&key));
        assert_eq!(stored.len(), peers.len());
    });
}

#[test]
fn it_works_force_set_peers_for_recovery() {
    new_test_ext().execute_with(|| {
        let network_id = bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let recovery_peers = vec![test_peers()[0]];

        assert_ok!(
            TrustedVerifier::force_set_peers(
                RuntimeOrigin::root(),
                network_id,
                recovery_peers.clone().try_into().unwrap(),
            ),
            ().into()
        );

        let stored = TrustedVerifier::get_peer_keys(network_id).expect("peer set should exist");
        assert_eq!(stored.len(), recovery_peers.len());
        assert!(stored.contains(&recovery_peers[0]));
    });
}

#[test]
fn it_fails_force_set_peers_when_substrate_peer_overlaps_another_network() {
    new_test_ext().execute_with(|| {
        let peers = test_peers();
        let mainnet: BoundedVec<ecdsa::Public, BridgeMaxPeers> =
            peers[..4].to_vec().try_into().unwrap();
        let kusama: BoundedVec<ecdsa::Public, BridgeMaxPeers> = vec![
            peers[2],
            peers[3],
            generated_public("kusama-verifier-6"),
            generated_public("kusama-verifier-7"),
        ]
        .try_into()
        .unwrap();

        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                mainnet,
            ),
            ().into()
        );

        assert_noop!(
            TrustedVerifier::force_set_peers(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Kusama),
                kusama,
            ),
            Error::<Test>::PeerRegisteredInOtherNetwork
        );
    });
}

#[test]
fn it_works_verify_signatures() {
    new_test_ext().execute_with(|| {
        let pairs = test_pairs();
        let peers: Vec<ecdsa::Public> = pairs.clone().into_iter().map(|x| x.public()).collect();
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                peers.try_into().unwrap(),
            ),
            ().into()
        );

        let hash = Keccak256::hash_of(&"");
        let signatures: Vec<ecdsa::Signature> = pairs
            .into_iter()
            .map(|x| x.sign_prehashed(&hash.0))
            .collect();

        assert_ok!(TrustedVerifier::verify_signatures(
            bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
            hash,
            &signatures,
        ));
    });
}

#[test]
fn it_fails_verify_dublicated_signatures() {
    new_test_ext().execute_with(|| {
        let pairs = test_pairs();
        let peers: Vec<ecdsa::Public> = pairs.into_iter().map(|x| x.public()).collect();
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                peers.try_into().unwrap(),
            ),
            ().into()
        );

        let hash = Keccak256::hash_of(&"");
        let signatures: Vec<ecdsa::Signature> = vec![
            Keccak256::hash_of(&"Password0").0,
            Keccak256::hash_of(&"Password0").0,
            Keccak256::hash_of(&"Password1").0,
            Keccak256::hash_of(&"Password2").0,
        ]
        .into_iter()
        .map(|x| ecdsa::Pair::from_seed(&x).sign_prehashed(&hash.0))
        .collect();

        assert_noop!(
            TrustedVerifier::verify_signatures(
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                hash,
                &signatures,
            ),
            Error::<Test>::DuplicatedPeer
        );
    });
}

#[test]
fn it_fails_verify_not_enough_signatures() {
    new_test_ext().execute_with(|| {
        let pairs = test_pairs();
        let peers: Vec<ecdsa::Public> = pairs.into_iter().map(|x| x.public()).collect();
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                peers.try_into().unwrap(),
            ),
            ().into()
        );

        let hash = Keccak256::hash_of(&"");
        let signatures: Vec<ecdsa::Signature> = vec![
            Keccak256::hash_of(&"Password0").0,
            Keccak256::hash_of(&"Password1").0,
            Keccak256::hash_of(&"Password2").0,
        ]
        .into_iter()
        .map(|x| ecdsa::Pair::from_seed(&x).sign_prehashed(&hash.0))
        .collect();

        assert_noop!(
            TrustedVerifier::verify_signatures(
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                hash,
                &signatures,
            ),
            Error::<Test>::InvalidNumberOfSignatures
        );
    });
}

#[test]
fn it_fails_verify_invalid_signature() {
    new_test_ext().execute_with(|| {
        let pairs = test_pairs();
        let peers: Vec<ecdsa::Public> = pairs.into_iter().map(|x| x.public()).collect();
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                peers.try_into().unwrap(),
            ),
            ().into()
        );

        let hash = Keccak256::hash_of(&"");
        let signatures: Vec<ecdsa::Signature> = vec![
            Keccak256::hash_of(&"IvalidPassword0").0,
            Keccak256::hash_of(&"Password1").0,
            Keccak256::hash_of(&"Password2").0,
            Keccak256::hash_of(&"Password3").0,
            Keccak256::hash_of(&"Password4").0,
        ]
        .into_iter()
        .map(|x| ecdsa::Pair::from_seed(&x).sign_prehashed(&hash.0))
        .collect();

        assert_noop!(
            TrustedVerifier::verify_signatures(
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                hash,
                &signatures,
            ),
            Error::<Test>::NotTrustedPeerSignature
        );
    });
}

#[test]
fn it_works_verify() {
    new_test_ext().execute_with(|| {
        let pairs = test_pairs();
        let peers: Vec<ecdsa::Public> = pairs.clone().into_iter().map(|x| x.public()).collect();
        assert_ok!(
            TrustedVerifier::initialize(
                RuntimeOrigin::root(),
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                peers.try_into().unwrap(),
            ),
            ().into()
        );

        let hash = Keccak256::hash_of(&"");
        let signatures: Vec<ecdsa::Signature> = pairs
            .into_iter()
            .map(|x| x.sign_prehashed(&hash.0))
            .collect();

        let proof = crate::Proof {
            digest: AuxiliaryDigest {
                logs: vec![AuxiliaryDigestItem::Commitment(
                    bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                    hash,
                )],
            },
            proof: signatures,
        };

        assert_noop!(
            TrustedVerifier::verify(
                bridge_types::GenericNetworkId::Sub(SubNetworkId::Mainnet),
                hash,
                &proof,
            ),
            Error::<Test>::NotTrustedPeerSignature
        );
    });
}
