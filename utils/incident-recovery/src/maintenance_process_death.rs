//! Opt-in committed-snapshot process death. No node database or consensus claim.
//!
//! A fresh in-process baseline is the sole source of expected block roots and
//! opaque incoming evidence. Every importer resumes the preceding killed
//! importer's disk snapshot. Worker JSON never constructs an incoming certificate.

use super::{replay, snapshot, Manifest, RuntimeBlock};
use crate::{hex_bytes, Result};
use codec::{DecodeAll, Encode};
use common::recovery;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use sp_core::H256;
use sp_runtime::traits::Header as _;
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, process::ExitStatusExt},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const DEADLINE: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_LOG: u64 = 64 * 1024 * 1024;
const MAX_JOB: usize = 64 * 1024;
const MAX_CODE: usize = 50 * 1024 * 1024;
const MAX_BLOCK: usize = 64 * 1024 * 1024;
const WORKER_SCHEMA: &str = "sora-nexus-maintenance-crash-worker-v1";

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
fn hash(value: &str) -> Result<H256> {
    let bytes: [u8; 32] = hex_bytes(value)?
        .try_into()
        .map_err(|_| "wrong hash length")?;
    need(
        value == format!("{:#x}", H256::from(bytes)),
        "noncanonical hash",
    )?;
    Ok(bytes.into())
}
fn physical(path: &Path) -> Result<()> {
    need(path.is_absolute(), "absolute owned path required")?;
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part);
        need(
            !fs::symlink_metadata(&current)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink(),
            "aliased process-death path",
        )?;
    }
    need(
        path.canonicalize().map_err(|e| e.to_string())? == path,
        "noncanonical process-death path",
    )
}
fn owned(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    need(
        !relative.is_empty()
            && !relative.contains('\\')
            && !path.is_absolute()
            && path
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
            && path.to_str() == Some(relative),
        "noncanonical worker relative path",
    )?;
    let path = root.join(path);
    physical(&path)?;
    Ok(path)
}
fn sync_dir(path: &Path) -> Result<()> {
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    sync_dir(path.parent().ok_or("missing output parent")?)
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    write_new(
        path,
        &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
}
fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    need(!path.exists(), "barrier already exists")?;
    let temporary = path.with_extension("next");
    write_json(&temporary, value)?;
    fs::rename(temporary, path).map_err(|e| e.to_string())?;
    sync_dir(path.parent().ok_or("missing barrier parent")?)
}
fn bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    physical(path)?;
    need(
        fs::metadata(path).map_err(|e| e.to_string())?.is_file(),
        "regular input required",
    )?;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    need(bytes.len() <= limit, "process-death input bound")?;
    Ok(bytes)
}
fn file_sha(path: &Path, limit: u64) -> Result<String> {
    physical(path)?;
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut chunk = [0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        count += n as u64;
        need(count <= limit, "process-death file bound")?;
        digest.update(&chunk[..n]);
    }
    Ok(hex::encode(digest.finalize()))
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pin {
    file: String,
    sha256: String,
}
impl Pin {
    fn expected(root: &Path, path: &Path, sha256: String) -> Result<Self> {
        Ok(Self {
            file: path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("non-UTF8 evidence path")?
                .into(),
            sha256,
        })
    }
    fn from_bytes(root: &Path, path: &Path, bytes: &[u8]) -> Result<Self> {
        Self::expected(root, path, sha(bytes))
    }
    fn verify(&self, root: &Path, limit: u64) -> Result<()> {
        need(
            file_sha(&owned(root, &self.file)?, limit)? == self.sha256,
            "retained expected bytes changed",
        )
    }
    fn read(&self, root: &Path, limit: usize) -> Result<Vec<u8>> {
        let bytes = bounded(&owned(root, &self.file)?, limit)?;
        need(sha(&bytes) == self.sha256, "pinned worker input changed")?;
        Ok(bytes)
    }
}
fn write_json_pin(root: &Path, path: &Path, value: &impl Serialize) -> Result<Pin> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let pin = Pin::from_bytes(root, path, &bytes)?;
    write_new(path, &bytes)?;
    Ok(pin)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Job {
    schema: String,
    index: u32,
    parent_snapshot: Pin,
    parent_root: String,
    parent_hash: String,
    parent_number: u32,
    block: Option<Pin>,
    engine: Option<Pin>,
    bootstrap_blake2: String,
    deployed_blake2: String,
    expected_root: String,
    expected_hash: String,
}
impl Job {
    fn validate(&self) -> Result<()> {
        need(
            self.schema == WORKER_SCHEMA && self.index < 100_000,
            "worker schema/index bound",
        )?;
        need(
            self.parent_number
                == recovery::CHECKPOINT_NUMBER
                    .checked_add(self.index)
                    .ok_or("worker height overflow")?,
            "worker parent height gap",
        )?;
        need(
            self.parent_snapshot.file == format!("state/snapshots/{:06}.json", self.index),
            "worker snapshot index gap/reset",
        )?;
        for value in [
            &self.parent_root,
            &self.parent_hash,
            &self.expected_root,
            &self.expected_hash,
        ] {
            hash(value)?;
        }
        need(
            self.block.is_some() == self.engine.is_some(),
            "worker block/engine mismatch",
        )?;
        if self.block.is_none() {
            need(
                self.parent_root == self.expected_root && self.parent_hash == self.expected_hash,
                "terminal restore target changed",
            )?;
        }
        for value in [&self.bootstrap_blake2, &self.deployed_blake2] {
            need(
                value.len() == 64 && hex::decode(value).is_ok() && value.to_lowercase() == *value,
                "worker code hash shape",
            )?;
        }
        Ok(())
    }
}

/// Internal child entry point. Untrusted standalone jobs cannot produce a
/// baseline witness or opaque incoming certificate. Successful children wait
/// for the coordinator's actual SIGKILL after the durable barrier.
pub fn worker(job_path: &Path) -> Result<()> {
    physical(job_path)?;
    let jobs = job_path.parent().ok_or("worker jobs directory absent")?;
    need(
        jobs.file_name().is_some_and(|name| name == "jobs"),
        "worker jobs directory identity",
    )?;
    let root = jobs.parent().ok_or("worker root absent")?;
    let bytes = bounded(job_path, MAX_JOB)?;
    let job: Job = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    job.validate()?;
    need(
        job_path.file_name().and_then(|name| name.to_str())
            == Some(format!("{:06}.json", job.index).as_str()),
        "worker job filename/index mismatch",
    )?;
    let state = root.join("state");
    let prior = job
        .parent_snapshot
        .file
        .strip_prefix("state/")
        .ok_or("worker parent snapshot location")?;
    let (snapshot, root_hash) = if let Some(block_pin) = &job.block {
        let (mut journal, mut ext) = snapshot::Journal::resume_pinned(
            &state,
            prior,
            &job.parent_snapshot.sha256,
            hash(&job.parent_root)?,
        )?;
        let bytes = block_pin.read(root, MAX_BLOCK)?;
        let block = RuntimeBlock::decode_all(&mut bytes.as_slice()).map_err(|e| e.to_string())?;
        need(block.encode() == bytes, "noncanonical worker block")?;
        need(
            *block.header.number() == job.parent_number + 1
                && *block.header.parent_hash() == hash(&job.parent_hash)?
                && block.header.hash() == hash(&job.expected_hash)?
                && *block.header.state_root() == hash(&job.expected_root)?,
            "worker block ancestry/root mismatch",
        )?;
        let code = job
            .engine
            .as_ref()
            .ok_or("worker engine absent")?
            .read(root, MAX_CODE)?;
        let stored = replay::code(&mut ext)?;
        let code_hash = hex::encode(sp_io::hashing::blake2_256(&code));
        need(
            code == stored
                || (hex::encode(sp_io::hashing::blake2_256(&stored)) == job.deployed_blake2
                    && code_hash == job.bootstrap_blake2),
            "worker runtime selection differs from stored code or pinned substitute",
        )?;
        let executor = replay::Executor::builder().build();
        need(
            replay::call(&executor, &mut ext, &code, "Core_execute_block", &bytes)?.is_empty(),
            "worker import returned bytes",
        )?;
        need(
            replay::root(&mut ext) == hash(&job.expected_root)?,
            "worker imported root mismatch",
        )?;
        let (mut restored, sha256) = journal.persist(ext)?;
        need(
            replay::root(&mut restored) == hash(&job.expected_root)?,
            "worker disk root mismatch",
        )?;
        (
            Pin {
                file: format!("state/snapshots/{:06}.json", job.index + 1),
                sha256,
            },
            replay::root(&mut restored),
        )
    } else {
        let mut ext = snapshot::restore_pinned(
            &state,
            prior,
            &job.parent_snapshot.sha256,
            hash(&job.parent_root)?,
        )?;
        (job.parent_snapshot.clone(), replay::root(&mut ext))
    };
    for directory in [
        &state.join("snapshot-objects"),
        &state.join("snapshots"),
        &state,
    ] {
        sync_dir(directory)?;
    }
    need(
        sha(&bounded(job_path, MAX_JOB)?) == sha(&bytes),
        "worker job changed",
    )?;
    let ready = json!({"schema":WORKER_SCHEMA,"pid":std::process::id(),"jobSha256":sha(&bytes),
        "parentSnapshot":job.parent_snapshot,"snapshot":snapshot,"root":format!("{root_hash:#x}"),
        "blockHash":job.expected_hash,"imported":job.block.is_some(),"durableSnapshotBarrier":true,
        "standaloneEvidenceAdmissible":false});
    atomic_json(
        &root.join(format!("stages/{:06}/ready.json", job.index)),
        &ready,
    )?;
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn ready_matches(ready: &Value, job: &Job, job_sha: &str, pid: u32) -> Result<Pin> {
    need(
        ready["schema"] == WORKER_SCHEMA
            && ready["pid"] == pid
            && ready["jobSha256"] == job_sha
            && ready["parentSnapshot"]
                == serde_json::to_value(&job.parent_snapshot).map_err(|e| e.to_string())?
            && ready["root"] == job.expected_root
            && ready["blockHash"] == job.expected_hash
            && ready["imported"] == job.block.is_some()
            && ready["durableSnapshotBarrier"] == true
            && ready["standaloneEvidenceAdmissible"] == false,
        "worker barrier identity/root mismatch",
    )?;
    let pin: Pin = serde_json::from_value(ready["snapshot"].clone()).map_err(|e| e.to_string())?;
    let index = job.index + u32::from(job.block.is_some());
    need(
        pin.file == format!("state/snapshots/{index:06}.json"),
        "worker continuation reset/skipped snapshot",
    )?;
    if job.block.is_none() {
        need(
            pin == job.parent_snapshot,
            "terminal worker replaced snapshot",
        )?;
    }
    Ok(pin)
}
fn stage(root: &Path, executable: &Path, executable_sha: &str, job: &Job) -> Result<(Pin, Value)> {
    job.validate()?;
    let stage = root.join(format!("stages/{:06}", job.index));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let job_path = root.join(format!("jobs/{:06}.json", job.index));
    let job_pin = write_json_pin(root, &job_path, job)?;
    let job_sha = job_pin.sha256.clone();
    need(
        file_sha(executable, 2 * 1024 * 1024 * 1024)? == executable_sha,
        "worker executable changed before start",
    )?;
    let stdout = stage.join("stdout.log");
    let stderr = stage.join("stderr.log");
    let out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stdout)
        .map_err(|e| e.to_string())?;
    let err = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stderr)
        .map_err(|e| e.to_string())?;
    let launch = json!({"schema":"sora-nexus-maintenance-crash-launch-v1",
        "executableSha256":executable_sha,"jobSha256":job_sha,
        "command":[executable.to_string_lossy(),"maintenance-crash-worker","--job",job_path.to_string_lossy()],
        "deadlineSeconds":DEADLINE.as_secs(),"productionReady":false});
    let launch_pin = write_json_pin(root, &stage.join("launch.json"), &launch)?;
    let launched = Command::new(executable)
        .args(["maintenance-crash-worker", "--job"])
        .arg(&job_path)
        .current_dir(root)
        .env_clear()
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .spawn();
    let mut child = match launched {
        Ok(child) => ChildGuard(child),
        Err(error) => {
            write_json(
                &stage.join("process.json"),
                &json!({"schema":"sora-nexus-maintenance-process-death-v1",
                "pid":null,"actualExitCode":null,"actualExitSignal":null,
                "deliberateKillAfterBarrier":false,"killRequested":false,
                "failure":format!("worker launch failed: {error}"),"executableSha256":executable_sha,
                "jobSha256":job_sha,"productionReady":false}),
            )?;
            return Err(format!("worker launch failed: {error}"));
        }
    };
    let pid = child.0.id();
    let started_pin = write_json_pin(
        root,
        &stage.join("started.json"),
        &json!({"pid":pid,"executableSha256":executable_sha,
        "command":[executable.to_string_lossy(),"maintenance-crash-worker","--job",job_path.to_string_lossy()],
        "jobSha256":job_sha,"deadlineSeconds":DEADLINE.as_secs(),"killScope":"owned importer process; no subprocesses are spawned by the worker"}),
    )?;
    let started = Instant::now();
    let observed: Result<(Value, Pin, Pin)> = (|| {
        loop {
            need(
                started.elapsed() < DEADLINE,
                "worker timeout; not a deliberate-crash success",
            )?;
            for path in [&stdout, &stderr] {
                need(
                    fs::metadata(path).map_err(|e| e.to_string())?.len() <= MAX_LOG,
                    "worker output limit",
                )?;
            }
            need(
                child.0.try_wait().map_err(|e| e.to_string())?.is_none(),
                "worker exited before deliberate kill",
            )?;
            let path = stage.join("ready.json");
            if path.exists() {
                let ready_bytes = bounded(&path, MAX_JOB)?;
                let value: Value =
                    serde_json::from_slice(&ready_bytes).map_err(|e| e.to_string())?;
                let ready_pin = Pin::from_bytes(root, &path, &ready_bytes)?;
                let pin = ready_matches(&value, job, &job_sha, pid)?;
                sync_dir(&stage)?;
                need(
                    file_sha(executable, 2 * 1024 * 1024 * 1024)? == executable_sha
                        && file_sha(&job_path, MAX_JOB as u64)? == job_sha,
                    "worker executable/job changed",
                )?;
                // Reconstruct the child-written snapshot while the worker is parked.
                let relative = pin
                    .file
                    .strip_prefix("state/")
                    .ok_or("child snapshot prefix")?;
                let mut ext = snapshot::restore_pinned(
                    &root.join("state"),
                    relative,
                    &pin.sha256,
                    hash(&job.expected_root)?,
                )?;
                need(
                    replay::root(&mut ext) == hash(&job.expected_root)?,
                    "coordinator snapshot root differs from fresh baseline",
                )?;
                need(
                    started.elapsed() < DEADLINE,
                    "snapshot verification exceeded worker deadline",
                )?;
                return Ok((value, pin, ready_pin));
            }
            thread::sleep(Duration::from_millis(250));
        }
    })();
    let deliberate = observed.is_ok();
    let kill_result = child.0.kill().map_err(|e| e.to_string());
    let exit = child.0.wait().map_err(|e| e.to_string())?;
    let stdout_hash = file_sha(&stdout, MAX_LOG);
    let stderr_hash = file_sha(&stderr, MAX_LOG);
    let process = json!({"schema":"sora-nexus-maintenance-process-death-v1","pid":pid,
        "deliberateKillAfterBarrier":deliberate,"killRequested":true,"killError":kill_result.as_ref().err(),
        "actualExitCode":exit.code(),"actualExitSignal":exit.signal(),"elapsedMillis":started.elapsed().as_millis(),
        "failure":observed.as_ref().err(),"jobSha256":job_sha,"executableSha256":executable_sha,
        "stdoutSha256":stdout_hash.as_ref().ok(),"stderrSha256":stderr_hash.as_ref().ok(),
        "stdoutHashError":stdout_hash.as_ref().err(),"stderrHashError":stderr_hash.as_ref().err()});
    let process_pin = write_json_pin(root, &stage.join("process.json"), &process)?;
    let (ready, pin, ready_pin) = observed?;
    let stdout_pin = Pin::expected(root, &stdout, stdout_hash?)?;
    let stderr_pin = Pin::expected(root, &stderr, stderr_hash?)?;
    kill_result?;
    need(
        exit.signal() == Some(9) && exit.code().is_none(),
        "worker did not exit from actual SIGKILL",
    )?;
    need(
        file_sha(executable, 2 * 1024 * 1024 * 1024)? == executable_sha,
        "worker executable changed after kill",
    )?;
    let relative = pin
        .file
        .strip_prefix("state/")
        .ok_or("child snapshot prefix")?;
    let mut ext = snapshot::restore_pinned(
        &root.join("state"),
        relative,
        &pin.sha256,
        hash(&job.expected_root)?,
    )?;
    need(
        replay::root(&mut ext) == hash(&job.expected_root)?,
        "killed process snapshot changed",
    )?;
    for expected in [
        &launch_pin,
        &started_pin,
        &job_pin,
        &ready_pin,
        &process_pin,
        &stdout_pin,
        &stderr_pin,
    ] {
        expected.verify(root, MAX_LOG)?;
    }
    Ok((
        pin,
        json!({"index":job.index,"kind":if job.block.is_some(){"import"}else{"terminal-restore"},
        "ready":ready,"process":process,"snapshotAuthenticatedAfterKill":true,
        "launchReceipt":launch_pin,
        "processReceipt":process_pin,
        "readyReceipt":ready_pin,
        "startedReceipt":started_pin,
        "jobReceipt":job_pin,
        "stdoutReceipt":stdout_pin,
        "stderrReceipt":stderr_pin}),
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    schema: &'static str,
    baseline: super::Report,
    incoming_replay_binding: crate::incoming_replay::CoverageBinding,
    executable_sha256: String,
    final_identity: Value,
    workers: Vec<Value>,
    coverage: &'static str,
    all_blocks_imported_from_continued_snapshots: bool,
    terminal_snapshot_restored_in_fresh_process: bool,
    in_commit_crash_verified: bool,
    consensus_seals_verified: bool,
    production_ready: bool,
}

/// Execute the trust-establishing baseline afresh, retaining its opaque
/// certificate in this process. Existing run()/CLI behavior is unchanged.
pub fn run(manifest_path: &Path, output: &Path, require_prior_incoming: bool) -> Result<Report> {
    physical(manifest_path)?;
    need(
        !output.exists() && output.is_absolute(),
        "fresh absolute crash output required",
    )?;
    physical(output.parent().ok_or("crash output parent absent")?)?;
    let manifest_bytes = bounded(manifest_path, 64 * 1024 * 1024)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
    need(
        !manifest.steps.is_empty() && manifest.steps.len() < 100_000,
        "crash rehearsal step bound",
    )?;
    fs::create_dir(output).map_err(|e| e.to_string())?;
    for name in ["jobs", "stages", "inputs"] {
        fs::create_dir(output.join(name)).map_err(|e| e.to_string())?;
    }
    let source_executable = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let executable_sha = file_sha(&source_executable, 2 * 1024 * 1024 * 1024)?;
    let executable = output.join("worker-executable");
    fs::copy(&source_executable, &executable).map_err(|e| e.to_string())?;
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o500))
        .map_err(|e| e.to_string())?;
    need(
        file_sha(&executable, 2 * 1024 * 1024 * 1024)? == executable_sha,
        "copied worker differs",
    )?;
    let (baseline, incoming) = super::run_inner(
        manifest_path,
        &output.join("baseline"),
        require_prior_incoming,
        false,
        false,
    )?;
    need(
        sha(&bounded(manifest_path, 64 * 1024 * 1024)?) == sha(&manifest_bytes)
            && baseline.manifest_sha256 == sha(&manifest_bytes)
            && baseline.blocks.len() == manifest.steps.len(),
        "baseline manifest identity changed",
    )?;
    let base = manifest_path
        .parent()
        .ok_or("maintenance manifest parent absent")?;
    let mut engines = std::collections::BTreeMap::new();
    let mut input_pins = Vec::new();
    for reference in [&manifest.bootstrap, &manifest.hold, &manifest.final_runtime]
        .into_iter()
        .chain(&manifest.repairs)
    {
        let bytes = replay::artifact(base, reference, MAX_CODE)?;
        let blake = hex::encode(sp_io::hashing::blake2_256(&bytes));
        let path = output.join(format!("inputs/{blake}.wasm"));
        write_new(&path, &bytes)?;
        let pin = Pin::from_bytes(output, &path, &bytes)?;
        input_pins.push(pin.clone());
        need(
            engines.insert(blake, pin).is_none(),
            "duplicate crash engine",
        )?;
    }
    let bootstrap = replay::artifact(base, &manifest.bootstrap, MAX_CODE)?;
    let bootstrap_blake2 = hex::encode(sp_io::hashing::blake2_256(&bootstrap));
    let original = output.join("baseline/snapshots/000000.json");
    let initial_bytes = bounded(&original, MAX_JOB)?;
    let initial: Value = serde_json::from_slice(&initial_bytes).map_err(|e| e.to_string())?;
    let base_file = initial["base"]["file"]
        .as_str()
        .ok_or("initial snapshot base absent")?;
    need(
        initial["index"] == 0 && initial["parent"].is_null() && initial["delta"].is_null(),
        "initial snapshot is not baseline checkpoint",
    )?;
    let initial_root = H256::from(recovery::CHECKPOINT_STATE_ROOT);
    let mut ext = snapshot::restore_pinned(
        &output.join("baseline"),
        "snapshots/000000.json",
        &sha(&initial_bytes),
        initial_root,
    )?;
    let deployed_blake2 = hex::encode(sp_io::hashing::blake2_256(&replay::code(&mut ext)?));
    drop(ext);
    let state = output.join("state");
    fs::create_dir(&state).map_err(|e| e.to_string())?;
    for name in ["snapshots", "snapshot-objects"] {
        fs::create_dir(state.join(name)).map_err(|e| e.to_string())?;
    }
    let source = owned(&output.join("baseline"), base_file)?;
    physical(&source)?;
    let copied = state.join(base_file);
    fs::copy(&source, &copied).map_err(|e| e.to_string())?;
    fs::File::open(&copied)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    write_new(&state.join("snapshots/000000.json"), &initial_bytes)?;
    let mut snapshot = Pin {
        file: "state/snapshots/000000.json".into(),
        sha256: sha(&initial_bytes),
    };
    let mut parent_root = format!("{initial_root:#x}");
    let mut parent_hash = format!("{:#x}", H256::from(recovery::CHECKPOINT_HASH));
    let mut workers = Vec::new();
    for (index, witness) in baseline.blocks.iter().enumerate() {
        need(
            witness.number == recovery::CHECKPOINT_NUMBER + index as u32 + 1
                && witness.independent_import_root_matches
                && witness.disk_restart_root_matches,
            "fresh baseline block witness incomplete",
        )?;
        let bytes = bounded(
            &output.join(format!("baseline/{}.block.scale", witness.number)),
            MAX_BLOCK,
        )?;
        need(
            sha(&bytes) == witness.block_sha256,
            "baseline block bytes changed",
        )?;
        let path = output.join(format!("inputs/{}.block.scale", witness.number));
        write_new(&path, &bytes)?;
        let block_pin = Pin::from_bytes(output, &path, &bytes)?;
        input_pins.push(block_pin.clone());
        let job = Job {
            schema: WORKER_SCHEMA.into(),
            index: index as u32,
            parent_snapshot: snapshot,
            parent_root,
            parent_hash,
            parent_number: witness.number - 1,
            block: Some(block_pin),
            engine: Some(
                engines
                    .get(&witness.executing_code_blake2)
                    .ok_or("baseline executing engine not pinned")?
                    .clone(),
            ),
            bootstrap_blake2: bootstrap_blake2.clone(),
            deployed_blake2: deployed_blake2.clone(),
            expected_root: witness.state_root.clone(),
            expected_hash: witness.hash.clone(),
        };
        let (next, row) = stage(output, &executable, &executable_sha, &job)?;
        snapshot = next;
        parent_root = witness.state_root.clone();
        parent_hash = witness.hash.clone();
        workers.push(row);
    }
    let job = Job {
        schema: WORKER_SCHEMA.into(),
        index: baseline.blocks.len() as u32,
        parent_snapshot: snapshot,
        parent_root: parent_root.clone(),
        parent_hash: parent_hash.clone(),
        parent_number: recovery::CHECKPOINT_NUMBER + baseline.blocks.len() as u32,
        block: None,
        engine: None,
        bootstrap_blake2,
        deployed_blake2,
        expected_root: parent_root,
        expected_hash: parent_hash,
    };
    let (_, terminal) = stage(output, &executable, &executable_sha, &job)?;
    workers.push(terminal);
    let relative = job
        .parent_snapshot
        .file
        .strip_prefix("state/")
        .ok_or("terminal snapshot prefix")?;
    let mut final_ext = snapshot::restore_pinned(
        &state,
        relative,
        &job.parent_snapshot.sha256,
        hash(&job.expected_root)?,
    )?;
    let final_status = super::status(&mut final_ext)?;
    let active = super::value(&mut final_ext, recovery::ACTIVE_POLICY_STORAGE_KEY)
        .unwrap_or_else(|| final_status.policy.to_vec());
    need(active.len() == 32, "terminal active policy encoding")?;
    let final_witness = baseline
        .blocks
        .last()
        .ok_or("baseline final witness absent")?;
    need(
        format!("{:?}", final_status.phase) == final_witness.phase
            && final_status.next_chunk == final_witness.cursor
            && final_status.operation_counter == final_witness.operation_counter
            && hex::encode(final_status.policy) == baseline.policy_commitment,
        "restored final recovery identity differs from baseline",
    )?;
    let binding = incoming.coverage_binding();
    // Comparing diagnostic views of the two same-process values does not
    // deserialize or admit a caller-supplied certificate.
    let binding_view = serde_json::to_value(&binding).map_err(|e| e.to_string())?;
    need(
        binding_view["checkpointStateRoot"]
            == format!("{:#x}", H256::from(recovery::CHECKPOINT_STATE_ROOT))
            && binding_view["captureManifestSha256"] == baseline.capture_manifest_sha256
            && binding_view["replayManifestSha256"] == baseline.manifest_sha256
            && binding_view["replayTranscriptSha256"] == baseline.replay_transcript_sha256,
        "same-process incoming binding differs from baseline identity",
    )?;
    if !binding_view["finalPolicyCommitment"].is_null() {
        need(
            binding_view["finalPolicyCommitment"] == hex::encode(&active)
                && binding_view["finalStateRoot"] == final_witness.state_root,
            "incoming binding differs from restored final policy/root",
        )?;
    }
    if let Some(expected) = &baseline.incoming_replay_binding {
        need(
            binding_view == serde_json::to_value(expected).map_err(|e| e.to_string())?,
            "baseline incoming binding changed",
        )?;
    }
    let final_identity = json!({"blockHash":final_witness.hash,"stateRoot":final_witness.state_root,
        "activePolicyCommitment":hex::encode(&active),"originalPolicyCommitment":baseline.policy_commitment,
        "phase":final_witness.phase,"cursor":final_witness.cursor,"operationCounter":final_witness.operation_counter,
        "runtimeCodeBlake2":hex::encode(sp_io::hashing::blake2_256(&replay::code(&mut final_ext)?))});
    drop(final_ext);
    need(
        file_sha(&source_executable, 2 * 1024 * 1024 * 1024)? == executable_sha
            && file_sha(&executable, 2 * 1024 * 1024 * 1024)? == executable_sha
            && sha(&bounded(manifest_path, 64 * 1024 * 1024)?) == sha(&manifest_bytes),
        "source executable/manifest changed",
    )?;
    for pin in &input_pins {
        pin.verify(output, MAX_BLOCK as u64)?;
    }
    for row in &workers {
        for field in [
            "launchReceipt",
            "processReceipt",
            "readyReceipt",
            "startedReceipt",
            "jobReceipt",
            "stdoutReceipt",
            "stderrReceipt",
        ] {
            let pin: Pin = serde_json::from_value(row[field].clone()).map_err(|e| e.to_string())?;
            pin.verify(output, MAX_LOG)?;
        }
    }
    let report = Report { schema:"sora-nexus-maintenance-process-death-rehearsal-v1", baseline,
        incoming_replay_binding:binding, executable_sha256:executable_sha, final_identity,
        workers, coverage:"SIGKILL after committed, fsynced snapshot; next process continues the killed worker snapshot chain",
        all_blocks_imported_from_continued_snapshots:true, terminal_snapshot_restored_in_fresh_process:true,
        in_commit_crash_verified:false, consensus_seals_verified:false, production_ready:false };
    write_json(&output.join("report.json"), &report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> Job {
        Job {
            schema: WORKER_SCHEMA.into(),
            index: 3,
            parent_snapshot: Pin {
                file: "state/snapshots/000003.json".into(),
                sha256: "aa".repeat(32),
            },
            parent_root: format!("{:#x}", H256::repeat_byte(1)),
            parent_hash: format!("{:#x}", H256::repeat_byte(2)),
            parent_number: recovery::CHECKPOINT_NUMBER + 3,
            block: Some(Pin {
                file: "inputs/block.scale".into(),
                sha256: "bb".repeat(32),
            }),
            engine: Some(Pin {
                file: "inputs/engine.wasm".into(),
                sha256: "cc".repeat(32),
            }),
            bootstrap_blake2: "dd".repeat(32),
            deployed_blake2: "ee".repeat(32),
            expected_root: format!("{:#x}", H256::repeat_byte(3)),
            expected_hash: format!("{:#x}", H256::repeat_byte(4)),
        }
    }
    fn barrier(job: &Job) -> Value {
        json!({"schema":WORKER_SCHEMA,"pid":123,"jobSha256":"unit-test-job-only",
            "parentSnapshot":job.parent_snapshot,
            "snapshot":{"file":"state/snapshots/000004.json","sha256":"ff".repeat(32)},
            "root":job.expected_root,"blockHash":job.expected_hash,"imported":true,
            "durableSnapshotBarrier":true,"standaloneEvidenceAdmissible":false})
    }
    #[test]
    fn restart_job_rejects_checkpoint_reset_gap_and_unpaired_code() {
        let mut value = job();
        value.validate().unwrap();
        value.parent_snapshot.file = "state/snapshots/000000.json".into();
        assert!(value.validate().is_err());
        value = job();
        value.parent_number -= 1;
        assert!(value.validate().is_err());
        value = job();
        value.engine = None;
        assert!(value.validate().is_err());
    }
    #[test]
    fn barrier_cannot_change_process_parent_root_or_continue_from_another_snapshot() {
        let job = job();
        let valid = barrier(&job);
        ready_matches(&valid, &job, "unit-test-job-only", 123).unwrap();
        for (field, changed) in [
            ("pid", json!(124)),
            ("root", json!(job.parent_root)),
            ("jobSha256", json!("different job")),
            ("durableSnapshotBarrier", json!(false)),
            ("standaloneEvidenceAdmissible", json!(true)),
            (
                "parentSnapshot",
                json!({"file":"state/snapshots/000002.json","sha256":"aa".repeat(32)}),
            ),
            (
                "snapshot",
                json!({"file":"state/snapshots/000000.json","sha256":"ff".repeat(32)}),
            ),
        ] {
            let mut bad = valid.clone();
            bad[field] = changed;
            assert!(
                ready_matches(&bad, &job, "unit-test-job-only", 123).is_err(),
                "{field}"
            );
        }
    }
    #[test]
    fn terminal_restore_cannot_replace_the_killed_snapshot() {
        let mut job = job();
        job.block = None;
        job.engine = None;
        job.expected_hash = job.parent_hash.clone();
        job.expected_root = job.parent_root.clone();
        job.validate().unwrap();
        let mut value = barrier(&job);
        value["imported"] = false.into();
        value["snapshot"] = serde_json::to_value(&job.parent_snapshot).unwrap();
        ready_matches(&value, &job, "unit-test-job-only", 123).unwrap();
        value["snapshot"]["sha256"] = "11".repeat(32).into();
        assert!(ready_matches(&value, &job, "unit-test-job-only", 123).is_err());
    }
    #[test]
    fn jobs_reject_caller_injected_trust_fields() {
        let mut value = serde_json::to_value(job()).unwrap();
        value["incomingCertificate"] = json!({"chainLocalReplayVerified":true});
        assert!(serde_json::from_value::<Job>(value).is_err());
    }
    #[test]
    fn expected_receipt_and_observed_input_pins_reject_late_mutation() {
        let directory =
            std::env::temp_dir().join(format!("sora-nexus-crash-pin-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        let path = directory.join("receipt.json");
        let generated =
            write_json_pin(&directory, &path, &json!({"pid":123,"state":"synthetic"})).unwrap();
        generated.verify(&directory, MAX_JOB as u64).unwrap();
        let observed_bytes = bounded(&path, MAX_JOB).unwrap();
        let observed = Pin::from_bytes(&directory, &path, &observed_bytes).unwrap();
        assert_eq!(generated, observed);
        fs::write(&path, br#"{"pid":999,"state":"mutated after validation"}"#).unwrap();
        assert!(generated.verify(&directory, MAX_JOB as u64).is_err());
        assert!(observed.verify(&directory, MAX_JOB as u64).is_err());
        assert_eq!(observed.sha256, sha(&observed_bytes));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn output_paths_cannot_escape_or_follow_aliases() {
        use std::os::unix::fs::symlink;
        let directory =
            std::env::temp_dir().join(format!("sora-nexus-crash-path-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        write_new(&directory.join("value"), b"synthetic").unwrap();
        symlink(directory.join("value"), directory.join("alias")).unwrap();
        assert!(owned(&directory, "value").is_ok());
        assert!(owned(&directory, "alias").is_err());
        assert!(owned(&directory, "../value").is_err());
        assert!(owned(&directory, "/value").is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
