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
    incident_recovery_incoming::{
        request_balance_keys, AuditedIncomingSource, IncomingAudit, IncomingOperation,
        NativeBalanceAudit, MAX_BALANCE_SOURCES,
    },
    incident_recovery_outgoing::OutgoingOperation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, BufReader, Read},
    path::Path,
};

// Complete checkpoint composition contains ~193 MiB of envelope text. Keep an
// explicit aggregate cap independent of the unchanged per-envelope runtime bound.
const MAX_ENVELOPE_TEXT: usize = 256 * 1024 * 1024;
// JSON whitespace/field overhead has a separate finite stream budget. The raw
// document is never buffered in a second Vec alongside its decoded strings.
const MAX_DOCUMENT: usize = 512 * 1024 * 1024;
const MAX_TEXT_ITEM: usize = recovery::MAX_INHERENT_BYTES * 2 + 2;

fn text_budget<'a>(values: impl IntoIterator<Item = &'a str>) -> Result<usize> {
    values.into_iter().try_fold(0usize, |sum, text| {
        if text.len() > MAX_TEXT_ITEM {
            return Err("mixed-work envelope exceeds per-item transport bound".into());
        }
        let next = sum
            .checked_add(text.len())
            .ok_or("mixed-work input size overflow")?;
        if next > MAX_ENVELOPE_TEXT {
            return Err("mixed-work input exceeds aggregate envelope text budget".into());
        }
        Ok(next)
    })
}

fn bounded_text<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    let text = String::deserialize(deserializer)?;
    text_budget([text.as_str()]).map_err(serde::de::Error::custom)?;
    Ok(text)
}

fn bounded_envelopes<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<String>, D::Error> {
    struct Envelopes;
    impl<'de> serde::de::Visitor<'de> for Envelopes {
        type Value = Vec<String>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a bounded sequence of recovery envelopes")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            if sequence
                .size_hint()
                .is_some_and(|count| count > recovery::MAX_WORK_CHUNKS)
            {
                return Err(serde::de::Error::custom(
                    "mixed-work envelope count exceeds bound",
                ));
            }
            let mut values = Vec::new();
            let mut total = 0usize;
            while let Some(text) = sequence.next_element::<String>()? {
                if values.len() >= recovery::MAX_WORK_CHUNKS {
                    return Err(serde::de::Error::custom(
                        "mixed-work envelope count exceeds bound",
                    ));
                }
                let bytes = text_budget([text.as_str()]).map_err(serde::de::Error::custom)?;
                total = total
                    .checked_add(bytes)
                    .ok_or_else(|| serde::de::Error::custom("mixed-work input size overflow"))?;
                if total > MAX_ENVELOPE_TEXT {
                    return Err(serde::de::Error::custom(
                        "mixed-work sequence exceeds aggregate envelope text budget",
                    ));
                }
                values.push(text);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_seq(Envelopes)
}

struct DocumentReader<R> {
    inner: R,
    maximum: usize,
    bytes: usize,
}
impl<R: Read> Read for DocumentReader<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let remaining = self.maximum.saturating_sub(self.bytes);
        let length = output.len().min(remaining.saturating_add(1));
        let count = self.inner.read(&mut output[..length])?;
        if count > remaining {
            return Err(io::Error::other(
                "mixed-work JSON document exceeds byte bound",
            ));
        }
        self.bytes += count;
        Ok(count)
    }
}
fn parse_manifest<R: Read>(reader: R, maximum: usize) -> Result<Manifest> {
    let reader = DocumentReader {
        inner: reader,
        maximum,
        bytes: 0,
    };
    // serde_json reads individual bytes; the outer buffer keeps file reads bounded
    // and efficient. from_reader also rejects any trailing non-whitespace input.
    serde_json::from_reader(BufReader::with_capacity(64 * 1024, reader))
        .map_err(|error| format!("mixed-work JSON document: {error}"))
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    /// Canonical SCALE RecoveryPolicy. Work/audit hashes may be empty, and the
    /// terminal hash zero, or each must exactly match the supplied work. Stale
    /// commitments are rejected rather than silently replaced.
    #[serde(deserialize_with = "bounded_text")]
    pub policy_template_scale: String,
    /// Full encode_remark envelopes, in caller-reviewed execution order.
    #[serde(deserialize_with = "bounded_envelopes")]
    pub work_envelopes: Vec<String>,
    #[serde(deserialize_with = "bounded_envelopes")]
    pub audit_envelopes: Vec<String>,
    #[serde(deserialize_with = "bounded_text")]
    pub terminal_envelope: String,
    #[serde(default, deserialize_with = "bounded_envelopes")]
    pub terminal_prelude_envelopes: Vec<String>,
    /// Derive explicit account-balance audit substitutions from unchanged baseline witnesses.
    #[serde(default)]
    pub derive_native_balance_audits: bool,
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
    pub terminal_prelude_envelopes: Vec<String>,
    pub work: Vec<WorkEntry>,
    pub baseline_audit_envelopes: Vec<String>,
    pub native_balance_audit_indices: Vec<u32>,
    pub native_balance_audits_derived: bool,
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
pub fn compile(mut manifest: Manifest) -> Result<Report> {
    if manifest.schema != "sora-nexus-mixed-work-input-v1"
        || manifest.work_envelopes.is_empty()
        || manifest.work_envelopes.len() > recovery::MAX_WORK_CHUNKS
        || manifest.audit_envelopes.is_empty()
        || manifest.audit_envelopes.len() > recovery::MAX_WORK_CHUNKS
    {
        return Err("unknown mixed-work schema or work count outside bounds".into());
    }
    text_budget(
        manifest
            .work_envelopes
            .iter()
            .chain(&manifest.audit_envelopes)
            .chain(&manifest.terminal_prelude_envelopes)
            .chain([&manifest.policy_template_scale, &manifest.terminal_envelope])
            .map(String::as_str),
    )?;
    let template_bytes = hex_bytes(&manifest.policy_template_scale)?;
    let mut policy: RecoveryPolicy = decode(&template_bytes, recovery::MAX_INHERENT_BYTES)?;
    let mut work = Vec::new();
    let mut commitments = Vec::new();
    let mut identities = BTreeSet::new();
    let mut pending_incoming = BTreeMap::new();
    let mut bridge_work_seen = false;
    for (position, text) in manifest.work_envelopes.iter().enumerate() {
        let (envelope, bytes) = envelope(text)?;
        let (index, kind, commitment, identity, future_signatures) = match envelope.action {
            RecoveryAction::Apply(mut chunk) => {
                // Composition models only raw edits, not native bridge effects
                // on balances, issuance, requests, locks or channel counters.
                // Its preimages are valid only before bridge execution begins.
                if bridge_work_seen {
                    return Err("all storage edits must precede typed bridge work".into());
                }
                let (index, commitment) = (chunk.index, chunk.commitment());
                chunk.index = 0;
                (index, "storage", commitment, chunk.commitment(), false)
            }
            RecoveryAction::Incoming(bytes) => {
                bridge_work_seen = true;
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
                bridge_work_seen = true;
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

    let baseline_audit_envelopes = manifest.audit_envelopes.clone();
    let mut native_balance_audit_indices = Vec::new();
    if manifest.derive_native_balance_audits {
        // The caller provides only the original exact audit stream. These are structural
        // source associations; their authentication and final values are established on-chain.
        let mut sources = Vec::new();
        for (position, text) in baseline_audit_envelopes.iter().enumerate() {
            match envelope(text)?.0.action {
                RecoveryAction::IncomingAudit(bytes) => {
                    let audit: IncomingAudit = decode(&bytes, recovery::MAX_CHUNK_BYTES)?;
                    audit
                        .validate()
                        .map_err(|e| format!("incoming audit shape: {e:?}"))?;
                    if audit.prior_policy.is_some() {
                        return Err("automatic balance audit derivation requires current-policy incoming work".into());
                    }
                    sources.push((position, audit));
                }
                RecoveryAction::NativeBalanceAudit(_) => {
                    return Err(
                        "caller-provided balance audit cannot replace compiler baseline".into(),
                    )
                }
                _ => {}
            }
        }
        // Reject oversized mapped-recipient cohorts before emitting a policy. The runtime
        // checks the same bound against its actual journal, including across policy repairs.
        let mut key_counts = BTreeMap::<Vec<u8>, usize>::new();
        for (_, source) in &sources {
            let keys: BTreeSet<_> = source
                .requests
                .iter()
                .flatten()
                .filter(|request| {
                    policy
                        .replacements
                        .iter()
                        .any(|entry| entry.new == request.beneficiary)
                })
                .flat_map(request_balance_keys)
                .collect();
            for key in keys {
                let count = key_counts.entry(key).or_default();
                *count += 1;
                if *count > MAX_BALANCE_SOURCES {
                    return Err(
                        "mapped recipient exceeds the complete native balance journal bound".into(),
                    );
                }
            }
        }
        // Work cursors are the already-reviewed actual native execution order. No work moves.
        sources.sort_by_key(|(_, source)| source.operation_index);
        for (position, text) in baseline_audit_envelopes.iter().enumerate() {
            let RecoveryAction::Audit(baseline) = envelope(text)?.0.action else {
                continue;
            };
            let keys: BTreeSet<_> = baseline
                .witnesses
                .iter()
                .filter(|w| w.scope == StorageScope::Top && w.value_hash.is_some())
                .map(|w| w.key.clone())
                .collect();
            let selected: Vec<_> = sources
                .iter()
                .filter(|(_, source)| {
                    source
                        .requests
                        .iter()
                        .flatten()
                        .filter(|request| {
                            policy
                                .replacements
                                .iter()
                                .any(|entry| entry.new == request.beneficiary)
                        })
                        .flat_map(request_balance_keys)
                        .any(|key| keys.contains(&key))
                })
                .collect();
            if selected.is_empty() {
                continue;
            }
            if selected
                .iter()
                .any(|(source_position, _)| *source_position >= position)
                || sources
                    .iter()
                    .any(|(source_position, _)| *source_position >= position)
            {
                return Err(
                    "all native IncomingAudit actions must precede a derived account balance audit"
                        .into(),
                );
            }
            let audit = NativeBalanceAudit {
                baseline,
                sources: selected
                    .into_iter()
                    .map(|(_, source)| AuditedIncomingSource {
                        audit: source.clone(),
                        audited_policy: None,
                    })
                    .collect(),
            };
            audit
                .validate()
                .map_err(|e| format!("native balance audit derivation: {e:?}"))?;
            native_balance_audit_indices.push(audit.baseline.index);
            manifest.audit_envelopes[position] = hx(&RecoveryInherent {
                format_version: 1,
                action: RecoveryAction::NativeBalanceAudit(audit.encode()),
            }
            .encode_remark());
        }
    }
    let mut admitted_incoming_audits = BTreeMap::<u32, IncomingAudit>::new();
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
            RecoveryAction::NativeBalanceAudit(bytes) => {
                let audit: NativeBalanceAudit = decode(&bytes, recovery::MAX_CHUNK_BYTES)?;
                audit.validate().map_err(|e| format!("native balance audit shape: {e:?}"))?;
                for source in &audit.sources {
                    if source.audited_policy.is_none() && admitted_incoming_audits.get(&source.audit.index)
                        .is_none_or(|accepted| accepted.encode() != source.audit.encode()) {
                        return Err("native balance source is not an earlier exact incoming audit".into());
                    }
                }
                for witness in &audit.baseline.witnesses {
                    let child = match &witness.scope { StorageScope::Top => None,
                        StorageScope::Child(child) => Some(child.clone()) };
                    if audit_locations.insert((child, witness.key.clone()), witness.value_hash).is_some() {
                        return Err("duplicate storage witness across audit chunks".into());
                    }
                }
                (audit.baseline.index, audit.commitment())
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
                admitted_incoming_audits.insert(audit.index, audit.clone());
                (audit.index, audit.commitment())
            }
            _ => {
                return Err(
                    "audit list permits only Audit, PrefixAudit, IncomingAudit and NativeBalanceAudit actions".into(),
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
    let terminal_hash = if manifest.terminal_prelude_envelopes.is_empty() {
        let RecoveryAction::TerminalAudit(terminal) = terminal.action else {
            return Err("single terminal envelope must contain TerminalAudit".into());
        };
        terminal.commitment()
    } else {
        let mut pages = Vec::new();
        for text in &manifest.terminal_prelude_envelopes {
            let (value, _) = envelope(text)?;
            let RecoveryAction::TerminalAuditPage(page) = value.action else {
                return Err("terminal prelude requires only terminal pages".into());
            };
            pages.push(page);
        }
        let RecoveryAction::TerminalAuditPage(last) = terminal.action else {
            return Err("paged terminal requires its last page".into());
        };
        pages.push(last);
        recovery::invariants::join_terminal_pages(&pages)
            .map_err(|e| format!("terminal page sequence: {e:?}"))?;
        pages[0].commitment()
    };
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
        terminal_prelude_envelopes: manifest.terminal_prelude_envelopes,
        work,
        baseline_audit_envelopes,
        native_balance_audit_indices,
        native_balance_audits_derived: manifest.derive_native_balance_audits,
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
    let file = fs::File::open(path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > MAX_DOCUMENT as u64 {
        return Err("mixed-work JSON document exceeds byte bound".into());
    }
    compile(parse_manifest(file, MAX_DOCUMENT)?)
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
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
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
            derive_native_balance_audits: false,
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
            terminal_prelude_envelopes: vec![],
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

    fn supply_keys() -> Vec<Vec<u8>> {
        let balances =
            frame_support::storage::storage_prefix(b"Balances", b"TotalIssuance").to_vec();
        let mut tokens =
            frame_support::storage::storage_prefix(b"Tokens", b"TotalIssuance").to_vec();
        // Tokens.TotalIssuance is a Twox64Concat map. Use a distinct asset:
        // the ordering rule covers every supply key, not only the incoming asset.
        let asset = common::PSWAP.encode();
        tokens.extend(sp_io::hashing::twox_64(&asset));
        tokens.extend(asset);
        vec![balances, tokens]
    }

    fn supply_edit(index: u32, scope: StorageScope, key: Vec<u8>, delete: bool) -> String {
        wrap(RecoveryAction::Apply(RecoveryChunk {
            index,
            edits: vec![GuardedStorageEdit {
                scope,
                key,
                before: Some([16; 32]),
                after: (!delete).then(|| 90u128.encode()),
            }],
        }))
    }

    #[test]
    fn issuance_edits_after_incoming_are_rejected_before_policy_emission() {
        for key in supply_keys() {
            for delete in [false, true] {
                let mut manifest = fixture();
                manifest.work_envelopes[2] = supply_edit(2, StorageScope::Top, key.clone(), delete);
                assert!(compile(manifest)
                    .unwrap_err()
                    .contains("all storage edits must precede typed bridge work"));
            }
        }
    }

    #[test]
    fn issuance_edits_before_incoming_remain_supported() {
        for key in supply_keys() {
            let mut manifest = fixture();
            manifest.work_envelopes[0] = supply_edit(0, StorageScope::Top, key, false);
            assert!(compile(manifest).is_ok());
        }
    }

    #[test]
    fn unrelated_and_child_storage_edits_after_incoming_are_rejected() {
        let mut manifest = fixture();
        manifest.work_envelopes[2] = supply_edit(
            2,
            StorageScope::Top,
            b"unrelated-final-cleanup".to_vec(),
            false,
        );
        assert!(compile(manifest)
            .unwrap_err()
            .contains("all storage edits must precede typed bridge work"));
        for key in supply_keys() {
            let mut manifest = fixture();
            manifest.work_envelopes[2] = supply_edit(
                2,
                StorageScope::Child(b"fixture-child".to_vec()),
                key,
                false,
            );
            assert!(compile(manifest)
                .unwrap_err()
                .contains("all storage edits must precede typed bridge work"));
        }
    }

    #[test]
    fn storage_edits_after_outgoing_are_rejected_even_without_incoming() {
        let mut manifest = fixture();
        let mut outgoing = envelope(&manifest.work_envelopes[2]).unwrap().0;
        let RecoveryAction::Outgoing(bytes) = &mut outgoing.action else {
            unreachable!()
        };
        let mut operation: OutgoingOperation = decode(bytes, recovery::MAX_CHUNK_BYTES).unwrap();
        operation.index = 0;
        *bytes = operation.encode();
        manifest.work_envelopes = vec![
            hx(&outgoing.encode_remark()),
            supply_edit(1, StorageScope::Top, b"bridge-request".to_vec(), true),
        ];
        assert!(compile(manifest)
            .unwrap_err()
            .contains("all storage edits must precede typed bridge work"));
    }

    fn fixture_json() -> Vec<u8> {
        let value = fixture();
        serde_json::to_vec(&serde_json::json!({
            "schema": value.schema,
            "policyTemplateScale": value.policy_template_scale,
            "workEnvelopes": value.work_envelopes,
            "auditEnvelopes": value.audit_envelopes,
            "terminalEnvelope": value.terminal_envelope,
            "deriveNativeBalanceAudits": value.derive_native_balance_audits,
        }))
        .unwrap()
    }

    #[test]
    fn paginated_terminal_requires_complete_ordered_prelude() {
        let mut manifest = fixture();
        let (single, _) = envelope(&manifest.terminal_envelope).unwrap();
        let RecoveryAction::TerminalAudit(mut audit) = single.action else {
            panic!("fixture terminal")
        };
        let base = audit.issuance[0].clone();
        for n in 0..128u32 {
            let mut item = base.clone();
            item.asset = sp_io::hashing::blake2_256(&n.encode());
            item.current.key = n.encode();
            audit.issuance.push(item);
        }
        let actions = recovery::invariants::terminal_actions(&audit).unwrap();
        manifest.terminal_prelude_envelopes = actions[..actions.len() - 1]
            .iter()
            .cloned()
            .map(wrap)
            .collect();
        manifest.terminal_envelope = wrap(actions.last().unwrap().clone());
        let report = compile(manifest.clone()).unwrap();
        assert_eq!(report.terminal_prelude_envelopes.len(), 1);
        manifest.terminal_prelude_envelopes.clear();
        assert!(compile(manifest).is_err());
    }

    #[test]
    fn complete_checkpoint_sized_transport_keeps_every_work_and_audit_entry() {
        // These counts and maximum item lengths come from the retained successful
        // full checkpoint composition. Reuse borrowed strings to exercise the
        // whole aggregate admission without allocating a 250 MiB unit fixture.
        let work = "0".repeat(85_538);
        let audit = "0".repeat(173_090);
        let texts = std::iter::repeat_n(work.as_str(), 738)
            .chain(std::iter::repeat_n(audit.as_str(), 1144));
        let accepted = text_budget(texts).unwrap();
        assert_eq!(accepted, 738 * 85_538 + 1144 * 173_090);
        assert!(accepted > 64 * 1024 * 1024);
        assert!(accepted < MAX_ENVELOPE_TEXT);
        assert_eq!(MAX_ENVELOPE_TEXT, 256 * 1024 * 1024);
    }

    #[test]
    fn aggregate_budget_rejects_overflow_without_changing_per_envelope_limits() {
        let text = "0".repeat(MAX_TEXT_ITEM);
        assert_eq!(text_budget([text.as_str()]).unwrap(), MAX_TEXT_ITEM);
        assert!(text_budget(["0".repeat(MAX_TEXT_ITEM + 1).as_str()])
            .unwrap_err()
            .contains("per-item"));
        let count = MAX_ENVELOPE_TEXT / MAX_TEXT_ITEM + 1;
        assert!(count < recovery::MAX_WORK_CHUNKS);
        assert!(text_budget(std::iter::repeat_n(text.as_str(), count))
            .unwrap_err()
            .contains("aggregate"));
        // Direct callers bypassing JSON still receive the same early admission.
        let mut value = fixture();
        value.terminal_envelope = "0".repeat(MAX_TEXT_ITEM + 1);
        assert!(compile(value).unwrap_err().contains("per-item"));
    }

    #[test]
    fn streamed_manifest_preserves_compilation_and_rejects_truncation_and_trailing_data() {
        let raw = fixture_json();
        let report = compile(parse_manifest(raw.as_slice(), raw.len()).unwrap()).unwrap();
        let original = compile(fixture()).unwrap();
        assert_eq!(report.policy_scale, original.policy_scale);
        assert_eq!(report.work_envelopes, original.work_envelopes);
        assert_eq!(report.audit_envelopes, original.audit_envelopes);
        assert_eq!(report.terminal_envelope, original.terminal_envelope);
        assert!(parse_manifest(raw.as_slice(), raw.len() - 1).is_err());
        let mut trailing = raw.clone();
        trailing.extend_from_slice(b"{}");
        assert!(parse_manifest(trailing.as_slice(), trailing.len()).is_err());
        let mut padded = raw.clone();
        padded.extend_from_slice(b"   ");
        assert!(parse_manifest(padded.as_slice(), raw.len()).is_err());
        assert!(parse_manifest(padded.as_slice(), padded.len()).is_ok());
    }

    #[test]
    fn streamed_manifest_rejects_excess_counts_and_oversized_fields_before_compilation() {
        let raw = fixture_json();
        for field in ["workEnvelopes", "auditEnvelopes"] {
            let mut json: serde_json::Value = serde_json::from_slice(&raw).unwrap();
            json[field] = serde_json::json!(vec![""; recovery::MAX_WORK_CHUNKS + 1]);
            let bytes = serde_json::to_vec(&json).unwrap();
            assert!(parse_manifest(bytes.as_slice(), MAX_DOCUMENT)
                .unwrap_err()
                .contains("count"));
        }
        for field in [
            "policyTemplateScale",
            "terminalEnvelope",
            "workEnvelopes",
            "auditEnvelopes",
        ] {
            let mut json: serde_json::Value = serde_json::from_slice(&raw).unwrap();
            let text = serde_json::json!("0".repeat(MAX_TEXT_ITEM + 1));
            if field.ends_with("Envelopes") {
                json[field][0] = text;
            } else {
                json[field] = text;
            }
            let bytes = serde_json::to_vec(&json).unwrap();
            assert!(parse_manifest(bytes.as_slice(), MAX_DOCUMENT)
                .unwrap_err()
                .contains("per-item"));
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
    #[test]
    fn native_balance_derivation_preserves_original_witnesses_and_exact_accepted_source() {
        let mut value = fixture();
        value.derive_native_balance_audits = true;
        let mut template: RecoveryPolicy = decode(
            &hex_bytes(&value.policy_template_scale).unwrap(),
            recovery::MAX_INHERENT_BYTES,
        )
        .unwrap();
        template.replacements = vec![recovery::AccountReplacement {
            old: AccountId32::new([8; 32]),
            new: AccountId32::new([9; 32]),
        }];
        value.policy_template_scale = hx(&template.encode());
        let source = incoming_audit(0, &incoming(1, 8));
        let baseline = RecoveryAuditChunk {
            index: 1,
            witnesses: vec![StorageWitness {
                scope: StorageScope::Top,
                key: request_balance_keys(source.requests[0].as_ref().unwrap())[0].clone(),
                value_hash: Some([31; 32]),
            }],
        };
        value.audit_envelopes = vec![
            wrap(RecoveryAction::IncomingAudit(source.encode())),
            wrap(RecoveryAction::Audit(baseline.clone())),
        ];
        let originals = value.audit_envelopes.clone();
        let report = compile(value).unwrap();
        assert_eq!(report.baseline_audit_envelopes, originals);
        assert_eq!(report.native_balance_audit_indices, vec![1]);
        assert_eq!(report.audit_envelopes[0], originals[0]);
        let RecoveryAction::NativeBalanceAudit(bytes) =
            envelope(&report.audit_envelopes[1]).unwrap().0.action
        else {
            panic!("derived action");
        };
        let derived: NativeBalanceAudit = decode(&bytes, recovery::MAX_CHUNK_BYTES).unwrap();
        assert_eq!(derived.baseline.encode(), baseline.encode());
        assert_eq!(derived.sources.len(), 1);
        assert_eq!(derived.sources[0].audit.encode(), source.encode());
        assert!(derived.sources[0].audited_policy.is_none());
    }

    #[test]
    fn native_balance_derivation_refuses_unaudited_source_and_caller_replacement() {
        let mut value = fixture();
        value.derive_native_balance_audits = true;
        let mut template: RecoveryPolicy = decode(
            &hex_bytes(&value.policy_template_scale).unwrap(),
            recovery::MAX_INHERENT_BYTES,
        )
        .unwrap();
        template.replacements = vec![recovery::AccountReplacement {
            old: AccountId32::new([8; 32]),
            new: AccountId32::new([9; 32]),
        }];
        value.policy_template_scale = hx(&template.encode());
        let source = incoming_audit(1, &incoming(1, 8));
        let baseline = RecoveryAuditChunk {
            index: 0,
            witnesses: vec![StorageWitness {
                scope: StorageScope::Top,
                key: request_balance_keys(source.requests[0].as_ref().unwrap())[0].clone(),
                value_hash: Some([31; 32]),
            }],
        };
        value.audit_envelopes = vec![
            wrap(RecoveryAction::Audit(baseline.clone())),
            wrap(RecoveryAction::IncomingAudit(source.encode())),
        ];
        assert!(compile(value).unwrap_err().contains("must precede"));
        let mut value = fixture();
        value.derive_native_balance_audits = true;
        value.audit_envelopes[0] = wrap(RecoveryAction::NativeBalanceAudit(
            NativeBalanceAudit {
                baseline,
                sources: vec![AuditedIncomingSource {
                    audit: source,
                    audited_policy: None,
                }],
            }
            .encode(),
        ));
        assert!(compile(value).unwrap_err().contains("caller-provided"));
    }
    #[test]
    fn native_balance_derivation_rejects_oversized_recipient_before_emitting_policy() {
        let mut value = fixture();
        value.derive_native_balance_audits = true;
        let mut template: RecoveryPolicy = decode(
            &hex_bytes(&value.policy_template_scale).unwrap(),
            recovery::MAX_INHERENT_BYTES,
        )
        .unwrap();
        template.replacements = vec![recovery::AccountReplacement {
            old: AccountId32::new([8; 32]),
            new: AccountId32::new([9; 32]),
        }];
        value.policy_template_scale = hx(&template.encode());
        let source = incoming_audit(0, &incoming(1, 8));
        value.audit_envelopes = (0..=MAX_BALANCE_SOURCES)
            .map(|index| {
                let mut source = source.clone();
                source.index = index as u32;
                source.operation_index = index as u32;
                source.operation_commitment = sp_io::hashing::blake2_256(&(index as u32).encode());
                wrap(RecoveryAction::IncomingAudit(source.encode()))
            })
            .collect();
        assert!(compile(value)
            .unwrap_err()
            .contains("complete native balance journal bound"));
    }
}
