//! Offline native expectations from the exact compiled bootstrap engine.
//! This binary does not accept policy overrides or fixture configuration writes.
use codec::{Compact, Decode, DecodeAll, DecodeLimit, Encode};
use common::recovery::{self, RecoveryAction, RecoveryInherent, RecoveryPhase};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest as _, Sha256};
use sp_core::{storage::StateVersion, H256};
use sp_runtime::{
    generic::{Block, Header},
    traits::{BlakeTwo256, Header as _},
    ApplyExtrinsicResult, Digest, DigestItem, ExtrinsicInclusionMode, OpaqueExtrinsic,
};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
mod replay;
// The reviewed immutable journal is shared verbatim except its host feature gates.
// Its public writer is used below for fresh independent disk restoration as well.
mod binding;
mod snapshot;
type Result<T> = std::result::Result<T, String>;
type ReplayHeader = Header<u32, BlakeTwo256>;
type ReplayBlock = Block<ReplayHeader, OpaqueExtrinsic>;
const MAX_INPUT: usize = 64 * 1024 * 1024;
const MAX_ROWS: usize = 4096;
const MAX_VALUE: usize = 32 * 1024 * 1024;
const MAX_READ: usize = 64 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema: String,
    input_root: PathBuf,
    engine: replay::Artifact,
    policy: replay::Artifact,
    parent_header: replay::Artifact,
    parent_snapshot_directory: String,
    parent_snapshot_manifest: String,
    parent_snapshot_sha256: String,
    // The outer rehearsal must join this reference to its verified prior block.
    prior_replay_evidence: replay::Artifact,
    envelope: replay::Artifact,
    slot: u64,
    authority_index: u32,
    timestamp: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValueHash {
    bytes: usize,
    blake2_256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct WriteDelta {
    key: String,
    before: Option<ValueHash>,
    after: Option<ValueHash>,
    changed: bool,
}
fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn executable_pin() -> Result<(PathBuf, String)> {
    let path = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(|e| e.to_string())?;
    let mut file = fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut size = 0usize;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        size = size.checked_add(count).ok_or("executable size overflow")?;
        require(size <= 2 * 1024 * 1024 * 1024, "executable size bound")?;
        digest.update(&buffer[..count]);
    }
    Ok((path, hex::encode(digest.finalize())))
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn hex_bytes(text: &str) -> Result<Vec<u8>> {
    hex::decode(text.strip_prefix("0x").unwrap_or(text)).map_err(|e| e.to_string())
}
fn local_path(root: &Path, relative: &str) -> Result<PathBuf> {
    require(
        !relative.contains('\\')
            && relative
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".."),
        "artifact path aliases are forbidden",
    )?;
    let relative = Path::new(relative);
    require(
        !relative.as_os_str().is_empty()
            && !relative.is_absolute()
            && relative
                .components()
                .all(|p| matches!(p, Component::Normal(_))),
        "noncanonical artifact path",
    )?;
    require(
        root.is_absolute() && root.canonicalize().map_err(|e| e.to_string())? == root,
        "noncanonical input root",
    )?;
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part);
        require(
            !fs::symlink_metadata(&path)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink(),
            "artifact symlink forbidden",
        )?;
    }
    Ok(path)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| e.to_string())
}
fn bare(call: Vec<u8>) -> Result<OpaqueExtrinsic> {
    let mut bytes = vec![4];
    bytes.extend(call);
    OpaqueExtrinsic::try_from_encoded_extrinsic(&bytes.encode()).map_err(|e| e.to_string())
}
fn remark(bytes: &[u8]) -> Result<OpaqueExtrinsic> {
    let mut call = vec![0, 0];
    call.extend(bytes.encode());
    bare(call)
}
fn timestamp(now: u64) -> Result<OpaqueExtrinsic> {
    let mut call = vec![1, 0];
    call.extend(Compact(now).encode());
    bare(call)
}
fn clone_state(
    mut state: sp_io::TestExternalities,
) -> Result<(sp_io::TestExternalities, sp_io::TestExternalities)> {
    state.commit_all().map_err(|e| e.to_string())?;
    let root = replay::root(&mut state);
    let (raw, observed) = state.into_raw_snapshot();
    require(root == observed, "raw clone root differs")?;
    Ok((
        sp_io::TestExternalities::from_raw_snapshot(raw.clone(), root, StateVersion::V0),
        sp_io::TestExternalities::from_raw_snapshot(raw, root, StateVersion::V0),
    ))
}
fn hash_at(
    ext: &mut sp_io::TestExternalities,
    key: &[u8],
    budget: &mut usize,
) -> Result<Option<ValueHash>> {
    ext.execute_with(|| {
        let Some(length) = sp_io::storage::read(key, &mut [], 0) else {
            return Ok(None);
        };
        require(length as usize <= MAX_VALUE, "recorded value exceeds bound")?;
        *budget = budget
            .checked_sub(length as usize)
            .ok_or("recorded aggregate exceeds bound")?;
        let value = sp_io::storage::get(key).ok_or("value disappeared")?;
        require(
            value.len() == length as usize,
            "recorded value length changed",
        )?;
        Ok(Some(ValueHash {
            bytes: value.len(),
            blake2_256: hx(&sp_io::hashing::blake2_256(&value)),
        }))
    })
}
fn deltas(
    before: &mut sp_io::TestExternalities,
    after: &mut sp_io::TestExternalities,
) -> Result<Vec<WriteDelta>> {
    require(
        !after
            .overlayed_changes()
            .children()
            .any(|(mut entries, _)| entries.next().is_some()),
        "child writes are unsupported by this initial recorder",
    )?;
    let keys: BTreeSet<Vec<u8>> = after
        .overlayed_changes()
        .changes()
        .map(|(key, _)| key.clone())
        .collect();
    require(
        !keys.is_empty() && keys.len() <= MAX_ROWS,
        "empty/oversized native write set",
    )?;
    let mut budget = MAX_READ;
    let mut output = Vec::new();
    for key in keys {
        require(
            !key.is_empty() && key.len() <= recovery::MAX_STORAGE_KEY_BYTES,
            "write key exceeds bound",
        )?;
        let prior = hash_at(before, &key, &mut budget)?;
        let next = hash_at(after, &key, &mut budget)?;
        output.push(WriteDelta {
            key: hx(&key),
            changed: prior != next,
            before: prior,
            after: next,
        });
    }
    Ok(output)
}
fn initialized_checks(
    ext: &mut sp_io::TestExternalities,
    policy: &recovery::RecoveryPolicy,
    index: u32,
) -> Result<()> {
    ext.execute_with(|| {
        let status = recovery::storage::status()
            .map_err(|e| format!("status:{e:?}"))?
            .ok_or("status absent")?;
        require(
            status.phase == RecoveryPhase::Applying
                && status.next_chunk == index
                && status.operation_counter == 0
                && status.failure.is_none()
                && status.policy == policy.commitment(),
            "noninitial/hold/repaired/wrong-cursor state",
        )?;
        for key in [
            recovery::POLICY_STORAGE_KEY,
            recovery::ACTIVE_POLICY_STORAGE_KEY,
        ] {
            require(
                sp_io::storage::get(key).as_deref() == Some(policy.commitment().as_slice()),
                "stored policy mismatch",
            )?;
        }
        require(
            sp_io::storage::get(recovery::BRIDGE_HALT_STORAGE_KEY).is_some(),
            "persistent bridge halt absent",
        )?;
        require(
            recovery::TRANSIENT_CONTEXT_KEYS
                .iter()
                .all(|key| !sp_io::storage::exists(key)),
            "transient context present before native operation",
        )
    })
}
fn decode_bounded<T: Decode + Encode>(bytes: &[u8], maximum: usize) -> Result<T> {
    require(
        !bytes.is_empty() && bytes.len() <= maximum,
        "bounded SCALE input",
    )?;
    let mut input = bytes;
    let value = T::decode_with_depth_limit(
        32,
        &mut codec::MemTrackingInput::new(
            &mut input,
            maximum.checked_mul(4).ok_or("decode bound overflow")?,
        ),
    )
    .map_err(|e| e.to_string())?;
    require(
        input.is_empty() && value.encode() == bytes,
        "noncanonical bounded SCALE",
    )?;
    Ok(value)
}
fn run(manifest_path: &Path, output: &Path) -> Result<serde_json::Value> {
    let executable_before = executable_pin()?;
    require(
        output.is_absolute()
            && !output.exists()
            && output
                .parent()
                .is_some_and(|parent| parent.canonicalize().is_ok_and(|actual| actual == parent)),
        "output must be new with a canonical absolute parent",
    )?;
    require(
        manifest_path.is_absolute()
            && manifest_path.canonicalize().map_err(|e| e.to_string())? == manifest_path,
        "manifest path must be canonical",
    )?;
    let mut manifest_bytes = Vec::new();
    fs::File::open(manifest_path)
        .map_err(|e| e.to_string())?
        .take(MAX_INPUT as u64 + 1)
        .read_to_end(&mut manifest_bytes)
        .map_err(|e| e.to_string())?;
    require(manifest_bytes.len() <= MAX_INPUT, "manifest bound")?;
    let m: Manifest = serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
    require(
        m.schema == "sora-nexus-native-operation-recorder-v1",
        "manifest schema",
    )?;
    let code = replay::artifact(&m.input_root, &m.engine, MAX_VALUE)?;
    require(
        sha(&code) == binding::ENGINE_SHA256
            && sp_io::hashing::blake2_256(&code) == binding::ENGINE_BLAKE2,
        "executing code differs from compiled native engine binding",
    )?;
    let policy_bytes =
        replay::artifact(&m.input_root, &m.policy, recovery::MAX_INHERENT_BYTES * 3)?;
    let policy: recovery::RecoveryPolicy =
        decode_bounded(&policy_bytes, recovery::MAX_INHERENT_BYTES * 3)?;
    policy.validate().map_err(|e| format!("policy:{e:?}"))?;
    require(
        policy.commitment() == binding::POLICY_COMMITMENT,
        "policy differs from native engine binding",
    )?;
    let (compiled, original) = framenode_runtime::native_recovery_recorder_policies()
        .map_err(|e| format!("compiled policy:{e:?}"))?;
    require(
        compiled == policy && original == policy,
        "native compiled/original policy differs; hold/repair unsupported",
    )?;
    require(
        framenode_runtime::VERSION.spec_version == 134
            && framenode_runtime::VERSION.transaction_version == 131,
        "native engine is not preserved bootstrap134/131",
    )?;
    let header_bytes = replay::artifact(&m.input_root, &m.parent_header, MAX_INPUT)?;
    let parent: ReplayHeader = decode_bounded(&header_bytes, 1024 * 1024)?;
    require(
        *parent.number() >= recovery::CHECKPOINT_NUMBER,
        "parent precedes recovery checkpoint",
    )?;
    if *parent.number() == recovery::CHECKPOINT_NUMBER {
        require(
            parent.hash() == H256::from(recovery::CHECKPOINT_HASH)
                && *parent.state_root() == H256::from(recovery::CHECKPOINT_STATE_ROOT),
            "checkpoint header mismatch",
        )?;
    }
    let prior_evidence = replay::artifact(&m.input_root, &m.prior_replay_evidence, MAX_INPUT)?;
    require(!prior_evidence.is_empty(), "prior replay evidence absent")?;
    let envelope_bytes =
        replay::artifact(&m.input_root, &m.envelope, recovery::MAX_INHERENT_BYTES)?;
    let envelope =
        RecoveryInherent::decode_remark(&envelope_bytes).map_err(|e| format!("envelope:{e:?}"))?;
    require(
        envelope.encode_remark() == envelope_bytes,
        "noncanonical recovery envelope",
    )?;
    let (kind, index, operation_commitment) = match &envelope.action {
        RecoveryAction::Incoming(bytes) => {
            let op: framenode_runtime::incident_recovery_incoming::IncomingOperation =
                decode_bounded(bytes, recovery::MAX_CHUNK_BYTES)?;
            ("incoming", op.index, op.commitment())
        }
        RecoveryAction::Outgoing(bytes) => {
            let op: framenode_runtime::incident_recovery_outgoing::OutgoingOperation =
                decode_bounded(bytes, recovery::MAX_CHUNK_BYTES)?;
            ("outgoing", op.index, op.commitment())
        }
        _ => {
            return Err("only original committed Incoming/Outgoing operations are supported".into())
        }
    };
    require(
        policy.chunk_hashes.get(index as usize) == Some(&operation_commitment),
        "operation is absent from compiled policy",
    )?;
    let directory = local_path(&m.input_root, &m.parent_snapshot_directory)?;
    let mut ext = snapshot::restore_pinned(
        &directory,
        &m.parent_snapshot_manifest,
        &m.parent_snapshot_sha256,
        *parent.state_root(),
    )?;
    let stored_code_before = ext
        .execute_with(|| sp_io::storage::get(b":code"))
        .ok_or("stored code absent")?;
    let executor = replay::Executor::builder().build();
    let version_before = replay::root(&mut ext);
    let version_bytes = replay::call(&executor, &mut ext, &code, "Core_version", &[])?;
    require(
        replay::root(&mut ext) == version_before
            && version_bytes == framenode_runtime::VERSION.encode(),
        "native/Wasm runtime version or API mismatch",
    )?;
    let header = ReplayHeader::new(
        parent.number().checked_add(1).ok_or("height overflow")?,
        H256::zero(),
        H256::zero(),
        parent.hash(),
        Digest {
            logs: vec![DigestItem::PreRuntime(
                *b"BABE",
                (2u8, m.authority_index, m.slot).encode(),
            )],
        },
    );
    let initialized = replay::call(
        &executor,
        &mut ext,
        &code,
        "Core_initialize_block",
        &header.encode(),
    )?;
    require(
        ExtrinsicInclusionMode::decode_all(&mut initialized.as_slice())
            .map_err(|e| e.to_string())?
            == ExtrinsicInclusionMode::OnlyInherents,
        "bootstrap admitted ordinary extrinsics",
    )?;
    let time = timestamp(m.timestamp)?;
    let time_result = replay::call(
        &executor,
        &mut ext,
        &code,
        "BlockBuilder_apply_extrinsic",
        &time.encode(),
    )?;
    require(
        ApplyExtrinsicResult::decode_all(&mut time_result.as_slice()).map_err(|e| e.to_string())?
            == Ok(Ok(())),
        "timestamp prefix rejected",
    )?;
    initialized_checks(&mut ext, &policy, index)?;
    let initialized_root = replay::root(&mut ext);
    let (before, mut wasm) = clone_state(ext)?;
    let (mut before, mut native) = clone_state(before)?;
    let extrinsic = remark(&envelope_bytes)?;
    let encoded_extrinsic = extrinsic.encode();
    let native_extrinsic =
        framenode_runtime::UncheckedExtrinsic::decode_all(&mut encoded_extrinsic.as_slice())
            .map_err(|e| e.to_string())?;
    let native_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        native.execute_with(|| framenode_runtime::native_recovery_recorder_apply(native_extrinsic))
    }))
    .map_err(|_| "native engine panic")?;
    require(
        native_result == Ok(Ok(())),
        "native committed operation rejected",
    )?;
    let native_delta = deltas(&mut before, &mut native)?;
    let wasm_bytes = replay::call(
        &executor,
        &mut wasm,
        &code,
        "BlockBuilder_apply_extrinsic",
        &encoded_extrinsic,
    )?;
    let wasm_result =
        ApplyExtrinsicResult::decode_all(&mut wasm_bytes.as_slice()).map_err(|e| e.to_string())?;
    require(
        wasm_result == native_result,
        "native/Wasm operation outcome mismatch",
    )?;
    let wasm_delta = deltas(&mut before, &mut wasm)?;
    let operation_root = replay::root(&mut wasm);
    require(
        native_delta == wasm_delta && replay::root(&mut native) == operation_root,
        "native/Wasm complete operation delta/root mismatch",
    )?;
    let terminal_status = wasm
        .execute_with(recovery::storage::status)
        .map_err(|e| format!("terminal:{e:?}"))?
        .ok_or("terminal status absent")?;
    require(
        terminal_status.next_chunk == index.checked_add(1).ok_or("index overflow")?
            && terminal_status.operation_counter == 0
            && terminal_status.failure.is_none()
            && matches!(
                terminal_status.phase,
                RecoveryPhase::Applying | RecoveryPhase::Auditing
            ),
        "native operation froze or skipped cursor",
    )?;
    require(
        wasm.execute_with(|| {
            recovery::TRANSIENT_CONTEXT_KEYS
                .iter()
                .all(|key| !sp_io::storage::exists(key))
        }),
        "operation leaked transient context",
    )?;
    let finalized_bytes = replay::call(
        &executor,
        &mut wasm,
        &code,
        "BlockBuilder_finalize_block",
        &[],
    )?;
    let finalized =
        ReplayHeader::decode_all(&mut finalized_bytes.as_slice()).map_err(|e| e.to_string())?;
    require(
        finalized.parent_hash() == &parent.hash()
            && finalized.number() == header.number()
            && finalized.state_root() == &replay::root(&mut wasm),
        "finalized block binding",
    )?;
    let block = ReplayBlock {
        header: finalized.clone(),
        extrinsics: vec![time, extrinsic],
    };
    require(
        *finalized.extrinsics_root()
            == sp_io::trie::blake2_256_ordered_root(
                block.extrinsics.iter().map(Encode::encode).collect(),
                StateVersion::V0,
            ),
        "finalized extrinsics root",
    )?;
    let mut imported = snapshot::restore_pinned(
        &directory,
        &m.parent_snapshot_manifest,
        &m.parent_snapshot_sha256,
        *parent.state_root(),
    )?;
    require(
        replay::call(
            &executor,
            &mut imported,
            &code,
            "Core_execute_block",
            &block.encode(),
        )?
        .is_empty()
            && replay::root(&mut imported) == *finalized.state_root(),
        "independent parent import differs",
    )?;
    require(
        wasm.execute_with(|| sp_io::storage::get(b":code")) == Some(stored_code_before.clone()),
        "dynamic operation changed runtime code",
    )?;
    fs::create_dir(output).map_err(|e| format!("output must be new:{e}"))?;
    let mut journal = snapshot::Journal::new(output)?;
    let (persisted, snapshot_sha) = journal.persist(wasm)?;
    drop(persisted);
    drop(journal);
    let mut restored = snapshot::restore_pinned(
        output,
        "snapshots/000000.json",
        &snapshot_sha,
        *finalized.state_root(),
    )?;
    require(
        replay::root(&mut restored) == *finalized.state_root(),
        "fresh disk restore differs",
    )?;
    write_new(&output.join("block.scale"), &block.encode())?;
    write_new(&output.join("original-extrinsic.scale"), &encoded_extrinsic)?;
    write_new(&output.join("manifest.json"), &manifest_bytes)?;
    let result = json!({"schema":"sora-nexus-native-operation-expectations-v1","engineBinding":binding::BINDING_JSON,
        "nativeExecutable":executable_before.0,"nativeExecutableSha256":executable_before.1,
        "manifestSha256":sha(&manifest_bytes),"engineSha256":sha(&code),"policyCommitment":hx(&policy.commitment()),"parentHeaderSha256":sha(&header_bytes),"parentHash":hx(parent.hash().as_bytes()),"parentStateRoot":hx(parent.state_root().as_bytes()),"parentSnapshotManifestSha256":m.parent_snapshot_sha256,"priorReplayEvidenceSha256":sha(&prior_evidence),"initializedStateRoot":hx(initialized_root.as_bytes()),"originalEnvelopeSha256":sha(&envelope_bytes),"originalExtrinsicSha256":sha(&encoded_extrinsic),"operationKind":kind,"operationIndex":index,"operationCommitment":hx(&operation_commitment),"nativeRuntimeVersionScale":hx(&version_bytes),"operationStateRoot":hx(operation_root.as_bytes()),"writes":native_delta,"terminalStatusScale":hx(&terminal_status.encode()),"blockSha256":sha(&block.encode()),"blockHash":hx(finalized.hash().as_bytes()),"stateRoot":hx(finalized.state_root().as_bytes()),"terminalSnapshotSha256":snapshot_sha,
        "completeTopWriteSetCompared":true,"consensusPrefixWaiver":false,"childWritesSupported":false,"compiledPolicyOverrideUsed":false,"configurationInjected":false,"nativeWasmOperationParityVerified":true,"independentImportVerified":true,"freshDiskRestoreVerified":true,"priorReplayLineageVerified":false,"sourceFinalityVerified":false,"economicPolicyIndependentlyVerified":false,"consensusSealsVerified":false,"holdRepairSupported":false,"fixtureOnly":binding::FIXTURE_ONLY,"productionReady":false});
    require(
        fs::read(manifest_path).map_err(|e| e.to_string())? == manifest_bytes,
        "manifest changed",
    )?;
    for (reference, expected) in [
        (&m.engine, &code),
        (&m.policy, &policy_bytes),
        (&m.parent_header, &header_bytes),
        (&m.prior_replay_evidence, &prior_evidence),
        (&m.envelope, &envelope_bytes),
    ] {
        require(
            replay::artifact(&m.input_root, reference, MAX_INPUT)? == *expected,
            "bound input changed",
        )?;
    }
    require(
        executable_pin()? == executable_before,
        "native executable changed",
    )?;
    write_new(
        &output.join("report.json"),
        &serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )?;
    Ok(result)
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let result = if args.len() == 3 {
        run(Path::new(&args[1]), Path::new(&args[2]))
    } else {
        Err("usage: sora-nexus-native-operation-recorder ABSOLUTE_MANIFEST NEW_OUTPUT".into())
    };
    match result {
        Ok(report) => println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("JSON report")
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn public_policy() -> recovery::RecoveryPolicy {
        recovery::RecoveryPolicy {
            format_version: 2,
            authorization_domain: recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![sp_core::crypto::AccountId32::from([9; 32])],
            chunk_hashes: vec![[1; 32]],
            audit_chunk_hashes: vec![[2; 32]],
            evidence_commitment: [3; 32],
            terminal_audit_commitment: [4; 32],
        }
    }
    #[test]
    fn malformed_compact_lengths_and_depth_are_bounded() {
        assert!(decode_bounded::<Vec<Vec<u8>>>(&[0xff; 16], 64).is_err());
        assert!(decode_bounded::<Vec<u8>>(&[0x05, 0x00, 0x01], 64).is_err());
        #[derive(codec::Decode, codec::Encode)]
        enum Recursive {
            End,
            Child(Box<Recursive>),
        }
        let mut nested = Recursive::End;
        for _ in 0..40 {
            nested = Recursive::Child(Box::new(nested));
        }
        assert!(decode_bounded::<Recursive>(&nested.encode(), 256).is_err());
    }
    #[test]
    fn complete_write_delta_keeps_system_and_noops_and_deletion() {
        let mut state = sp_io::TestExternalities::default();
        state.execute_with(|| {
            sp_io::storage::set(b"deleted", b"before");
            sp_io::storage::set(b"unchanged", b"same");
        });
        let (mut before, mut after) = clone_state(state).unwrap();
        let system = [
            sp_io::hashing::twox_128(b"System"),
            sp_io::hashing::twox_128(b"Events"),
        ]
        .concat();
        after.execute_with(|| {
            sp_io::storage::clear(b"deleted");
            sp_io::storage::set(b"unchanged", b"same");
            sp_io::storage::set(&system, b"event");
        });
        let rows = deltas(&mut before, &mut after).unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows
            .iter()
            .any(|r| r.key == hx(&system) && r.changed && r.before.is_none() && r.after.is_some()));
        assert!(rows
            .iter()
            .any(|r| r.key == hx(b"deleted") && r.changed && r.after.is_none()));
        assert!(rows.iter().any(|r| r.key == hx(b"unchanged") && !r.changed));
    }
    #[test]
    fn child_write_is_rejected_even_when_top_state_matches() {
        let (mut before, mut after) = clone_state(sp_io::TestExternalities::default()).unwrap();
        after.execute_with(|| {
            sp_io::storage::set(b"top", b"value");
            sp_io::default_child_storage::set(b"child", b"key", b"value");
        });
        assert!(deltas(&mut before, &mut after)
            .unwrap_err()
            .contains("child writes"));
    }
    #[test]
    fn oversized_key_and_aggregate_rows_fail_closed() {
        let (mut before, mut after) = clone_state(sp_io::TestExternalities::default()).unwrap();
        after.execute_with(|| {
            sp_io::storage::set(&vec![1; recovery::MAX_STORAGE_KEY_BYTES + 1], b"v")
        });
        assert!(deltas(&mut before, &mut after).is_err());
        let (mut before, mut after) = clone_state(sp_io::TestExternalities::default()).unwrap();
        after.execute_with(|| {
            for index in 0..=MAX_ROWS {
                let key = u32::try_from(index).expect("bounded test row index");
                sp_io::storage::set(&key.encode(), b"v");
            }
        });
        assert!(deltas(&mut before, &mut after).is_err());
    }
    #[test]
    fn initialization_rejects_hold_counter_and_transient_context() {
        let policy = public_policy();
        policy.validate().unwrap();
        let mut state = sp_io::TestExternalities::default();
        let mut status = recovery::RecoveryStatus {
            format_version: 1,
            policy: policy.commitment(),
            phase: RecoveryPhase::Applying,
            next_chunk: 0,
            operation_counter: 0,
            failure: None,
        };
        state.execute_with(|| {
            for key in [
                recovery::POLICY_STORAGE_KEY,
                recovery::ACTIVE_POLICY_STORAGE_KEY,
            ] {
                sp_io::storage::set(key, &policy.commitment());
            }
            sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
            sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &status.encode());
        });
        initialized_checks(&mut state, &policy, 0).unwrap();
        status.operation_counter = 1;
        state.execute_with(|| sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &status.encode()));
        assert!(initialized_checks(&mut state, &policy, 0).is_err());
        status.operation_counter = 0;
        status.phase = RecoveryPhase::Hold;
        state.execute_with(|| sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &status.encode()));
        assert!(initialized_checks(&mut state, &policy, 0).is_err());
        status.phase = RecoveryPhase::Applying;
        state.execute_with(|| {
            sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &status.encode());
            sp_io::storage::set(recovery::TRANSIENT_CONTEXT_KEYS[0], b"stale");
        });
        assert!(initialized_checks(&mut state, &policy, 0).is_err());
    }
}
