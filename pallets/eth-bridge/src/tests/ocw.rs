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
use crate::requests::{IncomingRequestKind, IncomingTransactionRequestKind, RequestStatus};
use crate::tests::mock::{get_account_id_from_seed, ExtBuilder};
use crate::tests::{last_outgoing_request, last_request, Assets, ETH_NETWORK_ID};
use crate::types::Log;
use crate::{
    types, AssetConfig, EthAddress, CONFIRMATION_INTERVAL, MAX_FAILED_SEND_SIGNED_TX_RETRIES,
    MAX_PENDING_TX_BLOCKS_PERIOD, RE_HANDLE_TXS_PERIOD, STORAGE_PENDING_TRANSACTIONS_KEY,
    SUBSTRATE_HANDLE_BLOCK_COUNT_PER_BLOCK, SUBSTRATE_MAX_BLOCK_NUM_EXPECTING_UNTIL_FINALIZATION,
};
use codec::Encode;
use common::{DEFAULT_BALANCE_PRECISION, VAL, XOR};
use frame_support::assert_ok;
use frame_support::dispatch::DispatchError;
use hex_literal::hex;
use sp_core::{sr25519, H256};
use std::str::FromStr;

#[test]
fn ocw_should_not_handle_non_finalized_outgoing_request() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR, &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR,
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
fn ocw_should_resend_signed_transaction_on_timeout() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR, &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR,
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
        Assets::mint_to(&XOR, &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR,
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
    assert!(!Error::FailedToSendSignedTransaction.should_abort());

    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: XOR,
            sidechain_id: sp_core::H160::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(XOR, common::balance!(350000))]),
        Some(2),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR, &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR,
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
fn ocw_should_abort_missing_transaction() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: VAL,
            sidechain_id: sp_core::H160::from_str("0x725c6b8cd3621eba4e0ccc40d532e7025b925a65")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(VAL, common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256([1; 32]);
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
        let dispatch_error: DispatchError = Error::FailedToLoadTransaction.into();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, tx_hash).unwrap(),
            RequestStatus::Failed(dispatch_error.stripped()),
        );
    });
}

#[test]
fn should_reapprove_on_long_pending() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR, &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR,
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
            id: XOR,
            sidechain_id: sp_core::H160::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677")
                .unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(XOR, common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR, &alice, &alice, 100).unwrap();

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
