use super::coverage_paths::deposit_log;
use super::*;
use crate::requests::{AssetKind, IncomingTransfer};
use crate::tests::{mock::*, Error};
use crate::types::{self, TransactionReceipt};
use common::mock::{alice, bob};
use frame_support::assert_ok;
use sp_core::H160;

fn receipt(hash: H256) -> TransactionReceipt {
    let mut log = deposit_log(hash);
    let block_hash = types::H256::repeat_byte(7);
    log.block_hash = Some(block_hash);
    log.log_index = Some(0.into());
    TransactionReceipt {
        transaction_hash: types::H256(hash.0),
        transaction_index: 0.into(),
        block_hash: Some(block_hash),
        block_number: Some(5.into()),
        from: types::H160(common::recovery::ATTACKER_EVM),
        to: Some(types::H160(EthBridge::bridge_contract_address(0).0)),
        logs: vec![log],
        status: Some(1.into()),
        ..Default::default()
    }
}

fn load(hash: H256) -> LoadIncomingTransactionRequest<Runtime> {
    LoadIncomingTransactionRequest::new(
        alice(),
        hash,
        bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(5, 0),
        IncomingTransactionRequestKind::Transfer,
        0,
    )
}

fn enable_policy() {
    sp_io::storage::set(common::recovery::POLICY_STORAGE_KEY, &[1]);
}

fn transfer(request: IncomingRequest<Runtime>) -> IncomingTransfer<Runtime> {
    let IncomingRequest::Transfer(transfer) = request else {
        panic!("deposit must parse as transfer")
    };
    transfer
}

#[test]
fn recovery_requested_deposit_binds_sender_without_changing_historical_parser() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let hash = H256::repeat_byte(9);
        let receipt = receipt(hash);
        let historical = EthBridge::parse_incoming_request(receipt.clone(), load(hash)).unwrap();
        assert_eq!(transfer(historical.clone()).from, H160::zero());
        enable_policy();
        let attributed = EthBridge::parse_incoming_request(receipt, load(hash)).unwrap();
        assert_eq!(
            transfer(attributed.clone()).from,
            H160(common::recovery::ATTACKER_EVM)
        );
        let mut only_sender = transfer(historical);
        only_sender.from = H160(common::recovery::ATTACKER_EVM);
        assert_eq!(
            attributed.encode(),
            IncomingRequest::Transfer(only_sender).encode()
        );
    });
}

#[test]
fn recovery_receipt_attribution_rejects_inconsistent_rpc_metadata() {
    for scenario in 0..11 {
        let (mut ext, _) = ExtBuilder::default().build();
        ext.execute_with(|| {
            enable_policy();
            let hash = H256::repeat_byte(9);
            let mut receipt = receipt(hash);
            match scenario {
                0 => receipt.transaction_hash = types::H256::repeat_byte(10),
                1 => receipt.from = types::H160::zero(),
                2 => receipt.block_number = None,
                3 => receipt.block_hash = None,
                4 => receipt.status = Some(0.into()),
                5 => receipt.logs[0].transaction_hash = Some(types::H256::repeat_byte(10)),
                6 => receipt.logs[0].block_number = Some(6.into()),
                7 => receipt.logs[0].block_hash = Some(types::H256::repeat_byte(10)),
                8 => receipt.logs[0].transaction_index = Some(1.into()),
                9 => receipt.logs[0].removed = Some(true),
                10 => receipt.logs[0].address = types::H160::repeat_byte(92),
                _ => unreachable!(),
            }
            assert_eq!(
                EthBridge::parse_attributed_transfer(receipt, load(hash), None),
                Err(Error::JsonDeserializationError),
                "scenario {scenario}"
            );
        });
    }
}

#[test]
fn recovery_pending_deposit_requires_exact_canonical_economic_fields_and_sender() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        enable_policy();
        let hash = H256::repeat_byte(9);
        let receipt = receipt(hash);
        let original = EthBridge::parse_incoming_request(receipt.clone(), load(hash)).unwrap();
        push_global_json_rpc_response(receipt.clone());
        assert_eq!(original.check_existence(), Ok(true));
        for scenario in 0..8 {
            let mut forged = transfer(original.clone());
            match scenario {
                0 => forged.from = H160::zero(),
                1 => forged.from = H160::repeat_byte(11),
                2 => forged.to = bob(),
                3 => forged.amount += 1,
                4 => forged.asset_id = common::DAI,
                5 => forged.asset_kind = AssetKind::Sidechain,
                6 => forged.at_height += 1,
                7 => forged.should_take_fee = true,
                _ => unreachable!(),
            }
            let forged = IncomingRequest::Transfer(forged);
            let original_bytes = forged.encode();
            push_global_json_rpc_response(receipt.clone());
            assert_eq!(
                forged.check_existence(),
                Err(Error::JsonDeserializationError),
                "scenario {scenario}"
            );
            assert_eq!(
                forged.encode(),
                original_bytes,
                "stored request is immutable"
            );
        }
        assert!(state.pool_state.read().transactions.is_empty());
    });
}

#[test]
fn recovery_invalid_public_hints_stop_retrying_but_pending_deposits_stay_held() {
    for scenario in 0..3 {
        let (mut ext, state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            enable_policy();
            let hash = H256::repeat_byte(9);
            let valid_receipt = receipt(hash);
            let pending =
                EthBridge::parse_incoming_request(valid_receipt.clone(), load(hash)).unwrap();
            let mut invalid = valid_receipt.clone();
            let expected = match scenario {
                0 => {
                    invalid.status = Some(0.into());
                    invalid.logs.clear();
                    Error::EthTransactionIsFailed
                }
                1 => {
                    invalid.logs.clear();
                    Error::UnknownEvent
                }
                2 => {
                    invalid.logs[0].address = types::H160::repeat_byte(92);
                    Error::UnknownEvent
                }
                _ => unreachable!(),
            };
            crate::tests::request_incoming(
                alice(),
                hash,
                IncomingTransactionRequestKind::Transfer.into(),
                0,
            )
            .unwrap();
            let hint = crate::Requests::<Runtime>::get(0, hash).unwrap();
            push_global_json_rpc_response(invalid.clone());
            let error = EthBridge::handle_offchain_request(hint).unwrap_err();
            assert_eq!(error, expected);
            assert!(!error.should_retry());

            // The same RPC response cannot authorize abandoning an already
            // parsed monetary request, or skipping an independently seen log.
            push_global_json_rpc_response(invalid.clone());
            assert_eq!(
                pending.check_existence(),
                Err(Error::JsonDeserializationError)
            );
            assert_eq!(
                EthBridge::parse_attributed_transfer(
                    invalid.clone(),
                    load(hash),
                    Some(&valid_receipt.logs[0]),
                ),
                Err(Error::JsonDeserializationError)
            );
            // An incomplete receipt still provides no deterministic rejection.
            invalid.block_hash = None;
            assert_eq!(
                EthBridge::parse_incoming_request(invalid, load(hash)),
                Err(Error::JsonDeserializationError)
            );
            assert!(state.pending_txs().is_empty());
            assert!(state.pool_state.read().transactions.is_empty());
        });
    }
}

#[test]
fn recovery_scanner_import_contains_receipt_sender() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        enable_policy();
        let hash = H256::repeat_byte(9);
        let receipt = receipt(hash);
        push_global_json_rpc_response(receipt.logs.clone());
        push_global_json_rpc_response(receipt);
        let mut cursor = 5;
        assert_ok!(EthBridge::handle_logs(5, 7, &mut cursor, 0));
        assert_eq!(cursor, 8);
        assert_eq!(state.pending_txs().len(), 1);
        let tx = state.pending_txs().into_values().next().unwrap();
        let RuntimeCall::Multisig(bridge_multisig::Call::as_multi { call, .. }) = tx.call else {
            panic!("expected multisig import")
        };
        let RuntimeCall::EthBridge(Call::import_incoming_request {
            incoming_request_result,
            ..
        }) = RuntimeCall::decode(&mut &call[..]).unwrap()
        else {
            panic!("expected incoming import")
        };
        let transfer = transfer(incoming_request_result.unwrap());
        assert_eq!(transfer.from, H160(common::recovery::ATTACKER_EVM));
        assert_eq!(transfer.tx_hash, hash);
        assert_eq!(transfer.to, alice());
        // The mock denomination factor is ten; attribution preserves conversion.
        assert_eq!(transfer.amount, 1);
    });
}

#[test]
fn recovery_scanner_receipt_failure_never_submits_failure_or_advances_cursor() {
    for scenario in 0..5 {
        let (mut ext, state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            enable_policy();
            let hash = H256::repeat_byte(9);
            let mut receipt = receipt(hash);
            let mut observed = receipt.logs[0].clone();
            if scenario == 2 {
                observed.block_hash = Some(types::H256::repeat_byte(10));
            }
            if scenario == 3 {
                // A well-formed but different deposit log is not canonical evidence.
                observed.data = ethabi::encode(&[
                    ethabi::Token::FixedBytes(bob().encode()),
                    ethabi::Token::Uint(10.into()),
                    ethabi::Token::Address(types::H160(
                        crate::RegisteredSidechainToken::<Runtime>::get(0, common::XOR)
                            .unwrap()
                            .0,
                    )),
                    ethabi::Token::FixedBytes(vec![0; 32]),
                ])
                .into();
            }
            push_global_json_rpc_response(vec![observed]);
            if scenario == 0 {
                push_global_http_failure();
            } else {
                if scenario == 1 {
                    receipt.transaction_hash = types::H256::repeat_byte(10);
                }
                if scenario == 4 {
                    receipt.from = types::H160::zero();
                }
                push_global_json_rpc_response(receipt);
            }
            let mut cursor = 5;
            let error = EthBridge::handle_logs(5, 7, &mut cursor, 0).unwrap_err();
            assert!(error.should_retry(), "scenario {scenario}: {error:?}");
            assert_eq!(cursor, 5, "scenario {scenario}");
            assert!(state.pending_txs().is_empty());
            assert!(state.pool_state.read().transactions.is_empty());
            assert!(!crate::RequestStatuses::<Runtime>::contains_key(0, hash));
        });
    }
}

#[test]
fn recovery_indirect_requested_deposit_and_pending_confirmation_use_known_emitter() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        enable_policy();
        let hash = H256::repeat_byte(9);
        let mut receipt = receipt(hash);
        receipt.to = Some(types::H160::repeat_byte(90));
        crate::tests::request_incoming(
            alice(),
            hash,
            IncomingTransactionRequestKind::Transfer.into(),
            0,
        )
        .unwrap();
        let request = crate::Requests::<Runtime>::get(0, hash).unwrap();
        push_global_json_rpc_response(receipt.clone());
        assert_ok!(EthBridge::handle_offchain_request(request));
        let tx = state.pending_txs().into_values().next().unwrap();
        let RuntimeCall::Multisig(bridge_multisig::Call::as_multi { call, .. }) = tx.call else {
            panic!("expected multisig registration")
        };
        let RuntimeCall::EthBridge(Call::register_incoming_request { incoming_request }) =
            RuntimeCall::decode(&mut &call[..]).unwrap()
        else {
            panic!("expected incoming registration")
        };
        assert_eq!(
            transfer(incoming_request.clone()).from,
            H160(common::recovery::ATTACKER_EVM)
        );
        push_global_json_rpc_response(receipt.clone());
        assert_eq!(incoming_request.check_existence(), Ok(true));
        // Other protocol paths retain the old known-outer-target restriction.
        push_global_json_rpc_response(receipt);
        assert_eq!(
            EthBridge::load_tx_receipt(hash, 0),
            Err(Error::UnknownContractAddress)
        );
    });
}

#[test]
fn recovery_scanner_indirect_and_duplicate_logs_do_not_block_later_deposits() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        enable_policy();
        let first_hash = H256::repeat_byte(9);
        let second_hash = H256::repeat_byte(10);
        let mut first = receipt(first_hash);
        first.to = Some(types::H160::repeat_byte(90));
        let mut another_deposit = first.logs[0].clone();
        another_deposit.log_index = Some(1.into());
        first.logs.push(another_deposit);
        let mut second = receipt(second_hash);
        second.transaction_index = 1.into();
        second.logs[0].transaction_index = Some(1.into());
        second.logs[0].log_index = Some(2.into());
        let mut logs = first.logs.clone();
        logs.extend(second.logs.clone());
        push_global_json_rpc_response(logs);
        push_global_json_rpc_response(first);
        push_global_json_rpc_response(second);
        let mut cursor = 5;
        assert_ok!(EthBridge::handle_logs(5, 7, &mut cursor, 0));
        assert_eq!(cursor, 8);
        let pending = state.pending_txs();
        assert_eq!(pending.len(), 2);
        let mut hashes = BTreeSet::new();
        for tx in pending.into_values() {
            let RuntimeCall::Multisig(bridge_multisig::Call::as_multi { call, .. }) = tx.call
            else {
                panic!("expected multisig import")
            };
            let RuntimeCall::EthBridge(Call::import_incoming_request {
                incoming_request_result,
                ..
            }) = RuntimeCall::decode(&mut &call[..]).unwrap()
            else {
                panic!("expected incoming import")
            };
            let incoming = transfer(incoming_request_result.unwrap());
            assert_eq!(incoming.from, H160(common::recovery::ATTACKER_EVM));
            hashes.insert(incoming.tx_hash);
        }
        assert_eq!(hashes, BTreeSet::from([first_hash, second_hash]));
    });
}

#[test]
fn recovery_scanner_canonical_unsupported_asset_does_not_pin_later_deposits() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        enable_policy();
        let invalid_hash = H256::repeat_byte(9);
        let valid_hash = H256::repeat_byte(10);
        let mut invalid = receipt(invalid_hash);
        invalid.logs[0].data = ethabi::encode(&[
            ethabi::Token::FixedBytes(alice().encode()),
            ethabi::Token::Uint(10.into()),
            ethabi::Token::Address(types::H160::repeat_byte(91)),
            ethabi::Token::FixedBytes(vec![0; 32]),
        ])
        .into();
        let mut valid = receipt(valid_hash);
        valid.transaction_index = 1.into();
        valid.logs[0].transaction_index = Some(1.into());
        valid.logs[0].log_index = Some(1.into());
        let mut logs = invalid.logs.clone();
        logs.extend(valid.logs.clone());
        push_global_json_rpc_response(logs);
        push_global_json_rpc_response(invalid);
        push_global_json_rpc_response(valid);
        let mut cursor = 5;
        assert_ok!(EthBridge::handle_logs(5, 7, &mut cursor, 0));
        assert_eq!(cursor, 8);
        assert_eq!(state.pending_txs().len(), 2);
        let mut hashes = BTreeSet::new();
        for tx in state.pending_txs().into_values() {
            let RuntimeCall::Multisig(bridge_multisig::Call::as_multi { call, .. }) = tx.call
            else {
                panic!("expected multisig import")
            };
            let RuntimeCall::EthBridge(Call::import_incoming_request {
                load_incoming_request,
                incoming_request_result,
            }) = RuntimeCall::decode(&mut &call[..]).unwrap()
            else {
                panic!("expected incoming import")
            };
            let hash = load_incoming_request.hash();
            hashes.insert(hash);
            if hash == invalid_hash {
                assert_eq!(
                    incoming_request_result,
                    Err(Error::UnsupportedAssetId.into())
                );
            } else {
                assert_eq!(
                    transfer(incoming_request_result.unwrap()).tx_hash,
                    valid_hash
                );
            }
        }
        assert_eq!(hashes, BTreeSet::from([invalid_hash, valid_hash]));
    });
}
