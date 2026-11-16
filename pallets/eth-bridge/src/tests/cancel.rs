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
use super::{Assets, Error, EthBridge};
use crate::contract::{functions, FUNCTIONS, RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID};
use crate::offchain::SignatureParams;
use crate::requests::{
    encode_outgoing_request_eth_call, AssetKind, IncomingAddToken, IncomingCancelOutgoingRequest,
    IncomingChangePeers, IncomingMetaRequestKind, IncomingMigrate, IncomingPrepareForMigration,
    IncomingRequest, IncomingTransfer, OffchainRequest, OutgoingAddAsset, OutgoingAddPeer,
    OutgoingAddPeerCompat, OutgoingAddToken, OutgoingMigrate, OutgoingPrepareForMigration,
    OutgoingRemovePeer, OutgoingRemovePeerCompat, OutgoingRequest, OutgoingTransfer, RequestStatus,
};
use crate::tests::mock::{get_account_id_from_seed, ExtBuilder};
use crate::tests::{
    approve_last_request, assert_incoming_request_registration_failed, last_outgoing_request,
    request_incoming, ETH_NETWORK_ID,
};
use crate::types::{Transaction, TransactionReceipt};
use crate::{AssetConfig, EthAddress};
use common::{
    AssetInfoProvider, AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION, DOT, KSM, PSWAP, USDT,
    VAL, XOR,
};
use frame_support::sp_runtime::{DispatchResult, TransactionOutcome};
use frame_support::traits::Currency;
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;
use sp_core::crypto::AccountId32;
use sp_core::{sr25519, H160, H256};
use std::str::FromStr;

#[test]
fn failed_duplicate_withdrawal_cannot_refund_from_another_pending_reserve() {
    let (mut ext, state) = ExtBuilder::default().build();
    let _ = FUNCTIONS.get_or_init(functions);
    ext.execute_with(|| {
        let net = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        for owner in [&alice, &bob] {
            Assets::mint_to(&XOR.into(), &alice, owner, 100).unwrap();
        }
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::repeat_byte(9),
            100,
            net
        ));
        let (outgoing, hash) = approve_last_request(&state, net).unwrap();
        // The first transfer's own reserve was released at quorum. A second
        // pending transfer makes the shared reserve sufficient for the old,
        // unsafe cancellation to refund an already redeemable withdrawal.
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(bob.clone()),
            XOR.into(),
            EthAddress::repeat_byte(10),
            100,
            net
        ));
        let (_, pending_hash) = last_outgoing_request(net).unwrap();
        let calldata = encode_outgoing_request_eth_call::<Runtime>(
            *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID.get().unwrap(),
            &outgoing,
            hash,
        )
        .unwrap();
        let signatures = crate::RequestApprovals::<Runtime>::get(net, hash);
        let approvers = crate::RequestApprovers::<Runtime>::get(net, hash);
        let incoming = IncomingRequest::CancelOutgoingRequest(IncomingCancelOutgoingRequest {
            outgoing_request: outgoing.clone(),
            outgoing_request_hash: hash,
            initial_request_hash: H256::repeat_byte(11),
            tx_input: calldata.clone(),
            author: alice.clone(),
            tx_hash: H256::repeat_byte(12),
            at_height: 1,
            timepoint: Default::default(),
            network_id: net,
        });
        let bridge = &state.networks[&net].config.bridge_account_id;
        let reserved = Assets::total_balance(&XOR.into(), bridge).unwrap()
            - Assets::free_balance(&XOR.into(), bridge).unwrap();
        assert_eq!(reserved, 100);
        // Reproduce the old refund primitive in a rolled-back storage layer:
        // its fungible reserve belongs to Bob, not the approved withdrawal.
        frame_support::storage::with_transaction(|| {
            assert_ok!(outgoing.cancel());
            assert_eq!(Assets::total_balance(&XOR.into(), &alice).unwrap(), 100);
            assert_eq!(
                Assets::total_balance(&XOR.into(), bridge).unwrap()
                    - Assets::free_balance(&XOR.into(), bridge).unwrap(),
                0
            );
            TransactionOutcome::Rollback(DispatchResult::Ok(()))
        })
        .unwrap();
        assert_noop!(incoming.validate(), Error::PublicApprovalsRequireSettlement);
        assert_noop!(incoming.prepare(), Error::PublicApprovalsRequireSettlement);
        assert_noop!(incoming.finalize(), Error::PublicApprovalsRequireSettlement);
        assert_eq!(
            incoming.check_existence(),
            Err(Error::PublicApprovalsRequireSettlement)
        );
        // Also cover cancellations already prepared by the old runtime.
        crate::RequestStatuses::<Runtime>::insert(net, hash, RequestStatus::Frozen);
        assert_noop!(incoming.finalize(), Error::PublicApprovalsRequireSettlement);
        assert_ok!(incoming.cancel());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::ApprovalsReady)
        );
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net, pending_hash),
            Some(RequestStatus::Pending)
        );
        assert_eq!(Assets::total_balance(&XOR.into(), &alice).unwrap(), 0);
        assert_eq!(Assets::total_balance(&XOR.into(), &bob).unwrap(), 0);
        assert_eq!(
            Assets::total_balance(&XOR.into(), bridge).unwrap()
                - Assets::free_balance(&XOR.into(), bridge).unwrap(),
            reserved
        );
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net, hash),
            signatures
        );
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net, hash),
            approvers
        );
        // Nothing revoked or altered the signed withdrawal. If it has not yet
        // succeeded remotely, its exact original calldata remains retryable.
        assert_eq!(
            encode_outgoing_request_eth_call::<Runtime>(
                *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID.get().unwrap(),
                &outgoing,
                hash
            )
            .unwrap(),
            calldata
        );
        crate::RequestStatuses::<Runtime>::insert(net, hash, RequestStatus::Done);
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_ok!(incoming.cancel());
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
    });
}

#[test]
fn should_fail_cancel_ready_outgoing_request_with_wrong_approvals() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        // Sending request part
        Assets::mint_to(&XOR.into(), &alice, &alice, 100u32.into()).unwrap();
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            100u32.into()
        );
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(Assets::total_balance(&XOR.into(), &alice).unwrap(), 0);
        let (outgoing_req, outgoing_req_hash) =
            approve_last_request(&state, net_id).expect("request wasn't approved");

        // Cancelling request part
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let request_hash = request_incoming(
            alice.clone(),
            tx_hash,
            IncomingMetaRequestKind::CancelOutgoingRequest.into(),
            net_id,
        )
        .unwrap();
        let tx_input = encode_outgoing_request_eth_call::<Runtime>(
            *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID.get().unwrap(),
            &outgoing_req,
            outgoing_req_hash,
        )
        .unwrap();
        let incoming_transfer =
            IncomingRequest::CancelOutgoingRequest(crate::IncomingCancelOutgoingRequest {
                outgoing_request: outgoing_req.clone(),
                outgoing_request_hash: outgoing_req_hash,
                initial_request_hash: request_hash,
                tx_input: tx_input.clone(),
                author: alice.clone(),
                tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: ETH_NETWORK_ID,
            });

        // Insert some signature
        crate::RequestApprovals::<Runtime>::mutate(net_id, outgoing_req_hash, |v| {
            v.insert(SignatureParams {
                r: [1; 32],
                s: [1; 32],
                v: 0,
            })
        });
        assert_incoming_request_registration_failed(
            &state,
            incoming_transfer.clone(),
            Error::PublicApprovalsRequireSettlement,
        )
        .unwrap();
        assert_eq!(Assets::total_balance(&XOR.into(), &alice).unwrap(), 0);
    });
}

#[test]
fn should_fail_cancel_unfinished_outgoing_request() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        // Sending request part
        Assets::mint_to(&XOR.into(), &alice, &alice, 100u32.into()).unwrap();
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            100u32.into()
        );
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(Assets::total_balance(&XOR.into(), &alice).unwrap(), 0);
        let (outgoing_req, outgoing_req_hash) =
            last_outgoing_request(net_id).expect("request wasn't found");

        // Cancelling request part
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let request_hash = request_incoming(
            alice.clone(),
            tx_hash,
            IncomingMetaRequestKind::CancelOutgoingRequest.into(),
            net_id,
        )
        .unwrap();
        let tx_input = encode_outgoing_request_eth_call::<Runtime>(
            *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID.get().unwrap(),
            &outgoing_req,
            outgoing_req_hash,
        )
        .unwrap();
        let incoming_transfer =
            IncomingRequest::CancelOutgoingRequest(crate::IncomingCancelOutgoingRequest {
                outgoing_request: outgoing_req,
                outgoing_request_hash: outgoing_req_hash,
                initial_request_hash: request_hash,
                tx_input,
                author: alice.clone(),
                tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: ETH_NETWORK_ID,
            });
        assert_incoming_request_registration_failed(
            &state,
            incoming_transfer.clone(),
            Error::PublicApprovalsRequireSettlement,
        )
        .unwrap();
        assert_eq!(Assets::total_balance(&XOR.into(), &alice).unwrap(), 0);
    });
}

#[test]
fn should_cancel_outgoing_prepared_requests() {
    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::default();
    builder.add_network(
        vec![
            AssetConfig::Thischain { id: PSWAP.into() },
            AssetConfig::Sidechain {
                id: XOR.into(),
                sidechain_id: sp_core::H160::from_str("41fd72257597aa14c7231a7b1aaa29fce868f677")
                    .unwrap(),
                owned: true,
                precision: DEFAULT_BALANCE_PRECISION,
            },
            AssetConfig::Sidechain {
                id: VAL.into(),
                sidechain_id: sp_core::H160::from_str("3f9feac97e5feb15d8bf98042a9a01b515da3dfb")
                    .unwrap(),
                owned: true,
                precision: DEFAULT_BALANCE_PRECISION,
            },
        ],
        Some(vec![
            (XOR.into(), common::balance!(350000)),
            (VAL.into(), common::balance!(33900000)),
        ]),
        Some(5),
        Default::default(),
    );
    let (mut ext, state) = builder.build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bridge_acc = &state.networks[&net_id].config.bridge_account_id;
        Assets::register_asset_id(
            alice.clone(),
            DOT,
            AssetSymbol::from_str("DOT").unwrap(),
            AssetName::from_str("Polkadot").unwrap(),
            DEFAULT_BALANCE_PRECISION,
            0,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        Assets::mint_to(&XOR.into(), &alice, &alice, 100u32.into()).unwrap();
        Assets::mint_to(&XOR.into(), &alice, bridge_acc, 100u32.into()).unwrap();
        let ocw0_account_id = &state.networks[&net_id].ocw_keypairs[0].1;
        let extra_peer = AccountId32::new([12u8; 32]);
        let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&extra_peer, 1u32.into());
        assert_ok!(EthBridge::force_add_peer(
            RuntimeOrigin::root(),
            extra_peer,
            EthAddress::from([12u8; 20]),
            net_id,
        ));
        // Paris (preparation requests, testable request).
        let test_acc = AccountId32::new([10u8; 32]);
        let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&test_acc, 1u32.into());
        let requests: Vec<(Vec<OffchainRequest<Runtime>>, OffchainRequest<Runtime>)> = vec![
            (
                vec![],
                OutgoingTransfer {
                    from: alice.clone(),
                    to: EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                    asset_id: XOR.into(),
                    amount: 1_u32.into(),
                    nonce: 0,
                    network_id: net_id,
                    timepoint: Default::default(),
                }
                .into(),
            ),
            (
                vec![],
                OutgoingAddAsset {
                    author: alice.clone(),
                    asset_id: DOT.into(),
                    nonce: 0,
                    network_id: net_id,
                    timepoint: Default::default(),
                }
                .into(),
            ),
            (
                vec![],
                OutgoingAddToken {
                    author: alice.clone(),
                    token_address: EthAddress::from([100u8; 20]),
                    name: "TEST".into(),
                    symbol: "TST".into(),
                    decimals: DEFAULT_BALANCE_PRECISION,
                    nonce: 0,
                    network_id: net_id,
                    timepoint: Default::default(),
                }
                .into(),
            ),
            (
                vec![],
                OutgoingAddPeer {
                    author: alice.clone(),
                    peer_address: EthAddress::from([10u8; 20]),
                    nonce: 0,
                    network_id: net_id,
                    peer_account_id: test_acc.clone(),
                    timepoint: Default::default(),
                }
                .into(),
            ),
            (
                vec![],
                OutgoingAddPeer {
                    author: alice.clone(),
                    peer_address: EthAddress::from([10u8; 20]),
                    nonce: 0,
                    network_id: net_id + 1,
                    peer_account_id: test_acc.clone(),
                    timepoint: Default::default(),
                }
                .into(),
            ),
            (
                vec![OutgoingAddPeer {
                    author: alice.clone(),
                    peer_address: EthAddress::from([10u8; 20]),
                    nonce: 0,
                    network_id: net_id,
                    peer_account_id: test_acc.clone(),
                    timepoint: Default::default(),
                }
                .into()],
                OutgoingAddPeerCompat {
                    author: alice.clone(),
                    peer_address: EthAddress::from([10u8; 20]),
                    nonce: 0,
                    network_id: net_id,
                    peer_account_id: test_acc.clone(),
                    timepoint: Default::default(),
                }
                .into(),
            ),
            (
                vec![
                    OutgoingAddPeer {
                        author: alice.clone(),
                        peer_address: EthAddress::from([10u8; 20]),
                        nonce: 0,
                        network_id: net_id + 1,
                        peer_account_id: test_acc.clone(),
                        timepoint: Default::default(),
                    }
                    .into(),
                    IncomingChangePeers {
                        peer_account_id: Some(test_acc.clone()),
                        peer_address: EthAddress::from([10u8; 20]),
                        removed: false,
                        author: alice.clone(),
                        tx_hash: H256([11; 32]),
                        at_height: 0,
                        timepoint: Default::default(),
                        network_id: net_id + 1,
                    }
                    .into(),
                ],
                OutgoingRemovePeer {
                    author: alice.clone(),
                    peer_address: EthAddress::from([10u8; 20]),
                    nonce: 0,
                    network_id: net_id + 1,
                    peer_account_id: test_acc.clone(),
                    timepoint: Default::default(),
                    compat_hash: None,
                }
                .into(),
            ),
            (
                vec![OutgoingRemovePeer {
                    author: alice.clone(),
                    peer_address: crate::PeerAddress::<Runtime>::get(&net_id, &ocw0_account_id),
                    nonce: 0,
                    network_id: net_id,
                    peer_account_id: ocw0_account_id.clone(),
                    timepoint: Default::default(),
                    compat_hash: None,
                }
                .into()],
                OutgoingRemovePeerCompat {
                    author: alice.clone(),
                    peer_address: crate::PeerAddress::<Runtime>::get(&net_id, &ocw0_account_id),
                    nonce: 0,
                    network_id: net_id,
                    peer_account_id: ocw0_account_id.clone(),
                    timepoint: Default::default(),
                }
                .into(),
            ),
            (
                vec![],
                OutgoingPrepareForMigration {
                    author: alice.clone(),
                    nonce: 0,
                    network_id: net_id,
                    timepoint: Default::default(),
                }
                .into(),
            ),
            (
                vec![
                    OutgoingPrepareForMigration {
                        author: alice.clone(),
                        nonce: 0,
                        network_id: net_id,
                        timepoint: Default::default(),
                    }
                    .into(),
                    IncomingPrepareForMigration {
                        author: alice.clone(),
                        tx_hash: [1u8; 32].into(),
                        at_height: 0,
                        timepoint: Default::default(),
                        network_id: net_id,
                    }
                    .into(),
                ],
                OutgoingMigrate {
                    author: alice.clone(),
                    new_contract_address: Default::default(),
                    erc20_native_tokens: vec![],
                    nonce: 0,
                    network_id: net_id,
                    timepoint: Default::default(),
                    new_signature_version: crate::BridgeSignatureVersion::V1,
                }
                .into(),
            ),
        ];
        for (preparations, request) in requests {
            frame_support::storage::with_transaction(|| {
                for preparation_request in &preparations {
                    preparation_request.validate().unwrap();
                    preparation_request.prepare().unwrap();
                    match preparation_request {
                        // Do not finalize add/remove peer requests for ethereum network,
                        // because of XOR and VAL contracts (see `OutgoingAddPeerCompat`).
                        OffchainRequest::Outgoing(OutgoingRequest::AddPeer(req), ..)
                            if req.network_id == ETH_NETWORK_ID => {}
                        OffchainRequest::Outgoing(OutgoingRequest::RemovePeer(req), ..)
                            if req.network_id == ETH_NETWORK_ID => {}
                        _ => preparation_request.finalize().unwrap(),
                    }
                }
                // Save the current storage root hash, apply transaction preparation,
                // cancel it and compare with the final root hash.
                frame_system::Pallet::<Runtime>::reset_events();
                let state_hash_before = sp_io::storage::root(sp_runtime::StateVersion::V1);
                println!("{:?}", request);
                request.validate().unwrap();
                request.prepare().unwrap();
                request.cancel().unwrap();
                frame_system::Pallet::<Runtime>::reset_events();
                let state_hash_after = sp_io::storage::root(sp_runtime::StateVersion::V1);
                assert_eq!(state_hash_before, state_hash_after);
                TransactionOutcome::Rollback(DispatchResult::Ok(()))
            })
            .unwrap();
        }
    });
}

#[test]
fn should_cancel_incoming_prepared_requests() {
    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::default();
    builder.add_currency(net_id, AssetConfig::Thischain { id: DOT.into() });
    builder.add_currency(
        net_id,
        AssetConfig::Sidechain {
            id: USDT.into(),
            sidechain_id: H160(hex!("dAC17F958D2ee523a2206206994597C13D831ec7")),
            owned: false,
            precision: DEFAULT_BALANCE_PRECISION,
        },
    );
    let (mut ext, state) = builder.build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bridge_acc = &state.networks[&net_id].config.bridge_account_id;
        Assets::mint_to(&XOR.into(), &alice, &alice, 100u32.into()).unwrap();
        Assets::mint_to(&XOR.into(), &alice, bridge_acc, 100u32.into()).unwrap();
        Assets::mint_to(&DOT.into(), &alice, bridge_acc, 100u32.into()).unwrap();
        // Paris (preparation requests, testable request).
        let requests: Vec<(Vec<OffchainRequest<Runtime>>, OffchainRequest<Runtime>)> = vec![
            (
                vec![],
                IncomingTransfer {
                    from: EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                    to: alice.clone(),
                    asset_id: XOR.into(),
                    asset_kind: AssetKind::SidechainOwned,
                    amount: 1_u32.into(),
                    author: alice.clone(),
                    tx_hash: Default::default(),
                    network_id: net_id,
                    timepoint: Default::default(),
                    at_height: 0,
                    should_take_fee: false,
                }
                .into(),
            ),
            (
                vec![],
                IncomingTransfer {
                    from: EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                    to: alice.clone(),
                    asset_id: DOT.into(),
                    asset_kind: AssetKind::Thischain,
                    amount: 1_u32.into(),
                    author: alice.clone(),
                    tx_hash: Default::default(),
                    network_id: net_id,
                    timepoint: Default::default(),
                    at_height: 0,
                    should_take_fee: false,
                }
                .into(),
            ),
            (
                vec![],
                IncomingTransfer {
                    from: EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                    to: alice.clone(),
                    asset_id: USDT.into(),
                    asset_kind: AssetKind::Sidechain,
                    amount: 1_u32.into(),
                    author: alice.clone(),
                    tx_hash: Default::default(),
                    network_id: net_id,
                    timepoint: Default::default(),
                    at_height: 0,
                    should_take_fee: false,
                }
                .into(),
            ),
            (
                vec![],
                IncomingAddToken {
                    token_address: EthAddress::from([100; 20]),
                    asset_id: KSM.into(),
                    precision: DEFAULT_BALANCE_PRECISION,
                    symbol: Default::default(),
                    name: Default::default(),
                    author: alice.clone(),
                    tx_hash: Default::default(),
                    network_id: net_id,
                    timepoint: Default::default(),
                    at_height: 0,
                }
                .into(),
            ),
            (
                vec![],
                IncomingPrepareForMigration {
                    author: alice.clone(),
                    tx_hash: Default::default(),
                    network_id: net_id,
                    timepoint: Default::default(),
                    at_height: 0,
                }
                .into(),
            ),
            (
                vec![
                    IncomingPrepareForMigration {
                        author: alice.clone(),
                        tx_hash: Default::default(),
                        network_id: net_id,
                        timepoint: Default::default(),
                        at_height: 0,
                    }
                    .into(),
                    OutgoingMigrate {
                        author: alice.clone(),
                        new_contract_address: Default::default(),
                        erc20_native_tokens: vec![],
                        nonce: Default::default(),
                        network_id: net_id,
                        timepoint: Default::default(),
                        new_signature_version: crate::BridgeSignatureVersion::V1,
                    }
                    .into(),
                ],
                IncomingMigrate {
                    new_contract_address: Default::default(),
                    author: alice.clone(),
                    tx_hash: Default::default(),
                    network_id: net_id,
                    timepoint: Default::default(),
                    at_height: 0,
                }
                .into(),
            ),
        ];
        for (preparations, request) in requests {
            frame_support::storage::with_transaction(|| {
                for preparation_request in preparations {
                    preparation_request.prepare().unwrap();
                    preparation_request.finalize().unwrap();
                }
                // Save the current storage root hash, apply transaction preparation,
                // cancel it and compare with the final root hash.
                frame_system::Pallet::<Runtime>::reset_events();
                let state_hash_before = sp_io::storage::root(sp_runtime::StateVersion::V1);
                request.prepare().unwrap();
                request.cancel().unwrap();
                frame_system::Pallet::<Runtime>::reset_events();
                let state_hash_after = sp_io::storage::root(sp_runtime::StateVersion::V1);
                assert_eq!(state_hash_before, state_hash_after);
                TransactionOutcome::Rollback(DispatchResult::Ok(()))
            })
            .unwrap();
        }
    });
}

#[test]
fn retained_cancellation_unprepare_restores_withdrawal_without_new_admission() {
    let _ = FUNCTIONS.get_or_init(functions);
    let (mut ext, _state) = ExtBuilder::default().build();

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
        let (outgoing_request, outgoing_request_hash) =
            last_outgoing_request(net_id).expect("outgoing request should exist");
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            outgoing_request_hash,
            RequestStatus::ApprovalsReady,
        );
        let tx_input = encode_outgoing_request_eth_call::<Runtime>(
            *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID.get().unwrap(),
            &outgoing_request,
            outgoing_request_hash,
        )
        .unwrap();
        let request = IncomingRequest::CancelOutgoingRequest(IncomingCancelOutgoingRequest {
            outgoing_request,
            outgoing_request_hash,
            initial_request_hash: H256::repeat_byte(0x55),
            tx_input,
            author: alice,
            tx_hash: H256::repeat_byte(0x77),
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });

        let state_hash_before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_noop!(request.prepare(), Error::PublicApprovalsRequireSettlement);
        // A retained old cancellation may already have frozen the request.
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            outgoing_request_hash,
            RequestStatus::Frozen,
        );
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, outgoing_request_hash),
            Some(RequestStatus::Frozen)
        );
        request.cancel().unwrap();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, outgoing_request_hash),
            Some(RequestStatus::ApprovalsReady)
        );
        let state_hash_after = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_eq!(state_hash_before, state_hash_after);
    });
}

#[test]
fn should_reject_manual_cancel_outgoing_meta_request() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");

        assert_noop!(
            EthBridge::request_from_sidechain(
                RuntimeOrigin::signed(alice),
                H256::repeat_byte(0x90),
                IncomingMetaRequestKind::CancelOutgoingRequest.into(),
                net_id
            ),
            Error::Unavailable
        );
    });
}

#[test]
fn receipt_cancellation_rejects_gas_limit_receipts() {
    let (mut ext, _state) = ExtBuilder::default().build();

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
        let (outgoing_request, outgoing_request_hash) =
            last_outgoing_request(net_id).expect("outgoing request should exist");
        let tx_hash = H256::repeat_byte(0x91);
        let contract = crate::BridgeContractAddress::<Runtime>::get(net_id);
        let request = IncomingRequest::CancelOutgoingRequest(IncomingCancelOutgoingRequest {
            outgoing_request,
            outgoing_request_hash,
            initial_request_hash: H256::repeat_byte(0x92),
            tx_input: vec![],
            author: alice,
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });

        push_global_json_rpc_response(TransactionReceipt {
            transaction_hash: crate::types::H256(tx_hash.0),
            to: Some(crate::types::H160(contract.0)),
            status: Some(0u64.into()),
            gas_used: Some(100u64.into()),
            ..Default::default()
        });
        push_global_json_rpc_response(Transaction {
            hash: crate::types::H256(tx_hash.0),
            to: Some(crate::types::H160(contract.0)),
            gas: 100u64.into(),
            ..Default::default()
        });

        assert_eq!(
            request.check_existence(),
            Err(Error::PublicApprovalsRequireSettlement)
        );
    });
}

#[test]
fn receipt_cancellation_rejects_other_failed_receipts() {
    let (mut ext, _state) = ExtBuilder::default().build();

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
        let (outgoing_request, outgoing_request_hash) =
            last_outgoing_request(net_id).expect("outgoing request should exist");
        let tx_hash = H256::repeat_byte(0x93);
        let contract = crate::BridgeContractAddress::<Runtime>::get(net_id);
        let request = IncomingRequest::CancelOutgoingRequest(IncomingCancelOutgoingRequest {
            outgoing_request,
            outgoing_request_hash,
            initial_request_hash: H256::repeat_byte(0x94),
            tx_input: vec![],
            author: alice,
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });

        push_global_json_rpc_response(TransactionReceipt {
            transaction_hash: crate::types::H256(tx_hash.0),
            to: Some(crate::types::H160(contract.0)),
            status: Some(0u64.into()),
            gas_used: Some(99u64.into()),
            ..Default::default()
        });
        push_global_json_rpc_response(Transaction {
            hash: crate::types::H256(tx_hash.0),
            to: Some(crate::types::H160(contract.0)),
            gas: 100u64.into(),
            ..Default::default()
        });

        assert_eq!(
            request.check_existence(),
            Err(Error::PublicApprovalsRequireSettlement)
        );
    });
}

#[test]
fn receipt_cancellation_rejects_successful_receipts() {
    let (mut ext, _state) = ExtBuilder::default().build();

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
        let (outgoing_request, outgoing_request_hash) =
            last_outgoing_request(net_id).expect("outgoing request should exist");
        let tx_hash = H256::repeat_byte(0x95);
        let contract = crate::BridgeContractAddress::<Runtime>::get(net_id);
        let request = IncomingRequest::CancelOutgoingRequest(IncomingCancelOutgoingRequest {
            outgoing_request,
            outgoing_request_hash,
            initial_request_hash: H256::repeat_byte(0x96),
            tx_input: vec![],
            author: alice,
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });

        push_global_json_rpc_response(TransactionReceipt {
            transaction_hash: crate::types::H256(tx_hash.0),
            to: Some(crate::types::H160(contract.0)),
            status: Some(1u64.into()),
            ..Default::default()
        });

        assert_eq!(
            request.check_existence(),
            Err(Error::PublicApprovalsRequireSettlement)
        );
    });
}

#[test]
fn receipt_cancellation_rejects_missing_gas_used() {
    let (mut ext, _state) = ExtBuilder::default().build();

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
        let (outgoing_request, outgoing_request_hash) =
            last_outgoing_request(net_id).expect("outgoing request should exist");
        let tx_hash = H256::repeat_byte(0x97);
        let contract = crate::BridgeContractAddress::<Runtime>::get(net_id);
        let request = IncomingRequest::CancelOutgoingRequest(IncomingCancelOutgoingRequest {
            outgoing_request,
            outgoing_request_hash,
            initial_request_hash: H256::repeat_byte(0x98),
            tx_input: vec![],
            author: alice,
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });

        push_global_json_rpc_response(TransactionReceipt {
            transaction_hash: crate::types::H256(tx_hash.0),
            to: Some(crate::types::H160(contract.0)),
            status: Some(0u64.into()),
            gas_used: None,
            ..Default::default()
        });
        push_global_json_rpc_response(Transaction {
            hash: crate::types::H256(tx_hash.0),
            to: Some(crate::types::H160(contract.0)),
            gas: 100u64.into(),
            ..Default::default()
        });

        assert_eq!(
            request.check_existence(),
            Err(Error::PublicApprovalsRequireSettlement)
        );
    });
}
