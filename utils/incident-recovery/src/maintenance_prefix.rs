//! A distinct, bounded first-child witness. It never certifies final activation.

use super::{
    recovery, replay, BlockWitness, Manifest, RecoveryInherent, RecoveryPhase, RecoveryPolicy,
    RecoveryStatus, Result,
};
use std::path::Path;

pub(super) const SCHEMA: &str = "sora-nexus-maintenance-prefix-v1";
pub(super) const REPORT_SCHEMA: &str = "sora-nexus-maintenance-prefix-witness-v1";

pub(super) struct Plan {
    cursor: u32,
    phase: RecoveryPhase,
}

impl Plan {
    fn shape(manifest: &Manifest) -> Result<()> {
        if manifest.schema != SCHEMA
            || manifest.steps.len() != 1
            || !manifest.repairs.is_empty()
            || manifest.fixture_council.is_some()
            || manifest.supplied_council.is_some()
            || manifest.public_dynamic_repair.is_some()
            || manifest.fixture_hashi_peers.is_some()
            || manifest.fixture_expectations.is_some()
            || manifest.native_operation_recorder.is_some()
            || !manifest.native_operation_recorders_v2.is_empty()
            || manifest.fixture_signed_transactions.is_some()
        {
            return Err(
                "prefix requires exactly one first child without signing or execution overrides"
                    .into(),
            );
        }
        let step = &manifest.steps[0];
        if step.recovery.is_none()
            || step.fixture_escape.is_some()
            || step.fixture_hashi
            || step.reject_previous_escapes
            || !step.rejected_recovery.is_empty()
            || step.expected_spec != 134
        {
            return Err("prefix requires one ordinary maintenance inherent at spec134".into());
        }
        Ok(())
    }

    pub(super) fn load(
        manifest: &Manifest,
        base: &Path,
        policy: &RecoveryPolicy,
    ) -> Result<Option<Self>> {
        if manifest.schema != SCHEMA {
            return Ok(None);
        }
        Self::shape(manifest)?;
        let raw = replay::artifact(
            base,
            manifest.steps[0]
                .recovery
                .as_ref()
                .ok_or("prefix inherent absent")?,
            recovery::MAX_INHERENT_BYTES,
        )?;
        let envelope = RecoveryInherent::decode_remark(&raw)
            .map_err(|error| format!("invalid prefix inherent: {error:?}"))?;
        Self::from_action(manifest, policy, &envelope.action).map(Some)
    }

    fn from_action(
        manifest: &Manifest,
        policy: &RecoveryPolicy,
        action: &recovery::RecoveryAction,
    ) -> Result<Self> {
        Self::shape(manifest)?;
        let (cursor, phase) = match action {
            recovery::RecoveryAction::Tick => (0, RecoveryPhase::Applying),
            recovery::RecoveryAction::Apply(chunk)
                if chunk.index == 0 && policy.chunk_hashes.first() == Some(&chunk.commitment()) =>
            {
                (
                    1,
                    if policy.chunk_hashes.len() == 1 {
                        RecoveryPhase::Auditing
                    } else {
                        RecoveryPhase::Applying
                    },
                )
            }
            _ => return Err("prefix accepts only Tick or the committed first Apply chunk".into()),
        };
        if manifest.steps[0].expected_phase != format!("{phase:?}") {
            return Err("prefix expected phase differs from its first committed operation".into());
        }
        Ok(Self { cursor, phase })
    }

    pub(super) fn verify_terminal(
        &self,
        status: &RecoveryStatus,
        policy: &RecoveryPolicy,
        witnesses: &[BlockWitness],
        original_code_retained: bool,
    ) -> Result<()> {
        let [row] = witnesses else {
            return Err("prefix must retain exactly one executed first-child witness".into());
        };
        if !original_code_retained
            || status.format_version != 1
            || status.policy != policy.commitment()
            || status.phase != self.phase
            || status.next_chunk != self.cursor
            || status.operation_counter != 0
            || status.failure.is_some()
            || row.number != recovery::CHECKPOINT_NUMBER + 1
            || row.resulting_spec != 134
            || row.phase != format!("{:?}", self.phase)
            || row.cursor != self.cursor
            || row.operation_counter != 0
            || row.generated_authorization_sha256.is_some()
            || !row.maintenance_session_frozen
            || !row.independent_import_root_matches
            || !row.disk_restart_root_matches
        {
            return Err(
                "first-child prefix terminal differs from its bounded maintenance scope".into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Manifest, RecoveryPolicy, recovery::RecoveryAction) {
        let chunk = recovery::RecoveryChunk {
            index: 0,
            edits: vec![recovery::GuardedStorageEdit {
                scope: recovery::StorageScope::Top,
                key: b"prefix-fixture".to_vec(),
                before: None,
                after: Some(vec![1]),
            }],
        };
        let policy = RecoveryPolicy {
            format_version: 2,
            authorization_domain: recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![sp_core::crypto::AccountId32::new([1; 32])],
            chunk_hashes: vec![chunk.commitment(), [2; 32]],
            audit_chunk_hashes: vec![[3; 32]],
            evidence_commitment: [4; 32],
            terminal_audit_commitment: [5; 32],
        };
        let artifact = serde_json::json!({"file":"unused", "sha256":"00".repeat(32)});
        let manifest = serde_json::from_value(serde_json::json!({
            "schema":SCHEMA, "checkpointHash":format!("0x{}",hex::encode(recovery::CHECKPOINT_HASH)),
            "checkpointCapture":"unused", "policy":artifact, "bootstrap":artifact,
            "hold":artifact, "finalRuntime":artifact, "previousGrandpaSet":1,
            "freshGrandpa":[[format!("0x{}","01".repeat(32)),1]],
            "steps":[{"slot":1,"timestamp":6000,"authorityIndex":0,"recovery":artifact,
                "expectedSpec":134,"expectedPhase":"Applying"}]
        })).unwrap();
        (manifest, policy, recovery::RecoveryAction::Apply(chunk))
    }

    #[test]
    fn prefix_only_accepts_a_committed_first_apply_or_tick() {
        let (mut manifest, policy, mut action) = fixture();
        assert_eq!(
            Plan::from_action(&manifest, &policy, &action)
                .unwrap()
                .cursor,
            1
        );
        assert_eq!(
            Plan::from_action(&manifest, &policy, &recovery::RecoveryAction::Tick)
                .unwrap()
                .cursor,
            0
        );
        if let recovery::RecoveryAction::Apply(chunk) = &mut action {
            chunk.index = 1;
        }
        assert!(Plan::from_action(&manifest, &policy, &action).is_err());
        let unrelated = recovery::RecoveryAction::StageCode {
            kind: recovery::RecoveryCodeKind::Final,
            index: 0,
            bytes: vec![1],
        };
        assert!(Plan::from_action(&manifest, &policy, &unrelated).is_err());
        manifest.steps[0].expected_phase = "Complete".into();
        assert!(Plan::from_action(&manifest, &policy, &recovery::RecoveryAction::Tick).is_err());
    }

    #[test]
    fn ordinary_schema_and_extra_execution_are_never_prefixes() {
        let (mut manifest, policy, action) = fixture();
        manifest.schema = "sora-nexus-maintenance-replay-v1".into();
        assert!(Plan::load(&manifest, Path::new("/unused"), &policy)
            .unwrap()
            .is_none());
        assert!(Plan::from_action(&manifest, &policy, &action).is_err());
        manifest.schema = SCHEMA.into();
        manifest.steps[0].reject_previous_escapes = true;
        assert!(Plan::from_action(&manifest, &policy, &action).is_err());
        manifest.steps[0].reject_previous_escapes = false;
        manifest.repairs.push(replay::Artifact {
            file: "unused".into(),
            sha256: "00".repeat(32),
        });
        assert!(Plan::from_action(&manifest, &policy, &action).is_err());
        manifest.repairs.clear();
        manifest.steps.clear();
        assert!(Plan::from_action(&manifest, &policy, &action).is_err());
    }

    #[test]
    fn prefix_terminal_requires_original_policy_code_and_fresh_imported_first_child() {
        let (manifest, policy, action) = fixture();
        let plan = Plan::from_action(&manifest, &policy, &action).unwrap();
        let mut status = RecoveryStatus {
            format_version: 1,
            policy: policy.commitment(),
            phase: RecoveryPhase::Applying,
            next_chunk: 1,
            operation_counter: 0,
            failure: None,
        };
        let mut rows = vec![BlockWitness {
            number: recovery::CHECKPOINT_NUMBER + 1,
            hash: String::new(),
            state_root: String::new(),
            executing_code_blake2: String::new(),
            resulting_spec: 134,
            phase: "Applying".into(),
            cursor: 1,
            operation_counter: 0,
            block_sha256: String::new(),
            disk_snapshot_manifest_sha256: String::new(),
            forbidden_extrinsics_rejected: 2,
            maintenance_session_frozen: true,
            independent_import_root_matches: true,
            disk_restart_root_matches: true,
            generated_authorization_sha256: None,
        }];
        plan.verify_terminal(&status, &policy, &rows, true).unwrap();
        assert!(plan
            .verify_terminal(&status, &policy, &rows, false)
            .is_err());
        status.policy = [9; 32];
        assert!(plan.verify_terminal(&status, &policy, &rows, true).is_err());
        status.policy = policy.commitment();
        rows[0].number += 1;
        assert!(plan.verify_terminal(&status, &policy, &rows, true).is_err());
        rows[0].number -= 1;
        rows[0].disk_restart_root_matches = false;
        assert!(plan.verify_terminal(&status, &policy, &rows, true).is_err());
        rows[0].disk_restart_root_matches = true;
        status.operation_counter = 1;
        assert!(plan.verify_terminal(&status, &policy, &rows, true).is_err());
    }
}
