//! Candidate-only regressions for delayed ordinary native delivery after Complete.
//! The halt is cleared only in test storage. No runtime call reopens a bridge.

use super::*;

fn complete_for_test(policy: &RecoveryPolicy) {
    recovery::storage::initialize(policy).unwrap();
    let mut status = recovery::storage::status().unwrap().unwrap();
    status.phase = RecoveryPhase::Complete;
    status.next_chunk = policy.chunk_hashes.len() as u32;
    sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
    // Model a separately approved future reopening without changing production
    // admission or adding a method to clear the permanent release halt.
    sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);
    assert!(!sp_io::storage::exists(ACTIVE_KEY));
    assert!(!sp_io::storage::exists(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY));
}

fn submit_native(
    operation: &IncomingOperation,
) -> frame_support::dispatch::DispatchResultWithPostInfo {
    match &operation.channel {
        IncomingChannel::Substrate {
            network,
            commitment,
            proof,
        } => SubstrateBridgeInboundChannel::submit(
            RuntimeOrigin::none(),
            *network,
            commitment.clone(),
            proof.clone(),
        ),
        IncomingChannel::Generic {
            network,
            commitment,
            proof,
        } => BridgeInboundChannel::submit(
            RuntimeOrigin::none(),
            *network,
            commitment.clone(),
            proof.clone(),
        ),
        IncomingChannel::Hashi { .. } | IncomingChannel::HashiAuthenticatedSource { .. } => {
            unreachable!()
        }
    }
}

fn edit_payload(channel: &mut IncomingChannel, edit: impl FnOnce(BridgeCall) -> BridgeCall) {
    let commitment = match channel {
        IncomingChannel::Substrate { commitment, .. }
        | IncomingChannel::Generic { commitment, .. } => commitment,
        IncomingChannel::Hashi { .. } | IncomingChannel::HashiAuthenticatedSource { .. } => {
            unreachable!()
        }
    };
    let payload = match commitment {
        GenericCommitment::Sub(batch) => &mut batch.messages[0].payload,
        GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(message)) => {
            &mut message.payload
        }
        GenericCommitment::EVM(bridge_types::evm::Commitment::Inbound(message)) => {
            &mut message.payload
        }
        _ => unreachable!(),
    };
    let call = decode_call(payload).unwrap();
    *payload = edit(call).encode().try_into().unwrap();
}

fn assert_effective_credit(
    operation: &IncomingOperation,
    entry: &ExpectedTransfer,
    owner: &AccountId,
) {
    assert_eq!(
        crate::Assets::free_balance(&common::DAI, owner).unwrap(),
        AMOUNT
    );
    assert_eq!(
        crate::Assets::free_balance(&common::DAI, &old()).unwrap(),
        0
    );
    assert!(!frame_system::Account::<Runtime>::contains_key(old()));
    assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), AMOUNT);
    assert_eq!(
        crate::BridgeProxy::locked_assets(operation.network(), common::DAI),
        AMOUNT
    );
    assert_eq!(
        crate::BridgeProxy::sender(operation.network(), entry.id),
        Some(owner.clone())
    );
    assert!(crate::BridgeProxy::transactions((operation.network(), old()), entry.id).is_none());
    let request =
        crate::BridgeProxy::transactions((operation.network(), owner.clone()), entry.id).unwrap();
    assert_eq!(request.source(), &entry.sender);
    assert_eq!(request.amount(), AMOUNT);
    assert_eq!(request.status(), MessageStatus::Done);
    assert_eq!(request.direction(), MessageDirection::Inbound);
}

#[test]
fn recovery_channels_post_complete_original_payload_credits_replacement_once() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation) = fixture(kind, old());
            let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
            let original = operation.encode();
            crate::incident_recovery::with_test_policy(&policy, || {
                complete_for_test(&policy);
                let status = sp_io::storage::get(STATUS_STORAGE_KEY).unwrap();
                assert_ok!(submit_native(&operation));
                assert_effective_credit(&operation, &entry, &replacement());
                assert_eq!(native_nonce(&operation), 1);
                assert_eq!(operation.encode(), original, "{kind:?}");
                assert_eq!(sp_io::storage::get(STATUS_STORAGE_KEY).unwrap(), status);
                assert_eq!(
                    sp_io::storage::get(recovery::INCOMING_AUDIT_PENDING_KEY)
                        .unwrap()
                        .to_vec(),
                    0u32.encode()
                );
                let companion_prefix = b":sora-nexus:recovery:incoming-consumption:v1:";
                assert!(!sp_io::storage::next_key(companion_prefix)
                    .is_some_and(|key| key.starts_with(companion_prefix)));

                assert!(!sp_io::storage::exists(&receipt_key(
                    operation.network(),
                    entry.id
                )));
                assert!(!sp_io::storage::exists(ACTIVE_KEY));
                assert!(!sp_io::storage::exists(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY));
                let state = sp_io::storage::root(sp_runtime::StateVersion::V1);
                assert!(submit_native(&operation).is_err());
                assert_eq!(state, sp_io::storage::root(sp_runtime::StateVersion::V1));
                assert_effective_credit(&operation, &entry, &replacement());
            });
        });
    }
}

#[test]
fn recovery_channels_post_complete_release_halt_still_blocks_ordinary_delivery() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation) = fixture(kind, old());
            let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
            crate::incident_recovery::with_test_policy(&policy, || {
                let mut status = recovery::storage::initialize(&policy).unwrap();
                status.phase = RecoveryPhase::Complete;
                sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
                let state = sp_io::storage::root(sp_runtime::StateVersion::V1);
                assert!(submit_native(&operation).is_err());
                assert_eq!(state, sp_io::storage::root(sp_runtime::StateVersion::V1));
                assert!(sp_io::storage::exists(BRIDGE_HALT_STORAGE_KEY));
                assert_eq!(native_nonce(&operation), 0);
                assert_no_credit(&operation, &entry);
            });
        });
    }
}

#[test]
fn recovery_channels_post_complete_unmapped_recipient_is_preserved() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let recipient = AccountId::new([9; 32]);
            let (policy, operation) = fixture(kind, recipient.clone());
            let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
            let original = operation.encode();
            crate::incident_recovery::with_test_policy(&policy, || {
                complete_for_test(&policy);
                assert_ok!(submit_native(&operation));
                assert_effective_credit(&operation, &entry, &recipient);
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
                    0
                );
                assert_eq!(operation.encode(), original);
            });
        });
    }
}

#[test]
fn recovery_channels_post_complete_bad_signature_or_changed_recipient_fails_before_nonce() {
    for kind in kinds() {
        for changed_recipient in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, mut operation) = fixture(kind, old());
                let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
                if changed_recipient {
                    edit_payload(&mut operation.channel, |call| {
                        decode_call(&replace_recipient(call, AccountId::new([9; 32])).unwrap())
                            .unwrap()
                    });
                    // Deliberately retain signatures over the original old-account payload.
                } else {
                    let proof = match &mut operation.channel {
                        IncomingChannel::Substrate { proof, .. }
                        | IncomingChannel::Generic { proof, .. } => proof,
                        IncomingChannel::Hashi { .. }
                        | IncomingChannel::HashiAuthenticatedSource { .. } => unreachable!(),
                    };
                    match proof {
                        MultiProof::Multisig(proof) => proof.proof[0].0[64] = 255,
                        MultiProof::EVMMultisig(proof) => proof.proof[0].0[64] = 255,
                        #[cfg(any(feature = "wip", feature = "runtime-benchmarks"))]
                        _ => unreachable!(),
                    }
                }
                crate::incident_recovery::with_test_policy(&policy, || {
                    complete_for_test(&policy);
                    let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
                    assert!(submit_native(&operation).is_err());
                    assert_eq!(before, sp_io::storage::root(sp_runtime::StateVersion::V1));
                    assert_eq!(native_nonce(&operation), 0);
                    assert_no_credit(&operation, &entry);
                    assert_eq!(
                        crate::Assets::free_balance(&common::DAI, &AccountId::new([9; 32]))
                            .unwrap(),
                        0
                    );
                });
            });
        }
    }
}

#[test]
fn recovery_channels_post_complete_policy_mismatch_never_recreates_old_recipient() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation) = fixture(kind, old());
            let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
            crate::incident_recovery::with_test_policy(&policy, || {
                complete_for_test(&policy);
                sp_io::storage::set(POLICY_STORAGE_KEY, &[99; 32]);
                // Authentication succeeds and the native channel consumes the
                // delivery, but the inconsistent local map cannot mint to anyone.
                assert_ok!(submit_native(&operation));
                assert_eq!(native_nonce(&operation), 1);
                assert_no_credit(&operation, &entry);
                assert!(!sp_io::storage::exists(ACTIVE_KEY));
                assert!(!sp_io::storage::exists(FAILED_KEY));
                assert!(!sp_io::storage::exists(&receipt_key(
                    operation.network(),
                    entry.id
                )));
            });
        });
    }
}

#[test]
fn recovery_channels_post_complete_unknown_token_still_fails_native_application_checks() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, mut operation) = fixture(kind, old());
            let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
            edit_payload(&mut operation.channel, |mut call| {
                match &mut call {
                    BridgeCall::SubstrateApp(SubstrateAppCall::Transfer { asset_id, .. }) => {
                        *asset_id = GenericAssetId::Sora(common::XOR.into());
                    }
                    BridgeCall::ParachainApp(ParachainAppCall::Transfer { asset_id, .. }) => {
                        *asset_id = common::XOR.into();
                    }
                    BridgeCall::JettonApp(JettonAppCall::Transfer { token, .. }) => {
                        *token =
                            bridge_types::ton::TonAddress::new(0, H256::repeat_byte(98)).into();
                    }
                    #[cfg(feature = "wip")]
                    BridgeCall::FAApp(FAAppCall::Transfer { token, .. }) => {
                        *token = bridge_types::H160::repeat_byte(98);
                    }
                    _ => unreachable!(),
                }
                call
            });
            sign(&mut operation.channel);
            let signed_payload = operation.encode();
            crate::incident_recovery::with_test_policy(&policy, || {
                complete_for_test(&policy);
                assert_ok!(submit_native(&operation));
                assert_eq!(native_nonce(&operation), 1);
                assert_no_credit(&operation, &entry);
                assert_eq!(operation.encode(), signed_payload);
            });
        });
    }
}

#[test]
fn recovery_channels_post_complete_wrong_app_origin_is_not_repaired_by_remapping() {
    let generic_kinds = vec![
        ChannelKind::Ton,
        #[cfg(feature = "wip")]
        ChannelKind::Evm,
    ];
    for kind in generic_kinds {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, mut operation) = fixture(kind, old());
            let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
            let IncomingChannel::Generic { commitment, .. } = &mut operation.channel else {
                unreachable!()
            };
            match commitment {
                GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(message)) => {
                    message.source = bridge_types::ton::TonAddress::new(0, H256::repeat_byte(99));
                }
                GenericCommitment::EVM(bridge_types::evm::Commitment::Inbound(message)) => {
                    message.source = bridge_types::H160::repeat_byte(99);
                }
                _ => unreachable!(),
            }
            sign(&mut operation.channel);
            let original = operation.encode();
            crate::incident_recovery::with_test_policy(&policy, || {
                complete_for_test(&policy);
                assert_ok!(submit_native(&operation));
                assert_eq!(native_nonce(&operation), 1);
                assert_no_credit(&operation, &entry);
                assert_eq!(operation.encode(), original);
            });
        });
    }
}
