mod tests {
    use super::*;
    use crate::incident_recovery_incoming::tests::{fixture as channel_fixture, hashi_fixture};
    use common::AssetInfoProvider;

    fn apply_operation(
        policy: &RecoveryPolicy,
        operation: &IncomingOperation,
    ) -> Result<(RecoveryStatus, Weight), RecoveryError> {
        crate::incident_recovery::with_test_policy(policy, || {
            crate::incident_recovery_incoming::apply_operation(policy, operation)
        })
    }

    fn fixture(recipient: AccountId) -> (RecoveryPolicy, IncomingOperation, Vec<u8>) {
        let result = channel_fixture(recipient);
        let owner = AccountId::new([45; 32]);
        System::inc_providers(&owner);
        crate::Assets::register_asset_id(
            owner,
            common::XOR,
            common::AssetSymbol(b"XOR".to_vec()),
            common::AssetName(b"SORA Nexus".to_vec()),
            18,
            0,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        <crate::BridgeProxy as bridge_types::traits::BridgeAssetRegistry<
            AccountId,
            crate::AssetId,
        >>::manage_asset(SubNetworkId::Liberland.into(), common::XOR)
        .unwrap();
        result
    }

    fn make_audit(policy: &RecoveryPolicy, operation: &IncomingOperation) -> IncomingAudit {
        IncomingAudit {
            index: 0,
            operation_index: operation.index,
            operation_commitment: operation.commitment(),
            requests: expected_requests(operation, policy).unwrap(),
            prior_policy: None,
        }
    }
    fn setup(excluded: bool, delayed: u32) -> (RecoveryPolicy, IncomingOperation, IncomingAudit) {
        let recipient = if excluded {
            AccountId::new(recovery::ATTACKER_ACCOUNT)
        } else {
            AccountId::new([7; 32])
        };
        let (mut policy, operation, _) = fixture(recipient);
        let audit = make_audit(&policy, &operation);
        policy.audit_chunk_hashes = vec![audit.commitment()];
        recovery::storage::initialize(&policy).unwrap();
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 1 + delayed);
        apply_operation(&policy, &operation).unwrap();
        (policy, operation, audit)
    }
    fn run(
        policy: &RecoveryPolicy,
        audit: &IncomingAudit,
    ) -> Result<(RecoveryStatus, Weight), RecoveryError> {
        audit_encoded_with_policy(policy, &audit.encode())
    }
    fn companion(policy: &RecoveryPolicy, operation: &IncomingOperation) -> Consumption {
        let mut budget = MAX_CHUNK_BYTES;
        required(
            &companion_key(policy.commitment(), operation.commitment()),
            &mut budget,
        )
        .unwrap()
    }

    #[test]
    fn typed_audit_accepts_tick_and_phase_delays_and_preserves_v1_receipts() {
        for delay in [0, 1, 52, 500] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, operation, audit) = setup(false, delay);
                let value = companion(&policy, &operation);
                assert_eq!(
                    value.execution_block,
                    recovery::CHECKPOINT_NUMBER + 1 + delay
                );
                assert_eq!(pending().unwrap(), 1);
                let entry = &value.transfers[0];
                let bytes =
                    sp_io::storage::get(&receipt_key(entry.transfer.network, entry.transfer.id))
                        .unwrap();
                let decoded: ([u8; 32], [u8; 32], [u8; 32], ExpectedTransfer) =
                    decode_bounded(&bytes).unwrap();
                assert_eq!(decoded.0, policy.commitment());
                assert_eq!(
                    bytes.to_vec(),
                    (
                        value.lineage,
                        value.policy,
                        value.operation,
                        &entry.transfer
                    )
                        .encode()
                );
                assert_eq!(
                    crate::BridgeProxy::transactions(
                        (entry.transfer.network, entry.transfer.beneficiary.clone()),
                        entry.transfer.id
                    )
                    .unwrap()
                    .encode(),
                    request_bytes(entry.request.as_ref().unwrap(), value.execution_block)
                );
                System::set_block_number(value.execution_block + 17);
                run(&policy, &audit).unwrap();
                assert_eq!(pending().unwrap(), 0);
                assert_eq!(
                    sp_io::storage::get(&receipt_key(entry.transfer.network, entry.transfer.id))
                        .unwrap(),
                    bytes
                );
                assert!(sp_io::storage::exists(&audit_marker_key(
                    policy.commitment(),
                    operation.commitment()
                )));
            });
        }
    }

    #[test]
    fn typed_audit_checks_derived_business_fields_not_observed_request_adoption() {
        // Every negative starts from a valid, actually consumed native delivery.
        for field in 0..5 {
            sp_io::TestExternalities::default().execute_with(|| {
                let (mut policy, operation, _) = fixture(AccountId::new([7; 32]));
                let mut audit = make_audit(&policy, &operation);
                let request = audit.requests[0].as_mut().unwrap();
                match field {
                    0 => request.beneficiary = AccountId::new([99; 32]),
                    1 => request.amount += 1,
                    2 => request.source = GenericAccount::Liberland(AccountId::new([99; 32])),
                    3 => request.asset_id = common::DAI,
                    _ => request.start_timepoint = GenericTimepoint::Sora(999),
                }
                policy.audit_chunk_hashes = vec![audit.commitment()];
                recovery::storage::initialize(&policy).unwrap();
                apply_operation(&policy, &operation).unwrap();
                assert_eq!(pending().unwrap(), 1);
                assert_eq!(
                    run(&policy, &audit).unwrap_err(),
                    RecoveryError::WrongPreimage
                );
                assert_eq!(
                    recovery::storage::status().unwrap().unwrap().phase,
                    RecoveryPhase::Failed
                );
                assert_eq!(pending().unwrap(), 1);
            });
        }
    }

    #[test]
    fn typed_audit_rejects_changed_status_block_policy_receipt_and_native_nonce() {
        for field in 0..6 {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, operation, audit) = setup(false, 3);
                let mut value = companion(&policy, &operation);
                let t = &value.transfers[0].transfer;
                match field {
                    0 => {
                        // Preserve all business fields, corrupt only native status.
                        let request = value.transfers[0].request.as_ref().unwrap();
                        let changed = (
                            &request.source,
                            GenericAccount::Sora(request.beneficiary.clone()),
                            request.asset_id,
                            request.amount,
                            MessageStatus::Failed,
                            request.start_timepoint,
                            GenericTimepoint::Sora(value.execution_block),
                            MessageDirection::Inbound,
                        )
                            .encode();
                        sp_io::storage::set(
                            &audit_request_key(t.network, &t.beneficiary, t.id),
                            &changed,
                        );
                    }
                    1 => {
                        value.execution_block -= 1;
                        sp_io::storage::set(
                            &companion_key(policy.commitment(), operation.commitment()),
                            &value.encode(),
                        );
                    }
                    2 => {
                        value.policy = [77; 32];
                        sp_io::storage::set(
                            &companion_key(policy.commitment(), operation.commitment()),
                            &value.encode(),
                        );
                    }
                    3 => {
                        value.operation_counter += 1;
                        sp_io::storage::set(
                            &companion_key(policy.commitment(), operation.commitment()),
                            &value.encode(),
                        );
                    }
                    4 => sp_io::storage::set(&receipt_key(t.network, t.id), b"changed-v1-receipt"),
                    _ => substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::insert(
                        SubNetworkId::Liberland,
                        0,
                    ),
                }
                assert_eq!(
                    run(&policy, &audit).unwrap_err(),
                    RecoveryError::WrongPreimage
                );
                assert_eq!(pending().unwrap(), 1);
            });
        }
    }

    #[test]
    fn excluded_audit_requires_absence_and_done_ack_and_legacy_missing_companion_fails() {
        for mutation in 0..4 {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, operation, audit) = setup(true, 7);
                assert_eq!(audit.requests, vec![None]);
                let value = companion(&policy, &operation);
                let entry = &value.transfers[0].transfer;
                match mutation {
                    0 => {
                        run(&policy, &audit).unwrap();
                        assert_eq!(pending().unwrap(), 0);
                        return;
                    }
                    1 => sp_io::storage::set(
                        &sender_key(entry.network, entry.id),
                        &entry.beneficiary.encode(),
                    ),
                    2 => substrate_bridge_app::DeferredTransferResults::<Runtime>::remove(
                        SubNetworkId::Liberland,
                        entry.id,
                    ),
                    _ => sp_io::storage::clear(&companion_key(
                        policy.commitment(),
                        operation.commitment(),
                    )),
                }
                let error = run(&policy, &audit).unwrap_err();
                assert_eq!(
                    error,
                    if mutation == 3 {
                        RecoveryError::IncompleteAudit
                    } else {
                        RecoveryError::WrongPreimage
                    }
                );
                assert_eq!(pending().unwrap(), 1);
            });
        }
    }

    #[test]
    fn typed_audit_replay_and_uncommitted_changes_cannot_decrement_counter() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, operation, mut audit) = setup(false, 0);
            let mut changed = audit.clone();
            changed.operation_commitment = [88; 32];
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert_eq!(
                run(&policy, &changed).unwrap_err(),
                RecoveryError::WrongChunk
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            run(&policy, &audit).unwrap();
            // A forward policy that explicitly re-audits consumed work still cannot replay its marker.
            let old = policy.commitment();
            audit.index = 0;
            audit.prior_policy = Some(old);
            policy.audit_chunk_hashes = vec![audit.commitment()];
            sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &policy.commitment());
            sp_io::storage::set(recovery::AUDIT_CURSOR_STORAGE_KEY, &0u32.encode());
            assert!(sp_io::storage::exists(&audit_marker_key(
                old,
                operation.commitment()
            )));
            assert_eq!(run(&policy, &audit).unwrap_err(), RecoveryError::Replay);
            assert_eq!(pending().unwrap(), 0);
        });
    }

    #[test]
    fn prior_policy_companion_is_retained_across_repair_and_counter_is_not_reset() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, _, mut audit) = setup(false, 5);
            let old = policy.commitment();
            audit.prior_policy = Some(old);
            policy.audit_chunk_hashes = vec![audit.commitment()];
            policy.evidence_commitment = [93; 32];
            sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &policy.commitment());
            let mut state = recovery::storage::status().unwrap().unwrap();
            state.operation_counter = 2;
            sp_io::storage::set(STATUS_STORAGE_KEY, &state.encode());
            System::set_block_number(System::block_number() + 100);
            run(&policy, &audit).unwrap();
            assert_eq!(pending().unwrap(), 0);
            assert_eq!(recovery::storage::status().unwrap().unwrap().policy, old);
        });
    }

    #[test]
    fn terminal_refuses_missing_or_unfinished_incoming_audit_even_with_all_static_audits_done() {
        use recovery::invariants::{IssuanceInvariant, StoredAmount, TerminalAudit};
        for missing in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (mut policy, operation, _) = fixture(AccountId::new([7; 32]));
                let amount = 10u128.encode();
                sp_io::storage::set(b"audit-test-issuance", &amount);
                let terminal = TerminalAudit {
                    issuance: vec![IssuanceInvariant {
                        asset: [1; 32],
                        current: StoredAmount {
                            scope: recovery::StorageScope::Top,
                            key: b"audit-test-issuance".to_vec(),
                            record_hash: sp_io::hashing::blake2_256(&amount),
                            offset: 0,
                        },
                        checkpoint_issuance: 10,
                        authenticated_mints: 0,
                        burned: 0,
                    }],
                    collateral: vec![],
                    zero_claims: vec![],
                    absent: vec![],
                };
                policy.terminal_audit_commitment = terminal.commitment();
                recovery::storage::initialize(&policy).unwrap();
                apply_operation(&policy, &operation).unwrap();
                sp_io::storage::set(
                    recovery::AUDIT_CURSOR_STORAGE_KEY,
                    &(policy.audit_chunk_hashes.len() as u32).encode(),
                );
                if missing {
                    sp_io::storage::clear(recovery::INCOMING_AUDIT_PENDING_KEY);
                }
                assert_eq!(
                    recovery::invariants::verify_and_record(&policy, &terminal).unwrap_err(),
                    RecoveryError::IncompleteAudit
                );
            });
        }
    }

    #[test]
    fn native_hashi_consumption_and_exclusion_have_typed_delay_independent_audits() {
        for excluded in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (mut policy, operation, _) = hashi_fixture(excluded);
                let audit = make_audit(&policy, &operation);
                policy.audit_chunk_hashes = vec![audit.commitment()];
                crate::incident_recovery::with_test_policy(&policy, || {
                    recovery::storage::initialize(&policy).unwrap();
                    apply_operation(&policy, &operation).unwrap();
                    let value = companion(&policy, &operation);
                    if !excluded {
                        let transfer = native_hashi_transfer(&operation.channel).unwrap();
                        assert_eq!(audit.requests[0].as_ref().unwrap().amount, transfer.amount);
                        assert_eq!(
                            audit.requests[0].as_ref().unwrap().source,
                            GenericAccount::EVM(transfer.from)
                        );
                    }
                    System::set_block_number(value.execution_block + 100);
                    run(&policy, &audit).unwrap();
                    assert_eq!(pending().unwrap(), 0);
                });
            });
        }
    }

    #[test]
    fn compact_audit_has_no_original_proof_size_amplification_and_rejects_bad_bounds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, _) = fixture(AccountId::new([7; 32]));
            let mut audit = make_audit(&policy, &operation);
            assert!(audit.validate().is_ok());
            assert!(audit.encoded_size() < 1024);
            audit.requests =
                vec![audit.requests[0].clone(); BridgeMaxMessagesPerCommit::get() as usize];
            assert!(audit.validate().is_ok());
            audit.requests.push(None);
            assert_eq!(audit.validate(), Err(RecoveryError::ChunkBounds));
            assert_eq!(
                recovery::RecoveryAction::IncomingAudit(vec![1]).encode()[0],
                9
            );
        });
    }

    #[test]
    fn companion_failure_rolls_back_native_credit_nonce_receipt_and_pending_together() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, _) = fixture(AccountId::new([7; 32]));
            let target = prepare(&operation, &policy).unwrap().transfers.remove(0);
            let old_request = audit_request_key(target.network, &target.original, target.id);
            sp_io::storage::set(&old_request, b"stale old-owner row");
            recovery::storage::initialize(&policy).unwrap();
            let balance_before =
                crate::Assets::free_balance(&common::XOR, &target.beneficiary).unwrap();
            assert_eq!(
                apply_operation(&policy, &operation).unwrap_err(),
                RecoveryError::WrongPreimage
            );
            assert_eq!(
                crate::Assets::free_balance(&common::XOR, &target.beneficiary).unwrap(),
                balance_before
            );
            assert_eq!(
                substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(
                    SubNetworkId::Liberland
                ),
                0
            );
            assert!(!sp_io::storage::exists(&receipt_key(
                target.network,
                target.id
            )));
            assert!(!sp_io::storage::exists(&companion_key(
                policy.commitment(),
                operation.commitment()
            )));
            assert_eq!(pending().unwrap(), 0);
            assert_eq!(
                sp_io::storage::get(&old_request).unwrap().to_vec(),
                b"stale old-owner row".to_vec()
            );
        });
    }

    #[test]
    fn incoming_bookkeeping_weights_cover_all_twenty_transfer_bounded_reads() {
        let n = u64::from(BridgeMaxMessagesPerCommit::get());
        let db = <Runtime as frame_system::Config>::DbWeight::get();
        assert!(db
            .reads_writes(16 + n * 14, 2)
            .all_lte(consumption_weight()));
        assert!(db
            .reads_writes(20 + n * 12, 3)
            .all_lte(incoming_audit_weight()));
        assert!(incoming_audit_weight().proof_size() >= (MAX_CHUNK_BYTES * 2) as u64);
    }

    #[test]
    fn old_ready_state_cannot_skip_missing_or_outstanding_incoming_audits() {
        for value in [None, Some(1u32), Some(0u32)] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, _, _) = fixture(AccountId::new([7; 32]));
                recovery::storage::initialize(&policy).unwrap();
                let mut status = recovery::storage::status().unwrap().unwrap();
                status.phase = RecoveryPhase::Ready;
                sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
                match value {
                    None => sp_io::storage::clear(recovery::INCOMING_AUDIT_PENDING_KEY),
                    Some(n) => {
                        sp_io::storage::set(recovery::INCOMING_AUDIT_PENDING_KEY, &n.encode())
                    }
                }
                assert_eq!(
                    recovery::storage::mark_complete(&policy),
                    if value == Some(0) {
                        Ok(())
                    } else {
                        Err(RecoveryError::IncompleteAudit)
                    }
                );
            });
        }
    }

    #[test]
    fn maximal_twenty_transfer_companion_and_compact_audit_are_encodable() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation, _) = fixture(AccountId::new([7; 32]));
            let IncomingChannel::Substrate {
                commitment: GenericCommitment::Sub(batch),
                ..
            } = &mut operation.channel
            else {
                unreachable!()
            };
            assert_eq!(BridgeMaxMessagesPerCommit::get(), 20);
            batch.messages = vec![batch.messages[0].clone(); 20].try_into().unwrap();
            policy.chunk_hashes = vec![operation.commitment()];
            recovery::storage::initialize(&policy).unwrap();
            let state = recovery::storage::status().unwrap().unwrap();
            let active = prepare(&operation, &policy).unwrap();
            // Structural size check only: the deliberately changed message vector
            // does not claim the original fixture signatures authenticate it.
            let value = prepare_consumption(&operation, &active, &state).unwrap();
            assert_eq!(value.transfers.len(), 20);
            assert!(value.encoded_size() + 20 * 32 <= MAX_CHUNK_BYTES);
            let audit = make_audit(&policy, &operation);
            assert_eq!(audit.requests.len(), 20);
            audit.validate().unwrap();
            assert!(audit.encoded_size() < MAX_CHUNK_BYTES);
        });
    }

    #[test]
    fn consumed_hashi_audit_rejects_oversized_native_queue_before_decoding_it() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, operation, _) = hashi_fixture(false);
            let audit = make_audit(&policy, &operation);
            policy.audit_chunk_hashes = vec![audit.commitment()];
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                apply_operation(&policy, &operation).unwrap();
                let native = companion(&policy, &operation).native;
                let NativeConsumption::Hashi(network, _, _, _) = native else {
                    unreachable!()
                };
                let queue_key = eth_bridge::RequestsQueue::<Runtime>::hashed_key_for(network);
                sp_io::storage::set(&queue_key, &vec![0; MAX_CHUNK_BYTES + 1]);
                assert_eq!(
                    run(&policy, &audit).unwrap_err(),
                    RecoveryError::ChunkBounds
                );
                assert_eq!(pending().unwrap(), 1);
            });
        });
    }

    #[test]
    fn hashi_load_height_preserves_native_fresh_retained_and_rejected_failed_paths() {
        for mode in 0..4 {
            sp_io::TestExternalities::default().execute_with(|| {
                let (mut policy, operation, _) = hashi_fixture(false);
                let transfer = native_hashi_transfer(&operation.channel).unwrap();
                let mut timepoint = transfer.timepoint;
                if mode == 2 {
                    timepoint.index += 1;
                } // Different failed prior load.
                if mode != 0 {
                    let load = eth_bridge::requests::OffchainRequest::<Runtime>::LoadIncoming(
                        eth_bridge::requests::LoadIncomingRequest::Transaction(
                            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                                transfer.author.clone(),
                                transfer.tx_hash,
                                timepoint,
                                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                                transfer.network_id,
                            ),
                        ),
                    );
                    eth_bridge::Requests::<Runtime>::insert(
                        transfer.network_id,
                        transfer.tx_hash,
                        load,
                    );
                    eth_bridge::RequestSubmissionHeight::<Runtime>::insert(
                        transfer.network_id,
                        transfer.tx_hash,
                        recovery::CHECKPOINT_NUMBER - 3,
                    );
                    eth_bridge::RequestStatuses::<Runtime>::insert(
                        transfer.network_id,
                        transfer.tx_hash,
                        if mode == 1 {
                            eth_bridge::requests::RequestStatus::Pending
                        } else {
                            eth_bridge::requests::RequestStatus::Failed(
                                sp_runtime::DispatchError::Other("fixture"),
                            )
                        },
                    );
                    if mode == 1 {
                        eth_bridge::RequestsQueue::<Runtime>::insert(
                            transfer.network_id,
                            vec![transfer.tx_hash],
                        );
                        eth_bridge::AccountRequests::<Runtime>::insert(
                            &transfer.author,
                            vec![(transfer.network_id, transfer.tx_hash)],
                        );
                    }
                }
                let height_key = eth_bridge::RequestSubmissionHeight::<Runtime>::hashed_key_for(
                    transfer.network_id,
                    transfer.tx_hash,
                );
                let dynamic = audit_dynamic_keys(&operation, &policy).unwrap();
                assert_eq!(dynamic.contains(&height_key), mode == 0 || mode == 2);
                let audit = make_audit(&policy, &operation);
                policy.audit_chunk_hashes = vec![audit.commitment()];
                crate::incident_recovery::with_test_policy(&policy, || {
                    recovery::storage::initialize(&policy).unwrap();
                    System::set_block_number(System::block_number() + 7);
                    let block = System::block_number();
                    if mode >= 2 {
                        // Native protocol validation rejects failed existing loads,
                        // including different payloads that add_request alone could
                        // resubmit. A height descriptor cannot authorize that retry.
                        assert_eq!(
                            apply_operation(&policy, &operation).unwrap_err(),
                            RecoveryError::WrongPreimage
                        );
                        assert_eq!(pending().unwrap(), 0);
                        return;
                    }
                    apply_operation(&policy, &operation)
                        .unwrap_or_else(|error| panic!("HASHI load mode {mode}: {error:?}"));
                    assert_eq!(
                        eth_bridge::RequestSubmissionHeight::<Runtime>::get(
                            transfer.network_id,
                            transfer.tx_hash
                        ),
                        if mode == 1 {
                            recovery::CHECKPOINT_NUMBER - 3
                        } else {
                            block
                        }
                    );
                    System::set_block_number(block + 100);
                    run(&policy, &audit).unwrap();
                    assert_eq!(pending().unwrap(), 0);
                });
            });
        }
    }

    #[test]
    fn hashi_audit_rejects_changed_consumption_height_without_adopting_observed_value() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, operation, _) = hashi_fixture(false);
            let transfer = native_hashi_transfer(&operation.channel).unwrap();
            let audit = make_audit(&policy, &operation);
            policy.audit_chunk_hashes = vec![audit.commitment()];
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                apply_operation(&policy, &operation).unwrap();
                eth_bridge::RequestSubmissionHeight::<Runtime>::insert(
                    transfer.network_id,
                    transfer.tx_hash,
                    System::block_number() - 1,
                );
                assert_eq!(
                    run(&policy, &audit).unwrap_err(),
                    RecoveryError::WrongPreimage
                );
                assert_eq!(pending().unwrap(), 1);
            });
        });
    }
}
