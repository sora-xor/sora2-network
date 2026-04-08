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
use crate::offchain::SignatureParams;
use crate::requests::{OffchainRequest, OutgoingRequest, OutgoingTransfer, RequestStatus};
use crate::tests::{
    approve_last_request, last_event, last_outgoing_request, last_request, Assets, ETH_NETWORK_ID,
};
use crate::util::majority;
use crate::{AssetConfig, EthAddress};
use common::{eth, AssetInfoProvider, DEFAULT_BALANCE_PRECISION, KSM, PSWAP, USDT, VAL, XOR};
use ethereum_types::U256;
use frame_support::sp_runtime::app_crypto::sp_core::{self, sr25519};
use frame_support::{assert_err, assert_ok};
use hex_literal::hex;
use secp256k1::{PublicKey, SecretKey};
use sp_core::{ecdsa, H160, H256};
use sp_std::prelude::*;
use std::str::FromStr;

fn malleate_signature(signature: &SignatureParams) -> SignatureParams {
    let curve_order = U256::from_big_endian(&hex!(
        "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141"
    ));
    let mut malleated = signature.clone();
    let mut s_bytes = [0u8; 32];
    (curve_order - U256::from_big_endian(&signature.s)).to_big_endian(&mut s_bytes);
    malleated.s = s_bytes;
    malleated.v = match signature.v {
        27 => 28,
        28 => 27,
        v => v ^ 1,
    };
    malleated
}

#[test]
fn outgoing_transfer_prepare_should_fail_for_unknown_network() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100u32.into()).unwrap();
        let request = OutgoingTransfer::<Runtime> {
            from: alice.clone(),
            to: EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            asset_id: XOR.into(),
            amount: 10u32.into(),
            nonce: Default::default(),
            network_id: ETH_NETWORK_ID + 1,
            timepoint: Default::default(),
        };
        assert_err!(
            request.prepare(H256::repeat_byte(0xAB)),
            Error::UnknownNetwork
        );
    });
}

#[test]
fn should_approve_outgoing_transfer() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            99900u32.into()
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
    });
}

#[test]
fn should_reserve_and_burn_sidechain_asset_in_outgoing_transfer() {
    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: USDT.into(),
            sidechain_id: H160(hex!("dAC17F958D2ee523a2206206994597C13D831ec7")),
            owned: false,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        None,
        None,
        Default::default(),
    );
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let bridge_acc = &state.networks[&net_id].config.bridge_account_id;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&USDT.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            USDT.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(Assets::free_balance(&USDT.into(), &bridge_acc).unwrap(), 0);
        // Sidechain asset was reserved.
        assert_eq!(
            Assets::total_balance(&USDT.into(), &bridge_acc).unwrap(),
            100u32.into()
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
        // Sidechain asset was burnt.
        assert_eq!(Assets::total_balance(&USDT.into(), &bridge_acc).unwrap(), 0);
        assert_eq!(
            Assets::free_balance(&USDT.into(), &bridge_acc).unwrap(),
            Assets::total_balance(&USDT.into(), &bridge_acc).unwrap()
        );
    });
}

#[test]
fn should_reserve_and_unreserve_thischain_asset_in_outgoing_transfer() {
    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Thischain { id: PSWAP.into() }],
        None,
        None,
        Default::default(),
    );
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let bridge_acc = &state.networks[&net_id].config.bridge_account_id;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&PSWAP.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            PSWAP.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(Assets::free_balance(&PSWAP.into(), &bridge_acc).unwrap(), 0);
        // Thischain asset was reserved.
        assert_eq!(
            Assets::total_balance(&PSWAP.into(), &bridge_acc).unwrap(),
            100u32.into()
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
        // Thischain asset was unreserved.
        assert_eq!(
            Assets::total_balance(&PSWAP.into(), &bridge_acc).unwrap(),
            100u32.into()
        );
        assert_eq!(
            Assets::free_balance(&PSWAP.into(), &bridge_acc).unwrap(),
            Assets::total_balance(&PSWAP.into(), &bridge_acc).unwrap()
        );
    });
}

#[test]
fn should_not_transfer() {
    let (mut ext, _) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_err!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                KSM.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                100_u32.into(),
                net_id,
            ),
            Error::UnsupportedToken
        );
        assert!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_000_000_u32.into(),
            net_id,
        )
        .is_err());
    });
}

#[test]
fn should_register_outgoing_transfer() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from([1; 20]),
            100u32.into(),
            net_id,
        ));
        let outgoing_transfer = OutgoingTransfer::<Runtime> {
            from: alice.clone(),
            to: EthAddress::from([1; 20]),
            asset_id: XOR.into(),
            amount: 100_u32.into(),
            nonce: 3,
            network_id: ETH_NETWORK_ID,
            timepoint: bridge_multisig::Pallet::<Runtime>::thischain_timepoint(),
        };
        let last_request = last_request(net_id).unwrap();
        match last_request {
            OffchainRequest::Outgoing(OutgoingRequest::Transfer(r), _) => {
                assert_eq!(r, outgoing_transfer)
            }
            _ => panic!("Invalid off-chain request"),
        }
    });
}

#[test]
fn abort_request_removes_all_duplicate_hashes_from_queue() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            10u32.into(),
            net_id,
        ));

        let request_hash = last_outgoing_request(net_id)
            .expect("outgoing request should exist")
            .1;
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(request_hash));
        assert_eq!(
            crate::RequestsQueue::<Runtime>::get(net_id)
                .iter()
                .filter(|hash| **hash == request_hash)
                .count(),
            2
        );

        let bridge_account = state.networks[&net_id].config.bridge_account_id.clone();
        assert_ok!(EthBridge::abort_request(
            RuntimeOrigin::signed(bridge_account),
            request_hash,
            Error::Cancelled.into(),
            net_id,
        ));
        assert_eq!(
            crate::RequestsQueue::<Runtime>::get(net_id)
                .iter()
                .filter(|hash| **hash == request_hash)
                .count(),
            0
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Failed(_))
        ));
    });
}

#[test]
fn ocw_should_handle_outgoing_request() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            net_id,
        ));
        state.run_next_offchain_and_dispatch_txs();
        let hash = last_outgoing_request(net_id).unwrap().1;
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, hash).len(),
            1
        );
    });
}

#[test]
fn ocw_should_not_handle_outgoing_request_twice() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            net_id,
        ));
        state.run_next_offchain_and_dispatch_txs();
        let hash = last_outgoing_request(net_id).unwrap().1;
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, hash).len(),
            1
        );
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, hash).len(),
            1
        );
    });
}

#[test]
fn same_peer_malleated_signature_does_not_advance_quorum() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            net_id,
        ));

        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        let encoded = request.to_eth_abi(hash).unwrap();
        let (_signer, account_id, seed) = &state.networks[&net_id].ocw_keypairs[0];
        let secret = SecretKey::parse_slice(seed).unwrap();
        let public = PublicKey::from_secret_key(&secret);
        let sig_pair = secp256k1::sign(&eth::prepare_message(encoded.as_raw()), &secret);
        let signature_params = super::get_signature_params(&sig_pair);
        let malleated_signature = malleate_signature(&signature_params);
        let ocw_public = ecdsa::Public::from_raw(public.serialize_compressed());

        assert!(EthBridge::verify_message(
            encoded.as_raw(),
            &signature_params,
            &ocw_public,
            account_id,
        ));
        assert!(EthBridge::verify_message(
            encoded.as_raw(),
            &malleated_signature,
            &ocw_public,
            account_id,
        ));

        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(account_id.clone()),
            ocw_public.clone(),
            hash,
            signature_params,
            net_id,
        ));
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            1
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, &hash),
            Some(RequestStatus::Pending)
        ));

        System::reset_events();
        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(account_id.clone()),
            ocw_public,
            hash,
            malleated_signature,
            net_id,
        ));
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            1
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, &hash),
            Some(RequestStatus::Pending)
        ));
        assert!(last_event().is_none());
    });
}

#[test]
fn quorum_counts_distinct_approvers_even_if_signature_storage_drifts() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            net_id,
        ));

        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        let encoded = request.to_eth_abi(hash).unwrap();
        let first_peer = &state.networks[&net_id].ocw_keypairs[0];
        let first_secret = SecretKey::parse_slice(&first_peer.2).unwrap();
        let first_public = ecdsa::Public::from_raw(
            PublicKey::from_secret_key(&first_secret).serialize_compressed(),
        );
        let first_signature = super::get_signature_params(&secp256k1::sign(
            &eth::prepare_message(encoded.as_raw()),
            &first_secret,
        ));

        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(first_peer.1.clone()),
            first_public,
            hash,
            first_signature.clone(),
            net_id,
        ));

        let mut approvals = crate::RequestApprovals::<Runtime>::get(net_id, &hash);
        approvals.insert(malleate_signature(&first_signature));
        crate::RequestApprovals::<Runtime>::insert(net_id, &hash, approvals);
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).len(),
            2
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            1
        );

        let second_peer = &state.networks[&net_id].ocw_keypairs[1];
        let second_secret = SecretKey::parse_slice(&second_peer.2).unwrap();
        let second_public = ecdsa::Public::from_raw(
            PublicKey::from_secret_key(&second_secret).serialize_compressed(),
        );
        let second_signature = super::get_signature_params(&secp256k1::sign(
            &eth::prepare_message(encoded.as_raw()),
            &second_secret,
        ));

        System::reset_events();
        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(second_peer.1.clone()),
            second_public,
            hash,
            second_signature,
            net_id,
        ));

        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).len(),
            3
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            2
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, &hash),
            Some(RequestStatus::Pending)
        ));
        assert!(last_event().is_none());
    });
}

#[test]
fn failed_finalization_allows_fresh_approvals_after_resubmission() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let amount = 100u32.into();
        Assets::mint_to(&XOR.into(), &alice, &alice, 1_000_000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            amount,
            net_id,
        ));
        let (request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        let bridge_account = state.networks[&net_id].config.bridge_account_id.clone();
        Assets::unreserve(&XOR.into(), &bridge_account, amount).unwrap();
        System::reset_events();

        let encoded = request.to_eth_abi(hash).unwrap();
        let keypairs = &state.networks[&net_id].ocw_keypairs;
        let additional = if EthBridge::is_additional_signature_needed(net_id, &request) {
            1
        } else {
            0
        };
        let sigs_needed = majority(keypairs.len()) + additional;

        for (i, (_signer, account_id, seed)) in keypairs.iter().enumerate().take(sigs_needed) {
            let secret = SecretKey::parse_slice(seed).unwrap();
            let public = PublicKey::from_secret_key(&secret);
            let msg = eth::prepare_message(encoded.as_raw());
            let sig_pair = secp256k1::sign(&msg, &secret);
            let signature_params = super::get_signature_params(&sig_pair);
            assert_ok!(EthBridge::approve_request(
                RuntimeOrigin::signed(account_id.clone()),
                ecdsa::Public::from_raw(public.serialize_compressed()),
                hash,
                signature_params,
                net_id,
            ));
            if i + 1 == sigs_needed {
                let event_hash = match last_event().expect("event expected") {
                    RuntimeEvent::EthBridge(crate::Event::RequestFinalizationFailed(
                        event_hash,
                    ))
                    | RuntimeEvent::EthBridge(crate::Event::CancellationFailed(event_hash)) => {
                        event_hash
                    }
                    other => panic!("unexpected event: {:?}", other),
                };
                assert_eq!(event_hash, hash);
            }
        }

        match crate::RequestStatuses::<Runtime>::get(net_id, &hash) {
            Some(RequestStatus::Failed(_)) | Some(RequestStatus::Broken(_, _)) => {}
            other => panic!("unexpected status: {:?}", other),
        }
        assert!(
            !crate::RequestApprovals::<Runtime>::get(net_id, &hash).is_empty(),
            "approvals should remain until operators explicitly reset them"
        );
        assert!(
            !crate::RequestApprovers::<Runtime>::get(net_id, &hash).is_empty(),
            "approvers should remain until operators explicitly reset them"
        );

        assert_ok!(EthBridge::reset_request_signatures(
            RuntimeOrigin::root(),
            net_id,
            hash
        ));
        match last_event().expect("signatures cleared event expected") {
            RuntimeEvent::EthBridge(crate::Event::RequestSignaturesCleared(event_hash)) => {
                assert_eq!(event_hash, hash);
            }
            other => panic!("unexpected event: {:?}", other),
        }

        assert!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).is_empty(),
            "reset must clear approvals before retry"
        );
        assert!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).is_empty(),
            "reset must clear approvers before retry"
        );

        crate::RequestStatuses::<Runtime>::insert(net_id, &hash, RequestStatus::Pending);
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(hash));
        System::reset_events();

        let (_signer, account_id, seed) = &keypairs[0];
        let secret = SecretKey::parse_slice(seed).unwrap();
        let public = PublicKey::from_secret_key(&secret);
        let msg = eth::prepare_message(encoded.as_raw());
        let sig_pair = secp256k1::sign(&msg, &secret);
        let signature_params = super::get_signature_params(&sig_pair);
        assert_ok!(EthBridge::approve_request(
            RuntimeOrigin::signed(account_id.clone()),
            ecdsa::Public::from_raw(public.serialize_compressed()),
            hash,
            signature_params,
            net_id,
        ));
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, &hash).len(),
            1
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net_id, &hash).len(),
            1
        );
    });
}

#[test]
fn reset_request_signatures_requires_failed_status() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 1_000_000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100u32.into(),
            net_id,
        ));
        let (_request, hash) = last_outgoing_request(net_id).expect("outgoing request exists");
        assert_err!(
            EthBridge::reset_request_signatures(RuntimeOrigin::root(), net_id, hash),
            Error::RequestStatusNotResettable
        );

        // Force the request into Failed state and confirm the extrinsic succeeds afterward.
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            &hash,
            RequestStatus::Failed(Error::Cancelled.into()),
        );
        assert_ok!(EthBridge::reset_request_signatures(
            RuntimeOrigin::root(),
            net_id,
            hash
        ));
        match last_event().expect("signatures cleared event expected") {
            RuntimeEvent::EthBridge(crate::Event::RequestSignaturesCleared(event_hash)) => {
                assert_eq!(event_hash, hash);
            }
            other => panic!("unexpected event: {:?}", other),
        }
    });
}

#[test]
fn requests_queue_respects_limit() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let max = MaxRequestsPerQueueConst::get() as usize;
        Assets::mint_to(&XOR.into(), &alice, &alice, 10_000_000u32.into()).unwrap();

        for _ in 0..max {
            assert_ok!(EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                XOR.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                10u32.into(),
                net_id,
            ));
        }

        assert_err!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                XOR.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                10u32.into(),
                net_id,
            ),
            Error::RequestsQueueFull
        );
        assert_eq!(crate::Requests::<Runtime>::iter_prefix(net_id).count(), max);
        assert_eq!(crate::RequestsQueue::<Runtime>::get(net_id).len(), max);
    });
}

#[test]
fn should_block_v1_signature_domain_requests_without_toggle() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&PSWAP.into(), &alice, &alice, 1000).unwrap();

        crate::BridgeSignatureVersions::<Runtime>::insert(
            net_id,
            crate::BridgeSignatureVersion::V1,
        );
        assert_err!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice),
                PSWAP.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                10,
                net_id,
            ),
            Error::WeakLegacySigningDisabled
        );
    });
}

#[test]
fn should_use_legacy_master_contract_path_for_val_only() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let to = EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap();
        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&alice);
        let timepoint = bridge_multisig::Pallet::<Runtime>::thischain_timepoint();

        let xor_req = OutgoingTransfer::<Runtime> {
            from: alice.clone(),
            to,
            asset_id: XOR.into(),
            amount: 10,
            nonce,
            network_id: net_id,
            timepoint,
        };
        let val_req = OutgoingTransfer::<Runtime> {
            from: alice,
            to,
            asset_id: VAL.into(),
            amount: 10,
            nonce,
            network_id: net_id,
            timepoint,
        };

        assert!(!xor_req.uses_legacy_master_contract_path());
        assert!(val_req.uses_legacy_master_contract_path());
    });
}
