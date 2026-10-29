//! Offline exact-Wasm maintenance authoring, independent import and disk restart.
//! This does not certify BABE seals, GRANDPA finality or operator approval.

use crate::{hex_bytes, local_path, replay, verify_checkpoint, Result};
use codec::{Compact, DecodeAll, DecodeWithMemLimit, Encode};
use common::recovery::{self, RecoveryInherent, RecoveryPhase, RecoveryPolicy, RecoveryStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sp_core::{storage::StateVersion, H256};
use sp_runtime::{
    generic::{Block, Digest},
    traits::Header as _,
    ApplyExtrinsicResult, DigestItem, ExtrinsicInclusionMode, OpaqueExtrinsic,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

type Header = replay::ReplayHeader;
type RuntimeBlock = Block<Header, OpaqueExtrinsic>;
#[path = "maintenance_activation.rs"]
mod activation;
#[cfg(feature = "runtime-work-types")]
#[path = "maintenance_bridge_state.rs"]
mod bridge_state;
#[path = "maintenance_expectations.rs"]
mod expectations;
#[path = "maintenance_fixture.rs"]
pub(crate) mod fixture;
#[path = "maintenance_hashi_fixture.rs"]
pub(crate) mod hashi_fixture;
#[path = "maintenance_historical_audit.rs"]
mod historical_audit;
#[path = "maintenance_native_expectations.rs"]
mod native_expectations;
#[path = "maintenance_native_expectations_v2.rs"]
mod native_expectations_v2;
#[path = "maintenance_native_lane.rs"]
mod native_lane;
#[cfg(feature = "runtime-work-types")]
#[path = "maintenance_position_conservation.rs"]
mod position_conservation;
#[cfg(feature = "bridge-fixture")]
#[path = "maintenance_post_complete.rs"]
pub mod post_complete;
#[cfg(feature = "bridge-fixture")]
#[path = "maintenance_post_complete_channel.rs"]
pub mod post_complete_channel;
#[cfg(all(unix, feature = "runtime-work-types"))]
#[path = "maintenance_process_death.rs"]
pub mod process_death;
#[cfg(feature = "bridge-fixture")]
#[path = "maintenance_signed_fixture.rs"]
mod signed_fixture;
#[path = "maintenance_snapshot.rs"]
mod snapshot;
#[path = "maintenance_state_observer.rs"]
mod state_observer;
#[path = "maintenance_storage_semantics.rs"]
mod storage_semantics;
#[path = "maintenance_supplied_escape.rs"]
mod supplied_escape;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step {
    pub slot: u64,
    pub timestamp: u64,
    pub authority_index: u32,
    /// Complete encode_remark bytes. None selects an ordinary post-138 block.
    pub recovery: Option<replay::Artifact>,
    /// Available only under the conspicuous public test-council fixture schema.
    pub fixture_escape: Option<fixture::Escape>,
    /// Generate only public fixture HASHI signature witnesses from actual state.
    #[serde(default)]
    pub fixture_hashi: bool,
    #[serde(default)]
    pub reject_previous_escapes: bool,
    /// Valid envelopes expected to fail before committing any state mutation.
    #[serde(default)]
    pub rejected_recovery: Vec<replay::Artifact>,
    pub expected_spec: u32,
    pub expected_phase: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub checkpoint_hash: String,
    pub checkpoint_capture: String,
    pub policy: replay::Artifact,
    pub bootstrap: replay::Artifact,
    pub hold: replay::Artifact,
    pub final_runtime: replay::Artifact,
    #[serde(default)]
    pub repairs: Vec<replay::Artifact>,
    pub previous_grandpa_set: u64,
    /// Exact ordered active fresh keys and preserved weights; public fixtures
    /// are supported but do not establish operator approval.
    pub fresh_grandpa: Vec<(String, u64)>,
    pub fixture_council: Option<fixture::Council>,
    /// Raw artifact-pinned signatures and an explicit non-fixture stopping state.
    pub supplied_council: Option<supplied_escape::Scenario>,
    /// Separate public mixed-account forward-repair fixture; raw public signatures only.
    pub public_dynamic_repair: Option<supplied_escape::Scenario>,
    pub fixture_hashi_peers: Option<hashi_fixture::Config>,
    /// Outer-bound finite raw-state expectations; explicitly public fixtures only.
    pub fixture_expectations: Option<replay::Artifact>,
    /// Fresh derivation by an explicitly pinned preserved-bootstrap native binary.
    pub native_operation_recorder: Option<native_expectations::Config>,
    #[serde(default)]
    pub native_operation_recorders_v2: Vec<native_expectations_v2::Config>,
    /// Separate opt-in public-key staking plan. No default replay path signs.
    pub fixture_signed_transactions: Option<replay::Artifact>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockWitness {
    pub number: u32,
    pub hash: String,
    pub state_root: String,
    pub executing_code_blake2: String,
    pub resulting_spec: u32,
    pub phase: String,
    pub cursor: u32,
    pub operation_counter: u64,
    pub block_sha256: String,
    pub disk_snapshot_manifest_sha256: String,
    pub forbidden_extrinsics_rejected: usize,
    pub maintenance_session_frozen: bool,
    pub independent_import_root_matches: bool,
    pub disk_restart_root_matches: bool,
    pub generated_authorization_sha256: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: &'static str,
    pub manifest_sha256: String,
    pub capture_manifest_sha256: String,
    pub policy_commitment: String,
    pub artifact_sha256: BTreeMap<String, String>,
    pub blocks: Vec<BlockWitness>,
    pub automatic_activation_observed: bool,
    pub council_hold_observed: bool,
    pub forward_repair_observed: bool,
    pub ordinary_runtime_blocks: usize,
    pub liveness_grace_observed: bool,
    pub fee_clock_shift_observed: bool,
    pub bridge_halt_retained: bool,
    /// Any native operation derivation ran; authoring and import remain exact Wasm.
    pub native_execution: bool,
    pub consensus_seals_verified: bool,
    pub operator_approval_verified: bool,
    pub production_ready: bool,
    pub public_test_council_fixture: bool,
    pub public_test_hashi_fixture: bool,
    pub fixture_governance_state_unchanged: Option<bool>,
    pub fixture_state_expectation_checks: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supplied_council: Option<supplied_escape::Report>,
    pub native_operation_derivations: Vec<native_expectations::Derivation>,
    pub repaired_native_operation_derivations: Vec<native_expectations_v2::Derivation>,
    pub replay_transcript_sha256: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prior_policy_incoming_audits: Vec<serde_json::Value>,
    /// Diagnostic binding from the same opaque completed in-process certificate.
    /// Never deserialized to admit recovery state or external-finality claims.
    #[cfg(feature = "runtime-work-types")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incoming_replay_binding: Option<crate::incoming_replay::CoverageBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_signed_transactions: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_state_semantics: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_validation: Option<serde_json::Value>,
}

fn value(ext: &mut sp_io::TestExternalities, key: &[u8]) -> Option<Vec<u8>> {
    ext.execute_with(|| sp_io::storage::get(key).map(|bytes| bytes.to_vec()))
}

fn recovery_anchor(ext: &mut sp_io::TestExternalities) -> Result<Option<[u8; 32]>> {
    value(ext, recovery::RECOVERY_ANCHOR_STORAGE_KEY)
        .map(|bytes| <[u8; 32]>::decode_all(&mut bytes.as_slice()).map_err(|e| e.to_string()))
        .transpose()
}

fn storage_key(pallet: &[u8], item: &[u8]) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(pallet),
        sp_io::hashing::twox_128(item),
    ]
    .concat()
}

fn status(ext: &mut sp_io::TestExternalities) -> Result<RecoveryStatus> {
    let bytes = value(ext, recovery::STATUS_STORAGE_KEY).ok_or("missing recovery status")?;
    RecoveryStatus::decode_all(&mut bytes.as_slice()).map_err(|error| error.to_string())
}

fn bare(call: &[u8]) -> Result<OpaqueExtrinsic> {
    let mut payload = vec![4u8];
    payload.extend(call);
    OpaqueExtrinsic::try_from_encoded_extrinsic(&payload.encode())
        .map_err(|error| error.to_string())
}

fn remark(bytes: &[u8]) -> Result<OpaqueExtrinsic> {
    let mut call = vec![0, 0];
    call.extend(bytes.encode());
    bare(&call)
}

fn timestamp(now: u64) -> Result<OpaqueExtrinsic> {
    let mut call = vec![1, 0];
    call.extend(Compact(now).encode());
    bare(&call)
}

fn apply(
    executor: &replay::Executor,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
    extrinsic: &OpaqueExtrinsic,
) -> Result<ApplyExtrinsicResult> {
    let encoded = replay::call(
        executor,
        ext,
        code,
        "BlockBuilder_apply_extrinsic",
        &extrinsic.encode(),
    )?;
    ApplyExtrinsicResult::decode_all(&mut encoded.as_slice()).map_err(|error| error.to_string())
}

fn reject(
    executor: &replay::Executor,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
    extrinsic: &OpaqueExtrinsic,
) -> Result<()> {
    let before = replay::root(ext);
    if apply(executor, ext, code, extrinsic)?.is_ok() || replay::root(ext) != before {
        return Err("forbidden maintenance extrinsic was admitted or changed state".into());
    }
    Ok(())
}

/// Check whole-block write confinement against the independent parent state.
/// Authenticated incoming routines have separate financial-effects gates.
fn confined_writes(
    ext: &mut sp_io::TestExternalities,
    parent: &mut sp_io::TestExternalities,
    action: &recovery::RecoveryAction,
) -> Result<()> {
    if matches!(
        action,
        recovery::RecoveryAction::Incoming(_) | recovery::RecoveryAction::Outgoing(_)
    ) {
        return Err("dynamic actions require complete non-consensus write expectations".into());
    }
    let mut top = BTreeSet::new();
    let mut children = BTreeSet::new();
    let incoming_audit_keys: Option<BTreeSet<Vec<u8>>> =
        if let recovery::RecoveryAction::IncomingAudit(bytes) = action {
            #[cfg(not(feature = "runtime-work-types"))]
            {
                let _ = bytes;
                return Err("IncomingAudit confinement requires runtime-work-types".into());
            }
            #[cfg(feature = "runtime-work-types")]
            {
                use framenode_runtime::incident_recovery_incoming::{
                    audit_marker_key, IncomingAudit,
                };
                if bytes.len() > recovery::MAX_CHUNK_BYTES {
                    return Err("incoming audit exceeds bounded transport".into());
                }
                let mut input = bytes.as_slice();
                let audit =
                    IncomingAudit::decode_with_mem_limit(&mut input, recovery::MAX_CHUNK_BYTES * 4)
                        .map_err(|error| format!("incoming audit decode: {error}"))?;
                if !input.is_empty() || audit.encode() != *bytes {
                    return Err("noncanonical incoming audit".into());
                }
                audit
                    .validate()
                    .map_err(|error| format!("incoming audit shape: {error:?}"))?;
                let active: [u8; 32] = value(parent, recovery::ACTIVE_POLICY_STORAGE_KEY)
                    .ok_or("incoming audit parent active policy absent")?
                    .try_into()
                    .map_err(|_| "incoming audit parent active policy width")?;
                let before = status(parent)?;
                let after = status(ext)?;
                if before.phase != RecoveryPhase::Auditing {
                    return Err("incoming audit parent is not auditing".into());
                }
                let keys = if after.phase == RecoveryPhase::Failed {
                    let mut frozen = before;
                    frozen.phase = RecoveryPhase::Failed;
                    frozen.failure = after.failure;
                    if after.failure.is_none() || frozen != after {
                        return Err("incoming audit failure changed lineage/counter/cursor".into());
                    }
                    // A committed mismatch can freeze recovery for council repair;
                    // failed audits must not partly advance any successful audit key.
                    BTreeSet::from([recovery::STATUS_STORAGE_KEY.to_vec()])
                } else {
                    if after != before {
                        return Err("incoming audit unexpectedly changed recovery status".into());
                    }
                    BTreeSet::from([
                        recovery::AUDIT_CURSOR_STORAGE_KEY.to_vec(),
                        recovery::INCOMING_AUDIT_PENDING_KEY.to_vec(),
                        audit_marker_key(
                            audit.prior_policy.unwrap_or(active),
                            audit.operation_commitment,
                        ),
                    ])
                };
                Some(keys)
            }
        } else if let recovery::RecoveryAction::NativeBalanceAudit(bytes) = action {
            #[cfg(not(feature = "runtime-work-types"))]
            {
                let _ = bytes;
                return Err("native balance audit requires runtime-work-types".into());
            }
            #[cfg(feature = "runtime-work-types")]
            {
                use framenode_runtime::incident_recovery_incoming::NativeBalanceAudit;
                let mut input = bytes.as_slice();
                let audit = NativeBalanceAudit::decode_with_mem_limit(
                    &mut input,
                    recovery::MAX_CHUNK_BYTES * 4,
                )
                .map_err(|e| format!("native balance audit decode: {e}"))?;
                if !input.is_empty() || audit.encode() != *bytes {
                    return Err("noncanonical native balance audit".into());
                }
                audit
                    .validate()
                    .map_err(|e| format!("native balance audit shape: {e:?}"))?;
                let before = status(parent)?;
                let after = status(ext)?;
                if before.phase != RecoveryPhase::Auditing {
                    return Err("native balance audit parent is not auditing".into());
                }
                let keys = if after.phase == RecoveryPhase::Failed {
                    let mut frozen = before;
                    frozen.phase = RecoveryPhase::Failed;
                    frozen.failure = after.failure;
                    if after.failure.is_none() || frozen != after {
                        return Err(
                            "native balance audit failure changed lineage/counter/cursor".into(),
                        );
                    }
                    BTreeSet::from([recovery::STATUS_STORAGE_KEY.to_vec()])
                } else {
                    if after != before {
                        return Err("native balance audit changed recovery status".into());
                    }
                    BTreeSet::from([recovery::AUDIT_CURSOR_STORAGE_KEY.to_vec()])
                };
                Some(keys)
            }
        } else {
            None
        };
    if let recovery::RecoveryAction::Apply(chunk) = action {
        for edit in &chunk.edits {
            match &edit.scope {
                recovery::StorageScope::Top => {
                    top.insert(edit.key.clone());
                }
                recovery::StorageScope::Child(child) => {
                    children.insert((child.clone(), edit.key.clone()));
                }
            }
        }
    }
    let system_account = storage_key(b"System", b"Account");
    let consensus: Vec<_> = [
        b"System".as_slice(),
        b"Babe",
        b"Grandpa",
        b"Session",
        b"Timestamp",
    ]
    .iter()
    .map(|pallet| sp_io::hashing::twox_128(pallet))
    .collect();
    let keys: Vec<_> = ext
        .overlayed_changes()
        .changes()
        .map(|(key, _)| key.clone())
        .collect();
    for key in keys {
        let permitted = top.contains(&key)
            || incoming_audit_keys.as_ref().map_or_else(
                || key.starts_with(recovery::STORAGE_PREFIX),
                |allowed| allowed.contains(&key),
            )
            || (incoming_audit_keys.is_none() && key == b":code")
            || key == b":extrinsic_index"
            || (!key.starts_with(&system_account)
                && consensus.iter().any(|prefix| key.starts_with(prefix)));
        if !permitted && value(ext, &key) != value(parent, &key) {
            return Err(format!(
                "uncommitted economic write during maintenance: 0x{}",
                hex::encode(key)
            ));
        }
    }
    let keys: Vec<_> = ext
        .overlayed_changes()
        .children()
        .flat_map(|(changes, info)| {
            changes
                .map(|(key, _)| (info.storage_key().to_vec(), key.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    for (child, key) in keys {
        let before = parent.execute_with(|| sp_io::default_child_storage::get(&child, &key));
        let after = ext.execute_with(|| sp_io::default_child_storage::get(&child, &key));
        if before != after && !children.contains(&(child, key)) {
            return Err("uncommitted child-storage write during maintenance".into());
        }
    }
    Ok(())
}

fn select_code<'a>(
    stored: &[u8],
    deployed: &[u8],
    bootstrap: &'a [u8],
    known: &'a BTreeMap<Vec<u8>, Vec<u8>>,
) -> Result<&'a [u8]> {
    if stored == deployed {
        return Ok(bootstrap);
    }
    known
        .get(&sp_io::hashing::blake2_256(stored).to_vec())
        .map(Vec::as_slice)
        .ok_or_else(|| "maintenance installed an artifact outside the pinned manifest".into())
}

/// The output directory must be new. Only files owned by this rehearsal are
/// written; input artifacts, captured state and node databases are never edited.
#[cfg(feature = "runtime-work-types")]
type IncomingEvidence = crate::incoming_replay::VerifiedReplay;
#[cfg(not(feature = "runtime-work-types"))]
type IncomingEvidence = ();

pub fn run(manifest_path: &Path, output: &Path) -> Result<Report> {
    run_inner(manifest_path, output, false, false, false).map(|(report, _)| report)
}

/// Re-execute the pinned recovery prefix and return an opaque, same-process
/// certificate. Reports and caller-supplied snapshots cannot replace this run.
#[cfg(feature = "runtime-work-types")]
pub fn run_with_incoming_evidence(
    manifest_path: &Path,
    output: &Path,
) -> Result<(Report, crate::incoming_replay::VerifiedReplay)> {
    run_inner(manifest_path, output, true, false, false)
}

/// Normal nonfixture activation with same-process, final-state semantic checks.
/// Neither archived reports nor a caller-supplied final snapshot can enter here.
pub fn run_with_storage_semantics(manifest_path: &Path, output: &Path) -> Result<Report> {
    run_inner(manifest_path, output, false, true, false).map(|(report, _)| report)
}

/// Explicit public-key fixture only. Inspect the first ordinary final138 block
/// before any signed staking transaction, then finish the entire signed replay.
#[cfg(feature = "bridge-fixture")]
pub fn run_public_storage_semantics(manifest_path: &Path, output: &Path) -> Result<Report> {
    run_inner(manifest_path, output, false, false, true).map(|(report, _)| report)
}

fn inspect_public_storage_block(
    requested: bool,
    already_inspected: bool,
    maintenance: bool,
    ordinary_blocks: usize,
    extrinsics: usize,
) -> Result<bool> {
    if !requested || already_inspected || maintenance {
        return Ok(false);
    }
    if ordinary_blocks != 1 || extrinsics != 1 {
        return Err("public storage diagnostic requires the first ordinary138 block to contain only timestamp; reserve it before all signed calls".into());
    }
    Ok(true)
}

fn run_inner(
    manifest_path: &Path,
    output: &Path,
    request_incoming_evidence: bool,
    request_storage_semantics: bool,
    request_public_storage_semantics: bool,
) -> Result<(Report, IncomingEvidence)> {
    let base = manifest_path
        .parent()
        .ok_or("manifest has no parent directory")?;
    const MAX_MANIFEST: u64 = 64 * 1024 * 1024;
    if fs::metadata(manifest_path)
        .map_err(|error| error.to_string())?
        .len()
        > MAX_MANIFEST
    {
        return Err("maintenance manifest exceeds 64 MiB".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(manifest_path)
        .map_err(|error| error.to_string())?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("maintenance manifest exceeds 64 MiB".into());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if request_storage_semantics
        && (manifest.schema != "sora-nexus-maintenance-replay-v1"
            || !manifest.repairs.is_empty()
            || manifest.supplied_council.is_some())
    {
        return Err(
            "storage semantics requires ordinary nonfixture activation without repair".into(),
        );
    }
    if request_storage_semantics && !cfg!(feature = "runtime-work-types") {
        return Err("real bridge state verification requires runtime-work-types".into());
    }
    let signed_fixture_only = manifest.schema == "sora-nexus-public-fixture-signed-maintenance-v1";
    if request_public_storage_semantics && (!signed_fixture_only || request_storage_semantics) {
        return Err(
            "public storage diagnostic admits only the explicit public signed fixture schema"
                .into(),
        );
    }
    let fixture_only = manifest.schema == fixture::SCHEMA;
    let hashi_fixture_only = manifest.schema == hashi_fixture::SCHEMA;
    let public_dynamic_only = manifest.schema == supplied_escape::PUBLIC_DYNAMIC_SCHEMA;
    let public_historical_audit_only = manifest.schema == historical_audit::SCHEMA;
    let public_prior_audit_only =
        manifest.schema == native_lane::PUBLIC_PRIOR_AUDIT_SCHEMA || public_historical_audit_only;
    let public_repair_fixture = public_dynamic_only || public_prior_audit_only;
    let supplied_council_only = manifest.schema == supplied_escape::SCHEMA;
    if request_incoming_evidence && !supplied_council_only && !public_repair_fixture {
        return Err("prior incoming coverage requires an authenticated supplied-council or explicit public repair replay".into());
    }
    let allow_dual_lanes = public_prior_audit_only
        || (request_incoming_evidence
            && supplied_council_only
            && manifest.native_operation_recorder.is_some()
            && !manifest.native_operation_recorders_v2.is_empty());
    if (manifest.schema != "sora-nexus-maintenance-replay-v1"
        && !fixture_only
        && !hashi_fixture_only
        && !supplied_council_only
        && !public_repair_fixture
        && !signed_fixture_only)
        || signed_fixture_only != manifest.fixture_signed_transactions.is_some()
        || (signed_fixture_only
            && (!cfg!(feature = "bridge-fixture")
                || manifest.native_operation_recorder.is_none()
                || !manifest.native_operation_recorders_v2.is_empty()
                || !manifest.repairs.is_empty()
                || manifest.fixture_expectations.is_some()))
        || hex_bytes(&manifest.checkpoint_hash)?.as_slice() != recovery::CHECKPOINT_HASH
        || manifest.steps.is_empty()
        || manifest.steps.len() > 100_000
        || manifest.repairs.len() > 2
        || manifest.fresh_grandpa.is_empty()
        || manifest.fresh_grandpa.len() > 1000
        || fixture_only != manifest.fixture_council.is_some()
        || supplied_council_only != manifest.supplied_council.is_some()
        || public_repair_fixture != manifest.public_dynamic_repair.is_some()
        || manifest.native_operation_recorders_v2.len() > 2
        || (!manifest.native_operation_recorders_v2.is_empty()
            && ((manifest.native_operation_recorder.is_some() && !allow_dual_lanes)
                || (!supplied_council_only && !public_repair_fixture)))
        || (public_repair_fixture
            && (manifest.native_operation_recorders_v2.is_empty()
                || manifest.fixture_expectations.is_some()))
        || (public_prior_audit_only
            && (manifest.native_operation_recorder.is_none()
                || !cfg!(feature = "runtime-work-types")))
        || (supplied_council_only && manifest.fixture_expectations.is_some())
        || (hashi_fixture_only && manifest.fixture_hashi_peers.is_none())
        || (!hashi_fixture_only && !signed_fixture_only && manifest.fixture_hashi_peers.is_some())
        || (hashi_fixture_only
            && (!cfg!(feature = "bridge-fixture")
                || manifest.fixture_expectations.is_none()
                || !manifest.steps.iter().any(|s| s.fixture_hashi)))
        || manifest.steps.iter().any(|step| {
            (step.fixture_escape.is_some() && step.recovery.is_some())
                || (step.fixture_hashi
                    && ((!hashi_fixture_only && !signed_fixture_only)
                        || manifest.fixture_hashi_peers.is_none()
                        || step.recovery.is_none()
                        || step.fixture_escape.is_some()))
                || (!fixture_only && step.fixture_escape.is_some())
                || (!fixture_only
                    && !supplied_council_only
                    && !public_repair_fixture
                    && step.reject_previous_escapes)
        })
    {
        return Err("invalid maintenance manifest identity or bounds".into());
    }
    let policy_bytes = replay::artifact(base, &manifest.policy, recovery::MAX_INHERENT_BYTES * 3)?;
    let mut policy_input = policy_bytes.as_slice();
    let policy =
        RecoveryPolicy::decode_with_mem_limit(&mut policy_input, recovery::MAX_INHERENT_BYTES * 6)
            .map_err(|error| error.to_string())?;
    if !policy_input.is_empty() {
        return Err("trailing policy bytes".into());
    }
    policy
        .validate()
        .map_err(|error| format!("invalid policy: {error:?}"))?;
    #[cfg(feature = "bridge-fixture")]
    let mut signed_session = manifest
        .fixture_signed_transactions
        .as_ref()
        .map(|reference| signed_fixture::Session::load(base, reference, &policy, &manifest.steps))
        .transpose()?;
    let mut supplied_council = manifest
        .supplied_council
        .as_ref()
        .map(|scenario| supplied_escape::Session::new(scenario, &policy, &manifest.steps))
        .transpose()?;
    if let Some(scenario) = &manifest.public_dynamic_repair {
        supplied_council = Some(supplied_escape::Session::public_dynamic(
            scenario,
            &policy,
            &manifest.steps,
        )?);
    }
    let mut historical_audit = if public_historical_audit_only {
        Some(historical_audit::Track::new(
            manifest
                .public_dynamic_repair
                .as_ref()
                .ok_or("historical scenario absent")?,
            &policy,
            manifest.repairs.len(),
            manifest.native_operation_recorders_v2.len(),
        )?)
    } else {
        None
    };
    if let Some(council) = &manifest.fixture_council {
        fixture::validate(base, council, &policy)?;
    }
    if let Some(config) = &manifest.fixture_hashi_peers {
        if signed_fixture_only {
            hashi_fixture::validate_funded(base, config, &policy)?;
        } else {
            hashi_fixture::validate(base, config, &policy)?;
        }
    }
    let fixture_expectations = manifest
        .fixture_expectations
        .as_ref()
        .map(|reference| {
            let bytes = replay::artifact(base, reference, 8 * 1024 * 1024)?;
            expectations::Expectations::decode(&bytes, &policy)
        })
        .transpose()?;
    let verified = verify_checkpoint(&local_path(base, &manifest.checkpoint_capture)?)?;
    let deployed = verified
        .storage
        .top
        .get(b":code".as_slice())
        .ok_or("missing checkpoint code")?
        .clone();
    let mut ext = sp_io::TestExternalities::new_with_code_and_state(
        &deployed,
        verified.storage,
        StateVersion::V0,
    );
    #[cfg(feature = "runtime-work-types")]
    let mut bridge_state = if request_storage_semantics {
        Some(bridge_state::Track::checkpoint(&mut ext, &policy)?)
    } else {
        None
    };
    let checkpoint_ledger = if request_storage_semantics || request_public_storage_semantics {
        Some(storage_semantics::capture_ledger(
            &mut ext,
            &verified.schema,
        )?)
    } else {
        None
    };
    #[cfg(feature = "runtime-work-types")]
    let mut position_conservation = if request_storage_semantics || request_public_storage_semantics
    {
        Some(position_conservation::Track::checkpoint(
            &mut ext,
            &verified.schema,
            &policy,
            request_public_storage_semantics,
        )?)
    } else {
        None
    };
    let executor = replay::Executor::builder().build();
    let import_executor = replay::Executor::builder().build();
    let bootstrap = replay::artifact(base, &manifest.bootstrap, 32 * 1024 * 1024)?;
    let native_recorder = manifest
        .native_operation_recorder
        .as_ref()
        .map(|config| {
            native_expectations::Recorder::load(
                base,
                config,
                &bootstrap,
                &policy_bytes,
                &policy,
                fixture_only
                    || hashi_fixture_only
                    || signed_fixture_only
                    || public_prior_audit_only
                    || fixture_expectations.is_some(),
            )
        })
        .transpose()?;
    let native_recorders_v2 = manifest
        .native_operation_recorders_v2
        .iter()
        .map(|config| {
            native_expectations_v2::Recorder::load(
                base,
                config,
                &policy_bytes,
                &policy,
                public_repair_fixture,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let mut native_parent = if native_recorder.is_some() || !native_recorders_v2.is_empty() {
        let capture = local_path(base, &manifest.checkpoint_capture)?;
        let header: Header = serde_json::from_slice(
            &fs::read(capture.join("checkpoint-header.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if header.hash() != H256::from(recovery::CHECKPOINT_HASH)
            || *header.state_root() != H256::from(recovery::CHECKPOINT_STATE_ROOT)
            || *header.number() != recovery::CHECKPOINT_NUMBER
        {
            return Err("native parent checkpoint header differs".into());
        }
        Some(header)
    } else {
        None
    };
    let mut native_derivations = Vec::new();
    let mut repaired_native_derivations = Vec::new();
    let mut repaired_dynamic_kinds = std::collections::BTreeSet::new();
    let mut original_dynamic_kinds = std::collections::BTreeSet::new();
    let mut replay_transcript =
        native_expectations::transcript_seed(&bytes, &verified.state.capture_manifest_sha256);
    let mut hashes = BTreeMap::new();
    if let Some(config) = &manifest.native_operation_recorder {
        hashes.insert(
            "native-operation-executable".into(),
            config.executable.sha256.clone(),
        );
        hashes.insert(
            "native-operation-engine-binding".into(),
            config.engine_binding.sha256.clone(),
        );
    }
    for (index, config) in manifest.native_operation_recorders_v2.iter().enumerate() {
        hashes.insert(
            format!("repaired-native-{index}-executable"),
            config.executable.sha256.clone(),
        );
        hashes.insert(
            format!("repaired-native-{index}-binding"),
            config.engine_binding.sha256.clone(),
        );
    }
    if let Some(reference) = &manifest.fixture_signed_transactions {
        hashes.insert(
            "public-signed-transaction-plan".into(),
            reference.sha256.clone(),
        );
    }
    if let Some(reference) = &manifest.fixture_expectations {
        hashes.insert(
            "fixture-state-expectations".into(),
            reference.sha256.clone(),
        );
    }
    if let Some(config) = &manifest.fixture_hashi_peers {
        hashes.insert(
            "public-hashi-provenance".into(),
            config.provenance.sha256.clone(),
        );
    }
    let mut known = BTreeMap::new();
    let mut by_version = BTreeMap::new();
    let mut final_code = Vec::new();
    let mut versions = std::collections::BTreeSet::new();
    for (role, reference, allowed) in [
        ("bootstrap", &manifest.bootstrap, vec![134]),
        ("hold", &manifest.hold, vec![135]),
        ("final", &manifest.final_runtime, vec![138]),
    ]
    .into_iter()
    .chain(
        manifest
            .repairs
            .iter()
            .map(|reference| ("repair", reference, vec![136, 137])),
    ) {
        let code = replay::artifact(base, reference, 32 * 1024 * 1024)?;
        let version = replay::version(&executor, &mut ext, &code)?;
        let transaction = match version.spec_version {
            134 => 131,
            138 => 134,
            _ => 132,
        };
        if !allowed.contains(&version.spec_version)
            || version.transaction_version != transaction
            || !versions.insert(version.spec_version)
        {
            return Err("wrong or duplicate actual Core_version of maintenance artifact".into());
        }
        hashes.insert(
            format!("{role}-{}", version.spec_version),
            reference.sha256.clone(),
        );
        if version.spec_version == 138 {
            final_code = code.clone();
        }
        if version.spec_version != 134 {
            by_version.insert(version.spec_version, code.clone());
            known.insert(sp_io::hashing::blake2_256(&code).to_vec(), code);
        }
    }
    let expected_authorities = manifest
        .fresh_grandpa
        .iter()
        .map(|(key, weight)| {
            let bytes: [u8; 32] = hex_bytes(key)?
                .try_into()
                .map_err(|_| "invalid fresh authority key")?;
            if bytes == [0; 32] || *weight == 0 {
                return Err("empty fresh authority key or weight".into());
            }
            Ok((bytes, *weight))
        })
        .collect::<Result<Vec<_>>>()?;
    let set_key = storage_key(b"Grandpa", b"CurrentSetId");
    let prior = value(&mut ext, &set_key).ok_or("missing checkpoint GRANDPA set")?;
    if u64::decode_all(&mut prior.as_slice()).map_err(|error| error.to_string())?
        != manifest.previous_grandpa_set
    {
        return Err("manifest GRANDPA set differs from verified checkpoint".into());
    }
    let frozen_keys = [
        storage_key(b"Session", b"CurrentIndex"),
        storage_key(b"Staking", b"CurrentEra"),
        storage_key(b"Staking", b"ActiveEra"),
    ];
    let frozen = frozen_keys
        .iter()
        .map(|key| value(&mut ext, key))
        .collect::<Vec<_>>();
    let governance_key = storage_key(b"Council", b"Members");
    let observed_governance = value(&mut ext, &governance_key);
    if fixture_only
        && (observed_governance.is_none() || value(&mut ext, fixture::PROBE_KEY).is_some())
    {
        return Err("escape fixture requires observed governance and an absent probe key".into());
    }
    fs::create_dir(output)
        .map_err(|error| format!("maintenance output must be a new directory: {error}"))?;
    let mut snapshots = snapshot::Journal::new(output)?;
    let (mut ext, mut parent_snapshot_sha256) = snapshots.persist(ext)?;
    let mut parent = H256::from(recovery::CHECKPOINT_HASH);
    let mut witnesses = Vec::new();
    let mut public_post_state_semantics = None;
    let mut state_observer = if request_storage_semantics {
        Some(state_observer::Track::new_nonfixture(
            &executor,
            &mut ext,
            &bootstrap,
            policy.commitment(),
        )?)
    } else if request_public_storage_semantics {
        Some(state_observer::Track::new(
            &executor,
            &mut ext,
            &bootstrap,
            policy.commitment(),
        )?)
    } else {
        None
    };

    let mut ordinary = 0;
    let mut activation_validation = None;
    let mut hold_observed = false;
    let mut repair_observed = false;
    let mut last_slot = 0;
    let mut previous_escapes = Vec::<Vec<u8>>::new();
    #[cfg(feature = "bridge-fixture")]
    let mut hashi_birth_headers = BTreeMap::new();
    let mut repaired_policy = None;
    let mut expectation_checks = 0usize;
    #[cfg(feature = "runtime-work-types")]
    let mut incoming_evidence = crate::incoming_replay::Collector::new(
        request_incoming_evidence
            || public_prior_audit_only
            || signed_fixture_only
            || request_storage_semantics,
        policy.commitment(),
        verified.state.state_root.clone(),
        verified.state.capture_manifest_sha256.clone(),
        hex::encode(Sha256::digest(&bytes)),
        fixture_only || public_repair_fixture || hashi_fixture_only || signed_fixture_only,
    );
    for (index, step) in manifest.steps.iter().enumerate() {
        if step.slot <= last_slot || step.rejected_recovery.len() > 128 {
            return Err("nonmonotonic maintenance slot or oversized negative cases".into());
        }
        last_slot = step.slot;
        let number = recovery::CHECKPOINT_NUMBER
            .checked_add(index as u32 + 1)
            .ok_or("block overflow")?;
        let stored = replay::code(&mut ext)?;
        let code = select_code(&stored, &deployed, &bootstrap, &known)?;
        let executing_version = replay::version(&executor, &mut ext, code)?;
        let maintenance = executing_version.spec_version != 138;
        let first_final_block = !maintenance && activation_validation.is_none();
        if first_final_block {
            let active_policy = status(&mut ext)?.policy;
            activation_validation = Some(activation::Track::before(
                &mut ext,
                number,
                parent,
                &parent_snapshot_sha256,
                active_policy,
                code,
                &executing_version,
            )?);
        }
        if maintenance != (step.recovery.is_some() || step.fixture_escape.is_some()) {
            return Err("recovery envelope presence differs from executing runtime phase".into());
        }
        if let Some(observer) = &mut state_observer {
            observer.before_block(&mut ext, step.slot, maintenance)?;
        }
        let header = Header::new(
            number,
            H256::zero(),
            H256::zero(),
            parent,
            Digest {
                logs: vec![DigestItem::PreRuntime(
                    *b"BABE",
                    (2u8, step.authority_index, step.slot).encode(),
                )],
            },
        );
        let repaired_work_context = if let Some(session) = &supplied_council {
            session.before_initialize(&mut ext, executing_version.spec_version, code)?
        } else {
            None
        };
        let initialized = replay::call(
            &executor,
            &mut ext,
            code,
            "Core_initialize_block",
            &header.encode(),
        )?;
        let inclusion = ExtrinsicInclusionMode::decode_all(&mut initialized.as_slice())
            .map_err(|error| error.to_string())?;
        if first_final_block {
            activation_validation
                .as_mut()
                .ok_or("activation observer absent")?
                .initialized(&mut ext)?;
        }
        if maintenance && inclusion != ExtrinsicInclusionMode::OnlyInherents {
            return Err("maintenance allowed ordinary extrinsics".into());
        }
        if let (Some(session), Some(context)) = (&supplied_council, &repaired_work_context) {
            session.after_initialize(&mut ext, context)?;
        }
        let time = timestamp(step.timestamp)?;
        if apply(&executor, &mut ext, code, &time)? != Ok(Ok(())) {
            return Err("timestamp rejected".into());
        }
        let mut extrinsics = vec![time];
        let mut rejected = 0;
        let mut action = None;
        let mut authorization_sha256 = None;
        let mut supplied_authorization = None;
        let mut native_context = None;
        let mut selected_native_lane = None;
        #[cfg(feature = "runtime-work-types")]
        let mut pending_incoming_evidence = None;
        if maintenance {
            reject(
                &executor,
                &mut ext,
                code,
                &remark(b"ordinary remark is forbidden")?,
            )?;
            rejected += 1;
            for negative in &step.rejected_recovery {
                let payload = replay::artifact(base, negative, recovery::MAX_INHERENT_BYTES)?;
                reject(&executor, &mut ext, code, &remark(&payload)?)?;
                rejected += 1;
            }
            if step.reject_previous_escapes
                || ((supplied_council_only || public_repair_fixture)
                    && !previous_escapes.is_empty())
            {
                if previous_escapes.is_empty() {
                    return Err("requested escape replay rejection before an actual escape".into());
                }
                for payload in &previous_escapes {
                    reject(&executor, &mut ext, code, &remark(payload)?)?;
                    rejected += 1;
                }
            }
            let payload = if let Some(request) = &step.fixture_escape {
                let observed = status(&mut ext)?;
                let target = by_version
                    .get(&request.destination_spec)
                    .ok_or("unlisted fixture destination code")?;
                let prepared = fixture::prepare(
                    base,
                    request,
                    &policy,
                    &observed,
                    recovery_anchor(&mut ext)?
                        .ok_or("escape requires an established first recovery child")?,
                    &stored,
                    code,
                    target,
                )?;
                for (name, negative) in prepared.negative {
                    let bytes = negative.encode_remark();
                    fs::write(
                        output.join(format!("{number}.rejected-{name}.scale")),
                        &bytes,
                    )
                    .map_err(|error| error.to_string())?;
                    reject(&executor, &mut ext, code, &remark(&bytes)?)?;
                    rejected += 1;
                }
                let recovery::RecoveryAction::Escape {
                    authorization,
                    work_policy,
                    ..
                } = &prepared.envelope.action
                else {
                    unreachable!()
                };
                let message = authorization.signing_message();
                let authorization_bytes = authorization.encode();
                authorization_sha256 = Some(hex::encode(Sha256::digest(&authorization_bytes)));
                let context = serde_json::json!({
                    "schema": "sora-nexus-public-fixture-generated-authorization-v1", "fixtureOnly": true, "productionReady": false,
                    "parentHash": format!("{parent:#x}"), "stateRootBeforeAuthorization": format!("{:#x}", replay::root(&mut ext)),
                    "recoveryAnchor": format!("0x{}", hex::encode(authorization.recovery_anchor)),
                    "phase": format!("{:?}", observed.phase), "cursor": observed.next_chunk, "operationCounter": observed.operation_counter,
                    "storedCodeBlake2": hex::encode(authorization.current_code_hash),
                    "executingCodeBlake2": hex::encode(authorization.current_executor_hash),
                    "destinationCodeBlake2": hex::encode(authorization.destination_code_hash),
                    "destinationSpec": authorization.destination_spec, "authorizationSha256": authorization_sha256,
                    "signingMessageSha256": hex::encode(Sha256::digest(&message)),
                });
                fs::write(
                    output.join(format!("{number}.authorization.scale")),
                    authorization_bytes,
                )
                .map_err(|error| error.to_string())?;
                fs::write(
                    output.join(format!("{number}.signing-message.scale")),
                    message,
                )
                .map_err(|error| error.to_string())?;
                fs::write(
                    output.join(format!("{number}.authorization.json")),
                    serde_json::to_vec_pretty(&context).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                if let Some(revised) = work_policy {
                    repaired_policy = Some(revised.commitment());
                }
                let payload = prepared.envelope.encode_remark();
                previous_escapes.push(payload.clone());
                fs::write(output.join(format!("{number}.escape.scale")), &payload)
                    .map_err(|error| error.to_string())?;
                payload
            } else {
                replay::artifact(
                    base,
                    step.recovery
                        .as_ref()
                        .ok_or("missing maintenance envelope")?,
                    recovery::MAX_INHERENT_BYTES,
                )?
            };
            #[cfg(feature = "bridge-fixture")]
            let payload = if step.fixture_hashi {
                let prepare = if signed_fixture_only {
                    hashi_fixture::prepare_funded
                } else {
                    hashi_fixture::prepare
                };
                let prepared = prepare(
                    &mut ext,
                    &policy,
                    &executing_version,
                    code,
                    &payload,
                    &hashi_birth_headers,
                )?;
                for (name, negative) in prepared.negatives {
                    fs::write(
                        output.join(format!("{number}.hashi-rejected-{name}.scale")),
                        &negative,
                    )
                    .map_err(|e| e.to_string())?;
                    reject(&executor, &mut ext, code, &remark(&negative)?)?;
                    rejected += 1;
                }
                let context =
                    serde_json::to_vec_pretty(&prepared.context).map_err(|e| e.to_string())?;
                authorization_sha256 = Some(hex::encode(Sha256::digest(&context)));
                fs::write(
                    output.join(format!("{number}.hashi-authorization.json")),
                    context,
                )
                .map_err(|e| e.to_string())?;
                fs::write(
                    output.join(format!("{number}.hashi.scale")),
                    &prepared.envelope,
                )
                .map_err(|e| e.to_string())?;
                prepared.envelope
            } else {
                payload
            };
            action = Some(
                RecoveryInherent::decode_remark(&payload)
                    .map_err(|error| format!("invalid committed envelope: {error:?}"))?
                    .action,
            );
            if let Some(track) = &mut historical_audit {
                let active = supplied_council
                    .as_ref()
                    .ok_or("historical council absent")?
                    .work_policy();
                track.observe(
                    executing_version.spec_version,
                    action.as_ref().ok_or("historical action absent")?,
                    &status(&mut ext)?,
                    active,
                    value(&mut ext, recovery::INCOMING_AUDIT_PENDING_KEY),
                    value(&mut ext, recovery::AUDIT_CURSOR_STORAGE_KEY),
                )?;
            }
            if fixture_only
                && !fixture::admitted_action(action.as_ref().ok_or("missing fixture action")?)
            {
                return Err(
                    "escape-only fixture forbids activation, governance and economic work".into(),
                );
            }
            if let Some(session) = &supplied_council {
                supplied_authorization = session.inspect(
                    index,
                    parent,
                    &payload,
                    action.as_ref().ok_or("missing supplied action")?,
                    &status(&mut ext)?,
                    recovery_anchor(&mut ext)?,
                    &stored,
                    code,
                    executing_version.spec_version,
                    &by_version,
                )?;
            }
            let work = remark(&payload)?;
            let dynamic_native = (native_recorder.is_some() || !native_recorders_v2.is_empty())
                && matches!(
                    action,
                    Some(recovery::RecoveryAction::Incoming(_))
                        | Some(recovery::RecoveryAction::Outgoing(_))
                );
            if dynamic_native {
                let lane = native_lane::select(
                    allow_dual_lanes,
                    native_recorder.is_some(),
                    !native_recorders_v2.is_empty(),
                    executing_version.spec_version,
                )?;
                if lane == native_lane::Lane::Original {
                    native_lane::check_original(
                        executing_version.spec_version,
                        executing_version.transaction_version,
                        code == bootstrap.as_slice(),
                        &status(&mut ext)?,
                        value(&mut ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref(),
                        policy.commitment(),
                    )?;
                } else {
                    let context = repaired_work_context
                        .as_ref()
                        .ok_or("dynamic repaired work lacks accepted council prefix")?;
                    native_expectations_v2::Recorder::select(
                        &native_recorders_v2,
                        code,
                        context,
                        executing_version.spec_version,
                        executing_version.transaction_version,
                    )?;
                }
                selected_native_lane = Some(lane);
            }

            #[cfg(feature = "runtime-work-types")]
            {
                let active_policy = supplied_council
                    .as_ref()
                    .map(|session| session.work_policy())
                    .unwrap_or(&policy);
                pending_incoming_evidence = incoming_evidence.prepare(
                    &mut ext,
                    active_policy,
                    action.as_ref().ok_or("incoming replay action absent")?,
                    number,
                    parent,
                    &parent_snapshot_sha256,
                    code,
                )?;
            }
            #[cfg(feature = "runtime-work-types")]
            if let Some(track) = &mut position_conservation {
                track.before_work(
                    &executor,
                    &mut ext,
                    code,
                    action.as_ref().ok_or("position action absent")?,
                    number,
                    executing_version.spec_version,
                )?;
            }
            let before_work_root = replay::root(&mut ext);
            if apply(&executor, &mut ext, code, &work)? != Ok(Ok(())) {
                return Err("committed recovery envelope rejected".into());
            }
            #[cfg(feature = "runtime-work-types")]
            if let Some(track) = &mut position_conservation {
                track.after_work(&mut ext)?;
            }
            if dynamic_native {
                native_context = Some((
                    payload,
                    before_work_root,
                    replay::root(&mut ext),
                    status(&mut ext)?.encode(),
                ));
            }
            reject(&executor, &mut ext, code, &work)?;
            rejected += 1;
            extrinsics.push(work);
        } else {
            #[cfg(feature = "bridge-fixture")]
            if let Some(session) = &mut signed_session {
                session.apply(
                    index,
                    number,
                    parent,
                    &executor,
                    &mut ext,
                    code,
                    &executing_version,
                    &mut extrinsics,
                    request_public_storage_semantics,
                )?;
            }
            ordinary += 1;
        }
        let encoded = replay::call(
            &executor,
            &mut ext,
            code,
            "BlockBuilder_finalize_block",
            &[],
        )?;
        let finalized =
            Header::decode_all(&mut encoded.as_slice()).map_err(|error| error.to_string())?;
        if *finalized.number() != number
            || *finalized.parent_hash() != parent
            || *finalized.state_root() != replay::root(&mut ext)
        {
            return Err("finalized maintenance header differs from authoring state".into());
        }
        let lineages: Vec<_> = finalized
            .digest()
            .logs
            .iter()
            .filter_map(|item| match item {
                DigestItem::Other(bytes) => recovery::parse_lineage_marker(bytes),
                _ => None,
            })
            .collect();
        if lineages != [policy.commitment()] {
            return Err("authored recovery block lacks its unique pinned lineage".into());
        }
        let expected_root = sp_io::trie::blake2_256_ordered_root(
            extrinsics.iter().map(Encode::encode).collect(),
            StateVersion::V0,
        );
        if *finalized.extrinsics_root() != expected_root {
            return Err("maintenance extrinsics root mismatch".into());
        }
        let block = RuntimeBlock {
            header: finalized.clone(),
            extrinsics,
        };
        let mut imported = snapshots.restore()?;
        if let Some(action) = &action {
            if let recovery::RecoveryAction::Incoming(payload)
            | recovery::RecoveryAction::Outgoing(payload) = action
            {
                let index = u32::decode_all(
                    &mut payload.get(..4).ok_or("dynamic action lacks work index")?,
                )
                .map_err(|e| e.to_string())?;
                if selected_native_lane == Some(native_lane::Lane::Repaired) {
                    let context = repaired_work_context
                        .as_ref()
                        .ok_or("repaired native context absent")?;
                    let recorder = native_expectations_v2::Recorder::select(
                        &native_recorders_v2,
                        code,
                        context,
                        executing_version.spec_version,
                        executing_version.transaction_version,
                    )?;
                    let (envelope, initialized_root, operation_root, terminal_status) =
                        native_context
                            .as_ref()
                            .ok_or("native operation context absent")?;
                    let kind = if matches!(action, recovery::RecoveryAction::Incoming(_)) {
                        "incoming"
                    } else {
                        "outgoing"
                    };
                    repaired_native_derivations.push(
                        recorder.derive(native_expectations_v2::Operation {
                            output,
                            snapshot_index: witnesses.len(),
                            snapshot_sha256: &parent_snapshot_sha256,
                            parent: native_parent
                                .as_ref()
                                .ok_or("native verified parent absent")?,
                            prior_transcript_sha256: &replay_transcript,
                            engine: code,
                            context,
                            envelope,
                            extrinsic: block
                                .extrinsics
                                .get(1)
                                .ok_or("native work extrinsic absent")?,
                            slot: step.slot,
                            authority_index: step.authority_index,
                            timestamp: step.timestamp,
                            operation_index: index,
                            operation_kind: kind,
                            initialized_root: *initialized_root,
                            operation_root: *operation_root,
                            runtime_version_scale: &executing_version.encode(),
                            terminal_status_scale: terminal_status,
                            block: &block,
                        })?,
                    );
                    repaired_dynamic_kinds.insert(kind);
                } else if selected_native_lane == Some(native_lane::Lane::Original) {
                    let recorder = native_recorder
                        .as_ref()
                        .ok_or("selected original recorder absent")?;
                    let (envelope, initialized_root, operation_root, terminal_status) =
                        native_context
                            .as_ref()
                            .ok_or("native operation context absent")?;
                    let kind = if matches!(action, recovery::RecoveryAction::Incoming(_)) {
                        "incoming"
                    } else {
                        "outgoing"
                    };
                    native_derivations.push(
                        recorder.derive(native_expectations::Operation {
                            output,
                            snapshot_index: witnesses.len(),
                            snapshot_sha256: &parent_snapshot_sha256,
                            parent: native_parent
                                .as_ref()
                                .ok_or("native verified parent absent")?,
                            prior_transcript_sha256: &replay_transcript,
                            engine: code,
                            policy_bytes: &policy_bytes,
                            policy: &policy,
                            envelope,
                            extrinsic: block
                                .extrinsics
                                .get(1)
                                .ok_or("native work extrinsic absent")?,
                            slot: step.slot,
                            authority_index: step.authority_index,
                            timestamp: step.timestamp,
                            operation_index: index,
                            operation_kind: kind,
                            initialized_root: *initialized_root,
                            operation_root: *operation_root,
                            runtime_version_scale: &executing_version.encode(),
                            terminal_status_scale: terminal_status,
                            block: &block,
                        })?,
                    );
                    original_dynamic_kinds.insert(kind);
                } else {
                    fixture_expectations.as_ref().ok_or("dynamic exact-Wasm rehearsal requires fresh native derivation or explicit public-fixture expectations")?.check_operation(index, &mut ext, &mut imported)?;
                }
            } else {
                confined_writes(&mut ext, &mut imported, action)?;
            }
        }
        if !replay::call(
            &import_executor,
            &mut imported,
            code,
            "Core_execute_block",
            &block.encode(),
        )?
        .is_empty()
            || replay::root(&mut imported) != *finalized.state_root()
        {
            return Err("independent maintenance import differs from authored block".into());
        }
        imported
            .commit_all()
            .map_err(|error| format!("import left an open transaction: {error}"))?;
        drop(imported);
        let current = status(&mut ext)?;
        if current.policy != policy.commitment()
            || current.format_version != 1
            || format!("{:?}", current.phase) != step.expected_phase
            || value(&mut ext, recovery::POLICY_STORAGE_KEY).as_deref()
                != Some(policy.commitment().as_slice())
            || value(&mut ext, recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() != Some(&[1][..])
            || recovery::TRANSIENT_CONTEXT_KEYS
                .iter()
                .any(|key| value(&mut ext, key).is_some())
        {
            return Err("recovery phase, lineage, halt or transient invariant failed".into());
        }
        if maintenance
            && frozen_keys
                .iter()
                .zip(&frozen)
                .any(|(key, original)| value(&mut ext, key) != *original)
        {
            return Err("economic session/era progressed during maintenance".into());
        }
        if (fixture_only || public_repair_fixture)
            && value(&mut ext, &governance_key) != observed_governance
        {
            return Err("escape fixture altered observed Council.Members".into());
        }
        if index == 0
            && (value(&mut ext, &set_key)
                != manifest
                    .previous_grandpa_set
                    .checked_add(1)
                    .map(|set| set.encode())
                || value(&mut ext, &storage_key(b"Grandpa", b"Authorities"))
                    != Some(expected_authorities.encode()))
        {
            return Err("first child did not install the exact fresh GRANDPA set".into());
        }
        let stored_after = replay::code(&mut ext)?;
        let next_code = select_code(&stored_after, &deployed, &bootstrap, &known)?;
        let next_version = replay::version(&executor, &mut ext, next_code)?;
        if next_version.spec_version != step.expected_spec {
            return Err("unexpected actual runtime activation version".into());
        }
        if let (Some(session), Some(context)) = (&mut supplied_council, &repaired_work_context) {
            session.imported_initialized_block(context)?;
        }
        if let Some(prepared) = supplied_authorization {
            let session = supplied_council
                .as_mut()
                .ok_or("missing supplied-council session")?;
            previous_escapes.push(session.accept(
                prepared,
                &mut ext,
                next_version.spec_version,
                output,
            )?);
        }
        hold_observed |= next_version.spec_version == 135;
        repair_observed |= matches!(next_version.spec_version, 136 | 137);
        let block_bytes = block.encode();
        fs::write(output.join(format!("{number}.block.scale")), &block_bytes)
            .map_err(|error| error.to_string())?;
        let hash = finalized.hash();
        let (restored, snapshot_hash) = snapshots.persist(ext)?;
        parent_snapshot_sha256 = snapshot_hash.clone();
        ext = restored;
        if matches!(
            current.phase,
            RecoveryPhase::Auditing | RecoveryPhase::Ready | RecoveryPhase::Complete
        ) {
            if let Some(expectations) = &fixture_expectations {
                expectation_checks += expectations.check(&mut ext)?;
            }
        }
        witnesses.push(BlockWitness {
            number,
            hash: format!("{hash:#x}"),
            state_root: format!("{:#x}", finalized.state_root()),
            executing_code_blake2: hex::encode(sp_io::hashing::blake2_256(code)),
            resulting_spec: next_version.spec_version,
            phase: format!("{:?}", current.phase),
            cursor: current.next_chunk,
            operation_counter: current.operation_counter,
            block_sha256: hex::encode(Sha256::digest(&block_bytes)),
            disk_snapshot_manifest_sha256: snapshot_hash,
            forbidden_extrinsics_rejected: rejected,
            maintenance_session_frozen: maintenance,
            independent_import_root_matches: true,
            disk_restart_root_matches: true,
            generated_authorization_sha256: authorization_sha256,
        });

        if let Some(observer) = &mut state_observer {
            observer.accepted(
                &executor,
                &mut ext,
                code,
                witnesses.last().ok_or("observer block witness absent")?,
                &finalized,
                step.slot,
                maintenance,
            )?;
        }
        #[cfg(feature = "runtime-work-types")]
        if let Some(track) = &mut bridge_state {
            track.accepted(
                &mut ext,
                action.as_ref(),
                witnesses.last().ok_or("bridge witness absent")?,
            )?;
        }

        #[cfg(feature = "runtime-work-types")]
        if let Some(track) = &mut position_conservation {
            track.accepted(
                &mut ext,
                witnesses.last().ok_or("position block witness absent")?,
            )?;
        }

        if inspect_public_storage_block(
            request_public_storage_semantics,
            public_post_state_semantics.is_some(),
            maintenance,
            ordinary,
            block.extrinsics.len(),
        )? {
            #[cfg(feature = "runtime-work-types")]
            position_conservation
                .as_mut()
                .ok_or("public position observer absent")?
                .inspect(
                    &executor,
                    &mut ext,
                    code,
                    witnesses
                        .last()
                        .ok_or("public position inspection witness absent")?,
                )?;
            let mut observed = storage_semantics::verify(
                &executor,
                &mut ext,
                storage_semantics::Context {
                    policy: &policy,
                    manifest: &manifest,
                    base,
                    checkpoint: &verified.state,
                    checkpoint_metadata: &verified.metadata_witness,
                    checkpoint_ledger: checkpoint_ledger
                        .as_ref()
                        .ok_or("checkpoint ledger baseline absent")?,
                    last: witnesses.last(),
                    output,
                },
            )?;
            observed["schema"] =
                serde_json::json!("sora-nexus-public-fixture-post-state-semantics-v1");
            observed["fixtureOnly"] = serde_json::json!(true);
            observed["inspectionStep"] = serde_json::json!(index);
            observed["inspectionSnapshotManifestSha256"] = serde_json::json!(
                witnesses
                    .last()
                    .ok_or("missing inspection witness")?
                    .disk_snapshot_manifest_sha256
            );
            observed["inspectionBeforeSignedTransactions"] = serde_json::json!(true);
            observed["inspectionIsReplayTerminal"] = serde_json::json!(false);
            observed["signedExtrinsicsInInspectedBlock"] = serde_json::json!(0);
            public_post_state_semantics = Some(observed);
        }

        #[cfg(feature = "runtime-work-types")]
        if let Some(prepared) = pending_incoming_evidence {
            let native = if let Some(value) = native_derivations
                .iter()
                .rev()
                .find(|value| value.block == number)
            {
                Some(serde_json::to_value(value).map_err(|e| e.to_string())?)
            } else if let Some(value) = repaired_native_derivations
                .iter()
                .rev()
                .find(|value| value.block == number)
            {
                Some(serde_json::to_value(value).map_err(|e| e.to_string())?)
            } else {
                None
            };
            incoming_evidence.accepted(
                prepared,
                &mut ext,
                witnesses
                    .last()
                    .ok_or("incoming replay accepted witness absent")?,
                native,
            )?;
        }
        replay_transcript = native_expectations::extend_transcript(
            &replay_transcript,
            witnesses.last().ok_or("missing completed block witness")?,
        )?;
        if native_recorder.is_some() || !native_recorders_v2.is_empty() {
            native_parent = Some(finalized.clone());
        }
        eprintln!("Rehearsed maintenance block {number}: {:?}, spec {}, cursor {}, disk restart and independent import agree", current.phase, next_version.spec_version, current.next_chunk);
        #[cfg(feature = "bridge-fixture")]
        if hashi_fixture_only || (signed_fixture_only && manifest.fixture_hashi_peers.is_some()) {
            hashi_birth_headers.insert(number, finalized);
        }
        parent = hash;
    }
    let completed = status(&mut ext)?.phase == RecoveryPhase::Complete
        && replay::code(&mut ext)? == final_code
        && ordinary > 0;
    if public_dynamic_only
        && (!completed
            || repaired_dynamic_kinds != std::collections::BTreeSet::from(["incoming", "outgoing"]))
    {
        return Err("public dynamic repair fixture must verify repaired incoming/outgoing and automatic final activation".into());
    }
    if public_prior_audit_only
        && (!completed
            || !original_dynamic_kinds.contains("incoming")
            || !repaired_dynamic_kinds.contains("outgoing")
            || value(&mut ext, b":sora-nexus:recovery:liveness-resumed:v1").is_none())
    {
        return Err("prior-policy audit fixture must verify original incoming, repaired outgoing, final activation and the deferred liveness boundary".into());
    }
    if fixture_only {
        let current = status(&mut ext)?;
        if completed
            || ordinary != 0
            || !hold_observed
            || !repair_observed
            || previous_escapes.len() != 2
            || current.phase != RecoveryPhase::Auditing
            || current.next_chunk != 1
            || current.operation_counter != 2
            || value(&mut ext, recovery::ACTIVE_POLICY_STORAGE_KEY)
                != repaired_policy.map(|hash| hash.to_vec())
            || value(&mut ext, recovery::AUDIT_CURSOR_STORAGE_KEY) != Some(1u32.encode())
            || value(&mut ext, fixture::PROBE_KEY).as_deref() != Some(fixture::PROBE_VALUE)
            || value(&mut ext, &governance_key) != observed_governance
        {
            return Err("escape-only fixture did not preserve governance and complete its signed forward repair probe".into());
        }
    } else if !supplied_council_only && !completed {
        return Err("rehearsal must automatically activate the pinned final code and execute an ordinary block".into());
    }
    #[cfg(feature = "runtime-work-types")]
    incoming_evidence.validate_final(
        &mut ext,
        supplied_council
            .as_ref()
            .map(|session| session.work_policy())
            .unwrap_or(&policy),
        completed,
        witnesses.last(),
    )?;
    let supplied_council = supplied_council
        .map(|session| {
            let stored = replay::code(&mut ext)?;
            let code = select_code(&stored, &deployed, &bootstrap, &known)?;
            let version = replay::version(&executor, &mut ext, code)?;
            session.finish(&mut ext, version.spec_version, ordinary, completed)
        })
        .transpose()?;

    #[cfg(feature = "runtime-work-types")]
    let incoming_evidence = incoming_evidence.finish(
        completed,
        replay_transcript.clone(),
        request_incoming_evidence || public_prior_audit_only,
    )?;
    #[cfg(feature = "bridge-fixture")]
    let public_signed_transactions = signed_session
        .map(|session| {
            session.finish(
                &policy,
                replay::root(&mut ext),
                &incoming_evidence,
                &witnesses,
            )
        })
        .transpose()?;
    #[cfg(not(feature = "bridge-fixture"))]
    let public_signed_transactions = None;
    #[cfg(feature = "runtime-work-types")]
    let prior_policy_incoming_audits = incoming_evidence.reports();
    #[cfg(feature = "runtime-work-types")]
    let incoming_replay_binding =
        (!prior_policy_incoming_audits.is_empty()).then(|| incoming_evidence.coverage_binding());
    #[cfg(not(feature = "runtime-work-types"))]
    let (incoming_evidence, prior_policy_incoming_audits) = ((), Vec::new());
    if let Some(track) = historical_audit {
        track.finish(
            &status(&mut ext)?,
            value(&mut ext, recovery::INCOMING_AUDIT_PENDING_KEY),
            value(&mut ext, historical_audit::Track::probe_key()),
            &prior_policy_incoming_audits,
        )?;
    }
    if request_public_storage_semantics {
        let observed = public_post_state_semantics
            .as_mut()
            .ok_or("public diagnostic never inspected an ordinary138 block")?;
        if public_signed_transactions.is_none() {
            return Err("public diagnostic did not finish its full signed fixture replay".into());
        }
        #[cfg(feature = "bridge-fixture")]
        {
            observed["maintenanceStateObservations"] = state_observer
                .take()
                .ok_or("public state observer absent")?
                .finish(
                    public_signed_transactions
                        .as_ref()
                        .ok_or("signed result absent")?,
                )?;
        }
        #[cfg(feature = "runtime-work-types")]
        {
            observed["positionConservation"] = position_conservation
                .take()
                .ok_or("public position observer absent")?
                .finish(&policy, replay::root(&mut ext), &incoming_evidence)?;
        }
        observed["subsequentSignedReplayCompleted"] = serde_json::json!(true);
        observed["replayTerminalStateRoot"] =
            serde_json::json!(format!("{:#x}", replay::root(&mut ext)));
    }
    let post_state_semantics = if request_storage_semantics {
        #[cfg(feature = "runtime-work-types")]
        {
            let code = replay::code(&mut ext)?;
            position_conservation
                .as_mut()
                .ok_or("position observer absent")?
                .inspect(
                    &executor,
                    &mut ext,
                    &code,
                    witnesses
                        .last()
                        .ok_or("position inspection witness absent")?,
                )?;
        }
        let mut observed = storage_semantics::verify(
            &executor,
            &mut ext,
            storage_semantics::Context {
                policy: &policy,
                manifest: &manifest,
                base,
                checkpoint: &verified.state,
                checkpoint_metadata: &verified.metadata_witness,
                checkpoint_ledger: checkpoint_ledger
                    .as_ref()
                    .ok_or("checkpoint ledger baseline absent")?,
                last: witnesses.last(),
                output,
            },
        )?;
        #[cfg(feature = "runtime-work-types")]
        {
            observed["positionConservation"] = position_conservation
                .take()
                .ok_or("position observer absent")?
                .finish(&policy, replay::root(&mut ext), &incoming_evidence)?;
        }
        observed["maintenanceStateObservations"] = state_observer
            .take()
            .ok_or("nonfixture state observer absent")?
            .finish_nonfixture()?;
        #[cfg(feature = "runtime-work-types")]
        {
            observed["bridgeStateEquations"] = bridge_state
                .take()
                .ok_or("bridge observer absent")?
                .finish(
                    &mut ext,
                    &policy,
                    &incoming_evidence,
                    witnesses.last().ok_or("bridge final witness absent")?,
                )?;
        }
        Some(observed)
    } else {
        public_post_state_semantics
    };
    let activation_validation = activation_validation
        .map(|track| track.finish(&witnesses, ordinary))
        .transpose()?;
    Ok((
        Report {
            schema: if signed_fixture_only {
                "sora-nexus-public-fixture-signed-maintenance-replay-witness-v1"
            } else if public_historical_audit_only {
                historical_audit::REPORT_SCHEMA
            } else if public_prior_audit_only {
                "sora-nexus-public-fixture-prior-policy-audit-maintenance-witness-v1"
            } else if public_dynamic_only {
                "sora-nexus-public-fixture-dynamic-repair-maintenance-witness-v1"
            } else if supplied_council_only {
                "sora-nexus-supplied-council-maintenance-replay-witness-v1"
            } else if fixture_only {
                "sora-nexus-public-fixture-escape-replay-witness-v1"
            } else if hashi_fixture_only {
                "sora-nexus-public-fixture-hashi-replay-witness-v1"
            } else {
                "sora-nexus-maintenance-replay-witness-v1"
            },
            manifest_sha256: hex::encode(Sha256::digest(bytes)),
            capture_manifest_sha256: verified.state.capture_manifest_sha256,
            policy_commitment: hex::encode(policy.commitment()),
            artifact_sha256: hashes,
            blocks: witnesses,
            automatic_activation_observed: completed,
            council_hold_observed: hold_observed,
            forward_repair_observed: repair_observed,
            ordinary_runtime_blocks: ordinary,
            liveness_grace_observed: value(&mut ext, b":sora-nexus:recovery:liveness-resumed:v1")
                .is_some(),
            fee_clock_shift_observed: value(&mut ext, b":sora-nexus:recovery:fee-clock-shifted:v1")
                .is_some(),
            bridge_halt_retained: true,
            native_execution: !native_derivations.is_empty()
                || !repaired_native_derivations.is_empty(),
            consensus_seals_verified: false,
            operator_approval_verified: false,
            production_ready: false,
            public_test_council_fixture: fixture_only || public_repair_fixture,
            public_test_hashi_fixture: hashi_fixture_only,
            fixture_governance_state_unchanged: (fixture_only || public_repair_fixture)
                .then_some(true),
            fixture_state_expectation_checks: expectation_checks,
            supplied_council,
            native_operation_derivations: native_derivations,
            repaired_native_operation_derivations: repaired_native_derivations,
            replay_transcript_sha256: replay_transcript,
            prior_policy_incoming_audits,
            #[cfg(feature = "runtime-work-types")]
            incoming_replay_binding,
            public_signed_transactions,
            post_state_semantics,
            activation_validation,
        },
        incoming_evidence,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_storage_inspection_requires_a_reserved_first_ordinary_block() {
        assert!(!inspect_public_storage_block(false, false, false, 1, 5).unwrap());
        assert!(!inspect_public_storage_block(true, false, true, 0, 2).unwrap());
        assert!(inspect_public_storage_block(true, false, false, 1, 1).unwrap());
        assert!(inspect_public_storage_block(true, false, false, 1, 2).is_err());
        assert!(inspect_public_storage_block(true, false, false, 2, 1).is_err());
        assert!(!inspect_public_storage_block(true, true, false, 2, 2).unwrap());
    }

    #[test]
    #[cfg(feature = "runtime-work-types")]
    fn incoming_audit_write_scope_is_exact_and_failure_is_atomic() {
        use framenode_runtime::incident_recovery_incoming::{audit_marker_key, IncomingAudit};
        let active = [1; 32];
        let audit = IncomingAudit {
            index: 0,
            operation_index: 1,
            operation_commitment: [4; 32],
            requests: vec![None],
            prior_policy: None,
        };
        let action = recovery::RecoveryAction::IncomingAudit(audit.encode());
        let state = RecoveryStatus {
            format_version: 1,
            policy: [2; 32],
            phase: RecoveryPhase::Auditing,
            next_chunk: 3,
            operation_counter: 2,
            failure: None,
        };
        let storage = sp_core::storage::Storage {
            top: BTreeMap::from([
                (
                    recovery::ACTIVE_POLICY_STORAGE_KEY.to_vec(),
                    active.to_vec(),
                ),
                (recovery::STATUS_STORAGE_KEY.to_vec(), state.encode()),
                (recovery::AUDIT_CURSOR_STORAGE_KEY.to_vec(), 0u32.encode()),
                (recovery::INCOMING_AUDIT_PENDING_KEY.to_vec(), 1u32.encode()),
            ]),
            children_default: Default::default(),
        };
        let marker = audit_marker_key(active, audit.operation_commitment);
        let mut parent = sp_io::TestExternalities::new(storage.clone());
        let mut after = sp_io::TestExternalities::new(storage.clone());
        after.execute_with(|| {
            sp_io::storage::set(&marker, b"scope-only synthetic marker");
            sp_io::storage::set(recovery::INCOMING_AUDIT_PENDING_KEY, &0u32.encode());
            sp_io::storage::set(recovery::AUDIT_CURSOR_STORAGE_KEY, &1u32.encode());
        });
        assert!(confined_writes(&mut after, &mut parent, &action).is_ok());
        for forbidden in [
            b":sora-nexus:recovery:uncommitted".to_vec(),
            b":code".to_vec(),
            audit_marker_key([5; 32], audit.operation_commitment),
            storage_key(b"System", b"Account"),
        ] {
            let mut parent = sp_io::TestExternalities::new(storage.clone());
            let mut after = sp_io::TestExternalities::new(storage.clone());
            after.execute_with(|| sp_io::storage::set(&forbidden, &[9]));
            assert!(confined_writes(&mut after, &mut parent, &action).is_err());
        }
        let mut parent = sp_io::TestExternalities::new(storage.clone());
        let mut after = sp_io::TestExternalities::new(storage);
        let mut failed = state;
        failed.phase = RecoveryPhase::Failed;
        failed.failure = Some(recovery::RecoveryError::WrongPreimage);
        after.execute_with(|| sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &failed.encode()));
        assert!(confined_writes(&mut after, &mut parent, &action).is_ok());
        after.execute_with(|| {
            sp_io::storage::set(recovery::INCOMING_AUDIT_PENDING_KEY, &0u32.encode())
        });
        assert!(confined_writes(&mut after, &mut parent, &action).is_err());
    }

    #[test]
    fn maintenance_write_audit_rejects_uncommitted_account_and_child_mutations() {
        let mut before = sp_io::TestExternalities::default();
        let mut after = sp_io::TestExternalities::default();
        let mut account = storage_key(b"System", b"Account");
        account.extend([1u8; 32]);
        after.execute_with(|| sp_io::storage::set(&account, &[1]));
        assert!(confined_writes(&mut after, &mut before, &recovery::RecoveryAction::Tick).is_err());
        let action = recovery::RecoveryAction::Apply(recovery::RecoveryChunk {
            index: 0,
            edits: vec![recovery::GuardedStorageEdit {
                scope: recovery::StorageScope::Top,
                key: account,
                before: None,
                after: Some(vec![1]),
            }],
        });
        assert!(confined_writes(&mut after, &mut before, &action).is_ok());
        after
            .execute_with(|| sp_io::default_child_storage::set(b"fixture-child", b"balance", &[2]));
        assert!(confined_writes(&mut after, &mut before, &action).is_err());
    }

    #[test]
    fn serialized_restart_retains_top_child_code_and_recovery_cursor() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "soranx-maintenance-snapshot-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            sp_io::storage::set(b":code", b"snapshot persistence fixture only");
            sp_io::storage::set(recovery::AUDIT_CURSOR_STORAGE_KEY, &7u32.encode());
            sp_io::default_child_storage::set(b"fixture-child", b"balance", &[9]);
        });
        let expected = replay::root(&mut ext);
        let mut snapshots = snapshot::Journal::new(&directory).unwrap();
        let (mut restored, hash) = snapshots.persist(ext).unwrap();
        assert_eq!(replay::root(&mut restored), expected);
        assert_eq!(hash.len(), 64);
        restored.execute_with(|| {
            assert_eq!(
                sp_io::storage::get(recovery::AUDIT_CURSOR_STORAGE_KEY)
                    .unwrap()
                    .to_vec(),
                7u32.encode()
            );
            assert_eq!(
                sp_io::default_child_storage::get(b"fixture-child", b"balance"),
                Some(vec![9])
            );
            assert_eq!(
                sp_io::storage::get(b":code").unwrap().as_ref(),
                b"snapshot persistence fixture only"
            );
        });
        fs::remove_dir_all(directory).unwrap();
    }
}
