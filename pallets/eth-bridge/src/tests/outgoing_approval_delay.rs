// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! The off-chain worker waits `OutgoingApprovalDelayMs` on its own clock before it signs an
//! outgoing request. Incoming requests are not delayed.

use super::mock::*;
use crate::requests::{IncomingRequestKind, IncomingTransactionRequestKind, RequestStatus};
use crate::tests::{last_outgoing_request, Assets, ETH_NETWORK_ID};
use crate::types::{Log, TransactionReceipt};
use crate::{
    types, AssetConfig, EthAddress, STORAGE_OUTGOING_APPROVAL_DELAY_START_KEY,
    ZERO_APPROVAL_OUTGOING_RETRY_PERIOD,
};
use codec::Encode;
use common::{DEFAULT_BALANCE_PRECISION, XOR};
use frame_support::assert_ok;
use hex_literal::hex;
use sp_core::offchain::OffchainStorage;
use sp_core::{sr25519, H160, H256};
use std::str::FromStr;

const DELAY_MS: u64 = 120_000;

fn delay_start_key(request_hash: H256) -> Vec<u8> {
    format!(
        "{}-{:?}-{:?}",
        STORAGE_OUTGOING_APPROVAL_DELAY_START_KEY, ETH_NETWORK_ID, request_hash
    )
    .into_bytes()
}

fn delay_start(state: &State, request_hash: H256) -> Option<u64> {
    state.storage_read::<u64>(&delay_start_key(request_hash))
}

fn set_local_time(state: &State, unix_millis: u64) {
    state.offchain_state.write().timestamp = unix_millis.into();
}

fn transfer_out() -> H256 {
    let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
    Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
    assert_ok!(EthBridge::transfer_to_sidechain(
        RuntimeOrigin::signed(alice),
        XOR.into(),
        EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
        100,
        ETH_NETWORK_ID,
    ));
    last_outgoing_request(ETH_NETWORK_ID).unwrap().1
}

fn approvals(request_hash: H256) -> usize {
    crate::RequestApprovals::<Runtime>::get(ETH_NETWORK_ID, request_hash).len()
}

#[test]
fn ocw_signs_outgoing_request_only_after_its_own_delay() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        OutgoingApprovalDelayMsConst::set(DELAY_MS);
        let start = 1_000_000;
        set_local_time(&state, start);
        let hash = transfer_out();

        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(approvals(hash), 0);
        assert!(state.pending_txs().is_empty());
        assert_eq!(delay_start(&state, hash), Some(start));

        // Later runs, including the zero-approval retry cadence, neither sign nor move the
        // start time forward.
        set_local_time(&state, start + DELAY_MS - 1);
        for _ in 0..=ZERO_APPROVAL_OUTGOING_RETRY_PERIOD {
            state.run_next_offchain_and_dispatch_txs();
            assert_eq!(approvals(hash), 0);
        }
        assert!(state.pending_txs().is_empty());
        assert_eq!(delay_start(&state, hash), Some(start));

        set_local_time(&state, start + DELAY_MS);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(approvals(hash), 1);
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(ETH_NETWORK_ID, hash),
            Some(RequestStatus::Pending)
        );
    });
}

#[test]
fn ocw_times_the_outgoing_delay_per_request() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        OutgoingApprovalDelayMsConst::set(DELAY_MS);
        let start = 5_000_000;
        set_local_time(&state, start);
        let first = transfer_out();
        state.run_next_offchain_and_dispatch_txs();

        set_local_time(&state, start + 60_000);
        let second = transfer_out();
        assert_ne!(first, second);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!((approvals(first), approvals(second)), (0, 0));

        set_local_time(&state, start + DELAY_MS);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!((approvals(first), approvals(second)), (1, 0));

        set_local_time(&state, start + 60_000 + DELAY_MS);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!((approvals(first), approvals(second)), (1, 1));
    });
}

#[test]
fn ocw_starts_the_outgoing_delay_once_the_request_is_finalized() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        OutgoingApprovalDelayMsConst::set(DELAY_MS);
        set_local_time(&state, 1_000);
        let hash = transfer_out();

        // The request's block is not final yet. Sidechain height 0 keeps the worker from
        // scanning Ethereum logs in every run of this test.
        state.run_next_offchain_with_params(0, 0, true);
        assert_eq!(delay_start(&state, hash), None);

        let run_with_request_final = |state: &mut State| {
            let finalized = frame_system::Pallet::<Runtime>::block_number() + 1;
            state.run_next_offchain_with_params(0, finalized, true);
        };
        set_local_time(&state, 50_000);
        run_with_request_final(&mut state);
        assert_eq!(delay_start(&state, hash), Some(50_000));

        set_local_time(&state, 50_000 + DELAY_MS - 1);
        run_with_request_final(&mut state);
        assert_eq!(approvals(hash), 0);

        set_local_time(&state, 50_000 + DELAY_MS);
        run_with_request_final(&mut state);
        assert_eq!(approvals(hash), 1);
    });
}

#[test]
fn ocw_restarts_the_outgoing_delay_when_its_clock_goes_back() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        OutgoingApprovalDelayMsConst::set(DELAY_MS);
        set_local_time(&state, 500_000);
        let hash = transfer_out();
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(delay_start(&state, hash), Some(500_000));

        // A start time in the worker's future would otherwise hold the request until the
        // clock caught up again.
        set_local_time(&state, 100_000);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(delay_start(&state, hash), Some(100_000));

        set_local_time(&state, 100_000 + DELAY_MS - 1);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(approvals(hash), 0);

        set_local_time(&state, 100_000 + DELAY_MS);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(approvals(hash), 1);
    });
}

#[test]
fn ocw_restarts_the_outgoing_delay_after_an_unreadable_start() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        OutgoingApprovalDelayMsConst::set(DELAY_MS);
        set_local_time(&state, 300_000);
        let hash = transfer_out();
        state
            .offchain_state
            .write()
            .persistent_storage
            .set(b"", &delay_start_key(hash), &[7u8]);

        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(approvals(hash), 0);
        assert_eq!(delay_start(&state, hash), Some(300_000));

        set_local_time(&state, 300_000 + DELAY_MS);
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(approvals(hash), 1);
    });
}

#[test]
fn zero_outgoing_delay_signs_on_the_first_run_without_a_timer() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        set_local_time(&state, 1_000_000);
        let hash = transfer_out();
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(approvals(hash), 1);
        assert_eq!(delay_start(&state, hash), None);
    });
}

#[test]
fn outgoing_delay_does_not_hold_incoming_requests() {
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: XOR.into(),
            sidechain_id: H160::from_str("41fd72257597aa14c7231a7b1aaa29fce868f677").unwrap(),
            owned: true,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        Some(vec![(XOR.into(), common::balance!(350000))]),
        Some(1),
        Default::default(),
    );
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        OutgoingApprovalDelayMsConst::set(DELAY_MS);
        set_local_time(&state, 1_000_000);
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256([1; 32]);
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            tx_hash,
            IncomingRequestKind::Transaction(IncomingTransactionRequestKind::Transfer),
            net_id
        ));
        let mut log = Log::default();
        log.topics = vec![types::H256(hex!(
            "85c0fa492ded927d3acca961da52b0dda1debb06d8c27fe189315f06bb6e26c8"
        ))];
        log.data = ethabi::encode(&[
            ethabi::Token::FixedBytes(alice.encode()),
            ethabi::Token::Uint(types::U256::from(100)),
            ethabi::Token::Address(types::EthAddress::from(
                crate::RegisteredSidechainToken::<Runtime>::get(net_id, XOR)
                    .unwrap()
                    .0,
            )),
            ethabi::Token::FixedBytes(XOR.code.to_vec()),
        ])
        .into();
        log.removed = Some(false);
        state.push_response(TransactionReceipt {
            transaction_hash: types::H256(tx_hash.0),
            block_number: Some(0u64.into()),
            to: Some(types::H160(
                crate::BridgeContractAddress::<Runtime>::get(net_id).0,
            )),
            logs: vec![log],
            status: Some(1u64.into()),
            ..Default::default()
        });
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, tx_hash),
            Some(RequestStatus::Done)
        );
    });
}
