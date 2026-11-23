// Public deterministic transaction bytes. This tests cryptographic source-actor
// binding and native peer admission, not external event/finality evidence.
fn source_fixture_raw() -> Vec<u8> {
    let text = b"f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";
    text.as_chunks::<2>().0.iter()
        .map(|pair| {
            let nibble = |x| match x {
                b'0'..=b'9' => x - b'0',
                b'a'..=b'f' => x - b'a' + 10,
                _ => unreachable!(),
            };
            (nibble(pair[0]) << 4) | nibble(pair[1])
        })
        .collect()
}

fn hashi_source_fixture() -> (RecoveryPolicy, IncomingOperation, Vec<sr25519::Pair>) {
    hashi_source_fixture_with_raw(source_fixture_raw())
}

fn hashi_source_fixture_with_raw(
    raw_transaction: Vec<u8>,
) -> (RecoveryPolicy, IncomingOperation, Vec<sr25519::Pair>) {
    let (mut policy, operation, keys) = hashi_fixture_with_author(AccountId::new([7; 32]));
    let IncomingChannel::Hashi {
        network,
        request,
        mut approvals,
    } = operation.channel
    else {
        unreachable!()
    };
    let mut transfer = hashi_transfer(&request).unwrap();
    transfer.from = bridge_types::H160::zero();
    transfer.tx_hash = H256(sp_io::hashing::keccak_256(&raw_transaction));
    let original = eth_bridge::requests::IncomingRequest::Transfer(transfer.clone());
    let inner = RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
        load_incoming_request: eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                transfer.author.clone(),
                transfer.tx_hash,
                transfer.timepoint,
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                network,
            ),
        ),
        incoming_request_result: Ok(original.clone()),
    });
    for (approval, key) in approvals.iter_mut().zip(&keys) {
        let mut call = decode_runtime_call(&approval.call).unwrap();
        let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
            call: opaque,
            max_weight,
            ..
        }) = &mut call
        else {
            unreachable!()
        };
        *opaque = inner.encode();
        *max_weight = inner.get_dispatch_info().total_weight();
        approval.call = call.encode();
        sign_hashi(approval, key);
    }
    let operation = IncomingOperation {
        channel: IncomingChannel::HashiAuthenticatedSource {
            network,
            request: original.encode(),
            raw_transaction,
            expected_chain_id: Some(U256::one()),
            approvals,
        },
        ..operation
    };
    policy.chunk_hashes = vec![operation.commitment()];
    (policy, operation, keys)
}

fn source_pending_load(
    operation: &IncomingOperation,
) -> eth_bridge::requests::OffchainRequest<Runtime> {
    let transfer = native_hashi_transfer(&operation.channel).unwrap();
    let load = eth_bridge::requests::OffchainRequest::LoadIncoming(
        eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                transfer.author,
                transfer.tx_hash,
                transfer.timepoint,
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                transfer.network_id,
            ),
        ),
    );
    eth_bridge::Requests::<Runtime>::insert(transfer.network_id, load.hash(), &load);
    eth_bridge::RequestStatuses::<Runtime>::insert(
        transfer.network_id,
        load.hash(),
        eth_bridge::requests::RequestStatus::Pending,
    );
    eth_bridge::RequestSubmissionHeight::<Runtime>::insert(
        transfer.network_id,
        load.hash(),
        recovery::CHECKPOINT_NUMBER,
    );
    eth_bridge::RequestsQueue::<Runtime>::insert(transfer.network_id, vec![load.hash()]);
    // The typed migration already moved this query entry; original load stays exact.
    eth_bridge::AccountRequests::<Runtime>::insert(
        AccountId::new([8; 32]),
        vec![(transfer.network_id, load.hash())],
    );
    load
}

#[test]
fn recovery_hashi_authenticated_source_preserves_zero_sender_and_maps_native_credit_once() {
    use common::AssetInfoProvider;
    for pending in [false, true] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, keys) = hashi_source_fixture();
            let transfer = native_hashi_transfer(&operation.channel).unwrap();
            let original = eth_bridge::requests::OffchainRequest::incoming(
                eth_bridge::requests::IncomingRequest::Transfer(transfer.clone()),
            );
            let retained = pending.then(|| source_pending_load(&operation));
            let actor = prepare(&operation, &policy).unwrap().transfers[0]
                .sender
                .clone();
            assert_ne!(actor, GenericAccount::EVM(transfer.from));
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &AccountId::new([8; 32])).unwrap(),
                    1_000_000_000_000_000_000
                );
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &AccountId::new([7; 32])).unwrap(),
                    0
                );
                assert_eq!(
                    eth_bridge::Requests::<Runtime>::get(0, original.hash()),
                    Some(original.clone())
                );
                assert_eq!(transfer.from, bridge_types::H160::zero());
                assert_eq!(
                    crate::BridgeProxy::sender(operation.network(), transfer.tx_hash),
                    Some(AccountId::new([8; 32]))
                );
                assert!(crate::BridgeProxy::transactions(
                    (operation.network(), AccountId::new([7; 32])),
                    transfer.tx_hash
                )
                .is_none());
                let query = crate::BridgeProxy::transactions(
                    (operation.network(), AccountId::new([8; 32])),
                    transfer.tx_hash,
                )
                .unwrap();
                assert_eq!(
                    query.source(),
                    &GenericAccount::EVM(bridge_types::H160::zero())
                );
                assert_eq!(query.amount(), transfer.amount);

                if let Some(load) = retained {
                    assert_eq!(
                        eth_bridge::Requests::<Runtime>::get(0, load.hash()),
                        Some(load)
                    );
                }
                assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(
                    AccountId::new([7; 32])
                ));
                assert_eq!(
                    eth_bridge::AccountRequests::<Runtime>::get(AccountId::new([8; 32])),
                    vec![(0, transfer.tx_hash), (0, original.hash())]
                );
                assert!(sp_io::storage::exists(&receipt_key(
                    operation.network(),
                    transfer.tx_hash
                )));
                assert_eq!(System::account_nonce(AccountId::from(keys[0].public())), 1);
                let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
                assert!(apply_operation(&policy, &operation).is_err());
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
            });
        });
    }
}

#[test]
fn recovery_hashi_authenticated_source_excludes_actor_without_rewriting_native_sender() {
    use common::AssetInfoProvider;
    sp_io::TestExternalities::default().execute_with(|| {
        let (mut policy, mut operation, _) = hashi_source_fixture();
        let actor = prepare(&operation, &policy).unwrap().transfers[0]
            .sender
            .clone();
        operation.registries[0].accounts.push(actor);
        policy.chunk_hashes = vec![operation.commitment()];
        let transfer = native_hashi_transfer(&operation.channel).unwrap();
        let original = eth_bridge::requests::OffchainRequest::incoming(
            eth_bridge::requests::IncomingRequest::Transfer(transfer.clone()),
        );
        crate::incident_recovery::with_test_policy(&policy, || {
            recovery::storage::initialize(&policy).unwrap();
            assert_ok!(apply_operation(&policy, &operation));
            assert_eq!(
                crate::Assets::free_balance(&common::DAI, &AccountId::new([8; 32])).unwrap(),
                0
            );
            assert_eq!(
                eth_bridge::Requests::<Runtime>::get(0, original.hash()),
                Some(original)
            );
            assert!(sp_io::storage::exists(&receipt_key(
                operation.network(),
                transfer.tx_hash
            )));
            assert_eq!(transfer.from, bridge_types::H160::zero());
        });
    });
}

#[test]
fn recovery_hashi_authenticated_source_rejects_wrong_hash_domain_native_sender_and_peer_signature_before_writes(
) {
    for variant in 0..4 {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation, _) = hashi_source_fixture();
            let IncomingChannel::HashiAuthenticatedSource {
                request,
                raw_transaction,
                expected_chain_id,
                approvals,
                ..
            } = &mut operation.channel
            else {
                unreachable!()
            };
            match variant {
                0 => raw_transaction[10] ^= 1,
                1 => *expected_chain_id = Some(U256::from(2u8)),
                2 => {
                    let mut transfer = hashi_zero_sender_transfer(request).unwrap();
                    transfer.from = bridge_types::H160::repeat_byte(2);
                    *request = eth_bridge::requests::IncomingRequest::Transfer(transfer).encode();
                }
                3 => approvals[0].signature = sr25519::Signature::from_raw([0; 64]).into(),
                _ => unreachable!(),
            }
            policy.chunk_hashes = vec![operation.commitment()];
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
                assert!(apply_operation(&policy, &operation).is_err());
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
            });
        });
    }
}

#[test]
fn recovery_hashi_authenticated_source_encoding_and_commitment_preserve_existing_variant() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation, _) = hashi_source_fixture();
        assert_eq!(operation.channel.encode()[0], 3);
        let IncomingChannel::HashiAuthenticatedSource {
            network,
            request,
            approvals,
            ..
        } = operation.channel.clone()
        else {
            unreachable!()
        };
        let mut legacy = operation.clone();
        legacy.channel = IncomingChannel::Hashi {
            network,
            request,
            approvals,
        };
        assert_eq!(legacy.channel.encode()[0], 2);
        assert!(matches!(
            prepare(&legacy, &policy),
            Err(RecoveryError::WrongPreimage)
        ));
        let mut altered = operation.clone();
        let IncomingChannel::HashiAuthenticatedSource { approvals, .. } = &mut altered.channel
        else {
            unreachable!()
        };
        approvals[0].signature = sr25519::Signature::from_raw([0; 64]).into();
        assert_eq!(altered.commitment(), operation.commitment());
        let IncomingChannel::HashiAuthenticatedSource {
            raw_transaction, ..
        } = &mut altered.channel
        else {
            unreachable!()
        };
        raw_transaction.push(0);
        assert_ne!(altered.commitment(), operation.commitment());
    });
}

#[test]
fn recovery_hashi_authenticated_source_invalid_evm_signature_fails_despite_native_hash_binding() {
    sp_io::TestExternalities::default().execute_with(|| {
        let valid = source_fixture_raw();
        let decoded = rlp::Rlp::new(&valid);
        let mut raw = rlp::RlpStream::new_list(9);
        for index in 0..9 {
            if index == 7 {
                raw.append_empty_data();
            } else {
                raw.append_raw(decoded.at(index).unwrap().as_raw(), 1);
            }
        }
        // Native approval bytes and tx_hash bind this exact invalid r=0 envelope.
        // Their signatures alone must not manufacture a valid external source actor.
        let (policy, operation, _) = hashi_source_fixture_with_raw(raw.out().to_vec());
        crate::incident_recovery::with_test_policy(&policy, || {
            recovery::storage::initialize(&policy).unwrap();
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert_eq!(
                apply_operation(&policy, &operation).unwrap_err(),
                RecoveryError::WrongPreimage
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        });
    });
}

#[test]
fn recovery_hashi_authenticated_source_offline_codec_golden() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (_, operation, _) = hashi_source_fixture();
        let IncomingChannel::HashiAuthenticatedSource { network, request, raw_transaction, expected_chain_id, approvals } = &operation.channel else { unreachable!() };
        let hex = |bytes: &[u8]| format!("0x{}", bytes.iter().map(|byte| format!("{byte:02x}")).collect::<alloc::string::String>());
        let mut chain = [0u8; 32];
        expected_chain_id.unwrap().write_as_big_endian(&mut chain);
        let template = serde_json::json!({
            "format":"sora-nexus-hashi-authenticated-source-template-v1", "index":operation.index,
            "network":network, "request":hex(request), "rawTransaction":hex(raw_transaction),
            "expectedChainId":hex(&chain),
            "approvals":approvals.iter().map(|approval| serde_json::json!({
                "signer":hex(approval.signer.as_ref()),"call":hex(&approval.call),"extra":hex(&approval.extra),
            })).collect::<Vec<_>>(),
            "registries":[
                {"network":{"evmLegacy":network},"accounts":[{"evm":hex(&recovery::ATTACKER_EVM)}],"noKnownAttackers":false,"attestation":hex(&[8u8;32])},
                {"network":{"sub":"Mainnet"},"accounts":[{"sora":hex(&recovery::ATTACKER_ACCOUNT)}],"noKnownAttackers":false,"attestation":hex(&[2u8;32])}
            ],
            "finalizedSourceEvidence":hex(&operation.finalized_source_evidence),
        });
        let evidence = serde_json::json!({
            "template":template, "operationScale":hex(&operation.encode()),
            "commitment":hex(&operation.commitment()),
            "signatures":approvals.iter().map(|approval| hex(&approval.signature.encode())).collect::<Vec<_>>(),
            "fixtureOnly":true,"productionReady":false,
            "sourceActorScope":"Exact outer transaction signer only; no external event, finality or beneficial-owner evidence."
        });
        if let Some(path) = std::env::var_os("SORA_RECOVERY_HASHI_SOURCE_GOLDEN_PATH") {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();
            file.write_all(serde_json::to_string_pretty(&evidence).unwrap().as_bytes()).unwrap();
        }
    });
}
