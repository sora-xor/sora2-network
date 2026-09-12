// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use super::mock::{push_global_http_response, EthBridge, ExtBuilder, Runtime, State};
use super::{Error, ETH_NETWORK_ID};
use crate::types::Log;
use crate::MAX_GET_LOGS_ITEMS;
use sp_runtime::offchain::storage::StorageValueRef;

const LOG_RANGE_LIMIT_RESPONSE: &[u8] = br#"{"jsonrpc":"2.0","id":0,"error":{"code":-32602,"message":"eth_getLogs is limited to a 5 blocks range"}}"#;
const RANGE_LIMIT_RETRY_DELAY_MS: u64 = 30 * 60 * 1000;

fn scan_keys(network_id: u32) -> (String, String, String) {
    (
        format!("eth-bridge-ocw::eth-to-handle-from-height-{network_id:?}"),
        format!("eth-bridge-ocw::eth-log-range-size-{network_id:?}"),
        format!("eth-bridge-ocw::eth-log-range-limit-v1-{network_id:?}"),
    )
}

fn run_scan(state: &mut State, sidechain_height: u64) {
    state.run_next_offchain_with_params(
        sidechain_height,
        frame_system::Pallet::<Runtime>::block_number() + 1,
        false,
    );
}

fn requested_log_spans(state: &State) -> Vec<u64> {
    state
        .http_requests()
        .into_iter()
        .filter(|request| request["method"] == "eth_getLogs")
        .map(|request| {
            let filter = &request["params"][0];
            let from =
                u64::from_str_radix(&filter["fromBlock"].as_str().unwrap()[2..], 16).unwrap();
            let to = u64::from_str_radix(&filter["toBlock"].as_str().unwrap()[2..], 16).unwrap();
            to - from + 1
        })
        .collect()
}

#[test]
fn http_413_log_limit_is_classified_as_an_oversized_response() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_http_response(413, LOG_RANGE_LIMIT_RESPONSE.to_vec());
        let error = EthBridge::load_transfers_logs(ETH_NETWORK_ID, 100, 149)
            .expect_err("the provider rejected the log range with HTTP 413");
        assert_eq!(error, Error::HttpResponseTooLarge);
        assert!(error.should_retry());
    });
}

#[test]
fn http_413_log_limits_shrink_the_range_and_recover_without_skipping_blocks() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let height_key = format!(
            "eth-bridge-ocw::eth-to-handle-from-height-{:?}",
            ETH_NETWORK_ID
        );
        let range_key = format!("eth-bridge-ocw::eth-log-range-size-{:?}", ETH_NETWORK_ID);
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(range_key.as_bytes()).set(&MAX_GET_LOGS_ITEMS);
        assert_eq!(MAX_GET_LOGS_ITEMS, 50);

        // A provider with a five-block limit rejects 50, 25, 12, and six blocks.
        for expected_range in [25u64, 12, 6, 3] {
            state.push_http_response(413, LOG_RANGE_LIMIT_RESPONSE.to_vec());
            state.run_next_offchain_with_params(
                200,
                frame_system::Pallet::<Runtime>::block_number() + 1,
                false,
            );
            assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(100));
            assert_eq!(
                state.storage_read::<u64>(range_key.as_bytes()),
                Some(expected_range)
            );
        }

        state.push_response::<[Log; 0]>([]);
        state.run_next_offchain_with_params(
            200,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(103));

        let requests: Vec<_> = state
            .http_requests()
            .into_iter()
            .filter(|request| request["method"] == "eth_getLogs")
            .collect();
        assert_eq!(requests.len(), 5);
        for (request, to_block) in requests.iter().zip([149u64, 124, 111, 105, 102]) {
            assert_eq!(request["params"][0]["fromBlock"], "0x64");
            assert_eq!(request["params"][0]["toBlock"], format!("0x{to_block:x}"));
            assert_eq!(
                request["params"][0]["address"],
                requests[0]["params"][0]["address"]
            );
            assert_eq!(
                request["params"][0]["topics"],
                requests[0]["params"][0]["topics"]
            );
        }
    });
}

#[test]
fn http_401_and_429_log_failures_preserve_the_cursor_and_range() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let height_key = format!(
            "eth-bridge-ocw::eth-to-handle-from-height-{:?}",
            ETH_NETWORK_ID
        );
        let range_key = format!("eth-bridge-ocw::eth-log-range-size-{:?}", ETH_NETWORK_ID);
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(range_key.as_bytes()).set(&7u64);

        for (status, body) in [
            (401, br#"{"jsonrpc":"2.0","id":0,"error":{"code":-32000,"message":"Unauthorized"}}"#.as_slice()),
            (429, br#"{"jsonrpc":"2.0","id":0,"error":{"code":-32005,"message":"Rate limit exceeded"}}"#.as_slice()),
        ] {
            push_global_http_response(status, body.to_vec());
            assert_eq!(
                EthBridge::load_transfers_logs(ETH_NETWORK_ID, 100, 106),
                Err(Error::HttpFetchingError)
            );

            state.push_http_response(status, body.to_vec());
            state.run_next_offchain_with_params(
                200,
                frame_system::Pallet::<Runtime>::block_number() + 1,
                false,
            );
            assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(100));
            assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(7));
        }

        state.push_response::<[Log; 0]>([]);
        state.run_next_offchain_with_params(
            200,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            false,
        );
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(107));
    });
}

#[test]
fn http_413_for_non_log_methods_remains_an_http_failure() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        push_global_http_response(413, b"Request Entity Too Large".to_vec());
        assert_eq!(
            EthBridge::load_current_height(ETH_NETWORK_ID),
            Err(Error::HttpFetchingError)
        );
    });
}

#[test]
fn learned_log_range_limit_prevents_repeated_413_after_recovery() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let (height_key, range_key, limit_key) = scan_keys(ETH_NETWORK_ID);
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(range_key.as_bytes()).set(&6u64);

        state.push_http_response(413, LOG_RANGE_LIMIT_RESPONSE.to_vec());
        run_scan(&mut state, 1000);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(100));
        assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(3));

        for expected_height in [103u64, 108, 113, 118] {
            state.push_response::<[Log; 0]>([]);
            run_scan(&mut state, 1000);
            assert_eq!(
                state.storage_read::<u64>(height_key.as_bytes()),
                Some(expected_height)
            );
            assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(5));
        }
        assert_eq!(
            state.storage_read::<(u64, u64)>(limit_key.as_bytes()),
            Some((5, RANGE_LIMIT_RETRY_DELAY_MS))
        );
        assert_eq!(requested_log_spans(&state), vec![6, 3, 5, 5, 5]);
    });
}

#[test]
fn log_limit_reduction_uses_the_actual_confirmed_query_span() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let (height_key, range_key, limit_key) = scan_keys(ETH_NETWORK_ID);
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(range_key.as_bytes()).set(&50u64);
        state.push_http_response(413, LOG_RANGE_LIMIT_RESPONSE.to_vec());
        // Only 100..=105 is confirmed despite the configured 50-block range.
        run_scan(&mut state, 135);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(100));
        assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(3));
        assert_eq!(
            state.storage_read::<(u64, u64)>(limit_key.as_bytes()),
            Some((5, RANGE_LIMIT_RETRY_DELAY_MS))
        );
        state.push_response::<[Log; 0]>([]);
        run_scan(&mut state, 135);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(103));
        assert_eq!(requested_log_spans(&state), vec![6, 3]);
    });
}

#[test]
fn failed_log_limit_probe_restores_the_learned_ceiling_and_cooldown() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let (height_key, range_key, limit_key) = scan_keys(ETH_NETWORK_ID);
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(range_key.as_bytes()).set(&5u64);
        StorageValueRef::persistent(limit_key.as_bytes()).set(&(5u64, RANGE_LIMIT_RETRY_DELAY_MS));

        state.offchain_state.write().timestamp = (RANGE_LIMIT_RETRY_DELAY_MS - 1).into();
        state.push_response::<[Log; 0]>([]);
        run_scan(&mut state, 1000);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(105));
        assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(5));

        state.offchain_state.write().timestamp = RANGE_LIMIT_RETRY_DELAY_MS.into();
        state.push_http_response(413, LOG_RANGE_LIMIT_RESPONSE.to_vec());
        run_scan(&mut state, 1000);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(105));
        assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(5));
        assert_eq!(
            state.storage_read::<(u64, u64)>(limit_key.as_bytes()),
            Some((5, 2 * RANGE_LIMIT_RETRY_DELAY_MS))
        );

        state.push_response::<[Log; 0]>([]);
        run_scan(&mut state, 1000);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(110));
        assert_eq!(requested_log_spans(&state), vec![5, 10, 5]);
    });
}

#[test]
fn successful_log_limit_probe_restores_normal_range_growth() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let (height_key, range_key, limit_key) = scan_keys(ETH_NETWORK_ID);
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(range_key.as_bytes()).set(&5u64);
        StorageValueRef::persistent(limit_key.as_bytes()).set(&(5u64, RANGE_LIMIT_RETRY_DELAY_MS));
        state.offchain_state.write().timestamp = RANGE_LIMIT_RETRY_DELAY_MS.into();

        for expected_height in [110u64, 130, 170, 220] {
            state.push_response::<[Log; 0]>([]);
            run_scan(&mut state, 1000);
            assert_eq!(
                state.storage_read::<u64>(height_key.as_bytes()),
                Some(expected_height)
            );
            assert_eq!(state.storage_read::<(u64, u64)>(limit_key.as_bytes()), None);
        }
        assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(50));
        assert_eq!(requested_log_spans(&state), vec![10, 20, 40, 50]);
    });
}

#[test]
fn small_confirmed_scan_does_not_clear_an_expired_log_limit() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let (height_key, range_key, limit_key) = scan_keys(ETH_NETWORK_ID);
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(range_key.as_bytes()).set(&5u64);
        let limit = (5u64, RANGE_LIMIT_RETRY_DELAY_MS);
        StorageValueRef::persistent(limit_key.as_bytes()).set(&limit);
        state.offchain_state.write().timestamp = RANGE_LIMIT_RETRY_DELAY_MS.into();
        state.push_response::<[Log; 0]>([]);
        run_scan(&mut state, 132);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(103));
        assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(5));
        assert_eq!(
            state.storage_read::<(u64, u64)>(limit_key.as_bytes()),
            Some(limit)
        );
        assert_eq!(requested_log_spans(&state), vec![3]);
    });
}

#[test]
fn failed_above_ceiling_probe_defers_retry_and_restores_smaller_scans() {
    // Each response exercises the actual HTTP status or pending-request failure path.
    for status in [Some(401), Some(429), Some(500), None] {
        let (mut ext, mut state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            let (height_key, range_key, limit_key) = scan_keys(ETH_NETWORK_ID);
            StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
            StorageValueRef::persistent(range_key.as_bytes()).set(&5u64);
            StorageValueRef::persistent(limit_key.as_bytes())
                .set(&(5u64, RANGE_LIMIT_RETRY_DELAY_MS));
            state.offchain_state.write().timestamp = RANGE_LIMIT_RETRY_DELAY_MS.into();

            match status {
                Some(status) => {
                    state.push_http_response(status, b"provider request rejected".to_vec());
                }
                None => state.push_http_failure(),
            }
            run_scan(&mut state, 1000);
            assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(100));
            assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(5));
            assert_eq!(
                state.storage_read::<(u64, u64)>(limit_key.as_bytes()),
                Some((5, 2 * RANGE_LIMIT_RETRY_DELAY_MS))
            );

            state.push_response::<[Log; 0]>([]);
            run_scan(&mut state, 1000);
            assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(105));
            assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(5));

            state.offchain_state.write().timestamp = (2 * RANGE_LIMIT_RETRY_DELAY_MS - 1).into();
            state.push_response::<[Log; 0]>([]);
            run_scan(&mut state, 1000);
            assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(110));
            assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(5));

            state.offchain_state.write().timestamp = (2 * RANGE_LIMIT_RETRY_DELAY_MS).into();
            state.push_response::<[Log; 0]>([]);
            run_scan(&mut state, 1000);
            assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(120));
            assert_eq!(state.storage_read::<(u64, u64)>(limit_key.as_bytes()), None);
            assert_eq!(requested_log_spans(&state), vec![10, 5, 5, 10]);
        });
    }
}

#[test]
fn failed_small_confirmed_scan_does_not_defer_a_larger_probe() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let (height_key, range_key, limit_key) = scan_keys(ETH_NETWORK_ID);
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(range_key.as_bytes()).set(&5u64);
        let limit = (5u64, RANGE_LIMIT_RETRY_DELAY_MS);
        StorageValueRef::persistent(limit_key.as_bytes()).set(&limit);
        state.offchain_state.write().timestamp = RANGE_LIMIT_RETRY_DELAY_MS.into();

        state.push_http_response(500, b"temporarily unavailable".to_vec());
        run_scan(&mut state, 132);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(100));
        assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(5));
        assert_eq!(
            state.storage_read::<(u64, u64)>(limit_key.as_bytes()),
            Some(limit)
        );

        state.push_response::<[Log; 0]>([]);
        run_scan(&mut state, 1000);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(110));
        assert_eq!(state.storage_read::<(u64, u64)>(limit_key.as_bytes()), None);
        assert_eq!(requested_log_spans(&state), vec![3, 10]);
    });
}

#[test]
fn json_rpc_rate_and_quota_errors_do_not_learn_a_log_range_limit() {
    for message in [
        "Rate limit exceeded",
        "ran out of cu",
        "limit exceeded",
        "Too many requests",
    ] {
        let (mut ext, mut state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            let (height_key, range_key, limit_key) = scan_keys(ETH_NETWORK_ID);
            StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
            StorageValueRef::persistent(range_key.as_bytes()).set(&7u64);
            let body = serde_json::to_vec(&serde_json::json!({
                "jsonrpc": "2.0", "id": 0,
                "error": { "code": -32005, "message": message }
            }))
            .unwrap();
            push_global_http_response(200, body.clone());
            let error = EthBridge::load_transfers_logs(ETH_NETWORK_ID, 100, 106)
                .expect_err("rate and quota failures must remain retryable RPC errors");
            assert_eq!(error, Error::JsonDeserializationError, "{message}");
            assert!(error.should_retry());

            state.push_http_response(200, body);
            run_scan(&mut state, 1000);
            assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(100));
            assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(7));
            assert_eq!(state.storage_read::<(u64, u64)>(limit_key.as_bytes()), None);
        });
    }
}

#[test]
fn explicit_json_rpc_log_count_limit_still_reduces_the_range() {
    let (mut ext, mut state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let (height_key, range_key, limit_key) = scan_keys(ETH_NETWORK_ID);
        StorageValueRef::persistent(height_key.as_bytes()).set(&100u64);
        StorageValueRef::persistent(range_key.as_bytes()).set(&50u64);
        state.push_http_response(
            200,
            br#"{"jsonrpc":"2.0","id":0,"error":{"code":-32005,"message":"logs count exceeds the limit 50000"}}"#.to_vec(),
        );
        run_scan(&mut state, 1000);
        assert_eq!(state.storage_read::<u64>(height_key.as_bytes()), Some(100));
        assert_eq!(state.storage_read::<u64>(range_key.as_bytes()), Some(25));
        assert_eq!(
            state.storage_read::<(u64, u64)>(limit_key.as_bytes()),
            Some((49, RANGE_LIMIT_RETRY_DELAY_MS))
        );
    });
}

#[test]
fn learned_log_range_limits_are_isolated_per_network() {
    let mut builder = ExtBuilder::default();
    let second_network =
        builder.add_network(vec![], None, Some(1), sp_core::H160::repeat_byte(0x23));
    let first_config = &builder.networks[&ETH_NETWORK_ID];
    let peers = first_config.config.initial_peers.clone();
    let keypairs = first_config.ocw_keypairs.clone();
    let second_config = builder.networks.get_mut(&second_network).unwrap();
    second_config.config.initial_peers = peers;
    second_config.ocw_keypairs = keypairs;
    let (mut ext, mut state) = builder.build();

    ext.execute_with(|| {
        let (first_height, first_range, first_limit) = scan_keys(ETH_NETWORK_ID);
        let (second_height, second_range, second_limit) = scan_keys(second_network);
        StorageValueRef::persistent(first_height.as_bytes()).set(&100u64);
        StorageValueRef::persistent(first_range.as_bytes()).set(&5u64);
        let limit = (5u64, RANGE_LIMIT_RETRY_DELAY_MS);
        StorageValueRef::persistent(first_limit.as_bytes()).set(&limit);
        StorageValueRef::persistent(second_height.as_bytes()).set(&200u64);
        StorageValueRef::persistent(second_range.as_bytes()).set(&50u64);

        state.push_response::<[Log; 0]>([]);
        state.push_response(crate::types::U64::from(1000u64));
        state.push_response::<[Log; 0]>([]);
        run_scan(&mut state, 1000);

        assert_eq!(
            state.storage_read::<u64>(first_height.as_bytes()),
            Some(105)
        );
        assert_eq!(
            state.storage_read::<u64>(second_height.as_bytes()),
            Some(250)
        );
        assert_eq!(state.storage_read::<u64>(first_range.as_bytes()), Some(5));
        assert_eq!(state.storage_read::<u64>(second_range.as_bytes()), Some(50));
        assert_eq!(
            state.storage_read::<(u64, u64)>(first_limit.as_bytes()),
            Some(limit)
        );
        assert_eq!(
            state.storage_read::<(u64, u64)>(second_limit.as_bytes()),
            None
        );
        assert_eq!(requested_log_spans(&state), vec![5, 50]);
    });
}
