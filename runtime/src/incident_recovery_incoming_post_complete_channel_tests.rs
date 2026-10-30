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
fn recovery_channels_post_complete_attacker_redirect_requires_matching_policy() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation) = fixture(
            ChannelKind::Substrate,
            AccountId::new(recovery::ATTACKER_ACCOUNT),
        );
        crate::incident_recovery::with_test_policy(&policy, || {
            complete_for_test(&policy);
            sp_io::storage::set(POLICY_STORAGE_KEY, &[99; 32]);
            assert_ok!(submit_native(&operation));
            assert_eq!(native_nonce(&operation), 1);
            assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), 0);
            assert_eq!(
                crate::Assets::free_balance(&common::DAI, &migration()).unwrap(),
                0
            );
        });
    });
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

#[cfg(feature = "wip")]
#[test]
fn recovery_channels_post_complete_wrong_app_origin_is_not_repaired_by_remapping() {
    let generic_kinds = vec![ChannelKind::Evm];
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

fn attacker() -> AccountId {
    AccountId::new(recovery::ATTACKER_ACCOUNT)
}

fn migration() -> AccountId {
    AccountId::new(recovery::ATTACKER_MIGRATION_ACCOUNT)
}

#[derive(Clone, Copy, Debug)]
enum Party {
    AttackerSender,
    AttackerRecipient,
    Ordinary,
}

/// Re-signed fixture whose authenticated sender is the attacker's identity on
/// that channel. TON has no attacker sender identity; it is covered by recipient.
fn attacker_sender_payload(operation: &mut IncomingOperation) {
    edit_payload(&mut operation.channel, |mut call| {
        match &mut call {
            BridgeCall::SubstrateApp(SubstrateAppCall::Transfer { sender, .. }) => {
                *sender = GenericAccount::Liberland(attacker());
            }
            BridgeCall::ParachainApp(ParachainAppCall::Transfer { sender, .. }) => {
                *sender = Some(xcm::VersionedLocation::V4(xcm::v4::Location::new(
                    1,
                    [xcm::v4::Junction::AccountId32 {
                        network: None,
                        id: recovery::ATTACKER_ACCOUNT,
                    }],
                )));
            }
            #[cfg(feature = "wip")]
            BridgeCall::FAApp(FAAppCall::Transfer { sender, .. }) => {
                *sender = bridge_types::H160::from(recovery::ATTACKER_EVM);
            }
            _ => unreachable!(),
        }
        call
    });
    sign(&mut operation.channel);
}

#[test]
fn recovery_channels_post_complete_attacker_proceeds_credit_migration_account() {
    let cases = vec![
        (ChannelKind::Substrate, Party::AttackerSender),
        (ChannelKind::Parachain, Party::AttackerSender),
        #[cfg(feature = "wip")]
        (ChannelKind::Evm, Party::AttackerSender),
        (ChannelKind::Substrate, Party::AttackerRecipient),
        (ChannelKind::Parachain, Party::AttackerRecipient),
        #[cfg(feature = "wip")]
        (ChannelKind::Evm, Party::AttackerRecipient),
        (ChannelKind::Substrate, Party::Ordinary),
        (ChannelKind::Parachain, Party::Ordinary),
        #[cfg(feature = "wip")]
        (ChannelKind::Evm, Party::Ordinary),
    ];
    for (kind, party) in cases {
        sp_io::TestExternalities::default().execute_with(|| {
            // An attacker sender names a retired recipient: the redirect takes
            // precedence over its replacement. Ordinary senders keep the remap.
            let recipient = match party {
                Party::AttackerRecipient => attacker(),
                Party::AttackerSender | Party::Ordinary => old(),
            };
            let (policy, mut operation) = fixture(kind, recipient.clone());
            if matches!(party, Party::AttackerSender) {
                attacker_sender_payload(&mut operation);
            }
            let owner = match party {
                Party::AttackerSender | Party::AttackerRecipient => migration(),
                Party::Ordinary => replacement(),
            };
            let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
            let signed = operation.encode();
            crate::incident_recovery::with_test_policy(&policy, || {
                complete_for_test(&policy);
                let status = sp_io::storage::get(STATUS_STORAGE_KEY).unwrap();
                assert_ok!(submit_native(&operation));
                assert_effective_credit(&operation, &entry, &owner);
                assert_eq!(native_nonce(&operation), 1);
                assert_eq!(operation.encode(), signed, "{kind:?} {party:?}");
                assert_eq!(sp_io::storage::get(STATUS_STORAGE_KEY).unwrap(), status);
                for account in [recipient.clone(), replacement(), attacker(), migration()] {
                    if account != owner {
                        assert_eq!(
                            crate::Assets::free_balance(&common::DAI, &account).unwrap(),
                            0,
                            "{kind:?} {party:?}"
                        );
                        assert!(!frame_system::Account::<Runtime>::contains_key(&account));
                    }
                }
                // Ordinary delivery writes no maintenance receipt or failure context.
                assert!(!sp_io::storage::exists(&receipt_key(
                    operation.network(),
                    entry.id
                )));
                assert!(!sp_io::storage::exists(FAILED_KEY));
                assert!(!sp_io::storage::exists(ACTIVE_KEY));
                // The native nonce remains the only replay identity.
                let state = sp_io::storage::root(sp_runtime::StateVersion::V1);
                assert!(submit_native(&operation).is_err());
                assert_eq!(state, sp_io::storage::root(sp_runtime::StateVersion::V1));
            });
        });
    }
}

#[test]
fn recovery_channels_post_complete_attacker_redirect_covers_shapes_parties_rejects() {
    // Native SubstrateApp also mints to Liberland-tagged recipients and accepts
    // unattributed senders; maintenance attribution (`parties`) rejects both.
    for (sender, recipient) in [
        (
            GenericAccount::Liberland(attacker()),
            GenericAccount::Liberland(old()),
        ),
        (
            GenericAccount::Sora(attacker()),
            GenericAccount::Liberland(AccountId::new([9; 32])),
        ),
        (
            GenericAccount::Unknown,
            GenericAccount::Liberland(attacker()),
        ),
        (GenericAccount::Root, GenericAccount::Sora(attacker())),
    ] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, mut operation) = fixture(ChannelKind::Substrate, old());
            edit_payload(&mut operation.channel, |mut call| {
                let BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
                    sender: value,
                    recipient: to,
                    ..
                }) = &mut call
                else {
                    unreachable!()
                };
                *value = sender.clone();
                *to = recipient.clone();
                call
            });
            sign(&mut operation.channel);
            assert!(parties(&transfer_call(&operation)).is_err());
            assert!(attacker_transfer(&transfer_call(&operation)));
            crate::incident_recovery::with_test_policy(&policy, || {
                complete_for_test(&policy);
                assert_ok!(submit_native(&operation));
                assert_eq!(native_nonce(&operation), 1);
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &migration()).unwrap(),
                    AMOUNT
                );
                for account in [old(), replacement(), attacker(), AccountId::new([9; 32])] {
                    assert_eq!(
                        crate::Assets::free_balance(&common::DAI, &account).unwrap(),
                        0
                    );
                }
                assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), AMOUNT);
            });
        });
    }
}

#[test]
fn recovery_attacker_sender_matches_each_identity_form_only() {
    // Registry lookups read storage, so identity checks need externalities.
    sp_io::TestExternalities::default().execute_with(|| {
        use xcm::{v3, v4, v5, VersionedLocation};
        let other = AccountId::new([6; 32]);
        for (sender, expected) in [
            (GenericAccount::Sora(attacker()), true),
            (GenericAccount::Liberland(attacker()), true),
            (
                GenericAccount::EVM(bridge_types::H160::from(recovery::ATTACKER_EVM)),
                true,
            ),
            (GenericAccount::Sora(other.clone()), false),
            (GenericAccount::Liberland(other.clone()), false),
            (
                GenericAccount::EVM(bridge_types::H160::repeat_byte(6)),
                false,
            ),
            (
                GenericAccount::TON(bridge_types::ton::TonAddress::new(
                    0,
                    H256::from(recovery::ATTACKER_ACCOUNT),
                )),
                false,
            ),
            (GenericAccount::Unknown, false),
            (GenericAccount::Root, false),
        ] {
            assert_eq!(attacker_sender(&sender), expected, "{sender:?}");
        }
        let locations = [
            (
                VersionedLocation::V3(v3::MultiLocation::new(
                    1,
                    v3::Junctions::X2(
                        v3::Junction::Parachain(2000),
                        v3::Junction::AccountId32 {
                            network: None,
                            id: recovery::ATTACKER_ACCOUNT,
                        },
                    ),
                )),
                true,
            ),
            (
                VersionedLocation::V4(v4::Location::new(
                    0,
                    [v4::Junction::AccountKey20 {
                        network: None,
                        key: recovery::ATTACKER_EVM,
                    }],
                )),
                true,
            ),
            (
                VersionedLocation::V5(v5::Location::new(
                    1,
                    [
                        v5::Junction::Parachain(2004),
                        v5::Junction::AccountId32 {
                            network: Some(v5::NetworkId::Polkadot),
                            id: recovery::ATTACKER_ACCOUNT,
                        },
                    ],
                )),
                true,
            ),
            (VersionedLocation::V4(v4::Location::here()), false),
            (
                VersionedLocation::V5(v5::Location::new(
                    1,
                    [v5::Junction::AccountId32 {
                        network: None,
                        id: [6; 32],
                    }],
                )),
                false,
            ),
            (
                VersionedLocation::V3(v3::MultiLocation::new(
                    0,
                    v3::Junctions::X1(v3::Junction::AccountKey20 {
                        network: None,
                        key: [6; 20],
                    }),
                )),
                false,
            ),
        ];
        for (location, expected) in locations {
            assert_eq!(
                attacker_sender(&GenericAccount::Parachain(location.clone())),
                expected,
                "{location:?}"
            );
        }
    });
}

#[test]
fn recovery_attacker_registry_and_collection_account_extend_identities() {
    use xcm::{v4, VersionedLocation};
    sp_io::TestExternalities::default().execute_with(|| {
        let collection = AccountId::new(recovery::ATTACKER_COLLECTION_ACCOUNT);
        let traced = AccountId::new([31; 32]);
        let traced_evm = [32u8; 20];
        let located = GenericAccount::Parachain(VersionedLocation::V4(v4::Location::new(
            1,
            [
                v4::Junction::Parachain(2000),
                v4::Junction::AccountId32 {
                    network: None,
                    id: [31; 32],
                },
            ],
        )));
        let redirect = |from: [u8; 20], to: &AccountId| {
            <PersistentAccountResolver as eth_bridge::recovery::IncomingAccountResolver<
                AccountId,
            >>::redirect_transfer(&bridge_types::H160::from(from), to)
        };

        // The recorded collection account is an attacker identity everywhere.
        assert!(attacker_sender(&GenericAccount::Sora(collection.clone())));
        assert!(attacker_sender(&GenericAccount::Liberland(
            collection.clone()
        )));
        // Untraced identities are ordinary until root registers them.
        assert!(!attacker_sender(&GenericAccount::Sora(traced.clone())));
        assert!(!attacker_sender(&located));
        assert!(!attacker_sender(&GenericAccount::EVM(
            bridge_types::H160::from(traced_evm)
        )));

        // HASHI redirects need the recovery policy and an open bridge.
        assert_eq!(redirect([0; 20], &collection), None);
        sp_io::storage::set(POLICY_STORAGE_KEY, &[1]);
        assert_eq!(redirect([0; 20], &collection), Some(migration()));
        assert_eq!(redirect(traced_evm, &old()), None);
        sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
        assert_eq!(redirect([0; 20], &collection), None, "halted");
        sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);

        sp_io::storage::set(&recovery::attacker_registry_account_key(&[31; 32]), &[1]);
        sp_io::storage::set(&recovery::attacker_registry_evm_key(&traced_evm), &[1]);
        assert!(attacker_sender(&GenericAccount::Sora(traced.clone())));
        assert!(attacker_sender(&GenericAccount::Liberland(traced.clone())));
        assert!(attacker_sender(&located));
        assert!(attacker_sender(&GenericAccount::EVM(
            bridge_types::H160::from(traced_evm)
        )));
        assert_eq!(redirect(traced_evm, &old()), Some(migration()));
        assert_eq!(redirect([0; 20], &traced), Some(migration()));
        // A registry entry never marks other accounts.
        assert!(!attacker_sender(&GenericAccount::Sora(AccountId::new(
            [33; 32]
        ))));
        assert_eq!(redirect([33; 20], &old()), None);
    });
}

#[test]
fn recovery_channels_post_complete_registered_attacker_sender_credits_migration_account() {
    sp_io::TestExternalities::default().execute_with(|| {
        let traced = AccountId::new([31; 32]);
        let (policy, mut operation) = fixture(ChannelKind::Substrate, old());
        edit_payload(&mut operation.channel, |mut call| {
            let BridgeCall::SubstrateApp(SubstrateAppCall::Transfer { sender, .. }) = &mut call
            else {
                unreachable!()
            };
            *sender = GenericAccount::Liberland(traced.clone());
            call
        });
        sign(&mut operation.channel);
        assert!(!attacker_transfer(&transfer_call(&operation)));
        sp_io::storage::set(
            &recovery::attacker_registry_account_key(AsRef::<[u8; 32]>::as_ref(&traced)),
            &[1],
        );
        assert!(attacker_transfer(&transfer_call(&operation)));
        crate::incident_recovery::with_test_policy(&policy, || {
            complete_for_test(&policy);
            assert_ok!(submit_native(&operation));
            assert_eq!(native_nonce(&operation), 1);
            assert_eq!(
                crate::Assets::free_balance(&common::DAI, &migration()).unwrap(),
                AMOUNT
            );
            for account in [old(), replacement(), traced.clone()] {
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &account).unwrap(),
                    0
                );
            }
        });
    });
}

#[test]
fn recovery_channels_post_complete_ton_remains_disabled_with_historical_bindings() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation) = fixture(ChannelKind::Ton, old());
        let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
        crate::incident_recovery::with_test_policy(&policy, || {
            complete_for_test(&policy);
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert_eq!(
                submit_native(&operation).unwrap_err().error,
                jetton_app::Error::<Runtime>::BridgeDisabled.into()
            );
            assert_eq!(native_nonce(&operation), 0);
            assert_no_credit(&operation, &entry);
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        });
    });
}
