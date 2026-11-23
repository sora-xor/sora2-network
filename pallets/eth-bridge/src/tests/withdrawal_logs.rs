use super::coverage_paths::deposit_log;
use crate::requests::{
    IncomingMetaRequestKind, IncomingRequest, LoadIncomingMetaRequest, LoadIncomingRequest,
    OffchainRequest, OutgoingTransfer, RequestStatus,
};
use crate::tests::{mock::*, ETH_NETWORK_ID};
use crate::types::{self, Log, TransactionReceipt};
use crate::{Call, RequestStatuses, Requests};
use codec::Decode;
use common::mock::alice;
use frame_support::assert_ok;
use serde_json::json;
use sp_core::{H160, H256};

pub(super) fn store_ready_transfer(nonce: u64) -> H256 {
    let transfer = OutgoingTransfer::<Runtime> {
        from: alice(),
        to: H160::repeat_byte(3),
        asset_id: common::PSWAP,
        amount: 10,
        nonce,
        network_id: ETH_NETWORK_ID,
        timepoint: Default::default(),
    };
    let request = OffchainRequest::from(transfer);
    let hash = request.hash();
    Requests::<Runtime>::insert(ETH_NETWORK_ID, hash, request);
    RequestStatuses::<Runtime>::insert(ETH_NETWORK_ID, hash, RequestStatus::ApprovalsReady);
    hash
}

pub(super) fn withdrawal_log(outgoing_hash: H256) -> Log {
    Log {
        address: types::H160(EthBridge::bridge_contract_address(ETH_NETWORK_ID).0),
        topics: vec![types::H256(sp_io::hashing::keccak_256(
            b"Withdrawal(bytes32)",
        ))],
        data: outgoing_hash.as_bytes().to_vec().into(),
        block_hash: Some(types::H256::repeat_byte(4)),
        block_number: Some(5.into()),
        transaction_hash: Some(types::H256::repeat_byte(6)),
        transaction_index: Some(3.into()),
        log_index: Some(11.into()),
        removed: Some(false),
        ..Default::default()
    }
}

pub(super) fn receipt(log: &Log) -> TransactionReceipt {
    TransactionReceipt {
        transaction_hash: log.transaction_hash.unwrap(),
        transaction_index: log.transaction_index.unwrap(),
        block_hash: log.block_hash,
        block_number: log.block_number,
        from: types::H160::repeat_byte(7),
        // A router may be the outer recipient while the bridge emits the log.
        to: Some(types::H160::repeat_byte(8)),
        status: Some(1.into()),
        logs: vec![log.clone()],
        ..Default::default()
    }
}

pub(super) fn signed_import(
    call: RuntimeCall,
) -> (LoadIncomingRequest<Runtime>, IncomingRequest<Runtime>) {
    let RuntimeCall::Multisig(bridge_multisig::Call::as_multi {
        maybe_timepoint,
        call,
        ..
    }) = call
    else {
        panic!("expected a peer multisig submission");
    };
    let RuntimeCall::EthBridge(Call::import_incoming_request {
        load_incoming_request,
        incoming_request_result,
    }) = RuntimeCall::decode(&mut &call[..]).unwrap()
    else {
        panic!("expected an incoming import");
    };
    assert_eq!(maybe_timepoint, Some(load_incoming_request.timepoint()));
    (load_incoming_request, incoming_request_result.unwrap())
}

#[test]
fn confirmed_withdrawal_import_marks_the_original_outgoing_transfer_done() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net = ETH_NETWORK_ID;
        let outgoing_hash = store_ready_transfer(0);
        let log = withdrawal_log(outgoing_hash);
        let receipt = receipt(&log);
        let evm_hash = receipt.transaction_hash;
        let bridge = EthBridge::bridge_account(net).unwrap();
        let timepoint = bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(5, 3);
        let expected_load = OffchainRequest::load_incoming_meta(LoadIncomingMetaRequest::new(
            alice(),
            outgoing_hash,
            timepoint,
            IncomingMetaRequestKind::MarkAsDone,
            net,
        ));

        push_global_json_rpc_response(vec![log.clone(), log.clone()]);
        push_global_json_rpc_response(receipt);
        let mut cursor = 5;
        assert_ok!(EthBridge::handle_logs(5, 7, &mut cursor, net));
        assert_eq!(cursor, 8);
        let rpc = state.http_requests();
        assert_eq!(rpc.len(), 2);
        assert_eq!(rpc[0]["method"], "eth_getLogs");
        let filter = &rpc[0]["params"][0];
        assert_eq!(filter["address"], json!(log.address));
        let topics = filter["topics"][0].as_array().unwrap();
        assert_eq!(topics.len(), 2);
        assert!(topics.contains(&json!(crate::DEPOSIT_TOPIC)));
        assert!(topics.contains(&json!(log.topics[0])));
        assert_eq!(rpc[1]["method"], "eth_getTransactionReceipt");
        assert_eq!(rpc[1]["params"], json!([evm_hash]));

        let transactions = state.pending_txs();
        assert_eq!(transactions.len(), 1);
        let (load, incoming) = signed_import(transactions.into_values().next().unwrap().call);
        let IncomingRequest::MarkAsDone(mark) = &incoming else {
            panic!("expected a withdrawal acknowledgement");
        };
        assert_eq!(mark.outgoing_request_hash, outgoing_hash);
        assert_eq!(mark.initial_request_hash, expected_load.hash());
        assert_eq!(mark.at_height, 5);
        assert_eq!(mark.timepoint, timepoint);
        assert_eq!(mark.author, alice());
        assert_eq!(OffchainRequest::LoadIncoming(load.clone()), expected_load);
        let call = Call::import_incoming_request {
            load_incoming_request: load.clone(),
            incoming_request_result: Ok(incoming.clone()),
        };
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&bridge, &call),
            Ok(true)
        );
        assert_ok!(EthBridge::import_incoming_request(
            RuntimeOrigin::signed(bridge),
            load,
            Ok(incoming),
        ));
        assert_eq!(
            RequestStatuses::<Runtime>::get(net, outgoing_hash),
            Some(RequestStatus::Done)
        );
        assert!(!RequestStatuses::<Runtime>::contains_key(
            net,
            H256(evm_hash.0)
        ));

        push_global_json_rpc_response(vec![log]);
        cursor = 5;
        assert_ok!(EthBridge::handle_logs(5, 7, &mut cursor, net));
        assert_eq!(cursor, 8);
        assert_eq!(state.http_requests().len(), 3);
        assert_eq!(state.pool_state.read().transactions.len(), 1);
    });
}

#[test]
fn recovery_scans_both_deposit_and_withdrawal_in_one_evm_transaction() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        sp_io::storage::set(common::recovery::POLICY_STORAGE_KEY, &[1]);
        let hash = store_ready_transfer(0);
        let withdrawal = withdrawal_log(hash);
        let mut receipt = receipt(&withdrawal);
        let mut deposit = deposit_log(H256(receipt.transaction_hash.0));
        deposit.block_hash = receipt.block_hash;
        deposit.transaction_index = Some(receipt.transaction_index);
        deposit.log_index = Some(12.into());
        receipt.logs.push(deposit);
        push_global_json_rpc_response(receipt.logs.clone());
        push_global_json_rpc_response(receipt.clone());
        push_global_json_rpc_response(receipt);
        let mut cursor = 5;
        assert_ok!(EthBridge::handle_logs(5, 7, &mut cursor, ETH_NETWORK_ID));
        assert_eq!(cursor, 8);
        let imports: Vec<_> = state
            .pending_txs()
            .into_values()
            .map(|tx| signed_import(tx.call).1)
            .collect();
        assert_eq!(imports.len(), 2);
        let evm_hash = H256(withdrawal.transaction_hash.unwrap().0);
        assert!(imports.iter().any(|request| matches!(
            request, IncomingRequest::MarkAsDone(mark) if mark.outgoing_request_hash == hash
        )));
        assert!(imports.iter().any(|request| matches!(
            request, IncomingRequest::Transfer(transfer) if transfer.tx_hash == evm_hash
        )));
    });
}

#[test]
fn removed_foreign_malformed_and_unready_withdrawals_are_skipped() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let hash = store_ready_transfer(0);
        let log = withdrawal_log(hash);
        let mut foreign = log.clone();
        foreign.address = types::H160(EthBridge::val_master_contract_address().0);
        let mut removed = log.clone();
        removed.removed = Some(true);
        let mut oversized = log.clone();
        oversized.data.0.push(0);
        let mut indexed = log.clone();
        indexed.topics.push(types::H256::zero());
        let unknown = withdrawal_log(H256::repeat_byte(0xab));
        push_global_json_rpc_response(vec![foreign, removed, oversized, indexed, unknown]);
        let mut cursor = 5;
        assert_ok!(EthBridge::handle_logs(5, 7, &mut cursor, ETH_NETWORK_ID));
        assert_eq!(cursor, 8);
        RequestStatuses::<Runtime>::insert(ETH_NETWORK_ID, hash, RequestStatus::Pending);
        push_global_json_rpc_response(vec![log]);
        cursor = 5;
        assert_ok!(EthBridge::handle_logs(5, 7, &mut cursor, ETH_NETWORK_ID));
        assert_eq!(cursor, 8);
        assert_eq!(state.http_requests().len(), 2);
        assert!(state.pool_state.read().transactions.is_empty());
    });
}

#[test]
fn withdrawal_requires_a_successful_receipt_matching_the_observed_log() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let hash = store_ready_transfer(0);
        let log = withdrawal_log(hash);
        // A provider must not be able to return an event beyond the confirmed query tip.
        let mut unconfirmed = log.clone();
        unconfirmed.block_number = Some(8.into());
        push_global_json_rpc_response(vec![unconfirmed]);
        let mut cursor = 5;
        assert!(EthBridge::handle_logs(5, 7, &mut cursor, ETH_NETWORK_ID).is_err());
        assert_eq!(cursor, 5);
        assert_eq!(state.http_requests().len(), 1);
        let good = receipt(&log);
        let mut failed = good.clone();
        failed.status = Some(0.into());
        let mut reorged = good.clone();
        reorged.block_hash = Some(types::H256::repeat_byte(9));
        let mut wrong_index = good.clone();
        wrong_index.logs[0].log_index = Some(99.into());
        let mut different = good;
        different.logs[0].data = vec![0xf1; 32].into();
        for response in [
            Some(failed),
            Some(reorged),
            Some(wrong_index),
            Some(different),
            None,
        ] {
            push_global_json_rpc_response(vec![log.clone()]);
            push_global_json_rpc_response(response);
            let mut cursor = 5;
            assert!(EthBridge::handle_logs(5, 7, &mut cursor, ETH_NETWORK_ID).is_err());
            assert_eq!(cursor, 5);
            assert!(state.pool_state.read().transactions.is_empty());
            assert_eq!(
                RequestStatuses::<Runtime>::get(ETH_NETWORK_ID, hash),
                Some(RequestStatus::ApprovalsReady)
            );
        }
    });
}
