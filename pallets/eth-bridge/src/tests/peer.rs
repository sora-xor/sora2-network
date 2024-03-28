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
use crate::requests::{
    ChangePeersContract, IncomingChangePeersCompat, IncomingRequest,
    IncomingTransactionRequestKind, LoadIncomingTransactionRequest,
};
use crate::tests::mock::{get_account_id_from_seed, ExtBuilder};
use crate::tests::{
    approve_next_request, assert_incoming_request_done, request_incoming, ETH_NETWORK_ID,
};
use crate::types::{Bytes, Transaction};
use crate::{types, EthAddress};
use common::eth;
use frame_support::sp_runtime::app_crypto::sp_core::{self, ecdsa, sr25519, Pair};
use frame_support::sp_runtime::traits::IdentifyAccount;
use frame_support::traits::Currency;
use frame_support::{assert_err, assert_ok};
use hex_literal::hex;
use secp256k1::{PublicKey, SecretKey};
use sp_core::{H160, H256};

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
            crate::PeerAccountId::<Runtime>::get(net_id, new_peer_address),
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
        assert_incoming_request_done(&state, incoming_request).unwrap();
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&new_peer_id));
        // peer is added to XOR contract
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[2u8; 32]),
            IncomingTransactionRequestKind::AddPeerCompat.into(),
            net_id,
        )
        .unwrap();
        let incoming_request =
            IncomingRequest::ChangePeersCompat(crate::IncomingChangePeersCompat {
                peer_account_id: new_peer_id.clone(),
                peer_address: new_peer_address,
                added: true,
                contract: ChangePeersContract::XOR,
                author: alice.clone(),
                tx_hash,
                at_height: 2,
                timepoint: Default::default(),
                network_id: net_id,
            });
        assert_incoming_request_done(&state, incoming_request).unwrap();
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
                author: alice,
                tx_hash,
                at_height: 3,
                timepoint: Default::default(),
                network_id: net_id,
            });
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&new_peer_id));
        assert!(crate::PendingPeer::<Runtime>::get(net_id).is_some());
        assert_incoming_request_done(&state, incoming_request).unwrap();
        assert!(crate::PendingPeer::<Runtime>::get(net_id).is_none());
        assert!(crate::Peers::<Runtime>::get(net_id).contains(&new_peer_id));
        assert!(bridge_multisig::Accounts::<Runtime>::get(bridge_acc_id)
            .unwrap()
            .is_signatory(&new_peer_id));
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
            crate::PeerAccountId::<Runtime>::get(net_id, new_peer_address),
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
            author: alice,
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(&new_peer_id));
        assert_incoming_request_done(&state, incoming_request).unwrap();
        assert!(crate::PendingPeer::<Runtime>::get(net_id).is_none());
        assert!(crate::Peers::<Runtime>::get(net_id).contains(&new_peer_id));
        assert!(bridge_multisig::Accounts::<Runtime>::get(bridge_acc_id)
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
        assert!(crate::Peers::<Runtime>::get(net_id).contains(peer_id));
        approve_next_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            &crate::PendingPeer::<Runtime>::get(net_id).unwrap(),
            peer_id
        );
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(peer_id));
        assert!(!bridge_multisig::Accounts::<Runtime>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(peer_id));

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
            author: alice,
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert_incoming_request_done(&state, incoming_request).unwrap();
        assert!(crate::PendingPeer::<Runtime>::get(net_id).is_none());
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(peer_id));
        assert!(!bridge_multisig::Accounts::<Runtime>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(peer_id));
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
        assert!(crate::Peers::<Runtime>::get(net_id).contains(peer_id));
        approve_next_request(&state, net_id).expect("request wasn't approved");
        approve_next_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            &crate::PendingPeer::<Runtime>::get(net_id).unwrap(),
            peer_id
        );
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(peer_id));
        assert!(!bridge_multisig::Accounts::<Runtime>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(peer_id));

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
        assert_incoming_request_done(&state, incoming_request).unwrap();
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(peer_id));
        // peer is added to XOR contract
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[2u8; 32]),
            IncomingTransactionRequestKind::AddPeerCompat.into(),
            net_id,
        )
        .unwrap();
        let incoming_request =
            IncomingRequest::ChangePeersCompat(crate::IncomingChangePeersCompat {
                peer_account_id: peer_id.clone(),
                peer_address,
                added: false,
                contract: ChangePeersContract::XOR,
                author: alice.clone(),
                tx_hash,
                at_height: 2,
                timepoint: Default::default(),
                network_id: net_id,
            });
        assert_incoming_request_done(&state, incoming_request).unwrap();
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(peer_id));
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
                author: alice,
                tx_hash,
                at_height: 3,
                timepoint: Default::default(),
                network_id: net_id,
            });
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(peer_id));
        assert_incoming_request_done(&state, incoming_request).unwrap();
        assert!(crate::PendingPeer::<Runtime>::get(net_id).is_none());
        assert!(!crate::Peers::<Runtime>::get(net_id).contains(peer_id));
        assert!(!bridge_multisig::Accounts::<Runtime>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(peer_id));
    });
}

#[test]
#[ignore]
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
            Error::Forbidden
        );
        assert_err!(
            EthBridge::add_peer(
                RuntimeOrigin::signed(bob),
                peer_id.clone(),
                EthAddress::from(&hex!("2222222222222222222222222222222222222222")),
                net_id,
            ),
            Error::Forbidden
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
            Error::UnknownPeerId
        );
        assert_err!(
            EthBridge::add_peer(RuntimeOrigin::root(), peer_id.clone(), address, net_id,),
            Error::TooManyPendingPeers
        );
    });
}

#[test]
fn should_parse_add_peer_on_old_contract() {
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
        let inc_req =
            EthBridge::parse_old_incoming_request_method_call(incoming_request, tx).unwrap();
        assert_eq!(
            inc_req,
            IncomingRequest::ChangePeersCompat(IncomingChangePeersCompat {
                peer_account_id: new_peer_id,
                peer_address: new_peer_address,
                added: true,
                contract: ChangePeersContract::XOR,
                author: alice,
                tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: net_id
            })
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
                author: alice,
                tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: net_id
            })
        );
    });
}
