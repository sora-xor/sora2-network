//! Public-key-only fixtures. No offchain, transaction-pool or keystore extension
//! is installed. The current runtime tests the shared inner path using its real
//! RuntimeVersion; only the legacy bootstrap may expose the public API.
use super::super::tests::{hashi_fixture, sign_hashi};
use super::*;
use crate::AccountId;
use common::AssetInfoProvider;
use sp_core::{sr25519, Pair, H256};
use sp_runtime::{traits::Verify, Digest, DigestItem};

fn root() -> Vec<u8> {
    sp_io::storage::root(StateVersion::V0)
}

pub(crate) fn parent(policy: &RecoveryPolicy) -> Header {
    Header::new(
        System::block_number(),
        H256::repeat_byte(31),
        H256::from_slice(&root()),
        System::parent_hash(),
        Digest {
            logs: vec![DigestItem::Other(recovery::lineage_marker(
                policy.commitment(),
            ))],
        },
    )
}

pub(crate) fn fixture() -> (RecoveryPolicy, IncomingOperation, Vec<sr25519::Pair>) {
    let (policy, mut operation, keys) = hashi_fixture(false);
    frame_system::BlockHash::<Runtime>::insert(0, H256::from(recovery::GENESIS_HASH));
    System::set_parent_hash(H256::repeat_byte(44));
    sp_io::storage::set(
        b":code",
        b"public fixture stored code; not an executor identity",
    );
    let IncomingChannel::Hashi { approvals, .. } = &mut operation.channel else {
        unreachable!()
    };
    // The native helper signed under a separate public genesis fixture. Sign
    // again under this actual context; no caller-provided domain is accepted.
    for (approval, key) in approvals.iter_mut().zip(&keys) {
        sign_hashi(approval, key);
    }
    recovery::storage::initialize(&policy).unwrap();
    recovery::storage::record_recovery_anchor([44; 32]).unwrap();
    (policy, operation, keys)
}

#[test]
fn witness_api_is_limited_to_the_original_bootstrap_version() {
    assert_eq!(supported_version(134, 131), Ok(()));
    for (spec, transaction) in [(133, 131), (134, 134), (135, 131), (136, 131), (138, 134)] {
        assert_eq!(
            supported_version(spec, transaction),
            Err(RecoveryError::BadCode)
        );
    }
}

#[test]
fn witness_temporary_system_keys_match_native_setters_and_getters() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation, _) = fixture();
        let number_key = frame_support::storage::storage_prefix(b"System", b"Number");
        let parent_key = frame_support::storage::storage_prefix(b"System", b"ParentHash");
        // Fixture setup used FRAME's native setters, not our production writer.
        assert_eq!(
            &sp_io::storage::get(&number_key).unwrap()[..],
            System::block_number().encode().as_slice()
        );
        assert_eq!(
            &sp_io::storage::get(&parent_key).unwrap()[..],
            System::parent_hash().encode().as_slice()
        );
        crate::incident_recovery::with_test_policy(&policy, || {
            let before = root();
            let header = parent(&policy);
            let context = context(&header.encode(), &operation.encode()).unwrap();
            rollback(|| {
                execution_context(&context);
                assert_eq!(System::block_number(), *header.number() + 1);
                assert_eq!(System::parent_hash(), header.hash());
                assert_eq!(System::block_hash(*header.number()), header.hash());
                Ok(())
            })
            .unwrap();
            assert_eq!(root(), before);
        });
    });
}

#[test]
fn witness_preparation_uses_child_context_and_native_signed_payload_without_writes() {
    let mut ext = sp_io::TestExternalities::default();
    ext.execute_with(|| {
        let (policy, operation, _) = fixture();
        crate::incident_recovery::with_test_policy(&policy, || {
            let header = parent(&policy);
            let before = root();
            let context = context(&header.encode(), &operation.encode()).unwrap();
            let prepared = rollback(|| prepare_native(&context)).unwrap();
            assert_eq!(root(), before);
            assert_eq!(prepared.parent_hash, *header.hash().as_fixed_bytes());
            assert_eq!(
                prepared.parent_state_root,
                *header.state_root().as_fixed_bytes()
            );
            assert_eq!(prepared.execution_number, recovery::CHECKPOINT_NUMBER + 3);
            assert_eq!(prepared.recovery_anchor, [44; 32]);
            assert_eq!(prepared.operation_commitment, operation.commitment());
            assert_eq!(prepared.spec_version, crate::VERSION.spec_version);
            assert_eq!(
                prepared.transaction_version,
                crate::VERSION.transaction_version
            );
            assert!(!prepared.external_finality_verified);
            let IncomingChannel::Hashi { approvals, .. } = &operation.channel else {
                unreachable!()
            };
            assert_eq!(prepared.approvals.len(), approvals.len());
            for (actual, approval) in prepared.approvals.iter().zip(approvals) {
                assert_eq!(actual.nonce, 0);
                assert_eq!(
                    actual.birth_number,
                    u64::from(recovery::CHECKPOINT_NUMBER + 1)
                );
                assert_eq!(actual.birth_hash, [44; 32]);
                assert!(actual.death_number > u64::from(prepared.execution_number));
                assert!(approval
                    .signature
                    .verify(actual.payload.as_slice(), &approval.signer));
            }
            assert!(recovery::TRANSIENT_CONTEXT_KEYS
                .iter()
                .all(|key| !sp_io::storage::exists(key)));
        });
    });
    assert_eq!(
        ext.overlayed_changes()
            .offchain()
            .overlay()
            .changes()
            .count(),
        0
    );
}

#[test]
fn witness_native_admission_mints_to_replacement_and_consumes_nonces_only_inside_rollback() {
    let mut ext = sp_io::TestExternalities::default();
    ext.execute_with(|| {
        let (policy, operation, keys) = fixture();
        crate::incident_recovery::with_test_policy(&policy, || {
            let before = root();
            let context = context(&parent(&policy).encode(), &operation.encode()).unwrap();
            let admitted = rollback(|| {
                let result = admit_native(&context)?;
                assert_eq!(System::block_number(), recovery::CHECKPOINT_NUMBER + 3);
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &AccountId::new([8; 32])).unwrap(),
                    1_000_000_000_000_000_000
                );
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &AccountId::new([7; 32])).unwrap(),
                    0
                );
                for key in keys.iter().take(3) {
                    assert_eq!(System::account_nonce(AccountId::from(key.public())), 1);
                }
                assert_eq!(
                    recovery::storage::status()?.unwrap().phase,
                    RecoveryPhase::Auditing
                );
                Ok(result)
            })
            .unwrap();
            assert_eq!(root(), before);
            assert_eq!(admitted.resulting_cursor, 1);
            assert_eq!(admitted.resulting_operation_counter, 0);
            assert!(admitted.native_signatures_checked && admitted.native_incoming_executed);
            assert!(!admitted.persistent_state_changed && !admitted.external_finality_verified);
            assert_eq!(
                crate::Assets::free_balance(&common::DAI, &AccountId::new([8; 32])).unwrap(),
                0
            );
            assert_eq!(
                recovery::storage::status().unwrap().unwrap().phase,
                RecoveryPhase::Applying
            );
            for key in keys.iter().take(3) {
                assert_eq!(System::account_nonce(AccountId::from(key.public())), 0);
            }
        });
    });
    assert_eq!(
        ext.overlayed_changes()
            .offchain()
            .overlay()
            .changes()
            .count(),
        0
    );
}

#[test]
fn witness_failed_native_mint_discards_even_the_failure_marker() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation, _) = fixture();
        assets::AssetInfosV2::<Runtime>::mutate(common::DAI, |info| info.is_mintable = false);
        crate::incident_recovery::with_test_policy(&policy, || {
            let before = root();
            let context = context(&parent(&policy).encode(), &operation.encode()).unwrap();
            assert!(rollback(|| {
                let result = admit_native(&context);
                assert!(result.is_err());
                assert_eq!(
                    recovery::storage::status().unwrap().unwrap().phase,
                    RecoveryPhase::Failed
                );
                result
            })
            .is_err());
            assert_eq!(root(), before);
            assert_eq!(
                recovery::storage::status().unwrap().unwrap().phase,
                RecoveryPhase::Applying
            );
        });
    });
}

#[test]
fn witness_bad_last_signature_has_the_same_business_commitment_but_fails_native_admission() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, mut operation, _) = fixture();
        let commitment = operation.commitment();
        let IncomingChannel::Hashi { approvals, .. } = &mut operation.channel else {
            unreachable!()
        };
        approvals.last_mut().unwrap().signature = sr25519::Signature::from_raw([0; 64]).into();
        assert_eq!(operation.commitment(), commitment);
        crate::incident_recovery::with_test_policy(&policy, || {
            let before = root();
            let context = context(&parent(&policy).encode(), &operation.encode()).unwrap();
            assert!(rollback(|| prepare_native(&context)).is_ok());
            assert!(rollback(|| admit_native(&context)).is_err());
            assert_eq!(root(), before);
        });
    });
}

#[test]
fn witness_context_rejects_unbound_or_noncanonical_parent_and_work() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation, _) = fixture();
        crate::incident_recovery::with_test_policy(&policy, || {
            let header = parent(&policy);
            let before = root();
            let mut bad = header.clone();
            bad.set_state_root(H256::repeat_byte(91));
            assert!(context(&bad.encode(), &operation.encode()).is_err());
            bad = header.clone();
            bad.set_number(*header.number() - 1);
            assert!(context(&bad.encode(), &operation.encode()).is_err());
            bad = header.clone();
            bad.set_parent_hash(H256::repeat_byte(92));
            assert!(context(&bad.encode(), &operation.encode()).is_err());
            bad = header.clone();
            bad.digest_mut()
                .logs
                .push(DigestItem::Other(recovery::lineage_marker(
                    policy.commitment(),
                )));
            assert!(context(&bad.encode(), &operation.encode()).is_err());
            let mut trailing = header.encode();
            trailing.push(0);
            assert!(context(&trailing, &operation.encode()).is_err());
            assert!(context(&vec![0; MAX_PARENT_HEADER_BYTES + 1], &operation.encode()).is_err());
            let mut changed = operation.clone();
            changed.finalized_source_evidence = [93; 32];
            assert!(context(&header.encode(), &changed.encode()).is_err());
            assert_eq!(root(), before);
        });
    });
}

#[test]
fn witness_context_requires_the_existing_anchor_original_policy_and_clean_maintenance_state() {
    for mode in 0..5 {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, _) = fixture();
            match mode {
                0 => sp_io::storage::clear(recovery::RECOVERY_ANCHOR_STORAGE_KEY),
                1 => sp_io::storage::set(recovery::ACTIVE_POLICY_STORAGE_KEY, &[94; 32]),
                2 => sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[0]),
                3 => sp_io::storage::set(
                    recovery::BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
                    &System::block_number().encode(),
                ),
                4 => System::set_block_number(recovery::CHECKPOINT_NUMBER + 1),
                _ => unreachable!(),
            }
            crate::incident_recovery::with_test_policy(&policy, || {
                let before = root();
                assert!(context(&parent(&policy).encode(), &operation.encode()).is_err());
                assert_eq!(root(), before);
            });
        });
    }
}

#[test]
fn witness_stale_nonce_missing_birth_and_removed_peer_fail_without_state_changes() {
    for mode in 0..3 {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, keys) = fixture();
            let signer = AccountId::from(keys[0].public());
            match mode {
                0 => {
                    frame_system::Account::<Runtime>::mutate(&signer, |account| account.nonce = 1);
                }
                1 => {
                    frame_system::BlockHash::<Runtime>::remove(recovery::CHECKPOINT_NUMBER + 1);
                }
                2 => {
                    eth_bridge::Peers::<Runtime>::mutate(0, |peers| {
                        peers.remove(&signer);
                    });
                }
                _ => unreachable!(),
            }
            crate::incident_recovery::with_test_policy(&policy, || {
                let before = root();
                let context = context(&parent(&policy).encode(), &operation.encode()).unwrap();
                assert!(rollback(|| prepare_native(&context)).is_err());
                assert!(rollback(|| admit_native(&context)).is_err());
                assert_eq!(root(), before);
            });
        });
    }
}
