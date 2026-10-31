use super::*;
use serde_json::json;
use sp_runtime::{AccountId32, MultiSignature};

fn plan() -> ExecutionPlan {
    ExecutionPlan {
        schema: "sora-nexus-hashi-execution-plan-v1".into(),
        template_commitment: H256::repeat_byte(1),
        network: 0,
        first_execution_block: recovery::CHECKPOINT_NUMBER + 5,
        last_execution_block: recovery::CHECKPOINT_NUMBER + 10,
        spec_version: 134,
        transaction_version: 131,
        execution_code_hash: H256::repeat_byte(2),
        metadata_hash: H256::repeat_byte(3),
    }
}

fn operation() -> IncomingOperation {
    let birth = u64::from(recovery::CHECKPOINT_NUMBER + 3);
    IncomingOperation {
        index: 0,
        channel: IncomingChannel::Hashi {
            network: 0,
            request: vec![1, 2], // Synthetic bytes; native API admission is separate.
            approvals: vec![HashiPeerApproval {
                signer: AccountId32::new([7; 32]),
                call: vec![5, 6],
                extra: (Era::mortal(64, birth), Compact(3u32), Compact(0u128)).encode(),
                signature: MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0; 64])),
            }],
        },
        registries: vec![],
        finalized_source_evidence: [4; 32],
    }
}

fn policy(operation: &IncomingOperation) -> RecoveryPolicy {
    RecoveryPolicy {
        format_version: 2,
        checkpoint_hash: recovery::CHECKPOINT_HASH,
        checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
        replacements: vec![],
        council: vec![AccountId32::new([8; 32])],
        chunk_hashes: vec![operation.commitment()],
        audit_chunk_hashes: vec![[9; 32]],
        evidence_commitment: [10; 32],
        terminal_audit_commitment: [11; 32],
        authorization_domain: recovery::RecoveryDomain {
            chain_spec_id: framenode_chain_spec::recovery::CHAIN_ID.as_bytes().to_vec(),
            release_nonce: [12; 32],
        },
    }
}

fn config(policy: &RecoveryPolicy, bootstrap: &[u8]) -> RecoveryConfig {
    let key: sc_finality_grandpa::AuthorityId = sp_core::ed25519::Public::from_raw([1; 32]).into();
    RecoveryConfig {
        schema_version: 3,
        checkpoint_hash: recovery::CHECKPOINT_HASH.into(),
        policy_commitment: policy.commitment().into(),
        policy_scale: sp_core::Bytes(policy.encode()),
        bootstrap_code_hash: sp_io::hashing::blake2_256(bootstrap).into(),
        final_code_hash: [3; 32].into(),
        hold_code_hash: [4; 32].into(),
        evidence_commitment: policy.evidence_commitment.into(),
        previous_grandpa_set_id: framenode_chain_spec::recovery::PREVIOUS_GRANDPA_SET_ID,
        previous_grandpa_authorities: vec![(key.clone(), 1)],
        grandpa_authorities: vec![(key.clone(), 1)],
        approved_grandpa_keys: vec![key],
        last_observed_abandoned_slot: framenode_chain_spec::recovery::LAST_ABANDONED_SLOT,
        first_recovery_slot: framenode_chain_spec::recovery::LAST_ABANDONED_SLOT + 1,
    }
}

struct Fixture {
    config: RecoveryConfig,
    manifest: Vec<u8>,
    plan: Vec<u8>,
    bootstrap: Vec<u8>,
    metadata: Vec<u8>,
}

impl Fixture {
    fn new() -> Self {
        let operation = operation();
        let policy = policy(&operation);
        let bootstrap = b"synthetic test-only code; never an executable release".to_vec();
        let metadata = b"synthetic metadata".to_vec();
        let config = config(&policy, &bootstrap);
        let p = plan();
        let plan = serde_json::to_vec(&json!({
            "schema":p.schema,"templateCommitment":H256::from(operation.commitment()),
            "network":p.network,"firstExecutionBlock":p.first_execution_block,
            "lastExecutionBlock":p.last_execution_block,"specVersion":134,"transactionVersion":131,
            "executionCodeHash":config.bootstrap_code_hash,
            "metadataHash":H256::from(sp_io::hashing::blake2_256(&metadata)),
        }))
        .unwrap();
        let artifacts: Vec<_> = [
            ("policyScale", config.policy_scale.0.as_slice()),
            ("bootstrapWasm", bootstrap.as_slice()),
            ("hashiPlan0", plan.as_slice()),
            ("bootstrapMetadata", metadata.as_slice()),
        ]
        .into_iter()
        .map(|(role, bytes)| {
            json!({
                "role":role,"bytes":bytes.len(),"sha256":hex(&sp_io::hashing::sha2_256(bytes)),
                "blake2_256":hex(&sp_io::hashing::blake2_256(bytes)),
            })
        })
        .collect();
        let manifest = serde_json::to_vec(&json!({
            "format":"sora-nexus-recovery-release-v1",
            "checkpoint":{"number":recovery::CHECKPOINT_NUMBER,"hash":H256::from(recovery::CHECKPOINT_HASH)},
            "policy":{"commitment":hex(config.policy_commitment.as_bytes()),"evidenceCommitment":hex(config.evidence_commitment.as_bytes())},
            "artifacts":artifacts,
        })).unwrap();
        Self {
            config,
            manifest,
            plan,
            bootstrap,
            metadata,
        }
    }

    fn artifacts(&self) -> ReleaseArtifacts<'_> {
        ReleaseArtifacts {
            expected_manifest_sha256: sp_io::hashing::sha2_256(&self.manifest),
            manifest: &self.manifest,
            plan: &self.plan,
            plan_role: "hashiPlan0",
            bootstrap: &self.bootstrap,
            metadata: &self.metadata,
            metadata_role: "bootstrapMetadata",
        }
    }
}

#[test]
fn release_requires_independent_pin_and_exact_artifact_bytes() {
    let fixture = Fixture::new();
    BoundRelease::new(&fixture.config, fixture.artifacts()).unwrap();
    let mut artifacts = fixture.artifacts();
    artifacts.expected_manifest_sha256 = [0; 32];
    assert!(BoundRelease::new(&fixture.config, artifacts).is_err());
    let mut artifacts = fixture.artifacts();
    artifacts.expected_manifest_sha256 = [1; 32];
    assert!(BoundRelease::new(&fixture.config, artifacts).is_err());
    let mut artifacts = fixture.artifacts();
    artifacts.metadata = b"same spec version, different metadata";
    assert!(BoundRelease::new(&fixture.config, artifacts).is_err());
    let mut artifacts = fixture.artifacts();
    artifacts.bootstrap = b"stored code is not the substitute";
    assert!(BoundRelease::new(&fixture.config, artifacts).is_err());
    let mut artifacts = fixture.artifacts();
    artifacts.plan_role = "bootstrapWasm";
    assert!(BoundRelease::new(&fixture.config, artifacts).is_err());
}

#[test]
fn release_rejects_duplicate_roles_and_changed_policy() {
    for repair in [false, true] {
        let mut f = Fixture::new();
        let mut manifest: serde_json::Value = serde_json::from_slice(&f.manifest).unwrap();
        if repair {
            manifest["policy"]["commitment"] = json!("00".repeat(32));
        } else {
            let row = manifest["artifacts"][0].clone();
            manifest["artifacts"].as_array_mut().unwrap().push(row);
        }
        f.manifest = serde_json::to_vec(&manifest).unwrap();
        assert!(BoundRelease::new(&f.config, f.artifacts()).is_err());
    }
}

#[test]
fn operation_witnesses_may_change_but_business_fields_must_not() {
    let f = Fixture::new();
    let release = BoundRelease::new(&f.config, f.artifacts()).unwrap();
    let mut op = operation();
    release.operation(&op.encode()).unwrap();
    if let IncomingChannel::Hashi { approvals, .. } = &mut op.channel {
        approvals[0].signature =
            MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([1; 64]));
    }
    release.operation(&op.encode()).unwrap();
    op.finalized_source_evidence = [3; 32];
    assert!(release.operation(&op.encode()).is_err());
    let mut bytes = operation().encode();
    bytes.push(0);
    assert!(release.operation(&bytes).is_err());
}

#[test]
fn absolute_plan_window_prevents_mortal_cycle_rebasing() {
    let op = operation();
    let approval = &hashi(&op).unwrap().1[0];
    let mut p = plan();
    let original = approval_window(approval, &p).unwrap();
    p.last_execution_block += 64;
    assert!(approval_window(approval, &p).is_err());
    // The same encoded era is otherwise valid a cycle later, but a pinned plan
    // cannot change either absolute endpoint without changing its artifact hash.
    p.first_execution_block += 64;
    let rebased = approval_window(approval, &p).unwrap();
    assert_eq!(rebased.1, original.1 + 64);
}

#[test]
fn empty_capabilities_are_created_for_each_disposable_call() {
    for method in [Method::Metadata, Method::Prepare, Method::Admit] {
        let result = disposable_call(method, |changes, extensions| {
            assert!(changes.borrow().is_empty());
            assert!(extensions
                .borrow_mut()
                .get_mut(std::any::TypeId::of::<sp_keystore::KeystoreExt>())
                .is_none());
            Ok(vec![1])
        })
        .unwrap();
        assert_eq!(result, [1]);
    }
}

#[test]
fn storage_offchain_and_open_transactions_fail_without_a_success_result() {
    for kind in 0..3 {
        let result = disposable_call(Method::Prepare, |changes, _| {
            let mut changes = changes.borrow_mut();
            match kind {
                0 => changes.set_storage(b"unexpected".to_vec(), Some(vec![1])),
                1 => changes.set_offchain_storage(b"unexpected", Some(&[1])),
                _ => changes.start_transaction(),
            }
            Ok(vec![1])
        });
        assert!(result.is_err());
    }
    assert!(disposable_call(Method::Admit, |_, _| Err("native failure".into())).is_err());
    assert!(disposable_call(Method::Prepare, |_, _| Ok(vec![0; MAX_RESPONSE + 1])).is_err());
}

#[test]
fn runtime_preparation_cannot_change_parent_code_claim_or_signature_context() {
    let op = operation();
    let f = Fixture::new();
    let release = BoundRelease::new(&f.config, f.artifacts()).unwrap();
    let parent = Parent {
        header: Header::new(
            plan().first_execution_block - 1,
            H256::repeat_byte(1),
            H256::repeat_byte(2),
            H256::repeat_byte(3),
            Default::default(),
        ),
        anchor: [8; 32],
        stored_code_hash: [9; 32],
        status: RecoveryStatus {
            format_version: 1,
            policy: release.policy.commitment(),
            phase: RecoveryPhase::Applying,
            next_chunk: 0,
            operation_counter: 0,
            failure: None,
        },
    };
    let approval = &hashi(&op).unwrap().1[0];
    let (nonce, birth, death) = approval_window(approval, &release.plan).unwrap();
    let valid = RecoveryHashiPreparation {
        format_version: 1,
        parent_hash: parent.header.hash().0,
        parent_state_root: parent.header.state_root().0,
        execution_number: plan().first_execution_block,
        policy_commitment: release.policy.commitment(),
        recovery_anchor: parent.anchor,
        operation_index: 0,
        operation_commitment: op.commitment(),
        network: 0,
        spec_version: 134,
        transaction_version: 131,
        stored_code_hash: parent.stored_code_hash,
        source_evidence_commitment: op.finalized_source_evidence,
        external_finality_verified: false,
        approvals: vec![eth_bridge_runtime_api::recovery::RecoveryHashiPayload {
            signer: *approval.signer.as_ref(),
            nonce,
            birth_number: birth,
            birth_hash: [10; 32],
            death_number: death,
            call_hash: sp_io::hashing::blake2_256(&approval.call),
            extra_hash: sp_io::hashing::blake2_256(&approval.extra),
            payload: vec![1],
        }],
    };
    validate_preparation(&release, &parent, &op, &valid).unwrap();
    for field in 0..6 {
        let mut changed = valid.clone();
        match field {
            0 => changed.parent_hash = [0; 32],
            1 => changed.stored_code_hash = release.plan.execution_code_hash.0,
            2 => changed.external_finality_verified = true,
            3 => changed.approvals[0].nonce += 1,
            4 => changed.approvals[0].birth_number += 64,
            _ => changed.approvals[0].extra_hash = [0; 32],
        }
        assert!(validate_preparation(&release, &parent, &op, &changed).is_err());
    }
}

#[test]
fn original_policy_can_carry_incoming_replay_evidence() {
    let mut f = Fixture::new();
    let mut manifest: serde_json::Value = serde_json::from_slice(&f.manifest).unwrap();
    manifest["incomingRecoveryReplay"] = json!({"evidenceOnly":true});
    f.manifest = serde_json::to_vec(&manifest).unwrap();
    BoundRelease::new(&f.config, f.artifacts()).unwrap();
}
