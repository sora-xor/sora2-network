//! Strict public-only fixture: preserve an accepted 136 audit through repair137.
//! No runtime behavior or production admission is modified by this host adapter.
use super::supplied_escape::Scenario;
use crate::Result;
use common::recovery::{
    GuardedStorageEdit, RecoveryAction, RecoveryAuditChunk, RecoveryChunk, RecoveryCodeKind,
    RecoveryPhase, RecoveryPolicy, RecoveryStatus, StorageScope,
};

pub const SCHEMA: &str = "sora-nexus-public-fixture-historical-audit-third-repair-v1";
pub const REPORT_SCHEMA: &str =
    "sora-nexus-public-fixture-historical-audit-third-repair-witness-v1";
const PROBE_KEY: &[u8] = b"sora-nexus/PUBLIC-FIXTURE-ONLY/historical-audit-third-repair/v1/probe";
const PROBE_VALUE: &[u8] = b"temporary-nonfinancial-repair137-probe";

fn probe(index: u32) -> RecoveryChunk {
    RecoveryChunk {
        index,
        edits: vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: PROBE_KEY.to_vec(),
            before: (index == 1).then(|| sp_io::hashing::blake2_256(PROBE_VALUE)),
            after: (index == 0).then(|| PROBE_VALUE.to_vec()),
        }],
    }
}

pub struct Track {
    original: RecoveryPolicy,
    static_audit: Option<RecoveryAuditChunk>,
    audited_policy: Option<[u8; 32]>,
    final_policy: Option<[u8; 32]>,
    probe_cursor: u32,
    audited137: bool,
}
impl Track {
    pub fn new(
        scenario: &Scenario,
        original: &RecoveryPolicy,
        repairs: usize,
        lanes: usize,
    ) -> Result<Self> {
        let terminal = &scenario.expected_terminal;
        if scenario
            .escapes
            .iter()
            .map(|e| e.destination_spec)
            .collect::<Vec<_>>()
            != [135, 136, 137]
            || repairs != 2
            || lanes != 1
            || terminal.spec_version != 138
            || terminal.phase != "Complete"
            || terminal.operation_counter != 3
            || terminal.cursor != 2
            || terminal.audit_cursor != 1
            || terminal.ordinary_blocks != 3
        {
            return Err(
                "historical audit fixture requires exact 135/136/137 sequence and final counter3"
                    .into(),
            );
        }
        Ok(Self {
            original: original.clone(),
            static_audit: None,
            audited_policy: None,
            final_policy: None,
            probe_cursor: 0,
            audited137: false,
        })
    }

    pub fn observe(
        &mut self,
        spec: u32,
        action: &RecoveryAction,
        state: &RecoveryStatus,
        policy: &RecoveryPolicy,
        pending: Option<Vec<u8>>,
        audit_cursor: Option<Vec<u8>>,
    ) -> Result<()> {
        if spec == 136 {
            match action {
                RecoveryAction::Audit(chunk) => {
                    if chunk.index != 0
                        || chunk.witnesses.len() != 22
                        || self.static_audit.is_some()
                    {
                        return Err(
                            "historical fixture requires one exact 22-witness static audit136"
                                .into(),
                        );
                    }
                    self.static_audit = Some(chunk.clone());
                }
                RecoveryAction::IncomingAudit(bytes) => {
                    if self.static_audit.is_none()
                        || self.audited_policy.is_some()
                        || bytes.get(..4) != Some(1u32.to_le_bytes().as_slice())
                        || state.phase != RecoveryPhase::Auditing
                        || state.operation_counter != 2
                        || audit_cursor.as_deref() != Some(1u32.to_le_bytes().as_slice())
                        || pending.as_deref() != Some(1u32.to_le_bytes().as_slice())
                    {
                        return Err("historical fixture requires one typed136 audit of the pending original incoming".into());
                    }
                    self.audited_policy = Some(policy.commitment());
                }
                RecoveryAction::StageCode {
                    kind: RecoveryCodeKind::Repair,
                    ..
                }
                | RecoveryAction::Escape { .. } => {
                    if self.audited_policy != Some(policy.commitment())
                        || state.phase != RecoveryPhase::Auditing
                        || state.next_chunk != 2
                        || state.operation_counter != 2
                        || pending.as_deref() != Some(0u32.to_le_bytes().as_slice())
                        || audit_cursor.as_deref() != Some(2u32.to_le_bytes().as_slice())
                    {
                        return Err("third repair must follow the completed136 typed audit with zero pending".into());
                    }
                    if let RecoveryAction::Escape {
                        authorization,
                        work_policy,
                        ..
                    } = action
                    {
                        let revised = work_policy.as_ref().ok_or("third repair policy absent")?;
                        let audit = self.static_audit.as_ref().ok_or("static audit absent")?;
                        if self.final_policy.is_some()
                            || authorization.destination_spec != 137
                            || revised.replacements != self.original.replacements
                            || revised.council != self.original.council
                            || revised.chunk_hashes
                                != vec![probe(0).commitment(), probe(1).commitment()]
                            || revised.audit_chunk_hashes != vec![audit.commitment()]
                            || revised.terminal_audit_commitment != policy.terminal_audit_commitment
                            || revised.commitment() == policy.commitment()
                        {
                            return Err("third repair must preserve owners and use only the exact temporary probe".into());
                        }
                        self.final_policy = Some(revised.commitment());
                    }
                }
                _ => {}
            }
        } else if spec == 137 {
            if self.final_policy != Some(policy.commitment())
                || state.operation_counter != 3
                || pending.as_deref() != Some(0u32.to_le_bytes().as_slice())
            {
                return Err(
                    "repair137 changed its admitted policy, counter or completed incoming state"
                        .into(),
                );
            }
            match action {
                RecoveryAction::Tick => {}
                RecoveryAction::Apply(chunk)
                    if self.probe_cursor < 2 && *chunk == probe(self.probe_cursor) =>
                {
                    self.probe_cursor += 1;
                }
                RecoveryAction::Audit(chunk)
                    if self.probe_cursor == 2
                        && !self.audited137
                        && self.static_audit.as_ref() == Some(chunk) =>
                {
                    self.audited137 = true
                }
                RecoveryAction::StageCode {
                    kind: RecoveryCodeKind::Final,
                    ..
                }
                | RecoveryAction::TerminalAudit(_)
                    if self.probe_cursor == 2 && self.audited137 => {}
                _ => return Err(
                    "repair137 fixture forbids financial work or changes to preserved witnesses"
                        .into(),
                ),
            }
        }
        Ok(())
    }

    pub fn finish(
        self,
        state: &RecoveryStatus,
        pending: Option<Vec<u8>>,
        probe_value: Option<Vec<u8>>,
        reports: &[serde_json::Value],
    ) -> Result<()> {
        let Some(audited) = self.audited_policy else {
            return Err("accepted audit136 absent".into());
        };
        let Some(active) = self.final_policy else {
            return Err("accepted repair137 absent".into());
        };
        if !self.audited137
            || self.probe_cursor != 2
            || state.phase != RecoveryPhase::Complete
            || state.operation_counter != 3
            || state.next_chunk != 2
            || probe_value.is_some()
            || pending.as_deref() != Some(0u32.to_le_bytes().as_slice())
            || reports.len() != 1
        {
            return Err(
                "historical audit fixture did not complete its nonfinancial repair and activation"
                    .into(),
            );
        }
        let proof = &reports[0];
        let final_check = &proof["finalValidation"];
        if proof["activePolicy"] != hex::encode(audited)
            || proof["consumedPolicy"] != hex::encode(self.original.commitment())
            || final_check["activePolicy"] != hex::encode(active)
            || final_check["auditedPolicy"] != hex::encode(audited)
            || final_check["auditedCounter"] != 2
            || final_check["consumptionCounter"] != 0
            || final_check["block"]["operationCounter"] != 3
            || final_check["pendingIncomingAudits"] != 0
            || final_check["readOnlyStateRootMatches"] != true
        {
            return Err("historical136 audit lacks final137 policy revalidation from the opaque certificate".into());
        }
        Ok(())
    }
    pub fn probe_key() -> &'static [u8] {
        PROBE_KEY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Track, RecoveryPolicy, RecoveryPolicy, RecoveryStatus) {
        let audit = RecoveryAuditChunk {
            index: 0,
            witnesses: (0..22)
                .map(|i| common::recovery::StorageWitness {
                    scope: StorageScope::Top,
                    key: vec![i],
                    value_hash: None,
                })
                .collect(),
        };
        let original = RecoveryPolicy {
            format_version: 1,
            checkpoint_hash: common::recovery::CHECKPOINT_HASH,
            checkpoint_state_root: common::recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![sp_runtime::AccountId32::new([8; 32])],
            chunk_hashes: vec![[1; 32]],
            audit_chunk_hashes: vec![[2; 32]],
            evidence_commitment: [3; 32],
            terminal_audit_commitment: [4; 32],
        };
        let mut revised = original.clone();
        revised.chunk_hashes = vec![probe(0).commitment(), probe(1).commitment()];
        revised.audit_chunk_hashes = vec![audit.commitment()];
        revised.evidence_commitment = [5; 32];
        let state = RecoveryStatus {
            format_version: 1,
            policy: original.commitment(),
            phase: RecoveryPhase::Auditing,
            next_chunk: 2,
            operation_counter: 2,
            failure: None,
        };
        let track = Track {
            original: original.clone(),
            static_audit: Some(audit),
            audited_policy: Some(original.commitment()),
            final_policy: None,
            probe_cursor: 0,
            audited137: false,
        };
        (track, original, revised, state)
    }
    fn count(n: u32) -> Option<Vec<u8>> {
        Some(n.to_le_bytes().to_vec())
    }
    fn escape(original: &RecoveryPolicy, revised: &RecoveryPolicy) -> RecoveryAction {
        RecoveryAction::Escape {
            authorization: Box::new(common::recovery::RepairAuthorization {
                policy: original.commitment(),
                phase: RecoveryPhase::Auditing,
                cursor: 2,
                operation_counter: 2,
                current_code_hash: [6; 32],
                current_executor_hash: [6; 32],
                destination_code_hash: [7; 32],
                destination_spec: 137,
                revised_policy: revised.commitment(),
            }),
            signatures: vec![],
            work_policy: Some(revised.clone()),
        }
    }
    #[test]
    fn third_escape_rejects_unfinished_audit_pending_or_wrong_counter() {
        for mode in 0..4 {
            let (mut track, original, revised, mut state) = fixture();
            let mut pending = count(0);
            match mode {
                0 => track.audited_policy = None,
                1 => pending = count(1),
                2 => state.operation_counter = 1,
                _ => state.phase = RecoveryPhase::Applying,
            }
            assert!(track
                .observe(
                    136,
                    &escape(&original, &revised),
                    &state,
                    &original,
                    pending,
                    count(2)
                )
                .is_err());
        }
        let (mut track, original, revised, state) = fixture();
        track
            .observe(
                136,
                &escape(&original, &revised),
                &state,
                &original,
                count(0),
                count(2),
            )
            .unwrap();
        assert_eq!(track.final_policy, Some(revised.commitment()));
    }
    #[test]
    fn repair137_rejects_financial_work_wrong_probe_and_changed_policy() {
        for mode in 0..5 {
            let (mut track, _, mut revised, mut state) = fixture();
            track.final_policy = Some(revised.commitment());
            state.operation_counter = 3;
            let mut wrong = probe(0);
            wrong.edits[0].key.push(9);
            let action = match mode {
                0 => RecoveryAction::Incoming(vec![1]),
                1 => RecoveryAction::Outgoing(vec![1]),
                2 => RecoveryAction::Apply(wrong),
                3 => {
                    revised.evidence_commitment = [9; 32];
                    RecoveryAction::Tick
                }
                _ => {
                    state.operation_counter = 2;
                    RecoveryAction::Tick
                }
            };
            assert!(track
                .observe(137, &action, &state, &revised, count(0), count(0))
                .is_err());
        }
    }
    #[test]
    fn repair137_static_audit_must_preserve_all_witnesses() {
        let (mut track, _, revised, mut state) = fixture();
        track.final_policy = Some(revised.commitment());
        state.operation_counter = 3;
        track
            .observe(
                137,
                &RecoveryAction::Apply(probe(0)),
                &state,
                &revised,
                count(0),
                count(0),
            )
            .unwrap();
        track
            .observe(
                137,
                &RecoveryAction::Apply(probe(1)),
                &state,
                &revised,
                count(0),
                count(0),
            )
            .unwrap();
        let mut changed = track.static_audit.clone().unwrap();
        changed.witnesses[0].value_hash = Some([9; 32]);
        assert!(track
            .observe(
                137,
                &RecoveryAction::Audit(changed),
                &state,
                &revised,
                count(0),
                count(0)
            )
            .is_err());
        let correct = track.static_audit.clone().unwrap();
        track
            .observe(
                137,
                &RecoveryAction::Audit(correct),
                &state,
                &revised,
                count(0),
                count(0),
            )
            .unwrap();
    }
    #[test]
    fn completion_requires_final_validation_and_preserved_counter_policy() {
        // Synthetic diagnostic rows only; these do not construct VerifiedReplay.
        for mode in 0..5 {
            let (mut track, original, revised, mut state) = fixture();
            track.final_policy = Some(revised.commitment());
            track.probe_cursor = 2;
            track.audited137 = true;
            state.phase = RecoveryPhase::Complete;
            state.operation_counter = 3;
            let mut proof = serde_json::json!({"activePolicy":hex::encode(original.commitment()),
                "consumedPolicy":hex::encode(original.commitment()),"finalValidation":{
                    "activePolicy":hex::encode(revised.commitment()),"auditedPolicy":hex::encode(original.commitment()),
                    "auditedCounter":2,"consumptionCounter":0,"block":{"operationCounter":3},
                    "pendingIncomingAudits":0,"readOnlyStateRootMatches":true}});
            match mode {
                0 => {}
                1 => {
                    proof.as_object_mut().unwrap().remove("finalValidation");
                }
                2 => proof["finalValidation"]["activePolicy"] = "wrong".into(),
                3 => proof["finalValidation"]["auditedCounter"] = 3.into(),
                _ => proof["finalValidation"]["pendingIncomingAudits"] = 1.into(),
            }
            assert_eq!(
                track.finish(&state, count(0), None, &[proof]).is_ok(),
                mode == 0
            );
        }
    }
    #[test]
    fn temporary_probe_has_exact_guarded_absent_value_absent_shape() {
        let first = probe(0);
        let second = probe(1);
        first.validate().unwrap();
        second.validate().unwrap();
        assert_eq!(first.edits[0].key, second.edits[0].key);
        assert_eq!(first.edits[0].before, None);
        assert_eq!(second.edits[0].after, None);
        assert_eq!(
            second.edits[0].before,
            Some(sp_io::hashing::blake2_256(
                first.edits[0].after.as_ref().unwrap()
            ))
        );
        assert_ne!(first.commitment(), second.commitment());
    }
}
