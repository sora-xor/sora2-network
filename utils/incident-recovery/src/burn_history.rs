//! Independently authenticate historical Burn inputs before the shared indexer
//! extractor decodes them. Empty event storage is accepted only with an actual
//! absence proof. Executing metadata always comes from the preceding block's
//! authenticated code, including at a runtime-upgrade boundary. Known active
//! client substitutions must match those bytes or execution evidence is required.
use crate::{hex_bytes, local_path, Result};
use codec::DecodeAll;
use common::recovery::{CHECKPOINT_HASH, CHECKPOINT_NUMBER, CHECKPOINT_STATE_ROOT};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sp_core::{
    storage::{StateVersion, Storage},
    traits::{CallContext, CodeExecutor, RuntimeCode, WrappedRuntimeCode},
    Blake2Hasher, H256,
};
use sp_runtime::{
    generic::Header,
    traits::{BlakeTwo256, Header as _},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufRead, Read, Write},
    path::Path,
    sync::Arc,
};

pub const FIRST_BURN_BLOCK: u32 = 25_043_003;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_NODE_BYTES: usize = 16 * 1024 * 1024;
const PAGE_SIZE: usize = 16;
type BlockHeader = Header<u32, BlakeTwo256>;
type Executor = sc_executor::WasmExecutor<sp_io::SubstrateHostFunctions>;
type ProvenValues = BTreeMap<Vec<u8>, Option<Vec<u8>>>;
type ProofLayout = sp_trie::LayoutV0<Blake2Hasher>;

#[derive(Clone)]
struct CachedNode {
    hash: H256,
    bytes: Arc<Vec<u8>>,
}

/// Only the last successfully proved :code path survives a block. In particular,
/// event nodes and arbitrary unused proof nodes never accumulate across history.
#[derive(Default)]
struct CodeProofCache {
    nodes: BTreeMap<String, CachedNode>,
}

struct CachedProof {
    values: ProvenValues,
    objects: Vec<String>,
}

impl CodeProofCache {
    fn read(
        &mut self,
        directory: &Path,
        state_root: H256,
        objects: &[String],
        with_events: bool,
        extend_cached_path: bool,
    ) -> Result<Option<CachedProof>> {
        if objects.is_empty() || objects.len() > 4096 {
            return Err("invalid incremental proof node count".into());
        }
        let mut nodes = if extend_cached_path {
            self.nodes.clone()
        } else {
            BTreeMap::new()
        };
        let mut seen = BTreeSet::new();
        let mut supplied_bytes = 0usize;
        for object in objects {
            if object.len() != 64
                || !object
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || !seen.insert(object)
            {
                return Err("invalid or duplicate incremental proof object".into());
            }
            if !nodes.contains_key(object) {
                if let Some(cached) = self.nodes.get(object) {
                    nodes.insert(object.clone(), cached.clone());
                } else {
                    let bytes = bounded_file(
                        &local_path(directory, &format!("objects/{object}"))?,
                        MAX_NODE_BYTES,
                    )?;
                    if sha(&bytes) != *object {
                        return Err("incremental proof object checksum mismatch".into());
                    }
                    nodes.insert(
                        object.clone(),
                        CachedNode {
                            hash: H256::from(sp_io::hashing::blake2_256(&bytes)),
                            bytes: Arc::new(bytes),
                        },
                    );
                }
            }
            supplied_bytes += nodes[object].bytes.len();
            if supplied_bytes > MAX_BYTES / 2 {
                return Err("incremental proof exceeds byte bound".into());
            }
        }
        // Each cached node was hashed from its exact bytes when first loaded.
        // Reusing that content-addressed node is safe only because both reads
        // below must traverse the CURRENT header's root. No digest or runtime
        // version is used to infer that code stayed unchanged.
        let mut db = sp_trie::MemoryDB::<Blake2Hasher>::default();
        let mut identities = BTreeMap::new();
        for (object, node) in &nodes {
            if identities.insert(node.hash, object).is_some() {
                return Err("conflicting incremental proof node identities".into());
            }
            sp_trie::HashDBT::emplace(
                &mut db,
                node.hash,
                sp_trie::EMPTY_PREFIX,
                (*node.bytes).clone(),
            );
        }
        let mut code_recorder = sp_trie::Recorder::<ProofLayout>::new();
        let mut event_recorder = sp_trie::Recorder::<ProofLayout>::new();
        let mut values = BTreeMap::new();
        let mut read = |key: &[u8], recorder: &mut sp_trie::Recorder<ProofLayout>| {
            match sp_trie::read_trie_value::<ProofLayout, _>(
                &db,
                &state_root,
                key,
                Some(recorder),
                None,
            ) {
                Ok(value) => {
                    values.insert(key.to_vec(), value);
                    Ok(true)
                }
                Err(error) => match *error {
                    sp_trie::TrieError::<ProofLayout>::InvalidStateRoot(_)
                    | sp_trie::TrieError::<ProofLayout>::IncompleteDatabase(_) => Ok(false),
                    other => Err(format!("invalid incremental storage proof: {other}")),
                },
            }
        };
        if !read(b":code", &mut code_recorder)?
            || (with_events && !read(&events_key(), &mut event_recorder)?)
        {
            return Ok(None);
        }
        code_value(&values)?;
        let code_records = code_recorder.drain();
        let all_records = code_records
            .iter()
            .chain(event_recorder.drain().iter())
            .map(|record| {
                identities
                    .get(&record.hash)
                    .map(|object| (*object).clone())
                    .ok_or_else(|| {
                        "recorded proof node is absent from authenticated objects".to_string()
                    })
            })
            .collect::<Result<BTreeSet<_>>>()?;
        let complete_bytes: usize = all_records
            .iter()
            .map(|object| nodes[object].bytes.len())
            .sum();
        if all_records.len() > 4096 || complete_bytes > MAX_BYTES / 2 {
            return Err("reconstructed incremental proof exceeds raw stream bounds".into());
        }
        let next = code_records
            .iter()
            .map(|record| {
                let object = identities[&record.hash];
                (object.clone(), nodes[object].clone())
            })
            .collect();
        self.nodes = next;
        Ok(Some(CachedProof {
            values,
            objects: all_records.into_iter().collect(),
        }))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofRequest {
    id: u64,
    initial: bool,
    hash: H256,
    header: BlockHeader,
    proof: Vec<String>,
}

/// Bounded JSON-lines helper for capture, never a final history certificate.
/// Missing paths request one full-proof retry without advancing cached state.
pub fn proof_session(directory: &Path) -> Result<()> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let mut cache = CodeProofCache::default();
    let mut previous: Option<BlockHeader> = None;
    loop {
        let mut line = Vec::new();
        input
            .by_ref()
            .take(1024 * 1024 + 1)
            .read_until(b'\n', &mut line)
            .map_err(|error| error.to_string())?;
        if line.is_empty() {
            return Ok(());
        }
        if line.len() > 1024 * 1024 || line.last() != Some(&b'\n') {
            return Err("oversized or truncated incremental proof request".into());
        }
        let request: ProofRequest =
            serde_json::from_slice(&line).map_err(|error| error.to_string())?;
        if request.header.hash() != request.hash
            || *request.header.number() < FIRST_BURN_BLOCK - 1
            || *request.header.number() > CHECKPOINT_NUMBER
            || request.initial != previous.is_none()
            || previous.as_ref().is_some_and(|parent| {
                parent.number().checked_add(1) != Some(*request.header.number())
                    || request.header.parent_hash() != &parent.hash()
            })
        {
            return Err("incremental proof header identity or ancestry mismatch".into());
        }
        let result = cache.read(
            directory,
            *request.header.state_root(),
            &request.proof,
            !request.initial,
            true,
        )?;
        let response = if let Some(verified) = result {
            previous = Some(request.header);
            serde_json::json!({"id":request.id,"verified":true,"proof":verified.objects,
                "cachedCodeNodes":cache.nodes.len(),"cachedCodeBytes":cache.nodes.values().map(|node| node.bytes.len()).sum::<usize>()})
        } else {
            serde_json::json!({"id":request.id,"verified":false,"needFullProof":true})
        };
        serde_json::to_writer(&mut output, &response).map_err(|error| error.to_string())?;
        output
            .write_all(b"\n")
            .and_then(|()| output.flush())
            .map_err(|error| error.to_string())?;
    }
}

fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn events_key() -> Vec<u8> {
    [
        sp_io::hashing::twox_128(b"System"),
        sp_io::hashing::twox_128(b"Events"),
    ]
    .concat()
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub file: String,
    pub sha256: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema: String,
    first: u32,
    last: u32,
    checkpoint_hash: String,
    initial: Artifact,
    pages: Vec<Artifact>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Initial {
    hash: H256,
    header: BlockHeader,
    proof: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    hash: H256,
    block: serde_json::Value,
    proof: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    schema: String,
    first: u32,
    records: Vec<Record>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetadataWitness {
    code_hash: String,
    metadata: Artifact,
    spec_version: u32,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifiedRecord {
    hash: String,
    block: serde_json::Value,
    events: String,
    executing_code_hash: String,
    metadata_sha256: String,
}

fn bounded_file(path: &Path, max: usize) -> Result<Vec<u8>> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > max as u64 {
        return Err("historical proof artifact exceeds bound".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("historical proof artifact grew beyond bound".into());
    }
    Ok(bytes)
}
fn read_artifact(root: &Path, artifact: &Artifact) -> Result<Vec<u8>> {
    let bytes = bounded_file(&local_path(root, &artifact.file)?, MAX_BYTES)?;
    if sha(&bytes) != artifact.sha256 {
        return Err("historical proof artifact checksum differs".into());
    }
    Ok(bytes)
}
#[cfg(test)]
fn proof_values(
    root: &Path,
    state_root: H256,
    objects: &[String],
    keys: &[Vec<u8>],
) -> Result<ProvenValues> {
    if objects.is_empty() || objects.len() > 4096 {
        return Err("invalid historical proof node count".into());
    }
    let mut seen = BTreeSet::new();
    let mut total = 0usize;
    let mut nodes = Vec::new();
    for object in objects {
        if object.len() != 64
            || !object
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || !seen.insert(object)
        {
            return Err("invalid or duplicate historical proof object".into());
        }
        let bytes = bounded_file(
            &local_path(root, &format!("objects/{object}"))?,
            MAX_NODE_BYTES,
        )?;
        total = total
            .checked_add(bytes.len())
            .ok_or("proof byte count overflow")?;
        if total > MAX_BYTES / 2 || sha(&bytes) != *object {
            return Err("proof object size or digest mismatch".into());
        }
        nodes.push(bytes);
    }
    sp_state_machine::read_proof_check::<Blake2Hasher, _>(
        state_root,
        sp_trie::StorageProof::new(nodes),
        keys,
    )
    .map(|values| values.into_iter().collect())
    .map_err(|e| format!("historical storage proof failed: {e}"))
}
fn code_value(values: &ProvenValues) -> Result<Vec<u8>> {
    values
        .get(b":code".as_slice())
        .and_then(Option::as_ref)
        .cloned()
        .filter(|code| !code.is_empty())
        .ok_or_else(|| "historical runtime code is absent or unproved".into())
}
// Exact retained mainnet client substitutes. The activation key is the parent
// height from which the replacement executes children of that matching spec.
// A storage proof authenticates :code, not a client's codeSubstitutes choice.
const RETAINED_SUBSTITUTES: &[(u32, u32, [u8; 32])] = &[
    (
        11_271_782,
        57,
        [
            0xe1, 0x8b, 0xc6, 0xbc, 0x95, 0xe7, 0x31, 0x8c, 0xa5, 0xe2, 0x2b, 0x3b, 0xdc, 0x71,
            0xb0, 0xd1, 0x09, 0x5c, 0x0d, 0x21, 0x5f, 0x46, 0x7c, 0x16, 0xeb, 0x7e, 0x41, 0x97,
            0x2c, 0x4c, 0xf3, 0xc1,
        ],
    ),
    (
        23_234_812,
        115,
        [
            0x31, 0x1d, 0x77, 0xb6, 0x1f, 0xaf, 0x69, 0x50, 0x68, 0x0f, 0x52, 0x03, 0x33, 0xe2,
            0xc8, 0xaf, 0x5a, 0xd3, 0x15, 0x5f, 0x0d, 0x3e, 0x60, 0xec, 0x94, 0x39, 0xfc, 0xf2,
            0xbb, 0xce, 0xef, 0x3e,
        ],
    ),
];

fn require_stored_execution_identity(parent: u32, spec: u32, stored_hash: [u8; 32]) -> Result<()> {
    if let Some((activation, _, executing_hash)) = RETAINED_SUBSTITUTES
        .iter()
        .find(|(activation, version, _)| parent >= *activation && spec == *version)
    {
        if &stored_hash != executing_hash {
            return Err(format!("historical spec {spec} at parent {parent} requires the retained client substitute activated at {activation} (executing code {}), but the authenticated stored :code differs; supply separately authenticated executing-artifact and substitution evidence before deriving Burn metadata", hx(executing_hash)));
        }
    }
    Ok(())
}

fn metadata(executor: &Executor, code: &[u8], parent: u32) -> Result<(u32, Vec<u8>)> {
    let fetcher = WrappedRuntimeCode(std::borrow::Cow::Borrowed(code));
    let runtime = RuntimeCode {
        code_fetcher: &fetcher,
        heap_pages: None,
        hash: sp_io::hashing::blake2_256(code).to_vec(),
    };
    let mut ext = sp_io::TestExternalities::new_with_code_and_state(
        code,
        Storage::default(),
        StateVersion::V0,
    );
    let mut call = |method| -> Result<Vec<u8>> {
        let (result, native) =
            executor.call(&mut ext.ext(), &runtime, method, &[], CallContext::Offchain);
        if native {
            return Err("historical metadata must execute Wasm".into());
        }
        result.map_err(|e| format!("historical metadata execution failed: {e}"))
    };
    let version = sp_version::RuntimeVersion::decode_all(&mut call("Core_version")?.as_slice())
        .map_err(|e| e.to_string())?;
    if version.spec_name != "sora-substrate" || version.state_version() != StateVersion::V0 {
        return Err("unreviewed historical runtime identity/state version".into());
    }
    require_stored_execution_identity(
        parent,
        version.spec_version,
        sp_io::hashing::blake2_256(code),
    )?;
    let metadata = Vec::<u8>::decode_all(&mut call("Metadata_metadata")?.as_slice())
        .map_err(|e| e.to_string())?;
    crate::inventory::MetadataSchema::decode(&metadata)?;
    Ok((version.spec_version, metadata))
}
fn verify_record(record: &Record, previous: &BlockHeader) -> Result<BlockHeader> {
    let header: BlockHeader = serde_json::from_value(record.block["block"]["header"].clone())
        .map_err(|e| e.to_string())?;
    if previous.number().checked_add(1) != Some(*header.number())
        || header.parent_hash() != &previous.hash()
        || header.hash() != record.hash
    {
        return Err("historical burn stream has a header gap, fork or hash mismatch".into());
    }
    let extrinsics = record.block["block"]["extrinsics"]
        .as_array()
        .ok_or("missing historical extrinsics")?
        .iter()
        .map(|x| hex_bytes(x.as_str().ok_or("invalid historical extrinsic")?))
        .collect::<Result<Vec<_>>>()?;
    if sp_io::trie::blake2_256_ordered_root(extrinsics, StateVersion::V0)
        != *header.extrinsics_root()
    {
        return Err("historical burn extrinsics root mismatch".into());
    }
    Ok(header)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

/// Verify a complete-to-checkpoint proof stream and emit only authenticated
/// bytes into a NEW directory. A shorter initial range is diagnostic only.
/// A partial output has no verification-complete.json and is never consumable.
pub fn verify(manifest_path: &Path, output: &Path) -> Result<serde_json::Value> {
    let manifest_bytes = bounded_file(manifest_path, MAX_BYTES)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
    if manifest.schema != "sora-nexus-burn-proof-stream-v1"
        || manifest.first < FIRST_BURN_BLOCK
        || manifest.first > CHECKPOINT_NUMBER
        || manifest.last != CHECKPOINT_NUMBER
        || manifest.checkpoint_hash != hx(&CHECKPOINT_HASH)
        || manifest.pages.len()
            != ((CHECKPOINT_NUMBER - manifest.first + 1) as usize).div_ceil(PAGE_SIZE)
    {
        return Err("invalid or incomplete historical burn proof range".into());
    }
    let root = manifest_path.parent().ok_or("manifest has no directory")?;
    let initial: Initial = serde_json::from_slice(&read_artifact(root, &manifest.initial)?)
        .map_err(|e| e.to_string())?;
    if initial.header.number().checked_add(1) != Some(manifest.first)
        || initial.header.hash() != initial.hash
    {
        return Err("historical burn initial parent mismatch".into());
    }
    let mut proof_cache = CodeProofCache::default();
    let mut code = code_value(
        &proof_cache
            .read(
                root,
                *initial.header.state_root(),
                &initial.proof,
                false,
                false,
            )?
            .ok_or("initial historical code proof is incomplete")?
            .values,
    )?;
    let mut previous = initial.header;
    let mut code_digest = sp_io::hashing::blake2_256(&code);
    let mut code_hash = hx(&code_digest);
    let executor = Executor::builder().build();
    // Do not overwrite or reuse a prior verification output, even after failure.
    fs::create_dir(output).map_err(|e| e.to_string())?;
    fs::create_dir(output.join("metadata")).map_err(|e| e.to_string())?;
    fs::create_dir(output.join("pages")).map_err(|e| e.to_string())?;
    let mut metadata_cache: BTreeMap<String, MetadataWitness> = BTreeMap::new();
    let mut verified_pages = Vec::new();
    let mut block_count = 0u32;
    let mut absent_events = 0u32;
    for page_ref in &manifest.pages {
        let encoded = read_artifact(root, page_ref)?;
        let mut bytes = Vec::new();
        GzDecoder::new(encoded.as_slice())
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("historical proof page expands beyond bound".into());
        }
        let page: Page = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let expected_first = previous.number().checked_add(1).ok_or("height overflow")?;
        if page.schema != "sora-nexus-burn-proof-page-v1"
            || page.first != expected_first
            || page.records.len()
                != (CHECKPOINT_NUMBER - page.first + 1).min(PAGE_SIZE as u32) as usize
        {
            return Err("historical proof page has missing or duplicated block coverage".into());
        }
        let mut verified_records = Vec::new();
        for record in page.records {
            let header = verify_record(&record, &previous)?;
            if let std::collections::btree_map::Entry::Vacant(entry) =
                metadata_cache.entry(code_hash.clone())
            {
                let (spec_version, bytes) = metadata(&executor, &code, *previous.number())?;
                let digest = sha(&bytes);
                let file = format!("metadata/{digest}.scale");
                if !output.join(&file).exists() {
                    write_new(&output.join(&file), &bytes)?;
                }
                entry.insert(MetadataWitness {
                    code_hash: code_hash.clone(),
                    metadata: Artifact {
                        file,
                        sha256: digest,
                    },
                    spec_version,
                });
            }
            let witness = metadata_cache
                .get(&code_hash)
                .ok_or("missing executed metadata")?;
            require_stored_execution_identity(
                *previous.number(),
                witness.spec_version,
                code_digest,
            )?;
            let key = events_key();
            let values = proof_cache
                .read(root, *header.state_root(), &record.proof, true, false)?
                .ok_or("historical storage proof is incomplete")?
                .values;
            let events = values.get(&key).ok_or("events key was not checked")?;
            // Storage's ValueQuery default is the empty SCALE Vec, and only an
            // authenticated absence may select that default.
            if events.is_none() {
                absent_events += 1;
            }
            verified_records.push(VerifiedRecord {
                hash: hx(record.hash.as_bytes()),
                block: record.block,
                events: events.as_deref().map(hx).unwrap_or_else(|| "0x00".into()),
                executing_code_hash: code_hash.clone(),
                metadata_sha256: witness.metadata.sha256.clone(),
            });
            let next_code = code_value(&values)?;
            // The next bytes were just proved against this block's root. Only
            // exact byte equality permits reuse of the previously computed hash;
            // spec versions and update digests are never an identity shortcut.
            if next_code != code {
                code_digest = sp_io::hashing::blake2_256(&next_code);
                code_hash = hx(&code_digest);
                code = next_code;
            }
            previous = header;
            block_count += 1;
        }
        let file = format!("pages/{}.json.gz", page.first);
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        serde_json::to_writer(&mut encoder, &serde_json::json!({"schema":"sora-nexus-verified-burn-page-v1", "first":page.first, "records":verified_records})).map_err(|e| e.to_string())?;
        let encoded = encoder.finish().map_err(|e| e.to_string())?;
        write_new(&output.join(&file), &encoded)?;
        verified_pages.push(Artifact {
            file,
            sha256: sha(&encoded),
        });
    }
    if *previous.number() != CHECKPOINT_NUMBER
        || previous.hash() != H256::from(CHECKPOINT_HASH)
        || *previous.state_root() != H256::from(CHECKPOINT_STATE_ROOT)
    {
        return Err("historical burn stream does not descend into the pinned checkpoint".into());
    }
    let report = serde_json::json!({"schema":"sora-nexus-verified-burn-stream-v1", "first":manifest.first,
        "last":CHECKPOINT_NUMBER, "checkpointHash":hx(&CHECKPOINT_HASH), "checkpointStateRoot":hx(&CHECKPOINT_STATE_ROOT),
        "sourceManifestSha256":sha(&manifest_bytes), "blockCount":block_count, "provedAbsentEventStorage":absent_events,
        "headersLinkedToCheckpoint":true, "extrinsicsRootsVerified":true, "storageProofsVerified":true,
        "metadataExecutedFromAuthenticatedParentCode":true, "nativeExecution":false,
        "coverageComplete":manifest.first == FIRST_BURN_BLOCK, "pages":verified_pages, "metadata":metadata_cache.into_values().collect::<Vec<_>>(),
        "productionReady":false, "consensusFinalityVerified":false});
    write_new(
        &output.join("verification-complete.json"),
        &serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_runtime::generic::Digest;

    fn save_proof(directory: &Path, ext: &sp_io::TestExternalities, keys: &[&[u8]]) -> Vec<String> {
        sp_state_machine::prove_read::<_, Blake2Hasher, _>(ext.backend.clone(), keys)
            .unwrap()
            .into_iter_nodes()
            .map(|node| {
                let object = sha(&node);
                let path = directory.join("objects").join(&object);
                if !path.exists() {
                    write_new(&path, &node).unwrap();
                }
                object
            })
            .collect()
    }

    #[test]
    fn retained_substitute_identity_uses_activation_parent_spec_and_exact_code() {
        for (activation, spec, expected) in RETAINED_SUBSTITUTES {
            assert!(require_stored_execution_identity(activation - 1, *spec, [0; 32]).is_ok());
            assert!(
                require_stored_execution_identity(*activation, *spec, [0; 32])
                    .unwrap_err()
                    .contains("executing-artifact and substitution evidence")
            );
            assert!(require_stored_execution_identity(*activation, *spec, *expected).is_ok());
            assert!(
                require_stored_execution_identity(FIRST_BURN_BLOCK - 1, *spec, [0; 32]).is_err()
            );
            assert!(
                require_stored_execution_identity(CHECKPOINT_NUMBER - 1, *spec, *expected).is_ok()
            );
            assert!(require_stored_execution_identity(*activation, spec + 1, [0; 32]).is_ok());
        }
        // Recovery spec134 substitution starts at the checkpoint parent, after
        // this verifier's final executing parent (checkpoint minus one).
        assert!(require_stored_execution_identity(CHECKPOINT_NUMBER - 1, 134, [0; 32]).is_ok());
    }

    #[test]
    fn cached_code_nodes_are_reauthenticated_at_each_root_and_old_nodes_are_pruned() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "sora-nexus-code-cache-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        fs::create_dir(directory.join("objects")).unwrap();
        let key = events_key();
        let mut storage = Storage::default();
        storage.top.insert(key.clone(), vec![1; 64]);
        let mut ext = sp_io::TestExternalities::new_with_code_and_state(
            &[42; 5000],
            storage,
            StateVersion::V0,
        );
        let initial = save_proof(&directory, &ext, &[b":code"]);
        let mut cache = CodeProofCache::default();
        assert!(cache
            .read(&directory, *ext.backend.root(), &initial, false, false)
            .unwrap()
            .is_some());
        let old_code_node = cache
            .nodes
            .iter()
            .find(|(_, node)| node.bytes.len() > 5000)
            .unwrap()
            .0
            .clone();
        ext.execute_with(|| sp_io::storage::set(&key, &[2; 64]));
        ext.commit_all().unwrap();
        let events_only = save_proof(&directory, &ext, &[&key]);
        assert!(!events_only.contains(&old_code_node));
        // The final raw-stream verifier never repairs an omitted explicit reference.
        assert!(cache
            .read(&directory, *ext.backend.root(), &events_only, true, false)
            .unwrap()
            .is_none());
        let proved = cache
            .read(&directory, *ext.backend.root(), &events_only, true, true)
            .unwrap()
            .unwrap();
        assert_eq!(code_value(&proved.values).unwrap(), vec![42; 5000]);
        assert!(proved.objects.contains(&old_code_node));
        let original_check = proof_values(
            &directory,
            *ext.backend.root(),
            &proved.objects,
            &[b":code".to_vec(), key.clone()],
        )
        .unwrap();
        assert_eq!(original_check, proved.values);
        // An arbitrary raw storage write changes code without any runtime-update
        // digest. The current root still rejects the old cached leaf.
        ext.execute_with(|| {
            sp_io::storage::set(b":code", &[43; 5000]);
            sp_io::storage::set(&key, &[3; 64]);
        });
        ext.commit_all().unwrap();
        let events_only = save_proof(&directory, &ext, &[&key]);
        let before: Vec<_> = cache.nodes.keys().cloned().collect();
        assert!(cache
            .read(&directory, *ext.backend.root(), &events_only, true, true)
            .unwrap()
            .is_none());
        assert_eq!(before, cache.nodes.keys().cloned().collect::<Vec<_>>());
        let full = save_proof(&directory, &ext, &[b":code", &key]);
        let proved = cache
            .read(&directory, *ext.backend.root(), &full, true, true)
            .unwrap()
            .unwrap();
        assert_eq!(code_value(&proved.values).unwrap(), vec![43; 5000]);
        assert!(!cache.nodes.contains_key(&old_code_node));
        assert!(cache.nodes.len() <= 4096);
        assert!(
            cache
                .nodes
                .values()
                .map(|node| node.bytes.len())
                .sum::<usize>()
                <= MAX_BYTES / 2
        );
        ext.execute_with(|| sp_io::storage::clear(&key));
        ext.commit_all().unwrap();
        let absent = save_proof(&directory, &ext, &[&key]);
        let result = cache
            .read(&directory, *ext.backend.root(), &absent, true, true)
            .unwrap()
            .unwrap();
        assert_eq!(result.values.get(&key), Some(&None));
        assert!(cache
            .read(&directory, H256::repeat_byte(9), &absent, true, true)
            .unwrap()
            .is_none());
        let mut duplicate = absent.clone();
        duplicate.push(absent[0].clone());
        assert!(cache
            .read(&directory, *ext.backend.root(), &duplicate, true, true)
            .is_err());
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn event_key_and_parent_chain_are_exact() {
        assert_eq!(
            hx(&events_key()),
            "0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7"
        );
        let parent = BlockHeader::new(
            10,
            H256::zero(),
            H256::zero(),
            H256::zero(),
            Digest::default(),
        );
        let child = BlockHeader::new(
            11,
            sp_io::trie::blake2_256_ordered_root(Vec::<Vec<u8>>::new(), StateVersion::V0),
            H256::zero(),
            parent.hash(),
            Digest::default(),
        );
        let mut record = Record {
            hash: child.hash(),
            block: serde_json::json!({"block":{"header":child, "extrinsics":[]}}),
            proof: vec![],
        };
        assert!(verify_record(&record, &parent).is_ok());
        record.block["block"]["extrinsics"] = serde_json::json!(["0x00"]);
        assert!(verify_record(&record, &parent)
            .unwrap_err()
            .contains("extrinsics root"));
        record.hash = H256::repeat_byte(1);
        assert!(verify_record(&record, &parent)
            .unwrap_err()
            .contains("header gap"));
    }
    #[test]
    fn absent_code_cannot_be_promoted_to_a_runtime() {
        assert!(code_value(&BTreeMap::new()).is_err());
        assert!(code_value(&BTreeMap::from([(b":code".to_vec(), None)])).is_err());
        assert!(code_value(&BTreeMap::from([(b":code".to_vec(), Some(vec![]))])).is_err());
    }
    #[test]
    fn proofs_bind_absence_presence_code_and_state_root() {
        let directory =
            std::env::temp_dir().join(format!("sora-nexus-burn-proof-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        fs::create_dir(directory.join("objects")).unwrap();
        let code_key = b":code".to_vec();
        let event_key = events_key();
        let ext = sp_io::TestExternalities::new_with_code_and_state(
            &[42; 5000],
            Storage::default(),
            StateVersion::V0,
        );
        let proof = sp_state_machine::prove_read::<_, Blake2Hasher, _>(
            ext.backend.clone(),
            [&code_key, &event_key],
        )
        .unwrap();
        let references: Vec<_> = proof
            .into_iter_nodes()
            .map(|node| {
                let digest = sha(&node);
                write_new(&directory.join("objects").join(&digest), &node).unwrap();
                digest
            })
            .collect();
        let values = proof_values(
            &directory,
            *ext.backend.root(),
            &references,
            &[code_key.clone(), event_key.clone()],
        )
        .unwrap();
        assert_eq!(values.get(&event_key), Some(&None));
        assert_eq!(code_value(&values).unwrap(), vec![42; 5000]);
        assert!(proof_values(
            &directory,
            H256::repeat_byte(9),
            &references,
            std::slice::from_ref(&event_key)
        )
        .is_err());
        assert!(proof_values(
            &directory,
            *ext.backend.root(),
            &[],
            std::slice::from_ref(&event_key)
        )
        .is_err());
        let mut duplicated = references.clone();
        duplicated.push(references[0].clone());
        assert!(proof_values(&directory, *ext.backend.root(), &duplicated, &[event_key]).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
