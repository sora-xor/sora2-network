//! Offline incident evidence verification. This tool never connects to a node,
//! signs a transaction, changes a live database or supplies missing release data.

use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read},
    path::{Component, Path},
};

use codec::{Decode, DecodeAll, Encode};
use common::recovery::{self, AccountReplacement};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sp_core::{
    crypto::{AccountId32, Ss58Codec},
    storage::{ChildInfo, StateVersion, Storage, StorageChild},
};
use sp_runtime::{
    traits::{BlakeTwo256, Header as _},
    MultiSignature,
};

pub mod bags_list;
#[cfg(feature = "runtime-work-types")]
pub mod bridge_hashi;
#[cfg(feature = "runtime-work-types")]
pub mod bridge_history;
#[cfg(feature = "runtime-work-types")]
pub mod bridge_signing;
pub mod burn_history;
pub mod composer;
pub mod democracy;
pub mod economics;
pub mod elections;
#[cfg(feature = "bridge-fixture")]
pub mod funded_staking_fixture;
pub mod governance;
#[cfg(feature = "runtime-work-types")]
pub mod hashi_association;
#[cfg(feature = "bridge-fixture")]
pub mod hashi_bridge_fixture;
#[cfg(feature = "runtime-work-types")]
pub mod incoming_coverage;
#[cfg(feature = "runtime-work-types")]
pub mod incoming_replay;
pub mod inventory;
pub mod maintenance;
#[cfg(feature = "bridge-fixture")]
pub mod mixed_bridge_fixture;
#[cfg(feature = "runtime-work-types")]
pub mod mixed_work;
pub mod offences;
pub mod referral;
pub mod rehearsal;
pub mod release_checks;
pub mod replay;
pub mod scheduler_controls;
pub mod staking_controls;
pub mod staking_history;
pub mod staking_roles;
pub mod substitution_semantics;
pub mod typed;

pub type Result<T> = std::result::Result<T, String>;

/// Authenticate the portable type registry by executing the captured :code in
/// its independently root-verified state. An RPC metadata file/hash alone does
/// not establish that the schema describes the checkpoint runtime.
pub struct VerifiedCheckpoint {
    pub state: VerificationReport,
    pub storage: Storage,
    pub schema: inventory::MetadataSchema,
    pub metadata_witness: serde_json::Value,
}

/// Verify the full snapshot once and authenticate its type registry by executing
/// that snapshot's own Wasm. Downstream adapters consume the same verified state.
pub fn verify_checkpoint(root: &Path) -> Result<VerifiedCheckpoint> {
    use sp_core::traits::{CallContext, CodeExecutor, RuntimeCode, WrappedRuntimeCode};
    let (state_report, storage) = verify_capture(root)?;
    let code = storage
        .top
        .get(b":code".as_slice())
        .ok_or("missing code")?
        .clone();
    let expected = fs::read(root.join("runtime-metadata.scale")).map_err(|e| e.to_string())?;
    let fetcher = WrappedRuntimeCode(std::borrow::Cow::Borrowed(&code));
    let runtime = RuntimeCode {
        code_fetcher: &fetcher,
        heap_pages: None,
        hash: sp_io::hashing::blake2_256(&code).to_vec(),
    };
    let executor = sc_executor::WasmExecutor::<sp_io::SubstrateHostFunctions>::builder().build();
    let mut ext =
        sp_io::TestExternalities::new_with_code_and_state(&code, storage.clone(), StateVersion::V0);
    let (output, native) = executor.call(
        &mut ext.ext(),
        &runtime,
        "Metadata_metadata",
        &[],
        CallContext::Offchain,
    );
    let output = output.map_err(|e| format!("checkpoint metadata execution failed: {e}"))?;
    let metadata = Vec::<u8>::decode_all(&mut output.as_slice()).map_err(|e| e.to_string())?;
    if native || metadata != expected {
        return Err("executed metadata differs from captured schema".into());
    }
    let schema = inventory::MetadataSchema::decode(&metadata)?;
    let metadata_witness = serde_json::json!({"schema":"sora-nexus-checkpoint-metadata-witness-v1",
        "checkpointHash":state_report.checkpoint_hash,"stateRoot":state_report.state_root,
        "executingCodeBlake2":format!("0x{}",hex::encode(runtime.hash)),
        "metadataSha256":hex::encode(Sha256::digest(&metadata)),
        "wasmExecutionVerified":true,"nativeExecution":false,"productionReady":false});
    Ok(VerifiedCheckpoint {
        state: state_report,
        storage,
        schema,
        metadata_witness,
    })
}

pub fn verify_metadata(root: &Path) -> Result<serde_json::Value> {
    Ok(verify_checkpoint(root)?.metadata_witness)
}

pub fn hex_bytes(value: &str) -> Result<Vec<u8>> {
    let text = value
        .strip_prefix("0x")
        .ok_or("hex data must start with 0x")?;
    if text.len() % 2 != 0
        || text
            .bytes()
            .any(|b| !b.is_ascii_hexdigit() || b.is_ascii_uppercase())
    {
        return Err("noncanonical lowercase hex data".into());
    }
    hex::decode(text).map_err(|e| e.to_string())
}

pub fn account(value: &str) -> Result<AccountId32> {
    if value.starts_with("0x") {
        let bytes: [u8; 32] = hex_bytes(value)?
            .try_into()
            .map_err(|_| "account must be 32 bytes")?;
        Ok(AccountId32::from(bytes))
    } else {
        AccountId32::from_ss58check(value).map_err(|e| format!("invalid SS58 account: {e:?}"))
    }
}

fn csv_field(value: &str) -> Result<&str> {
    let value = value.trim();
    let value = if value.starts_with('"') {
        value
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or("unclosed CSV field")?
    } else {
        value
    };
    if value.contains('"') || value.is_empty() {
        return Err("invalid CSV field".into());
    }
    Ok(value)
}

/// Address fields cannot contain commas/newlines/escaped quotes; reject them
/// rather than accidentally accepting extra columns or a truncated address.
pub fn parse_mapping_csv(text: &str) -> Result<Vec<AccountReplacement>> {
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    let header: Vec<_> = lines
        .next()
        .ok_or("empty mapping CSV")?
        .split(',')
        .map(csv_field)
        .collect::<Result<_>>()?;
    if header != ["old account address", "new account address"] {
        return Err("mapping CSV header must be old account address,new account address".into());
    }
    let mut map = Vec::new();
    for (index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split(',').map(csv_field).collect::<Result<_>>()?;
        if fields.len() != 2 {
            return Err(format!(
                "CSV row {} must have exactly two addresses",
                index + 2
            ));
        }
        map.push(AccountReplacement {
            old: account(fields[0])?,
            new: account(fields[1])?,
        });
    }
    if map.is_empty() {
        return Err(
            "empty mapping requires an explicit separately reviewed no-substitution release".into(),
        );
    }
    map.sort_by(|a, b| a.old.cmp(&b.old));
    recovery::validate_replacements(&map).map_err(|e| format!("invalid mapping: {e:?}"))?;
    Ok(map)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnershipProof {
    pub old: String,
    pub new: String,
    /// SCALE MultiSignature, including its signature-scheme discriminant, over
    /// `replacement_ownership_message`; with `intake`, the new wallet's
    /// signature of that polkaswap.io migration request instead.
    pub signature: String,
    #[serde(default)]
    pub intake: Option<IntakeRequest>,
}

/// The request fields of a polkaswap.io migration submission that are not
/// already bound by the CSV pair. Only v1 requests carry a new-key signature.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct IntakeRequest {
    pub version: u8,
    pub nonce: String,
    pub created_at: String,
}

/// The exact text `migrationMessage` in polkaswap-exchange-web asks wallets to
/// sign. Like `replacement_ownership_message` it binds the genesis, checkpoint
/// and old/new pair; the fixed field shapes keep the lines unambiguous.
fn intake_message(entry: &AccountReplacement, request: &IntakeRequest) -> Result<Vec<u8>> {
    if request.version != 1 {
        return Err("only v1 migration requests carry a new-key signature".into());
    }
    let nonce = request.nonce.as_bytes();
    let uuid_v4 = nonce.len() == 36
        && nonce.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            14 => *c == b'4',
            19 => matches!(c, b'8' | b'9' | b'a' | b'b'),
            _ => matches!(c, b'0'..=b'9' | b'a'..=b'f'),
        });
    let created = request.created_at.as_bytes();
    let iso_millis = created.len() == 24
        && created.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'.',
            23 => *c == b'Z',
            _ => c.is_ascii_digit(),
        });
    if !uuid_v4 || !iso_millis {
        return Err("malformed migration request nonce or timestamp".into());
    }
    let address = |account: &AccountId32| account.to_ss58check_with_version(69u16.into());
    Ok([
        "Polkaswap migration request v1".to_string(),
        "Domain: polkaswap.io".into(),
        "Network: SORA Nexus mainnet".into(),
        format!("Genesis: 0x{}", hex::encode(recovery::GENESIS_HASH)),
        format!("Checkpoint block: {}", recovery::CHECKPOINT_NUMBER),
        format!(
            "Checkpoint hash: 0x{}",
            hex::encode(recovery::CHECKPOINT_HASH)
        ),
        format!("Old account: {}", address(&entry.old)),
        format!("New account: {}", address(&entry.new)),
        format!("Request nonce: {}", request.nonce),
        format!("Created at: {}", request.created_at),
        "I request review of the account replacement above.".into(),
        "This off-chain message does not authorize a transaction or transfer funds.".into(),
    ]
    .join("\n")
    .into_bytes())
}

/// Wallets sign the request as `<Bytes>`-wrapped or raw bytes, with or without
/// a scheme discriminant, matching the page's own `signatureVerify` check.
fn intake_signature_valid(message: &[u8], bytes: &[u8], signer: &AccountId32) -> bool {
    use sp_runtime::traits::Verify;
    let wrapped = [b"<Bytes>".as_slice(), message, b"</Bytes>"].concat();
    let mut candidates = Vec::new();
    if let Ok(signature) = MultiSignature::decode_all(&mut &bytes[..]) {
        candidates.push(signature);
    }
    if let Ok(raw) = <[u8; 64]>::try_from(bytes) {
        candidates.push(sp_core::sr25519::Signature::from_raw(raw).into());
        candidates.push(sp_core::ed25519::Signature::from_raw(raw).into());
    }
    if let Ok(raw) = <[u8; 65]>::try_from(bytes) {
        candidates.push(sp_core::ecdsa::Signature::from_raw(raw).into());
    }
    candidates.iter().any(|signature| {
        signature.verify(wrapped.as_slice(), signer) || signature.verify(message, signer)
    })
}

pub fn verify_ownership_proofs(
    map: &[AccountReplacement],
    proofs: &[OwnershipProof],
) -> Result<()> {
    if proofs.len() != map.len() {
        return Err("each independently controlled replacement requires exactly one proof".into());
    }
    let mut by_old = BTreeMap::new();
    for proof in proofs {
        let old = account(&proof.old)?;
        if by_old.insert(old, proof).is_some() {
            return Err("duplicate ownership proof".into());
        }
    }
    for entry in map {
        let proof = by_old
            .get(&entry.old)
            .ok_or("missing replacement ownership proof")?;
        if account(&proof.new)? != entry.new {
            return Err("proof destination differs from CSV".into());
        }
        let bytes = hex_bytes(&proof.signature)?;
        if bytes.len() > 66 {
            return Err("oversized ownership signature".into());
        }
        let valid = match &proof.intake {
            None => {
                let mut input = bytes.as_slice();
                let signature = MultiSignature::decode(&mut input).map_err(|e| e.to_string())?;
                input.is_empty() && recovery::verify_replacement_ownership(entry, &signature)
            }
            Some(request) => {
                intake_signature_valid(&intake_message(entry, request)?, &bytes, &entry.new)
            }
        };
        if !valid {
            return Err("invalid replacement ownership proof".into());
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureComplete {
    schema: String,
    complete: bool,
    top: TrieCapture,
    children: Vec<TrieCapture>,
    artifacts: BTreeMap<String, CapturedArtifact>,
}

#[derive(Debug, Deserialize)]
struct CapturedArtifact {
    sha256: String,
    bytes: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrieCapture {
    trie: String,
    root: Option<String>,
    key_count: u64,
    page_count: usize,
    pages: Vec<PageReference>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageReference {
    file: String,
    sha256: String,
    key_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageHeader {
    schema: String,
    checkpoint_hash: String,
    trie: String,
    page_index: usize,
    start_key: Option<String>,
    page_size: usize,
    key_count: usize,
    last_key: Option<String>,
    terminal: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationReport {
    pub schema: &'static str,
    pub checkpoint_hash: String,
    pub state_root: String,
    pub top_keys: usize,
    pub child_tries: usize,
    pub header_hash_verified: bool,
    pub extrinsics_root_verified: bool,
    pub state_root_verified: bool,
    pub consensus_proof_verified: bool,
    pub history_complete: bool,
    pub capture_manifest_sha256: String,
}

fn local_path(root: &Path, relative: &str) -> Result<std::path::PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("capture page escapes artifact directory".into());
    }
    let base = root.canonicalize().map_err(|e| e.to_string())?;
    let target = base.join(path).canonicalize().map_err(|e| e.to_string())?;
    if !target.starts_with(&base) {
        return Err("capture symlink escapes artifact directory".into());
    }
    Ok(target)
}

fn read_trie(
    root: &Path,
    capture: &TrieCapture,
    expected_hash: sp_core::H256,
) -> Result<BTreeMap<Vec<u8>, Vec<u8>>> {
    if capture.pages.is_empty() || capture.page_count != capture.pages.len() {
        return Err("missing trie pages".into());
    }
    let mut storage = BTreeMap::new();
    let mut cursor: Option<String> = None;
    let mut terminal = false;
    for (index, page) in capture.pages.iter().enumerate() {
        if terminal {
            return Err("page follows terminal traversal marker".into());
        }
        let bytes = fs::read(local_path(root, &page.file)?).map_err(|e| e.to_string())?;
        if hex::encode(Sha256::digest(&bytes)) != page.sha256 {
            return Err(format!("page hash mismatch: {}", page.file));
        }
        let decoder = GzDecoder::new(bytes.as_slice()).take(256 * 1024 * 1024 + 1);
        let mut lines = BufReader::new(decoder).lines();
        let header: PageHeader = serde_json::from_str(
            &lines
                .next()
                .ok_or("missing page header")?
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if header.schema != "sora-nexus-checkpoint-page-v1"
            || header.checkpoint_hash != format!("0x{}", hex::encode(expected_hash))
            || header.trie != capture.trie
            || header.page_index != index
            || header.start_key != cursor
            || header.page_size == 0
            || header.page_size > 1000
            || header.key_count != page.key_count
            || header.key_count > header.page_size
        {
            return Err("page identity, count or cursor mismatch".into());
        }
        let mut count = 0;
        for line in lines.by_ref() {
            let pair: [String; 2] = serde_json::from_str(&line.map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            if cursor.as_ref().is_some_and(|previous| previous >= &pair[0]) {
                return Err("unordered or repeated storage key".into());
            }
            let key = hex_bytes(&pair[0])?;
            let value = hex_bytes(&pair[1])?;
            if storage.insert(key, value).is_some() {
                return Err("duplicate storage entry".into());
            }
            cursor = Some(pair[0].clone());
            count += 1;
        }
        if count != header.key_count || header.last_key != cursor || header.terminal != (count == 0)
        {
            return Err("page does not exactly cover its declared keys".into());
        }
        terminal = header.terminal;
    }
    if !terminal || storage.len() as u64 != capture.key_count {
        return Err("trie traversal incomplete".into());
    }
    Ok(storage)
}

/// Recompute rather than trusting the RPC provider's root/completeness flags.
/// This verifies state inclusion/completeness relative to the pinned header; it
/// does not establish GRANDPA finality or provide a historical client database.
pub fn verify_capture(root: &Path) -> Result<(VerificationReport, Storage)> {
    let result = verify_capture_at(
        root,
        recovery::CHECKPOINT_NUMBER,
        recovery::CHECKPOINT_HASH.into(),
        recovery::CHECKPOINT_STATE_ROOT.into(),
    )?;
    let code = result
        .1
        .top
        .get(b":code".as_slice())
        .ok_or("checkpoint code missing")?;
    if hex::encode(Sha256::digest(code))
        != "de7173a9e0137a32265b341354383958d85beef81103571e6b0aa09622ef2221"
    {
        return Err("checkpoint runtime code mismatch".into());
    }
    Ok(result)
}

/// Verify a complete historical snapshot against an externally authenticated
/// retained header. Replay callers must establish ancestry to the pinned
/// checkpoint; caller-provided hashes alone do not establish that ancestry.
pub fn verify_capture_at(
    root: &Path,
    expected_number: u32,
    expected_hash: sp_core::H256,
    expected_root: sp_core::H256,
) -> Result<(VerificationReport, Storage)> {
    if expected_number == 0 || expected_number > recovery::CHECKPOINT_NUMBER {
        return Err("historical capture lies outside retained non-genesis execution".into());
    }
    let manifest_bytes = fs::read(root.join("capture-complete.json")).map_err(|e| e.to_string())?;
    let capture: CaptureComplete =
        serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
    if capture.schema != "sora-nexus-checkpoint-capture-v1"
        || !capture.complete
        || capture.top.trie != "top"
    {
        return Err("capture is not complete or supported".into());
    }
    for name in [
        "checkpoint-header.json",
        "parent-header.json",
        "checkpoint-block.json",
        "runtime-version.json",
        "runtime-metadata.scale",
        "runtime-code.wasm",
    ] {
        let binding = capture
            .artifacts
            .get(name)
            .ok_or("missing capture artifact binding")?;
        let bytes = fs::read(local_path(root, name)?).map_err(|e| e.to_string())?;
        if bytes.len() != binding.bytes || hex::encode(Sha256::digest(&bytes)) != binding.sha256 {
            return Err(format!("captured artifact changed: {name}"));
        }
    }
    let header: sp_runtime::generic::Header<u32, BlakeTwo256> = serde_json::from_slice(
        &fs::read(root.join("checkpoint-header.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if header.hash() != expected_hash
        || *header.number() != expected_number
        || *header.state_root() != expected_root
    {
        return Err("checkpoint header hash/root differs from pinned recovery anchor".into());
    }
    let parent: sp_runtime::generic::Header<u32, BlakeTwo256> = serde_json::from_slice(
        &fs::read(root.join("parent-header.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if parent.hash() != *header.parent_hash() || *parent.number() != expected_number - 1 {
        return Err("checkpoint parent header mismatch".into());
    }
    let block: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("checkpoint-block.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let block_header: sp_runtime::generic::Header<u32, BlakeTwo256> =
        serde_json::from_value(block["block"]["header"].clone()).map_err(|e| e.to_string())?;
    if block_header != header {
        return Err("captured block header differs from authenticated header".into());
    }
    let extrinsics = block["block"]["extrinsics"]
        .as_array()
        .ok_or("missing checkpoint extrinsics")?
        .iter()
        .map(|v| hex_bytes(v.as_str().ok_or("non-string extrinsic")?))
        .collect::<Result<Vec<_>>>()?;
    let extrinsics_root = sp_io::trie::blake2_256_ordered_root(extrinsics, StateVersion::V0);
    if extrinsics_root.as_bytes() != header.extrinsics_root().as_bytes() {
        return Err("checkpoint extrinsics root mismatch".into());
    }
    let mut storage = Storage {
        top: read_trie(root, &capture.top, expected_hash)?,
        ..Default::default()
    };
    let top_keys = storage.top.len();
    let child_prefix = b":child_storage:default:";
    let mut expected_children: BTreeMap<Vec<u8>, Vec<u8>> = storage
        .top
        .iter()
        .filter(|(key, _)| key.starts_with(child_prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if storage
        .top
        .keys()
        .any(|key| key.starts_with(b":child_storage:") && !key.starts_with(child_prefix))
    {
        return Err("unsupported child trie kind".into());
    }
    for child in &capture.children {
        let key = hex_bytes(&child.trie)?;
        let expected = expected_children
            .remove(&key)
            .ok_or("unexpected or duplicated child trie")?;
        if child.root.as_deref().map(hex_bytes).transpose()?.as_ref() != Some(&expected) {
            return Err("child root declaration mismatch".into());
        }
        let id = key
            .strip_prefix(child_prefix)
            .ok_or("invalid default child prefix")?
            .to_vec();
        let data = read_trie(root, child, expected_hash)?;
        let computed = sp_io::trie::blake2_256_root(
            data.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            StateVersion::V0,
        );
        if computed.as_bytes() != expected {
            return Err("child trie root mismatch".into());
        }
        storage.top.remove(&key);
        storage.children_default.insert(
            id.clone(),
            StorageChild {
                data,
                child_info: ChildInfo::new_default(&id),
            },
        );
    }
    if !expected_children.is_empty() {
        return Err("child trie capture missing".into());
    }
    let code = storage
        .top
        .get(b":code".as_slice())
        .ok_or("runtime code missing")?
        .clone();
    if code != fs::read(local_path(root, "runtime-code.wasm")?).map_err(|e| e.to_string())? {
        return Err("captured runtime artifact differs from authenticated :code".into());
    }
    // TestExternalities constructors insert :code; pass the actual code explicitly.
    let mut ext =
        sp_io::TestExternalities::new_with_code_and_state(&code, storage.clone(), StateVersion::V0);
    let computed = ext.execute_with(|| sp_io::storage::root(StateVersion::V0));
    if computed.as_slice() != expected_root.as_bytes() {
        return Err(format!(
            "computed state root 0x{} differs from checkpoint",
            hex::encode(computed)
        ));
    }
    Ok((
        VerificationReport {
            schema: "sora-nexus-checkpoint-verification-v1",
            checkpoint_hash: format!("0x{}", hex::encode(expected_hash)),
            state_root: format!("0x{}", hex::encode(computed)),
            top_keys,
            child_tries: storage.children_default.len(),
            header_hash_verified: true,
            extrinsics_root_verified: true,
            state_root_verified: true,
            consensus_proof_verified: false,
            history_complete: false,
            capture_manifest_sha256: hex::encode(Sha256::digest(&manifest_bytes)),
        },
        storage,
    ))
}

pub fn ownership_payloads(map: &[AccountReplacement]) -> serde_json::Value {
    serde_json::json!({
        "schema": "sora-nexus-replacement-signing-payloads-v1",
        "checkpointHash": format!("0x{}", hex::encode(recovery::CHECKPOINT_HASH)),
        "mappingCommitment": format!("0x{}", hex::encode(sp_io::hashing::blake2_256(&map.encode()))),
        "productionReady": false,
        "proofs": map.iter().map(|entry| serde_json::json!({
            "old": format!("0x{}", hex::encode(entry.old.as_ref() as &[u8])),
            "new": format!("0x{}", hex::encode(entry.new.as_ref() as &[u8])),
            "message": format!("0x{}", hex::encode(recovery::replacement_ownership_message(entry))),
        })).collect::<Vec<_>>()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::{sr25519, Pair};

    #[test]
    fn csv_accepts_quoted_header_and_canonicalizes_ss58() {
        let old = AccountId32::from([1; 32]);
        let new = AccountId32::from([2; 32]);
        let csv = format!(
            "\"old account address\",\"new account address\"\n\"{}\",\"{}\"\n",
            old.to_ss58check(),
            new.to_ss58check()
        );
        assert_eq!(
            parse_mapping_csv(&csv).unwrap(),
            vec![AccountReplacement { old, new }]
        );
        assert!(parse_mapping_csv("old,new\n1,2").is_err());
        assert!(parse_mapping_csv("old account address,new account address\n").is_err());
    }

    #[test]
    fn ownership_proof_rejects_changed_csv_and_duplicate_proofs() {
        let key = sr25519::Pair::from_seed(&[9; 32]);
        let map = vec![AccountReplacement {
            old: AccountId32::from([1; 32]),
            new: AccountId32::from(key.public().0),
        }];
        let proof = OwnershipProof {
            old: map[0].old.to_ss58check(),
            new: map[0].new.to_ss58check(),
            signature: format!(
                "0x{}",
                hex::encode(
                    MultiSignature::from(
                        key.sign(&recovery::replacement_ownership_message(&map[0]))
                    )
                    .encode()
                )
            ),
            intake: None,
        };
        assert!(verify_ownership_proofs(&map, &[proof]).is_ok());
        assert!(verify_ownership_proofs(&map, &[]).is_err());
    }

    #[test]
    fn polkaswap_migration_request_signature_proves_the_new_key() {
        // Produced by polkaswap-exchange-web's own polkadot-js signing path for
        // ed25519 seed [7; 32]: `<Bytes>`-wrapped request, scheme-prefixed.
        let page = "0x00639b5894fd12978e77d1ff19793046d45680380592bf4dc6f541f6ad9e6e42f12c7d4b11f14bd6a941254f4a9bd428ea209e2d2f2a61225e3625fdb676edf70e";
        let key = sp_core::ed25519::Pair::from_seed(&[7; 32]);
        let map = vec![AccountReplacement {
            old: AccountId32::from([1; 32]),
            new: AccountId32::from(key.public().0),
        }];
        assert_eq!(
            map[0].new.to_ss58check_with_version(69u16.into()),
            "cnWnUnj4w6PzqBXDBMZzbPGFW6Gh6693ZEVf49cbMyiDWYRQQ"
        );
        let nonce = "123e4567-e89b-42d3-a456-426614174000";
        let request = |version: u8, nonce: &str| IntakeRequest {
            version,
            nonce: nonce.into(),
            created_at: "2026-10-27T01:02:03.004Z".into(),
        };
        let proof = |signature: String, intake: IntakeRequest| OwnershipProof {
            old: map[0].old.to_ss58check(),
            new: map[0].new.to_ss58check(),
            signature,
            intake: Some(intake),
        };
        verify_ownership_proofs(&map, &[proof(page.into(), request(1, nonce))]).unwrap();
        // The signature vouches for exactly one request.
        let other = "123e4567-e89b-42d3-a456-426614174001";
        assert!(verify_ownership_proofs(&map, &[proof(page.into(), request(1, other))]).is_err());
        // v2 requests carry no new-key proof.
        assert!(verify_ownership_proofs(&map, &[proof(page.into(), request(2, nonce))]).is_err());
        // Request fields cannot inject extra message lines.
        let injected = "123e4567-e89b-42d3-a456-42661417400\n";
        assert!(
            verify_ownership_proofs(&map, &[proof(page.into(), request(1, injected))]).is_err()
        );

        let message = intake_message(&map[0], &request(1, nonce)).unwrap();
        let raw = key.sign(&message);
        let raw = format!("0x{}", hex::encode(AsRef::<[u8]>::as_ref(&raw)));
        verify_ownership_proofs(&map, &[proof(raw, request(1, nonce))]).unwrap();
        // Only the replacement key can vouch for the replacement.
        let other_key = sp_core::ed25519::Pair::from_seed(&[8; 32]);
        let forged = other_key.sign(&message);
        let forged = format!("0x{}", hex::encode(AsRef::<[u8]>::as_ref(&forged)));
        assert!(verify_ownership_proofs(&map, &[proof(forged, request(1, nonce))]).is_err());
    }
}
