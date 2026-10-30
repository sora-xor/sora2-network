//! Opt-in local public-file request processing for the live read-only adapter.
//! No signer, RPC method, transaction submission or automatic retry is provided.

use super::*;
use serde_json::json;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_REQUESTS: usize = 4096;
const MAX_REQUEST: usize = 64 * 1024;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    file: String,
    bytes: usize,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    schema: String,
    release: Reference,
    bootstrap: Reference,
    metadata: Reference,
    metadata_role: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Action {
    Prepare,
    Admit,
}

impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::Admit => "admit",
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    schema: String,
    /// Public caller-chosen identity for an explicit new attempt, never a retry
    /// counter interpreted as transaction authority.
    attempt_id: H256,
    action: Action,
    parent_hash: H256,
    plan_role: String,
    plan: Reference,
    operation: Reference,
}

fn digest(raw: &[u8]) -> String {
    hex(&sp_io::hashing::sha2_256(raw))
}

fn canonical_directory(path: &Path) -> Result<PathBuf> {
    let absolute = fs::canonicalize(path).map_err(|e| e.to_string())?;
    require(
        path.is_absolute() && absolute == path && path.is_dir(),
        "canonical absolute directory required",
    )?;
    Ok(absolute)
}

fn relative(root: &Path, name: &str) -> Result<PathBuf> {
    require(
        !name.is_empty() && !name.contains('\\') && name.len() <= 4096,
        "invalid relative artifact path",
    )?;
    let mut result = root.to_path_buf();
    for component in Path::new(name).components() {
        let Component::Normal(part) = component else {
            return Err("artifact path escapes request directory".into());
        };
        result.push(part);
        require(
            !fs::symlink_metadata(&result)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink(),
            "artifact path contains a symlink",
        )?;
    }
    require(
        fs::canonicalize(&result).map_err(|e| e.to_string())? == result,
        "artifact path is aliased",
    )?;
    Ok(result)
}

fn read(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    require(
        before.file_type().is_file() && before.len() > 0 && before.len() <= maximum as u64,
        "regular bounded artifact required",
    )?;
    let file = File::open(path).map_err(|e| e.to_string())?;
    let opened = file.metadata().map_err(|e| e.to_string())?;
    require(
        opened.is_file() && opened.len() == before.len(),
        "artifact changed while opening",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        require(
            before.dev() == opened.dev() && before.ino() == opened.ino() && opened.nlink() == 1,
            "artifact was aliased while opening",
        )?;
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let after = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    require(
        after.file_type().is_file()
            && bytes.len() == before.len() as usize
            && after.len() == before.len(),
        "artifact changed while reading",
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        require(
            after.dev() == opened.dev() && after.ino() == opened.ino(),
            "artifact replaced while reading",
        )?;
    }
    Ok(bytes)
}

fn referenced(root: &Path, reference: &Reference, maximum: usize) -> Result<Vec<u8>> {
    require(
        reference.bytes > 0 && reference.bytes <= maximum && valid_id(&reference.sha256),
        "artifact reference bounds differ",
    )?;
    let bytes = read(&relative(root, &reference.file)?, maximum)?;
    require(
        bytes.len() == reference.bytes && digest(&bytes) == reference.sha256,
        "artifact reference hash/size differs",
    )?;
    Ok(bytes)
}

fn valid_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}

fn json_new(path: &Path, value: &serde_json::Value) -> Result<()> {
    write_new(
        path,
        &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
}

struct PublicRelease {
    input_raw: Vec<u8>,
    input: Input,
    manifest: Vec<u8>,
    bootstrap: Vec<u8>,
    metadata: Vec<u8>,
    expected_manifest: [u8; 32],
}

impl PublicRelease {
    fn load(root: &Path, expected_manifest: H256) -> Result<Self> {
        let input_raw = read(&relative(root, "release-input.json")?, MAX_REQUEST)?;
        let input: Input =
            serde_json::from_slice(&input_raw).map_err(|_| "invalid local release descriptor")?;
        require(
            input.schema == "sora-nexus-hashi-local-release-v1",
            "local release descriptor schema differs",
        )?;
        let manifest = referenced(root, &input.release, MAX_JSON)?;
        require(
            expected_manifest != H256::zero()
                && sp_io::hashing::sha2_256(&manifest) == expected_manifest.0,
            "local manifest differs from startup release pin",
        )?;
        let bootstrap = referenced(root, &input.bootstrap, MAX_CODE)?;
        let metadata = referenced(root, &input.metadata, MAX_METADATA)?;
        Ok(Self {
            input_raw,
            input,
            manifest,
            bootstrap,
            metadata,
            expected_manifest: expected_manifest.0,
        })
    }

    fn guard(&self, root: &Path) -> Result<()> {
        require(
            read(&relative(root, "release-input.json")?, MAX_REQUEST)? == self.input_raw,
            "startup release descriptor changed",
        )?;
        for (reference, bytes, maximum) in [
            (&self.input.release, &self.manifest, MAX_JSON),
            (&self.input.bootstrap, &self.bootstrap, MAX_CODE),
            (&self.input.metadata, &self.metadata, MAX_METADATA),
        ] {
            require(
                referenced(root, reference, maximum)? == *bytes,
                "startup release artifact changed",
            )?;
        }
        Ok(())
    }

    fn artifacts<'a>(&'a self, plan: &'a [u8], role: &'a str) -> ReleaseArtifacts<'a> {
        ReleaseArtifacts {
            expected_manifest_sha256: self.expected_manifest,
            manifest: &self.manifest,
            plan,
            plan_role: role,
            bootstrap: &self.bootstrap,
            metadata: &self.metadata,
            metadata_role: &self.input.metadata_role,
        }
    }
}

/// One optional service task. Requests and results are public evidence files;
/// outputs never enter the existing authoring-inherent directory or tx pool.
pub(crate) struct LocalRequestPump {
    root: PathBuf,
    requests: PathBuf,
    results: PathBuf,
    client: AdmittedClient,
    release: PublicRelease,
}

impl LocalRequestPump {
    pub(crate) fn open(
        directory: &Path,
        manifest_sha256: H256,
        client: Arc<crate::service::FullClient>,
        config: &sc_service::Configuration,
    ) -> Result<Self> {
        let root = canonical_directory(directory)?;
        let requests = canonical_directory(&root.join("requests"))?;
        let results = canonical_directory(&root.join("results"))?;
        let release = PublicRelease::load(&root, manifest_sha256)?;
        let client = AdmittedClient::new(client, config)?;
        Ok(Self {
            root,
            requests,
            results,
            client,
            release,
        })
    }

    /// Sequential task on the node's blocking lane. The timer also observes new
    /// requests at an unchanged finalized head; no finality event is required.
    pub(crate) async fn run(self) {
        loop {
            if let Err(error) = self.step() {
                log::error!("SORA Nexus local HASHI witness processing stopped: {error}");
                return;
            }
            futures_timer::Delay::new(Duration::from_secs(2)).await;
        }
    }

    fn step(&self) -> Result<()> {
        require(
            canonical_directory(&self.root)? == self.root
                && canonical_directory(&self.requests)? == self.requests
                && canonical_directory(&self.results)? == self.results,
            "local request directory changed",
        )?;
        let request = next_request(&self.requests, &self.results)?;
        let Some((id, path)) = request else {
            return Ok(());
        };
        // Exclusive attempt directory is also an interruption marker. A restart
        // never retries a partially executed request or overwrites its evidence.
        let output = self.results.join(&id);
        fs::create_dir(&output).map_err(|e| e.to_string())?;
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        json_new(
            &output.join("started.json"),
            &json!({"schema":"sora-nexus-hashi-local-attempt-v1","requestSha256":id,"pid":std::process::id(),"startedUnixMillis":started,"productionReady":false}),
        )?;
        let outcome = self.execute(&id, &path, &output);
        match outcome {
            Ok(receipt) => json_new(&output.join("result.json"), &receipt),
            Err(error) => {
                json_new(
                    &output.join("result.json"),
                    &json!({"schema":"sora-nexus-hashi-local-result-v1","requestSha256":id,"success":false,"error":error,"automaticallyRetried":false,"persistentStateChanged":false,"externalFinalityVerified":false,"productionReady":false}),
                )?;
                Ok(())
            }
        }
    }

    fn execute(&self, id: &str, path: &Path, output: &Path) -> Result<serde_json::Value> {
        self.release.guard(&self.root)?;
        let request_raw = read(path, MAX_REQUEST)?;
        write_new(&output.join("request.json"), &request_raw)?;
        require(
            digest(&request_raw) == id,
            "request file name differs from exact content hash",
        )?;
        let request: Request =
            serde_json::from_slice(&request_raw).map_err(|_| "invalid local HASHI request")?;
        require(
            request.schema == "sora-nexus-hashi-local-request-v1"
                && request.attempt_id != H256::zero(),
            "local HASHI request schema differs",
        )?;
        let plan = referenced(&self.root, &request.plan, MAX_JSON)?;
        let operation = referenced(&self.root, &request.operation, recovery::MAX_CHUNK_BYTES)?;
        let adapter = LiveHashiAdapter::new(
            &self.client,
            self.release.artifacts(&plan, &request.plan_role),
        )?;
        let (bytes, identity) = match request.action {
            Action::Prepare => encoded_witness(adapter.prepare(request.parent_hash, &operation)?),
            Action::Admit => encoded_witness(adapter.admit(request.parent_hash, &operation)?),
        };
        self.release.guard(&self.root)?;
        require(
            read(path, MAX_REQUEST)? == request_raw
                && referenced(&self.root, &request.plan, MAX_JSON)? == plan
                && referenced(&self.root, &request.operation, recovery::MAX_CHUNK_BYTES)?
                    == operation,
            "request inputs changed during native execution",
        )?;
        // Recheck after filesystem guards too; they may be slow on this host.
        require(
            self.client.client.info().finalized_hash == request.parent_hash,
            "finalized head advanced before evidence publication",
        )?;
        write_new(&output.join("response.scale"), &bytes)?;
        Ok(
            json!({"schema":"sora-nexus-hashi-local-result-v1","requestSha256":id,"success":true,"action":request.action.name(),"parentHash":request.parent_hash,"identity":identity,"response":{"file":"response.scale","bytes":bytes.len(),"sha256":digest(&bytes),"blake2_256":hex(&sp_io::hashing::blake2_256(&bytes))},"planSha256":digest(&plan),"operationSha256":digest(&operation),"ownClientFinalizedParentChecked":true,"independentGrandpaProofVerified":false,"automaticallyRetried":false,"persistentStateChanged":false,"externalFinalityVerified":false,"signingPerformed":false,"transactionSubmitted":false,"productionReady":false}),
        )
    }
}

fn encoded_witness<T: Encode>(witness: HostWitness<T>) -> (Vec<u8>, serde_json::Value) {
    (
        witness.native.encode(),
        json!({
            "releaseManifestSha256":hex(&witness.release_manifest_sha256),
            "executingCodeHash":H256::from(witness.executing_code_hash),
            "executingMetadataHash":H256::from(witness.executing_metadata_hash),
            "finalizedParent":witness.finalized_parent,
        }),
    )
}

fn next_request(requests: &Path, results: &Path) -> Result<Option<(String, PathBuf)>> {
    let mut pending = Vec::new();
    let mut count = 0;
    for entry in fs::read_dir(requests).map_err(|e| e.to_string())? {
        count += 1;
        require(count <= MAX_REQUESTS, "local request count exceeds bound")?;
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "non-UTF8 local request name")?;
        // A producer should publish complete content-addressed files by atomic
        // rename from outside this directory, never leave temporary files here.
        let id = name
            .strip_suffix(".json")
            .filter(|id| valid_id(id))
            .ok_or("request name must be SHA256.json")?;
        require(
            entry.file_type().map_err(|e| e.to_string())?.is_file(),
            "request must be a regular file",
        )?;
        if fs::symlink_metadata(results.join(id)).is_ok() {
            continue;
        }
        pending.push((id.to_owned(), entry.path()));
    }
    pending.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(pending.into_iter().next())
}

#[cfg(test)]
#[path = "recovery_hashi_local_tests.rs"]
mod tests;
