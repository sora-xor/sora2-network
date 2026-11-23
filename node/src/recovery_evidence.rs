//! Optional passive-node evidence. This never supplies a block, signature or state change.

use codec::{DecodeAll, Encode};
use common::recovery::{RecoveryStatus, CHECKPOINT_HASH, CHECKPOINT_NUMBER, GENESIS_HASH};
use sc_client_api::{AuxStore, ProofProvider, StorageProvider};
use sc_network_sync::strategy::warp::{EncodedProof, VerificationResult, WarpSyncProvider};
use sp_blockchain::HeaderBackend;
use sp_core::{storage::StorageKey, H256};
use sp_runtime::traits::Header as HeaderT;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

const MAX_RECORDS: u64 = 300_000;
const MAX_FILE: usize = 64 * 1024 * 1024;

fn hex(bytes: &[u8]) -> String {
    let mut result = String::from("0x");
    for byte in bytes {
        use std::fmt::Write;
        write!(result, "{byte:02x}").expect("String write");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "sora-nexus-evidence-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn requested_observations_bind_raw_proof_process_and_sequence() {
        let path = directory();
        let recorder = Recorder {
            directory: path.clone(),
            sequence: Mutex::new(0),
        };
        recorder
            .record(
                serde_json::json!({"kind":"warp-accepted","context":"synthetic-test-only"}),
                Some(b"public synthetic proof"),
            )
            .unwrap();
        recorder
            .record(
                serde_json::json!({"kind":"import-result","imported":false}),
                None,
            )
            .unwrap();
        let first: serde_json::Value =
            serde_json::from_slice(&fs::read(path.join("00000000.json")).unwrap()).unwrap();
        assert_eq!(first["pid"], std::process::id());
        assert_eq!(first["sequence"], 0);
        assert_eq!(
            first["proofBlake2"],
            hex(&sp_io::hashing::blake2_256(b"public synthetic proof"))
        );
        assert_eq!(
            fs::read(path.join("00000000.scale")).unwrap(),
            b"public synthetic proof"
        );
        assert!(path.join("00000001.json").is_file());
        assert!(!path.join("00000001.scale").exists());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn journal_never_overwrites_or_silently_wraps_its_bound() {
        let path = directory();
        let recorder = Recorder {
            directory: path.clone(),
            sequence: Mutex::new(0),
        };
        fs::write(path.join("00000000.json"), b"retained prior bytes").unwrap();
        assert!(recorder
            .record(serde_json::json!({"kind":"import-attempt"}), None)
            .is_err());
        assert_eq!(
            fs::read(path.join("00000000.json")).unwrap(),
            b"retained prior bytes"
        );
        *recorder.sequence.lock().unwrap() = MAX_RECORDS;
        assert!(recorder.record(serde_json::json!({}), None).is_err());
        fs::remove_dir_all(path).unwrap();
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_FILE {
        return Err("recovery evidence size bound".into());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}

/// A fresh, process-owned observation journal. No archived journal is resumed.
pub struct Recorder {
    directory: PathBuf,
    sequence: Mutex<u64>,
}

impl Recorder {
    /// Create a journal only for a passive node using the recovery chain spec.
    pub fn create(
        directory: &Path,
        config: &sc_service::Configuration,
    ) -> Result<Arc<Self>, String> {
        if config.role.is_authority()
            || framenode_chain_spec::recovery::config(&*config.chain_spec).is_none()
        {
            return Err("recovery evidence requires a passive recovery node".into());
        }
        fs::create_dir(directory).map_err(|e| e.to_string())?;
        Ok(Arc::new(Self {
            directory: directory.to_path_buf(),
            sequence: Mutex::new(0),
        }))
    }

    /// Record the database only after its recovery startup guard succeeds.
    /// A missing number index is explicit and distinct from a missing header.
    pub fn record_database_admission(
        &self,
        client: &crate::service::FullClient,
    ) -> Result<(), String> {
        let info = client.info();
        let checkpoint_number_index = client.hash(CHECKPOINT_NUMBER).map_err(|e| e.to_string())?;
        let checkpoint_header = client
            .header(H256::from(CHECKPOINT_HASH))
            .map_err(|e| e.to_string())?;
        let checkpoint_import_policy = client
            .get_aux(b"sora-nexus/recovery/checkpoint-imported/v1")
            .map_err(|e| e.to_string())?;
        self.record(
            serde_json::json!({
                "kind": "database-admitted",
                "bestHash": format!("{:#x}", info.best_hash),
                "bestNumber": info.best_number,
                "finalizedHash": format!("{:#x}", info.finalized_hash),
                "finalizedNumber": info.finalized_number,
                "checkpointNumberIndexHash": checkpoint_number_index.map(|hash| format!("{hash:#x}")),
                "checkpointHeaderPresentByHash": checkpoint_header.is_some(),
                "checkpointImportPolicy": checkpoint_import_policy.as_deref().map(hex),
            }),
            None,
        )
    }

    /// Retain an actual observation and optional raw proof with no overwrite.
    pub fn record(&self, mut value: serde_json::Value, proof: Option<&[u8]>) -> Result<(), String> {
        let mut sequence = self
            .sequence
            .lock()
            .map_err(|_| "recovery evidence lock poisoned")?;
        if *sequence >= MAX_RECORDS {
            return Err("recovery evidence record bound".into());
        }
        value["schema"] = "sora-nexus-passive-node-observation-v1".into();
        value["pid"] = std::process::id().into();
        value["sequence"] = (*sequence).into();
        if let Some(proof) = proof {
            let name = format!("{:08}.scale", *sequence);
            write_new(&self.directory.join(&name), proof)?;
            value["proofFile"] = name.into();
            value["proofBlake2"] = hex(&sp_io::hashing::blake2_256(proof)).into();
        }
        write_new(
            &self.directory.join(format!("{:08}.json", *sequence)),
            &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
        )?;
        fs::File::open(&self.directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|e| e.to_string())?;
        *sequence += 1;
        Ok(())
    }
}

/// Inspect only an owned stopped database copy, with no network or signer.
#[derive(Debug, Clone, clap::Parser)]
pub struct VerifyRecoveryDatabaseCmd {
    /// Exact expected best and finalized header hash; they must coincide.
    #[arg(long)]
    pub expected_hash: String,
    /// Fresh output directory for proof bytes and the final report.
    #[arg(long)]
    pub output: PathBuf,
    /// Optional journal from the actual warp process; recheck its accepted proof bytes.
    #[arg(long)]
    pub warp_evidence: Option<PathBuf>,
    /// Required actual process ID for the supplied warp journal.
    #[arg(long, requires = "warp_evidence")]
    pub expected_warp_pid: Option<u32>,
    /// Require the complete canonical recovery header sequence (full/provider lanes).
    #[arg(long)]
    pub complete_history: bool,
    #[allow(missing_docs)]
    #[clap(flatten)]
    pub shared_params: sc_cli::SharedParams,
    #[allow(missing_docs)]
    #[clap(flatten)]
    pub pruning_params: sc_cli::PruningParams,
    #[allow(missing_docs)]
    #[clap(flatten)]
    pub database_params: sc_cli::DatabaseParams,
}

impl sc_cli::CliConfiguration for VerifyRecoveryDatabaseCmd {
    fn shared_params(&self) -> &sc_cli::SharedParams {
        &self.shared_params
    }
    fn pruning_params(&self) -> Option<&sc_cli::PruningParams> {
        Some(&self.pruning_params)
    }
    fn database_params(&self) -> Option<&sc_cli::DatabaseParams> {
        Some(&self.database_params)
    }
}

fn storage_keys() -> Vec<Vec<u8>> {
    use common::recovery::*;
    let mut keys = vec![
        b":code".to_vec(),
        POLICY_STORAGE_KEY.to_vec(),
        STATUS_STORAGE_KEY.to_vec(),
        ACTIVE_POLICY_STORAGE_KEY.to_vec(),
        AUDIT_CURSOR_STORAGE_KEY.to_vec(),
        BRIDGE_HALT_STORAGE_KEY.to_vec(),
        INCOMING_AUDIT_PENDING_KEY.to_vec(),
        TERMINAL_AUDIT_STORAGE_KEY.to_vec(),
    ];
    for (pallet, item) in [
        ("Grandpa", "CurrentSetId"),
        ("Grandpa", "Authorities"),
        ("Session", "CurrentIndex"),
        ("Babe", "EpochIndex"),
        ("Babe", "CurrentSlot"),
        ("Babe", "EpochConfig"),
        ("Babe", "NextEpochConfig"),
        ("Liveness", "DisabledDuringSession"),
    ] {
        let mut key = sp_io::hashing::twox_128(pallet.as_bytes()).to_vec();
        key.extend_from_slice(&sp_io::hashing::twox_128(item.as_bytes()));
        keys.push(key);
    }
    keys
}

impl VerifyRecoveryDatabaseCmd {
    /// Export and independently verify data from the opened stopped copy.
    pub fn run(
        &self,
        client: Arc<crate::service::FullClient>,
        backend: Arc<crate::service::FullBackend>,
        authorities: sc_finality_grandpa::SharedAuthoritySet<H256, u32>,
        spec: &dyn sc_service::ChainSpec,
    ) -> Result<(), String> {
        let recovery = framenode_chain_spec::recovery::config(spec)
            .ok_or("recovery specification required")?
            .clone();
        let expected = H256::from_str(&self.expected_hash).map_err(|e| e.to_string())?;
        crate::recovery::validate_database(&client, &recovery).map_err(|e| e.to_string())?;
        let info = client.info();
        if info.best_hash != expected
            || info.finalized_hash != expected
            || info.finalized_number <= CHECKPOINT_NUMBER
        {
            return Err(
                "exact finalized recovery descendant and coincident best head required".into(),
            );
        }
        let header = client
            .header(expected)
            .map_err(|e| e.to_string())?
            .ok_or("missing finalized header")?;
        if client.hash(*header.number()).map_err(|e| e.to_string())? != Some(expected) {
            return Err("noncanonical target".into());
        }
        let checkpoint = client
            .header(H256::from(CHECKPOINT_HASH))
            .map_err(|e| e.to_string())?
            .ok_or("missing checkpoint")?;
        let checkpoint_number_index = client.hash(CHECKPOINT_NUMBER).map_err(|e| e.to_string())?;
        let checkpoint_import_policy = client
            .get_aux(b"sora-nexus/recovery/checkpoint-imported/v1")
            .map_err(|e| e.to_string())?;
        fs::create_dir(&self.output).map_err(|e| e.to_string())?;
        write_new(&self.output.join("target-header.scale"), &header.encode())?;
        write_new(
            &self.output.join("checkpoint-header.scale"),
            &checkpoint.encode(),
        )?;
        let keys = storage_keys();
        let proof = client
            .read_proof(expected, &mut keys.iter().map(Vec::as_slice))
            .map_err(|e| e.to_string())?;
        let values = sp_state_machine::read_proof_check::<sp_core::Blake2Hasher, _>(
            *header.state_root(),
            proof.clone(),
            &keys,
        )
        .map_err(|e| e.to_string())?;
        write_new(&self.output.join("state-proof.scale"), &proof.encode())?;
        let mut storage = serde_json::Map::new();
        for key in &keys {
            let actual = client
                .storage(expected, &StorageKey(key.clone()))
                .map_err(|e| e.to_string())?
                .map(|v| v.0);
            if values.get(key) != Some(&actual) {
                return Err("persisted storage proof mismatch".into());
            }
            storage.insert(hex(key), match actual { Some(value) if key == b":code" => serde_json::json!({"blake2":hex(&sp_io::hashing::blake2_256(&value)), "bytes":value.len()}),
                Some(value) => serde_json::Value::String(hex(&value)), None => serde_json::Value::Null });
        }
        let status = values
            .get(common::recovery::STATUS_STORAGE_KEY)
            .and_then(Option::as_ref)
            .ok_or("missing recovery status")?;
        let decoded =
            RecoveryStatus::decode_all(&mut status.as_slice()).map_err(|e| e.to_string())?;
        if decoded.policy != recovery.policy_commitment.0 || decoded.format_version != 1 {
            return Err("recovery status lineage mismatch".into());
        }
        let inner = Arc::new(sc_finality_grandpa::warp_proof::NetworkProvider::new(
            backend,
            authorities.clone(),
            crate::recovery::hard_forks(spec),
        ));
        let provider = crate::recovery::RecoveryWarpProvider::new(
            inner,
            client.clone(),
            Some(recovery.clone()),
        );
        let mut verifier = provider.create_verifier();
        let mut context = H256::from(GENESIS_HASH);
        let mut finality = Vec::new();
        loop {
            if finality.len() >= 128 {
                return Err("persisted finality proof count bound".into());
            }
            let raw = provider.generate(context).map_err(|e| e.to_string())?;
            let result = verifier.verify(&raw).map_err(|e| e.to_string())?;
            let name = format!("finality-{:04}.scale", finality.len());
            write_new(&self.output.join(&name), &raw.0)?;
            finality.push(serde_json::json!({"file":name,"context":format!("{context:#x}"),"blake2":hex(&sp_io::hashing::blake2_256(&raw.0))}));
            match result {
                VerificationResult::Complete(final_header, _) => {
                    if final_header.hash() != expected {
                        return Err("persisted finality proof ends at another header".into());
                    }
                    break;
                }
                VerificationResult::Partial(_) => {
                    let next = verifier.next_proof_context();
                    if next == context {
                        return Err("nonadvancing persisted proof".into());
                    }
                    context = next;
                }
            }
        }
        let mut actual_warp = None;
        if let Some(directory) = &self.warp_evidence {
            let pid = self.expected_warp_pid.ok_or("actual warp PID required")?;
            let mut names = fs::read_dir(directory)
                .map_err(|e| e.to_string())?
                .map(|entry| entry.map(|e| e.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            names.sort();
            if names.len() > (MAX_RECORDS * 2) as usize {
                return Err("warp journal entry bound".into());
            }
            let mut observed = provider.create_verifier();
            let mut accepted = 0usize;
            let mut completed = false;
            for path in names
                .into_iter()
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
            {
                let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                if !meta.is_file() || meta.len() > 65536 {
                    return Err("invalid journal record".into());
                }
                let record: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if record["schema"] != "sora-nexus-passive-node-observation-v1"
                    || record["pid"] != pid
                {
                    return Err("warp journal process mismatch".into());
                }
                if record["kind"] != "warp-accepted" {
                    continue;
                }
                if completed {
                    return Err("warp proof after completion".into());
                }
                let name = record["proofFile"].as_str().ok_or("missing proof file")?;
                if name.len() != 14
                    || !name.ends_with(".scale")
                    || !name[..8].bytes().all(|b| b.is_ascii_digit())
                {
                    return Err("invalid proof filename".into());
                }
                let raw_path = directory.join(name);
                let meta = fs::symlink_metadata(&raw_path).map_err(|e| e.to_string())?;
                if !meta.is_file() || meta.len() > 8 * 1024 * 1024 {
                    return Err("invalid proof size/type".into());
                }
                let bytes = fs::read(raw_path).map_err(|e| e.to_string())?;
                if record["proofBlake2"] != hex(&sp_io::hashing::blake2_256(&bytes))
                    || record["context"] != format!("{:#x}", observed.next_proof_context())
                {
                    return Err("recorded warp proof binding mismatch".into());
                }
                match observed
                    .verify(&EncodedProof(bytes.clone()))
                    .map_err(|e| e.to_string())?
                {
                    VerificationResult::Complete(target, _) => {
                        if target.hash() != expected {
                            return Err(
                                "observed warp completed at different persisted target".into()
                            );
                        }
                        completed = true;
                    }
                    VerificationResult::Partial(_) => {}
                }
                write_new(
                    &self
                        .output
                        .join(format!("observed-warp-{accepted:04}.scale")),
                    &bytes,
                )?;
                accepted += 1;
            }
            if !completed || accepted == 0 {
                return Err("no observed complete accepted warp proof".into());
            }
            actual_warp = Some(
                serde_json::json!({"pid":pid,"proofs":accepted,"target":format!("{expected:#x}"),"sdkReverified":true}),
            );
        } else if self.expected_warp_pid.is_some() {
            return Err("warp PID without journal".into());
        }
        let mut auxiliaries = serde_json::Map::new();
        for key in [
            b"grandpa_schema_version".as_slice(),
            b"grandpa_voters",
            b"babe_epoch_changes_version",
            b"babe_epoch_changes",
            b"sora-nexus/recovery/checkpoint-imported/v1",
        ] {
            let value = client
                .get_aux(key)
                .map_err(|e| e.to_string())?
                .ok_or("required consensus auxiliary unavailable")?;
            write_new(&self.output.join(format!("aux-{}.scale", hex(key))), &value)?;
            auxiliaries.insert(
                hex(key),
                serde_json::json!({"blake2":hex(&sp_io::hashing::blake2_256(&value)),"bytes":value.len()}),
            );
        }
        let mut history = Vec::new();
        if self.complete_history {
            if info.finalized_number - CHECKPOINT_NUMBER > 100_000 {
                return Err("recovery history bound".into());
            }
            let mut previous = H256::from(CHECKPOINT_HASH);
            for number in CHECKPOINT_NUMBER + 1..=info.finalized_number {
                let hash = client
                    .hash(number)
                    .map_err(|e| e.to_string())?
                    .ok_or("missing canonical recovery height")?;
                let item = client
                    .header(hash)
                    .map_err(|e| e.to_string())?
                    .ok_or("missing canonical recovery header")?;
                if item.parent_hash() != &previous || *item.number() != number {
                    return Err("recovery header ancestry mismatch".into());
                }
                history.push(serde_json::json!({"number":number,"hash":format!("{hash:#x}"),
                    "parentHash":format!("{previous:#x}"),"stateRoot":format!("{:#x}",item.state_root()),"header":hex(&item.encode())}));
                previous = hash;
            }
            if previous != expected {
                return Err("recovery ancestry does not reach target".into());
            }
        }
        let info_after = client.info();
        if info_after.best_hash != info.best_hash
            || info_after.finalized_hash != info.finalized_hash
            || client.hash(CHECKPOINT_NUMBER).map_err(|e| e.to_string())? != checkpoint_number_index
            || client
                .get_aux(b"sora-nexus/recovery/checkpoint-imported/v1")
                .map_err(|e| e.to_string())?
                != checkpoint_import_policy
        {
            return Err("database changed during verification".into());
        }
        let report = serde_json::json!({"schema":"sora-nexus-persisted-recovery-database-v1", "genesisHash":format!("{:#x}",info.genesis_hash),
            "checkpointHash":format!("{:#x}",H256::from(CHECKPOINT_HASH)),"checkpointParentHash":format!("{:#x}",checkpoint.parent_hash()),"policyCommitment":format!("{:#x}",recovery.policy_commitment),
            "checkpointNumberIndexHash":checkpoint_number_index.map(|hash|format!("{hash:#x}")),
            "checkpointHeaderPresentByHash":true,"checkpointImportPolicy":checkpoint_import_policy.as_deref().map(hex),
            "bestHash":format!("{expected:#x}"),"finalizedHash":format!("{expected:#x}"),"number":info.finalized_number,
            "stateRoot":format!("{:#x}",header.state_root()),"storage":storage,"auxiliaries":auxiliaries,
            "grandpaSetId":authorities.set_id(),"grandpaAuthorities":authorities.current_authorities().iter().map(|(id,v)| (hex(id.as_ref()),v.weight().get())).collect::<Vec<_>>(),
            "recoveryHeaders":history,"phase":format!("{:?}",decoded.phase),"cursor":decoded.next_chunk,"operationCounter":decoded.operation_counter,
            "persistedFinality":finality,"storageProofVerified":true,"finalityProofVerified":true,"observedWarp":actual_warp,
            "operatorApprovalVerified":false,"productionReady":false});
        write_new(
            &self.output.join("report.json"),
            &serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )?;
        println!(
            "{}",
            serde_json::to_string(&report).map_err(|e| e.to_string())?
        );
        Ok(())
    }
}
