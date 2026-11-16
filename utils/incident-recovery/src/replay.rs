//! Exact-Wasm historical compatibility witnesses, anchored to the retained
//! checkpoint. This tool never executes native runtime code or mutates a node DB.

use crate::{hex_bytes, local_path, verify_capture_at, Result};
use codec::{DecodeAll, Encode};
use common::recovery::{CHECKPOINT_HASH, CHECKPOINT_NUMBER, CHECKPOINT_STATE_ROOT};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sp_core::{
    storage::{StateVersion, Storage},
    traits::{CallContext, CodeExecutor, RuntimeCode, WrappedRuntimeCode},
    H256,
};
use sp_runtime::{
    generic::{Block, Header},
    traits::{BlakeTwo256, Header as _},
    DigestItem, OpaqueExtrinsic,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(crate) type ReplayHeader = Header<u32, BlakeTwo256>;
type ReplayBlock = Block<ReplayHeader, OpaqueExtrinsic>;
pub(crate) type Executor = sc_executor::WasmExecutor<sp_io::SubstrateHostFunctions>;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Artifact {
    pub file: String,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Boundary {
    Session,
    Epoch,
    Upgrade,
    Checkpoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Range {
    pub name: String,
    /// Why these exact boundaries represent the retained execution path.
    pub rationale: String,
    pub boundaries: Vec<Boundary>,
    pub parent_capture: String,
    pub blocks: Vec<Artifact>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub checkpoint_hash: String,
    pub candidate: Artifact,
    /// JSON array of consecutive sealed headers, ending at the pinned checkpoint.
    pub ancestry: Artifact,
    pub ranges: Vec<Range>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockWitness {
    pub number: u32,
    pub hash: String,
    pub parent_hash: String,
    pub parent_state_root: String,
    pub post_state_root: String,
    pub deployed_executor_hash: String,
    pub candidate_executor_hash: String,
    pub deployed_spec: u32,
    pub substitute_applied: bool,
    pub boundaries: BTreeSet<Boundary>,
    pub identical_state_roots: bool,
    pub native_execution: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RangeWitness {
    pub name: String,
    pub rationale: String,
    pub parent_capture_manifest_sha256: String,
    pub boundaries: BTreeSet<Boundary>,
    pub blocks: Vec<BlockWitness>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: &'static str,
    pub checkpoint_hash: String,
    pub manifest_sha256: String,
    pub candidate_sha256: String,
    pub candidate_blake2: String,
    pub ancestry_sha256: String,
    pub ranges: Vec<RangeWitness>,
    pub representative_boundary_coverage: bool,
    pub consensus_seals_verified: bool,
    pub production_ready: bool,
}

pub(crate) fn artifact(root: &Path, reference: &Artifact, maximum: usize) -> Result<Vec<u8>> {
    let path = local_path(root, &reference.file)?;
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > maximum as u64 {
        return Err(format!("oversized replay artifact: {}", reference.file));
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if hex::encode(Sha256::digest(&bytes)) != reference.sha256 {
        return Err(format!(
            "replay artifact checksum mismatch: {}",
            reference.file
        ));
    }
    Ok(bytes)
}

fn anchored_headers(headers: Vec<ReplayHeader>) -> Result<BTreeMap<u32, ReplayHeader>> {
    let last = headers.last().ok_or("empty replay ancestry")?;
    if *last.number() != CHECKPOINT_NUMBER
        || last.hash() != H256::from(CHECKPOINT_HASH)
        || *last.state_root() != H256::from(CHECKPOINT_STATE_ROOT)
    {
        return Err("ancestry does not end at the exact retained checkpoint".into());
    }
    for pair in headers.windows(2) {
        if pair[0].number().checked_add(1) != Some(*pair[1].number())
            || pair[0].hash() != *pair[1].parent_hash()
        {
            return Err("ancestry has a gap, wrong parent or non-consecutive height".into());
        }
    }
    Ok(headers
        .into_iter()
        .map(|header| (*header.number(), header))
        .collect())
}

fn block_from_rpc(bytes: &[u8], ancestry: &BTreeMap<u32, ReplayHeader>) -> Result<ReplayBlock> {
    let rpc: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let header: ReplayHeader = serde_json::from_value(rpc["block"]["header"].clone())
        .map_err(|e| format!("invalid historical block header: {e}"))?;
    if *header.number() == 0 || *header.number() > CHECKPOINT_NUMBER {
        return Err(
            "replay may execute only retained non-genesis blocks through checkpoint".into(),
        );
    }
    if ancestry.get(header.number()) != Some(&header) {
        return Err("replay block is not authenticated by retained header ancestry".into());
    }
    let bytes = rpc["block"]["extrinsics"]
        .as_array()
        .ok_or("missing historical extrinsics")?
        .iter()
        .map(|value| hex_bytes(value.as_str().ok_or("invalid extrinsic hex")?))
        .collect::<Result<Vec<_>>>()?;
    if sp_io::trie::blake2_256_ordered_root(bytes.clone(), StateVersion::V0)
        != *header.extrinsics_root()
    {
        return Err("historical extrinsics root mismatch".into());
    }
    let extrinsics = bytes
        .into_iter()
        .map(|bytes| {
            OpaqueExtrinsic::try_from_encoded_extrinsic(&bytes)
                .map_err(|e| format!("invalid opaque extrinsic: {e}"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ReplayBlock { header, extrinsics })
}

fn without_seal(mut block: ReplayBlock) -> Result<ReplayBlock> {
    match block.header.digest_mut().logs.pop() {
        Some(DigestItem::Seal(engine, signature))
            if engine == *b"BABE" && signature.len() == 64 => {}
        _ => return Err("historical block must have exactly one trailing BABE seal".into()),
    }
    if block
        .header
        .digest()
        .logs
        .iter()
        .any(|item| matches!(item, DigestItem::Seal(_, _)))
    {
        return Err("historical block contains additional seals".into());
    }
    Ok(block)
}

pub(crate) fn code(ext: &mut sp_io::TestExternalities) -> Result<Vec<u8>> {
    ext.execute_with(|| sp_io::storage::get(b":code").map(|v| v.to_vec()))
        .ok_or_else(|| "replay state has no runtime code".into())
}

pub(crate) fn root(ext: &mut sp_io::TestExternalities) -> H256 {
    H256::from_slice(&ext.execute_with(|| sp_io::storage::root(StateVersion::V0)))
}

pub(crate) fn call(
    executor: &Executor,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
    method: &str,
    data: &[u8],
) -> Result<Vec<u8>> {
    // set_code and recovery activation validate destination Wasm through this
    // host extension. Use the same real executor as block execution.
    ext.register_extension(sp_core::traits::ReadRuntimeVersionExt::new(
        executor.clone(),
    ));
    let fetcher = WrappedRuntimeCode(std::borrow::Cow::Borrowed(code));
    let heap_pages = ext
        .execute_with(|| sp_io::storage::get(b":heappages"))
        .map(|bytes| u64::decode_all(&mut bytes.as_ref()).map_err(|e| e.to_string()))
        .transpose()?;
    let runtime = RuntimeCode {
        code_fetcher: &fetcher,
        heap_pages,
        hash: sp_io::hashing::blake2_256(code).to_vec(),
    };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        executor.call(
            &mut ext.ext(),
            &runtime,
            method,
            data,
            CallContext::Onchain { import: true },
        )
    }));
    let (result, native) = outcome.map_err(|_| format!("Wasm {method} panicked"))?;
    if native {
        return Err("historical replay must never use native execution".into());
    }
    result.map_err(|e| format!("Wasm {method} failed: {e}"))
}

pub(crate) fn version(
    executor: &Executor,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
) -> Result<sp_version::RuntimeVersion> {
    let before = root(ext);
    let result = call(executor, ext, code, "Core_version", &[])?;
    if root(ext) != before {
        return Err("Core_version mutated the replay parent".into());
    }
    let version = sp_version::RuntimeVersion::decode_all(&mut result.as_slice())
        .map_err(|e| e.to_string())?;
    if version.spec_name != "sora-substrate" || version.state_version() != StateVersion::V0 {
        return Err("unsupported historical runtime identity/state trie version".into());
    }
    Ok(version)
}

fn counter<T: DecodeAll>(
    ext: &mut sp_io::TestExternalities,
    pallet: &[u8],
    field: &[u8],
) -> Result<Option<T>> {
    let key = [
        sp_io::hashing::twox_128(pallet),
        sp_io::hashing::twox_128(field),
    ]
    .concat();
    ext.execute_with(|| sp_io::storage::get(&key))
        .map(|value| T::decode_all(&mut value.as_ref()).map_err(|e| e.to_string()))
        .transpose()
}

fn replay_range(
    root_path: &Path,
    range: &Range,
    ancestry: &BTreeMap<u32, ReplayHeader>,
    candidate: &[u8],
    executor: &Executor,
) -> Result<RangeWitness> {
    if range.name.trim().is_empty()
        || range.rationale.trim().is_empty()
        || range.blocks.is_empty()
        || range.boundaries.is_empty()
    {
        return Err("replay range requires a name, rationale, boundaries and blocks".into());
    }
    let blocks = range
        .blocks
        .iter()
        .map(|reference| {
            block_from_rpc(&artifact(root_path, reference, 64 * 1024 * 1024)?, ancestry)
        })
        .collect::<Result<Vec<_>>>()?;
    for pair in blocks.windows(2) {
        if pair[0].header.number().checked_add(1) != Some(*pair[1].header.number())
            || pair[0].header.hash() != *pair[1].header.parent_hash()
        {
            return Err("replay range blocks must be contiguous and ordered".into());
        }
    }
    let first = &blocks[0].header;
    let parent = ancestry
        .get(&(first.number() - 1))
        .ok_or("parent header missing from ancestry")?;
    let capture = local_path(root_path, &range.parent_capture)?;
    let (verified, storage): (_, Storage) = verify_capture_at(
        &capture,
        *parent.number(),
        parent.hash(),
        *parent.state_root(),
    )?;
    let initial_code = storage
        .top
        .get(b":code".as_slice())
        .ok_or("missing parent :code")?
        .clone();
    let mut deployed = sp_io::TestExternalities::new_with_code_and_state(
        &initial_code,
        storage.clone(),
        StateVersion::V0,
    );
    let mut substitute =
        sp_io::TestExternalities::new_with_code_and_state(&initial_code, storage, StateVersion::V0);
    let candidate_version = version(executor, &mut substitute, candidate)?;
    if candidate_version.spec_version != 134 || candidate_version.transaction_version != 131 {
        return Err("candidate is not the immutable spec-134/transaction-131 bootstrap".into());
    }
    let mut witnesses = Vec::new();
    let mut observed = BTreeSet::new();
    for block in blocks {
        let header = block.header.clone();
        let parent = ancestry
            .get(&(header.number() - 1))
            .ok_or("missing parent")?;
        if root(&mut deployed) != *parent.state_root()
            || root(&mut substitute) != *parent.state_root()
        {
            return Err(format!(
                "parent state differs before replaying {}",
                header.number()
            ));
        }
        let deployed_code = code(&mut deployed)?;
        if code(&mut substitute)? != deployed_code {
            return Err("branches diverged in stored :code".into());
        }
        let deployed_version = version(executor, &mut deployed, &deployed_code)?;
        if deployed_version.spec_version > 134 || matches!(deployed_version.spec_version, 57 | 115)
        {
            return Err("historical runtime requires an unsupported substitute or reserved recovery version".into());
        }
        let apply_substitute = deployed_version.spec_version == 134;
        if apply_substitute && deployed_version.transaction_version != 131 {
            return Err("retained spec-134 transaction identity differs".into());
        }
        let candidate_code = if apply_substitute {
            candidate
        } else {
            &deployed_code
        };
        let session_before = counter::<u32>(&mut deployed, b"Session", b"CurrentIndex")?;
        let epoch_before = counter::<u64>(&mut deployed, b"Babe", b"EpochIndex")?;
        let execution = without_seal(block)?.encode();
        let deployed_output = call(
            executor,
            &mut deployed,
            &deployed_code,
            "Core_execute_block",
            &execution,
        )?;
        let candidate_output = call(
            executor,
            &mut substitute,
            candidate_code,
            "Core_execute_block",
            &execution,
        )?;
        if !deployed_output.is_empty() || !candidate_output.is_empty() {
            return Err("Core_execute_block returned a non-unit result".into());
        }
        deployed
            .commit_all()
            .map_err(|e| format!("deployed open transaction: {e}"))?;
        substitute
            .commit_all()
            .map_err(|e| format!("candidate open transaction: {e}"))?;
        if root(&mut deployed) != *header.state_root()
            || root(&mut substitute) != *header.state_root()
        {
            return Err(format!(
                "state-root compatibility failed at {}",
                header.number()
            ));
        }
        let mut boundaries = BTreeSet::new();
        if counter::<u32>(&mut deployed, b"Session", b"CurrentIndex")? != session_before {
            boundaries.insert(Boundary::Session);
        }
        if counter::<u64>(&mut deployed, b"Babe", b"EpochIndex")? != epoch_before {
            boundaries.insert(Boundary::Epoch);
        }
        if code(&mut deployed)? != deployed_code {
            boundaries.insert(Boundary::Upgrade);
        }
        if *header.number() == CHECKPOINT_NUMBER {
            boundaries.insert(Boundary::Checkpoint);
        }
        observed.extend(&boundaries);
        witnesses.push(BlockWitness {
            number: *header.number(),
            hash: format!("{:#x}", header.hash()),
            parent_hash: format!("{:#x}", header.parent_hash()),
            parent_state_root: format!("{:#x}", parent.state_root()),
            post_state_root: format!("{:#x}", header.state_root()),
            deployed_executor_hash: format!(
                "0x{}",
                hex::encode(sp_io::hashing::blake2_256(&deployed_code))
            ),
            candidate_executor_hash: format!(
                "0x{}",
                hex::encode(sp_io::hashing::blake2_256(candidate_code))
            ),
            deployed_spec: deployed_version.spec_version,
            substitute_applied: apply_substitute,
            boundaries,
            identical_state_roots: true,
            native_execution: false,
        });
    }
    if !witnesses.iter().any(|block| block.substitute_applied) {
        return Err("range never exercised spec-134 substitution".into());
    }
    if range
        .boundaries
        .iter()
        .any(|boundary| !observed.contains(boundary))
    {
        return Err("declared boundary was not observed in executed storage transitions".into());
    }
    Ok(RangeWitness {
        name: range.name.clone(),
        rationale: range.rationale.clone(),
        parent_capture_manifest_sha256: verified.capture_manifest_sha256,
        boundaries: observed,
        blocks: witnesses,
    })
}

pub fn replay(manifest_path: &Path) -> Result<Report> {
    let root_path = manifest_path
        .parent()
        .ok_or("manifest has no parent directory")?;
    let manifest_bytes = fs::read(manifest_path).map_err(|e| e.to_string())?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
    if manifest.schema != "sora-nexus-historical-replay-v1"
        || hex_bytes(&manifest.checkpoint_hash)?.as_slice() != CHECKPOINT_HASH
        || manifest.ranges.is_empty()
    {
        return Err("wrong historical replay manifest identity or empty ranges".into());
    }
    let candidate = artifact(root_path, &manifest.candidate, 32 * 1024 * 1024)?;
    let ancestry_bytes = artifact(root_path, &manifest.ancestry, 256 * 1024 * 1024)?;
    let ancestry =
        anchored_headers(serde_json::from_slice(&ancestry_bytes).map_err(|e| e.to_string())?)?;
    let executor = Executor::builder().build();
    let ranges = manifest
        .ranges
        .iter()
        .map(|range| replay_range(root_path, range, &ancestry, &candidate, &executor))
        .collect::<Result<Vec<_>>>()?;
    let observed: BTreeSet<_> = ranges
        .iter()
        .flat_map(|range| range.boundaries.iter().copied())
        .collect();
    let covered = [
        Boundary::Session,
        Boundary::Epoch,
        Boundary::Upgrade,
        Boundary::Checkpoint,
    ]
    .iter()
    .all(|boundary| observed.contains(boundary));
    Ok(Report {
        schema: "sora-nexus-historical-replay-witness-v1",
        checkpoint_hash: manifest.checkpoint_hash,
        manifest_sha256: hex::encode(Sha256::digest(&manifest_bytes)),
        candidate_sha256: manifest.candidate.sha256,
        candidate_blake2: format!("0x{}", hex::encode(sp_io::hashing::blake2_256(&candidate))),
        ancestry_sha256: manifest.ancestry.sha256,
        ranges,
        representative_boundary_coverage: covered,
        consensus_seals_verified: false,
        production_ready: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn header(number: u32) -> ReplayHeader {
        ReplayHeader::new(
            number,
            H256::zero(),
            H256::zero(),
            H256::zero(),
            Default::default(),
        )
    }
    #[test]
    fn unattached_or_post_checkpoint_history_is_rejected() {
        assert!(anchored_headers(vec![header(CHECKPOINT_NUMBER)]).is_err());
        assert!(anchored_headers(vec![header(CHECKPOINT_NUMBER + 1)]).is_err());
        assert!(anchored_headers(vec![]).is_err());
    }
    #[test]
    fn strips_only_one_trailing_babe_seal() {
        let mut block = ReplayBlock {
            header: header(1),
            extrinsics: vec![],
        };
        assert!(without_seal(block.clone()).is_err());
        block
            .header
            .digest_mut()
            .logs
            .push(DigestItem::Seal(*b"BABE", vec![0; 64]));
        assert!(without_seal(block.clone())
            .unwrap()
            .header
            .digest()
            .logs
            .is_empty());
        block
            .header
            .digest_mut()
            .logs
            .push(DigestItem::Seal(*b"BABE", vec![0; 64]));
        assert!(without_seal(block).is_err());
    }
}
