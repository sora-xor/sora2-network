//! Supporting, fixture-only try-runtime checks on an exact maintenance snapshot.
//!
//! The companion never replaces `:code`. Paired build receipts are byte-bound
//! supporting evidence, not independently authenticated build provenance. The
//! release controller must establish that provenance separately. No input JSON
//! can make this module issue a production or consensus certificate.
use super::{replay, snapshot, status, storage_key, value, Result, RuntimeBlock};
use codec::{Compact, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use common::recovery::{self, RecoveryPhase, RecoveryPolicy};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sp_core::H256;
use sp_runtime::{traits::Header as _, Weight};
use std::{collections::BTreeSet, fs, io::Read, path::Path};

const MAX_JSON: usize = 128 * 1024 * 1024;
const MAX_WASM: usize = 32 * 1024 * 1024;
const MAX_BLOCK: usize = 2 * 1024 * 1024;
const MAX_BLOCKS: usize = 10_000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema: String,
    production_wasm: replay::Artifact,
    companion_wasm: replay::Artifact,
    production_build_receipt: replay::Artifact,
    companion_build_receipt: replay::Artifact,
    original_policy: replay::Artifact,
    active_policy: replay::Artifact,
    production_replay: replay::Artifact,
    snapshot_directory: String,
    blocks: Vec<replay::Artifact>,
}

fn need(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    need(
        bytes.len() <= limit,
        "try-runtime input exceeds its byte bound",
    )?;
    Ok(bytes)
}
fn artifact(base: &Path, reference: &replay::Artifact, limit: usize) -> Result<Vec<u8>> {
    let raw = read(&crate::local_path(base, &reference.file)?, limit)?;
    need(
        reference.sha256 == sha(&raw),
        "try-runtime artifact digest differs",
    )?;
    Ok(raw)
}
fn json_bytes(raw: &[u8]) -> Result<Value> {
    serde_json::from_slice(raw).map_err(|e| e.to_string())
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .ok_or_else(|| format!("try-runtime missing text {field}"))
}
fn integer(value: &Value, field: &str) -> Result<u64> {
    value[field]
        .as_u64()
        .ok_or_else(|| format!("try-runtime missing unsigned integer {field}"))
}
fn hash(value: &Value, field: &str) -> Result<H256> {
    let raw = text(value, field)?;
    let parsed: H256 = raw
        .parse()
        .map_err(|_| format!("try-runtime malformed hash {field}"))?;
    need(
        raw == format!("{parsed:#x}"),
        "try-runtime noncanonical hash",
    )?;
    Ok(parsed)
}
fn decode<T: DecodeWithMemTracking + Encode>(raw: &[u8]) -> Result<T> {
    need(raw.len() <= 128, "try-runtime oversized scalar result")?;
    let mut input = raw;
    let value = T::decode_with_mem_limit(&mut input, 4096).map_err(|e| e.to_string())?;
    need(
        input.is_empty() && value.encode() == raw,
        "try-runtime noncanonical SCALE",
    )?;
    Ok(value)
}
fn policy(raw: &[u8]) -> Result<RecoveryPolicy> {
    let mut input = raw;
    let policy = RecoveryPolicy::decode_with_mem_limit(&mut input, 8 * 1024 * 1024)
        .map_err(|e| e.to_string())?;
    need(
        input.is_empty() && policy.encode() == raw,
        "try-runtime policy encoding differs",
    )?;
    policy
        .validate()
        .map_err(|e| format!("try-runtime policy: {e:?}"))?;
    need(
        policy.authorization_domain == recovery::RecoveryDomain::public_fixture(),
        "supporting fixture audit refuses a production recovery domain",
    )?;
    Ok(policy)
}

fn record_matches(raw: &[u8], record: &Value) -> bool {
    record["sha256"] == sha(raw)
        && record["bytes"] == raw.len() as u64
        && record["blake2_256"] == hex::encode(sp_io::hashing::blake2_256(raw))
}

fn companion_policy(receipt: &Value, original_raw: &[u8], original: &RecoveryPolicy) -> Result<()> {
    // Final-runtime admission binds its compiled POLICY_SCALE to the immutable
    // original lineage. Revised work is checked separately by `admission`.
    need(
        receipt["artifactRole"] == "tryRuntimeCompanion"
            && receipt["tryRuntimeCompanion"] == true
            && record_matches(original_raw, &receipt["companionFixturePolicy"]["policy"])
            && receipt["companionFixturePolicy"]["policyCommitment"]
                == hex::encode(original.commitment()),
        "try-runtime companion original fixture policy differs",
    )
}

/// Bind the existing fixture builder's retained files; do not infer that an
/// arbitrary JSON receipt proves its command actually ran.
fn build_receipt(
    base: &Path,
    reference: &replay::Artifact,
    wasm: &[u8],
    companion: bool,
) -> Result<Value> {
    let receipt = json_bytes(&artifact(base, reference, 4 * 1024 * 1024)?)?;
    need(
        receipt["schema"] == "sora-nexus-fixture-wasm-build-v1"
            && receipt["status"] == "passed"
            && receipt["exitCode"] == 0
            && receipt["fixtureOnly"] == true
            && receipt["productionReady"] == false
            && receipt["specVersion"] == 138
            && receipt["transactionVersion"] == 134
            && receipt["lockBefore"] == receipt["lockAfter"],
        "try-runtime build receipt scope/status differs",
    )?;
    let path = crate::local_path(base, &reference.file)?;
    let directory = path.parent().ok_or("build receipt has no parent")?;
    for (file, record) in [
        ("source-files.json", &receipt["sourceFingerprint"]),
        ("source-provenance.json", &receipt["sourceProvenance"]),
        ("Cargo.lock", &receipt["lockAfter"]),
        ("build-fixture-runner.py", &receipt["runner"]),
        ("build-environment.py", &receipt["environmentHelper"]),
        ("llvm-wrapper.sh", &receipt["llvmWrapper"]),
        ("cargo.stdout.log", &receipt["logs"]["cargo.stdout.log"]),
        ("cargo.stderr.log", &receipt["logs"]["cargo.stderr.log"]),
    ] {
        let raw = read(&crate::local_path(directory, file)?, MAX_JSON)?;
        need(
            record_matches(&raw, record),
            "try-runtime retained build input/log differs",
        )?;
    }
    let toolchain = read(
        &crate::local_path(directory, "rust-toolchain.toml")?,
        64 * 1024,
    )?;
    need(
        receipt["environmentPolicy"]["toolchainFileSha256"] == sha(&toolchain),
        "try-runtime toolchain bytes differ",
    )?;
    let exported = receipt["artifacts"]
        .as_object()
        .ok_or("build artifacts missing")?;
    need(
        exported.values().any(|record| record_matches(wasm, record)),
        "try-runtime Wasm absent from build receipt",
    )?;
    let features = receipt["wasmProjectDependency"]["features"]
        .as_array()
        .ok_or("Wasm build features absent")?;
    let expected: BTreeSet<&str> = if companion {
        ["build-wasm-binary", "try-runtime"].into_iter().collect()
    } else {
        ["build-wasm-binary"].into_iter().collect()
    };
    let actual = features
        .iter()
        .map(|value| value.as_str().ok_or("invalid build feature"))
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    need(
        actual == expected
            && features.len() == expected.len()
            && receipt["wasmProjectDependency"]["default-features"] == false
            && receipt["wasmProjectDependency"]["package"] == "framenode-runtime",
        "try-runtime unexpected Wasm feature profile",
    )?;
    Ok(receipt)
}
fn same_build_inputs(production: &Value, companion: &Value) -> Result<()> {
    for field in [
        "sourceFingerprint",
        "sourceProvenance",
        "lockBefore",
        "lockAfter",
        "rustc",
        "environmentHelper",
        "llvmWrapper",
    ] {
        need(
            !production[field].is_null() && production[field] == companion[field],
            "try-runtime companion changed build inputs",
        )?;
    }
    let environment = |receipt: &Value| -> Result<Value> {
        let mut env = receipt["environmentPolicy"].clone();
        let controlled = env["controlled"]
            .as_object_mut()
            .ok_or("build environment missing")?;
        for field in [
            "FORCE_WASM_BUILD",
            "CARGO_TARGET_DIR",
            "WASM_BUILD_WORKSPACE_HINT",
        ] {
            controlled.remove(field);
        }
        // Removed inherited variables do not affect the resulting sanitized environment.
        env.as_object_mut()
            .ok_or("build environment malformed")?
            .remove("removedKeys");
        Ok(env)
    };
    need(
        environment(production)? == environment(companion)?,
        "try-runtime companion changed build environment",
    )
}

fn admission(
    ext: &mut sp_io::TestExternalities,
    production: &[u8],
    original: &RecoveryPolicy,
    active: &RecoveryPolicy,
) -> Result<()> {
    let before = replay::root(ext);
    let state = status(ext)?;
    need(
        replay::code(ext)? == production
            && state.format_version == 1
            && state.phase == RecoveryPhase::Complete
            && state.policy == original.commitment()
            && state.failure.is_none()
            && state.next_chunk as usize == active.chunk_hashes.len()
            && value(ext, recovery::POLICY_STORAGE_KEY).as_deref()
                == Some(original.commitment().as_slice())
            && value(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
                == Some(active.commitment().as_slice())
            && value(ext, recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() == Some(&[1][..])
            && value(ext, recovery::INCOMING_AUDIT_PENDING_KEY) == Some(0u32.encode())
            && value(ext, recovery::AUDIT_CURSOR_STORAGE_KEY)
                == Some((active.audit_chunk_hashes.len() as u32).encode())
            && value(ext, recovery::TERMINAL_AUDIT_STORAGE_KEY)
                == Some(
                    (
                        active.commitment(),
                        active.terminal_audit_commitment,
                        state.next_chunk,
                    )
                        .encode(),
                )
            && recovery::TRANSIENT_CONTEXT_KEYS
                .iter()
                .all(|key| value(ext, key).is_none()),
        "try-runtime parent lacks exact completed final-runtime admission",
    )?;
    let anchor =
        value(ext, recovery::RECOVERY_ANCHOR_STORAGE_KEY).ok_or("missing recovery anchor")?;
    need(
        anchor.len() == 32 && anchor != [0; 32],
        "invalid recovery anchor",
    )?;
    need(
        before == replay::root(ext),
        "try-runtime admission mutated state",
    )
}

fn legacy_parent(ext: &mut sp_io::TestExternalities, number: u32) -> Result<()> {
    let stored =
        |ext: &mut sp_io::TestExternalities, pallet: &[u8], item: &[u8]| -> Result<Vec<u8>> {
            let raw =
                value(ext, &storage_key(pallet, item)).ok_or("try-runtime legacy field missing")?;
            need(raw.len() <= 128, "try-runtime oversized legacy field")?;
            Ok(raw)
        };
    let version: u16 = decode(&stored(ext, b"Staking", b":__STORAGE_VERSION__:")?)?;
    let upgrade: (Compact<u32>, Vec<u8>) = decode(&stored(ext, b"System", b"LastRuntimeUpgrade")?)?;
    let actual: u32 = decode(&stored(ext, b"System", b"Number")?)?;
    need(
        version == 16 && upgrade.0 .0 < 138 && upgrade.1 == b"sora-substrate" && actual == number,
        "try-runtime parent already migrated or has wrong runtime/block identity",
    )
}

#[derive(Clone)]
struct Clock {
    slot: u64,
    epoch: u64,
    session: u32,
    current: u32,
    active: u32,
    start: Option<u64>,
    timestamp: u64,
}
impl Clock {
    fn read(ext: &mut sp_io::TestExternalities) -> Result<Self> {
        fn item<T: DecodeWithMemTracking + Encode>(
            ext: &mut sp_io::TestExternalities,
            pallet: &[u8],
            field: &[u8],
        ) -> Result<T> {
            let raw =
                value(ext, &storage_key(pallet, field)).ok_or("try-runtime clock field missing")?;
            need(raw.len() <= 32, "try-runtime oversized clock")?;
            decode(&raw)
        }
        let (active, start) = item(ext, b"Staking", b"ActiveEra")?;
        Ok(Self {
            slot: item(ext, b"Babe", b"CurrentSlot")?,
            epoch: item(ext, b"Babe", b"EpochIndex")?,
            session: item(ext, b"Session", b"CurrentIndex")?,
            current: item(ext, b"Staking", b"CurrentEra")?,
            active,
            start,
            timestamp: item(ext, b"Timestamp", b"Now")?,
        })
    }
    fn observed(&self, row: &Value) -> Result<()> {
        need(
            row["slot"] == self.slot
                && row["epoch"] == self.epoch
                && row["session"] == self.session
                && row["currentEra"] == self.current
                && row["activeEra"] == self.active
                && row["timestamp"] == self.timestamp
                && row["activeEraStart"] == json!(self.start),
            "try-runtime state differs from exact activation observation",
        )
    }
    fn transitions(&self, next: &Self) -> Result<(bool, bool)> {
        let session = next.session != self.session;
        let era = next.active != self.active;
        need(
            next.slot > self.slot
                && next.epoch >= self.epoch
                && (!session || self.session.checked_add(1) == Some(next.session))
                && session == (next.epoch > self.epoch)
                && (!era || (session && self.active.checked_add(1) == Some(next.active)))
                && (next.current == self.current
                    || self.current.checked_add(1) == Some(next.current))
                && (next.current == next.active
                    || next.active.checked_add(1) == Some(next.current))
                && next.start.is_some_and(|start| start <= next.timestamp)
                && (!era || next.start == Some(next.timestamp)),
            "try-runtime invalid staking transition",
        )?;
        Ok((session, era))
    }
}

fn block(raw: &[u8]) -> Result<RuntimeBlock> {
    need(
        raw.len() <= MAX_BLOCK,
        "try-runtime oversized ordinary block",
    )?;
    let mut input = raw;
    let result = RuntimeBlock::decode_with_mem_limit(&mut input, MAX_BLOCK * 8)
        .map_err(|e| e.to_string())?;
    need(
        input.is_empty() && result.encode() == raw,
        "try-runtime noncanonical ordinary block",
    )?;
    Ok(result)
}
fn execute_input(raw_block: &[u8]) -> Result<Vec<u8>> {
    block(raw_block)?;
    let mut raw = raw_block.to_vec();
    // stable2609 TryRuntime::execute_block(block, state_root_check=true,
    // signature_check=true, TryStateSelect::All). All's SCALE discriminant is 1.
    raw.extend([1, 1, 1]);
    Ok(raw)
}

/// Run two independent restores. Never feed pass A's migrated state into B.
/// This only returns after actual companion Wasm calls and production-root joins.
pub fn run(manifest_path: &Path) -> Result<Value> {
    let raw_manifest = read(manifest_path, 4 * 1024 * 1024)?;
    let manifest: Manifest = serde_json::from_slice(&raw_manifest).map_err(|e| e.to_string())?;
    need(
        manifest.schema == "sora-nexus-fixture-try-runtime-audit-v1"
            && !manifest.blocks.is_empty()
            && manifest.blocks.len() <= MAX_BLOCKS,
        "try-runtime manifest schema/bounds differ",
    )?;
    let base = manifest_path
        .parent()
        .ok_or("try-runtime manifest parent missing")?;
    let production = artifact(base, &manifest.production_wasm, MAX_WASM)?;
    let companion = artifact(base, &manifest.companion_wasm, MAX_WASM)?;
    need(
        production != companion,
        "try-runtime requires a separate companion artifact",
    )?;
    let production_build =
        build_receipt(base, &manifest.production_build_receipt, &production, false)?;
    let companion_build = build_receipt(base, &manifest.companion_build_receipt, &companion, true)?;
    same_build_inputs(&production_build, &companion_build)?;
    let original_raw = artifact(base, &manifest.original_policy, 2 * 1024 * 1024)?;
    let original = policy(&original_raw)?;
    let active_raw = artifact(base, &manifest.active_policy, 2 * 1024 * 1024)?;
    let active = policy(&active_raw)?;
    companion_policy(&companion_build, &original_raw, &original)?;
    let report = json_bytes(&artifact(base, &manifest.production_replay, MAX_JSON)?)?;
    let observation = &report["activationValidation"];
    need(
        observation["schema"] == "sora-nexus-exact-post-maintenance-activation-v2"
            && observation["lineagePolicyCommitment"] == hex::encode(original.commitment())
            && observation["policyCommitment"] == hex::encode(active.commitment())
            && observation["executingCodeBlake2"]
                == hex::encode(sp_io::hashing::blake2_256(&production))
            && observation["tryRuntimeFeatureChecksExecuted"] == false
            && observation["productionReady"] == false,
        "try-runtime production activation witness differs",
    )?;
    let rows = report["blocks"]
        .as_array()
        .ok_or("production replay blocks missing")?;
    let observed = observation["rows"]
        .as_array()
        .ok_or("activation observations missing")?;
    let first_number = integer(observation, "firstBlock")?;
    let first: usize = first_number
        .checked_sub(recovery::CHECKPOINT_NUMBER as u64 + 1)
        .ok_or("invalid activation height")?
        .try_into()
        .map_err(|_| "activation index overflow")?;
    need(
        first > 0
            && first < rows.len()
            && first_number
                .checked_add(manifest.blocks.len() as u64)
                .is_some_and(|end| end <= u32::MAX as u64)
            && rows.len() - first == manifest.blocks.len()
            && observed.len() == manifest.blocks.len()
            && observation["ordinaryBlocks"] == manifest.blocks.len() as u64,
        "try-runtime ordinary block coverage differs",
    )?;
    let parent = &rows[first - 1];
    let parent_root = hash(observation, "maintenanceStateRoot")?;
    let mut parent_hash = hash(observation, "maintenanceParentHash")?;
    need(
        parent["stateRoot"] == observation["maintenanceStateRoot"]
            && parent["hash"] == observation["maintenanceParentHash"]
            && parent["diskSnapshotManifestSha256"] == observation["maintenanceSnapshotSha256"]
            && parent["phase"] == "Complete"
            && parent["resultingSpec"] == 138,
        "try-runtime maintenance snapshot detached from activation",
    )?;
    let directory = crate::local_path(base, &manifest.snapshot_directory)?;
    let restore = || {
        snapshot::restore_pinned(
            &directory,
            &format!("snapshots/{first:06}.json"),
            text(observation, "maintenanceSnapshotSha256")?,
            parent_root,
        )
    };
    let executor = replay::Executor::builder().build();
    let mut upgrade = restore()?;
    admission(&mut upgrade, &production, &original, &active)?;
    legacy_parent(&mut upgrade, (first_number - 1) as u32)?;
    let production_version = replay::version(&executor, &mut upgrade, &production)?;
    let companion_version = replay::version(&executor, &mut upgrade, &companion)?;
    need(
        production_version.spec_version == 138
            && production_version.transaction_version == 134
            && companion_version.spec_version == 138
            && companion_version.transaction_version == 134
            && production_version.spec_name == companion_version.spec_name
            && production_version.impl_name == companion_version.impl_name
            && production_version.authoring_version == companion_version.authoring_version
            && production_version.impl_version == companion_version.impl_version
            && production_version
                .apis
                .iter()
                .all(|api| companion_version.apis.contains(api)),
        "try-runtime companion runtime/API identity differs",
    )?;
    // stable2609 UpgradeCheckSelect::All has SCALE discriminant 1.
    let upgrade_return = replay::call(
        &executor,
        &mut upgrade,
        &companion,
        "TryRuntime_on_runtime_upgrade",
        &[1],
    )?;
    need(
        upgrade_return.len() <= 36,
        "try-runtime oversized upgrade return",
    )?;
    let (used, limit): (Weight, Weight) = decode(&upgrade_return)?;
    need(
        used.all_lte(limit),
        "try-runtime migration exceeds returned block weight",
    )?;
    admission(&mut upgrade, &production, &original, &active)?;
    let upgrade_root = replay::root(&mut upgrade);
    drop(upgrade);

    let mut ext = restore()?;
    admission(&mut ext, &production, &original, &active)?;
    legacy_parent(&mut ext, (first_number - 1) as u32)?;
    let mut previous = Clock::read(&mut ext)?;
    let mut first_session = None;
    let mut first_era = None;
    let mut executed = Vec::new();
    for (index, reference) in manifest.blocks.iter().enumerate() {
        let raw = artifact(base, reference, MAX_BLOCK)?;
        let block = block(&raw)?;
        let row = &rows[first + index];
        let number = first_number + index as u64;
        need(
            *block.header.number() as u64 == number
                && *block.header.parent_hash() == parent_hash
                && hash(row, "hash")? == block.header.hash()
                && hash(row, "stateRoot")? == *block.header.state_root()
                && row["blockSha256"] == sha(&raw)
                && row["number"] == number
                && row["executingCodeBlake2"]
                    == hex::encode(sp_io::hashing::blake2_256(&production))
                && row["phase"] == "Complete"
                && row["resultingSpec"] == 138
                && row["independentImportRootMatches"] == true
                && row["diskRestartRootMatches"] == true
                && observed[index]["blockHash"] == row["hash"]
                && observed[index]["stateRoot"] == row["stateRoot"],
            "try-runtime ordinary bytes/ancestry/production witness differs",
        )?;
        let returned = replay::call(
            &executor,
            &mut ext,
            &companion,
            "TryRuntime_execute_block",
            &execute_input(&raw)?,
        )?;
        need(returned.len() <= 18, "try-runtime oversized block return")?;
        let weight: Weight = decode(&returned)?;
        need(
            replay::root(&mut ext) == *block.header.state_root(),
            "try-runtime companion changed production state root",
        )?;
        admission(&mut ext, &production, &original, &active)?;
        let after = Clock::read(&mut ext)?;
        after.observed(&observed[index])?;
        let (session, era) = previous.transitions(&after)?;
        if session && first_session.is_none() {
            first_session = Some(number);
        }
        if era && first_era.is_none() {
            first_era = Some(number);
        }
        executed.push(
            json!({"block":number,"blockHash":row["hash"],"stateRoot":row["stateRoot"],
            "productionBlockSha256":sha(&raw),"returnScale":hex::encode(&returned),
            "weight":{"refTime":weight.ref_time(),"proofSize":weight.proof_size()},
            "session":after.session,"activeEra":after.active,"matchesProductionStateRoot":true}),
        );
        previous = after;
        parent_hash = block.header.hash();
    }
    let last_number = first_number + manifest.blocks.len() as u64 - 1;
    need(
        first_session.is_some_and(|number| number < last_number)
            && first_era.is_some_and(|number| number < last_number)
            && observation["transitions"]["firstSession"]["block"] == json!(first_session)
            && observation["transitions"]["firstActiveEra"]["block"] == json!(first_era),
        "try-runtime must cross observed session and era boundaries and execute follow-ups",
    )?;
    Ok(
        json!({"schema":"sora-nexus-fixture-try-runtime-audit-witness-v1","manifestSha256":sha(&raw_manifest),
        "productionReplaySha256":manifest.production_replay.sha256,"productionWasmSha256":sha(&production),
        "companionWasmSha256":sha(&companion),"originalPolicyCommitment":hex::encode(original.commitment()),
        "activePolicyCommitment":hex::encode(active.commitment()),"maintenanceStateRoot":format!("{parent_root:#x}"),
        "maintenanceSnapshotSha256":observation["maintenanceSnapshotSha256"],
        "buildEvidenceBound":true,"buildExecutionIndependentlyVerified":false,
        "productionBuildReceiptSha256":manifest.production_build_receipt.sha256,
        "companionBuildReceiptSha256":manifest.companion_build_receipt.sha256,
        "upgrade":{"checks":"All","returnScale":hex::encode(upgrade_return),"resultingStateRoot":format!("{upgrade_root:#x}"),
            "weight":{"refTime":used.ref_time(),"proofSize":used.proof_size()}},
        "blocks":executed,"firstSessionBlock":first_session,"firstActiveEraBlock":first_era,
        "freshRestoreForEachPass":true,"installedCodeUnchanged":true,"stateRootChecks":true,"signatureChecks":true,
        "tryStateChecks":"All","nativeExecution":false,"fixtureOnly":true,"productionReady":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_runtime::generic::Digest;
    fn fixture() -> RecoveryPolicy {
        RecoveryPolicy {
            format_version: 2,
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![[1; 32].into()],
            chunk_hashes: vec![[2; 32]],
            audit_chunk_hashes: vec![[3; 32]],
            authorization_domain: recovery::RecoveryDomain::public_fixture(),
            evidence_commitment: [4; 32],
            terminal_audit_commitment: [5; 32],
        }
    }
    #[test]
    fn repaired_companion_keeps_original_lineage_separate_from_active_work() {
        let original = fixture();
        let mut active = original.clone();
        active.evidence_commitment = [8; 32];
        let raw = original.encode();
        let receipt = json!({"artifactRole":"tryRuntimeCompanion","tryRuntimeCompanion":true,
            "companionFixturePolicy":{"policyCommitment":hex::encode(original.commitment()),
                "policy":{"sha256":sha(&raw),"bytes":raw.len(),
                    "blake2_256":hex::encode(sp_io::hashing::blake2_256(&raw))}}});
        companion_policy(&receipt, &raw, &original).unwrap();
        assert!(companion_policy(&receipt, &active.encode(), &active).is_err());
        assert!(companion_policy(&receipt, &raw, &active).is_err());
    }
    #[test]
    fn execute_arguments_bind_canonical_block_and_all_checks() {
        let block = RuntimeBlock {
            header: replay::ReplayHeader::new(
                3,
                H256::repeat_byte(1),
                H256::repeat_byte(2),
                H256::repeat_byte(3),
                Digest::default(),
            ),
            extrinsics: vec![],
        };
        let raw = block.encode();
        assert_eq!(
            execute_input(&raw).unwrap(),
            [raw.clone(), vec![1, 1, 1]].concat()
        );
        let mut trailing = raw;
        trailing.push(0);
        assert!(execute_input(&trailing).is_err());
        assert!(execute_input(&vec![0; MAX_BLOCK + 1]).is_err());
        assert!(decode::<Weight>(&[255]).is_err());
    }
    #[test]
    fn bad_policy_code_or_completion_marker_cannot_enter_companion() {
        let original = fixture();
        let mut active = original.clone();
        active.evidence_commitment = [8; 32];
        let mut ext = sp_io::TestExternalities::default();
        assert!(admission(&mut ext, b"production", &original, &active).is_err());
        let state = recovery::RecoveryStatus {
            format_version: 1,
            policy: original.commitment(),
            phase: RecoveryPhase::Complete,
            next_chunk: 1,
            operation_counter: 2,
            failure: None,
        };
        ext.execute_with(|| {
            for (key, raw) in [
                (b":code".as_slice(), b"production".to_vec()),
                (recovery::STATUS_STORAGE_KEY, state.encode()),
                (recovery::POLICY_STORAGE_KEY, original.commitment().to_vec()),
                (
                    recovery::ACTIVE_POLICY_STORAGE_KEY,
                    active.commitment().to_vec(),
                ),
                (recovery::BRIDGE_HALT_STORAGE_KEY, vec![1]),
                (recovery::INCOMING_AUDIT_PENDING_KEY, 0u32.encode()),
                (recovery::AUDIT_CURSOR_STORAGE_KEY, 1u32.encode()),
                (recovery::RECOVERY_ANCHOR_STORAGE_KEY, vec![9; 32]),
                (
                    recovery::TERMINAL_AUDIT_STORAGE_KEY,
                    (active.commitment(), active.terminal_audit_commitment, 1u32).encode(),
                ),
            ] {
                sp_io::storage::set(key, &raw);
            }
        });
        let root = replay::root(&mut ext);
        admission(&mut ext, b"production", &original, &active).unwrap();
        assert_eq!(root, replay::root(&mut ext));
        assert!(admission(&mut ext, b"companion", &original, &active).is_err());
        ext.execute_with(|| {
            sp_io::storage::set(recovery::INCOMING_AUDIT_PENDING_KEY, &1u32.encode())
        });
        assert!(admission(&mut ext, b"production", &original, &active).is_err());
    }
    #[test]
    fn policy_and_build_feature_source_changes_fail_closed() {
        let mut value = fixture();
        assert!(policy(&value.encode()).is_ok());
        value.authorization_domain.release_nonce = [7; 32];
        assert!(policy(&value.encode()).is_err());
        let mut a = json!({"sourceFingerprint":1,"sourceProvenance":2,"lockBefore":3,"lockAfter":3,"rustc":"pinned",
            "environmentHelper":4,"llvmWrapper":5,"environmentPolicy":{"controlled":{"FORCE_WASM_BUILD":"a","CARGO_TARGET_DIR":"a","X":"same"},"removedKeys":[]}});
        let mut b = a.clone();
        b["environmentPolicy"]["controlled"]["FORCE_WASM_BUILD"] = json!("b");
        same_build_inputs(&a, &b).unwrap();
        b["lockAfter"] = json!(4);
        assert!(same_build_inputs(&a, &b).is_err());
        b = a.clone();
        b["environmentPolicy"]["controlled"]["X"] = json!("different");
        assert!(same_build_inputs(&a, &b).is_err());
        a["sourceFingerprint"] = Value::Null;
        assert!(same_build_inputs(&a, &a).is_err());
    }
}
