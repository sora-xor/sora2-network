//! In-process certificates from exact recovery replay; JSON is diagnostic only.
use crate::{maintenance::BlockWitness, replay, Result};
use codec::{DecodeAll, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use common::recovery::{self, RecoveryAction, RecoveryPhase, RecoveryPolicy, RecoveryStatus};
use framenode_runtime::incident_recovery_incoming::{IncomingAudit, IncomingOperation};
use serde::Serialize;
use serde_json::{json, Value};
use sp_core::{
    storage::{StateVersion, Storage},
    H256,
};
use std::collections::BTreeMap;

type OperationKey = ([u8; 32], u32, [u8; 32]);
fn decode<T: DecodeWithMemTracking + Encode>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > recovery::MAX_CHUNK_BYTES {
        return Err("replay incoming payload exceeds bound".into());
    }
    let mut input = bytes;
    let value = T::decode_with_mem_limit(&mut input, recovery::MAX_CHUNK_BYTES * 4)
        .map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != bytes {
        return Err("noncanonical replay incoming payload".into());
    }
    Ok(value)
}
fn decode_policy(bytes: &[u8]) -> Result<RecoveryPolicy> {
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("accepted audit policy exceeds replay bound".into());
    }
    let mut input = bytes;
    let policy = RecoveryPolicy::decode_with_mem_limit(&mut input, 8 * 1024 * 1024)
        .map_err(|error| error.to_string())?;
    if !input.is_empty() || policy.encode() != bytes {
        return Err("accepted audit policy is noncanonical".into());
    }
    policy
        .validate()
        .map_err(|error| format!("accepted audit policy: {error:?}"))?;
    Ok(policy)
}
fn read(ext: &mut sp_io::TestExternalities, key: &[u8]) -> Option<Vec<u8>> {
    ext.execute_with(|| sp_io::storage::get(key).map(|b| b.to_vec()))
}
fn status(ext: &mut sp_io::TestExternalities) -> Result<RecoveryStatus> {
    ext.execute_with(recovery::storage::status)
        .map_err(|e| format!("incoming replay status: {e:?}"))?
        .ok_or("incoming replay status absent".into())
}
fn counter(ext: &mut sp_io::TestExternalities, key: &[u8]) -> Result<u32> {
    let bytes = read(ext, key).ok_or("incoming replay counter absent")?;
    u32::decode_all(&mut bytes.as_slice()).map_err(|e| e.to_string())
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn key(policy: [u8; 32], operation: &IncomingOperation) -> OperationKey {
    (policy, operation.index, operation.commitment())
}

/// Serialized for traceability only. It cannot be deserialized or construct a
/// VerifiedReplay. All approval and external-finality flags remain false.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageBinding {
    checkpoint_state_root: String,
    capture_manifest_sha256: String,
    replay_manifest_sha256: String,
    replay_transcript_sha256: String,
    final_policy_commitment: Option<String>,
    final_state_root: Option<String>,
    public_test_council_fixture: bool,
    chain_local_replay_verified: bool,
    consensus_seals_verified: bool,
    external_finality_verified: bool,
    operator_approval_verified: bool,
    production_ready: bool,
}
impl CoverageBinding {
    pub(crate) fn verify_checkpoint_storage(&self, storage: &Storage) -> Result<()> {
        let code = storage
            .top
            .get(b":code".as_slice())
            .ok_or("incoming coverage checkpoint code absent")?;
        let mut ext = sp_io::TestExternalities::new_with_code_and_state(
            code,
            storage.clone(),
            StateVersion::V0,
        );
        if format!("{:#x}", replay::root(&mut ext)) != self.checkpoint_state_root {
            return Err("prior incoming replay is bound to another checkpoint state root".into());
        }
        Ok(())
    }
}
struct Proof {
    policy: Vec<u8>,
    operation: IncomingOperation,
    audit: IncomingAudit,
    audited_counter: u64,
    audited_block: u32,
    witness: Value,
}
/// No public constructor and no Deserialize implementation. Only a successful
/// in-process maintenance run can return one of these certificates.
pub struct VerifiedReplay {
    binding: CoverageBinding,
    proofs: BTreeMap<([u8; 32], [u8; 32]), Proof>,
    final_policy: Option<Vec<u8>>,
}
impl VerifiedReplay {
    pub(crate) fn operation(
        &self,
        policy: &RecoveryPolicy,
        audit: &IncomingAudit,
    ) -> Result<&IncomingOperation> {
        if !self.binding.chain_local_replay_verified {
            return Err("prior incoming certificate lacks a completed exact replay".into());
        }
        let proof = self
            .proofs
            .get(&(policy.commitment(), audit.commitment()))
            .ok_or("prior incoming audit has no accepted exact-replay companion proof")?;
        if self.final_policy.as_deref() != Some(policy.encode().as_slice())
            || proof.policy != policy.encode()
            || proof.audit.encode() != audit.encode()
            || audit.prior_policy.is_none()
            || proof.operation.index != audit.operation_index
            || proof.operation.commitment() != audit.operation_commitment
        {
            return Err(
                "prior incoming audit differs from accepted replay policy or operation".into(),
            );
        }
        Ok(&proof.operation)
    }
    /// Earlier accepted audits omitted by a later forward-repair policy. The
    /// final state was revalidated before this opaque certificate was created.
    pub(crate) fn historical_operations(
        &self,
        final_policy: &RecoveryPolicy,
    ) -> Result<Vec<(&IncomingOperation, &IncomingAudit)>> {
        if !self.binding.chain_local_replay_verified
            || self.final_policy.as_deref() != Some(final_policy.encode().as_slice())
            || self.binding.final_policy_commitment.as_deref()
                != Some(hex::encode(final_policy.commitment()).as_str())
            || self.binding.final_state_root.is_none()
        {
            return Err(
                "historical incoming evidence lacks the exact final-policy validation".into(),
            );
        }
        let encoded = final_policy.encode();
        Ok(self
            .proofs
            .values()
            .filter(|proof| proof.policy != encoded)
            .map(|proof| (&proof.operation, &proof.audit))
            .collect())
    }
    pub(crate) fn coverage_binding(&self) -> CoverageBinding {
        self.binding.clone()
    }
    pub(crate) fn reports(&self) -> Vec<Value> {
        self.proofs
            .values()
            .filter(|proof| {
                proof.audit.prior_policy.is_some()
                    || Some(proof.policy.as_slice()) != self.final_policy.as_deref()
            })
            .map(|proof| proof.witness.clone())
            .collect()
    }
}
struct AcceptedIncoming {
    operation: IncomingOperation,
    policy: [u8; 32],
    counter: u64,
    block: u32,
    witness: Value,
}
pub(crate) struct Context {
    policy: Vec<u8>,
    status: RecoveryStatus,
    pending: u32,
    number: u32,
    parent_hash: H256,
    parent_snapshot_sha256: String,
    initialized_root: H256,
    executing_code: [u8; 32],
    stored_code: [u8; 32],
}
pub(crate) enum Prepared {
    Incoming(Context, IncomingOperation),
    Audit(Context, IncomingAudit),
}
pub(crate) struct Collector {
    enabled: bool,
    original: [u8; 32],
    checkpoint_state_root: String,
    capture_manifest_sha256: String,
    replay_manifest_sha256: String,
    fixture: bool,
    incoming: BTreeMap<OperationKey, AcceptedIncoming>,
    retained_bytes: usize,
    proofs: BTreeMap<([u8; 32], [u8; 32]), Proof>,
    final_policy: Option<Vec<u8>>,
    final_state_root: Option<String>,
}
impl Collector {
    pub(crate) fn new(
        enabled: bool,
        original: [u8; 32],
        checkpoint_state_root: String,
        capture_manifest_sha256: String,
        replay_manifest_sha256: String,
        fixture: bool,
    ) -> Self {
        Self {
            enabled,
            original,
            checkpoint_state_root,
            capture_manifest_sha256,
            replay_manifest_sha256,
            fixture,
            incoming: BTreeMap::new(),
            retained_bytes: 0,
            proofs: BTreeMap::new(),
            final_policy: None,
            final_state_root: None,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        &self,
        ext: &mut sp_io::TestExternalities,
        policy: &RecoveryPolicy,
        action: &RecoveryAction,
        number: u32,
        parent_hash: H256,
        parent_snapshot_sha256: &str,
        code: &[u8],
    ) -> Result<Option<Prepared>> {
        if !self.enabled {
            return Ok(None);
        }
        let (incoming, audit) = match action {
            RecoveryAction::Incoming(bytes) => (Some(decode::<IncomingOperation>(bytes)?), None),
            RecoveryAction::IncomingAudit(bytes) => {
                let audit = decode::<IncomingAudit>(bytes)?;
                (None, Some(audit))
            }
            _ => return Ok(None),
        };
        policy
            .validate()
            .map_err(|e| format!("incoming replay policy: {e:?}"))?;
        let observed = status(ext)?;
        let active = policy.commitment();
        if observed.policy != self.original
            || observed.format_version != 1
            || read(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref() != Some(active.as_slice())
            || read(ext, recovery::POLICY_STORAGE_KEY).as_deref() != Some(self.original.as_slice())
            || read(ext, recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() != Some(&[1])
            || number <= recovery::CHECKPOINT_NUMBER
        {
            return Err("incoming replay parent lineage/policy/halt mismatch".into());
        }
        let context = Context {
            policy: policy.encode(),
            status: observed,
            pending: counter(ext, recovery::INCOMING_AUDIT_PENDING_KEY)?,
            number,
            parent_hash,
            parent_snapshot_sha256: parent_snapshot_sha256.into(),
            initialized_root: replay::root(ext),
            executing_code: hash(code),
            stored_code: hash(&replay::code(ext)?),
        };
        if let Some(operation) = incoming {
            if context.status.phase != RecoveryPhase::Applying
                || context.status.next_chunk != operation.index
                || policy.chunk_hashes.get(operation.index as usize)
                    != Some(&operation.commitment())
                || self.incoming.contains_key(&key(active, &operation))
            {
                return Err(
                    "incoming replay operation differs from actual work cursor/commitment".into(),
                );
            }
            return Ok(Some(Prepared::Incoming(context, operation)));
        }
        let audit = audit.ok_or("prior audit decode absent")?;
        let consumed = audit.prior_policy.unwrap_or(active);
        let prior = self
            .incoming
            .get(&(consumed, audit.operation_index, audit.operation_commitment))
            .ok_or("prior incoming audit lacks earlier accepted native operation in this replay")?;
        let verified = ext
            .execute_with(|| {
                framenode_runtime::incident_recovery_incoming::validate_incoming_audit(
                    policy,
                    &audit.encode(),
                )
            })
            .map_err(|e| format!("authenticated recovery-parent companion: {e:?}"))?;
        if verified.lineage != self.original
            || verified.active_policy != active
            || verified.consumed_policy != prior.policy
            || verified.operation_index != prior.operation.index
            || verified.operation_commitment != prior.operation.commitment()
            || verified.operation_counter != context.status.operation_counter
            || verified.consumption_counter != prior.counter
            || verified.execution_block != prior.block
            || verified.audit_index != audit.index
            || verified.pending_before != context.pending
        {
            return Err("prior audit companion differs from the accepted consuming block".into());
        }
        if replay::root(ext) != context.initialized_root {
            return Err("read-only incoming companion validation changed storage".into());
        }
        Ok(Some(Prepared::Audit(context, audit)))
    }
    fn reserve(&mut self, bytes: usize) -> Result<()> {
        self.retained_bytes = self
            .retained_bytes
            .checked_add(bytes)
            .ok_or("incoming replay certificate size overflow")?;
        if self.retained_bytes > 64 * 1024 * 1024
            || self.incoming.len() >= recovery::MAX_WORK_CHUNKS
            || self.proofs.len() >= recovery::MAX_WORK_CHUNKS
        {
            return Err("incoming replay certificate exceeds bounded evidence budget".into());
        }
        Ok(())
    }
    pub(crate) fn accepted(
        &mut self,
        prepared: Prepared,
        ext: &mut sp_io::TestExternalities,
        witness: &BlockWitness,
        native: Option<Value>,
    ) -> Result<()> {
        let retained = match &prepared {
            Prepared::Incoming(context, operation) => context
                .policy
                .len()
                .saturating_add(operation.encoded_size()),
            Prepared::Audit(context, audit) => context
                .policy
                .len()
                .saturating_add(audit.encoded_size())
                .saturating_add(recovery::MAX_CHUNK_BYTES),
        };
        self.reserve(retained)?;
        let (context, is_audit) = match &prepared {
            Prepared::Incoming(c, _) => (c, false),
            Prepared::Audit(c, _) => (c, true),
        };
        let after = status(ext)?;
        if witness.number != context.number
            || witness.executing_code_blake2 != hex::encode(context.executing_code)
            || witness.state_root != format!("{:#x}", replay::root(ext))
            || !witness.independent_import_root_matches
            || !witness.disk_restart_root_matches
            || witness.disk_snapshot_manifest_sha256.len() != 64
            || after.policy != context.status.policy
            || after.operation_counter != context.status.operation_counter
        {
            return Err("incoming replay accepted block/root/counter binding mismatch".into());
        }
        let active = hash(&context.policy);
        if read(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref() != Some(active.as_slice()) {
            return Err("incoming replay active policy changed during operation".into());
        }
        // The actual witness is emitted only after independent import and a
        // journal persist which reopens and restores the exact state root.
        let mut receipt = json!({"block":witness,"parentHash":format!("{:#x}",context.parent_hash),
            "parentSnapshotSha256":context.parent_snapshot_sha256,"initializedRoot":format!("{:#x}",context.initialized_root),
            "storedCodeBlake2":hex::encode(context.stored_code),"executingCodeBlake2":hex::encode(context.executing_code),
            "originalPolicy":hex::encode(self.original),"activePolicy":hex::encode(active),
            "publicTestCouncilFixture":self.fixture,"externalFinalityVerified":false,"operatorApprovalVerified":false,
            "consensusSealsVerified":false,"productionReady":false});
        if !is_audit {
            let Prepared::Incoming(context, operation) = prepared else {
                unreachable!()
            };
            let native = native.ok_or("accepted incoming lacks actual native comparison")?;
            for field in [
                "joinedToVerifiedReplayPrefix",
                "exactAuthoredBlockBytesMatch",
                "completeOperationDeltaCompared",
                "nativeTerminalSnapshotIndependentlyRestored",
            ] {
                if native.get(field) != Some(&Value::Bool(true)) {
                    return Err(format!("incoming native comparison lacks {field}"));
                }
            }
            if native.get("block").and_then(Value::as_u64) != Some(u64::from(witness.number))
                || native.get("operationIndex").and_then(Value::as_u64)
                    != Some(u64::from(operation.index))
                || after.next_chunk
                    != operation
                        .index
                        .checked_add(1)
                        .ok_or("incoming cursor overflow")?
                || !matches!(
                    after.phase,
                    RecoveryPhase::Applying | RecoveryPhase::Auditing
                )
                || counter(ext, recovery::INCOMING_AUDIT_PENDING_KEY)?
                    != context
                        .pending
                        .checked_add(1)
                        .ok_or("incoming pending overflow")?
            {
                return Err("accepted incoming cursor/pending/native context mismatch".into());
            }
            receipt["nativeComparison"] = native;
            let key = key(active, &operation);
            if self
                .incoming
                .insert(
                    key,
                    AcceptedIncoming {
                        operation,
                        policy: active,
                        counter: after.operation_counter,
                        block: witness.number,
                        witness: receipt,
                    },
                )
                .is_some()
            {
                return Err("duplicate accepted incoming operation".into());
            }
        } else {
            let Prepared::Audit(context, audit) = prepared else {
                unreachable!()
            };
            let consumed = audit.prior_policy.unwrap_or(active);
            let marker = framenode_runtime::incident_recovery_incoming::audit_marker_key(
                consumed,
                audit.operation_commitment,
            );
            if after.encode() != context.status.encode()
                || counter(ext, recovery::AUDIT_CURSOR_STORAGE_KEY)?
                    != audit.index.checked_add(1).ok_or("audit cursor overflow")?
                || counter(ext, recovery::INCOMING_AUDIT_PENDING_KEY)?
                    != context
                        .pending
                        .checked_sub(1)
                        .ok_or("audit pending underflow")?
                || read(ext, &marker)
                    != Some(
                        (
                            self.original,
                            active,
                            audit.commitment(),
                            after.operation_counter,
                        )
                            .encode(),
                    )
            {
                return Err("accepted prior incoming audit marker/cursor/pending mismatch".into());
            }
            let prior = self
                .incoming
                .get(&(consumed, audit.operation_index, audit.operation_commitment))
                .ok_or("accepted consuming operation disappeared")?;
            receipt["consumingBlock"] = prior.witness.clone();
            receipt["consumedPolicy"] = hex::encode(consumed).into();
            receipt["auditCommitment"] = hex::encode(audit.commitment()).into();
            let proof = Proof {
                policy: context.policy,
                operation: prior.operation.clone(),
                audit,
                audited_counter: after.operation_counter,
                audited_block: witness.number,
                witness: receipt,
            };
            if self
                .proofs
                .insert((active, proof.audit.commitment()), proof)
                .is_some()
            {
                return Err("duplicate accepted prior audit".into());
            }
        }
        Ok(())
    }
    pub(crate) fn validate_final(
        &mut self,
        ext: &mut sp_io::TestExternalities,
        policy: &RecoveryPolicy,
        completed: bool,
        witness: Option<&BlockWitness>,
    ) -> Result<()> {
        if self.proofs.is_empty() {
            return Ok(());
        }
        if !self.enabled || !completed || self.final_policy.is_some() {
            return Err("historical audit validation requires one completed exact replay".into());
        }
        let witness = witness.ok_or("historical audit final block absent")?;
        let before = replay::root(ext);
        let state = status(ext)?;
        if state.policy != self.original
            || state.phase != RecoveryPhase::Complete
            || state.failure.is_some()
            || witness.resulting_spec != 138
            || witness.phase != "Complete"
            || witness.operation_counter != state.operation_counter
            || witness.number != ext.execute_with(framenode_runtime::System::block_number)
            || witness.state_root != format!("{before:#x}")
            || !witness.independent_import_root_matches
            || !witness.disk_restart_root_matches
            || witness.disk_snapshot_manifest_sha256.len() != 64
        {
            return Err(
                "historical audit final state differs from accepted import/restart witness".into(),
            );
        }
        for proof in self.proofs.values_mut() {
            let audited_policy = decode_policy(&proof.policy)?;
            let checked = ext.execute_with(|| {
                framenode_runtime::incident_recovery_incoming::validate_completed_incoming_audit(
                    policy, &audited_policy, &proof.audit.encode(), proof.audited_counter, proof.audited_block,
                )
            }).map_err(|e| format!("historical incoming final-state validation: {e:?}"))?;
            let consumed = proof
                .audit
                .prior_policy
                .unwrap_or(audited_policy.commitment());
            let prior = self
                .incoming
                .get(&(
                    consumed,
                    proof.operation.index,
                    proof.operation.commitment(),
                ))
                .ok_or("historical consuming operation absent")?;
            if checked.lineage != self.original
                || checked.active_policy != policy.commitment()
                || checked.consumed_policy != prior.policy
                || checked.operation_counter != state.operation_counter
                || checked.consumption_counter != prior.counter
                || checked.execution_block != prior.block
                || checked.operation_index != proof.operation.index
                || checked.operation_commitment != proof.operation.commitment()
                || checked.audit_index != proof.audit.index
                || checked.pending_before != 0
                || replay::root(ext) != before
            {
                return Err(
                    "historical audit final companion differs from accepted consumption".into(),
                );
            }
            proof.witness["finalValidation"] = json!({"block":witness,
                "activePolicy":hex::encode(policy.commitment()),"auditedPolicy":hex::encode(audited_policy.commitment()),
                "auditedCounter":proof.audited_counter,"auditedBlock":proof.audited_block,
                "consumptionCounter":prior.counter,"consumptionBlock":prior.block,
                "readOnlyStateRootMatches":true,"pendingIncomingAudits":0,
                "productionReady":false});
        }
        self.final_policy = Some(policy.encode());
        self.final_state_root = Some(witness.state_root.clone());
        Ok(())
    }
    pub(crate) fn finish(
        self,
        completed: bool,
        transcript: String,
        require_prior: bool,
    ) -> Result<VerifiedReplay> {
        let has_prior = self.proofs.values().any(|proof| {
            proof.audit.prior_policy.is_some()
                || Some(proof.policy.as_slice()) != self.final_policy.as_deref()
        });
        if require_prior && (!self.enabled || !completed || !has_prior) {
            return Err("authenticated prior incoming evidence requires successful activation and a prior-policy audit".into());
        }
        if !self.proofs.is_empty()
            && (self.final_policy.is_none() || self.final_state_root.is_none())
        {
            return Err("incoming replay certificate lacks final-state audit validation".into());
        }
        Ok(VerifiedReplay {
            binding: CoverageBinding {
                checkpoint_state_root: self.checkpoint_state_root,
                capture_manifest_sha256: self.capture_manifest_sha256,
                replay_manifest_sha256: self.replay_manifest_sha256,
                replay_transcript_sha256: transcript,
                final_policy_commitment: self.final_policy.as_ref().map(|p| hex::encode(hash(p))),
                final_state_root: self.final_state_root,
                public_test_council_fixture: self.fixture,
                chain_local_replay_verified: self.enabled && completed,
                consensus_seals_verified: false,
                external_finality_verified: false,
                operator_approval_verified: false,
                production_ready: false,
            },
            proofs: self.proofs,
            final_policy: self.final_policy,
        })
    }
}

/// Synthetic certificate fixture for loader unit tests only. Never compiled into
/// the CLI and never written as replay evidence.
#[cfg(test)]
pub(crate) fn test_certificate(
    policy: &RecoveryPolicy,
    audit: &IncomingAudit,
    operation: IncomingOperation,
) -> VerifiedReplay {
    VerifiedReplay {
        binding: CoverageBinding {
            checkpoint_state_root: format!("{:#x}", H256::zero()),
            capture_manifest_sha256: "11".repeat(32),
            replay_manifest_sha256: "22".repeat(32),
            replay_transcript_sha256: "33".repeat(32),
            final_policy_commitment: Some(hex::encode(policy.commitment())),
            final_state_root: Some(format!("{:#x}", H256::zero())),
            public_test_council_fixture: true,
            chain_local_replay_verified: true,
            consensus_seals_verified: false,
            external_finality_verified: false,
            operator_approval_verified: false,
            production_ready: false,
        },
        proofs: BTreeMap::from([(
            (policy.commitment(), audit.commitment()),
            Proof {
                policy: policy.encode(),
                operation,
                audit: audit.clone(),
                audited_counter: 2,
                audited_block: recovery::CHECKPOINT_NUMBER + 2,
                witness: json!({"testOnly":true}),
            },
        )]),
        final_policy: Some(policy.encode()),
    }
}

#[cfg(test)]
pub(crate) fn test_historical_certificate(
    final_policy: &RecoveryPolicy,
    audited_policy: &RecoveryPolicy,
    audit: &IncomingAudit,
    operation: IncomingOperation,
) -> VerifiedReplay {
    let mut certificate = test_certificate(audited_policy, audit, operation);
    certificate.final_policy = Some(final_policy.encode());
    certificate.binding.final_policy_commitment = Some(hex::encode(final_policy.commitment()));
    certificate
}

#[cfg(test)]
mod tests {
    use super::*;
    use framenode_runtime::incident_recovery_incoming::IncomingChannel;
    fn policy() -> RecoveryPolicy {
        RecoveryPolicy {
            format_version: 1,
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![[1; 32].into()],
            chunk_hashes: vec![[2; 32]],
            audit_chunk_hashes: vec![[3; 32]],
            evidence_commitment: [4; 32],
            terminal_audit_commitment: [5; 32],
        }
    }
    fn operation() -> IncomingOperation {
        IncomingOperation {
            index: 0,
            channel: IncomingChannel::Hashi {
                network: 0,
                request: vec![1],
                approvals: vec![],
            },
            registries: vec![],
            finalized_source_evidence: [6; 32],
        }
    }
    fn collector(enabled: bool) -> Collector {
        Collector::new(
            enabled,
            [7; 32],
            format!("{:#x}", H256::repeat_byte(8)),
            "11".repeat(32),
            "22".repeat(32),
            true,
        )
    }
    // These are isolated private-certificate guard tests, not replay evidence.
    fn certificate() -> (VerifiedReplay, RecoveryPolicy, IncomingAudit) {
        let mut policy = policy();
        let operation = operation();
        let audit = IncomingAudit {
            index: 0,
            operation_index: 0,
            operation_commitment: operation.commitment(),
            requests: vec![None],
            prior_policy: Some([7; 32]),
        };
        policy.audit_chunk_hashes = vec![audit.commitment()];
        let mut collector = collector(true);
        collector.proofs.insert(
            (policy.commitment(), audit.commitment()),
            Proof {
                policy: policy.encode(),
                operation,
                audit: audit.clone(),
                audited_counter: 2,
                audited_block: recovery::CHECKPOINT_NUMBER + 2,
                witness: json!({"testOnly":true}),
            },
        );
        collector.final_policy = Some(policy.encode());
        collector.final_state_root = Some(format!("{:#x}", H256::zero()));
        (
            collector.finish(true, "33".repeat(32), true).unwrap(),
            policy,
            audit,
        )
    }
    #[test]
    fn prior_certificate_rejects_policy_audit_and_payload_substitution() {
        let (certificate, policy, audit) = certificate();
        assert_eq!(
            certificate.operation(&policy, &audit).unwrap().commitment(),
            audit.operation_commitment
        );
        let mut changed = policy.clone();
        changed.evidence_commitment = [9; 32];
        assert!(certificate
            .operation(&changed, &audit)
            .unwrap_err()
            .contains("no accepted"));
        let mut changed = audit.clone();
        changed.prior_policy = Some([9; 32]);
        assert!(certificate
            .operation(&policy, &changed)
            .unwrap_err()
            .contains("no accepted"));
        let mut changed = audit.clone();
        changed.operation_index = 1;
        assert!(certificate
            .operation(&policy, &changed)
            .unwrap_err()
            .contains("no accepted"));
        let mut changed = audit;
        changed.requests.clear();
        assert!(certificate
            .operation(&policy, &changed)
            .unwrap_err()
            .contains("no accepted"));
    }
    #[test]
    fn certificate_requires_completed_replay_and_never_claims_external_approval() {
        assert!(collector(false)
            .finish(true, "33".repeat(32), true)
            .err()
            .unwrap()
            .contains("successful activation"));
        assert!(collector(true)
            .finish(false, "33".repeat(32), true)
            .err()
            .unwrap()
            .contains("successful activation"));
        assert!(collector(true)
            .finish(true, "33".repeat(32), true)
            .err()
            .unwrap()
            .contains("prior-policy audit"));
        let (mut certificate, policy, audit) = certificate();
        let report = serde_json::to_value(certificate.coverage_binding()).unwrap();
        assert_eq!(report["chainLocalReplayVerified"], true);
        assert_eq!(report["publicTestCouncilFixture"], true);
        for field in [
            "consensusSealsVerified",
            "externalFinalityVerified",
            "operatorApprovalVerified",
            "productionReady",
        ] {
            assert_eq!(report[field], false);
        }
        certificate.binding.chain_local_replay_verified = false;
        assert!(certificate
            .operation(&policy, &audit)
            .unwrap_err()
            .contains("completed exact replay"));
    }
    #[test]
    fn prior_certificate_checkpoint_root_includes_child_storage() {
        let (mut certificate, _, _) = certificate();
        let mut storage = Storage::default();
        storage.top.insert(b":code".to_vec(), vec![1]);
        let code = storage.top[b":code".as_slice()].clone();
        let mut ext = sp_io::TestExternalities::new_with_code_and_state(
            &code,
            storage.clone(),
            StateVersion::V0,
        );
        certificate.binding.checkpoint_state_root = format!("{:#x}", replay::root(&mut ext));
        assert!(certificate
            .binding
            .verify_checkpoint_storage(&storage)
            .is_ok());
        storage.top.insert(b"balance".to_vec(), vec![1]);
        assert!(certificate
            .binding
            .verify_checkpoint_storage(&storage)
            .unwrap_err()
            .contains("another checkpoint"));
        storage.top.remove(b"balance".as_slice());
        storage.children_default.insert(
            b"child".to_vec(),
            sp_core::storage::StorageChild {
                data: BTreeMap::from([(b"owned".to_vec(), vec![1])]),
                child_info: sp_core::storage::ChildInfo::new_default(b"child"),
            },
        );
        assert!(certificate
            .binding
            .verify_checkpoint_storage(&storage)
            .unwrap_err()
            .contains("another checkpoint"));
    }
    #[test]
    fn payload_decoding_and_retained_evidence_have_finite_bounds() {
        let operation = operation();
        let encoded = operation.encode();
        assert_eq!(
            decode::<IncomingOperation>(&encoded).unwrap().encode(),
            encoded
        );
        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode::<IncomingOperation>(&trailing)
            .unwrap_err()
            .contains("noncanonical"));
        assert!(
            decode::<IncomingOperation>(&vec![0; recovery::MAX_CHUNK_BYTES + 1])
                .unwrap_err()
                .contains("bound")
        );
        let mut collector = collector(true);
        collector.reserve(64 * 1024 * 1024).unwrap();
        assert!(collector
            .reserve(1)
            .unwrap_err()
            .contains("bounded evidence budget"));
    }
    #[test]
    fn historical_certificate_requires_exact_final_policy_and_final_validation() {
        let (mut certificate, audited, audit) = certificate();
        let mut final_policy = audited.clone();
        final_policy.evidence_commitment = [111; 32];
        assert!(certificate.historical_operations(&final_policy).is_err());
        certificate.final_policy = Some(final_policy.encode());
        certificate.binding.final_policy_commitment = Some(hex::encode(final_policy.commitment()));
        let historical = certificate.historical_operations(&final_policy).unwrap();
        assert_eq!(historical.len(), 1);
        assert_eq!(historical[0].1.encode(), audit.encode());
        assert!(certificate.operation(&final_policy, &audit).is_err());
        certificate.binding.final_state_root = None;
        assert!(certificate.historical_operations(&final_policy).is_err());
        let mut incomplete = collector(true);
        incomplete.proofs = certificate.proofs;
        assert!(incomplete
            .finish(true, "33".repeat(32), true)
            .err()
            .unwrap()
            .contains("final-state"));
    }
}
