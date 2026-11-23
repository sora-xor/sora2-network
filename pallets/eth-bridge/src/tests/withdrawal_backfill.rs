use super::withdrawal_logs::{receipt, signed_import, store_ready_transfer, withdrawal_log};
use crate::requests::{IncomingRequest, RequestStatus};
use crate::tests::{mock::*, ETH_NETWORK_ID};
use crate::types::{Log, TransactionReceipt};
use crate::{eth_scan_cursor_key, RequestStatuses, CONFIRMATION_INTERVAL};
use serde_json::json;
use sp_core::{H160, H256};
use sp_runtime::offchain::storage::StorageValueRef;

fn run_history_slot(live_from: u64, head: u64) {
    for _ in 0..8 {
        if EthBridge::backfill_withdrawals(ETH_NETWORK_ID, live_from, head) {
            return;
        }
    }
    panic!("expected a historical query within eight worker passes");
}

fn history_ranges(state: &State) -> Vec<(u64, u64)> {
    let height = |v: &serde_json::Value| {
        u64::from_str_radix(v.as_str().unwrap().strip_prefix("0x").unwrap(), 16).unwrap()
    };
    state
        .http_requests()
        .iter()
        .filter(|rpc| rpc["method"] == "eth_getLogs")
        .map(|rpc| {
            let filter = &rpc["params"][0];
            (height(&filter["fromBlock"]), height(&filter["toBlock"]))
        })
        .collect()
}

fn submitted_withdrawals(state: &State) -> Vec<H256> {
    state
        .pending_txs()
        .into_values()
        .map(|tx| {
            let (_, incoming) = signed_import(tx.call);
            let IncomingRequest::MarkAsDone(mark) = incoming else {
                panic!("expected a withdrawal acknowledgement");
            };
            mark.outgoing_request_hash
        })
        .collect()
}

#[test]
fn worker_backfills_missed_withdrawal_through_multisig_without_rewinding_live_scan() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(1), H160::repeat_byte(9));
    let (mut ext, mut state) = builder.build();
    ext.execute_with(|| {
        let net = ETH_NETWORK_ID;
        let hash = store_ready_transfer(1);
        let log = withdrawal_log(hash);
        let cursor = eth_scan_cursor_key(net);
        StorageValueRef::persistent(&cursor).set(&11u64);
        for height in 11..18 {
            state.push_response(Vec::<Log>::new());
            state.run_next_offchain_worker_with_params(
                height + CONFIRMATION_INTERVAL,
                System::block_number() + 1,
                true,
            );
        }
        assert_eq!(state.storage_read::<u64>(&cursor), Some(18));
        assert_eq!(
            RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::ApprovalsReady)
        );
        state.push_response(vec![log.clone()]);
        state.push_response(receipt(&log));
        state.run_next_offchain_worker_with_params(
            18 + CONFIRMATION_INTERVAL,
            System::block_number() + 1,
            true,
        );
        assert_eq!(
            RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::Done)
        );
        assert_eq!(state.storage_read::<u64>(&cursor), Some(18));
        let rpc = state.http_requests();
        let logs: Vec<_> = rpc
            .iter()
            .filter(|rpc| rpc["method"] == "eth_getLogs")
            .collect();
        assert_eq!(logs.len(), 8);
        let historical_filter = &logs[7]["params"][0];
        assert_eq!(
            historical_filter,
            &json!({
                "address": log.address, "fromBlock": "0x0", "toBlock": "0xa", "topics": log.topics,
            })
        );
        assert_eq!(
            rpc.last().unwrap()["params"],
            json!([log.transaction_hash.unwrap()])
        );

        state.push_response(Vec::<Log>::new());
        state.run_next_offchain_worker_with_params(
            19 + CONFIRMATION_INTERVAL,
            System::block_number() + 1,
            true,
        );
        assert_eq!(state.storage_read::<u64>(&cursor), Some(20));
    });
}

#[test]
fn historical_scan_resumes_between_withdrawals_in_the_same_block() {
    let (mut ext, state) = ExtBuilder::default().build();
    let (first_hash, second_hash, logs, tx) = ext.execute_with(|| {
        let first_hash = store_ready_transfer(1);
        let second_hash = store_ready_transfer(2);
        let first = withdrawal_log(first_hash);
        let mut second = withdrawal_log(second_hash);
        second.log_index = Some(12.into());
        let logs = vec![first, second];
        let mut tx = receipt(&logs[0]);
        tx.logs = logs.clone();
        push_global_json_rpc_response(logs.clone());
        push_global_json_rpc_response(tx.clone());
        run_history_slot(8, 100);
        assert_eq!(submitted_withdrawals(&state), vec![second_hash]);
        (first_hash, second_hash, logs, tx)
    });

    ext.execute_with(|| {
        // The previous import is still in the pool. Its saved log position
        // should survive the new invocation and the change in live cursor.
        push_global_json_rpc_response(logs.clone());
        push_global_json_rpc_response(tx);
        run_history_slot(1000, 2000);
        let imports = submitted_withdrawals(&state);
        assert_eq!(imports.len(), 2);
        assert!(imports.contains(&first_hash) && imports.contains(&second_hash));
        assert_eq!(history_ranges(&state), vec![(0, 7), (0, 5)]);

        // The second query included genesis and exhausted the remaining logs.
        for _ in 0..16 {
            assert!(!EthBridge::backfill_withdrawals(ETH_NETWORK_ID, 1000, 2000));
        }
    });
}

#[test]
fn backfill_retries_rejected_range_and_missing_receipt_before_advancing() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let hash = store_ready_transfer(1);
        let mut log = withdrawal_log(hash);
        log.block_number = Some(7999.into());
        push_global_http_response(413, vec![]);
        run_history_slot(8000, 9000);
        let (first_lower, first_upper) = history_ranges(&state)[0];
        assert_eq!(first_upper, 7999);
        push_global_json_rpc_response(vec![log.clone()]);
        push_global_json_rpc_response(Option::<TransactionReceipt>::None);
        run_history_slot(9000, 10000);
        let reduced = history_ranges(&state)[1];
        assert_eq!(reduced.1, first_upper);
        assert!(reduced.0 > first_lower);
        assert!(submitted_withdrawals(&state).is_empty());

        push_global_json_rpc_response(vec![log.clone()]);
        push_global_json_rpc_response(receipt(&log));
        run_history_slot(10000, 11000);
        assert_eq!(submitted_withdrawals(&state), vec![hash]);
        assert_eq!(history_ranges(&state)[2], reduced);
    });
}

#[test]
fn backfill_waits_for_confirmations_and_handles_genesis_once() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        for _ in 0..16 {
            assert!(!EthBridge::backfill_withdrawals(
                ETH_NETWORK_ID,
                1,
                CONFIRMATION_INTERVAL - 1
            ));
        }
        assert!(state.http_requests().is_empty());
        push_global_json_rpc_response(Vec::<Log>::new());
        run_history_slot(99, CONFIRMATION_INTERVAL);
        assert_eq!(history_ranges(&state), vec![(0, 0)]);
        for _ in 0..16 {
            assert!(!EthBridge::backfill_withdrawals(ETH_NETWORK_ID, 99, 100));
        }
        // A second network whose live cursor started at genesis has no gap.
        for _ in 0..16 {
            assert!(!EthBridge::backfill_withdrawals(ETH_NETWORK_ID + 1, 0, 100));
        }
        assert_eq!(state.http_requests().len(), 1);
    });
}
