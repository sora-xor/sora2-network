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
use crate::offchain::MAX_LARGE_JSON_RPC_RESPONSE_BYTES;
use crate::requests::{
    IncomingMetaRequestKind, IncomingRequest, IncomingRequestKind, IncomingTransactionRequestKind,
    RequestStatus,
};
use crate::tests::mock::{get_account_id_from_seed, ExtBuilder};
use crate::tests::{last_outgoing_request, last_request, Assets, ETH_NETWORK_ID};
use crate::types::{Log, TransactionReceipt};
use crate::{
    types, AssetConfig, EthAddress, CONFIRMATION_INTERVAL, MAX_FAILED_SEND_SIGNED_TX_RETRIES,
    MAX_GET_LOGS_ITEMS, MAX_PENDING_TX_BLOCKS_PERIOD,
    OUTGOING_APPROVAL_FAILURE_FAILED_SEND_SIGNED_TX, RE_HANDLE_TXS_PERIOD, STORAGE_ETH_NODE_PARAMS,
    STORAGE_LOCAL_PEER_READY_KEY, STORAGE_OUTGOING_APPROVAL_FAILURES_KEY,
    STORAGE_OUTGOING_ZERO_APPROVAL_REQUESTS_KEY, STORAGE_PEER_MARKER_KEY, STORAGE_PEER_SECRET_KEY,
    STORAGE_PENDING_TRANSACTIONS_KEY, SUBSTRATE_HANDLE_BLOCK_COUNT_PER_BLOCK,
    SUBSTRATE_MAX_BLOCK_NUM_EXPECTING_UNTIL_FINALIZATION, ZERO_APPROVAL_OUTGOING_RETRY_PERIOD,
};
use codec::Encode;
use common::{DEFAULT_BALANCE_PRECISION, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use hex_literal::hex;
use rustc_hex::ToHex;
use sp_core::offchain::OffchainStorage;
use sp_core::{sr25519, H256};
use sp_runtime::offchain::storage::StorageValueRef;
use sp_runtime::DispatchError;
use std::str::FromStr;

fn eth_call_bool(value: bool) -> types::Bytes {
    types::Bytes(ethabi::encode(&[ethabi::Token::Bool(value)]))
}

fn raw_eth_call_result(bytes: &[u8]) -> types::Bytes {
    types::Bytes(bytes.to_vec())
}

const ETH_CALL_TRUE_JSON_RPC_RESPONSE: &str = r#"{"jsonrpc":"2.0","result":"0x0000000000000000000000000000000000000000000000000000000000000001","id":0}"#;

fn insert_pending_sidechain_multisig(
    multisig_account: &AccountId,
    call_hash_byte: u8,
    sidechain_height: u64,
    timepoint_index: u32,
) {
    bridge_multisig::Multisigs::<Runtime>::insert(
        multisig_account,
        [call_hash_byte; 32],
        bridge_multisig::Multisig {
            when: bridge_multisig::BridgeTimepoint {
                height: bridge_multisig::MultiChainHeight::Sidechain(sidechain_height),
                index: timepoint_index,
            },
            deposit: 0,
            depositor: multisig_account.clone(),
            approvals: vec![multisig_account.clone()],
        },
    );
}

fn pending_multisig_rehandle_key(
    prefix: &str,
    call_hash_byte: u8,
    sidechain_height: u64,
    timepoint_index: u32,
) -> String {
    format!(
        "{}-v2-{:?}-{}-{}-{}",
        prefix,
        ETH_NETWORK_ID,
        [call_hash_byte; 32].to_hex::<String>(),
        sidechain_height,
        timepoint_index,
    )
}

fn insert_failed_incoming_import(network_id: u32, tx_hash: H256) {
    let timepoint = bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(0, 0);
    let load_incoming_request = crate::requests::LoadIncomingRequest::Transaction(
        crate::requests::LoadIncomingTransactionRequest::new(
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            tx_hash,
            timepoint,
            IncomingTransactionRequestKind::Transfer,
            network_id,
        ),
    );
    let import_call: RuntimeCall = crate::Call::<Runtime>::import_incoming_request {
        load_incoming_request,
        incoming_request_result: Err(Error::EthTransactionIsFailed.into()),
    }
    .into();
    let call = bridge_multisig::Call::<Runtime>::as_multi_threshold_1 {
        id: crate::BridgeAccount::<Runtime>::get(network_id).unwrap(),
        call: Box::new(import_call),
        timepoint,
    };
    let failed = std::collections::BTreeMap::from([(
        tx_hash,
        crate::offchain::SignedTransactionData::<Runtime>::new(tx_hash, None, call),
    )]);
    StorageValueRef::persistent(crate::STORAGE_FAILED_PENDING_TRANSACTIONS_KEY).set(&failed);
}

fn handle_substrate_at_finalized_height(height: BlockNumber) {
    // Isolate secondary-queue recovery from primary-queue retries: these finalized blocks
    // have already been scanned, so this worker only observes the finalized tip.
    StorageValueRef::persistent(crate::STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY).set(&(height + 1));
    push_global_json_rpc_response(H256::zero());
    push_global_json_rpc_response(types::SubstrateHeaderLimited {
        parent_hash: Default::default(),
        number: height.into(),
        state_root: Default::default(),
        extrinsics_root: Default::default(),
        digest: (),
    });
    assert_ok!(EthBridge::handle_substrate(), height);
}

#[test]
fn ocw_should_not_handle_non_finalized_outgoing_request() {
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
        state.run_next_offchain_with_params(0, 0, true);
        let hash = last_outgoing_request(net_id).unwrap().1;
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, hash).len(),
            0
        );
    });
}

#[test]
fn ocw_mark_as_done_targets_original_outgoing_hash() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Thischain { id: XOR.into() }],
        None,
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let outgoing_hash = H256::repeat_byte(0x42);

        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            outgoing_hash,
            RequestStatus::ApprovalsReady,
        );
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            outgoing_hash,
            IncomingRequestKind::Meta(IncomingMetaRequestKind::MarkAsDone),
            net_id
        ));
        let load_hash = last_request(net_id).unwrap().hash();
        assert_ne!(load_hash, outgoing_hash);

        state.push_response(types::U64::from(777u64));
        // Match the wire response returned by an Ethereum node for an ABI-encoded `bool true`.
        state.push_response_raw(ETH_CALL_TRUE_JSON_RPC_RESPONSE.as_bytes().to_vec());
        state.run_next_offchain_and_dispatch_txs();

        let incoming_hash = crate::LoadToIncomingRequestHash::<Runtime>::get(net_id, load_hash);
        assert_ne!(incoming_hash, H256::zero());
        let incoming = crate::Requests::<Runtime>::get(net_id, incoming_hash)
            .unwrap()
            .into_incoming()
            .unwrap()
            .0;
        let IncomingRequest::MarkAsDone(mark_as_done) = incoming else {
            panic!("expected MarkAsDone incoming request");
        };
        assert_eq!(mark_as_done.outgoing_request_hash, outgoing_hash);
        assert_eq!(mark_as_done.initial_request_hash, load_hash);
        assert_eq!(mark_as_done.author, alice);
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, load_hash),
            Some(RequestStatus::Done)
        );
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, incoming_hash),
            Some(RequestStatus::Pending)
        );
    });
}

#[test]
fn load_is_used_decodes_abi_true_result() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_json_rpc_response(eth_call_bool(true));

        assert_eq!(
            EthBridge::load_is_used(H256::repeat_byte(0x41), ETH_NETWORK_ID),
            Ok(true)
        );
    });
}

#[test]
fn load_is_used_rejects_json_boolean_result() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_json_rpc_response(true);

        assert_eq!(
            EthBridge::load_is_used(H256::repeat_byte(0x4a), ETH_NETWORK_ID),
            Err(Error::FailedToLoadIsUsed)
        );
    });
}

#[test]
fn load_is_used_rejects_null_result() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_json_rpc_response(Option::<types::Bytes>::None);

        assert_eq!(
            EthBridge::load_is_used(H256::repeat_byte(0x4b), ETH_NETWORK_ID),
            Err(Error::FailedToLoadIsUsed)
        );
    });
}

#[test]
fn load_is_used_returns_false_when_all_contracts_report_false() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_json_rpc_response(eth_call_bool(false));
        push_global_json_rpc_response(eth_call_bool(false));

        assert_eq!(
            EthBridge::load_is_used(H256::repeat_byte(0x42), ETH_NETWORK_ID),
            Ok(false)
        );
    });
}

#[test]
fn load_is_used_queries_single_contract_for_non_ethereum_network() {
    let mut builder = ExtBuilder::default();
    let net_id = builder.add_network(
        vec![AssetConfig::Thischain { id: XOR.into() }],
        None,
        Some(1),
        sp_core::H160::repeat_byte(0x23),
    );
    let (mut ext, _state) = builder.build();

    ext.execute_with(|| {
        push_global_json_rpc_response(eth_call_bool(false));

        assert_eq!(
            EthBridge::load_is_used(H256::repeat_byte(0x48), net_id),
            Ok(false)
        );
    });
}

#[test]
fn load_is_used_checks_val_master_after_bridge_contract_false() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_json_rpc_response(eth_call_bool(false));
        push_global_json_rpc_response(eth_call_bool(true));

        assert_eq!(
            EthBridge::load_is_used(H256::repeat_byte(0x43), ETH_NETWORK_ID),
            Ok(true)
        );
    });
}

#[test]
fn load_is_used_rejects_empty_eth_call_result() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_json_rpc_response(raw_eth_call_result(&[]));

        assert_eq!(
            EthBridge::load_is_used(H256::repeat_byte(0x44), ETH_NETWORK_ID),
            Err(Error::FailedToLoadIsUsed)
        );
    });
}

#[test]
fn load_is_used_rejects_short_abi_result() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_json_rpc_response(raw_eth_call_result(&[0; 31]));

        assert_eq!(
            EthBridge::load_is_used(H256::repeat_byte(0x45), ETH_NETWORK_ID),
            Err(Error::FailedToLoadIsUsed)
        );
    });
}

#[test]
fn load_is_used_rejects_non_canonical_abi_bool() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let mut result = [0u8; 32];
        result[31] = 2;
        push_global_json_rpc_response(raw_eth_call_result(&result));

        assert_eq!(
            EthBridge::load_is_used(H256::repeat_byte(0x46), ETH_NETWORK_ID),
            Err(Error::FailedToLoadIsUsed)
        );
    });
}

#[test]
fn load_is_used_rejects_non_zero_abi_bool_padding() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let mut result = [0u8; 32];
        result[30] = 1;
        push_global_json_rpc_response(raw_eth_call_result(&result));

        assert_eq!(
            EthBridge::load_is_used(H256::repeat_byte(0x49), ETH_NETWORK_ID),
            Err(Error::FailedToLoadIsUsed)
        );
    });
}

#[test]
fn ocw_retries_mark_as_done_when_used_result_is_malformed() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Thischain { id: XOR.into() }],
        None,
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let outgoing_hash = H256::repeat_byte(0x47);

        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            outgoing_hash,
            RequestStatus::ApprovalsReady,
        );
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(alice),
            outgoing_hash,
            IncomingRequestKind::Meta(IncomingMetaRequestKind::MarkAsDone),
            net_id
        ));
        let load_hash = last_request(net_id).unwrap().hash();

        state.push_response(types::U64::from(777u64));
        state.push_response(raw_eth_call_result(&[0; 31]));
        state.run_next_offchain_and_dispatch_txs();

        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, load_hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&load_hash));
        assert_eq!(
            crate::LoadToIncomingRequestHash::<Runtime>::get(net_id, load_hash),
            H256::zero()
        );
        let handled_key = format!("eth-bridge-ocw::handled-request-{:?}", load_hash);
        assert_eq!(
            state.storage_read::<BlockNumber>(handled_key.as_bytes()),
            None
        );
    });
}

#[test]
fn ocw_should_prune_stale_hashes_from_requests_queue() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let stale_hash = H256::repeat_byte(0xaa);
        assert!(crate::Requests::<Runtime>::get(net_id, stale_hash).is_none());
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(stale_hash));
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&stale_hash));

        state.run_next_offchain_with_params(
            0,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );

        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&stale_hash));
    });
}

#[test]
fn ocw_should_resend_signed_transaction_on_timeout() {
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
        state.run_next_offchain_with_params(
            0,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );
        assert_eq!(state.pending_txs().len(), 1);
        assert!(state
            .pending_txs()
            .iter()
            .all(|(hash, x)| hash == &x.extrinsic_hash));
        assert_eq!(state.pool_state.read().transactions.len(), 1);
        for _ in 0..SUBSTRATE_MAX_BLOCK_NUM_EXPECTING_UNTIL_FINALIZATION + 1 {
            state.run_next_offchain_with_params(
                0,
                frame_system::Pallet::<Runtime>::block_number() + 1,
                false,
            );
        }
        assert_eq!(state.pending_txs().len(), 1);
        assert!(state
            .pending_txs()
            .iter()
            .all(|(hash, x)| hash == &x.extrinsic_hash));
        assert_eq!(state.pool_state.read().transactions.len(), 2);
    });
}

#[test]
fn ocw_should_remove_pending_transaction_on_max_retries() {
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
        state.set_should_fail_send_signed_transactions(true);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(state.pending_txs().len(), 1);
        assert_eq!(state.pool_state.read().transactions.len(), 0);
        for _ in 0..MAX_FAILED_SEND_SIGNED_TX_RETRIES {
            state.run_next_offchain_and_dispatch_txs();
            assert_eq!(state.pending_txs().len(), 1);
            assert_eq!(state.failed_pending_txs().len(), 0);
            assert_eq!(state.pool_state.read().transactions.len(), 0);
        }
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(state.pending_txs().len(), 0);
        assert_eq!(state.failed_pending_txs().len(), 1);
        assert_eq!(state.pool_state.read().transactions.len(), 0);
    });
}

#[test]
fn should_not_abort_request_with_failed_to_send_signed_tx_error() {
    assert_eq!(Error::FailedToSendSignedTransaction.should_abort(), false);

    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: XOR.into(),
            sidechain_id: sp_core::H160::from_str("41fd72257597aa14c7231a7b1aaa29fce868f677")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(XOR.into(), common::balance!(350000))]),
        Some(2),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
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
        state.set_should_fail_send_signed_transactions(true);
        state.run_next_offchain_and_dispatch_txs();
        let request_hash = last_request(net_id).unwrap().hash();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash).unwrap(),
            RequestStatus::Pending
        );
    });
}

#[test]
fn outgoing_zero_approval_requests_retry_on_short_cadence() {
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

        state.set_should_fail_send_signed_transactions(true);
        state.run_next_offchain_and_dispatch_txs();
        let request_hash = last_request(net_id).unwrap().hash();
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, request_hash).len(),
            0
        );
        let zero_approval_key = format!(
            "{}-{:?}",
            STORAGE_OUTGOING_ZERO_APPROVAL_REQUESTS_KEY, net_id
        );
        assert_eq!(
            state.storage_read::<u64>(zero_approval_key.as_bytes()),
            Some(1)
        );
        let local_peer_ready_key = format!("{}-{:?}", STORAGE_LOCAL_PEER_READY_KEY, net_id);
        assert_eq!(
            state.storage_read::<u64>(local_peer_ready_key.as_bytes()),
            Some(1)
        );
        let failure_key = format!(
            "{}-{:?}-{}",
            STORAGE_OUTGOING_APPROVAL_FAILURES_KEY,
            net_id,
            OUTGOING_APPROVAL_FAILURE_FAILED_SEND_SIGNED_TX
        );
        assert_eq!(state.storage_read::<u64>(failure_key.as_bytes()), Some(1));
        assert_eq!(state.pending_txs().len(), 1);
        state.storage_remove(STORAGE_PENDING_TRANSACTIONS_KEY);

        state.set_should_fail_send_signed_transactions(false);
        for _ in 0..ZERO_APPROVAL_OUTGOING_RETRY_PERIOD - 2 {
            state.run_next_offchain_and_dispatch_txs();
            assert_eq!(
                crate::RequestApprovals::<Runtime>::get(net_id, request_hash).len(),
                0
            );
        }

        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, request_hash).len(),
            1
        );
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(
            state.storage_read::<u64>(zero_approval_key.as_bytes()),
            Some(0)
        );
    });
}

#[test]
fn should_not_abort_request_when_peer_secret_key_is_missing() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: XOR.into(),
            sidechain_id: sp_core::H160::from_str("41fd72257597aa14c7231a7b1aaa29fce868f677")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(XOR.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
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
        state.storage_remove(STORAGE_PEER_MARKER_KEY);
        state.storage_remove(STORAGE_PEER_SECRET_KEY);
        state.run_next_offchain_worker_and_dispatch_txs();
        let request_hash = last_request(net_id).unwrap().hash();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&request_hash));
    });
}

#[test]
fn should_not_abort_request_when_peer_marker_does_not_match_local_key() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: XOR.into(),
            sidechain_id: sp_core::H160::from_str("41fd72257597aa14c7231a7b1aaa29fce868f677")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(XOR.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
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

        // Corrupt marker to emulate stale/offline DB state.
        let request_hash = last_request(net_id).unwrap().hash();
        state.offchain_state.write().persistent_storage.set(
            b"",
            STORAGE_PEER_MARKER_KEY,
            &vec![7u8; 33].encode(),
        );
        state.run_next_offchain_worker_and_dispatch_txs();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&request_hash));
    });
}

#[test]
fn should_not_treat_32_byte_peer_marker_value_as_legacy_secret_marker() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: XOR.into(),
            sidechain_id: sp_core::H160::from_str("41fd72257597aa14c7231a7b1aaa29fce868f677")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(XOR.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
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

        // A 32-byte marker under the *new* marker key must not be accepted as legacy marker.
        let request_hash = last_request(net_id).unwrap().hash();
        state.offchain_state.write().persistent_storage.set(
            b"",
            STORAGE_PEER_MARKER_KEY,
            &vec![7u8; 32].encode(),
        );
        state.run_next_offchain_worker_and_dispatch_txs();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&request_hash));
    });
}

#[test]
fn send_multisig_transaction_should_return_unknown_network_for_unregistered_network() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let network_id = ETH_NETWORK_ID + 1;
        let call = crate::Call::<Runtime>::abort_request {
            hash: H256::repeat_byte(0x11),
            error: Error::Other.into(),
            network_id,
        };
        let result = EthBridge::send_multisig_transaction(
            call,
            bridge_multisig::Pallet::<Runtime>::thischain_timepoint(),
            network_id,
        );
        assert_eq!(result, Err(Error::UnknownNetwork));
    });
}

#[test]
fn multisig_threshold_1_unknown_account_should_return_error() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let unknown_multisig = get_account_id_from_seed::<sr25519::Public>("Eve");
        let call = Box::new(RuntimeCall::System(frame_system::Call::remark {
            remark: b"bridge-multisig-regression".to_vec(),
        }));
        assert_err!(
            bridge_multisig::Pallet::<Runtime>::as_multi_threshold_1(
                RuntimeOrigin::signed(get_account_id_from_seed::<sr25519::Public>("Alice")),
                unknown_multisig,
                call,
                bridge_multisig::Pallet::<Runtime>::thischain_timepoint(),
            ),
            bridge_multisig::Error::<Runtime>::UnknownMultisigAccount
        );
    });
}

#[test]
fn ocw_should_load_substrate_blocks_sequentially() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_eq!(state.substrate_to_handle_from_height(), 0);
        let finalized_height = 10;
        state.run_next_offchain_with_params(0, finalized_height, false);
        assert_eq!(
            state.substrate_to_handle_from_height(),
            finalized_height + 1
        );
        let blocks_passed = 10;
        for i in 1..=blocks_passed {
            // Assume the new finalized height doesn't change.
            state.run_next_offchain_with_params(0, finalized_height + blocks_passed, false);
            // Then off-chain workers should load each block sequentially up to the finalized one.
            assert_eq!(
                state.substrate_to_handle_from_height(),
                (finalized_height + i * SUBSTRATE_HANDLE_BLOCK_COUNT_PER_BLOCK as BlockNumber)
                    .min(finalized_height + blocks_passed)
                    + 1
            );
        }
    });
}

#[test]
fn ocw_should_handle_substrate_height_above_u32_max() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let finalized_height = (u32::MAX as u64) + 1;
        state.run_next_offchain_with_params(0, finalized_height, false);
        assert_eq!(
            state.substrate_to_handle_from_height(),
            finalized_height + 1
        );
    });
}

#[test]
fn ocw_should_abort_missing_transaction() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: VAL.into(),
            sidechain_id: sp_core::H160::from_str("0x725c6b8cd3621eba4e0ccc40d532e7025b925a65")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(VAL.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256([1; 32]);
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            tx_hash,
            IncomingRequestKind::Transaction(IncomingTransactionRequestKind::Transfer),
            net_id
        ));
        let raw_response = r#"{
 "jsonrpc": "2.0",
   "id": 0,
   "result": null
 }"#;
        state.push_response_raw(raw_response.as_bytes().to_owned());
        state.run_next_offchain_and_dispatch_txs();
        let dispatch_error: DispatchError = Error::FailedToLoadTransaction.into();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, tx_hash).unwrap(),
            RequestStatus::Failed(dispatch_error.stripped()),
        );
    });
}

#[test]
fn ocw_should_retry_when_abort_submission_fails() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: VAL.into(),
            sidechain_id: sp_core::H160::from_str("0x725c6b8cd3621eba4e0ccc40d532e7025b925a65")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(VAL.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256([11; 32]);
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(alice),
            tx_hash,
            IncomingRequestKind::Transaction(IncomingTransactionRequestKind::Transfer),
            net_id
        ));

        let raw_response = r#"{
 "jsonrpc": "2.0",
   "id": 0,
   "result": null
 }"#;
        state.push_response_raw(raw_response.as_bytes().to_owned());
        state.set_should_fail_send_signed_transactions(true);
        state.run_next_offchain_and_dispatch_txs();

        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, tx_hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&tx_hash));
        let handled_key = format!("eth-bridge-ocw::handled-request-{:?}", tx_hash);
        assert_eq!(
            state.storage_read::<BlockNumber>(handled_key.as_bytes()),
            None
        );

        state.set_should_fail_send_signed_transactions(false);
        state.push_response_raw(raw_response.as_bytes().to_owned());
        state.run_next_offchain_and_dispatch_txs();

        let dispatch_error: DispatchError = Error::FailedToLoadTransaction.into();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, tx_hash),
            Some(RequestStatus::Failed(dispatch_error.stripped()))
        );
    });
}

#[test]
fn ocw_should_not_panic_on_receipt_without_block_number() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: VAL.into(),
            sidechain_id: sp_core::H160::from_str("0x725c6b8cd3621eba4e0ccc40d532e7025b925a65")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(VAL.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256([2; 32]);
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(alice),
            tx_hash,
            IncomingRequestKind::Transaction(IncomingTransactionRequestKind::Transfer),
            net_id
        ));
        let contract = crate::BridgeContractAddress::<Runtime>::get(net_id);
        state.push_response(TransactionReceipt {
            transaction_hash: crate::types::H256(tx_hash.0),
            to: Some(crate::types::H160(contract.0)),
            status: Some(1u64.into()),
            block_number: None,
            ..Default::default()
        });
        state.run_next_offchain_and_dispatch_txs();
        let dispatch_error: DispatchError = Error::EthTransactionIsPending.into();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, tx_hash),
            Some(RequestStatus::Failed(dispatch_error.stripped()))
        );
    });
}

#[test]
fn ocw_should_retry_on_malformed_json_rpc_response() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: VAL.into(),
            sidechain_id: sp_core::H160::from_str("0x725c6b8cd3621eba4e0ccc40d532e7025b925a65")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(VAL.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256([8; 32]);
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(alice),
            tx_hash,
            IncomingRequestKind::Transaction(IncomingTransactionRequestKind::Transfer),
            net_id
        ));
        let raw_response = r#"{
 "jsonrpc": "2.0",
   "id": 0,
   "result":
 }"#;
        state.push_response_raw(raw_response.as_bytes().to_owned());
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, tx_hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&tx_hash));
    });
}

#[test]
fn ocw_should_retry_on_oversized_json_rpc_response() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: VAL.into(),
            sidechain_id: sp_core::H160::from_str("0x725c6b8cd3621eba4e0ccc40d532e7025b925a65")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(VAL.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256([9; 32]);
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(alice),
            tx_hash,
            IncomingRequestKind::Transaction(IncomingTransactionRequestKind::Transfer),
            net_id
        ));
        state.push_response_raw(vec![b' '; MAX_LARGE_JSON_RPC_RESPONSE_BYTES + 1]);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, tx_hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&tx_hash));
    });
}

#[test]
fn ocw_should_retry_on_json_rpc_batch_response() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: VAL.into(),
            sidechain_id: sp_core::H160::from_str("0x725c6b8cd3621eba4e0ccc40d532e7025b925a65")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(VAL.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256([10; 32]);
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(alice),
            tx_hash,
            IncomingRequestKind::Transaction(IncomingTransactionRequestKind::Transfer),
            net_id
        ));
        let raw_response = r#"[{
 "jsonrpc": "2.0",
   "id": 0,
   "result": {}
 }]"#;
        state.push_response_raw(raw_response.as_bytes().to_owned());
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, tx_hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&tx_hash));
    });
}

#[test]
fn ocw_should_retry_when_sidechain_node_params_are_missing() {
    assert!(Error::FailedToLoadSidechainNodeParams.should_retry());
    assert!(Error::FailedToLoadIsUsed.should_retry());
    assert!(Error::HttpResponseTooLarge.should_retry());

    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: XOR.into(),
            sidechain_id: sp_core::H160::from_str("41fd72257597aa14c7231a7b1aaa29fce868f677")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(XOR.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256([9; 32]);
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(alice),
            tx_hash,
            IncomingRequestKind::Transaction(IncomingTransactionRequestKind::Transfer),
            net_id
        ));

        let key = format!("{}-{:?}", STORAGE_ETH_NODE_PARAMS, net_id);
        state.storage_remove(key.as_bytes());
        state.run_next_offchain_and_dispatch_txs();

        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, tx_hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&tx_hash));
    });
}

#[test]
fn large_block_rpc_response_uses_the_block_specific_limit() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let large_result = "x".repeat(2 * 1024 * 1024);
        push_global_json_rpc_response(large_result.clone());

        let decoded = EthBridge::substrate_json_rpc_request::<_, String>("chain_getBlock", &())
            .expect("valid block-sized response should not use the small RPC limit");
        assert_eq!(decoded, large_result);

        push_global_json_rpc_response("x".repeat(2 * 1024 * 1024));
        assert_eq!(
            EthBridge::substrate_json_rpc_request::<_, String>("chain_getHeader", &()),
            Err(Error::HttpResponseTooLarge)
        );
    });
}

#[test]
fn incomplete_json_rpc_response_is_retriable() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_raw_response(br#"{"jsonrpc":"2.0","id":0}"#);

        let error = EthBridge::substrate_json_rpc_request::<_, String>("chain_getHeader", &())
            .expect_err("an incomplete response must not be accepted as a null result");
        assert_eq!(error, Error::JsonDeserializationError);
        assert!(error.should_retry());
    });
}

#[test]
fn log_http_failure_is_retryable() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_http_failure();
        let error = EthBridge::load_transfers_logs(ETH_NETWORK_ID, 100, 106)
            .expect_err("invalidated pending HTTP request must fail");
        assert_eq!(error, Error::HttpFetchingError);
        assert!(error.should_retry());
    });
}

#[test]
fn log_http_failure_preserves_cursor_and_retries_the_same_range() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let height_key = format!(
            "eth-bridge-ocw::eth-to-handle-from-height-{:?}",
            ETH_NETWORK_ID
        );
        let range_key = format!("eth-bridge-ocw::eth-log-range-size-{:?}", ETH_NETWORK_ID);
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(range_key.as_bytes()).set(&7u64);

        state.push_http_failure();
        state.run_next_offchain_with_params(
            200,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(100));
        assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(7));

        state.push_response::<[Log; 0]>([]);
        state.run_next_offchain_with_params(
            201,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(107));
        assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(14));

        let requests: Vec<_> = state
            .http_requests()
            .into_iter()
            .filter(|request| request["method"] == "eth_getLogs")
            .collect();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["params"][0]["fromBlock"], "0x64");
        assert_eq!(requests[0]["params"][0]["toBlock"], "0x6a");
        assert_eq!(
            requests[0], requests[1],
            "retry must preserve the full filter"
        );
    });
}

#[test]
fn log_http_failure_does_not_block_outgoing_approvals() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            ETH_NETWORK_ID,
        ));
        let request_hash = last_request(ETH_NETWORK_ID).unwrap().hash();
        let height_key = format!(
            "eth-bridge-ocw::eth-to-handle-from-height-{:?}",
            ETH_NETWORK_ID
        );
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);

        state.push_http_failure();
        state.run_next_offchain_with_params(
            200,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            true,
        );

        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(100));
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(ETH_NETWORK_ID, request_hash).len(),
            1
        );
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(ETH_NETWORK_ID, request_hash),
            Some(RequestStatus::Pending)
        );
    });
}

#[test]
fn log_http_failure_does_not_block_another_network() {
    let mut builder = ExtBuilder::default();
    let second_network =
        builder.add_network(vec![], None, Some(1), sp_core::H160::repeat_byte(0x23));
    // The same local signing key must be eligible for both networks in this worker.
    let peers = builder.networks[&ETH_NETWORK_ID]
        .config
        .initial_peers
        .clone();
    let keypairs = builder.networks[&ETH_NETWORK_ID].ocw_keypairs.clone();
    let second_config = builder.networks.get_mut(&second_network).unwrap();
    second_config.config.initial_peers = peers;
    second_config.ocw_keypairs = keypairs;
    let (mut ext, mut state) = builder.build();

    ext.execute_with(|| {
        let first_height_key = format!(
            "eth-bridge-ocw::eth-to-handle-from-height-{:?}",
            ETH_NETWORK_ID
        );
        let second_height_key = format!(
            "eth-bridge-ocw::eth-to-handle-from-height-{:?}",
            second_network
        );
        StorageValueRef::persistent(first_height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(second_height_key.as_bytes()).set(&300u64);

        // The helper supplies the first network's blockNumber response. These responses then
        // cover its failing log scan and the second network's successful height and log scan.
        state.push_http_failure();
        state.push_response(types::U64::from(400u64));
        state.push_response::<[Log; 0]>([]);
        state.run_next_offchain_with_params(
            200,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );

        assert_eq!(
            state.storage_read::<u64>(first_height_key.as_bytes()),
            Some(100)
        );
        assert_eq!(
            state.storage_read::<u64>(second_height_key.as_bytes()),
            Some(350)
        );
        let requests: Vec<_> = state
            .http_requests()
            .into_iter()
            .filter(|request| request["method"] == "eth_getLogs")
            .collect();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["params"][0]["fromBlock"], "0x64");
        assert_eq!(requests[1]["params"][0]["fromBlock"], "0x12c");
        assert_eq!(requests[1]["params"][0]["toBlock"], "0x15d");
    });
}

#[test]
fn oversized_log_range_is_reduced_for_the_next_worker() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        state.run_next_offchain_with_params(
            0,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );

        state.push_response_raw(vec![b' '; MAX_LARGE_JSON_RPC_RESPONSE_BYTES + 1]);
        state.run_next_offchain_with_params(
            CONFIRMATION_INTERVAL + MAX_GET_LOGS_ITEMS,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );

        let height_key = format!(
            "eth-bridge-ocw::eth-to-handle-from-height-{:?}",
            ETH_NETWORK_ID
        );
        let range_key = format!("eth-bridge-ocw::eth-log-range-size-{:?}", ETH_NETWORK_ID);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(0));
        assert_eq!(
            state.storage_read::<u64>(range_key.as_bytes()),
            Some(MAX_GET_LOGS_ITEMS / 2)
        );

        state.push_response::<[Log; 0]>([]);
        state.run_next_offchain_with_params(
            CONFIRMATION_INTERVAL + MAX_GET_LOGS_ITEMS,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );
        assert_eq!(
            state.storage_read::<u64>(height_key.as_bytes()),
            Some(MAX_GET_LOGS_ITEMS / 2)
        );
    });
}

#[test]
fn provider_log_limit_error_reduces_the_next_query_range() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        state.run_next_offchain_with_params(
            0,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );

        state.push_response_raw(
            br#"{"jsonrpc":"2.0","error":{"code":-32005,"message":"query returned more than 10000 results"},"id":0}"#
                .to_vec(),
        );
        state.run_next_offchain_with_params(
            CONFIRMATION_INTERVAL + MAX_GET_LOGS_ITEMS,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );

        let range_key = format!(
            "eth-bridge-ocw::eth-log-range-size-{:?}",
            ETH_NETWORK_ID
        );
        assert_eq!(
            state.storage_read::<u64>(range_key.as_bytes()),
            Some(MAX_GET_LOGS_ITEMS / 2)
        );
    });
}

#[test]
fn failed_pending_multisig_rehandle_is_retried_without_skipping_blocks() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let sidechain_height = 0u64;
        let timepoint_index = 7u32;
        let bridge_account = crate::BridgeAccount::<Runtime>::get(ETH_NETWORK_ID).unwrap();
        insert_pending_sidechain_multisig(
            &bridge_account,
            0x42,
            sidechain_height,
            timepoint_index,
        );
        let handled_key = pending_multisig_rehandle_key(
            "eth-bridge-ocw::eth-re-handle-handled",
            0x42,
            sidechain_height,
            timepoint_index,
        );
        let progress_key = pending_multisig_rehandle_key(
            "eth-bridge-ocw::eth-re-handle-progress",
            0x42,
            sidechain_height,
            timepoint_index,
        );

        push_global_raw_response(
            br#"{"jsonrpc":"2.0","error":{"code":-32005,"message":"query returned more than 10000 results"},"id":0}"#,
        );
        assert!(EthBridge::handle_pending_multisig_calls(
            ETH_NETWORK_ID,
            MAX_PENDING_TX_BLOCKS_PERIOD as u64,
        ));
        assert_eq!(state.storage_read::<bool>(handled_key.as_bytes()), None);

        push_global_json_rpc_response::<[Log; 0]>([]);
        assert!(EthBridge::handle_pending_multisig_calls(
            ETH_NETWORK_ID,
            MAX_PENDING_TX_BLOCKS_PERIOD as u64,
        ));
        assert_eq!(state.storage_read::<bool>(handled_key.as_bytes()), None);
        assert_eq!(
            state.storage_read::<u64>(progress_key.as_bytes()),
            Some(sidechain_height + 1)
        );

        push_global_json_rpc_response::<[Log; 0]>([]);
        assert!(EthBridge::handle_pending_multisig_calls(
            ETH_NETWORK_ID,
            MAX_PENDING_TX_BLOCKS_PERIOD as u64,
        ));
        assert_eq!(
            state.storage_read::<bool>(handled_key.as_bytes()),
            Some(true)
        );
    });
}

#[test]
fn pending_multisig_rehandle_due_survives_main_log_scan() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let sidechain_height = 0u64;
        let timepoint_index = 8u32;
        let bridge_account = crate::BridgeAccount::<Runtime>::get(ETH_NETWORK_ID).unwrap();
        insert_pending_sidechain_multisig(&bridge_account, 0x43, sidechain_height, timepoint_index);

        let scan_height_key = format!(
            "eth-bridge-ocw::eth-to-handle-from-height-{:?}",
            ETH_NETWORK_ID
        );
        StorageValueRef::persistent(scan_height_key.as_bytes()).set(&50u64);
        let due_key = format!("eth-bridge-ocw::eth-re-handle-due-{:?}", ETH_NETWORK_ID);
        let handled_key = pending_multisig_rehandle_key(
            "eth-bridge-ocw::eth-re-handle-handled",
            0x43,
            sidechain_height,
            timepoint_index,
        );

        // The main scan consumes this worker's log-request budget at the periodic boundary.
        push_global_json_rpc_response(types::U64::from(200u64));
        push_global_json_rpc_response::<[Log; 0]>([]);
        EthBridge::handle_network(ETH_NETWORK_ID, RE_HANDLE_TXS_PERIOD.into());
        assert_eq!(state.storage_read::<bool>(due_key.as_bytes()), Some(true));
        assert_eq!(state.storage_read::<bool>(handled_key.as_bytes()), None);
        assert_eq!(
            state.storage_read::<u64>(scan_height_key.as_bytes()),
            Some(100)
        );

        // The main cursor remains backlogged, but the persisted due flag gives the re-handle
        // priority on the first worker after the periodic boundary.
        push_global_json_rpc_response(types::U64::from(200u64));
        push_global_json_rpc_response::<[Log; 0]>([]);
        EthBridge::handle_network(ETH_NETWORK_ID, (RE_HANDLE_TXS_PERIOD + 1).into());
        assert_eq!(state.storage_read::<bool>(due_key.as_bytes()), Some(false));
        assert_eq!(
            state.storage_read::<bool>(handled_key.as_bytes()),
            Some(true)
        );
        assert_eq!(
            state.storage_read::<u64>(scan_height_key.as_bytes()),
            Some(100),
            "the pending re-handle should reserve this worker's log request"
        );
    });
}

#[test]
fn pending_multisig_rehandle_generation_handles_boundary_jump_once() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let bridge_account = crate::BridgeAccount::<Runtime>::get(ETH_NETWORK_ID).unwrap();
        let scan_height_key = format!(
            "eth-bridge-ocw::eth-to-handle-from-height-{:?}",
            ETH_NETWORK_ID
        );
        StorageValueRef::persistent(scan_height_key.as_bytes()).set(&u64::MAX);
        let generation_key = format!(
            "eth-bridge-ocw::eth-re-handle-generation-v2-{:?}",
            ETH_NETWORK_ID
        );
        let due_key = format!("eth-bridge-ocw::eth-re-handle-due-{:?}", ETH_NETWORK_ID);

        insert_pending_sidechain_multisig(&bridge_account, 0x47, 0, 12);
        let first_handled_key =
            pending_multisig_rehandle_key("eth-bridge-ocw::eth-re-handle-handled", 0x47, 0, 12);

        // Observe the generation immediately before the boundary.
        push_global_json_rpc_response(types::U64::from(100u64));
        EthBridge::handle_network(ETH_NETWORK_ID, (RE_HANDLE_TXS_PERIOD - 1).into());
        assert_eq!(
            state.storage_read::<u64>(generation_key.as_bytes()),
            Some(0)
        );
        assert_eq!(state.storage_read::<bool>(due_key.as_bytes()), None);

        // Jump over the exact boundary. Advancing the generation still schedules one attempt.
        push_global_json_rpc_response(types::U64::from(100u64));
        push_global_json_rpc_response::<[Log; 0]>([]);
        EthBridge::handle_network(ETH_NETWORK_ID, (RE_HANDLE_TXS_PERIOD + 1).into());
        assert_eq!(
            state.storage_read::<bool>(first_handled_key.as_bytes()),
            Some(true)
        );
        assert_eq!(
            state.storage_read::<u64>(generation_key.as_bytes()),
            Some(1)
        );
        assert_eq!(state.storage_read::<bool>(due_key.as_bytes()), Some(false));

        // Re-observing the same finalized height must not re-arm the generation.
        insert_pending_sidechain_multisig(&bridge_account, 0x48, 0, 13);
        let second_handled_key =
            pending_multisig_rehandle_key("eth-bridge-ocw::eth-re-handle-handled", 0x48, 0, 13);
        push_global_json_rpc_response(types::U64::from(100u64));
        EthBridge::handle_network(ETH_NETWORK_ID, (RE_HANDLE_TXS_PERIOD + 1).into());
        assert_eq!(
            state.storage_read::<bool>(second_handled_key.as_bytes()),
            None
        );
        assert_eq!(state.storage_read::<bool>(due_key.as_bytes()), Some(false));
    });
}

#[test]
fn pending_multisig_rehandle_round_robin_distinguishes_shared_timepoint_calls() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let bridge_account = crate::BridgeAccount::<Runtime>::get(ETH_NETWORK_ID).unwrap();
        // Distinct calls can legitimately share a sidechain timepoint. Their retry state must be
        // keyed by call hash rather than `(height, index)`.
        let candidates = [(0u64, 9u32, 0x44u8), (0u64, 9u32, 0x45u8)];
        for (height, index, hash_byte) in candidates {
            insert_pending_sidechain_multisig(&bridge_account, hash_byte, height, index);
        }

        push_global_raw_response(
            br#"{"jsonrpc":"2.0","error":{"code":-32005,"message":"query returned more than 10000 results"},"id":0}"#,
        );
        assert!(EthBridge::handle_pending_multisig_calls(
            ETH_NETWORK_ID,
            MAX_PENDING_TX_BLOCKS_PERIOD as u64 + 2,
        ));

        let range_keys = candidates.map(|(height, index, hash_byte)| {
            pending_multisig_rehandle_key(
                "eth-bridge-ocw::eth-re-handle-range-size",
                hash_byte,
                height,
                index,
            )
        });
        let handled_keys = candidates.map(|(height, index, hash_byte)| {
            pending_multisig_rehandle_key(
                "eth-bridge-ocw::eth-re-handle-handled",
                hash_byte,
                height,
                index,
            )
        });
        let first_failed = if state.storage_read::<u64>(range_keys[0].as_bytes()) == Some(1) {
            0
        } else {
            assert_eq!(
                state.storage_read::<u64>(range_keys[1].as_bytes()),
                Some(1)
            );
            1
        };

        push_global_json_rpc_response::<[Log; 0]>([]);
        assert!(EthBridge::handle_pending_multisig_calls(
            ETH_NETWORK_ID,
            MAX_PENDING_TX_BLOCKS_PERIOD as u64 + 2,
        ));
        assert_eq!(
            state.storage_read::<bool>(handled_keys[first_failed].as_bytes()),
            None
        );
        assert_eq!(
            state.storage_read::<bool>(handled_keys[1 - first_failed].as_bytes()),
            Some(true)
        );
        let cursor_key = format!(
            "eth-bridge-ocw::eth-re-handle-cursor-{:?}",
            ETH_NETWORK_ID
        );
        assert_eq!(state.storage_read::<u64>(cursor_key.as_bytes()), Some(2));
    });
}

#[test]
fn pending_multisig_rehandle_treats_reused_call_hash_at_new_timepoint_as_new() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let bridge_account = crate::BridgeAccount::<Runtime>::get(ETH_NETWORK_ID).unwrap();
        let call_hash_byte = 0x49;
        insert_pending_sidechain_multisig(&bridge_account, call_hash_byte, 0, 14);
        push_global_json_rpc_response::<[Log; 0]>([]);
        assert!(EthBridge::handle_pending_multisig_calls(
            ETH_NETWORK_ID,
            MAX_PENDING_TX_BLOCKS_PERIOD as u64 + 2,
        ));
        let first_handled_key = pending_multisig_rehandle_key(
            "eth-bridge-ocw::eth-re-handle-handled",
            call_hash_byte,
            0,
            14,
        );
        assert_eq!(
            state.storage_read::<bool>(first_handled_key.as_bytes()),
            Some(true)
        );

        bridge_multisig::Multisigs::<Runtime>::remove(&bridge_account, [call_hash_byte; 32]);
        insert_pending_sidechain_multisig(&bridge_account, call_hash_byte, 2, 15);
        push_global_json_rpc_response::<[Log; 0]>([]);
        assert!(EthBridge::handle_pending_multisig_calls(
            ETH_NETWORK_ID,
            MAX_PENDING_TX_BLOCKS_PERIOD as u64 + 2,
        ));
        let second_handled_key = pending_multisig_rehandle_key(
            "eth-bridge-ocw::eth-re-handle-handled",
            call_hash_byte,
            2,
            15,
        );
        assert_eq!(
            state.storage_read::<bool>(second_handled_key.as_bytes()),
            Some(true)
        );
    });
}

#[test]
fn pending_multisig_rehandle_ignores_other_multisig_accounts() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let unrelated_account = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_ne!(
            unrelated_account,
            crate::BridgeAccount::<Runtime>::get(ETH_NETWORK_ID).unwrap()
        );
        insert_pending_sidechain_multisig(&unrelated_account, 0x46, 0, 11);

        // No RPC response is queued: selecting the unrelated operation would make the mock panic.
        assert!(!EthBridge::handle_pending_multisig_calls(
            ETH_NETWORK_ID,
            MAX_PENDING_TX_BLOCKS_PERIOD as u64,
        ));
        let cursor_key = format!("eth-bridge-ocw::eth-re-handle-cursor-{:?}", ETH_NETWORK_ID);
        assert_eq!(state.storage_read::<u64>(cursor_key.as_bytes()), None);
    });
}

#[test]
fn outgoing_approvals_do_not_depend_on_sidechain_rpc_preflight() {
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

        let key = format!("{}-{:?}", STORAGE_ETH_NODE_PARAMS, net_id);
        state.storage_remove(key.as_bytes());
        state.run_next_offchain_and_dispatch_txs();

        let request_hash = last_request(net_id).unwrap().hash();
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, request_hash).len(),
            1
        );
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Pending)
        );
    });
}

#[test]
fn should_reapprove_on_long_pending() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            10,
            net_id,
        ));
        state.run_next_offchain_with_params(
            CONFIRMATION_INTERVAL,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );
        assert_eq!(crate::RequestsQueue::<Runtime>::get(net_id).len(), 1);
        let mut guard = state.pool_state.write();
        assert!(!guard.transactions.is_empty());
        guard.transactions.clear();
        state.storage_remove(STORAGE_PENDING_TRANSACTIONS_KEY);
        frame_system::Pallet::<Runtime>::set_block_number(MAX_PENDING_TX_BLOCKS_PERIOD as u64);
        drop(guard);
        for _ in 0..MAX_PENDING_TX_BLOCKS_PERIOD - 1 {
            state.run_next_offchain_with_params(
                CONFIRMATION_INTERVAL,
                frame_system::Pallet::<Runtime>::block_number() + 1,
                false,
            );
        }
        let guard = state.pool_state.read();
        assert!(!guard.transactions.is_empty());
        assert_eq!(crate::RequestsQueue::<Runtime>::get(net_id).len(), 1);
    });
}

#[test]
fn should_resend_incoming_requests_from_failed_offchain_queue() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: XOR.into(),
            sidechain_id: sp_core::H160::from_str("41fd72257597aa14c7231a7b1aaa29fce868f677")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(XOR.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();

        let mut log = Log::default();
        log.topics = vec![types::H256(hex!(
            "85c0fa492ded927d3acca961da52b0dda1debb06d8c27fe189315f06bb6e26c8"
        ))];
        let data = ethabi::encode(&[
            ethabi::Token::FixedBytes(alice.encode()),
            ethabi::Token::Uint(types::U256::from(100)),
            ethabi::Token::Address(types::EthAddress::from(
                crate::RegisteredSidechainToken::<Runtime>::get(net_id, XOR)
                    .unwrap()
                    .0,
            )),
            ethabi::Token::FixedBytes(XOR.code.to_vec()),
        ]);
        let tx_hash = H256([1; 32]);
        log.data = data.into();
        log.removed = Some(false);
        log.transaction_hash = Some(types::H256(tx_hash.0));
        log.block_number = Some(0u64.into());
        log.transaction_index = Some(0u64.into());
        state.run_next_offchain_with_params(
            0,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            true,
        );
        state.push_response([log]);

        state.set_should_fail_send_signed_transactions(true);

        // "Wait" `CONFIRMATION_INTERVAL` blocks on sidechain, but fail the approval submission.
        state.run_next_offchain_with_params(
            CONFIRMATION_INTERVAL,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            true,
        );

        state.push_response::<[Log; 0]>([]);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(state.pending_txs().len(), 1);
        assert_eq!(state.pool_state.read().transactions.len(), 0);
        // Make the extrinsic move to the secondary (failed txs) queue.
        for _ in 0..MAX_FAILED_SEND_SIGNED_TX_RETRIES - 1 {
            state.run_next_offchain_and_dispatch_txs();
            assert_eq!(state.pending_txs().len(), 1);
            assert_eq!(state.failed_pending_txs().len(), 0);
            assert_eq!(state.pool_state.read().transactions.len(), 0);
        }
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(state.pending_txs().len(), 0);
        assert_eq!(state.failed_pending_txs().len(), 1);
        assert_eq!(state.pool_state.read().transactions.len(), 0);

        // Wait for the re-handle stage.
        for _ in 0..RE_HANDLE_TXS_PERIOD - 5 {
            state.run_next_offchain_and_dispatch_txs();
        }

        assert_eq!(state.pending_txs().len(), 1);
        assert_eq!(state.failed_pending_txs().len(), 1);
        assert_eq!(state.pool_state.read().transactions.len(), 0);

        state.set_should_fail_send_signed_transactions(false);

        state.run_next_offchain_and_dispatch_txs();
        // Re-handle again and check that the transactions was removed from the secondary qeueue.
        for _ in 0..RE_HANDLE_TXS_PERIOD {
            state.run_next_offchain_and_dispatch_txs();
        }

        assert_eq!(state.pending_txs().len(), 1);
        assert_eq!(state.failed_pending_txs().len(), 0);
        assert_eq!(state.pool_state.read().transactions.len(), 0);
    });
}

#[test]
fn failed_import_recovery_removes_completed_request_on_its_own_network() {
    let mut builder = ExtBuilder::default();
    let network_id = builder.add_network(vec![], None, Some(1), Default::default());
    let (mut ext, state) = builder.build();
    ext.execute_with(|| {
        let tx_hash = H256::repeat_byte(0x71);
        insert_failed_incoming_import(network_id, tx_hash);
        crate::RequestStatuses::<Runtime>::insert(network_id, tx_hash, RequestStatus::Done);

        handle_substrate_at_finalized_height(RE_HANDLE_TXS_PERIOD.into());

        assert!(state.failed_pending_txs().is_empty());
        assert!(state.pending_txs().is_empty());
        assert!(state.pool_state.read().transactions.is_empty());
    });
}

#[test]
fn failed_import_recovery_ignores_status_from_another_network() {
    let mut builder = ExtBuilder::default();
    let network_id = builder.add_network(vec![], None, Some(1), Default::default());
    let (mut ext, state) = builder.build();
    ext.execute_with(|| {
        let tx_hash = H256::repeat_byte(0x72);
        insert_failed_incoming_import(network_id, tx_hash);
        crate::RequestStatuses::<Runtime>::insert(ETH_NETWORK_ID, tx_hash, RequestStatus::Done);

        handle_substrate_at_finalized_height(RE_HANDLE_TXS_PERIOD.into());

        assert_eq!(state.failed_pending_txs().len(), 1);
        assert_eq!(state.pending_txs().len(), 1);
        assert_eq!(state.pool_state.read().transactions.len(), 1);
    });
}

#[test]
fn failed_import_recovery_runs_after_skipped_finalized_boundary() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        insert_failed_incoming_import(ETH_NETWORK_ID, H256::repeat_byte(0x73));

        handle_substrate_at_finalized_height((RE_HANDLE_TXS_PERIOD - 1).into());
        assert!(state.pool_state.read().transactions.is_empty());

        handle_substrate_at_finalized_height((RE_HANDLE_TXS_PERIOD + 1).into());
        assert_eq!(state.pool_state.read().transactions.len(), 1);

        handle_substrate_at_finalized_height((RE_HANDLE_TXS_PERIOD + 1).into());
        assert_eq!(state.pool_state.read().transactions.len(), 1);

        handle_substrate_at_finalized_height((RE_HANDLE_TXS_PERIOD - 1).into());
        handle_substrate_at_finalized_height((RE_HANDLE_TXS_PERIOD + 1).into());
        assert_eq!(state.pool_state.read().transactions.len(), 1);

        handle_substrate_at_finalized_height((2 * RE_HANDLE_TXS_PERIOD + 1).into());
        assert_eq!(state.pool_state.read().transactions.len(), 2);
    });
}

#[test]
fn failed_import_recovery_runs_once_for_repeated_finalized_boundary() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        insert_failed_incoming_import(ETH_NETWORK_ID, H256::repeat_byte(0x74));

        handle_substrate_at_finalized_height(RE_HANDLE_TXS_PERIOD.into());
        assert_eq!(state.pool_state.read().transactions.len(), 1);

        handle_substrate_at_finalized_height(RE_HANDLE_TXS_PERIOD.into());
        assert_eq!(state.pool_state.read().transactions.len(), 1);
        assert_eq!(state.failed_pending_txs().len(), 1);
    });
}

#[test]
fn ocw_should_skip_log_with_overflowing_transaction_index() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: XOR.into(),
            sidechain_id: sp_core::H160::from_str("41fd72257597aa14c7231a7b1aaa29fce868f677")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(XOR.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();

        let mut log = Log::default();
        log.topics = vec![types::H256(hex!(
            "85c0fa492ded927d3acca961da52b0dda1debb06d8c27fe189315f06bb6e26c8"
        ))];
        let data = ethabi::encode(&[
            ethabi::Token::FixedBytes(alice.encode()),
            ethabi::Token::Uint(types::U256::from(100)),
            ethabi::Token::Address(types::EthAddress::from(
                crate::RegisteredSidechainToken::<Runtime>::get(net_id, XOR)
                    .unwrap()
                    .0,
            )),
            ethabi::Token::FixedBytes(XOR.code.to_vec()),
        ]);
        let tx_hash = H256([7; 32]);
        log.data = data.into();
        log.removed = Some(false);
        log.transaction_hash = Some(types::H256(tx_hash.0));
        log.block_number = Some(0u64.into());
        log.transaction_index = Some(types::U64::from(u64::MAX));

        // Initialize OCW height pointers.
        state.run_next_offchain_with_params(
            0,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );
        assert_eq!(state.pool_state.read().transactions.len(), 0);

        state.push_response([log]);
        state.run_next_offchain_with_params(
            CONFIRMATION_INTERVAL,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );

        // No import extrinsic should be submitted for a malformed log.
        assert_eq!(state.pool_state.read().transactions.len(), 0);
        assert!(crate::RequestStatuses::<Runtime>::get(net_id, tx_hash).is_none());
    });
}
