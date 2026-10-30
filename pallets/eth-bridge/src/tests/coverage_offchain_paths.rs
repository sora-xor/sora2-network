use super::*;
use crate::requests::{encode_outgoing_request_eth_call, OutgoingTransfer, RequestStatus};
use crate::tests::{mock::*, Error};
use crate::types::{self, Log, SubstrateHeaderLimited, Transaction, TransactionReceipt};
use common::mock::alice;
use ethabi::Token;
use frame_support::{assert_ok, traits::Hooks, weights::Weight};
use sp_core::H160;
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
use sp_runtime::offchain::storage_lock::{StorageLock, Time};

pub(super) fn header(number: u64) -> SubstrateHeaderLimited {
    SubstrateHeaderLimited {
        parent_hash: Default::default(),
        number: number.into(),
        state_root: Default::default(),
        extrinsics_root: Default::default(),
        digest: (),
    }
}

pub(super) fn outgoing() -> OutgoingRequest<Runtime> {
    OutgoingRequest::Transfer(OutgoingTransfer {
        from: alice(),
        to: H160::repeat_byte(3),
        asset_id: common::XOR,
        amount: 10,
        nonce: 0,
        network_id: 0,
        timepoint: Default::default(),
    })
}

pub(super) fn deposit_log(hash: H256) -> Log {
    Log {
        address: types::H160(EthBridge::bridge_contract_address(0).0),
        removed: Some(false),
        topics: vec![types::H256(crate::DEPOSIT_TOPIC.0)],
        data: ethabi::encode(&[
            Token::FixedBytes(alice().encode()),
            Token::Uint(10.into()),
            Token::Address(types::H160(
                crate::RegisteredSidechainToken::<Runtime>::get(0, common::XOR)
                    .unwrap()
                    .0,
            )),
            Token::FixedBytes(vec![0; 32]),
        ])
        .into(),
        transaction_hash: Some(types::H256(hash.0)),
        block_number: Some(5.into()),
        transaction_index: Some(0.into()),
        ..Default::default()
    }
}

#[test]
fn finalized_extrinsic_is_removed_without_resubmission() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let block = SubstrateBlockLimited {
            header: header(10),
            extrinsics: vec![Decode::decode(&mut &vec![1u8, 2, 3].encode()[..]).unwrap()],
        };
        let hash = H256(blake2_256(&block.extrinsics[0].encode()));
        let call = frame_system::Call::<Runtime>::remark { remark: vec![9] };
        let pending = BTreeMap::from([(
            hash,
            SignedTransactionData::<Runtime>::new(hash, None, call),
        )]);
        StorageValueRef::persistent(STORAGE_PENDING_TRANSACTIONS_KEY).set(&pending);
        assert_ok!(EthBridge::handle_substrate_block(block, 10));
        assert!(state.pending_txs().is_empty());
        assert!(state.pool_state.read().transactions.is_empty());
    });
}

#[test]
fn substrate_cursor_survives_unavailable_block_and_rejects_corruption() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        push_global_http_failure();
        assert_eq!(EthBridge::handle_substrate(), Err(Error::HttpFetchingError));
        let cursor = StorageValueRef::persistent(STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY);
        cursor.set(&1u8);
        push_global_json_rpc_response(H256::zero());
        push_global_json_rpc_response(header(10));
        assert_eq!(EthBridge::handle_substrate(), Err(Error::ReadStorageError));
        cursor.set(&8u64);
        push_global_json_rpc_response(H256::zero());
        push_global_json_rpc_response(header(10));
        push_global_http_failure();
        assert_eq!(EthBridge::handle_substrate(), Ok(10));
        assert_eq!(cursor.get::<u64>().unwrap(), Some(8));
    });
}

#[test]
fn log_scanning_skips_foreign_malformed_and_already_imported_events() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let hash = H256::repeat_byte(8);
        let seen = deposit_log(hash);
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        let mut foreign = seen.clone();
        foreign.address = types::H160::repeat_byte(0xee);
        let mut malformed = seen.clone();
        malformed.data = vec![1].into();
        let mut removed = seen.clone();
        removed.removed = Some(true);
        push_global_json_rpc_response(vec![foreign, malformed, removed, seen]);
        let mut cursor = 5;
        assert_ok!(EthBridge::handle_logs(5, 7, &mut cursor, 0));
        assert_eq!(cursor, 8);
        assert!(state.pool_state.read().transactions.is_empty());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(0, hash),
            Some(RequestStatus::Pending)
        );
    });
}

#[test]
fn cancellation_handler_submits_the_parsed_request() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let outgoing = outgoing();
        let request = OffchainRequest::outgoing(outgoing.clone());
        let hash = request.hash();
        crate::Requests::<Runtime>::insert(0, hash, request);
        crate::contract::FUNCTIONS.get_or_init(crate::contract::functions);
        let input = encode_outgoing_request_eth_call::<Runtime>(
            *crate::contract::RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID
                .get()
                .unwrap(),
            &outgoing,
            hash,
        )
        .unwrap();
        let tx_hash = H256::repeat_byte(4);
        let load = LoadIncomingMetaRequest::<Runtime> {
            author: alice(),
            hash: tx_hash,
            kind: IncomingMetaRequestKind::CancelOutgoingRequest,
            timepoint: Default::default(),
            network_id: 0,
        };
        push_global_json_rpc_response(TransactionReceipt {
            transaction_hash: types::H256(tx_hash.0),
            to: Some(types::H160(EthBridge::bridge_contract_address(0).0)),
            status: Some(0.into()),
            block_number: Some(55.into()),
            gas_used: Some(90.into()),
            ..Default::default()
        });
        push_global_json_rpc_response(Transaction {
            hash: types::H256(tx_hash.0),
            to: Some(types::H160(EthBridge::bridge_contract_address(0).0)),
            gas: 100.into(),
            input: input.into(),
            ..Default::default()
        });
        assert_ok!(EthBridge::handle_offchain_request(
            OffchainRequest::LoadIncoming(LoadIncomingRequest::Meta(load, hash))
        ));
        assert_eq!(state.pending_txs().len(), 1);
        assert_eq!(state.pool_state.read().transactions.len(), 1);
        let tx = state.pending_txs().into_values().next().unwrap();
        let RuntimeCall::Multisig(bridge_multisig::Call::as_multi { call, .. }) = tx.call else {
            panic!("expected multisig registration")
        };
        let RuntimeCall::EthBridge(Call::register_incoming_request {
            incoming_request, ..
        }) = RuntimeCall::decode(&mut &call[..]).unwrap()
        else {
            panic!("expected incoming registration")
        };
        let IncomingRequest::CancelOutgoingRequest(request) = incoming_request else {
            panic!("expected cancellation")
        };
        assert_eq!(request.outgoing_request_hash, hash);
        assert_eq!(request.tx_hash, tx_hash);
    });
}

#[test]
fn failed_multisig_queue_retries_stored_call_and_ignores_non_imports() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let id = EthBridge::bridge_account(0).unwrap();
        let hash = H256::repeat_byte(40);
        let load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
            alice(),
            hash,
            Default::default(),
            IncomingTransactionRequestKind::Transfer,
            0,
        ));
        let inner: RuntimeCall = Call::<Runtime>::import_incoming_request {
            load_incoming_request: load,
            incoming_request_result: Err(Error::EthTransactionIsFailed.into()),
        }
        .into();
        let retry = bridge_multisig::Call::<Runtime>::as_multi {
            id: id.clone(),
            maybe_timepoint: None,
            call: inner.encode(),
            store_call: true,
            max_weight: Weight::zero(),
        };
        let irrelevant = bridge_multisig::Call::<Runtime>::as_multi_threshold_1 {
            id: id.clone(),
            timepoint: Default::default(),
            call: Box::new(
                Call::<Runtime>::abort_request {
                    hash,
                    error: Error::UnknownRequest.into(),
                    network_id: 0,
                }
                .into(),
            ),
        };
        let unsupported = bridge_multisig::Call::<Runtime>::approve_as_multi {
            id,
            maybe_timepoint: None,
            call_hash: [1; 32],
            max_weight: Weight::zero(),
        };
        let failed = BTreeMap::from([
            (
                hash,
                SignedTransactionData::<Runtime>::new(hash, None, retry),
            ),
            (
                H256::repeat_byte(41),
                SignedTransactionData::<Runtime>::new(H256::repeat_byte(41), None, irrelevant),
            ),
            (
                H256::repeat_byte(42),
                SignedTransactionData::<Runtime>::new(H256::repeat_byte(42), None, unsupported),
            ),
        ]);
        let storage = StorageValueRef::persistent(STORAGE_FAILED_PENDING_TRANSACTIONS_KEY);
        storage.set(&failed);
        EthBridge::handle_failed_transactions_queue();
        assert_eq!(state.pool_state.read().transactions.len(), 1);
        assert_eq!(state.failed_pending_txs().len(), 3);
        crate::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        EthBridge::handle_failed_transactions_queue();
        assert_eq!(state.pool_state.read().transactions.len(), 1);
        assert_eq!(state.failed_pending_txs().len(), 2);
        assert!(!state.failed_pending_txs().contains_key(&hash));
    });
}

#[test]
fn missing_signing_key_prevents_outgoing_approval_retry_and_hook_bootstrap() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.register_extension(KeystoreExt::new(MemoryKeystore::new()));
    ext.execute_with(|| {
        let request = outgoing();
        let hash = OffchainRequest::outgoing(request.clone()).hash();
        assert_eq!(
            EthBridge::handle_outgoing_request(request, hash),
            Err(Error::NoLocalAccountForSigning)
        );
        assert!(matches!(
            EthBridge::get_signer(),
            Err(Error::NoLocalAccountForSigning)
        ));
        let mut cached = SignedTransactionData::<Runtime>::new(
            hash,
            None,
            frame_system::Call::<Runtime>::remark { remark: vec![] },
        );
        let signer = Signer::<Runtime, crate::offchain::crypto::TestAuthId>::any_account();
        assert!(!cached.resend(&signer));
        <EthBridge as Hooks<BlockNumber>>::offchain_worker(1);
        assert_eq!(
            StorageValueRef::persistent(crate::STORAGE_BOOTSTRAP_READY_KEY)
                .get::<u64>()
                .unwrap(),
            Some(0)
        );
        assert_eq!(
            StorageValueRef::persistent(crate::STORAGE_LOCAL_SIGNING_KEY_READY_KEY)
                .get::<u64>()
                .unwrap(),
            Some(0)
        );
        assert!(state.pool_state.read().transactions.is_empty());
    });
}

#[test]
fn non_peer_key_cannot_approve_or_bootstrap_any_network() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::Peers::<Runtime>::remove(0);
        let request = outgoing();
        let hash = OffchainRequest::outgoing(request.clone()).hash();
        assert_eq!(
            EthBridge::handle_outgoing_request(request, hash),
            Err(Error::NoLocalAccountForSigning)
        );
        EthBridge::handle_network(0, 1);
        <EthBridge as Hooks<BlockNumber>>::offchain_worker(1);
        assert_eq!(
            StorageValueRef::persistent(crate::STORAGE_BOOTSTRAP_READY_KEY)
                .get::<u64>()
                .unwrap(),
            Some(0)
        );
        let metric = EthBridge::per_network_reason_metric_key(
            STORAGE_OUTGOING_APPROVAL_FAILURES_KEY,
            0,
            OUTGOING_APPROVAL_FAILURE_NO_LOCAL_PEER_KEY,
        );
        assert_eq!(
            StorageValueRef::persistent(&metric).get::<u64>().unwrap(),
            Some(1)
        );
        assert!(state.http_requests().is_empty());
        assert!(state.pool_state.read().transactions.is_empty());
    });
}

#[test]
fn legacy_seed_marker_bootstraps_but_worker_respects_lock_and_halt() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        StorageValueRef::persistent(crate::STORAGE_PEER_MARKER_KEY).clear();
        StorageValueRef::persistent(crate::STORAGE_PEER_SECRET_KEY).set(&vec![7u8; 32]);
        let mut lock = StorageLock::<'_, Time>::with_deadline(
            b"eth-bridge-ocw::lock",
            sp_core::offchain::Duration::from_millis(100000),
        );
        let _guard = lock.try_lock().unwrap();
        <EthBridge as Hooks<BlockNumber>>::offchain_worker(1);
        assert_eq!(
            StorageValueRef::persistent(crate::STORAGE_BOOTSTRAP_READY_KEY)
                .get::<u64>()
                .unwrap(),
            Some(1)
        );
        assert!(state.http_requests().is_empty());
        sp_io::storage::set(crate::recovery::HALT_KEY, &[]);
        <EthBridge as Hooks<BlockNumber>>::offchain_worker(2);
        EthBridge::offchain();
        assert!(state.http_requests().is_empty());
    });
}

#[test]
fn rpc_helpers_validate_params_encoding_auth_and_default_substrate_url() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let body = b"{}".to_vec();
        push_global_raw_response(b"response");
        assert_eq!(
            EthBridge::http_request("http://test", body.clone(), &[], 8),
            Ok(b"response".to_vec())
        );
        push_global_raw_response(b"toolong");
        assert_eq!(
            EthBridge::http_request("http://test", body, &[], 1),
            Err(Error::HttpResponseTooLarge)
        );
        assert_eq!(
            EthBridge::json_rpc_request::<_, u64>("http://test", 0, "number", &3u64, &[]),
            Err(Error::JsonSerializationError)
        );
        push_global_raw_response(&[255]);
        assert_eq!(
            EthBridge::json_rpc_request::<_, u64>("http://test", 0, "number", &(), &[]),
            Err(Error::HttpFetchingError)
        );
        let params = serde_json::json!({"number": 5});
        push_global_json_rpc_response(10u64);
        assert_eq!(
            EthBridge::json_rpc_request::<_, u64>("http://test", 0, "number", &params, &[]),
            Ok(10)
        );
        assert_eq!(state.http_requests().last().unwrap()["params"], params);
        let key = format!("{}-0", crate::STORAGE_ETH_NODE_PARAMS);
        StorageValueRef::persistent(key.as_bytes()).set(&crate::NodeParams {
            url: "http://credential-test".into(),
            credentials: Some("Basic test".into()),
        });
        push_global_json_rpc_response(11u64);
        assert_eq!(
            EthBridge::eth_json_rpc_request::<_, u64>("number", &(), 0),
            Ok(11)
        );
        StorageValueRef::persistent(crate::STORAGE_SUB_NODE_URL_KEY).clear();
        push_global_json_rpc_response(12u64);
        assert_eq!(
            EthBridge::substrate_json_rpc_request::<_, u64>("number", &()),
            Ok(12)
        );
        push_global_http_failure();
        assert_eq!(
            EthBridge::load_is_used(H256::zero(), 0),
            Err(Error::HttpFetchingError)
        );
    });
}

#[test]
fn legacy_xor_guard_and_missing_multisig_account_fail_without_http() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::LegacyEthereumXorDecommissioned::<Runtime>::put(true);
        let load = LoadIncomingTransactionRequest::new(
            alice(),
            H256::repeat_byte(1),
            Default::default(),
            IncomingTransactionRequestKind::TransferXOR,
            0,
        );
        assert_eq!(
            EthBridge::handle_offchain_request(OffchainRequest::LoadIncoming(
                LoadIncomingRequest::Transaction(load)
            )),
            Err(Error::DeprecatedLegacyXor)
        );
        assert!(!EthBridge::handle_pending_multisig_calls(99, 1000));
        assert_eq!(
            EthBridge::send_abort_request(
                H256::zero(),
                Error::UnknownRequest,
                Default::default(),
                0
            ),
            Err(Error::ExpectedPendingRequest)
        );
        assert!(state.http_requests().is_empty());
    });
}

#[test]
fn protocol_signer_selection_distinguishes_stored_bridge_and_generic_calls() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let id = EthBridge::bridge_account(0).unwrap();
        let user_call = Call::<Runtime>::transfer_to_sidechain {
            asset_id: common::XOR,
            to: H160::repeat_byte(2),
            amount: 10,
            network_id: 0,
        };
        assert_ok!(EthBridge::send_transaction(user_call.clone()));
        for call in [
            RuntimeCall::System(frame_system::Call::remark { remark: vec![1] }),
            RuntimeCall::EthBridge(user_call),
        ] {
            assert_ok!(EthBridge::send_transaction(bridge_multisig::Call::<
                Runtime,
            >::as_multi_threshold_1 {
                id: id.clone(),
                timepoint: Default::default(),
                call: Box::new(call),
            }));
        }
        assert_ok!(EthBridge::send_transaction(bridge_multisig::Call::<
            Runtime,
        >::cancel_as_multi {
            id: id.clone(),
            timepoint: Default::default(),
            call_hash: [9; 32],
        }));
        let approve = |call_hash| bridge_multisig::Call::<Runtime>::approve_as_multi {
            id: id.clone(),
            maybe_timepoint: None,
            call_hash,
            max_weight: Weight::zero(),
        };
        // An absent or undecodable stored call keeps generic signing behavior.
        assert_ok!(EthBridge::send_transaction(approve([10; 32])));
        bridge_multisig::Calls::<Runtime>::insert([11; 32], (vec![255u8], alice(), 0u128));
        assert_ok!(EthBridge::send_transaction(approve([11; 32])));
        let protocol: RuntimeCall = Call::<Runtime>::import_incoming_request {
            load_incoming_request: LoadIncomingRequest::Transaction(
                LoadIncomingTransactionRequest::new(
                    alice(),
                    H256::repeat_byte(13),
                    Default::default(),
                    IncomingTransactionRequestKind::Transfer,
                    0,
                ),
            ),
            incoming_request_result: Err(Error::EthTransactionIsFailed.into()),
        }
        .into();
        let encoded = protocol.encode();
        let hash = blake2_256(&encoded);
        bridge_multisig::Calls::<Runtime>::insert(hash, (encoded, alice(), 0u128));
        assert_ok!(EthBridge::send_transaction(approve(hash)));
        let before = state.pool_state.read().transactions.len();
        assert_eq!(
            EthBridge::send_transaction(bridge_multisig::Call::<Runtime>::as_multi_threshold_1 {
                id: alice(),
                timepoint: Default::default(),
                call: Box::new(protocol.clone()),
            }),
            Err(Error::UnknownNetwork)
        );
        bridge_multisig::Accounts::<Runtime>::remove(&id);
        assert_eq!(
            EthBridge::send_transaction(approve(hash)),
            Err(Error::UnknownNetwork)
        );
        assert_eq!(state.pool_state.read().transactions.len(), before);
    });
}

#[test]
fn pending_multisig_selection_ignores_thischain_and_unconfirmed_entries() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let id = EthBridge::bridge_account(0).unwrap();
        for (hash, height) in [
            ([1; 32], MultiChainHeight::Thischain(1)),
            ([2; 32], MultiChainHeight::Sidechain(999)),
        ] {
            bridge_multisig::Multisigs::<Runtime>::insert(
                &id,
                hash,
                bridge_multisig::Multisig {
                    when: bridge_multisig::BridgeTimepoint { height, index: 0 },
                    deposit: 0,
                    depositor: id.clone(),
                    approvals: vec![id.clone()],
                },
            );
        }
        assert!(!EthBridge::handle_pending_multisig_calls(0, 1000));
        assert!(state.http_requests().is_empty());
        assert_eq!(
            bridge_multisig::Multisigs::<Runtime>::iter_prefix(id).count(),
            2
        );
    });
}

#[test]
fn xor_compatibility_receipts_accept_only_xor_deposits() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        crate::LegacyEthereumXorDecommissioned::<Runtime>::put(false);
        let hash = H256::repeat_byte(20);
        let load = LoadIncomingTransactionRequest::new(
            alice(),
            hash,
            Default::default(),
            IncomingTransactionRequestKind::TransferXOR,
            0,
        );
        let log = deposit_log(hash);
        let receipt = TransactionReceipt {
            transaction_hash: types::H256(hash.0),
            to: Some(types::H160(EthBridge::bridge_contract_address(0).0)),
            status: Some(1.into()),
            block_number: Some(5.into()),
            logs: vec![log.clone()],
            ..Default::default()
        };
        push_global_json_rpc_response(receipt.clone());
        assert_ok!(EthBridge::handle_offchain_request(
            OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(load.clone()))
        ));
        assert_eq!(state.pool_state.read().transactions.len(), 1);
        let mut other = log;
        other.data = ethabi::encode(&[
            Token::FixedBytes(alice().encode()),
            Token::Uint(10.into()),
            Token::Address(types::H160(
                crate::RegisteredSidechainToken::<Runtime>::get(0, common::VAL)
                    .unwrap()
                    .0,
            )),
            Token::FixedBytes(vec![0; 32]),
        ])
        .into();
        push_global_json_rpc_response(TransactionReceipt {
            logs: vec![other],
            ..receipt
        });
        assert_eq!(
            EthBridge::handle_offchain_request(OffchainRequest::LoadIncoming(
                LoadIncomingRequest::Transaction(load)
            )),
            Err(Error::ExpectedXORTransfer)
        );
        assert_eq!(state.pool_state.read().transactions.len(), 1);
    });
}

#[test]
fn legacy_peer_parser_checks_method_abi_and_receipt_compatibility_path() {
    use crate::requests::OutgoingAddPeerCompat;
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let load = LoadIncomingTransactionRequest::new(
            alice(),
            H256::repeat_byte(30),
            Default::default(),
            IncomingTransactionRequestKind::AddPeerCompat,
            0,
        );
        assert_eq!(
            EthBridge::parse_old_incoming_request_method_call(load.clone(), Transaction::default()),
            Err(Error::UnknownMethodId)
        );
        let selector = *crate::contract::ADD_PEER_BY_PEER_ID
            .get_or_init(crate::contract::init_add_peer_by_peer_fn);
        assert_eq!(
            EthBridge::parse_old_incoming_request_method_call(
                load.clone(),
                Transaction {
                    input: selector.to_vec().into(),
                    ..Default::default()
                }
            ),
            Err(Error::EthAbiDecodingError)
        );
        let request =
            OffchainRequest::outgoing(OutgoingRequest::AddPeerCompat(OutgoingAddPeerCompat::<
                Runtime,
            > {
                author: alice(),
                peer_address: H160::repeat_byte(2),
                peer_account_id: common::mock::bob(),
                nonce: 1,
                network_id: 0,
                timepoint: Default::default(),
            }));
        let hash = request.hash();
        crate::Requests::<Runtime>::insert(0, hash, request);
        let function = crate::contract::ADD_PEER_BY_PEER_FN.get().unwrap();
        let input = function
            .encode_input(&[
                Token::Address(types::H160::repeat_byte(2)),
                Token::FixedBytes(hash.0.to_vec()),
                Token::Array(vec![]),
                Token::Array(vec![]),
                Token::Array(vec![]),
            ])
            .unwrap();
        push_global_json_rpc_response(Transaction {
            to: Some(types::H160(EthBridge::val_master_contract_address().0)),
            input: input.into(),
            block_number: Some(15.into()),
            ..Default::default()
        });
        let parsed = EthBridge::parse_incoming_request(
            TransactionReceipt {
                transaction_hash: types::H256(load.hash.0),
                status: Some(1.into()),
                ..Default::default()
            },
            load,
        )
        .unwrap();
        let IncomingRequest::ChangePeersCompat(parsed) = parsed else {
            panic!("compatibility peer request expected")
        };
        assert_eq!(parsed.at_height, 15);
        assert_eq!(parsed.peer_account_id, common::mock::bob());
        assert!(parsed.added);
        assert_eq!(state.http_requests().len(), 1);
    });
}
