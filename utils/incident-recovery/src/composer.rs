//! Deterministic assembly of reviewed storage transformations. Authentication
//! and economic semantics belong to the typed adapters; this layer never
//! resolves conflicting edits by ordering or silently splits an atomic group.

use crate::{hex_bytes, inventory::GuardedCandidate, Result};
use codec::Encode;
use common::recovery::{
    self, invariants::TerminalAudit, AccountReplacement, GuardedStorageEdit, RecoveryAction,
    RecoveryAuditChunk, RecoveryChunk, RecoveryInherent, RecoveryPolicy, RecoveryPrefixAuditChunk,
    StoragePrefixWitness, StorageScope, StorageWitness, MAX_CHUNK_BYTES, MAX_WORK_CHUNKS,
};
use serde::Serialize;
use sp_core::{
    crypto::AccountId32,
    storage::{StateVersion, Storage},
};
use std::collections::{BTreeMap, BTreeSet};

type Location = (Option<Vec<u8>>, Vec<u8>);

fn location(scope: &StorageScope, key: &[u8]) -> Location {
    (
        match scope {
            StorageScope::Top => None,
            StorageScope::Child(id) => Some(id.clone()),
        },
        key.to_vec(),
    )
}

fn read<'a>(storage: &'a Storage, scope: &StorageScope, key: &[u8]) -> Option<&'a Vec<u8>> {
    match scope {
        StorageScope::Top => storage.top.get(key),
        StorageScope::Child(id) => storage.children_default.get(id)?.data.get(key),
    }
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// Emit the final runtime's immutable source binding into an isolated release
/// export. This does not certify the policy's evidence or admit an artifact.
pub fn final_runtime_source(policy: &RecoveryPolicy) -> Result<String> {
    policy
        .validate()
        .map_err(|e| format!("invalid final policy: {e:?}"))?;
    let array = |bytes: &[u8]| format!("{:?}", bytes);
    let mut source = format!(
        "//! Generated SORA Nexus recovery policy. Bound artifact rehearsals remain mandatory.\n\
         pub const POLICY_SCALE: &[u8] = &{};\n\
         pub const POLICY_COMMITMENT: [u8; 32] = {};\n\
         pub const REPLACEMENTS: &[([u8; 32], [u8; 32])] = &[\n",
        array(&policy.encode()),
        array(&policy.commitment()),
    );
    for entry in &policy.replacements {
        source.push_str(&format!(
            "    ({}, {}),\n",
            array(entry.old.as_ref()),
            array(entry.new.as_ref())
        ));
    }
    source.push_str("];\n");
    Ok(source)
}

/// Each source deletion and destination insertion is inseparable. The typed
/// inventory has already proved the destination is unused throughout state.
pub fn mapping_groups(candidates: &[GuardedCandidate]) -> Result<Vec<Vec<GuardedStorageEdit>>> {
    candidates
        .iter()
        .map(|candidate| {
            let scope = if candidate.trie == "top" {
                StorageScope::Top
            } else {
                let key = hex_bytes(&candidate.trie)?;
                StorageScope::Child(
                    key.strip_prefix(b":child_storage:default:")
                        .ok_or("unsupported child-trie kind")?
                        .to_vec(),
                )
            };
            let source = hex_bytes(&candidate.source_key)?;
            let destination = hex_bytes(&candidate.destination_key)?;
            let before: [u8; 32] = hex_bytes(&candidate.expected_source_value_hash)?
                .try_into()
                .map_err(|_| "source hash must contain 32 bytes")?;
            let after = hex_bytes(&candidate.replacement_value)?;
            if source == destination {
                if candidate.require_destination_absent {
                    return Err("in-place edit cannot require its source absent".into());
                }
                Ok(vec![GuardedStorageEdit {
                    scope,
                    key: source,
                    before: Some(before),
                    after: Some(after),
                }])
            } else {
                if !candidate.require_destination_absent {
                    return Err("account move lacks an absent destination guard".into());
                }
                Ok(vec![
                    GuardedStorageEdit {
                        scope: scope.clone(),
                        key: source,
                        before: Some(before),
                        after: None,
                    },
                    GuardedStorageEdit {
                        scope,
                        key: destination,
                        before: None,
                        after: Some(after),
                    },
                ])
            }
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkReport {
    pub schema: &'static str,
    pub policy_scale: String,
    pub policy_commitment: String,
    pub apply_envelopes: Vec<String>,
    pub audit_envelopes: Vec<String>,
    pub terminal_envelope: String,
    pub changed_keys: usize,
    pub storage_engine_rehearsed: bool,
    pub production_ready: bool,
    pub remaining: &'static str,
}

pub struct CompiledWork {
    pub policy: RecoveryPolicy,
    pub chunks: Vec<RecoveryChunk>,
    pub audits: Vec<RecoveryAuditChunk>,
    pub prefix_audits: Vec<RecoveryPrefixAuditChunk>,
    pub terminal: TerminalAudit,
    pub report: WorkReport,
}

#[derive(Default)]
pub struct AuditRequests {
    pub keys: Vec<StorageWitness>,
    pub empty_prefixes: Vec<StoragePrefixWitness>,
}

fn envelope(action: RecoveryAction) -> Result<String> {
    let bytes = RecoveryInherent {
        format_version: 1,
        action,
    }
    .encode_remark();
    RecoveryInherent::decode_remark(&bytes).map_err(|e| format!("envelope rejected: {e:?}"))?;
    Ok(hx(&bytes))
}

/// Caller supplies only typed, independently reviewed groups with no unresolved
/// report blockers. It must still bind consensus, bridge, entitlement and
/// history evidence in the outer release. This output alone is not a release.
pub fn compose(
    storage: &Storage,
    replacements: Vec<AccountReplacement>,
    council: Vec<AccountId32>,
    evidence_commitment: [u8; 32],
    groups: Vec<Vec<GuardedStorageEdit>>,
    additional_audits: Vec<StorageWitness>,
    terminal: TerminalAudit,
) -> Result<CompiledWork> {
    compose_with_audits(
        storage,
        replacements,
        council,
        evidence_commitment,
        groups,
        AuditRequests {
            keys: additional_audits,
            empty_prefixes: vec![],
        },
        terminal,
    )
}

pub fn compose_with_audits(
    storage: &Storage,
    replacements: Vec<AccountReplacement>,
    council: Vec<AccountId32>,
    evidence_commitment: [u8; 32],
    groups: Vec<Vec<GuardedStorageEdit>>,
    audit_requests: AuditRequests,
    terminal: TerminalAudit,
) -> Result<CompiledWork> {
    terminal
        .validate_bounds()
        .map_err(|e| format!("terminal audit: {e:?}"))?;
    let mut seen = BTreeSet::new();
    let mut ordered = BTreeMap::new();
    for mut group in groups {
        if group.is_empty() {
            return Err("empty atomic edit group".into());
        }
        group.sort_by_key(|e| location(&e.scope, &e.key));
        let mut old_bytes = 0usize;
        for edit in &group {
            let old = read(storage, &edit.scope, &edit.key);
            if old.map(|v| hash(v)) != edit.before {
                return Err("edit does not match captured checkpoint preimage".into());
            }
            old_bytes = old_bytes
                .checked_add(old.map_or(0, Vec::len))
                .ok_or("read budget overflow")?;
            if old.map(Vec::as_slice) == edit.after.as_deref() {
                return Err("no-op edit requires no recovery mutation".into());
            }
            if !seen.insert(location(&edit.scope, &edit.key)) {
                return Err(
                    "overlapping adapters touch the same key; resolve semantics before composing"
                        .into(),
                );
            }
        }
        let probe = RecoveryChunk {
            index: 0,
            edits: group.clone(),
        };
        probe
            .validate()
            .map_err(|e| format!("atomic group cannot execute: {e:?}"))?;
        if old_bytes > MAX_CHUNK_BYTES {
            return Err("atomic group exceeds aggregate old-value read budget".into());
        }
        ordered.insert(location(&group[0].scope, &group[0].key), (group, old_bytes));
    }
    if ordered.is_empty() {
        return Err("no reviewed storage transformations; incoming-only recovery requires its dedicated compiler".into());
    }
    let mut chunks = Vec::new();
    let mut current = RecoveryChunk {
        index: 0,
        edits: Vec::new(),
    };
    let mut old_bytes = 0usize;
    for (_, (group, bytes)) in ordered {
        let mut probe = current.clone();
        probe.edits.extend(group.clone());
        if !current.edits.is_empty()
            && (probe.validate().is_err() || old_bytes + bytes > MAX_CHUNK_BYTES)
        {
            chunks.push(current);
            current = RecoveryChunk {
                index: chunks.len() as u32,
                edits: Vec::new(),
            };
            old_bytes = 0;
        }
        current.edits.extend(group);
        old_bytes += bytes;
    }
    chunks.push(current);
    if chunks.len() > MAX_WORK_CHUNKS {
        return Err("too many recovery chunks".into());
    }
    let mut witnesses = BTreeMap::new();
    for edit in chunks.iter().flat_map(|c| &c.edits) {
        witnesses.insert(
            location(&edit.scope, &edit.key),
            (
                StorageWitness {
                    scope: edit.scope.clone(),
                    key: edit.key.clone(),
                    value_hash: edit.after.as_deref().map(hash),
                },
                edit.after.as_ref().map_or(0, Vec::len),
            ),
        );
    }
    for witness in audit_requests.keys {
        let key = location(&witness.scope, &witness.key);
        if let Some((existing, _)) = witnesses.get(&key) {
            if existing != &witness {
                return Err("additional audit conflicts with projected storage edit".into());
            }
        } else {
            let value = read(storage, &witness.scope, &witness.key);
            if value.map(|v| hash(v)) != witness.value_hash {
                return Err("additional audit does not match retained checkpoint value".into());
            }
            witnesses.insert(key, (witness, value.map_or(0, Vec::len)));
        }
    }
    let mut audits = Vec::new();
    let mut audit = RecoveryAuditChunk {
        index: 0,
        witnesses: Vec::new(),
    };
    let mut read_bytes = 0usize;
    for (_, (witness, bytes)) in witnesses {
        if bytes > MAX_CHUNK_BYTES {
            return Err("audit value exceeds read budget".into());
        }
        let mut probe = audit.clone();
        probe.witnesses.push(witness.clone());
        if !audit.witnesses.is_empty()
            && (probe.validate().is_err() || read_bytes + bytes > MAX_CHUNK_BYTES)
        {
            audits.push(audit);
            audit = RecoveryAuditChunk {
                index: audits.len() as u32,
                witnesses: Vec::new(),
            };
            read_bytes = 0;
        }
        audit.witnesses.push(witness);
        read_bytes += bytes;
    }
    audits.push(audit);
    let mut prefix_locations = BTreeMap::new();
    for witness in audit_requests.empty_prefixes {
        prefix_locations.insert(location(&witness.scope, &witness.prefix), witness);
    }
    let mut previous: Option<Location> = None;
    for key in prefix_locations.keys() {
        if previous
            .as_ref()
            .is_some_and(|prior| prior.0 == key.0 && key.1.starts_with(&prior.1))
        {
            return Err("overlapping empty-prefix audit scopes".into());
        }
        previous = Some(key.clone());
    }
    let mut prefix_audits = Vec::new();
    let mut current = RecoveryPrefixAuditChunk {
        index: audits.len() as u32,
        prefixes: vec![],
    };
    for (_, witness) in prefix_locations {
        let mut probe = current.clone();
        probe.prefixes.push(witness.clone());
        if !current.prefixes.is_empty() && probe.validate().is_err() {
            prefix_audits.push(current);
            current = RecoveryPrefixAuditChunk {
                index: (audits.len() + prefix_audits.len()) as u32,
                prefixes: vec![],
            };
        }
        current.prefixes.push(witness);
        current
            .validate()
            .map_err(|error| format!("prefix audit bounds: {error:?}"))?;
    }
    if !current.prefixes.is_empty() {
        prefix_audits.push(current);
    }
    let policy = RecoveryPolicy {
        format_version: 1,
        checkpoint_hash: recovery::CHECKPOINT_HASH,
        checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
        replacements,
        council,
        chunk_hashes: chunks.iter().map(RecoveryChunk::commitment).collect(),
        audit_chunk_hashes: audits
            .iter()
            .map(RecoveryAuditChunk::commitment)
            .chain(
                prefix_audits
                    .iter()
                    .map(RecoveryPrefixAuditChunk::commitment),
            )
            .collect(),
        evidence_commitment,
        terminal_audit_commitment: terminal.commitment(),
    };
    policy
        .validate()
        .map_err(|e| format!("invalid composed policy: {e:?}"))?;
    // Run exactly the shared engine, including terminal conservation checks.
    // No mocked success flags or caller-supplied state-root assertions are used.
    let mut ext = sp_io::TestExternalities::new_with_code_and_state(
        storage
            .top
            .get(b":code".as_slice())
            .map(Vec::as_slice)
            .unwrap_or_default(),
        storage.clone(),
        StateVersion::V0,
    );
    ext.execute_with(|| -> Result<()> {
        recovery::storage::initialize(&policy).map_err(|e| format!("initialize: {e:?}"))?;
        for chunk in &chunks {
            recovery::storage::apply_chunk(&policy, chunk)
                .map_err(|e| format!("chunk {}: {e:?}", chunk.index))?;
        }
        for audit in &audits {
            recovery::storage::audit_chunk(&policy, audit)
                .map_err(|e| format!("audit {}: {e:?}", audit.index))?;
        }
        for audit in &prefix_audits {
            recovery::storage::audit_prefix_chunk(&policy, audit)
                .map_err(|e| format!("prefix audit {}: {e:?}", audit.index))?;
        }
        recovery::invariants::verify_and_record(&policy, &terminal)
            .map_err(|e| format!("terminal: {e:?}"))?;
        recovery::storage::mark_complete(&policy).map_err(|e| format!("complete: {e:?}"))?;
        Ok(())
    })?;
    let report = WorkReport {
        schema: "sora-nexus-storage-work-v1",
        policy_scale: hx(&policy.encode()), policy_commitment: hx(&policy.commitment()),
        apply_envelopes: chunks.iter().cloned().map(|c| envelope(RecoveryAction::Apply(c))).collect::<Result<_>>()?,
        audit_envelopes: audits.iter().cloned().map(RecoveryAction::Audit)
            .chain(prefix_audits.iter().cloned().map(RecoveryAction::PrefixAudit)).map(envelope).collect::<Result<_>>()?,
        terminal_envelope: envelope(RecoveryAction::TerminalAudit(terminal.clone()))?,
        changed_keys: seen.len(), storage_engine_rehearsed: true, production_ready: false,
        remaining: "Bind and rehearse exact consensus/runtime artifacts, incoming/outgoing reconciliation, entitlement ledger and all release evidence; storage composition alone does not authorize recovery",
    };
    Ok(CompiledWork {
        policy,
        chunks,
        audits,
        prefix_audits,
        terminal,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use recovery::{
        invariants::{IssuanceInvariant, StoredAmount},
        MAX_EDITS_PER_CHUNK,
    };
    fn fixture() -> (Storage, Vec<GuardedStorageEdit>, TerminalAudit) {
        let storage = Storage {
            top: BTreeMap::from([
                (b"supply".to_vec(), 10u128.encode()),
                (b"old".to_vec(), 10u128.encode()),
            ]),
            ..Default::default()
        };
        let edits = vec![
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: b"old".to_vec(),
                before: Some(hash(&10u128.encode())),
                after: None,
            },
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: b"new".to_vec(),
                before: None,
                after: Some(10u128.encode()),
            },
        ];
        let terminal = TerminalAudit {
            issuance: vec![IssuanceInvariant {
                asset: [1; 32],
                current: StoredAmount {
                    scope: StorageScope::Top,
                    key: b"supply".to_vec(),
                    record_hash: hash(&10u128.encode()),
                    offset: 0,
                },
                checkpoint_issuance: 10,
                authenticated_mints: 0,
                burned: 0,
            }],
            collateral: vec![],
            zero_claims: vec![],
            absent: vec![],
        };
        (storage, edits, terminal)
    }
    fn run(
        storage: &Storage,
        groups: Vec<Vec<GuardedStorageEdit>>,
        terminal: TerminalAudit,
    ) -> Result<CompiledWork> {
        compose(
            storage,
            vec![],
            vec![AccountId32::from([9; 32])],
            [8; 32],
            groups,
            vec![],
            terminal,
        )
    }
    #[test]
    fn prefix_audits_share_cursor_and_rehearse_the_complete_namespace() {
        let (storage, edits, terminal) = fixture();
        let compile = |prefixes| {
            compose_with_audits(
                &storage,
                vec![],
                vec![AccountId32::from([9; 32])],
                [8; 32],
                vec![edits.clone()],
                AuditRequests {
                    keys: vec![],
                    empty_prefixes: prefixes,
                },
                terminal.clone(),
            )
        };
        let witness = |key: &[u8]| StoragePrefixWitness {
            scope: StorageScope::Top,
            prefix: key.to_vec(),
        };
        let result = compile(vec![witness(b"old")]).unwrap();
        assert_eq!(result.prefix_audits.len(), 1);
        assert_eq!(result.prefix_audits[0].index as usize, result.audits.len());
        assert_eq!(
            result.policy.audit_chunk_hashes.last(),
            Some(&result.prefix_audits[0].commitment())
        );
        assert!(compile(vec![witness(b"new")]).is_err());
        assert!(compile(vec![witness(b"absent"), witness(b"absent/child")]).is_err());
        assert_eq!(
            compile(vec![witness(b"old"), witness(b"old")])
                .unwrap()
                .policy,
            result.policy
        );
    }

    #[test]
    fn move_is_atomic_rehearsed_and_deterministic() {
        let (storage, edits, terminal) = fixture();
        let result = run(&storage, vec![edits.clone()], terminal.clone()).unwrap();
        let mut reversed = edits;
        reversed.reverse();
        assert_eq!(
            result.policy,
            run(&storage, vec![reversed], terminal).unwrap().policy
        );
        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.chunks[0].edits.len(), 2);
        assert!(!result.report.production_ready);
    }
    #[test]
    fn collisions_occupied_destination_and_terminal_mismatch_reject() {
        let (mut storage, edits, terminal) = fixture();
        assert!(run(
            &storage,
            vec![edits.clone(), edits.clone()],
            terminal.clone()
        )
        .is_err());
        let mut bad = terminal.clone();
        bad.issuance[0].burned = 1;
        assert!(run(&storage, vec![edits.clone()], bad).is_err());
        storage.top.insert(b"new".to_vec(), vec![0]);
        assert!(run(&storage, vec![edits], terminal).is_err());
    }
    #[test]
    fn oversized_atomic_group_is_not_split_to_evade_limits() {
        let (mut storage, _, terminal) = fixture();
        let mut edits = Vec::new();
        for i in 0..=MAX_EDITS_PER_CHUNK {
            let key = (i as u32).encode();
            storage.top.insert(key.clone(), vec![1]);
            edits.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key,
                before: Some(hash(&[1])),
                after: None,
            });
        }
        assert!(run(&storage, vec![edits], terminal).is_err());
    }
    #[test]
    fn old_value_read_budget_is_independent_of_small_delete_envelope() {
        let (mut storage, mut edits, terminal) = fixture();
        let large = vec![3; MAX_CHUNK_BYTES + 1];
        storage.top.insert(b"old".to_vec(), large.clone());
        edits[0].before = Some(hash(&large));
        assert!(run(&storage, vec![edits], terminal).is_err());
    }

    #[test]
    fn exact_audits_pack_independently_of_write_count_and_preserve_every_witness() {
        let (mut storage, edits, terminal) = fixture();
        let mut keys = Vec::new();
        for index in 0..recovery::MAX_AUDIT_WITNESSES {
            let key = (b"retained", index as u32).encode();
            storage.top.insert(key.clone(), vec![7]);
            keys.push(StorageWitness {
                scope: StorageScope::Top,
                key,
                value_hash: Some(hash(&[7])),
            });
        }
        let result = compose_with_audits(
            &storage,
            vec![],
            vec![AccountId32::from([9; 32])],
            [8; 32],
            vec![edits],
            AuditRequests {
                keys,
                empty_prefixes: vec![],
            },
            terminal,
        )
        .unwrap();
        assert_eq!(result.chunks.len(), 1);
        assert_eq!(result.audits.len(), 2);
        assert_eq!(
            result.audits[0].witnesses.len(),
            recovery::MAX_AUDIT_WITNESSES
        );
        assert_eq!(
            result
                .audits
                .iter()
                .map(|audit| audit.witnesses.len())
                .sum::<usize>(),
            recovery::MAX_AUDIT_WITNESSES + 2
        );
        assert!(result.report.storage_engine_rehearsed);
    }
}
