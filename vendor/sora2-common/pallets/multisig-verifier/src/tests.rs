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
            sp_runtime::DispatchError::BadOrigin
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

#[test]
fn authenticated_digest_requires_exactly_one_matching_substrate_destination() {
    use bridge_types::GenericNetworkId;
    new_test_ext().execute_with(|| {
        let network = GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let hash = sp_core::H256::repeat_byte(42);
        let pairs = test_pairs();
        assert_ok!(TrustedVerifier::initialize(
            RuntimeOrigin::root(),
            network,
            test_peers().try_into().unwrap()
        ));
        let matching = AuxiliaryDigestItem::Commitment(network, hash);
        for (logs, expected) in [
            (vec![], false),
            (vec![matching, matching], false),
            (
                vec![AuxiliaryDigestItem::Commitment(
                    SubNetworkId::Kusama.into(),
                    hash,
                )],
                false,
            ),
            (
                vec![AuxiliaryDigestItem::Commitment(
                    network,
                    sp_core::H256::zero(),
                )],
                false,
            ),
            (
                vec![AuxiliaryDigestItem::Commitment(
                    GenericNetworkId::EVMLegacy(1),
                    hash,
                )],
                false,
            ),
            (
                vec![
                    matching,
                    AuxiliaryDigestItem::Commitment(GenericNetworkId::EVMLegacy(1), hash),
                ],
                true,
            ),
        ] {
            let digest = AuxiliaryDigest { logs };
            let digest_hash = Keccak256::hash_of(&digest);
            let proof = crate::Proof {
                digest,
                proof: pairs
                    .iter()
                    .map(|pair| pair.sign_prehashed(&digest_hash.0))
                    .collect(),
            };
            if expected {
                assert_ok!(TrustedVerifier::verify(network, hash, &proof));
            } else {
                assert_eq!(
                    TrustedVerifier::verify(network, hash, &proof),
                    Err(Error::<Test>::CommitmentNotFoundInDigest.into())
                );
            }
            assert_eq!(
                TrustedVerifier::verify_weight(&proof),
                <() as crate::WeightInfo>::verifier_verify(5)
            );
        }
    });
}

#[test]
fn evm_signatures_bind_source_destination_and_commitment() {
    use bridge_types::GenericNetworkId;
    new_test_ext().execute_with(|| {
        let network = GenericNetworkId::EVM([1u8; 32].into());
        let hash = sp_core::H256::repeat_byte(42);
        let pairs = test_pairs();
        let approved = Keccak256::hash_of(&(network, ThisNetworkId::get(), hash));
        let proof = crate::MultiEVMProof {
            proof: pairs
                .iter()
                .map(|pair| pair.sign_prehashed(&approved.0))
                .collect(),
        };
        assert_noop!(
            crate::MultiEVMVerifier::<Test>::verify(network, hash, &proof),
            Error::<Test>::NetworkNotInitialized
        );
        assert_ok!(TrustedVerifier::initialize(
            RuntimeOrigin::root(),
            network,
            test_peers().try_into().unwrap()
        ));
        assert_ok!(crate::MultiEVMVerifier::<Test>::verify(
            network, hash, &proof
        ));
        assert_noop!(
            crate::MultiEVMVerifier::<Test>::verify(network, sp_core::H256::zero(), &proof),
            Error::<Test>::NotTrustedPeerSignature
        );
        assert_eq!(
            crate::MultiEVMVerifier::<Test>::verify_weight(&proof),
            <() as crate::WeightInfo>::verifier_verify(5)
        );
        assert_noop!(
            TrustedVerifier::verify_signatures(
                network,
                hash,
                &[ecdsa::Signature::from_raw([0; 65])]
            ),
            Error::<Test>::InvalidSignature
        );
    });
}

#[test]
fn recovery_peer_acknowledgements_require_live_context_and_cannot_be_replayed() {
    use codec::Encode;
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        let network = SubNetworkId::Mainnet;
        assert_ok!(TrustedVerifier::initialize(
            RuntimeOrigin::root(),
            network.into(),
            test_peers().try_into().unwrap()
        ));
        let added = ecdsa::Pair::from_seed(&[9; 32]).public();
        sp_io::storage::set(b":sora-nexus:recovery:bridge-halt:v1", &[1]);
        assert_noop!(
            TrustedVerifier::add_peer(RuntimeOrigin::signed(1), added),
            Error::<Test>::RecoveryHalted
        );
        sp_io::storage::set(b":sora-nexus:recovery:incoming-active:v1", &[1]);
        sp_io::storage::set(
            b":sora-nexus:recovery:bridge-inbound-context:v1",
            &9u64.encode(),
        );
        assert_noop!(
            TrustedVerifier::add_peer(RuntimeOrigin::signed(1), added),
            Error::<Test>::RecoveryHalted
        );
        sp_io::storage::set(
            b":sora-nexus:recovery:bridge-inbound-context:v1",
            &10u64.encode(),
        );
        assert_ok!(TrustedVerifier::add_peer(RuntimeOrigin::signed(1), added));
        let key = crate::recovery_peer_ack_key(network, [1; 32].into());
        assert_eq!(
            sp_io::storage::get(&key).unwrap().to_vec(),
            bridge_types::substrate::DataSignerCall::AddPeer { peer: added }.encode()
        );
        assert_noop!(
            TrustedVerifier::remove_peer(RuntimeOrigin::signed(1), added),
            Error::<Test>::RecoveryAcknowledgementExists
        );
        assert!(
            TrustedVerifier::get_peer_keys(bridge_types::GenericNetworkId::Sub(network))
                .unwrap()
                .contains(&added)
        );
        assert_ne!(
            key,
            crate::recovery_peer_ack_key(SubNetworkId::Kusama, [1; 32].into())
        );
    });
}

#[test]
fn peer_bounds_and_cross_network_audit_reject_invalid_memberships() {
    use bridge_types::GenericNetworkId;
    new_test_ext().execute_with(|| {
        let network = GenericNetworkId::Sub(SubNetworkId::Mainnet);
        assert_ok!(TrustedVerifier::initialize(
            RuntimeOrigin::root(),
            network,
            test_peers().try_into().unwrap()
        ));
        assert_ok!(TrustedVerifier::ensure_unique_substrate_peers());
        assert_noop!(
            TrustedVerifier::add_peer(RuntimeOrigin::signed(1), test_peers()[0]),
            Error::<Test>::PeerExists
        );
        assert_noop!(
            TrustedVerifier::remove_peer(
                RuntimeOrigin::signed(1),
                ecdsa::Pair::from_seed(&[9; 32]).public()
            ),
            Error::<Test>::NoSuchPeer
        );
        let peers = TrustedVerifier::get_peer_keys(network).unwrap();
        crate::PeerKeys::<Test>::insert(GenericNetworkId::EVM([1u8; 32].into()), peers.clone());
        assert_ok!(TrustedVerifier::ensure_unique_substrate_peers());
        crate::PeerKeys::<Test>::insert(GenericNetworkId::Sub(SubNetworkId::Kusama), peers);
        assert_noop!(
            TrustedVerifier::ensure_unique_substrate_peers(),
            Error::<Test>::PeerRegisteredInOtherNetwork
        );
    });
    new_test_ext().execute_with(|| {
        let peers: Vec<_> = (0..BridgeMaxPeers::get())
            .map(|i| ecdsa::Pair::from_seed(&[i as u8 + 1; 32]).public())
            .collect();
        assert_ok!(TrustedVerifier::initialize(
            RuntimeOrigin::root(),
            SubNetworkId::Mainnet.into(),
            peers.try_into().unwrap()
        ));
        assert_noop!(
            TrustedVerifier::add_peer(
                RuntimeOrigin::signed(1),
                ecdsa::Pair::from_seed(&[200; 32]).public()
            ),
            Error::<Test>::TooMuchPeers
        );
    });
}

#[test]
fn bridge_peer_messages_convert_to_the_correct_dispatchables() {
    use bridge_types::substrate::MultisigVerifierCall;
    let peer = ecdsa::Pair::from_seed(&[1; 32]).public();
    assert!(
        matches!(crate::Call::<Test>::from(MultisigVerifierCall::AddPeer { peer }), crate::Call::add_peer { peer: actual } if actual == peer)
    );
    assert!(
        matches!(crate::Call::<Test>::from(MultisigVerifierCall::RemovePeer { peer }), crate::Call::remove_peer { peer: actual } if actual == peer)
    );
}
