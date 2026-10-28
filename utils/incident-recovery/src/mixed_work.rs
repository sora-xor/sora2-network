//! Structural assembly of ordered, already typed recovery work.
//!
//! This module deliberately does not execute storage or bridge adapters. It
//! cannot establish authentication, liabilities, economic conservation, complete
//! inventory coverage or release readiness. The exact runtime types calculate
//! adapter commitments, including HASHI's future-signature exclusion.

use crate::{hex_bytes, Result};
use codec::{DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use common::recovery::{self, RecoveryAction, RecoveryInherent, RecoveryPolicy, StorageScope};
use framenode_runtime::{
    incident_recovery_incoming::{IncomingAudit, IncomingOperation},
    incident_recovery_outgoing::OutgoingOperation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

const MAX_DOCUMENT: usize = 64 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    /// Canonical SCALE RecoveryPolicy. Work/audit hashes may be empty, and the
    /// terminal hash zero, or each must exactly match the supplied work. Stale
    /// commitments are rejected rather than silently replaced.
    pub policy_template_scale: String,
    /// Full encode_remark envelopes, in caller-reviewed execution order.
    pub work_envelopes: Vec<String>,
    pub audit_envelopes: Vec<String>,
    pub terminal_envelope: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkEntry {
    pub index: u32,
    pub kind: &'static str,
    pub commitment: String,
    pub bytes: usize,
    pub future_signatures_excluded_from_commitment: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: &'static str,
    pub policy_template_sha256: String,
    pub policy_scale: String,
    pub policy_commitment: String,
    pub work_envelopes: Vec<String>,
    pub audit_envelopes: Vec<String>,
    pub terminal_envelope: String,
    pub work: Vec<WorkEntry>,
    pub structural_validation_only: bool,
    pub authentication_verified: bool,
    pub economic_completion_verified: bool,
    pub storage_engine_rehearsed: bool,
    pub exact_artifact_rehearsed: bool,
    pub production_ready: bool,
    pub remaining: &'static str,
}

fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

fn decode<T: DecodeWithMemTracking + Encode>(bytes: &[u8], limit: usize) -> Result<T> {
    if bytes.is_empty() || bytes.len() > limit {
        return Err("typed input exceeds transport bounds".into());
    }
    let mut input = bytes;
    let value = T::decode_with_mem_limit(&mut input, limit.saturating_mul(4))
        .map_err(|error| format!("typed decode failed: {error}"))?;
    if !input.is_empty() || value.encode() != bytes {
        return Err("noncanonical or trailing typed input".into());
    }
    Ok(value)
}

fn envelope(text: &str) -> Result<(RecoveryInherent, Vec<u8>)> {
    if text.len() > recovery::MAX_INHERENT_BYTES * 2 + 2 {
        return Err("oversized work envelope".into());
    }
    let bytes = hex_bytes(text)?;
    let envelope = RecoveryInherent::decode_remark(&bytes)
        .map_err(|error| format!("invalid recovery envelope: {error:?}"))?;
    if envelope.encode_remark() != bytes {
        return Err("noncanonical recovery envelope".into());
    }
    Ok((envelope, bytes))
}

/// Preserve every caller-selected index and byte. Only the explicitly blank
/// policy commitment slots are filled. No operation is sorted, split or signed.
pub fn compile(manifest: Manifest) -> Result<Report> {
    if manifest.schema != "sora-nexus-mixed-work-input-v1"
        || manifest.work_envelopes.is_empty()
        || manifest.work_envelopes.len() > recovery::MAX_WORK_CHUNKS
        || manifest.audit_envelopes.is_empty()
        || manifest.audit_envelopes.len() > recovery::MAX_WORK_CHUNKS
    {
        return Err("unknown mixed-work schema or work count outside bounds".into());
    }
    let text_bytes = manifest
        .work_envelopes
        .iter()
        .chain(&manifest.audit_envelopes)
        .chain([&manifest.policy_template_scale, &manifest.terminal_envelope])
        .try_fold(0usize, |sum, text| sum.checked_add(text.len()))
        .ok_or("mixed-work input size overflow")?;
    if text_bytes > MAX_DOCUMENT
        || manifest.policy_template_scale.len() > recovery::MAX_INHERENT_BYTES * 2 + 2
    {
        return Err("mixed-work input exceeds bounded document budget".into());
    }
    let template_bytes = hex_bytes(&manifest.policy_template_scale)?;
    let mut policy: RecoveryPolicy = decode(&template_bytes, recovery::MAX_INHERENT_BYTES)?;
    let mut work = Vec::new();
    let mut commitments = Vec::new();
    let mut identities = BTreeSet::new();
    let mut pending_incoming = BTreeMap::new();
    for (position, text) in manifest.work_envelopes.iter().enumerate() {
        let (envelope, bytes) = envelope(text)?;
        let (index, kind, commitment, identity, future_signatures) = match envelope.action {
            RecoveryAction::Apply(mut chunk) => {
                let (index, commitment) = (chunk.index, chunk.commitment());
                chunk.index = 0;
                (index, "storage", commitment, chunk.commitment(), false)
            }
            RecoveryAction::Incoming(bytes) => {
                let mut operation: IncomingOperation = decode(&bytes, recovery::MAX_CHUNK_BYTES)?;
                let future_signatures = matches!(
                    &operation.channel,
                    framenode_runtime::incident_recovery_incoming::IncomingChannel::Hashi { .. }
                    | framenode_runtime::incident_recovery_incoming::IncomingChannel::HashiAuthenticatedSource { .. }
                );
                let (index, commitment) = (operation.index, operation.commitment());
                if pending_incoming.insert(index, commitment).is_some() {
                    return Err("duplicate incoming work index".into());
                }
                operation.index = 0;
                (
                    index,
                    "incoming",
                    commitment,
                    operation.commitment(),
                    future_signatures,
                )
            }
            RecoveryAction::Outgoing(bytes) => {
                let mut operation: OutgoingOperation = decode(&bytes, recovery::MAX_CHUNK_BYTES)?;
                let (index, commitment) = (operation.index, operation.commitment());
                operation.index = 0;
                (index, "outgoing", commitment, operation.commitment(), false)
            }
            _ => return Err("work list permits only Apply, Incoming and Outgoing actions".into()),
        };
        if index as usize != position {
            return Err("work indices must be contiguous and preserve supplied order".into());
        }
        if !identities.insert((kind, identity)) {
            return Err(
                "duplicate work operation (ignoring its cursor and future HASHI signatures)".into(),
            );
        }
        commitments.push(commitment);
        work.push(WorkEntry {
            index,
            kind,
            commitment: hx(&commitment),
            bytes: bytes.len(),
            future_signatures_excluded_from_commitment: future_signatures,
        });
    }

    let mut audit_hashes = Vec::new();
    let mut audit_locations = BTreeMap::new();
    let mut empty_prefixes = BTreeSet::new();
    let mut incoming_audits = BTreeSet::new();
    for (position, text) in manifest.audit_envelopes.iter().enumerate() {
        let (envelope, _) = envelope(text)?;
        let (index, commitment) = match envelope.action {
            RecoveryAction::Audit(audit) => {
                for witness in &audit.witnesses {
                    let child = match &witness.scope {
                        StorageScope::Top => None,
                        StorageScope::Child(child) => Some(child.clone()),
                    };
                    if audit_locations
                        .insert((child, witness.key.clone()), witness.value_hash)
                        .is_some()
                    {
                        return Err("duplicate storage witness across audit chunks".into());
                    }
                }
                (audit.index, audit.commitment())
            }
            RecoveryAction::PrefixAudit(audit) => {
                for witness in &audit.prefixes {
                    let child = match &witness.scope {
                        StorageScope::Top => None,
                        StorageScope::Child(child) => Some(child.clone()),
                    };
                    if !empty_prefixes.insert((child, witness.prefix.clone())) {
                        return Err("duplicate empty-prefix witness across audit chunks".into());
                    }
                }
                (audit.index, audit.commitment())
            }
            RecoveryAction::IncomingAudit(bytes) => {
                let audit: IncomingAudit = decode(&bytes, recovery::MAX_CHUNK_BYTES)?;
                audit
                    .validate()
                    .map_err(|error| format!("incoming audit shape: {error:?}"))?;
                let operation = audit.operation_commitment;
                if !incoming_audits.insert((audit.prior_policy, audit.operation_index, operation)) {
                    return Err("duplicate incoming semantic audit".into());
                }
                if audit.prior_policy.is_none()
                    && pending_incoming.remove(&audit.operation_index) != Some(operation)
                {
                    return Err("incoming audit differs from its exact work operation".into());
                }
                // A prior-policy companion is verified only by runtime execution
                // against an authenticated recovery parent. Structural assembly
                // cannot establish that it exists or was consumed on this branch.
                (audit.index, audit.commitment())
            }
            _ => {
                return Err(
                    "audit list permits only Audit, PrefixAudit and IncomingAudit actions".into(),
                )
            }
        };
        if index as usize != position {
            return Err("audit indices must be contiguous and preserve supplied order".into());
        }
        audit_hashes.push(commitment);
    }
    if !pending_incoming.is_empty() {
        return Err("every incoming work operation requires one matching IncomingAudit".into());
    }
    let mut previous: Option<&(Option<Vec<u8>>, Vec<u8>)> = None;
    for prefix in &empty_prefixes {
        if previous.is_some_and(|prior| prior.0 == prefix.0 && prefix.1.starts_with(&prior.1)) {
            return Err("overlapping empty-prefix witnesses across audit chunks".into());
        }
        previous = Some(prefix);
        for ((scope, key), value) in audit_locations.range((prefix.0.clone(), prefix.1.clone())..) {
            if scope != &prefix.0 || !key.starts_with(&prefix.1) {
                break;
            }
            if value.is_some() {
                return Err("present-key witness contradicts an empty-prefix audit".into());
            }
        }
    }
    let (terminal, _) = envelope(&manifest.terminal_envelope)?;
    let RecoveryAction::TerminalAudit(terminal) = terminal.action else {
        return Err("terminal envelope must contain TerminalAudit".into());
    };
    let terminal_hash = terminal.commitment();
    if (!policy.chunk_hashes.is_empty() && policy.chunk_hashes != commitments)
        || (!policy.audit_chunk_hashes.is_empty() && policy.audit_chunk_hashes != audit_hashes)
        || (policy.terminal_audit_commitment != [0; 32]
            && policy.terminal_audit_commitment != terminal_hash)
    {
        return Err(
            "policy template contains stale or contradictory work/audit commitments".into(),
        );
    }
    policy.chunk_hashes = commitments;
    policy.audit_chunk_hashes = audit_hashes;
    policy.terminal_audit_commitment = terminal_hash;
    policy
        .validate()
        .map_err(|error| format!("invalid assembled policy: {error:?}"))?;
    if policy.encoded_size() > recovery::MAX_INHERENT_BYTES {
        return Err("assembled policy exceeds bounded release transport".into());
    }
    Ok(Report {
        schema: "sora-nexus-structural-mixed-work-v1",
        policy_template_sha256: hex::encode(Sha256::digest(&template_bytes)),
        policy_scale: hx(&policy.encode()),
        policy_commitment: hx(&policy.commitment()),
        work_envelopes: manifest.work_envelopes,
        audit_envelopes: manifest.audit_envelopes,
        terminal_envelope: manifest.terminal_envelope,
        work,
        structural_validation_only: true,
        authentication_verified: false,
        economic_completion_verified: false,
        storage_engine_rehearsed: false,
        exact_artifact_rehearsed: false,
        production_ready: false,
        remaining: "Execute every adapter against the independently verified checkpoint and exact recovery Wasm; verify complete ownership, liability, authorization, incoming/outgoing and entitlement evidence and terminal conservation. Structural assembly establishes none of these release gates.",
    })
}

pub fn compile_file(path: &Path) -> Result<Report> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > MAX_DOCUMENT as u64 {
        return Err("mixed-work manifest exceeds 64 MiB".into());
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_DOCUMENT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_DOCUMENT {
        return Err("mixed-work manifest exceeds 64 MiB".into());
    }
    compile(serde_json::from_slice(&bytes).map_err(|error| error.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::recovery::{
        invariants::{IssuanceInvariant, StoredAmount, TerminalAudit},
        GuardedStorageEdit, RecoveryAuditChunk, RecoveryChunk, StorageWitness,
    };
    use framenode_runtime::incident_recovery_incoming::{
        HashiPeerApproval, IncomingChannel, IncomingRequestExpectation,
    };
    use framenode_runtime::incident_recovery_outgoing::OutgoingTarget;
    use sp_core::{crypto::AccountId32, H256};

    fn wrap(action: RecoveryAction) -> String {
        hx(&RecoveryInherent {
            format_version: 1,
            action,
        }
        .encode_remark())
    }
    fn incoming(index: u32, signature_byte: u8) -> IncomingOperation {
        IncomingOperation {
            index,
            channel: IncomingChannel::Hashi {
                network: 0,
                request: vec![3],
                approvals: vec![HashiPeerApproval {
                    signer: AccountId32::new([4; 32]),
                    call: vec![5],
                    extra: vec![6],
                    signature: sp_runtime::MultiSignature::Ed25519(
                        sp_core::ed25519::Signature::from_raw([signature_byte; 64]),
                    ),
                }],
            },
            registries: Vec::new(),
            finalized_source_evidence: [7; 32],
        }
    }
    fn incoming_audit(index: u32, operation: &IncomingOperation) -> IncomingAudit {
        IncomingAudit {
            index,
            operation_index: operation.index,
            operation_commitment: operation.commitment(),
            requests: vec![Some(IncomingRequestExpectation {
                source: bridge_types::GenericAccount::EVM(sp_core::H160::repeat_byte(3)),
                beneficiary: AccountId32::new([9; 32]),
                asset_id: common::XOR,
                amount: 1,
                start_timepoint: bridge_types::GenericTimepoint::EVM(1),
            })],
            prior_policy: None,
        }
    }
    fn fixture() -> Manifest {
        let policy = RecoveryPolicy {
            format_version: 1,
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: Vec::new(),
            council: vec![AccountId32::new([1; 32])],
            chunk_hashes: Vec::new(),
            audit_chunk_hashes: Vec::new(),
            evidence_commitment: [2; 32],
            terminal_audit_commitment: [0; 32],
        };
        let outgoing = OutgoingOperation {
            index: 2,
            target: OutgoingTarget::Hashi {
                network: 0,
                request: H256::repeat_byte(9),
                request_hash: [10; 32],
                status_hash: [11; 32],
                author: AccountId32::new([12; 32]),
                transfer: Some((common::XOR, 100)),
            },
            reconciliation_evidence: [13; 32],
        };
        Manifest {
            schema: "sora-nexus-mixed-work-input-v1".into(),
            policy_template_scale: hx(&policy.encode()),
            work_envelopes: vec![
                wrap(RecoveryAction::Apply(RecoveryChunk {
                    index: 0,
                    edits: vec![GuardedStorageEdit {
                        scope: StorageScope::Top,
                        key: b"fixture".to_vec(),
                        before: None,
                        after: Some(vec![1]),
                    }],
                })),
                wrap(RecoveryAction::Incoming(incoming(1, 8).encode())),
                wrap(RecoveryAction::Outgoing(outgoing.encode())),
            ],
            audit_envelopes: vec![
                wrap(RecoveryAction::Audit(RecoveryAuditChunk {
                    index: 0,
                    witnesses: vec![StorageWitness {
                        scope: StorageScope::Top,
                        key: b"fixture".to_vec(),
                        value_hash: Some([14; 32]),
                    }],
                })),
                wrap(RecoveryAction::IncomingAudit(
                    incoming_audit(1, &incoming(1, 8)).encode(),
                )),
            ],
            terminal_envelope: wrap(RecoveryAction::TerminalAudit(TerminalAudit {
                issuance: vec![IssuanceInvariant {
                    asset: [0; 32],
                    current: StoredAmount {
                        scope: StorageScope::Top,
                        key: b"issuance".to_vec(),
                        record_hash: [15; 32],
                        offset: 0,
                    },
                    checkpoint_issuance: 100,
                    authenticated_mints: 0,
                    burned: 0,
                }],
                collateral: Vec::new(),
                zero_claims: Vec::new(),
                absent: Vec::new(),
            })),
        }
    }

    #[test]
    fn prefix_audits_are_bound_and_reject_duplicate_overlap_or_present_keys() {
        use common::recovery::{RecoveryPrefixAuditChunk, StoragePrefixWitness};
        let prefix = |index, key: &[u8]| {
            wrap(RecoveryAction::PrefixAudit(RecoveryPrefixAuditChunk {
                index,
                prefixes: vec![StoragePrefixWitness {
                    scope: StorageScope::Top,
                    prefix: key.to_vec(),
                }],
            }))
        };
        let mut manifest = fixture();
        manifest.audit_envelopes.push(prefix(2, b"absent/"));
        let report = compile(manifest).unwrap();
        let policy: RecoveryPolicy = decode(
            &hex_bytes(&report.policy_scale).unwrap(),
            recovery::MAX_INHERENT_BYTES,
        )
        .unwrap();
        assert_eq!(policy.audit_chunk_hashes.len(), 3);
        for second in [b"absent/".as_slice(), b"absent/child".as_slice()] {
            let mut manifest = fixture();
            manifest
                .audit_envelopes
                .extend([prefix(2, b"absent/"), prefix(3, second)]);
            assert!(compile(manifest).is_err());
        }
        let mut manifest = fixture();
        manifest.audit_envelopes.push(prefix(2, b"fixture"));
        assert!(compile(manifest).unwrap_err().contains("contradicts"));
        let mut manifest = fixture();
        manifest.audit_envelopes.push(prefix(3, b"absent/"));
        assert!(compile(manifest).unwrap_err().contains("contiguous"));
    }

    #[test]
    fn mixed_order_and_runtime_domains_preserved_without_claiming_execution() {
        let manifest = fixture();
        let original = manifest.work_envelopes.clone();
        let report = compile(manifest).unwrap();
        assert_eq!(report.work_envelopes, original);
        assert_eq!(
            report.work.iter().map(|row| row.kind).collect::<Vec<_>>(),
            vec!["storage", "incoming", "outgoing"]
        );
        assert_eq!(report.work[1].commitment, hx(&incoming(1, 8).commitment()));
        assert!(report.work[1].future_signatures_excluded_from_commitment);
        assert!(report.structural_validation_only);
        assert!(
            !(report.authentication_verified
                || report.economic_completion_verified
                || report.storage_engine_rehearsed
                || report.exact_artifact_rehearsed
                || report.production_ready)
        );
    }

    #[test]
    fn hashi_future_signatures_preserve_policy_but_business_fields_change_it() {
        let base = compile(fixture()).unwrap();
        let mut manifest = fixture();
        manifest.work_envelopes[1] = wrap(RecoveryAction::Incoming(incoming(1, 99).encode()));
        let changed = compile(manifest).unwrap();
        assert_eq!(base.policy_commitment, changed.policy_commitment);
        assert_ne!(base.work_envelopes, changed.work_envelopes);
        let mut manifest = fixture();
        let mut operation = incoming(1, 8);
        if let IncomingChannel::Hashi { approvals, .. } = &mut operation.channel {
            approvals[0].call.push(1);
        }
        manifest.work_envelopes[1] = wrap(RecoveryAction::Incoming(operation.encode()));
        manifest.audit_envelopes[1] = wrap(RecoveryAction::IncomingAudit(
            incoming_audit(1, &operation).encode(),
        ));
        assert_ne!(
            base.policy_commitment,
            compile(manifest).unwrap().policy_commitment
        );
    }

    #[test]
    fn rejects_nonwork_gaps_duplicates_and_trailing_adapter_bytes() {
        let mut manifest = fixture();
        manifest.work_envelopes[0] = wrap(RecoveryAction::Tick);
        assert!(compile(manifest).is_err());
        for index in [0, 2] {
            let mut manifest = fixture();
            manifest.work_envelopes[1] =
                wrap(RecoveryAction::Incoming(incoming(index, 8).encode()));
            assert!(compile(manifest).is_err());
        }
        let mut manifest = fixture();
        manifest.work_envelopes[2] = wrap(RecoveryAction::Incoming(incoming(2, 99).encode()));
        assert!(compile(manifest).unwrap_err().contains("duplicate work"));
        let mut manifest = fixture();
        let mut operation = incoming(1, 8).encode();
        operation.push(0);
        manifest.work_envelopes[1] = wrap(RecoveryAction::Incoming(operation));
        assert!(compile(manifest).is_err());
    }

    #[test]
    fn incoming_audit_requires_exact_single_current_work_coverage() {
        assert!(compile(fixture()).is_ok());
        let mut manifest = fixture();
        manifest.audit_envelopes.pop();
        assert!(compile(manifest).unwrap_err().contains("every incoming"));
        for mutation in 0..3 {
            let mut manifest = fixture();
            let mut audit = incoming_audit(1, &incoming(1, 8));
            match mutation {
                0 => audit.operation_index = 0,
                1 => audit.operation_commitment = [33; 32],
                _ => audit.prior_policy = Some([34; 32]),
            }
            manifest.audit_envelopes[1] = wrap(RecoveryAction::IncomingAudit(audit.encode()));
            assert!(compile(manifest).is_err(), "mutation {mutation}");
        }
        let mut manifest = fixture();
        manifest
            .audit_envelopes
            .push(wrap(RecoveryAction::IncomingAudit(
                incoming_audit(2, &incoming(1, 99)).encode(),
            )));
        assert!(compile(manifest)
            .unwrap_err()
            .contains("duplicate incoming"));
    }

    #[test]
    fn explicit_prior_policy_audit_is_only_structural_and_cannot_cover_current_work() {
        let mut manifest = fixture();
        let mut audit = incoming_audit(2, &incoming(7, 8));
        audit.prior_policy = Some([35; 32]);
        manifest
            .audit_envelopes
            .push(wrap(RecoveryAction::IncomingAudit(audit.encode())));
        let result = compile(manifest).unwrap();
        assert!(result.structural_validation_only);
        assert!(
            !result.authentication_verified
                && !result.exact_artifact_rehearsed
                && !result.production_ready
        );
    }

    #[test]
    fn rejects_stale_policy_hashes_and_repeated_audit_witnesses() {
        for field in 0..3 {
            let mut manifest = fixture();
            let mut policy: RecoveryPolicy = decode(
                &hex_bytes(&manifest.policy_template_scale).unwrap(),
                recovery::MAX_INHERENT_BYTES,
            )
            .unwrap();
            match field {
                0 => policy.chunk_hashes = vec![[99; 32]],
                1 => policy.audit_chunk_hashes = vec![[99; 32]],
                _ => policy.terminal_audit_commitment = [99; 32],
            }
            manifest.policy_template_scale = hx(&policy.encode());
            assert!(compile(manifest).unwrap_err().contains("stale"));
        }
        let mut manifest = fixture();
        let (mut audit, _) = envelope(&manifest.audit_envelopes[0]).unwrap();
        if let RecoveryAction::Audit(audit) = &mut audit.action {
            audit.index = 1;
        }
        manifest.audit_envelopes.push(hx(&audit.encode_remark()));
        assert!(compile(manifest)
            .unwrap_err()
            .contains("duplicate storage witness"));
    }

    #[test]
    fn accepts_matching_bound_policy_and_rejects_oversized_or_invalid_storage_work() {
        let report = compile(fixture()).unwrap();
        let mut manifest = fixture();
        manifest.policy_template_scale = report.policy_scale;
        assert_eq!(
            compile(manifest).unwrap().policy_commitment,
            report.policy_commitment
        );
        let mut manifest = fixture();
        manifest.work_envelopes[0] = format!("0x{}", "00".repeat(recovery::MAX_INHERENT_BYTES + 1));
        assert!(compile(manifest).is_err());
        let mut manifest = fixture();
        manifest.work_envelopes[0] = wrap(RecoveryAction::Apply(RecoveryChunk {
            index: 0,
            edits: vec![GuardedStorageEdit {
                scope: StorageScope::Top,
                key: b":code".to_vec(),
                before: None,
                after: Some(vec![1]),
            }],
        }));
        assert!(compile(manifest).is_err());
    }
}
