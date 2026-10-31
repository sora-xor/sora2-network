//! Native regression fixtures only: no result here is production replay evidence.
use super::*;
use crate::incident_recovery_incoming::tests::{
    fixture as channel_fixture, hashi_fixture_with_kind,
};
use common::AssetInfoProvider;
use frame_support::assert_ok;
use sp_core::{ecdsa, Pair};
use sp_runtime::traits::{Hash, Keccak256};

const ONE: u128 = 1_000_000_000_000_000_000;
fn replacement() -> AccountId {
    AccountId::new([8; 32])
}
fn old() -> AccountId {
    AccountId::new([7; 32])
}
fn native(
    policy: &RecoveryPolicy,
    operation: &IncomingOperation,
) -> Result<(RecoveryStatus, Weight), RecoveryError> {
    crate::incident_recovery::with_test_policy(policy, || {
        super::super::apply_operation(policy, operation)
    })
}
fn witness(location: Vec<u8>) -> StorageWitness {
    StorageWitness {
        scope: StorageScope::Top,
        value_hash: sp_io::storage::get(&location).map(|v| sp_io::hashing::blake2_256(&v)),
        key: location,
    }
}
fn incoming_audit(
    policy: &RecoveryPolicy,
    operation: &IncomingOperation,
    index: u32,
) -> IncomingAudit {
    IncomingAudit {
        index,
        operation_index: operation.index,
        operation_commitment: operation.commitment(),
        requests: audit::expected_requests(operation, policy).unwrap(),
        prior_policy: None,
    }
}
fn balance_audit(mut keys: Vec<Vec<u8>>, audits: &[IncomingAudit]) -> NativeBalanceAudit {
    keys.sort();
    keys.dedup();
    NativeBalanceAudit {
        baseline: RecoveryAuditChunk {
            index: audits.len() as u32,
            witnesses: keys.into_iter().map(witness).collect(),
        },
        sources: audits
            .iter()
            .cloned()
            .map(|audit| AuditedIncomingSource {
                audit,
                audited_policy: None,
            })
            .collect(),
    }
}
fn prepare_policy(
    policy: &mut RecoveryPolicy,
    operations: &[IncomingOperation],
    audits: &[IncomingAudit],
    balance: &NativeBalanceAudit,
) {
    policy.chunk_hashes = operations
        .iter()
        .map(IncomingOperation::commitment)
        .collect();
    policy.audit_chunk_hashes = audits
        .iter()
        .map(IncomingAudit::commitment)
        .chain(core::iter::once(balance.commitment()))
        .collect();
    recovery::storage::initialize(policy).unwrap();
}
fn consume_and_audit(
    policy: &RecoveryPolicy,
    operations: &[IncomingOperation],
    audits: &[IncomingAudit],
) {
    for (index, operation) in operations.iter().enumerate() {
        System::set_block_number(recovery::CHECKPOINT_NUMBER + index as u32 + 1);
        assert_ok!(native(policy, operation));
    }
    for (index, audit) in audits.iter().enumerate() {
        System::set_block_number(
            recovery::CHECKPOINT_NUMBER + operations.len() as u32 + index as u32 + 1,
        );
        assert_ok!(super::super::audit_encoded_with_policy(
            policy,
            &audit.encode()
        ));
    }
    assert_eq!(
        sp_io::storage::get(recovery::INCOMING_AUDIT_PENDING_KEY)
            .unwrap()
            .to_vec(),
        0u32.encode()
    );
}
fn channel_with_existing_balance() -> (RecoveryPolicy, IncomingOperation) {
    let (policy, operation, _) = channel_fixture(old());
    let owner = AccountId::new([45; 32]);
    System::inc_providers(&owner);
    assert_ok!(crate::Assets::register_asset_id(
        owner.clone(),
        common::XOR,
        common::AssetSymbol(b"XOR".to_vec()),
        common::AssetName(b"SORA Nexus".to_vec()),
        18,
        0,
        true,
        common::AssetType::Regular,
        None,
        None
    ));
    assert_ok!(
        <crate::BridgeProxy as bridge_types::traits::BridgeAssetRegistry<
            AccountId,
            crate::AssetId,
        >>::manage_asset(SubNetworkId::Liberland.into(), common::XOR)
    );
    assert_ok!(crate::Assets::mint_to(
        &common::XOR,
        &owner,
        &replacement(),
        10 * ONE
    ));
    frame_system::Account::<Runtime>::mutate(replacement(), |account| account.nonce = 17);
    assert_ok!(System::inc_consumers(&replacement()));
    (policy, operation)
}
fn second_channel(first: &IncomingOperation) -> IncomingOperation {
    let mut second = first.clone();
    second.index = 1;
    let IncomingChannel::Substrate {
        commitment, proof, ..
    } = &mut second.channel
    else {
        unreachable!()
    };
    let GenericCommitment::Sub(inner) = commitment else {
        unreachable!()
    };
    inner.nonce += 1;
    let digest = bridge_types::types::AuxiliaryDigest {
        logs: vec![bridge_types::types::AuxiliaryDigestItem::Commitment(
            ThisNetworkId::get(),
            commitment.hash(),
        )],
    };
    *proof = MultiProof::Multisig(multisig_verifier::Proof {
        digest: digest.clone(),
        proof: (1u8..=4)
            .map(|seed| {
                ecdsa::Pair::from_seed(&[seed; 32])
                    .sign_prehashed(Keccak256::hash_of(&digest).as_fixed_bytes())
            })
            .collect(),
    });
    second
}

#[test]
fn native_balance_audit_preserves_migrated_system_fields_across_multiple_credits() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (mut policy, first) = channel_with_existing_balance();
        let second = second_channel(&first);
        let operations = vec![first, second];
        let audits = operations
            .iter()
            .enumerate()
            .map(|(i, o)| incoming_audit(&policy, o, i as u32))
            .collect::<Vec<_>>();
        let keys = request_balance_keys(audits[0].requests[0].as_ref().unwrap()).to_vec();
        let balance = balance_audit(keys, &audits);
        let original_system = System::account(replacement());
        prepare_policy(&mut policy, &operations, &audits, &balance);
        consume_and_audit(&policy, &operations, &audits);
        assert_eq!(
            crate::Assets::free_balance(&common::XOR, &old()).unwrap(),
            0
        );
        assert_eq!(
            crate::Assets::free_balance(&common::XOR, &replacement()).unwrap(),
            12 * ONE
        );
        let changed = System::account(replacement());
        assert_eq!(
            (
                changed.nonce,
                changed.providers,
                changed.consumers,
                changed.sufficients
            ),
            (
                original_system.nonce,
                original_system.providers,
                original_system.consumers,
                original_system.sufficients
            )
        );
        assert_ne!(
            witness(frame_system::Account::<Runtime>::hashed_key_for(
                replacement()
            ))
            .value_hash,
            balance
                .baseline
                .witnesses
                .iter()
                .find(|w| w.key == frame_system::Account::<Runtime>::hashed_key_for(replacement()))
                .unwrap()
                .value_hash
        );
        // Run the original static-audit alternative in a rolled-back unit-only
        // branch. Its exact baseline must fail after native credit; the new
        // action below succeeds without changing that baseline witness.
        let actual_root = sp_io::storage::root(sp_core::storage::StateVersion::V0);
        sp_io::storage::start_transaction();
        let mut static_policy = policy.clone();
        static_policy.audit_chunk_hashes[balance.baseline.index as usize] =
            balance.baseline.commitment();
        sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &static_policy.commitment());
        let static_result = recovery::storage::audit_chunk(&static_policy, &balance.baseline);
        sp_io::storage::rollback_transaction();
        assert_eq!(static_result, Err(RecoveryError::WrongPreimage));
        assert_eq!(
            sp_io::storage::root(sp_core::storage::StateVersion::V0),
            actual_root
        );
        assert_ok!(audit_native_balances_with_policy(
            &policy,
            &balance.encode()
        ));
        let after = sp_io::storage::root(sp_core::storage::StateVersion::V0);
        assert!(audit_native_balances_with_policy(&policy, &balance.encode()).is_err());
        assert_eq!(
            sp_io::storage::root(sp_core::storage::StateVersion::V0),
            after,
            "audit replay must not alter state"
        );
    });
}

#[test]
fn native_balance_audit_rejects_omitted_reordered_or_duplicate_credit_links() {
    for case in 0..3 {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, first) = channel_with_existing_balance();
            let operations = vec![first.clone(), second_channel(&first)];
            let audits = operations
                .iter()
                .enumerate()
                .map(|(i, o)| incoming_audit(&policy, o, i as u32))
                .collect::<Vec<_>>();
            let mut balance = balance_audit(
                request_balance_keys(audits[0].requests[0].as_ref().unwrap()).to_vec(),
                &audits,
            );
            match case {
                0 => {
                    balance.sources.remove(0);
                }
                1 => balance.sources.reverse(),
                _ => balance.sources.push(balance.sources[0].clone()),
            }
            prepare_policy(&mut policy, &operations, &audits, &balance);
            consume_and_audit(&policy, &operations, &audits);
            assert!(audit_native_balances_with_policy(&policy, &balance.encode()).is_err());
            assert_eq!(System::account(replacement()).data.free, 12 * ONE);
        });
    }
}

#[test]
fn native_balance_audit_binds_entire_system_account_and_original_baseline() {
    for case in 0..3 {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, operation) = channel_with_existing_balance();
            let audit = incoming_audit(&policy, &operation, 0);
            let mut balance = balance_audit(
                request_balance_keys(audit.requests[0].as_ref().unwrap()).to_vec(),
                core::slice::from_ref(&audit),
            );
            if case == 0 {
                balance
                    .baseline
                    .witnesses
                    .iter_mut()
                    .find(|w| {
                        w.key == frame_system::Account::<Runtime>::hashed_key_for(replacement())
                    })
                    .unwrap()
                    .value_hash = Some([42; 32]);
            }
            prepare_policy(
                &mut policy,
                core::slice::from_ref(&operation),
                core::slice::from_ref(&audit),
                &balance,
            );
            consume_and_audit(
                &policy,
                core::slice::from_ref(&operation),
                core::slice::from_ref(&audit),
            );
            if case == 1 {
                frame_system::Account::<Runtime>::mutate(replacement(), |a| a.nonce += 1);
            }
            if case == 2 {
                assert_ok!(System::inc_consumers(&replacement()));
            }
            assert!(audit_native_balances_with_policy(&policy, &balance.encode()).is_err());
        });
    }
}

#[test]
fn native_balance_journal_rejects_intervening_non_native_account_changes() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (mut policy, first) = channel_with_existing_balance();
        let second = second_channel(&first);
        let operations = vec![first.clone(), second.clone()];
        let audits = operations
            .iter()
            .enumerate()
            .map(|(i, o)| incoming_audit(&policy, o, i as u32))
            .collect::<Vec<_>>();
        let balance = balance_audit(
            request_balance_keys(audits[0].requests[0].as_ref().unwrap()).to_vec(),
            &audits,
        );
        prepare_policy(&mut policy, &operations, &audits, &balance);
        assert_ok!(native(&policy, &first));
        frame_system::Account::<Runtime>::mutate(replacement(), |a| a.nonce += 1);
        let before = System::account(replacement());
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 2);
        assert!(native(&policy, &second).is_err());
        assert_eq!(
            System::account(replacement()),
            before,
            "rejected second credit cannot overwrite the intervening state"
        );
        assert_eq!(
            substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(
                SubNetworkId::Liberland
            ),
            1
        );
    });
}

#[test]
fn native_balance_audit_token_record_uses_native_credit_without_rebasing_custody() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (mut policy, operation, _) =
            hashi_fixture_with_kind(false, eth_bridge::requests::AssetKind::Thischain);
        let bridge = crate::EthBridge::bridge_account(crate::GetEthNetworkId::get()).unwrap();
        // Unit-only initial existing mapped position; normal native mint establishes issuance.
        assert_ok!(crate::Assets::mint_unchecked(
            &common::DAI,
            &replacement(),
            10 * ONE
        ));
        let audit = incoming_audit(&policy, &operation, 0);
        let mut keys = request_balance_keys(audit.requests[0].as_ref().unwrap()).to_vec();
        let issuance_key = tokens::TotalIssuance::<Runtime>::hashed_key_for(common::DAI);
        keys.push(issuance_key.clone());
        let balance = balance_audit(keys, core::slice::from_ref(&audit));
        let issuance = crate::Assets::total_issuance(&common::DAI).unwrap();
        let custody = crate::Assets::total_balance(&common::DAI, &bridge).unwrap();
        prepare_policy(
            &mut policy,
            core::slice::from_ref(&operation),
            core::slice::from_ref(&audit),
            &balance,
        );
        // HASHI fixture signatures use the checkpoint+1 birth and checkpoint+2 timepoint.
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 2);
        assert_ok!(native(&policy, &operation));
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 3);
        assert_ok!(super::super::audit_encoded_with_policy(
            &policy,
            &audit.encode()
        ));
        assert_eq!(
            crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
            11 * ONE
        );
        assert_eq!(
            crate::Assets::total_balance(&common::DAI, &bridge).unwrap(),
            custody - ONE
        );
        assert_eq!(
            crate::Assets::total_issuance(&common::DAI).unwrap(),
            issuance
        );
        assert_ok!(audit_native_balances_with_policy(
            &policy,
            &balance.encode()
        ));
        assert_eq!(
            sp_io::storage::get(&key(HEAD_PREFIX, &issuance_key, None)),
            None,
            "global issuance cannot receive an account journal"
        );
    });
}

#[test]
fn native_balance_audit_exclusion_is_not_a_credit_or_a_pooled_custody_burn() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (mut policy, operation, _) =
            hashi_fixture_with_kind(true, eth_bridge::requests::AssetKind::Thischain);
        let bridge = crate::EthBridge::bridge_account(crate::GetEthNetworkId::get()).unwrap();
        let audit = incoming_audit(&policy, &operation, 0);
        assert_eq!(audit.requests, vec![None]);
        let balance = balance_audit(
            vec![frame_system::Account::<Runtime>::hashed_key_for(
                replacement(),
            )],
            core::slice::from_ref(&audit),
        );
        let view = || {
            (
                crate::Assets::total_issuance(&common::DAI).unwrap(),
                crate::Assets::total_balance(&common::DAI, &bridge).unwrap(),
                crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
                crate::BridgeProxy::locked_assets(
                    GenericNetworkId::EVMLegacy(crate::GetEthNetworkId::get()),
                    common::DAI,
                ),
            )
        };
        let before = view();
        prepare_policy(
            &mut policy,
            core::slice::from_ref(&operation),
            core::slice::from_ref(&audit),
            &balance,
        );
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 2);
        assert_ok!(native(&policy, &operation));
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 3);
        assert_ok!(super::super::audit_encoded_with_policy(
            &policy,
            &audit.encode()
        ));
        assert_eq!(
            view(),
            before,
            "excluded claim cannot consume third-party pooled backing"
        );
        assert!(
            balance.validate().is_err(),
            "excluded request cannot authorize account rebasing"
        );
        assert!(native(&policy, &operation).is_err());
        assert_eq!(view(), before);
        assert_eq!(
            sp_io::storage::get(&key(
                HEAD_PREFIX,
                &frame_system::Account::<Runtime>::hashed_key_for(replacement()),
                None
            )),
            None
        );
    });
}

#[test]
fn native_balance_journal_does_not_authorize_unmapped_or_technical_beneficiaries() {
    for technical in [false, true] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation) = channel_with_existing_balance();
            let recipient = if technical {
                crate::BridgeProxy::bridge_account(SubNetworkId::Liberland.into()).unwrap()
            } else {
                AccountId::new([99; 32])
            };
            let IncomingChannel::Substrate {
                commitment, proof, ..
            } = &mut operation.channel
            else {
                unreachable!()
            };
            let GenericCommitment::Sub(inner) = commitment else {
                unreachable!()
            };
            let mut call = super::super::decode_call(&inner.messages[0].payload).unwrap();
            let BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
                recipient: destination,
                ..
            }) = &mut call
            else {
                unreachable!()
            };
            *destination = GenericAccount::Sora(recipient.clone());
            inner.messages[0].payload = call.encode().try_into().unwrap();
            let digest = bridge_types::types::AuxiliaryDigest {
                logs: vec![bridge_types::types::AuxiliaryDigestItem::Commitment(
                    ThisNetworkId::get(),
                    commitment.hash(),
                )],
            };
            *proof = MultiProof::Multisig(multisig_verifier::Proof {
                digest: digest.clone(),
                proof: (1u8..=4)
                    .map(|seed| {
                        ecdsa::Pair::from_seed(&[seed; 32])
                            .sign_prehashed(Keccak256::hash_of(&digest).as_fixed_bytes())
                    })
                    .collect(),
            });
            let audit = incoming_audit(&policy, &operation, 0);
            let keys = request_balance_keys(audit.requests[0].as_ref().unwrap()).to_vec();
            let balance = balance_audit(keys.clone(), core::slice::from_ref(&audit));
            prepare_policy(
                &mut policy,
                core::slice::from_ref(&operation),
                core::slice::from_ref(&audit),
                &balance,
            );
            consume_and_audit(
                &policy,
                core::slice::from_ref(&operation),
                core::slice::from_ref(&audit),
            );
            assert_eq!(
                crate::Assets::free_balance(&common::XOR, &recipient).unwrap(),
                ONE
            );
            assert_eq!(
                crate::Assets::free_balance(&common::XOR, &replacement()).unwrap(),
                10 * ONE
            );
            for location in keys {
                assert_eq!(
                    sp_io::storage::get(&key(HEAD_PREFIX, &location, None)),
                    None
                );
            }
            assert!(audit_native_balances_with_policy(&policy, &balance.encode()).is_err());
        });
    }
}

#[test]
fn native_balance_audit_revalidates_historical_policy_counter_and_marker() {
    for case in 0..4 {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut original, operation) = channel_with_existing_balance();
            let accepted = incoming_audit(&original, &operation, 0);
            let mut balance = balance_audit(
                request_balance_keys(accepted.requests[0].as_ref().unwrap()).to_vec(),
                core::slice::from_ref(&accepted),
            );
            prepare_policy(
                &mut original,
                core::slice::from_ref(&operation),
                core::slice::from_ref(&accepted),
                &balance,
            );
            consume_and_audit(
                &original,
                core::slice::from_ref(&operation),
                core::slice::from_ref(&accepted),
            );
            balance.baseline.index = 0;
            balance.sources[0].audited_policy = Some(if case == 1 {
                [42; 32]
            } else {
                original.commitment()
            });
            let mut repaired = original.clone();
            repaired.audit_chunk_hashes = vec![balance.commitment()];
            repaired.evidence_commitment = [82; 32];
            // Explicit unit-only context setup. This is NOT council escape or
            // authority evidence; it isolates the journal's prior-policy checks.
            let mut state = recovery::storage::status().unwrap().unwrap();
            assert_eq!(state.policy, original.commitment());
            state.operation_counter = if case == 2 { 0 } else { 1 };
            state.phase = RecoveryPhase::Auditing;
            state.next_chunk = repaired.chunk_hashes.len() as u32;
            sp_io::storage::set(STATUS_STORAGE_KEY, &state.encode());
            sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &repaired.commitment());
            sp_io::storage::set(recovery::AUDIT_CURSOR_STORAGE_KEY, &0u32.encode());
            if case == 3 {
                let marker = audit::audit_marker_key(original.commitment(), operation.commitment());
                let mut value: ([u8; 32], [u8; 32], [u8; 32], u64) =
                    decode_bounded(&sp_io::storage::get(&marker).unwrap()).unwrap();
                value.2 = [43; 32];
                sp_io::storage::set(&marker, &value.encode());
            }
            let result = audit_native_balances_with_policy(&repaired, &balance.encode());
            if case == 0 {
                assert_ok!(result);
            } else {
                assert!(result.is_err());
            }
            assert_eq!(System::account(replacement()).data.free, 11 * ONE);
            assert_eq!(
                sp_io::storage::get(POLICY_STORAGE_KEY).unwrap().to_vec(),
                original.commitment().to_vec()
            );
        });
    }
}

#[test]
fn native_balance_audit_rejects_missing_tampered_truncated_or_miscounted_journal() {
    for case in 0..5 {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, operation) = channel_with_existing_balance();
            let accepted = incoming_audit(&policy, &operation, 0);
            let balance = balance_audit(
                request_balance_keys(accepted.requests[0].as_ref().unwrap()).to_vec(),
                core::slice::from_ref(&accepted),
            );
            prepare_policy(
                &mut policy,
                core::slice::from_ref(&operation),
                core::slice::from_ref(&accepted),
                &balance,
            );
            consume_and_audit(
                &policy,
                core::slice::from_ref(&operation),
                core::slice::from_ref(&accepted),
            );
            let location = frame_system::Account::<Runtime>::hashed_key_for(replacement());
            let link_key = key(
                LINK_PREFIX,
                &location,
                Some(identity(policy.commitment(), operation.commitment())),
            );
            let raw = sp_io::storage::get(&link_key).unwrap().to_vec();
            match case {
                0 => sp_io::storage::clear(&link_key),
                1 => {
                    let mut link: Link = decode_bounded(&raw).unwrap();
                    link.after = Some([42; 32]);
                    sp_io::storage::set(&link_key, &link.encode());
                }
                2 => sp_io::storage::set(&link_key, &raw[..raw.len() - 1]),
                3 => {
                    let head_key = key(HEAD_PREFIX, &location, None);
                    let mut head: Head =
                        decode_bounded(&sp_io::storage::get(&head_key).unwrap()).unwrap();
                    head.count += 1;
                    sp_io::storage::set(&head_key, &head.encode());
                }
                _ => {
                    let mut link: Link = decode_bounded(&raw).unwrap();
                    link.counter += 1;
                    sp_io::storage::set(&link_key, &link.encode());
                }
            }
            assert!(audit_native_balances_with_policy(&policy, &balance.encode()).is_err());
            assert_eq!(System::account(replacement()).data.free, 11 * ONE);
        });
    }
}

#[test]
fn native_balance_audit_cannot_clear_unaccepted_incoming_or_pending_counter() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (mut policy, operation) = channel_with_existing_balance();
        let accepted = incoming_audit(&policy, &operation, 0);
        let balance = balance_audit(
            request_balance_keys(accepted.requests[0].as_ref().unwrap()).to_vec(),
            core::slice::from_ref(&accepted),
        );
        prepare_policy(
            &mut policy,
            core::slice::from_ref(&operation),
            core::slice::from_ref(&accepted),
            &balance,
        );
        assert_ok!(native(&policy, &operation));
        let before = sp_io::storage::root(sp_core::storage::StateVersion::V0);
        assert!(audit_native_balances_with_policy(&policy, &balance.encode()).is_err());
        assert_eq!(
            sp_io::storage::root(sp_core::storage::StateVersion::V0),
            before
        );
        assert_eq!(
            sp_io::storage::get(recovery::INCOMING_AUDIT_PENDING_KEY)
                .unwrap()
                .to_vec(),
            1u32.encode()
        );
        assert_ok!(super::super::audit_encoded_with_policy(
            &policy,
            &accepted.encode()
        ));
        assert_ok!(audit_native_balances_with_policy(
            &policy,
            &balance.encode()
        ));
    });
}
