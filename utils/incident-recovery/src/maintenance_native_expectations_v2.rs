//! Fresh native derivation, joined to this exact ordered Wasm rehearsal.
//! Native agreement is a compatibility witness, not independent economic approval.
use super::{replay, supplied_escape::WorkContext, Header, RuntimeBlock};
use crate::Result;
use codec::Encode;
use common::recovery;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use sp_core::H256;
use sp_runtime::{traits::Header as _, OpaqueExtrinsic};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const MAX_REPORT: usize = 4 * 1024 * 1024;
const MAX_LOG: u64 = 8 * 1024 * 1024;
const MAX_ROWS: usize = 4096;
const NATIVE_OPERATION_TIMEOUT: Duration = Duration::from_secs(3600);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub executable: replay::Artifact,
    pub engine_binding: replay::Artifact,
}

pub struct Recorder {
    executable: PathBuf,
    executable_sha256: String,
    binding: Value,
    binding_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Derivation {
    pub block: u32,
    pub operation_index: u32,
    pub report_file: String,
    pub report_sha256: String,
    pub process_receipt_file: String,
    pub process_receipt_sha256: String,
    pub executable_sha256: String,
    pub engine_binding_sha256: String,
    pub parent_replay_transcript_sha256: String,
    pub joined_to_verified_replay_prefix: bool,
    pub exact_authored_block_bytes_match: bool,
    pub complete_operation_delta_compared: bool,
    pub native_terminal_snapshot_independently_restored: bool,
    pub native_source_build_authenticity_verified: bool,
    pub source_finality_verified: bool,
    pub economic_policy_independently_verified: bool,
    pub fixture_only: bool,
    pub production_ready: bool,
}

pub struct Operation<'a> {
    pub output: &'a Path,
    pub snapshot_index: usize,
    pub snapshot_sha256: &'a str,
    pub parent: &'a Header,
    pub prior_transcript_sha256: &'a str,
    pub engine: &'a [u8],
    pub context: &'a WorkContext,
    pub envelope: &'a [u8],
    pub extrinsic: &'a OpaqueExtrinsic,
    pub slot: u64,
    pub authority_index: u32,
    pub timestamp: u64,
    pub operation_index: u32,
    pub operation_kind: &'a str,
    pub initialized_root: H256,
    pub operation_root: H256,
    pub runtime_version_scale: &'a [u8],
    pub terminal_status_scale: &'a [u8],
    pub block: &'a RuntimeBlock,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ValueHash {
    bytes: usize,
    blake2_256: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Delta {
    key: String,
    before: Option<ValueHash>,
    after: Option<ValueHash>,
    changed: bool,
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn canonical_hex(text: &str, bytes: Option<usize>, prefix: bool) -> Result<Vec<u8>> {
    let plain = if prefix {
        text.strip_prefix("0x").ok_or("hex prefix missing")?
    } else {
        text
    };
    let decoded = hex::decode(plain).map_err(|e| e.to_string())?;
    require(
        hex::encode(&decoded) == plain && bytes.is_none_or(|n| decoded.len() == n),
        "noncanonical hex or length",
    )?;
    Ok(decoded)
}
fn strict_path(base: &Path, relative: &str) -> Result<PathBuf> {
    require(
        base.is_absolute() && base.canonicalize().map_err(|e| e.to_string())? == base,
        "noncanonical input root",
    )?;
    require(
        !relative.contains('\\')
            && relative
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != ".."),
        "noncanonical relative path",
    )?;
    let relative = Path::new(relative);
    require(
        relative
            .components()
            .all(|p| matches!(p, Component::Normal(_))),
        "absolute artifact path",
    )?;
    let mut path = base.to_path_buf();
    for component in relative.components() {
        path.push(component);
        require(
            !fs::symlink_metadata(&path)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink(),
            "artifact symlink",
        )?;
    }
    Ok(path)
}
fn bounded_read(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(maximum as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    require(data.len() <= maximum, "bounded file exceeded")?;
    Ok(data)
}
fn executable_hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    let mut total = 0u64;
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or("executable size overflow")?;
        require(total <= 2 * 1024 * 1024 * 1024, "executable size bound")?;
        digest.update(&buffer[..count]);
    }
    require(total > 0, "empty executable")?;
    Ok(hex::encode(digest.finalize()))
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| e.to_string())
}
fn assert_fields(report: &Value, expected: &Value) -> Result<()> {
    for (key, expected_value) in expected.as_object().ok_or("expected object missing")? {
        require(
            report.get(key) == Some(expected_value),
            &format!("native recorder binding mismatch: {key}"),
        )?;
    }
    Ok(())
}
fn validate_delta(value: &Value) -> Result<usize> {
    let rows: Vec<Delta> = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    require(
        !rows.is_empty() && rows.len() <= MAX_ROWS,
        "native delta row bound",
    )?;
    let mut keys = BTreeSet::new();
    let mut total = 0usize;
    for row in &rows {
        let key = canonical_hex(&row.key, None, true)?;
        require(
            !key.is_empty() && key.len() <= recovery::MAX_STORAGE_KEY_BYTES && keys.insert(key),
            "native delta key duplicate or bound",
        )?;
        for value in [&row.before, &row.after].into_iter().flatten() {
            canonical_hex(&value.blake2_256, Some(32), true)?;
            require(value.bytes <= 32 * 1024 * 1024, "native delta value bound")?;
            total = total
                .checked_add(value.bytes)
                .ok_or("delta byte overflow")?;
        }
        require(
            row.changed == (row.before != row.after),
            "native delta changed flag mismatch",
        )?;
    }
    require(
        total <= 64 * 1024 * 1024,
        "native delta aggregate value bound",
    )?;
    Ok(rows.len())
}

fn unique_exact_binding<'a>(recorders: &'a [Recorder], expected: &Value) -> Result<&'a Recorder> {
    let mut matches = recorders
        .iter()
        .filter(|r| assert_fields(&r.binding, expected).is_ok());
    let found = matches
        .next()
        .ok_or("missing or duplicate exact repaired native engine")?;
    require(
        matches.next().is_none(),
        "missing or duplicate exact repaired native engine",
    )?;
    Ok(found)
}

impl Recorder {
    pub fn load(
        base: &Path,
        config: &Config,
        original_bytes: &[u8],
        original: &recovery::RecoveryPolicy,
        fixture_allowed: bool,
    ) -> Result<Self> {
        let base = base.canonicalize().map_err(|e| e.to_string())?;
        canonical_hex(&config.executable.sha256, Some(32), false)?;
        let executable = strict_path(&base, &config.executable.file)?;
        require(
            executable_hash(&executable)? == config.executable.sha256,
            "native executable checksum",
        )?;
        let binding_path = strict_path(&base, &config.engine_binding.file)?;
        let binding_bytes = bounded_read(&binding_path, MAX_REPORT)?;
        require(
            sha(&binding_bytes) == config.engine_binding.sha256,
            "native engine binding checksum",
        )?;
        let binding: Value = serde_json::from_slice(&binding_bytes).map_err(|e| e.to_string())?;
        assert_fields(
            &binding,
            &json!({"schema":"sora-nexus-native-operation-engine-binding-v2","originalPolicySha256":sha(original_bytes),"originalPolicyCommitment":hex::encode(original.commitment()),"sdk":"polkadot-stable2603-3","transactionVersion":132,"productionReady":false,"templateCarriesReviewedSdkAndTestPathRepairs":true}),
        )?;
        require(
            matches!(binding["specVersion"].as_u64(), Some(136 | 137)),
            "admission v2 requires a repaired engine",
        )?;
        for field in [
            "engineSha256",
            "engineBlake2",
            "policySha256",
            "policyCommitment",
        ] {
            canonical_hex(
                binding[field].as_str().ok_or("missing engine/policy pin")?,
                Some(32),
                false,
            )?;
        }
        let fixture = binding
            .get("fixtureOnly")
            .and_then(Value::as_bool)
            .ok_or("native fixture flag missing")?;
        require(
            !fixture || fixture_allowed,
            "fixture native recorder requires explicit public-fixture evidence",
        )?;
        require(
            binding.get("runtimeFeatureProfile").is_some()
                && binding.get("hostLockAmendment").is_some()
                && binding["templateCarriesReviewedSdkAndTestPathRepairs"] == true,
            "reviewed recorder build amendments absent",
        )?;
        Ok(Self {
            executable,
            executable_sha256: config.executable.sha256.clone(),
            binding,
            binding_sha256: config.engine_binding.sha256.clone(),
        })
    }

    pub fn select<'a>(
        recorders: &'a [Self],
        engine: &[u8],
        context: &WorkContext,
        spec: u32,
        tx: u32,
    ) -> Result<&'a Self> {
        require(
            context.spec() == spec
                && context.engine_hash() == sp_io::hashing::blake2_256(engine)
                && matches!(
                    (spec, tx, context.counter()),
                    (136, 132, 2) | (137, 132, 2..=3)
                ),
            "native repaired selection detached from accepted work context",
        )?;
        let expected = json!({"engineSha256":sha(engine),"engineBlake2":hex::encode(context.engine_hash()),
            "specVersion":spec,"transactionVersion":tx,"policyCommitment":hex::encode(context.active().commitment()),
            "policySha256":sha(&context.active().encode()),"originalPolicyCommitment":hex::encode(context.original().commitment()),
            "originalPolicySha256":sha(&context.original().encode())});
        unique_exact_binding(recorders, &expected)
    }

    pub fn derive(&self, input: Operation<'_>) -> Result<Derivation> {
        let policy = input.context.active();
        let policy_bytes = policy.encode();
        let original_bytes = input.context.original().encode();
        require(
            input.context.engine_hash() == sp_io::hashing::blake2_256(input.engine),
            "native context code differs",
        )?;
        assert_fields(
            &self.binding,
            &json!({"engineSha256":sha(input.engine),"policySha256":sha(&policy_bytes),
            "originalPolicySha256":sha(&original_bytes),"specVersion":input.context.spec()}),
        )?;
        require(
            executable_hash(&self.executable)? == self.executable_sha256,
            "native executable changed before launch",
        )?;
        let output = input.output.canonicalize().map_err(|e| e.to_string())?;
        require(output.is_absolute(), "maintenance output is not absolute")?;
        let number = *input.block.header.number();
        require(
            input.parent.number().checked_add(1) == Some(number)
                && input.block.header.parent_hash() == &input.parent.hash(),
            "native parent does not precede authored block",
        )?;
        canonical_hex(input.prior_transcript_sha256, Some(32), false)?;
        let operation_commitment = policy
            .chunk_hashes
            .get(input.operation_index as usize)
            .ok_or("uncommitted native operation index")?;
        let directory_name = format!("native-operation-{number}");
        let directory = output.join(&directory_name);
        fs::create_dir(&directory)
            .map_err(|e| format!("native derivation output must be new:{e}"))?;
        let parent_bytes = input.parent.encode();
        let context_bytes = serde_json::to_vec_pretty(&input.context.manifest(
            input.parent,
            input.snapshot_sha256,
            input.prior_transcript_sha256,
        ))
        .map_err(|e| e.to_string())?;
        let evidence = serde_json::to_vec_pretty(&json!({"schema":"sora-nexus-native-operation-replay-prefix-v2","recoveryContextSha256":sha(&context_bytes),"verifiedPrefixBlocks":input.snapshot_index,"priorTranscriptSha256":input.prior_transcript_sha256,"parentHash":format!("{:#x}",input.parent.hash()),"parentStateRoot":format!("{:#x}",input.parent.state_root()),"parentSnapshotManifestSha256":input.snapshot_sha256,"sourceFinalityVerified":false,"productionReady":false})).map_err(|e| e.to_string())?;
        let root = output.parent().ok_or("maintenance output lacks parent")?;
        let output_name = output
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("non-UTF8 output name")?;
        let shared_name = format!("native-operation-inputs-{}", sha(input.engine));
        let shared = output.join(&shared_name);
        if !shared.exists() {
            fs::create_dir(&shared).map_err(|e| e.to_string())?;
        }
        require(
            strict_path(&output, &shared_name)? == shared && shared.is_dir(),
            "native shared input directory differs",
        )?;
        for (name, bytes) in [
            ("engine.wasm", input.engine),
            ("policy.scale", policy_bytes.as_slice()),
            ("original-policy.scale", original_bytes.as_slice()),
        ] {
            let path = shared.join(name);
            if path.exists() {
                require(
                    strict_path(&output, &format!("{shared_name}/{name}"))? == path
                        && bounded_read(&path, 32 * 1024 * 1024)? == bytes,
                    "shared native engine/policy changed",
                )?;
            } else {
                write_new(&path, bytes)?;
            }
        }
        for (name, bytes) in [
            ("parent.scale", parent_bytes.as_slice()),
            ("prior-replay.json", evidence.as_slice()),
            ("envelope.scale", input.envelope),
            ("recovery-context.json", context_bytes.as_slice()),
        ] {
            write_new(&directory.join(name), bytes)?;
        }
        let reference = |name: &str, bytes: &[u8]| json!({"file":format!("{output_name}/{directory_name}/{name}"),"sha256":sha(bytes)});
        let manifest = json!({"schema":"sora-nexus-native-operation-recorder-v2","inputRoot":root,
            "engine":{"file":format!("{output_name}/{shared_name}/engine.wasm"),"sha256":sha(input.engine)},
            "policy":{"file":format!("{output_name}/{shared_name}/policy.scale"),"sha256":sha(&policy_bytes)},
            "originalPolicy":{"file":format!("{output_name}/{shared_name}/original-policy.scale"),"sha256":sha(&original_bytes)},
            "recoveryContext":reference("recovery-context.json",&context_bytes),
            "parentHeader":reference("parent.scale",&parent_bytes),"parentSnapshotDirectory":output_name,
            "parentSnapshotManifest":format!("snapshots/{:06}.json",input.snapshot_index),"parentSnapshotSha256":input.snapshot_sha256,
            "priorReplayEvidence":reference("prior-replay.json",&evidence),"envelope":reference("envelope.scale",input.envelope),
            "slot":input.slot,"authorityIndex":input.authority_index,"timestamp":input.timestamp});
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
        let manifest_path = directory.join("manifest.json");
        write_new(&manifest_path, &manifest_bytes)?;
        let native_output = directory.join("recorded");
        let stdout_path = directory.join("stdout.json");
        let stderr_path = directory.join("stderr.log");
        let stdout = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stdout_path)
            .map_err(|e| e.to_string())?;
        let stderr = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stderr_path)
            .map_err(|e| e.to_string())?;
        let started_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        let started = Instant::now();
        let mut child = ChildGuard(
            Command::new(&self.executable)
                .current_dir(&output)
                .arg(&manifest_path)
                .arg(&native_output)
                .stdin(Stdio::null())
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr))
                .spawn()
                .map_err(|e| format!("native recorder launch:{e}"))?,
        );
        let pid = child.0.id();
        let mut stop_reason = None;
        let exit = loop {
            if let Some(exit) = child.0.try_wait().map_err(|e| e.to_string())? {
                break exit;
            }
            if started.elapsed() > NATIVE_OPERATION_TIMEOUT {
                stop_reason = Some("timeout");
            }
            for log in [&stdout_path, &stderr_path] {
                if fs::metadata(log).map_err(|e| e.to_string())?.len() > MAX_LOG {
                    stop_reason = Some("log bound");
                }
            }
            if stop_reason.is_some() {
                child.0.kill().map_err(|e| e.to_string())?;
                break child.0.wait().map_err(|e| e.to_string())?;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let stdout_bytes = bounded_read(&stdout_path, MAX_LOG as usize)?;
        let stderr_bytes = bounded_read(&stderr_path, MAX_LOG as usize)?;
        let receipt = serde_json::to_vec_pretty(&json!({"schema":"sora-nexus-native-operation-process-v1","pid":pid,"cwd":output,"environmentPolicy":"inherited; source/build authenticity is not established here","executable":self.executable,"executableSha256":self.executable_sha256,"arguments":[manifest_path,native_output],"startedUnixMillis":started_ms,"elapsedMillis":started.elapsed().as_millis(),"exitCode":exit.code(),"success":exit.success(),"stopReason":stop_reason,"stdoutSha256":sha(&stdout_bytes),"stderrSha256":sha(&stderr_bytes),"manifestSha256":sha(&manifest_bytes),"sourceFinalityVerified":false,"productionReady":false})).map_err(|e|e.to_string())?;
        write_new(&directory.join("process.json"), &receipt)?;
        require(
            exit.success() && stop_reason.is_none(),
            "native recorder process failed",
        )?;
        require(
            executable_hash(&self.executable)? == self.executable_sha256,
            "native executable changed after launch",
        )?;
        require(
            bounded_read(&manifest_path, MAX_REPORT)? == manifest_bytes,
            "native manifest changed",
        )?;
        let report_bytes = bounded_read(&native_output.join("report.json"), MAX_REPORT)?;
        let report: Value = serde_json::from_slice(&report_bytes).map_err(|e| e.to_string())?;
        let stdout_report: Value =
            serde_json::from_slice(&stdout_bytes).map_err(|e| e.to_string())?;
        require(
            report == stdout_report,
            "native process output differs from completed report",
        )?;
        let embedded: Value = serde_json::from_str(
            report["engineBinding"]
                .as_str()
                .ok_or("native engine binding absent")?,
        )
        .map_err(|e| e.to_string())?;
        require(
            embedded == self.binding,
            "native binary embedded binding differs",
        )?;
        let block_bytes = input.block.encode();
        let extrinsic_bytes = input.extrinsic.encode();
        assert_fields(
            &report,
            &json!({"schema":"sora-nexus-native-operation-expectations-v2","nativeExecutable":self.executable,"nativeExecutableSha256":self.executable_sha256,"manifestSha256":sha(&manifest_bytes),"engineSha256":sha(input.engine),"originalPolicyCommitment":hx(&input.context.original().commitment()),"originalPolicySha256":sha(&original_bytes),"activePolicySha256":sha(&policy_bytes),"specVersion":input.context.spec(),"transactionVersion":132,"operationCounter":input.context.counter(),"recoveryContextSha256":sha(&context_bytes),"parentRepairAuthorizationSha256":input.context.parent_authorization_sha256(),"repairHandoverLifecycleChecked":true,"policyCommitment":hx(&policy.commitment())}),
        )?;
        assert_fields(
            &report,
            &json!({"parentHeaderSha256":sha(&parent_bytes),"parentHash":format!("{:#x}",input.parent.hash()),"parentStateRoot":format!("{:#x}",input.parent.state_root()),"parentSnapshotManifestSha256":input.snapshot_sha256,"priorReplayEvidenceSha256":sha(&evidence),"initializedStateRoot":format!("{:#x}",input.initialized_root),"originalEnvelopeSha256":sha(input.envelope),"originalExtrinsicSha256":sha(&extrinsic_bytes),"operationKind":input.operation_kind,"operationIndex":input.operation_index,"operationCommitment":hx(operation_commitment),"nativeRuntimeVersionScale":hx(input.runtime_version_scale),"operationStateRoot":format!("{:#x}",input.operation_root),"terminalStatusScale":hx(input.terminal_status_scale),"blockSha256":sha(&block_bytes),"blockHash":format!("{:#x}",input.block.header.hash()),"stateRoot":format!("{:#x}",input.block.header.state_root()),"completeTopWriteSetCompared":true,"consensusPrefixWaiver":false,"childWritesSupported":false,"compiledPolicyOverrideUsed":false,"configurationInjected":false,"nativeWasmOperationParityVerified":true,"independentImportVerified":true,"freshDiskRestoreVerified":true,"priorReplayLineageVerified":false,"sourceFinalityVerified":false,"economicPolicyIndependentlyVerified":false,"consensusSealsVerified":false,"holdExecutionSupported":false,"repairPrefixVerifiedByStandalone":false,"fixtureOnly":self.binding["fixtureOnly"],"productionReady":false}),
        )?;
        validate_delta(&report["writes"])?;
        let terminal_snapshot = report["terminalSnapshotSha256"]
            .as_str()
            .ok_or("native snapshot pin absent")?;
        canonical_hex(terminal_snapshot, Some(32), false)?;
        let mut independently_restored = super::snapshot::restore_pinned(
            &native_output,
            "snapshots/000000.json",
            terminal_snapshot,
            *input.block.header.state_root(),
        )?;
        require(
            replay::root(&mut independently_restored) == *input.block.header.state_root(),
            "native terminal journal differs from main authored state",
        )?;
        drop(independently_restored);
        require(
            bounded_read(
                &native_output.join("block.scale"),
                recovery::MAX_INHERENT_BYTES + 1024 * 1024,
            )? == block_bytes,
            "native block bytes differ from exact authored block",
        )?;
        require(
            bounded_read(
                &native_output.join("original-extrinsic.scale"),
                recovery::MAX_INHERENT_BYTES + 1024,
            )? == extrinsic_bytes,
            "native original extrinsic differs",
        )?;
        Ok(Derivation {
            block: number,
            operation_index: input.operation_index,
            report_file: format!("{directory_name}/recorded/report.json"),
            report_sha256: sha(&report_bytes),
            process_receipt_file: format!("{directory_name}/process.json"),
            process_receipt_sha256: sha(&receipt),
            executable_sha256: self.executable_sha256.clone(),
            engine_binding_sha256: self.binding_sha256.clone(),
            parent_replay_transcript_sha256: input.prior_transcript_sha256.into(),
            joined_to_verified_replay_prefix: true,
            exact_authored_block_bytes_match: true,
            complete_operation_delta_compared: true,
            native_terminal_snapshot_independently_restored: true,
            native_source_build_authenticity_verified: false,
            source_finality_verified: false,
            economic_policy_independently_verified: false,
            fixture_only: self.binding["fixtureOnly"]
                .as_bool()
                .ok_or("fixture flag missing")?,
            production_ready: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(key: &str) -> Value {
        serde_json::to_value(Delta {
            key: key.into(),
            before: None,
            after: Some(ValueHash {
                bytes: 1,
                blake2_256: hx(&[1u8; 32]),
            }),
            changed: true,
        })
        .unwrap()
    }
    #[test]
    fn all_storage_prefixes_are_retained_and_noop_is_valid() {
        let system = hex::encode(sp_io::hashing::twox_128(b"System"));
        let changed = row(&format!("0x{system}"));
        let mut noop = row("0x01");
        noop["before"] = noop["after"].clone();
        noop["changed"] = json!(false);
        assert_eq!(validate_delta(&json!([changed, noop])).unwrap(), 2);
    }
    #[test]
    fn delta_rejects_duplicate_keys_wrong_flags_and_noncanonical_hashes() {
        let original = row("0x01");
        assert!(validate_delta(&json!([original.clone(), original.clone()])).is_err());
        let mut wrong = original.clone();
        wrong["changed"] = json!(false);
        assert!(validate_delta(&json!([wrong])).is_err());
        let mut wrong: Delta = serde_json::from_value(original).unwrap();
        wrong.after.as_mut().unwrap().blake2_256 = "0XAA".into();
        assert!(validate_delta(&json!([wrong])).is_err());
    }
    #[test]
    fn exact_binding_rejects_changed_input_and_false_parity() {
        let expected = json!({"parentHash":"exact","completeTopWriteSetCompared":true});
        assert!(assert_fields(&expected, &expected).is_ok());
        assert!(assert_fields(
            &json!({"parentHash":"abandoned","completeTopWriteSetCompared":true}),
            &expected
        )
        .is_err());
        assert!(assert_fields(
            &json!({"parentHash":"exact","completeTopWriteSetCompared":false}),
            &expected
        )
        .is_err());
    }
    fn test_recorder(binding: Value) -> Recorder {
        Recorder {
            executable: PathBuf::from("/test-only/not-executed"),
            executable_sha256: "test-only".into(),
            binding,
            binding_sha256: "test-only".into(),
        }
    }
    #[test]
    fn engine_selection_rejects_duplicate_swapped_policy_and_same_version_other_code() {
        let expected = json!({"specVersion":136,"transactionVersion":132,"engineSha256":"engine-a","policyCommitment":"active-a","originalPolicyCommitment":"original"});
        let control = vec![test_recorder(expected.clone())];
        assert!(unique_exact_binding(&control, &expected).is_ok());
        assert!(unique_exact_binding(&[], &expected).is_err());
        let duplicate = vec![
            test_recorder(expected.clone()),
            test_recorder(expected.clone()),
        ];
        assert!(unique_exact_binding(&duplicate, &expected).is_err());
        for field in [
            "engineSha256",
            "policyCommitment",
            "originalPolicyCommitment",
            "specVersion",
            "transactionVersion",
        ] {
            let mut changed = expected.clone();
            changed[field] = json!("different");
            assert!(unique_exact_binding(&[test_recorder(changed)], &expected).is_err());
        }
    }
}
