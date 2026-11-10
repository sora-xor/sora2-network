// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! A guardian halt stops this node's HASHI workers until the operator confirms
//! exactly that halt; nothing on chain restarts them.

use crate::tests::mock::*;
use crate::{
    STORAGE_GUARDIAN_HALT_STOPPED_KEY, STORAGE_PEER_MARKER_KEY, STORAGE_PEER_SECRET_KEY,
    STORAGE_RESUMED_AFTER_HALT_KEY,
};
use codec::Encode;
use common::bridge_halt::{GuardianHalt, GUARDIAN_HALT_STORAGE_KEY};
use frame_support::traits::Hooks;
use sp_runtime::offchain::storage::StorageValueRef;

const FIRST: GuardianHalt = GuardianHalt {
    number: 1,
    block: 5,
};

fn halt_on_chain(halt: &GuardianHalt) {
    sp_io::storage::set(GUARDIAN_HALT_STORAGE_KEY, &halt.encode());
}

/// What `--bridge-resume-after-halt` leaves in this node's offchain storage.
fn operator_confirms(halt: GuardianHalt) {
    StorageValueRef::persistent(STORAGE_RESUMED_AFTER_HALT_KEY).set(&halt);
}

fn stopped_metric() -> Option<u64> {
    StorageValueRef::persistent(STORAGE_GUARDIAN_HALT_STOPPED_KEY)
        .get::<u64>()
        .unwrap()
}

fn run_hook(block: BlockNumber) {
    <EthBridge as Hooks<BlockNumber>>::offchain_worker(block);
}

/// One full worker pass with the node responses it needs.
fn run_worker(state: &mut State) {
    state.run_next_offchain_worker_with_params(
        crate::CONFIRMATION_INTERVAL,
        frame_system::Pallet::<Runtime>::block_number() + 1,
        false,
    );
}

#[test]
fn workers_run_normally_before_any_halt() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        run_worker(&mut state);
        assert!(!state.http_requests().is_empty());
        assert_eq!(stopped_metric(), Some(0));
    });
}

#[test]
fn a_halt_stops_the_workers_until_the_operator_confirms_it() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        halt_on_chain(&FIRST);
        run_hook(6);
        run_hook(7);
        assert!(state.http_requests().is_empty());
        assert_eq!(stopped_metric(), Some(1));

        operator_confirms(FIRST);
        run_worker(&mut state);
        let requests = state.http_requests().len();
        assert!(requests > 0);
        assert_eq!(stopped_metric(), Some(0));

        // The next halt stops them again, although an earlier halt was confirmed.
        let second = GuardianHalt {
            number: 2,
            block: 40,
        };
        halt_on_chain(&second);
        run_hook(41);
        assert_eq!(state.http_requests().len(), requests);
        assert_eq!(stopped_metric(), Some(2));

        operator_confirms(second);
        run_worker(&mut state);
        assert!(state.http_requests().len() > requests);
        assert_eq!(stopped_metric(), Some(0));
    });
}

#[test]
fn only_the_exact_halt_restarts_the_workers() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        halt_on_chain(&FIRST);
        for confirmed in [
            // An older halt.
            GuardianHalt {
                number: 0,
                block: 5,
            },
            // A number typed ahead of time does not cover a later halt.
            GuardianHalt {
                number: 2,
                block: 5,
            },
            // The same number recorded in another block: a rewound chain's halt.
            GuardianHalt {
                number: 1,
                block: 4,
            },
        ] {
            operator_confirms(confirmed);
            run_hook(6);
            assert!(state.http_requests().is_empty());
            assert_eq!(stopped_metric(), Some(1));
        }
    });
}

#[test]
fn a_halt_record_that_does_not_decode_keeps_the_workers_stopped() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        operator_confirms(FIRST);
        sp_io::storage::set(GUARDIAN_HALT_STORAGE_KEY, &[1, 0, 0]);
        run_hook(6);
        assert!(state.http_requests().is_empty());
        assert_eq!(stopped_metric(), Some(u64::MAX));
    });
}

#[test]
fn nodes_that_are_not_bridge_peers_ignore_halts() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        StorageValueRef::persistent(STORAGE_PEER_MARKER_KEY).clear();
        StorageValueRef::persistent(STORAGE_PEER_SECRET_KEY).clear();
        halt_on_chain(&FIRST);
        run_hook(6);
        // Validators without a bridge key get no halt warning or metric.
        assert_eq!(stopped_metric(), None);
        assert!(state.http_requests().is_empty());
    });
}

#[test]
fn roots_recovery_latch_still_stops_the_workers_on_its_own() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        halt_on_chain(&FIRST);
        operator_confirms(FIRST);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        run_hook(6);
        assert!(state.http_requests().is_empty());
    });
}

#[test]
fn guardian_reporting_continues_while_roots_latch_stops_work() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        halt_on_chain(&FIRST);
        run_hook(6);
        assert_eq!(stopped_metric(), Some(1));
        operator_confirms(FIRST);
        run_hook(7);
        assert_eq!(stopped_metric(), Some(0));
        halt_on_chain(&GuardianHalt {
            number: 2,
            block: 20,
        });
        run_hook(21);
        assert_eq!(stopped_metric(), Some(2));
        assert!(state.http_requests().is_empty());
    });
}

#[test]
fn hosts_without_a_live_guard_cannot_start_a_bridge_pass() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        StorageValueRef::persistent(crate::STORAGE_SERIAL_SIGNING_GUARD_KEY).clear();
        // A host providing just the v1 signing guard cannot serialize overdue passes.
        assert_eq!(
            StorageValueRef::persistent(crate::STORAGE_LIVE_SIGNING_GUARD_KEY)
                .get::<u8>()
                .unwrap(),
            Some(1)
        );
        run_hook(6);
        assert!(state.http_requests().is_empty());
    });
}
