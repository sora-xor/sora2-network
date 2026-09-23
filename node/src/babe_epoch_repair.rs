// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! Deliberate, offline recovery of the historical mainnet BABE configuration mismatch.
//!
//! This never changes block verification. The local finalized canonical chain is the trust
//! anchor; runtime state supplies epoch membership, authorities and randomness, while finalized
//! headers supply the slot mode and any announced change. Only the known VRF-to-Plain mismatch
//! in schema 3 is repairable. Uncertain or unrelated cache damage requires separate diagnosis.

use codec::{DecodeAll, Encode};
use framenode_runtime::{opaque::Block, RuntimeApi};
use sc_client_api::{AuxStore, HeaderBackend};
use sc_consensus_babe::Epoch as CachedEpoch;
use sc_consensus_epochs::{
    EpochChangesFor, IsDescendentOfBuilder, PersistedEpochHeader, ViableEpochDescriptor,
};
use sp_api::ProvideRuntimeApi;
use sp_consensus_babe::{
    digests::PreDigest, AllowedSlots, BabeApi, BabeEpochConfiguration, ConsensusLog, Epoch,
    BABE_ENGINE_ID,
};
use sp_core::H256;
use sp_runtime::{traits::Header as HeaderT, DigestItem};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
};

type Header = <Block as sp_runtime::traits::Block>::Header;
type EpochChanges = EpochChangesFor<Block, CachedEpoch>;
type RepairResult<T> = Result<T, String>;

// Private SDK key names, deliberately restricted to the pinned SDK's exact schema 3 layout.
const VERSION_KEY: &[u8] = b"babe_epoch_changes_version";
const CACHE_KEY: &[u8] = b"babe_epoch_changes";
const MAINNET_GENESIS: &str = "7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5";
const EPOCH_DURATION: u64 = 600;

/// Inspect an existing mainnet database without starting networking or consensus.
#[derive(Debug, Clone, clap::Parser)]
#[command(group(clap::ArgGroup::new("explicit_database").args(["database"]).required(true)))]
pub struct RepairBabeEpochCacheCmd {
    /// Commit the verified repair. Without this flag the command only diagnoses the cache.
    #[arg(long, requires = "backup")]
    apply: bool,
    /// New backup file for the original cache and its evidence. Existing files are never replaced.
    #[arg(long, requires = "apply")]
    backup: Option<PathBuf>,
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

impl sc_cli::CliConfiguration for RepairBabeEpochCacheCmd {
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

impl RepairBabeEpochCacheCmd {
    pub fn run(&self, mut config: sc_service::Configuration) -> RepairResult<()> {
        self.run_inner(&mut config)
    }

    fn run_inner(&self, config: &mut sc_service::Configuration) -> RepairResult<()> {
        require_existing_database(&config.database)?;
        use_on_chain_runtime(config)?;
        // The standard SDK opener can migrate metadata or remove block-gap records even during
        // diagnosis. Open only the exact existing engine layout and deny every logical write until
        // the durable backup has authorized one specific cache replacement.
        let database = Arc::new(GuardedDatabase::open(&config.database)?);
        let genesis = H256::from_str(MAINNET_GENESIS).expect("valid constant genesis");
        if sp_database::Database::get(&*database, 0, b"gen") != Some(genesis.encode()) {
            return Err("Existing database is not initialized SORA mainnet".into());
        }
        if sp_database::Database::get(&*database, 0, b"fstate").is_none() {
            return Err(
                "Existing finalized state is required; genesis initialization is forbidden".into(),
            );
        }
        config.database = sc_client_db::DatabaseSource::Custom {
            db: database.clone(),
            require_create_flag: false,
        };
        // Do not call service::new_partial: that also opens the bridge keystore and starts
        // consensus import tasks. The backend's exclusive database lock requires the node stopped.
        config.keystore = sc_service::config::KeystoreConfig::InMemory;
        type HostFunctions = (
            sp_io::SubstrateHostFunctions,
            frame_benchmarking::benchmarking::HostFunctions,
        );
        let executor = sc_service::new_wasm_executor::<HostFunctions>(&config.executor);
        let backend =
            sc_service::new_db_backend::<Block>(config.db_config()).map_err(|e| e.to_string())?;
        let (client, _backend, _keystore, _task_manager) =
            sc_service::new_full_parts_with_genesis_builder::<Block, RuntimeApi, _, _>(
                config,
                None,
                executor,
                backend,
                RefuseGenesis,
                false,
            )
            .map_err(|e| e.to_string())?;
        let snapshot = Snapshot::from_info(client.info());
        if snapshot.genesis_hash != H256::from_str(MAINNET_GENESIS).expect("valid constant genesis")
        {
            return Err("This command is restricted to the SORA mainnet genesis".into());
        }
        let api = client.runtime_api();
        let current = api
            .current_epoch(snapshot.finalized_hash)
            .map_err(|e| format!("Finalized current_epoch state unavailable: {e}"))?;
        let next = api
            .next_epoch(snapshot.finalized_hash)
            .map_err(|e| format!("Finalized next_epoch state unavailable: {e}"))?;
        let evidence = collect_evidence(
            snapshot,
            current,
            next,
            |hash| client.header(hash).map_err(|e| e.to_string()),
            |number| client.hash(number).map_err(|e| e.to_string()),
        )?;
        let (version, original) = read_cache(&client)?;
        let plan = plan_repair(&original, &evidence, |number| {
            client.hash(number).map_err(|e| e.to_string())
        })?;
        let mut report = plan.report(&evidence);
        report["mode"] = if self.apply { "apply" } else { "diagnose" }.into();
        report["applied"] = false.into();
        if self.apply && plan.changed > 0 {
            if Snapshot::from_info(client.info()) != snapshot {
                return Err("Chain head changed during inspection; refusing repair".into());
            }
            let backup = self.backup.as_deref().ok_or("--apply requires --backup")?;
            let receipt = serde_json::json!({
                "format": "sora-babe-epoch-cache-backup-v1",
                "report": report,
                "original_version_scale": hex(&version),
                "original_cache_scale": hex(&original),
                "replacement_cache_scale": hex(&plan.replacement),
                "runtime_current_epoch_scale": hex(&evidence.current.encode()),
                "runtime_next_epoch_scale": hex(&evidence.next.encode()),
                "canonical_headers_scale": evidence.headers.iter().map(|h| hex(&h.encode())).collect::<Vec<_>>(),
            });
            apply_repair(
                &client,
                &version,
                &original,
                &plan.replacement,
                backup,
                &serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?,
                || database.authorize_cache_write(plan.replacement.clone()),
            )?;
            report["applied"] = true.into();
            report["backup"] = backup.display().to_string().into();
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        Ok(())
    }
}

/// Mainnet's ordinary chain spec contains historical runtime substitutes. Evidence must execute
/// the finalized state's actual :code, so remove substitutes only from this offline client's
/// in-memory copy. Normal node startup and the operator's chain-spec file are unchanged.
fn use_on_chain_runtime(config: &mut sc_service::Configuration) -> RepairResult<()> {
    if config.wasm_runtime_overrides.is_some() {
        return Err("Runtime overrides cannot establish canonical epoch evidence".into());
    }
    if !config.chain_spec.code_substitutes().is_empty() {
        let mut spec: serde_json::Value = serde_json::from_str(&config.chain_spec.as_json(false)?)
            .map_err(|e| format!("Cannot copy chain spec for on-chain runtime evidence: {e}"))?;
        spec.as_object_mut()
            .ok_or("Chain spec is not a JSON object")?
            .insert("codeSubstitutes".into(), serde_json::json!({}));
        config.chain_spec = Box::new(framenode_chain_spec::ChainSpec::from_json_bytes(
            serde_json::to_vec(&spec).map_err(|e| e.to_string())?,
        )?);
    }
    Ok(())
}

struct RefuseGenesis;

impl sc_service::BuildGenesisBlock<Block> for RefuseGenesis {
    type BlockImportOperation =
        <sc_client_db::Backend<Block> as sc_client_api::Backend<Block>>::BlockImportOperation;

    fn build_genesis_block(self) -> sp_blockchain::Result<(Block, Self::BlockImportOperation)> {
        Err(sp_blockchain::Error::Backend(
            "Repair must never initialize or regenerate genesis".into(),
        ))
    }
}

enum ExistingDatabase {
    Parity(parity_db::Db),
    Rocks(Arc<dyn sp_database::Database<H256>>),
}

/// This incident recovery is pinned to SDK e373717: 13 columns, AUX column 8.
/// No SDK initialization, pruning, genesis or offchain write may pass this guard.
struct GuardedDatabase {
    inner: ExistingDatabase,
    permitted_cache: Mutex<Option<Vec<u8>>>,
}

impl GuardedDatabase {
    fn open(source: &sc_client_db::DatabaseSource) -> RepairResult<Self> {
        require_existing_database(source)?;
        let inner = match source {
            sc_client_db::DatabaseSource::ParityDb { path } => {
                let options = parity_options(path);
                let metadata = options
                    .load_and_validate_metadata(false)
                    .map_err(|e| e.to_string())?;
                if metadata.version != 8 {
                    return Err("Only existing ParityDB format 8 is supported".into());
                }
                // Unlike the SDK opener, never create a database or rewrite incompatible metadata.
                ExistingDatabase::Parity(parity_db::Db::open(&options).map_err(|e| e.to_string())?)
            }
            sc_client_db::DatabaseSource::RocksDb { path, .. } => {
                // kvdb-rocksdb can otherwise create missing column families even with
                // create_if_missing=false. Reject any layout other than the pinned SDK's.
                let mut actual = rocksdb::DB::list_cf(&rocksdb::Options::default(), path)
                    .map_err(|e| e.to_string())?;
                let mut expected = vec!["default".to_string()];
                expected.extend((0..13).map(|column| format!("col{column}")));
                actual.sort();
                expected.sort();
                if actual != expected {
                    return Err("RocksDB column families do not match the pinned SDK layout".into());
                }
                let mut options = kvdb_rocksdb::DatabaseConfig::with_columns(13);
                options.create_if_missing = false;
                let db = kvdb_rocksdb::Database::open(&options, path).map_err(|e| e.to_string())?;
                ExistingDatabase::Rocks(sp_database::as_rocksdb_database(db))
            }
            _ => return Err("Explicit existing ParityDB or RocksDB backend required".into()),
        };
        Ok(Self {
            inner,
            permitted_cache: Mutex::new(None),
        })
    }

    fn authorize_cache_write(&self, replacement: Vec<u8>) -> RepairResult<()> {
        let mut permitted = self.permitted_cache.lock().map_err(|e| e.to_string())?;
        if permitted.is_some() {
            return Err("A cache replacement is already authorized".into());
        }
        *permitted = Some(replacement);
        Ok(())
    }
}

fn parity_options(path: &Path) -> parity_db::Options {
    let mut options = parity_db::Options::with_columns(path, 13);
    for col in [1, 4, 5, 12, 11, 6] {
        options.columns[col].compression = parity_db::CompressionType::Lz4;
    }
    for col in [1, 11] {
        options.columns[col].ref_counted = true;
        options.columns[col].preimage = true;
        options.columns[col].uniform = true;
    }
    options
}

impl sp_database::Database<H256> for GuardedDatabase {
    fn commit(
        &self,
        transaction: sp_database::Transaction<H256>,
    ) -> sp_database::error::Result<()> {
        if transaction.0.is_empty() {
            return Ok(());
        }
        let refusal = || {
            sp_database::error::DatabaseError(Box::new(std::io::Error::other(
            "Database initialization or unrelated write refused; cache repair requires a durable backup",
        )))
        };
        let [sp_database::Change::Set(8, key, value)] = &transaction.0[..] else {
            return Err(refusal());
        };
        let mut permitted = self.permitted_cache.lock().map_err(|_| refusal())?;
        if key != CACHE_KEY || permitted.as_ref() != Some(value) {
            return Err(refusal());
        }
        // Consume the permission before writing, including on failure. Retrying requires a new run.
        *permitted = None;
        match &self.inner {
            ExistingDatabase::Parity(db) => db
                .commit([(8, key, Some(value.clone()))])
                .map_err(|e| sp_database::error::DatabaseError(Box::new(e))),
            ExistingDatabase::Rocks(db) => db.commit(transaction),
        }
    }

    fn get(&self, column: u32, key: &[u8]) -> Option<Vec<u8>> {
        match &self.inner {
            ExistingDatabase::Parity(db) => db
                .get(column as u8, key)
                .expect("Existing database read failed; repair cannot continue"),
            ExistingDatabase::Rocks(db) => db.get(column, key),
        }
    }

    fn supports_ref_counting(&self) -> bool {
        matches!(self.inner, ExistingDatabase::Parity(_))
    }

    fn sanitize_key(&self, key: &mut Vec<u8>) {
        if matches!(self.inner, ExistingDatabase::Parity(_)) {
            key.drain(..key.len() - 32);
        }
    }
}

fn require_existing_database(source: &sc_client_db::DatabaseSource) -> RepairResult<()> {
    use sc_client_db::DatabaseSource;
    let path = match source {
        DatabaseSource::ParityDb { path } => {
            if !path.join("metadata").is_file() {
                return Err("Existing ParityDB metadata is required; refusing to initialize a database".into());
            }
            path
        }
        DatabaseSource::RocksDb { path, .. } => {
            // SDK e373717's RocksDB schema is 4. Do not run its automatic older-schema migration.
            if !path.join("CURRENT").is_file()
                || std::fs::read(path.join("db_version")).map_err(|e| e.to_string())? != b"4"
            {
                return Err("An existing RocksDB database at SDK schema 4 is required".into());
            }
            path
        }
        _ => return Err("Select the existing backend explicitly with --database paritydb or --database rocksdb; auto/custom databases are not supported".into()),
    };
    if let Some(parent) = path.parent() {
        if path.ends_with("full")
            && (parent.join("metadata").exists() || parent.join("db_version").exists())
        {
            return Err(
                "Legacy database directory migration would be required; refusing to move files"
                    .into(),
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Snapshot {
    genesis_hash: H256,
    finalized_hash: H256,
    finalized_number: u32,
    best_hash: H256,
    best_number: u32,
}

impl Snapshot {
    fn from_info(info: sp_blockchain::Info<Block>) -> Self {
        Self {
            genesis_hash: info.genesis_hash,
            finalized_hash: info.finalized_hash,
            finalized_number: info.finalized_number,
            best_hash: info.best_hash,
            best_number: info.best_number,
        }
    }
}

struct Evidence {
    snapshot: Snapshot,
    current: Epoch,
    next: Epoch,
    next_config: BabeEpochConfiguration,
    first_hash: H256,
    plain_proof_hash: H256,
    // Best to first epoch header, then its predecessor, all bound to the canonical chain.
    headers: Vec<Header>,
}

fn known_config(config: &BabeEpochConfiguration) -> RepairResult<()> {
    if config.c != (1, 4)
        || !matches!(
            config.allowed_slots,
            AllowedSlots::PrimaryAndSecondaryPlainSlots | AllowedSlots::PrimaryAndSecondaryVRFSlots
        )
    {
        return Err("Configuration is outside the known c=1/4 Plain/VRF incident".into());
    }
    Ok(())
}

fn plain_config() -> BabeEpochConfiguration {
    BabeEpochConfiguration {
        c: (1, 4),
        allowed_slots: AllowedSlots::PrimaryAndSecondaryPlainSlots,
    }
}

fn pre_digest(header: &Header) -> RepairResult<PreDigest> {
    let mut found = None;
    for item in header.digest().logs() {
        if let DigestItem::PreRuntime(engine, bytes) = item {
            if engine == &BABE_ENGINE_ID {
                let digest = PreDigest::decode_all(&mut &bytes[..])
                    .map_err(|e| format!("Malformed BABE pre-digest: {e}"))?;
                if found.replace(digest).is_some() {
                    return Err("Multiple BABE pre-digests".into());
                }
            }
        }
    }
    found.ok_or_else(|| "Missing BABE pre-digest".into())
}

fn consensus_logs(header: &Header) -> RepairResult<Vec<ConsensusLog>> {
    header
        .digest()
        .logs()
        .iter()
        .filter_map(|item| match item {
            DigestItem::Consensus(engine, bytes) if engine == &BABE_ENGINE_ID => Some(
                ConsensusLog::decode_all(&mut &bytes[..])
                    .map_err(|e| format!("Malformed BABE consensus digest: {e}")),
            ),
            _ => None,
        })
        .collect()
}

fn collect_evidence(
    snapshot: Snapshot,
    current: Epoch,
    next: Epoch,
    mut header: impl FnMut(H256) -> RepairResult<Option<Header>>,
    mut canonical: impl FnMut(u32) -> RepairResult<Option<H256>>,
) -> RepairResult<Evidence> {
    let genesis = H256::from_str(MAINNET_GENESIS).expect("constant genesis hash is valid");
    if snapshot.genesis_hash != genesis || canonical(0)? != Some(genesis) {
        return Err("This command is restricted to the SORA mainnet genesis".into());
    }
    if snapshot.finalized_number == 0
        || snapshot.best_number < snapshot.finalized_number
        || canonical(snapshot.finalized_number)? != Some(snapshot.finalized_hash)
        || canonical(snapshot.best_number)? != Some(snapshot.best_hash)
    {
        return Err("Finalized/best heads are not a consistent canonical chain".into());
    }
    known_config(&current.config)?;
    known_config(&next.config)?;
    let start = u64::from(current.start_slot);
    let end = start
        .checked_add(current.duration)
        .ok_or("Epoch end overflow")?;
    if current.epoch_index == 0
        || current.duration != EPOCH_DURATION
        || next.duration != current.duration
        || u64::from(next.start_slot) != end
        || current.epoch_index.checked_add(1) != Some(next.epoch_index)
        || current.authorities.is_empty()
        || next.authorities.is_empty()
    {
        return Err("Finalized runtime epoch identities are inconsistent or unsupported".into());
    }
    let mut headers = Vec::new();
    let mut hash = snapshot.best_hash;
    let mut number = snapshot.best_number;
    let mut child_slot = None;
    let mut saw_finalized = false;
    let mut plain_proof_hash = None;
    loop {
        if headers.len() > EPOCH_DURATION as usize {
            return Err("Epoch header scan exceeded one epoch; refusing ambiguous history".into());
        }
        let h = header(hash)?.ok_or("Required canonical header is unavailable/pruned")?;
        if h.hash() != hash || *h.number() != number || canonical(number)? != Some(hash) {
            return Err("Header is not the expected canonical ancestor".into());
        }
        if number == snapshot.finalized_number {
            if hash != snapshot.finalized_hash {
                return Err("Best chain does not include the trusted finalized head".into());
            }
            saw_finalized = true;
        }
        let digest = pre_digest(&h)?;
        let slot = u64::from(digest.slot());
        if child_slot.is_some_and(|child| slot >= child) {
            return Err("Canonical BABE slots are not strictly increasing".into());
        }
        if slot >= end || (slot < start && headers.is_empty()) {
            return Err("Best and finalized state are not in the same current epoch".into());
        }
        // Decode every BABE log even when no transition is expected. Partial decoding is unsafe.
        consensus_logs(&h)?;
        if slot < start {
            headers.push(h);
            break;
        }
        if matches!(digest, PreDigest::SecondaryVRF(_)) {
            return Err(
                "Current canonical epoch contains VRF slots; no Plain-only repair is justified"
                    .into(),
            );
        }
        if number <= snapshot.finalized_number && matches!(digest, PreDigest::SecondaryPlain(_)) {
            plain_proof_hash = Some(hash);
        }
        child_slot = Some(slot);
        hash = *h.parent_hash();
        number = number
            .checked_sub(1)
            .ok_or("Epoch boundary precedes genesis")?;
        headers.push(h);
    }
    if !saw_finalized {
        return Err("Finalized head is outside the epoch proved by the headers".into());
    }
    let plain_proof_hash = plain_proof_hash
        .ok_or("No finalized SecondaryPlain header proves the current slot mode")?;
    let first = &headers[headers.len() - 2];
    let first_hash = first.hash();
    let mut announcement = None;
    let mut next_config = None;
    for (index, h) in headers[..headers.len() - 1].iter().enumerate() {
        for log in consensus_logs(h)? {
            match log {
                ConsensusLog::NextEpochData(data) => {
                    if index != headers.len() - 2 || announcement.replace(data).is_some() {
                        return Err("Missing, duplicate or misplaced epoch transition data".into());
                    }
                }
                ConsensusLog::NextConfigData(config) => {
                    if index != headers.len() - 2 || next_config.replace(config.into()).is_some() {
                        return Err("Duplicate or misplaced configuration announcement".into());
                    }
                }
                ConsensusLog::OnDisabled(_) => {}
            }
        }
    }
    let announcement = announcement.ok_or("First epoch header has no NextEpochData")?;
    if announcement.authorities != next.authorities || announcement.randomness != next.randomness {
        return Err(
            "Finalized NextEpochData does not match runtime next authorities/randomness".into(),
        );
    }
    let next_config = next_config.unwrap_or_else(plain_config);
    known_config(&next_config)?;
    Ok(Evidence {
        snapshot,
        current,
        next,
        next_config,
        first_hash,
        plain_proof_hash,
        headers,
    })
}

struct RepairPlan {
    replacement: Vec<u8>,
    changed: usize,
    matched: usize,
}

impl RepairPlan {
    fn report(&self, evidence: &Evidence) -> serde_json::Value {
        serde_json::json!({
            "genesis": format!("{:#x}", evidence.snapshot.genesis_hash),
            "finalized_hash": format!("{:#x}", evidence.snapshot.finalized_hash),
            "finalized_number": evidence.snapshot.finalized_number,
            "best_hash": format!("{:#x}", evidence.snapshot.best_hash),
            "best_number": evidence.snapshot.best_number,
            "current_epoch": evidence.current.epoch_index,
            "current_epoch_first_header": format!("{:#x}", evidence.first_hash),
            "finalized_plain_proof": format!("{:#x}", evidence.plain_proof_hash),
            "current_config": format!("{:?}", plain_config()),
            "next_config": format!("{:?}", evidence.next_config),
            "schema": 3,
            "matched_entries": self.matched,
            "changed_entries": self.changed,
            "replacement_blake2_256": hex(&sp_core::hashing::blake2_256(&self.replacement)),
        })
    }
}

fn read_cache(store: &impl AuxStore) -> RepairResult<(Vec<u8>, Vec<u8>)> {
    let version = store
        .get_aux(VERSION_KEY)
        .map_err(|e| e.to_string())?
        .ok_or("Missing BABE cache schema version")?;
    if u32::decode_all(&mut &version[..]).map_err(|e| e.to_string())? != 3 {
        return Err("Only the pinned SDK BABE epoch cache schema 3 is supported".into());
    }
    let bytes = store
        .get_aux(CACHE_KEY)
        .map_err(|e| e.to_string())?
        .ok_or("Missing BABE epoch cache")?;
    Ok((version, bytes))
}

fn plan_repair(
    original: &[u8],
    evidence: &Evidence,
    mut canonical: impl FnMut(u32) -> RepairResult<Option<H256>>,
) -> RepairResult<RepairPlan> {
    let cache = EpochChanges::decode_all(&mut &original[..])
        .map_err(|e| format!("Invalid epoch cache: {e}"))?;
    let tree_before = cache.tree().encode();
    let mut ranges: BTreeMap<(H256, u32), Vec<(u64, u64)>> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut matched = [0usize; 2];
    let mut changed = 0;
    let mut canonical_numbers =
        BTreeMap::from([(evidence.snapshot.best_hash, evidence.snapshot.best_number)]);
    let replacement = cache.map(|hash, number, mut epoch| {
        let start = u64::from(epoch.start_slot);
        let Some(end) = start.checked_add(epoch.duration) else {
            errors.push("Cached epoch end overflows".to_string());
            return epoch;
        };
        ranges
            .entry((*hash, *number))
            .or_default()
            .push((start, end));
        let is_canonical = match canonical(*number) {
            Ok(Some(canonical_hash)) if *number <= evidence.snapshot.best_number => {
                if canonical_hash == *hash {
                    canonical_numbers.insert(*hash, *number);
                    true
                } else {
                    false
                }
            }
            _ => {
                errors.push("Cache anchor canonical ancestry is unavailable".into());
                false
            }
        };
        let (position, expected, config) = if epoch.epoch_index == evidence.current.epoch_index {
            (0, &evidence.current, plain_config())
        } else if epoch.epoch_index == evidence.next.epoch_index {
            (1, &evidence.next, evidence.next_config.clone())
        } else {
            // Older entries are retained byte-for-byte. Unknown live/future entries could select a
            // different epoch during import, so an operator must investigate those separately.
            if epoch.epoch_index > evidence.next.epoch_index
                || end > u64::from(evidence.current.start_slot)
            {
                errors.push("Ambiguous live/future epoch cache entry".into());
            }
            return epoch;
        };
        if *number > evidence.snapshot.finalized_number {
            errors.push("Epoch cache entry is anchored after finality".into());
            return epoch;
        }
        if !is_canonical {
            errors.push("Epoch cache entry is on an unknown/noncanonical fork".into());
            return epoch;
        }
        let mut normalized = (*epoch).clone();
        normalized.config = expected.config.clone();
        if &normalized != expected {
            errors.push(
                "Cached epoch membership/authorities/randomness differs from finalized state"
                    .into(),
            );
            return epoch;
        }
        if let Err(error) = known_config(&epoch.config) {
            errors.push(error);
            return epoch;
        }
        matched[position] += 1;
        if epoch.config != config {
            if epoch.config.allowed_slots != AllowedSlots::PrimaryAndSecondaryVRFSlots
                || config != plain_config()
            {
                errors.push("Mismatch is not the known VRF-to-Plain recovery".into());
                return epoch;
            }
            epoch.config = config;
            changed += 1;
        }
        epoch
    });
    // Validate the public tree headers against all mapped payloads before preserving the tree.
    for (hash, number, header) in replacement.tree().iter() {
        let header_ranges = match header {
            PersistedEpochHeader::Regular(h) => {
                vec![(u64::from(h.start_slot), u64::from(h.end_slot))]
            }
            PersistedEpochHeader::Genesis(a, b) => vec![
                (u64::from(a.start_slot), u64::from(a.end_slot)),
                (u64::from(b.start_slot), u64::from(b.end_slot)),
            ],
        };
        if ranges.remove(&(*hash, *number)) != Some(header_ranges) {
            errors.push("Epoch tree metadata does not match its payload".into());
        }
    }
    if !ranges.is_empty() || replacement.tree().encode() != tree_before {
        errors.push("Ambiguous/orphaned epoch tree metadata".into());
    }
    if matched != [1, 1] {
        errors.push(format!(
            "Expected exactly one canonical current and next epoch, found {matched:?}"
        ));
    }
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    verify_selected_epochs(&replacement, evidence, canonical_numbers)?;
    let replacement = if changed == 0 {
        original.to_vec()
    } else {
        replacement.encode()
    };
    Ok(RepairPlan {
        replacement,
        changed,
        matched: matched.iter().sum(),
    })
}

// All included hashes were independently checked against canonical(number). Noncanonical cache
// anchors cannot be ancestors of best. This also lets the SDK resolve its synthetic child hash.
#[derive(Clone)]
struct CanonicalAncestry(BTreeMap<H256, u32>);

impl IsDescendentOfBuilder<H256> for CanonicalAncestry {
    type Error = std::io::Error;
    type IsDescendentOf = Box<dyn Fn(&H256, &H256) -> Result<bool, Self::Error>>;

    fn build_is_descendent_of(&self, current: Option<(H256, H256)>) -> Self::IsDescendentOf {
        let numbers = self.0.clone();
        Box::new(move |base, head| {
            let mut head = *head;
            if let Some((current_hash, parent_hash)) = current {
                if head == current_hash {
                    if *base == parent_hash {
                        return Ok(true);
                    }
                    head = parent_hash;
                }
            }
            Ok(match (numbers.get(base), numbers.get(&head)) {
                (Some(base_number), Some(head_number)) => base_number < head_number,
                _ => false,
            })
        })
    }
}

fn verify_selected_epochs(
    cache: &EpochChanges,
    evidence: &Evidence,
    numbers: BTreeMap<H256, u32>,
) -> RepairResult<()> {
    let ancestry = CanonicalAncestry(numbers);
    let best_slot = pre_digest(&evidence.headers[0])?.slot();
    for (slot, mut expected) in [
        (best_slot, evidence.current.clone()),
        (evidence.next.start_slot, evidence.next.clone()),
    ] {
        expected.config = if expected.epoch_index == evidence.current.epoch_index {
            plain_config()
        } else {
            evidence.next_config.clone()
        };
        let descriptor = cache
            .epoch_descriptor_for_child_of(
                ancestry.clone(),
                &evidence.snapshot.best_hash,
                evidence.snapshot.best_number,
                slot,
            )
            .map_err(|e| format!("Cache epoch selection failed: {e}"))?;
        let selected = match descriptor {
            Some(ViableEpochDescriptor::Signaled(id, _)) => cache.epoch(&id),
            _ => None,
        };
        if selected.map(|epoch| &**epoch) != Some(&expected) {
            return Err(
                "Cache tree selects an unexpected current/next epoch; refusing ambiguous metadata"
                    .into(),
            );
        }
    }
    Ok(())
}

fn durable_backup(path: &Path, bytes: &[u8]) -> RepairResult<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Cannot create new backup: {e}"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("Cannot durably write backup; cache unchanged: {e}"))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|e| format!("Cannot sync backup directory; cache unchanged: {e}"))?;
    Ok(())
}

fn apply_repair(
    store: &impl AuxStore,
    version: &[u8],
    original: &[u8],
    replacement: &[u8],
    backup: &Path,
    receipt: &[u8],
    authorize_write: impl FnOnce() -> RepairResult<()>,
) -> RepairResult<()> {
    let unchanged = || -> RepairResult<()> {
        if read_cache(store)? != (version.to_vec(), original.to_vec()) {
            return Err("Epoch cache changed during inspection; refusing repair".into());
        }
        Ok(())
    };
    unchanged()?;
    durable_backup(backup, receipt)?;
    unchanged()?;
    authorize_write()?;
    // AuxStore commits this insertion in one database transaction. No chain data, weights,
    // keystore entries, tree topology, or schema version are changed.
    store
        .insert_aux(&[(CACHE_KEY, replacement)], &[])
        .map_err(|e| {
            format!(
                "Auxiliary write failed; retain backup {}: {e}",
                backup.display()
            )
        })?;
    if store
        .get_aux(CACHE_KEY)
        .map_err(|e| e.to_string())?
        .as_deref()
        != Some(replacement)
    {
        return Err(format!(
            "Cache read-back did not match; retain backup {} and keep node stopped",
            backup.display()
        ));
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(2 + bytes.len() * 2);
    result.push_str("0x");
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 15) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_consensus_babe::digests::{
        CompatibleDigestItem, NextConfigDescriptor, NextEpochDescriptor, SecondaryPlainPreDigest,
    };
    use std::{
        cell::RefCell,
        sync::atomic::{AtomicU64, Ordering},
    };

    fn vrf_config() -> BabeEpochConfiguration {
        BabeEpochConfiguration {
            c: (1, 4),
            allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
        }
    }

    fn runtime_epoch(index: u64) -> Epoch {
        Epoch {
            epoch_index: index,
            start_slot: (index * EPOCH_DURATION).into(),
            duration: EPOCH_DURATION,
            authorities: vec![(sp_core::sr25519::Public::from_raw([1; 32]).into(), 1)],
            randomness: [index as u8; 32],
            config: vrf_config(),
        }
    }

    fn announcement(next: &Epoch) -> DigestItem {
        DigestItem::Consensus(
            BABE_ENGINE_ID,
            ConsensusLog::NextEpochData(NextEpochDescriptor {
                authorities: next.authorities.clone(),
                randomness: next.randomness,
            })
            .encode(),
        )
    }

    struct Fixture {
        snapshot: Snapshot,
        current: Epoch,
        next: Epoch,
        headers: BTreeMap<H256, Header>,
        canonical: BTreeMap<u32, H256>,
    }

    impl Fixture {
        fn new(first_logs: Vec<DigestItem>) -> Self {
            let current = runtime_epoch(10);
            let next = runtime_epoch(11);
            let genesis_hash = H256::from_str(MAINNET_GENESIS).unwrap();
            let mut canonical = BTreeMap::from([(0, genesis_hash)]);
            let mut headers = BTreeMap::new();
            let mut parent = H256::repeat_byte(99);
            for (number, slot) in [(100, 5999), (101, 6000), (102, 6001), (103, 6002)] {
                let mut logs = vec![DigestItem::babe_pre_digest(PreDigest::SecondaryPlain(
                    SecondaryPlainPreDigest {
                        authority_index: 0,
                        slot: slot.into(),
                    },
                ))];
                if number == 101 {
                    logs.extend(first_logs.clone());
                }
                let h = Header::new(
                    number,
                    H256::zero(),
                    H256::zero(),
                    parent,
                    sp_runtime::Digest { logs },
                );
                parent = h.hash();
                canonical.insert(number, parent);
                headers.insert(parent, h);
            }
            let snapshot = Snapshot {
                genesis_hash,
                finalized_hash: canonical[&102],
                finalized_number: 102,
                best_hash: canonical[&103],
                best_number: 103,
            };
            Self {
                snapshot,
                current,
                next,
                headers,
                canonical,
            }
        }

        fn normal() -> Self {
            Self::new(vec![announcement(&runtime_epoch(11))])
        }

        fn evidence(&self) -> RepairResult<Evidence> {
            collect_evidence(
                self.snapshot,
                self.current.clone(),
                self.next.clone(),
                |hash| Ok(self.headers.get(&hash).cloned()),
                |number| Ok(self.canonical.get(&number).copied()),
            )
        }

        fn cache(&self) -> EpochChanges {
            // This is exactly the layout produced by the SDK's state-sync reset at finality.
            let mut cache = EpochChanges::new();
            cache.reset(
                self.canonical[&101],
                self.canonical[&102],
                102,
                self.current.clone().into(),
                self.next.clone().into(),
            );
            cache
        }

        fn plan(&self, bytes: &[u8]) -> RepairResult<RepairPlan> {
            plan_repair(bytes, &self.evidence()?, |number| {
                Ok(self.canonical.get(&number).copied())
            })
        }
    }

    #[test]
    fn repairs_only_slot_mode_and_preserves_epoch_payload_and_tree() {
        let f = Fixture::normal();
        let cache = f.cache();
        let plan = f.plan(&cache.encode()).unwrap();
        assert_eq!(plan.changed, 2);
        let repaired = EpochChanges::decode_all(&mut &plan.replacement[..]).unwrap();
        assert_eq!(repaired.tree().encode(), cache.tree().encode());
        // Restoring only the two config values recovers the original bytes exactly.
        let restored = repaired.map(|_, _, mut epoch| {
            epoch.config = vrf_config();
            epoch
        });
        assert_eq!(restored.encode(), cache.encode());
        assert_eq!(f.plan(&plan.replacement).unwrap().changed, 0);
        assert_eq!(
            f.plan(&plan.replacement).unwrap().replacement,
            plan.replacement
        );
    }

    #[test]
    fn honors_a_legitimate_announced_vrf_transition() {
        let log = ConsensusLog::NextConfigData(NextConfigDescriptor::V1 {
            c: (1, 4),
            allowed_slots: AllowedSlots::PrimaryAndSecondaryVRFSlots,
        });
        let f = Fixture::new(vec![
            announcement(&runtime_epoch(11)),
            DigestItem::Consensus(BABE_ENGINE_ID, log.encode()),
        ]);
        let plan = f.plan(&f.cache().encode()).unwrap();
        assert_eq!(plan.changed, 1);
        let repaired = EpochChanges::decode_all(&mut &plan.replacement[..]).unwrap();
        repaired.map(|_, _, epoch| {
            assert_eq!(
                epoch.config,
                if epoch.epoch_index == 10 {
                    plain_config()
                } else {
                    vrf_config()
                }
            );
            epoch
        });
    }

    #[test]
    fn refuses_untrusted_finalized_head_wrong_genesis_and_cross_epoch_best() {
        let mut f = Fixture::normal();
        f.snapshot.finalized_hash = H256::repeat_byte(9);
        assert!(f.evidence().is_err());
        let mut f = Fixture::normal();
        f.snapshot.genesis_hash = H256::repeat_byte(9);
        assert!(f.evidence().is_err());
        let mut f = Fixture::normal();
        let mut best = f.headers.remove(&f.snapshot.best_hash).unwrap();
        best.digest_mut().logs[0] =
            DigestItem::babe_pre_digest(PreDigest::SecondaryPlain(SecondaryPlainPreDigest {
                authority_index: 0,
                slot: 6600.into(),
            }));
        f.snapshot.best_hash = best.hash();
        f.canonical.insert(103, best.hash());
        f.headers.insert(best.hash(), best);
        assert!(f
            .evidence()
            .unwrap_err_message()
            .contains("same current epoch"));
    }

    #[test]
    fn refuses_missing_or_forged_canonical_ancestor_headers() {
        let mut f = Fixture::normal();
        f.headers.remove(&f.canonical[&100]);
        assert!(f
            .evidence()
            .unwrap_err_message()
            .contains("unavailable/pruned"));
        let mut f = Fixture::normal();
        f.canonical.insert(101, H256::repeat_byte(8));
        assert!(f
            .evidence()
            .unwrap_err_message()
            .contains("canonical ancestor"));
        let mut f = Fixture::normal();
        let hash = f.canonical[&101];
        f.headers.get_mut(&hash).unwrap().set_number(98);
        assert!(f.evidence().is_err());
    }

    #[test]
    fn requires_first_epoch_announcement_to_match_next_authorities_and_randomness() {
        let f = Fixture::new(vec![]);
        assert!(f
            .evidence()
            .unwrap_err_message()
            .contains("no NextEpochData"));
        let f = Fixture::new(vec![
            announcement(&runtime_epoch(11)),
            announcement(&runtime_epoch(11)),
        ]);
        assert!(f.evidence().is_err());
        let mut f = Fixture::normal();
        f.next.randomness = [44; 32];
        assert!(f
            .evidence()
            .unwrap_err_message()
            .contains("authorities/randomness"));
        let mut f = Fixture::normal();
        f.next.authorities[0].1 = 5;
        assert!(f.evidence().is_err());
    }

    #[test]
    fn rejects_partial_unknown_or_duplicate_config_digests() {
        let valid = ConsensusLog::NextConfigData(NextConfigDescriptor::V1 {
            c: (1, 4),
            allowed_slots: AllowedSlots::PrimaryAndSecondaryPlainSlots,
        })
        .encode();
        let mut trailing = valid.clone();
        trailing.push(0);
        for invalid in [valid[..3].to_vec(), trailing, vec![255]] {
            let f = Fixture::new(vec![
                announcement(&runtime_epoch(11)),
                DigestItem::Consensus(BABE_ENGINE_ID, invalid),
            ]);
            assert!(f
                .evidence()
                .unwrap_err_message()
                .contains("Malformed BABE consensus"));
        }
        let config = DigestItem::Consensus(BABE_ENGINE_ID, valid);
        let f = Fixture::new(vec![
            announcement(&runtime_epoch(11)),
            config.clone(),
            config,
        ]);
        assert!(f.evidence().unwrap_err_message().contains("Duplicate"));
    }

    #[test]
    fn refuses_noncanonical_anchors_payload_changes_and_bad_tree_metadata() {
        let f = Fixture::normal();
        let mut fork = EpochChanges::new();
        fork.reset(
            H256::repeat_byte(4),
            f.canonical[&102],
            102,
            f.current.clone().into(),
            f.next.clone().into(),
        );
        assert!(f
            .plan(&fork.encode())
            .unwrap_err_message()
            .contains("noncanonical fork"));
        let wrong_randomness = f.cache().map(|_, _, mut epoch| {
            epoch.randomness = [98; 32];
            epoch
        });
        assert!(f
            .plan(&wrong_randomness.encode())
            .unwrap_err_message()
            .contains("authorities/randomness"));
        let wrong_range = f.cache().map(|_, _, mut epoch| {
            epoch.duration += 1;
            epoch
        });
        assert!(f
            .plan(&wrong_range.encode())
            .unwrap_err_message()
            .contains("tree metadata"));
        let wrong_config = f.cache().map(|_, _, mut epoch| {
            epoch.config.c = (1, 3);
            epoch
        });
        assert!(f.plan(&wrong_config.encode()).is_err());
        let mut trailing = f.cache().encode();
        trailing.push(0);
        assert!(f.plan(&trailing).is_err());
    }

    #[test]
    fn refuses_an_old_epoch_payload_that_the_tree_would_select_at_best() {
        use sc_consensus_epochs::ViableEpoch;
        let f = Fixture::normal();
        let mut cache = f.cache();
        let earlier: CachedEpoch = runtime_epoch(8).into();
        let descriptor = NextEpochDescriptor {
            authorities: earlier.authorities.clone(),
            randomness: [9; 32],
        };
        let viable: ViableEpoch<CachedEpoch, &CachedEpoch> = ViableEpoch::Signaled(&earlier);
        let old_epoch = viable.increment((descriptor, vrf_config()));
        let ancestry = CanonicalAncestry(
            f.canonical
                .iter()
                .map(|(number, hash)| (*hash, *number))
                .collect(),
        );
        cache
            .import(
                ancestry,
                f.canonical[&103],
                103,
                f.canonical[&102],
                old_epoch,
            )
            .unwrap();
        // Range/payload checks alone allow this older epoch. The real SDK lookup must reject it.
        assert!(f
            .plan(&cache.encode())
            .unwrap_err_message()
            .contains("selects an unexpected"));
    }

    // Avoid imposing Debug on evidence/plan just to print an expected error in tests.
    trait ErrorMessage {
        fn unwrap_err_message(self) -> String;
    }
    impl<T> ErrorMessage for RepairResult<T> {
        fn unwrap_err_message(self) -> String {
            match self {
                Ok(_) => panic!("expected refusal"),
                Err(error) => error,
            }
        }
    }

    #[derive(Default)]
    struct MemoryStore {
        data: RefCell<BTreeMap<Vec<u8>, Vec<u8>>>,
        writes: RefCell<usize>,
        expected_backup: RefCell<Option<(PathBuf, Vec<u8>)>>,
    }

    impl MemoryStore {
        fn initialized(original: &[u8]) -> Self {
            Self {
                data: RefCell::new(BTreeMap::from([
                    (VERSION_KEY.to_vec(), 3u32.encode()),
                    (CACHE_KEY.to_vec(), original.to_vec()),
                    (b"unrelated".to_vec(), b"preserved".to_vec()),
                ])),
                ..Self::default()
            }
        }
    }

    impl AuxStore for MemoryStore {
        fn insert_aux<
            'a,
            'b: 'a,
            'c: 'a,
            I: IntoIterator<Item = &'a (&'c [u8], &'c [u8])>,
            D: IntoIterator<Item = &'a &'b [u8]>,
        >(
            &self,
            insert: I,
            delete: D,
        ) -> sp_blockchain::Result<()> {
            let (path, receipt) = self
                .expected_backup
                .borrow()
                .clone()
                .expect("backup expectation set");
            assert_eq!(
                std::fs::read(path).unwrap(),
                receipt,
                "backup must exist before any database write"
            );
            let operations: Vec<_> = insert.into_iter().collect();
            assert_eq!(
                operations.len(),
                1,
                "only one atomic cache insertion is allowed"
            );
            assert_eq!(operations[0].0, CACHE_KEY);
            assert_eq!(delete.into_iter().count(), 0);
            self.data
                .borrow_mut()
                .insert(CACHE_KEY.to_vec(), operations[0].1.to_vec());
            *self.writes.borrow_mut() += 1;
            Ok(())
        }
        fn get_aux(&self, key: &[u8]) -> sp_blockchain::Result<Option<Vec<u8>>> {
            Ok(self.data.borrow().get(key).cloned())
        }
    }

    struct TempDirectory(PathBuf);
    impl TempDirectory {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "sora-babe-repair-test-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn backup_precedes_single_atomic_write_and_preserves_unrelated_auxiliary_data() {
        let f = Fixture::normal();
        let original = f.cache().encode();
        let plan = f.plan(&original).unwrap();
        let store = MemoryStore::initialized(&original);
        let temp = TempDirectory::new();
        let path = temp.0.join("backup.json");
        let receipt = b"test receipt with original cache";
        *store.expected_backup.borrow_mut() = Some((path.clone(), receipt.to_vec()));
        apply_repair(
            &store,
            &3u32.encode(),
            &original,
            &plan.replacement,
            &path,
            receipt,
            || Ok(()),
        )
        .unwrap();
        assert_eq!(*store.writes.borrow(), 1);
        assert_eq!(store.get_aux(CACHE_KEY).unwrap().unwrap(), plan.replacement);
        assert_eq!(store.get_aux(VERSION_KEY).unwrap().unwrap(), 3u32.encode());
        assert_eq!(store.get_aux(b"unrelated").unwrap().unwrap(), b"preserved");
    }

    #[test]
    fn backup_failure_existing_backup_or_changed_cache_never_writes() {
        let original = b"original";
        let store = MemoryStore::initialized(original);
        let temp = TempDirectory::new();
        let existing = temp.0.join("existing.json");
        std::fs::write(&existing, b"keep me").unwrap();
        assert!(apply_repair(
            &store,
            &3u32.encode(),
            original,
            b"new",
            &existing,
            b"receipt",
            || Ok(()),
        )
        .is_err());
        assert_eq!(std::fs::read(&existing).unwrap(), b"keep me");
        assert!(apply_repair(
            &store,
            &3u32.encode(),
            original,
            b"new",
            &temp.0.join("absent/backup"),
            b"receipt",
            || Ok(()),
        )
        .is_err());
        assert!(apply_repair(
            &store,
            &3u32.encode(),
            b"stale",
            b"new",
            &temp.0.join("unused"),
            b"receipt",
            || Ok(()),
        )
        .is_err());
        assert!(!temp.0.join("unused").exists());
        assert_eq!(*store.writes.borrow(), 0);
        assert_eq!(store.get_aux(CACHE_KEY).unwrap().unwrap(), original);
    }

    #[test]
    fn unknown_missing_or_partial_schema_is_refused() {
        let store = MemoryStore::initialized(b"cache");
        for value in [
            Some(2u32.encode()),
            Some(vec![3]),
            Some(vec![3, 0, 0, 0, 9]),
            None,
        ] {
            match value {
                Some(version) => {
                    store
                        .data
                        .borrow_mut()
                        .insert(VERSION_KEY.to_vec(), version);
                }
                None => {
                    store.data.borrow_mut().remove(VERSION_KEY);
                }
            }
            assert!(read_cache(&store).is_err());
        }
    }

    fn db_settings(source: sc_client_db::DatabaseSource) -> sc_client_db::DatabaseSettings {
        sc_client_db::DatabaseSettings {
            trie_cache_maximum_size: None,
            state_pruning: None,
            source,
            blocks_pruning: sc_client_db::BlocksPruning::Some(256),
            pruning_filters: vec![],
            metrics_registry: None,
        }
    }

    // Build a real SDK database exclusively inside an owned temporary directory. Unlike the
    // production command, this fixture deliberately initializes a new empty database.
    fn initialized_engine(path: &Path, parity: bool) -> sc_client_db::DatabaseSource {
        let source = if parity {
            sc_client_db::DatabaseSource::ParityDb { path: path.into() }
        } else {
            sc_client_db::DatabaseSource::RocksDb {
                path: path.into(),
                cache_size: 8,
            }
        };
        drop(sc_client_db::Backend::<Block>::new(db_settings(source.clone()), 4096).unwrap());
        source
    }

    #[test]
    fn real_engines_preserve_reads_and_allow_only_one_exact_auxiliary_replacement() {
        use sp_database::{Database, Transaction};
        for parity in [false, true] {
            let temp = TempDirectory::new();
            let source = initialized_engine(&temp.0.join("db"), parity);
            let db = GuardedDatabase::open(&source).unwrap();
            assert_eq!(db.supports_ref_counting(), parity);
            assert_eq!(db.get(0, b"type"), Some(b"full".to_vec()));
            let mut key = vec![1; 64];
            db.sanitize_key(&mut key);
            assert_eq!(key.len(), if parity { 32 } else { 64 });
            db.commit(Transaction::new()).unwrap();
            let mut replacement = Transaction::new();
            replacement.set(8, CACHE_KEY, b"replacement");
            assert!(db.commit(replacement.clone()).is_err());
            db.authorize_cache_write(b"replacement".to_vec()).unwrap();
            for (column, key, value) in [
                (0, &CACHE_KEY[..], &b"replacement"[..]),
                (8, &b"unrelated"[..], &b"replacement"[..]),
                (8, &CACHE_KEY[..], &b"incorrect"[..]),
            ] {
                let mut forbidden = Transaction::new();
                forbidden.set(column, key, value);
                assert!(db.commit(forbidden).is_err());
            }
            let mut mixed = replacement.clone();
            mixed.remove(0, b"type");
            assert!(db.commit(mixed).is_err());
            db.commit(replacement.clone()).unwrap();
            assert!(db.commit(replacement).is_err());
            assert_eq!(db.get(8, CACHE_KEY), Some(b"replacement".to_vec()));
            assert_eq!(db.get(0, b"type"), Some(b"full".to_vec()));
            drop(db);
            let reopened = GuardedDatabase::open(&source).unwrap();
            assert_eq!(reopened.get(8, CACHE_KEY), Some(b"replacement".to_vec()));
            assert_eq!(reopened.get(0, b"type"), Some(b"full".to_vec()));
        }
    }

    #[test]
    fn real_sdk_backends_require_backup_and_preserve_cache_schema_and_unrelated_data() {
        use clap::Parser;
        use sc_cli::CliConfiguration;
        use sc_client_api::{Backend, BlockImportOperation, NewBlockState, TrieCacheContext};
        use sp_core::storage::{StateVersion, Storage};
        use sp_runtime::traits::Block as BlockT;
        use sp_state_machine::Backend as _;

        for parity in [false, true] {
            let temp = TempDirectory::new();
            let source = initialized_engine(&temp.0.join("db"), parity);
            let fixture = Fixture::normal();
            let original = fixture.cache().encode();
            let plan = fixture.plan(&original).unwrap();
            let version = 3u32.encode();
            let backend =
                sc_client_db::Backend::<Block>::new(db_settings(source.clone()), 4096).unwrap();
            let mut storage = Storage::default();
            storage
                .top
                .insert(b"fixture-key".to_vec(), b"fixture-state".to_vec());
            let mut operation = backend.begin_operation().unwrap();
            let root = operation
                .set_genesis_state(storage, true, StateVersion::V1)
                .unwrap();
            let genesis = sc_service::construct_genesis_block::<Block>(root, StateVersion::V1);
            let header = genesis.header().clone();
            let hash = header.hash();
            operation
                .set_block_data(
                    header.clone(),
                    Some(vec![]),
                    None,
                    None,
                    NewBlockState::Final,
                    true,
                )
                .unwrap();
            backend.commit_operation(operation).unwrap();
            AuxStore::insert_aux(
                &backend,
                &[
                    (VERSION_KEY, version.as_slice()),
                    (CACHE_KEY, original.as_slice()),
                    (&b"unrelated"[..], &b"preserved"[..]),
                ],
                &[],
            )
            .unwrap();
            drop(backend);

            let database = Arc::new(GuardedDatabase::open(&source).unwrap());
            let settings = db_settings(sc_client_db::DatabaseSource::Custom {
                db: database.clone(),
                require_create_flag: false,
            });
            let guarded = Arc::new(sc_client_db::Backend::<Block>::new(settings, 4096).unwrap());
            assert_eq!(
                guarded.blockchain().header(hash).unwrap(),
                Some(header.clone())
            );
            assert_eq!(guarded.blockchain().hash(0).unwrap(), Some(hash));
            assert_eq!(guarded.blockchain().info().finalized_state, Some((hash, 0)));
            assert_eq!(
                guarded
                    .state_at(hash, TrieCacheContext::Untrusted)
                    .unwrap()
                    .storage(b"fixture-key")
                    .unwrap(),
                Some(b"fixture-state".to_vec())
            );
            assert_eq!(
                read_cache(&*guarded).unwrap(),
                (version.clone(), original.clone())
            );
            // Exercise the production client builder with an existing finalized state. This
            // fixture intentionally has no :code, so the API evidence call must fail without
            // any genesis fallback or logical database write.
            {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                let cli = crate::cli::Cli::try_parse_from(["framenode"]).unwrap();
                let cmd = RepairBabeEpochCacheCmd::try_parse_from([
                    "repair",
                    "--chain",
                    "main",
                    "--database",
                    "rocksdb",
                    "--base-path",
                    temp.0.to_str().unwrap(),
                ])
                .unwrap();
                let mut config = cmd
                    .create_configuration(&cli, runtime.handle().clone())
                    .unwrap();
                let original_spec = config.chain_spec.as_json(false).unwrap();
                let original_genesis = config.chain_spec.build_storage().unwrap();
                let mut spec: serde_json::Value = serde_json::from_str(&original_spec).unwrap();
                assert_eq!(config.chain_spec.code_substitutes().len(), 2);
                // An invalid extra substitute would make the SDK client constructor fail if
                // any substitute reached it, even before the deliberately unavailable API call.
                spec["codeSubstitutes"]["0"] = "0xdeadbeef".into();
                config.chain_spec = Box::new(
                    framenode_chain_spec::ChainSpec::from_json_bytes(
                        serde_json::to_vec(&spec).unwrap(),
                    )
                    .unwrap(),
                );
                config.wasm_runtime_overrides = Some(temp.0.join("overrides"));
                assert!(use_on_chain_runtime(&mut config).is_err());
                assert_eq!(config.chain_spec.code_substitutes().len(), 3);
                config.wasm_runtime_overrides = None;
                use_on_chain_runtime(&mut config).unwrap();
                assert!(config.chain_spec.code_substitutes().is_empty());
                spec["codeSubstitutes"] = serde_json::json!({});
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(
                        &config.chain_spec.as_json(false).unwrap()
                    )
                    .unwrap(),
                    spec
                );
                let copied_genesis = config.chain_spec.build_storage().unwrap();
                assert_eq!(copied_genesis.top, original_genesis.top);
                assert_eq!(
                    copied_genesis.children_default,
                    original_genesis.children_default
                );
                assert_eq!(
                    framenode_chain_spec::main_net()
                        .unwrap()
                        .as_json(false)
                        .unwrap(),
                    original_spec
                );
                config.keystore = sc_service::config::KeystoreConfig::InMemory;
                config.database = sc_client_db::DatabaseSource::Custom {
                    db: database.clone(),
                    require_create_flag: false,
                };
                let executor = sc_service::new_wasm_executor::<(
                    sp_io::SubstrateHostFunctions,
                    frame_benchmarking::benchmarking::HostFunctions,
                )>(&config.executor);
                let (client, _, _, task_manager) =
                    sc_service::new_full_parts_with_genesis_builder::<Block, RuntimeApi, _, _>(
                        &config,
                        None,
                        executor,
                        guarded.clone(),
                        RefuseGenesis,
                        false,
                    )
                    .unwrap();
                assert_eq!(client.header(hash).unwrap(), Some(header.clone()));
                assert!(client.runtime_api().current_epoch(hash).is_err());
                assert!(database.permitted_cache.lock().unwrap().is_none());
                assert_eq!(
                    read_cache(&client).unwrap(),
                    (version.clone(), original.clone())
                );
                drop(client);
                drop(task_manager);
            }
            let backup = temp.0.join("backup.json");
            std::fs::write(&backup, b"existing backup must not be replaced").unwrap();
            assert!(apply_repair(
                &*guarded,
                &version,
                &original,
                &plan.replacement,
                &backup,
                b"receipt",
                || panic!("A failed backup must never authorize a write"),
            )
            .is_err());
            assert_eq!(
                read_cache(&*guarded).unwrap(),
                (version.clone(), original.clone())
            );
            let backup = temp.0.join("new-backup.json");
            apply_repair(
                &*guarded,
                &version,
                &original,
                &plan.replacement,
                &backup,
                b"receipt",
                || {
                    assert_eq!(std::fs::read(&backup).unwrap(), b"receipt");
                    database.authorize_cache_write(plan.replacement.clone())
                },
            )
            .unwrap();
            assert_eq!(
                read_cache(&*guarded).unwrap(),
                (version.clone(), plan.replacement.clone())
            );
            assert_eq!(
                AuxStore::get_aux(&*guarded, b"unrelated").unwrap().unwrap(),
                b"preserved"
            );
            drop(guarded);
            drop(database);

            let reopened = sc_client_db::Backend::<Block>::new(db_settings(source), 4096).unwrap();
            assert_eq!(reopened.blockchain().header(hash).unwrap(), Some(header));
            assert_eq!(
                reopened
                    .state_at(hash, TrieCacheContext::Untrusted)
                    .unwrap()
                    .storage(b"fixture-key")
                    .unwrap(),
                Some(b"fixture-state".to_vec())
            );
            assert_eq!(read_cache(&reopened).unwrap(), (version, plan.replacement));
            assert_eq!(
                AuxStore::get_aux(&reopened, b"unrelated").unwrap().unwrap(),
                b"preserved"
            );
        }
    }

    #[test]
    fn existing_engine_guards_refuse_missing_future_or_incompatible_layouts_without_creating_data()
    {
        let temp = TempDirectory::new();
        let absent = temp.0.join("absent");
        assert!(
            GuardedDatabase::open(&sc_client_db::DatabaseSource::ParityDb {
                path: absent.clone(),
            })
            .is_err()
        );
        assert!(!absent.exists());
        assert!(
            GuardedDatabase::open(&sc_client_db::DatabaseSource::RocksDb {
                path: absent.clone(),
                cache_size: 8,
            })
            .is_err()
        );
        assert!(!absent.exists());

        let parity = initialized_engine(&temp.0.join("parity"), true);
        let metadata = temp.0.join("parity/metadata");
        let original = std::fs::read_to_string(&metadata).unwrap();
        for version in [7, 9] {
            let unsupported = original.replacen("version=8", &format!("version={version}"), 1);
            assert_ne!(original, unsupported);
            std::fs::write(&metadata, &unsupported).unwrap();
            assert!(GuardedDatabase::open(&parity).is_err());
            assert_eq!(std::fs::read_to_string(&metadata).unwrap(), unsupported);
        }

        let rocks_path = temp.0.join("rocks-default-only");
        let mut options = rocksdb::Options::default();
        options.create_if_missing(true);
        drop(rocksdb::DB::open(&options, &rocks_path).unwrap());
        std::fs::write(rocks_path.join("db_version"), b"4").unwrap();
        let before = rocksdb::DB::list_cf(&options, &rocks_path).unwrap();
        assert_eq!(before, vec!["default"]);
        let source = sc_client_db::DatabaseSource::RocksDb {
            path: rocks_path.clone(),
            cache_size: 8,
        };
        assert!(GuardedDatabase::open(&source).is_err());
        assert_eq!(rocksdb::DB::list_cf(&options, &rocks_path).unwrap(), before);
    }

    #[test]
    fn backend_initialization_cannot_remove_existing_missing_body_gap() {
        use sp_database::{Database, Transaction};
        let temp = TempDirectory::new();
        let source = initialized_engine(&temp.0.join("db"), false);
        let sc_client_db::DatabaseSource::RocksDb { path, .. } = &source else {
            unreachable!()
        };
        // Populate just the metadata needed to reproduce the SDK's restart-time gap removal.
        let raw =
            kvdb_rocksdb::Database::open(&kvdb_rocksdb::DatabaseConfig::with_columns(13), path)
                .unwrap();
        let raw = sp_database::as_rocksdb_database::<H256>(raw);
        let gap = sp_blockchain::BlockGap {
            start: 1u32,
            end: 10u32,
            gap_type: sp_blockchain::BlockGapType::MissingBody,
        }
        .encode();
        let mut fixture = Transaction::new();
        fixture.set(0, b"gen", &H256::repeat_byte(7).encode());
        fixture.set(0, b"gap", &gap);
        fixture.set(0, b"gap_ver", &1u32.encode());
        raw.commit(fixture).unwrap();
        drop(raw);
        let guard = Arc::new(GuardedDatabase::open(&source).unwrap());
        let settings = db_settings(sc_client_db::DatabaseSource::Custom {
            db: guard.clone(),
            require_create_flag: false,
        });
        let error = sc_client_db::Backend::<Block>::new(settings, 4096)
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("unrelated write refused"),
            "{error}"
        );
        assert_eq!(guard.get(0, b"gap"), Some(gap.clone()));
        assert_eq!(guard.get(0, b"gap_ver"), Some(1u32.encode()));
        drop(guard);
        let reopened = GuardedDatabase::open(&source).unwrap();
        assert_eq!(reopened.get(0, b"gap"), Some(gap));
    }

    #[test]
    fn cli_defaults_to_diagnosis_and_requires_explicit_apply_with_backup() {
        use clap::Parser;
        assert!(RepairBabeEpochCacheCmd::try_parse_from(["repair"]).is_err());
        let cmd =
            RepairBabeEpochCacheCmd::try_parse_from(["repair", "--database", "paritydb"]).unwrap();
        assert!(!cmd.apply);
        assert!(cmd.backup.is_none());
        assert!(RepairBabeEpochCacheCmd::try_parse_from([
            "repair",
            "--database",
            "rocksdb",
            "--apply"
        ])
        .is_err());
        assert!(RepairBabeEpochCacheCmd::try_parse_from([
            "repair",
            "--database",
            "rocksdb",
            "--backup",
            "new.json"
        ])
        .is_err());
        assert!(RepairBabeEpochCacheCmd::try_parse_from([
            "repair",
            "--database",
            "rocksdb",
            "--apply",
            "--backup",
            "new.json"
        ])
        .is_ok());
    }
}
