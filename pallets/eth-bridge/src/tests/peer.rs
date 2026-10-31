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

use super::mock::*;
use super::Error;
use crate::requests::RequestStatus;
use crate::requests::{
    ChangePeersContract, IncomingChangePeersCompat, IncomingRequest,
    IncomingTransactionRequestKind, LoadIncomingTransactionRequest,
};
use crate::tests::mock::{get_account_id_from_seed, ExtBuilder};
use crate::tests::{
    approve_next_request, approve_request, assert_incoming_request_done,
    assert_incoming_request_registration_failed, request_incoming, ETH_NETWORK_ID,
};
use crate::types::{Bytes, Transaction};
use crate::{types, EthAddress};
use common::eth;
use frame_support::sp_runtime::app_crypto::sp_core::{self, ecdsa, sr25519, Pair};
use frame_support::sp_runtime::traits::IdentifyAccount;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::Currency;
use frame_support::{assert_err, assert_ok};
use hex_literal::hex;
use secp256k1::{PublicKey, SecretKey};
use sp_core::{H160, H256};

fn canonical_peer_pair_fixture(
    nonce: u64,
) -> (
    crate::requests::OffchainRequest<Runtime>,
    crate::requests::OutgoingAddPeerCompat<Runtime>,
    crate::requests::OffchainRequest<Runtime>,
    crate::requests::OutgoingRemovePeerCompat<Runtime>,
) {
    use crate::requests::{OffchainRequest, OutgoingRequest};
    let author = AccountId::new([1; 32]);
    let peer_account_id = AccountId::new([3; 32]);
    let peer_address = H160::repeat_byte(2);
    let timepoint = bridge_multisig::BridgeTimepoint {
        height: bridge_multisig::MultiChainHeight::Thischain(42),
        index: 3,
    };
    let add =
        OffchainRequest::outgoing(OutgoingRequest::AddPeer(crate::requests::OutgoingAddPeer {
            author: author.clone(),
            peer_account_id: peer_account_id.clone(),
            peer_address,
            nonce,
            network_id: ETH_NETWORK_ID,
            timepoint,
        }));
    let add_compat = crate::requests::OutgoingAddPeerCompat {
        author: author.clone(),
        peer_account_id: peer_account_id.clone(),
        peer_address,
        nonce: nonce.wrapping_add(1),
        network_id: ETH_NETWORK_ID,
        timepoint,
    };
    let remove_compat = crate::requests::OutgoingRemovePeerCompat {
        author: author.clone(),
        peer_account_id: peer_account_id.clone(),
        peer_address,
        nonce,
        network_id: ETH_NETWORK_ID,
        timepoint,
    };
    let compat_hash =
        OffchainRequest::outgoing(OutgoingRequest::RemovePeerCompat(remove_compat.clone())).hash();
    let remove = OffchainRequest::outgoing(OutgoingRequest::RemovePeer(
        crate::requests::OutgoingRemovePeer {
            author,
            peer_account_id,
            peer_address,
            nonce: nonce.wrapping_add(1),
            network_id: ETH_NETWORK_ID,
            timepoint,
            compat_hash: Some(compat_hash),
        },
    ));
    (add, add_compat, remove, remove_compat)
}

#[test]
fn peer_pair_hashes_preserve_historical_source_layout() {
    use crate::requests::{OffchainRequest, OutgoingRequest};
    // Frozen synthetic fixtures using the SCALE layout in 66a100429 (the commit
    // introducing compat_hash), instantiated with this mock's u64 nonce/height.
    // These are source-layout fixtures, not a claim of captured production state.
    let (add, add_compat, remove, remove_compat) = canonical_peer_pair_fixture(7);
    assert_eq!(
        add.hash(),
        H256(hex!(
            "6f1b25bb28e61c10a9581035c3b869cce10b649a6e16257cdd0d55b17929a0c2"
        ))
    );
    assert_eq!(
        OffchainRequest::outgoing(OutgoingRequest::AddPeerCompat(add_compat)).hash(),
        H256(hex!(
            "8086d0f0983167d5552b4f59f856de7d5f68aa832cce35f98ac78643e52369f6"
        ))
    );
    assert_eq!(
        remove.hash(),
        H256(hex!(
            "1905ba99345ad46cfa4d67b3254a1d41420df3beb408ce10ad13b96a212fbd8e"
        ))
    );
    assert_eq!(
        OffchainRequest::outgoing(OutgoingRequest::RemovePeerCompat(remove_compat)).hash(),
        H256(hex!(
            "4df1948ff52caa6aea7c5c5c9d734c52889e221ca89686580a1ba97b4063e2a0"
        ))
    );
}

#[test]
fn peer_pair_lookup_matches_legacy_scan_after_primary_leaves_queue() {
    use crate::requests::{OffchainRequest, OutgoingRequest};
    for nonce in [0, 7, u64::MAX] {
        let (mut ext, _) = ExtBuilder::default().build();
        ext.execute_with(|| {
            let (add, add_compat, remove, remove_compat) = canonical_peer_pair_fixture(nonce);
            let add_hash = add.hash();
            let remove_hash = remove.hash();
            crate::Requests::<Runtime>::insert(ETH_NETWORK_ID, add_hash, add);
            crate::Requests::<Runtime>::insert(ETH_NETWORK_ID, remove_hash, remove);
            // Do not rely on a queue entry: the primary may already be approved/done.
            crate::RequestsQueue::<Runtime>::remove(ETH_NETWORK_ID);
            for status in [
                RequestStatus::Pending,
                RequestStatus::ApprovalsReady,
                RequestStatus::Done,
                RequestStatus::Failed(Error::Cancelled.into()),
            ] {
                crate::RequestStatuses::<Runtime>::insert(ETH_NETWORK_ID, add_hash, &status);
                let legacy_status = crate::Requests::<Runtime>::iter_prefix(ETH_NETWORK_ID)
                    .find_map(|(hash, request)| match request {
                        OffchainRequest::Outgoing(OutgoingRequest::AddPeer(req), _)
                            if req.peer_account_id == add_compat.peer_account_id
                                && req.peer_address == add_compat.peer_address
                                && req.timepoint == add_compat.timepoint =>
                        {
                            crate::RequestStatuses::<Runtime>::get(ETH_NETWORK_ID, hash)
                        }
                        _ => None,
                    });
                assert_eq!(
                    add_compat.should_be_skipped(),
                    legacy_status == Some(RequestStatus::Pending)
                );
                let expected = if matches!(
                    legacy_status,
                    Some(RequestStatus::ApprovalsReady | RequestStatus::Done)
                ) {
                    Ok(())
                } else {
                    Err(Error::RequestIsNotReady.into())
                };
                assert_eq!(add_compat.finalize(), expected);
            }
            let compat_hash =
                OffchainRequest::outgoing(OutgoingRequest::RemovePeerCompat(remove_compat.clone()))
                    .hash();
            let legacy_match =
                crate::Requests::<Runtime>::iter_prefix(ETH_NETWORK_ID).any(|(_, request)| {
                    matches!(
                        request,
                        OffchainRequest::Outgoing(OutgoingRequest::RemovePeer(req), _)
                            if req.compat_hash == Some(compat_hash)
                                && req.peer_account_id == remove_compat.peer_account_id
                                && req.peer_address == remove_compat.peer_address
                                && req.timepoint == remove_compat.timepoint
                    )
                });
            assert!(legacy_match);
            assert_ok!(remove_compat.finalize());
            crate::Requests::<Runtime>::remove(ETH_NETWORK_ID, add_hash);
            crate::Requests::<Runtime>::remove(ETH_NETWORK_ID, remove_hash);
            // A retained status alone must not authenticate a missing primary request.
            assert_err!(add_compat.finalize(), Error::UnknownRequest);
            assert_err!(remove_compat.finalize(), Error::UnknownRequest);
        });
    }
}

#[test]
fn peer_pair_lookup_rejects_inconsistent_primary_payloads() {
    use crate::requests::{OffchainRequest, OutgoingRequest};
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let (add, add_compat, remove, remove_compat) = canonical_peer_pair_fixture(7);
        let add_hash = add.hash();
        let remove_hash = remove.hash();
        let OffchainRequest::Outgoing(OutgoingRequest::AddPeer(mut bad_add), _) = add else {
            unreachable!()
        };
        bad_add.author = AccountId::new([9; 32]);
        let OffchainRequest::Outgoing(OutgoingRequest::RemovePeer(mut bad_remove), _) = remove
        else {
            unreachable!()
        };
        bad_remove.compat_hash = Some(H256::repeat_byte(9));
        crate::Requests::<Runtime>::insert(
            ETH_NETWORK_ID,
            add_hash,
            OffchainRequest::outgoing(OutgoingRequest::AddPeer(bad_add)),
        );
        crate::Requests::<Runtime>::insert(
            ETH_NETWORK_ID,
            remove_hash,
            OffchainRequest::outgoing(OutgoingRequest::RemovePeer(bad_remove)),
        );
        crate::RequestStatuses::<Runtime>::insert(ETH_NETWORK_ID, add_hash, RequestStatus::Done);
        assert_err!(add_compat.finalize(), Error::UnknownRequest);
        assert_err!(remove_compat.finalize(), Error::UnknownRequest);
    });
}

#[test]
fn peer_pair_lookup_does_not_scan_unrelated_history() {
    let (mut ext, _) = ExtBuilder::default().build();
    let (_, add_compat, _, remove_compat) = canonical_peer_pair_fixture(7);
    ext.execute_with(|| {
        for nonce in 100..356 {
            let (mut unrelated, _, _, _) = canonical_peer_pair_fixture(nonce);
            // Give each historical request a different peer, so the old scan must
            // inspect every value to establish that the requested pair is missing.
            if let crate::requests::OffchainRequest::Outgoing(
                crate::requests::OutgoingRequest::AddPeer(ref mut peer),
                _,
            ) = unrelated
            {
                peer.peer_address = H160::repeat_byte(8);
            }
            let crate::requests::OffchainRequest::Outgoing(unrelated, _) = unrelated else {
                unreachable!()
            };
            let unrelated = crate::requests::OffchainRequest::outgoing(unrelated);
            crate::Requests::<Runtime>::insert(ETH_NETWORK_ID, unrelated.hash(), unrelated);
        }
    });
    ext.commit_all().unwrap();
    let (_, add_proof) = ext.execute_and_prove(|| {
        assert!(!add_compat.should_be_skipped());
        assert_err!(add_compat.finalize(), Error::UnknownRequest);
    });
    let (_, remove_proof) = ext.execute_and_prove(|| {
        assert_err!(remove_compat.finalize(), Error::UnknownRequest);
    });
    let (_, legacy_proof) = ext.execute_and_prove(|| {
        assert!(
            !crate::Requests::<Runtime>::iter_prefix(ETH_NETWORK_ID).any(|(_, request)| {
                matches!(request,
                    crate::requests::OffchainRequest::Outgoing(
                        crate::requests::OutgoingRequest::AddPeer(peer), _
                    ) if peer.peer_address == add_compat.peer_address
                )
            })
        );
    });
    assert!(
        legacy_proof.len() >= 256,
        "fixture must expose the old historical scan"
    );
    for (kind, proof) in [("add", add_proof), ("remove", remove_proof)] {
        assert!(
            proof.len() < 80,
            "{kind} peer lookup scanned history: {} proof nodes",
            proof.len()
        );
    }
}

#[test]
fn should_add_peer_in_eth_network() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let kp = ecdsa::Pair::from_string("//OCW5", None).unwrap();
        let signer = AccountPublic::from(kp.public());
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&kp.seed()).unwrap());

        // outgoing request part
        let new_peer_id = signer.into_account();
        let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&new_peer_id, 1u32.into());
        let new_peer_address = eth::public_key_to_eth_address(&public);
        assert_ok!(EthBridge::add_peer(
            RuntimeOrigin::root(),
            new_peer_id.clone(),
            new_peer_address,
            net_id,
        ));
        assert_eq!(
            crate::PendingPeer::<Runtime>::get(net_id).unwrap(),
            new_peer_id
        );
        approve_next_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            crate::PendingPeer::<Runtime>::get(net_id).unwrap(),
            new_peer_id
        );
        assert_eq!(
            crate::PeerAccountId::<Runtime>::get(&net_id, &new_peer_address),
            Some(new_peer_id.clone())
        );
        assert_eq!(
            crate::PeerAddress::<Runtime>::get(net_id, &new_peer_id),
            new_peer_address
        );
        approve_next_request(&state, net_id).expect("request wasn't approved");
        // incoming request part
        // peer is added to Bridge contract
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingTransactionRequestKind::AddPeer.into(),
            net_id,
        )
        .unwrap();
        let incoming_request = IncomingRequest::ChangePeers(crate::IncomingChangePeers {
            peer_account_id: Some(new_peer_id.clone()),
            peer_address: new_peer_address,
            removed: false,
            author: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert_incoming_request_done(&state, incoming_request.clone()).unwrap();
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&new_peer_id));
        // peer is added to VAL contract
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[3u8; 32]),
            IncomingTransactionRequestKind::AddPeerCompat.into(),
            net_id,
        )
        .unwrap();
        let incoming_request =
            IncomingRequest::ChangePeersCompat(crate::IncomingChangePeersCompat {
                peer_account_id: new_peer_id.clone(),
                peer_address: new_peer_address,
                added: true,
                contract: ChangePeersContract::VAL,
                author: alice.clone(),
                tx_hash,
                at_height: 3,
                timepoint: Default::default(),
                network_id: net_id,
            });
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&new_peer_id));
        assert!(crate::PendingPeer::<Runtime>::get(net_id).is_some());
        assert_incoming_request_done(&state, incoming_request.clone()).unwrap();
        assert!(crate::PendingPeer::<Runtime>::get(net_id).is_none());
        assert!(crate::Peers::<Runtime>::get(net_id).contains(&new_peer_id));
        assert!(bridge_multisig::Accounts::<Runtime>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(&new_peer_id));
    });
}

#[test]
fn should_reject_direct_incoming_xor_peer_compat_request() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let peer_id = get_account_id_from_seed::<sr25519::Public>("Bob");
        let peer_address = H160::repeat_byte(0x11);
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[9u8; 32]),
            IncomingTransactionRequestKind::AddPeerCompat.into(),
            net_id,
        )
        .unwrap();

        let incoming_request =
            IncomingRequest::ChangePeersCompat(crate::IncomingChangePeersCompat {
                peer_account_id: peer_id,
                peer_address,
                added: true,
                contract: ChangePeersContract::XOR,
                author: alice,
                tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: net_id,
            });

        assert_incoming_request_registration_failed(
            &state,
            incoming_request,
            Error::DeprecatedLegacyXor,
        )
        .unwrap();
    });
}

#[test]
fn should_add_peer_in_simple_networks() {
    let mut builder = ExtBuilder::default();
    let net_id = builder.add_network(vec![], None, Some(4), Default::default());
    assert_ne!(net_id, ETH_NETWORK_ID);
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let kp = ecdsa::Pair::from_string("//OCW5", None).unwrap();
        let signer = AccountPublic::from(kp.public());
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&kp.seed()).unwrap());

        // outgoing request part
        let new_peer_id = signer.into_account();
        let new_peer_address = eth::public_key_to_eth_address(&public);
        let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&new_peer_id, 1u32.into());
        assert_ok!(EthBridge::add_peer(
            RuntimeOrigin::root(),
            new_peer_id.clone(),
            new_peer_address,
            net_id,
        ));
        assert_eq!(
            crate::PendingPeer::<Runtime>::get(net_id).unwrap(),
            new_peer_id
        );
        approve_next_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            crate::PendingPeer::<Runtime>::get(net_id).unwrap(),
            new_peer_id
        );
        assert_eq!(
            crate::PeerAccountId::<Runtime>::get(&net_id, &new_peer_address),
            Some(new_peer_id.clone())
        );
        assert_eq!(
            crate::PeerAddress::<Runtime>::get(net_id, &new_peer_id),
            new_peer_address
        );
        // incoming request part
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingTransactionRequestKind::AddPeer.into(),
            net_id,
        )
        .unwrap();
        let incoming_request = IncomingRequest::ChangePeers(crate::IncomingChangePeers {
            peer_account_id: Some(new_peer_id.clone()),
            peer_address: new_peer_address,
            removed: false,
            author: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&new_peer_id));
        assert_incoming_request_done(&state, incoming_request.clone()).unwrap();
        assert!(crate::PendingPeer::<Runtime>::get(net_id).is_none());
        assert!(crate::Peers::<Runtime>::get(net_id).contains(&new_peer_id));
        assert!(bridge_multisig::Accounts::<Runtime>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(&new_peer_id));
    });
}

#[test]
fn should_remove_peer_in_simple_network() {
    let mut builder = ExtBuilder::default();
    let net_id = builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let extended_network_config = &state.networks[&net_id];
        let bridge_acc_id = extended_network_config.config.bridge_account_id.clone();
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let (_, peer_id, seed) = &extended_network_config.ocw_keypairs[4];
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&seed[..]).unwrap());

        // outgoing request part
        assert_ok!(EthBridge::remove_peer(
            RuntimeOrigin::root(),
            peer_id.clone(),
            Some(H160::repeat_byte(12)),
            net_id,
        ));
        assert_eq!(
            &crate::PendingPeer::<Runtime>::get(net_id).unwrap(),
            peer_id
        );
        assert!(crate::Peers::<Runtime>::get(net_id).contains(&peer_id));
        approve_next_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            &crate::PendingPeer::<Runtime>::get(net_id).unwrap(),
            peer_id
        );
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&peer_id));
        assert!(!bridge_multisig::Accounts::<Runtime>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(&peer_id));

        // incoming request part
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingTransactionRequestKind::RemovePeer.into(),
            net_id,
        )
        .unwrap();
        let peer_address = eth::public_key_to_eth_address(&public);
        let incoming_request = IncomingRequest::ChangePeers(crate::IncomingChangePeers {
            peer_account_id: Some(peer_id.clone()),
            peer_address,
            removed: true,
            author: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert_incoming_request_done(&state, incoming_request.clone()).unwrap();
        assert!(crate::PendingPeer::<Runtime>::get(net_id).is_none());
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&peer_id));
        assert!(!bridge_multisig::Accounts::<Runtime>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(&peer_id));
    });
}

#[test]
fn should_remove_peer_in_eth_network() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let extended_network_config = &state.networks[&net_id];
        let bridge_acc_id = extended_network_config.config.bridge_account_id.clone();
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let (_, peer_id, seed) = &extended_network_config.ocw_keypairs[4];
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&seed[..]).unwrap());

        // outgoing request part
        assert_ok!(EthBridge::remove_peer(
            RuntimeOrigin::root(),
            peer_id.clone(),
            Some(H160::repeat_byte(12)),
            net_id,
        ));
        assert_eq!(
            &crate::PendingPeer::<Runtime>::get(net_id).unwrap(),
            peer_id
        );
        assert!(crate::Peers::<Runtime>::get(net_id).contains(&peer_id));
        approve_next_request(&state, net_id).expect("request wasn't approved");
        approve_next_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            &crate::PendingPeer::<Runtime>::get(net_id).unwrap(),
            peer_id
        );
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&peer_id));
        assert!(!bridge_multisig::Accounts::<Runtime>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(&peer_id));

        // incoming request part
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingTransactionRequestKind::RemovePeer.into(),
            net_id,
        )
        .unwrap();
        let peer_address = eth::public_key_to_eth_address(&public);
        let incoming_request = IncomingRequest::ChangePeers(crate::IncomingChangePeers {
            peer_account_id: Some(peer_id.clone()),
            peer_address,
            removed: true,
            author: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert_incoming_request_done(&state, incoming_request.clone()).unwrap();
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&peer_id));
        // peer is added to VAL contract
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[3u8; 32]),
            IncomingTransactionRequestKind::AddPeerCompat.into(),
            net_id,
        )
        .unwrap();
        let incoming_request =
            IncomingRequest::ChangePeersCompat(crate::IncomingChangePeersCompat {
                peer_account_id: peer_id.clone(),
                peer_address,
                added: false,
                contract: ChangePeersContract::VAL,
                author: alice.clone(),
                tx_hash,
                at_height: 3,
                timepoint: Default::default(),
                network_id: net_id,
            });
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&peer_id));
        assert_incoming_request_done(&state, incoming_request.clone()).unwrap();
        assert!(crate::PendingPeer::<Runtime>::get(net_id).is_none());
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&peer_id));
        assert!(!bridge_multisig::Accounts::<Runtime>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(&peer_id));
    });
}

#[test]
fn should_not_allow_add_and_remove_peer_only_to_authority() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let (_, peer_id, _) = &state.networks[&net_id].ocw_keypairs[4];
        assert_err!(
            EthBridge::remove_peer(
                RuntimeOrigin::signed(bob.clone()),
                peer_id.clone(),
                None,
                net_id
            ),
            frame_support::sp_runtime::DispatchError::BadOrigin
        );
        assert_err!(
            EthBridge::add_peer(
                RuntimeOrigin::signed(bob.clone()),
                peer_id.clone(),
                EthAddress::from(&hex!("2222222222222222222222222222222222222222")),
                net_id,
            ),
            frame_support::sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn should_not_allow_changing_peers_simultaneously() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let (_, peer_id, seed) = &state.networks[&net_id].ocw_keypairs[4];
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&seed[..]).unwrap());
        let address = eth::public_key_to_eth_address(&public);
        assert_ok!(EthBridge::remove_peer(
            RuntimeOrigin::root(),
            peer_id.clone(),
            Some(H160::repeat_byte(12)),
            net_id,
        ));
        approve_next_request(&state, net_id).expect("request wasn't approved");
        approve_next_request(&state, net_id).expect("request wasn't approved");
        assert_err!(
            EthBridge::remove_peer(
                RuntimeOrigin::root(),
                peer_id.clone(),
                Some(H160::repeat_byte(12)),
                net_id
            ),
            Error::CantRemoveMorePeers
        );
        assert_err!(
            EthBridge::add_peer(RuntimeOrigin::root(), peer_id.clone(), address, net_id,),
            Error::TooManyPendingPeers
        );
    });
}

#[test]
fn should_not_add_peer_when_peers_at_limit() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, _state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let peers: std::collections::BTreeSet<AccountId> = (0..crate::MAX_PEERS)
            .map(|i| AccountId::new([i as u8; 32]))
            .collect();
        crate::Peers::<Runtime>::insert(net_id, peers);

        let kp = ecdsa::Pair::from_string("//MAXPEER", None).unwrap();
        let signer = AccountPublic::from(kp.public());
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&kp.seed()).unwrap());
        let new_peer_id = signer.into_account();
        let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&new_peer_id, 1u32.into());
        let new_peer_address = eth::public_key_to_eth_address(&public);

        assert_err!(
            EthBridge::add_peer(RuntimeOrigin::root(), new_peer_id, new_peer_address, net_id,),
            Error::CantAddMorePeers
        );
    });
}

#[test]
fn force_add_peer_rejects_a_peer_set_at_the_limit() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, _state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let peers: std::collections::BTreeSet<AccountId> = (0..crate::MAX_PEERS)
            .map(|i| AccountId::new([i as u8; 32]))
            .collect();
        crate::Peers::<Runtime>::insert(net_id, peers);

        assert_err!(
            EthBridge::force_add_peer(
                RuntimeOrigin::root(),
                AccountId::new([250u8; 32]),
                H160::repeat_byte(0x25),
                net_id,
            ),
            Error::CantAddMorePeers
        );
        assert_eq!(crate::Peers::<Runtime>::get(net_id).len(), crate::MAX_PEERS);
    });
}

#[test]
fn register_bridge_rejects_more_than_the_maximum_peers() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let initial_peers = (0..=crate::MAX_PEERS)
            .map(|i| AccountId::new([i as u8; 32]))
            .collect::<Vec<_>>();

        assert_err!(
            EthBridge::register_bridge(
                RuntimeOrigin::root(),
                H160::repeat_byte(0x26),
                initial_peers,
                crate::BridgeSignatureVersion::V3,
            ),
            Error::CantAddMorePeers
        );
    });
}

#[test]
fn add_peer_compat_validate_fails_when_peers_at_limit() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, _state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let peers: std::collections::BTreeSet<AccountId> = (0..crate::MAX_PEERS)
            .map(|i| AccountId::new([i as u8; 32]))
            .collect();
        crate::Peers::<Runtime>::insert(net_id, peers);

        let author = get_account_id_from_seed::<sr25519::Public>("Alice");
        let request = crate::requests::OutgoingAddPeerCompat::<Runtime> {
            author,
            peer_address: EthAddress::from(&hex!("7777777777777777777777777777777777777777")),
            peer_account_id: AccountId::new([250u8; 32]),
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
        };
        assert_err!(request.validate(), Error::CantAddMorePeers);
    });
}

#[test]
fn should_not_remove_peer_when_peers_at_minimum() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, _state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let peers: std::collections::BTreeSet<AccountId> = (0..crate::MIN_PEERS)
            .map(|i| AccountId::new([i as u8; 32]))
            .collect();
        crate::Peers::<Runtime>::insert(net_id, peers);

        assert_err!(
            EthBridge::remove_peer(
                RuntimeOrigin::root(),
                AccountId::new([0u8; 32]),
                Some(EthAddress::from(&hex!(
                    "8888888888888888888888888888888888888888"
                ))),
                net_id,
            ),
            Error::CantRemoveMorePeers
        );
    });
}

#[test]
fn remove_peer_compat_validate_fails_when_peers_at_minimum() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, _state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let peers: std::collections::BTreeSet<AccountId> = (0..crate::MIN_PEERS)
            .map(|i| AccountId::new([i as u8; 32]))
            .collect();
        crate::Peers::<Runtime>::insert(net_id, peers);

        let author = get_account_id_from_seed::<sr25519::Public>("Alice");
        let request = crate::requests::OutgoingRemovePeerCompat::<Runtime> {
            author,
            peer_account_id: AccountId::new([0u8; 32]),
            peer_address: EthAddress::from(&hex!("8888888888888888888888888888888888888888")),
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
        };
        assert_err!(request.validate(), Error::CantRemoveMorePeers);
    });
}

#[test]
fn force_add_peer_preserves_slot_reserved_for_pending_addition() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, _state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let peers: std::collections::BTreeSet<AccountId> = (0..crate::MAX_PEERS - 1)
            .map(|i| AccountId::new([i as u8; 32]))
            .collect();
        crate::Peers::<Runtime>::insert(net_id, peers);
        crate::PendingPeer::<Runtime>::insert(net_id, AccountId::new([249u8; 32]));

        assert_err!(
            EthBridge::force_add_peer(
                RuntimeOrigin::root(),
                AccountId::new([250u8; 32]),
                H160::repeat_byte(0x25),
                net_id,
            ),
            Error::TooManyPendingPeers
        );
        assert_eq!(
            crate::Peers::<Runtime>::get(net_id).len(),
            crate::MAX_PEERS - 1
        );
    });
}

#[test]
fn should_not_approve_add_peer_compat_before_add_peer_is_processed() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let kp = ecdsa::Pair::from_string("//OCW5", None).unwrap();
        let signer = AccountPublic::from(kp.public());
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&kp.seed()).unwrap());
        let new_peer_id = signer.into_account();
        let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&new_peer_id, 1u32.into());
        let new_peer_address = eth::public_key_to_eth_address(&public);
        assert_ok!(EthBridge::add_peer(
            RuntimeOrigin::root(),
            new_peer_id,
            new_peer_address,
            net_id,
        ));

        let (compat_request, compat_hash) = crate::RequestsQueue::<Runtime>::get(net_id)
            .iter()
            .find_map(|hash| {
                crate::Requests::<Runtime>::get(net_id, *hash)
                    .and_then(|r| r.into_outgoing())
                    .and_then(|(req, req_hash)| match req {
                        crate::requests::OutgoingRequest::AddPeerCompat(_) => Some((req, req_hash)),
                        _ => None,
                    })
            })
            .expect("add peer compat request should be queued");

        assert!(approve_request(&state, compat_request, compat_hash).is_err());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, compat_hash),
            Some(RequestStatus::Pending)
        );

        approve_next_request(&state, net_id).expect("add peer request wasn't approved");
        approve_next_request(&state, net_id).expect("add peer compat request wasn't approved");
    });
}

#[test]
fn should_fail_add_peer_compat_without_corresponding_add_peer_request() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let kp = ecdsa::Pair::from_string("//OCW5", None).unwrap();
        let signer = AccountPublic::from(kp.public());
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&kp.seed()).unwrap());
        let new_peer_id = signer.into_account();
        let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&new_peer_id, 1u32.into());
        let new_peer_address = eth::public_key_to_eth_address(&public);
        assert_ok!(EthBridge::add_peer(
            RuntimeOrigin::root(),
            new_peer_id,
            new_peer_address,
            net_id,
        ));

        let (compat_request, compat_hash, add_hash) = crate::RequestsQueue::<Runtime>::get(net_id)
            .into_iter()
            .fold((None, None, None), |(compat_req, compat_h, add_h), hash| {
                let Some((req, req_hash)) =
                    crate::Requests::<Runtime>::get(net_id, hash).and_then(|r| r.into_outgoing())
                else {
                    return (compat_req, compat_h, add_h);
                };
                match req {
                    crate::requests::OutgoingRequest::AddPeerCompat(_) => {
                        (Some(req), Some(req_hash), add_h)
                    }
                    crate::requests::OutgoingRequest::AddPeer(_) => {
                        (compat_req, compat_h, Some(req_hash))
                    }
                    _ => (compat_req, compat_h, add_h),
                }
            });
        let compat_request = compat_request.expect("add peer compat request should be queued");
        let compat_hash = compat_hash.expect("add peer compat hash should exist");
        let add_hash = add_hash.expect("add peer hash should exist");

        // Orphan the compat request by deleting its corresponding AddPeer request.
        crate::Requests::<Runtime>::remove(net_id, add_hash);
        crate::RequestStatuses::<Runtime>::remove(net_id, add_hash);
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| {
            queue.retain(|hash| *hash != add_hash);
        });

        assert!(approve_request(&state, compat_request, compat_hash).is_err());
        let expected_error: DispatchError = Error::UnknownRequest.into();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, compat_hash),
            Some(RequestStatus::Broken(
                expected_error,
                Error::PublicApprovalsRequireSettlement.into()
            ))
        );
    });
}

#[test]
fn should_not_approve_remove_peer_before_compat_request_is_processed() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let (_, peer_id, _) = &state.networks[&net_id].ocw_keypairs[4];
        assert_ok!(EthBridge::remove_peer(
            RuntimeOrigin::root(),
            peer_id.clone(),
            Some(H160::repeat_byte(12)),
            net_id,
        ));

        let queue = crate::RequestsQueue::<Runtime>::get(net_id);
        let (remove_request, remove_hash) = queue
            .iter()
            .find_map(|hash| {
                crate::Requests::<Runtime>::get(net_id, *hash)
                    .and_then(|r| r.into_outgoing())
                    .and_then(|(req, req_hash)| match req {
                        crate::requests::OutgoingRequest::RemovePeer(_) => Some((req, req_hash)),
                        _ => None,
                    })
            })
            .expect("remove peer request should be queued");

        assert!(approve_request(&state, remove_request, remove_hash).is_err());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, remove_hash),
            Some(RequestStatus::Pending)
        );

        approve_next_request(&state, net_id).expect("compat request wasn't approved");
        approve_next_request(&state, net_id).expect("remove request wasn't approved");
    });
}

#[test]
fn should_fail_remove_peer_compat_without_corresponding_remove_peer_request() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let (_, peer_id, _) = &state.networks[&net_id].ocw_keypairs[4];
        assert_ok!(EthBridge::remove_peer(
            RuntimeOrigin::root(),
            peer_id.clone(),
            Some(H160::repeat_byte(12)),
            net_id,
        ));

        let (compat_request, compat_hash, remove_hash) =
            crate::RequestsQueue::<Runtime>::get(net_id)
                .into_iter()
                .fold(
                    (None, None, None),
                    |(compat_req, compat_h, remove_h), hash| {
                        let Some((req, req_hash)) = crate::Requests::<Runtime>::get(net_id, hash)
                            .and_then(|r| r.into_outgoing())
                        else {
                            return (compat_req, compat_h, remove_h);
                        };
                        match req {
                            crate::requests::OutgoingRequest::RemovePeerCompat(_) => {
                                (Some(req), Some(req_hash), remove_h)
                            }
                            crate::requests::OutgoingRequest::RemovePeer(_) => {
                                (compat_req, compat_h, Some(req_hash))
                            }
                            _ => (compat_req, compat_h, remove_h),
                        }
                    },
                );
        let compat_request = compat_request.expect("remove peer compat request should be queued");
        let compat_hash = compat_hash.expect("remove peer compat hash should exist");
        let remove_hash = remove_hash.expect("remove peer hash should exist");

        // Orphan the compat request by deleting its corresponding RemovePeer request.
        crate::Requests::<Runtime>::remove(net_id, remove_hash);
        crate::RequestStatuses::<Runtime>::remove(net_id, remove_hash);
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| {
            queue.retain(|hash| *hash != remove_hash);
        });

        assert!(approve_request(&state, compat_request, compat_hash).is_err());
        let expected_error: DispatchError = Error::UnknownRequest.into();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, compat_hash),
            Some(RequestStatus::Broken(
                expected_error,
                Error::PublicApprovalsRequireSettlement.into()
            ))
        );
    });
}

#[test]
fn should_fail_remove_peer_without_corresponding_compat_request() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5), Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let (_, peer_id, _) = &state.networks[&net_id].ocw_keypairs[4];
        assert_ok!(EthBridge::remove_peer(
            RuntimeOrigin::root(),
            peer_id.clone(),
            Some(H160::repeat_byte(12)),
            net_id,
        ));

        let (compat_hash, remove_request, remove_hash) =
            crate::RequestsQueue::<Runtime>::get(net_id)
                .into_iter()
                .fold(
                    (None, None, None),
                    |(compat_h, remove_req, remove_h), hash| {
                        let Some((req, req_hash)) = crate::Requests::<Runtime>::get(net_id, hash)
                            .and_then(|r| r.into_outgoing())
                        else {
                            return (compat_h, remove_req, remove_h);
                        };
                        match req {
                            crate::requests::OutgoingRequest::RemovePeerCompat(_) => {
                                (Some(req_hash), remove_req, remove_h)
                            }
                            crate::requests::OutgoingRequest::RemovePeer(_) => {
                                (compat_h, Some(req), Some(req_hash))
                            }
                            _ => (compat_h, remove_req, remove_h),
                        }
                    },
                );
        let compat_hash = compat_hash.expect("remove peer compat hash should exist");
        let remove_request = remove_request.expect("remove peer request should exist");
        let remove_hash = remove_hash.expect("remove peer hash should exist");

        // Orphan the RemovePeer request by deleting its corresponding compat request.
        crate::Requests::<Runtime>::remove(net_id, compat_hash);
        crate::RequestStatuses::<Runtime>::remove(net_id, compat_hash);
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| {
            queue.retain(|hash| *hash != compat_hash);
        });

        assert!(approve_request(&state, remove_request, remove_hash).is_err());
        let expected_error: DispatchError = Error::UnknownRequest.into();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, remove_hash),
            Some(RequestStatus::Broken(
                expected_error,
                Error::PublicApprovalsRequireSettlement.into()
            ))
        );
    });
}

#[test]
fn should_reject_add_peer_on_old_xor_contract() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");

        let kp = ecdsa::Pair::from_string("//Bob", None).unwrap();
        let signer = AccountPublic::from(kp.public());
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&kp.seed()).unwrap());
        let new_peer_id = signer.into_account();
        let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&new_peer_id, 1u32.into());
        let new_peer_address = eth::public_key_to_eth_address(&public);
        assert_ok!(EthBridge::add_peer(
            RuntimeOrigin::root(),
            new_peer_id.clone(),
            new_peer_address,
            net_id,
        ));
        approve_next_request(&state, net_id).expect("request wasn't approved");
        approve_next_request(&state, net_id).expect("request wasn't approved");

        let tx_hash = H256([1; 32]);
        // add peer
        let incoming_request = LoadIncomingTransactionRequest::<Runtime> {
            author: alice.clone(),
            hash: tx_hash,
            timepoint: Default::default(),
            kind: IncomingTransactionRequestKind::AddPeer,
            network_id: net_id,
        };
        let tx = Transaction {
            input: Bytes(hex!("ca70cf6e00000000000000000000000025451a4de12dccc2d166922fa938e900fcc4ed246c47988669c11ca53e19e2f41b5bc8b7ce1188509235a6470d14fefaffc063b300000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000008900000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec()),
            block_number: Some(1u64.into()),
            to: Some(types::H160(EthBridge::xor_master_contract_address().0)),
            ..Default::default()
        };
        assert_err!(
            EthBridge::parse_old_incoming_request_method_call(incoming_request, tx),
            Error::UnknownContractAddress
        );
    });
}

#[test]
fn should_parse_remove_peer_on_old_contract() {
    let (mut ext, _state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");

        let kp = ecdsa::Pair::from_string("//Bob", None).unwrap();
        let signer = AccountPublic::from(kp.public());
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&kp.seed()).unwrap());
        let new_peer_id = signer.into_account();
        let new_peer_address = eth::public_key_to_eth_address(&public);
        let tx_hash = H256([1; 32]);
        let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&new_peer_id, 1u32.into());
        assert_ok!(EthBridge::force_add_peer(RuntimeOrigin::root(), new_peer_id.clone(), new_peer_address, net_id));
        assert_ok!(EthBridge::remove_peer(
            RuntimeOrigin::root(),
            new_peer_id.clone(),
            None,
            net_id,
        ));

        let incoming_request = LoadIncomingTransactionRequest::<Runtime> {
            author: alice.clone(),
            hash: tx_hash,
            timepoint: Default::default(),
            kind: IncomingTransactionRequestKind::RemovePeerCompat,
            network_id: net_id,
        };
        let tx = Transaction {
            input: Bytes(hex!("89c39baf00000000000000000000000025451a4de12dccc2d166922fa938e900fcc4ed242b1bd542bd68ef39afeee8c1d9957a9bfa53038558ce2c618859205a77d6ffce00000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec()),
            block_number: Some(1u64.into()),
            to: Some(types::H160(EthBridge::val_master_contract_address().0)),
            ..Default::default()
        };
        assert_eq!(
            EthBridge::parse_old_incoming_request_method_call(incoming_request, tx).unwrap(),
            IncomingRequest::ChangePeersCompat(IncomingChangePeersCompat {
                peer_account_id: new_peer_id,
                peer_address: new_peer_address,
                added: false,
                contract: ChangePeersContract::VAL,
                author: alice.clone(),
                tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: net_id
            })
        );
    });
}

#[test]
fn force_add_peer_should_reject_unknown_network() {
    let (mut ext, _state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let peer_address = H160::repeat_byte(0x11);
        assert_err!(
            EthBridge::force_add_peer(RuntimeOrigin::root(), bob, peer_address, ETH_NETWORK_ID + 1),
            Error::UnknownNetwork
        );
    });
}
