//! Fixed predecessor reconstruction, with explicit verification before resuming an
//! owned partial output. Never rewinds or edits the source DB.
use super::*;
use sc_client_api::backend::{
    AuxStore, Backend as _, BlockImportOperation as _, NewBlockState, TrieCacheContext,
};
use sc_client_db::{Backend, BlocksPruning, DatabaseSettings, DatabaseSource, PruningMode};
use sp_blockchain::{Backend as _, HeaderBackend as _};
use sp_consensus_babe::{AllowedSlots, BabeEpochConfiguration, Epoch};
use sp_core::storage::StateVersion;
use sp_runtime::Justifications;
use sp_state_machine::{Backend as _, IterArgs};

const GENESIS: &str = "7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5";
const MAX_BODY: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OutputMode {
    Fresh,
    Resume,
}

fn owned_directory(directory: &Path) -> Result<()> {
    need(
        fs::symlink_metadata(directory)
            .map_err(|e| e.to_string())?
            .is_dir()
            && fs::canonicalize(directory).map_err(|e| e.to_string())? == directory,
        "independent canonical reconstruction directory required",
    )
}

/// Only the output shape made by this tool may be resumed. In particular an
/// existing (even truncated) receipt is never authorization to reopen a result.
fn partial_output_layout(output_dir: &Path) -> Result<()> {
    owned_directory(output_dir)?;
    for relative in [
        "",
        "chains",
        "chains/sora-nexus-recovery-predecessor",
        "chains/sora-nexus-recovery-predecessor/db",
    ] {
        let directory = output_dir.join(relative);
        owned_directory(&directory)?;
        let expected: BTreeSet<&str> = match relative {
            "" => BTreeSet::from(["chains", "source-read-only-logs"]),
            "chains" => BTreeSet::from(["sora-nexus-recovery-predecessor"]),
            "chains/sora-nexus-recovery-predecessor" => BTreeSet::from(["db"]),
            _ => BTreeSet::from(["full"]),
        };
        let names = fs::read_dir(&directory)
            .map_err(|e| e.to_string())?
            .map(|entry| {
                entry
                    .map_err(|e| e.to_string())?
                    .file_name()
                    .into_string()
                    .map_err(|_| "nonutf8 reconstruction entry".to_string())
            })
            .collect::<Result<BTreeSet<_>>>()?;
        need(
            names.iter().map(String::as_str).collect::<BTreeSet<_>>() == expected,
            "partial reconstruction contains foreign entries or a completion receipt",
        )?;
    }
    let db_path = output_dir.join("chains/sora-nexus-recovery-predecessor/db/full");
    owned_directory(&db_path)?;
    let mut count = 0;
    for entry in fs::read_dir(&db_path).map_err(|e| e.to_string())? {
        regular(&entry.map_err(|e| e.to_string())?.path())?;
        count += 1;
        need(count <= 100_000, "output database file count bound")?;
    }
    need(count > 0, "missing partial database")?;
    owned_directory(&output_dir.join("source-read-only-logs"))
}

fn readonly_output(path: &Path, logs: &Path) -> Result<DB> {
    need(
        read(&path.join("db_version"), 16)? == b"4",
        "partial SDK schema differs",
    )?;
    let mut options = Options::default();
    options.create_if_missing(false);
    options.create_missing_column_families(false);
    options.set_db_log_dir(logs);
    let mut families = DB::list_cf(&options, path).map_err(|e| e.to_string())?;
    families.sort();
    let mut expected = (0..13).map(|i| format!("col{i}")).collect::<Vec<_>>();
    expected.push("default".into());
    expected.sort();
    need(families == expected, "partial database columns differ")?;
    DB::open_cf_for_read_only(&options, path, families, false).map_err(|e| e.to_string())
}

/// Raw metadata is checked before the writable SDK open (which can repair old
/// missing fstate metadata). A cursor is never taken from a progress JSON file.
fn raw_cursor(db: &DB, genesis: H256, end: u32) -> Result<Option<(u32, H256)>> {
    need(
        get(db, 0, b"type")? == b"full",
        "partial database type differs",
    )?;
    // ArchiveAll in the pinned SDK writes only this state metadata key; pending
    // noncanonical/pruning journals cannot belong to this reconstruction lane.
    need(
        get(db, 2, b"mode")? == b"archive" && column_count(db, "col2", 1)? == 1,
        "partial state metadata differs from ArchiveAll reconstruction",
    )?;
    need(
        optional(db, 0, b"gap")?.is_none(),
        "partial database has a block gap",
    )?;
    let gen = optional(db, 0, b"gen")?;
    let best = optional(db, 0, b"best")?;
    let finalized = optional(db, 0, b"final")?;
    let state = optional(db, 0, b"fstate")?;
    let leaves: Vec<(u32, Vec<H256>)> = optional(db, 0, b"leaf")?
        .map(|raw| decode(&raw))
        .transpose()?
        .unwrap_or_default();
    match gen {
        None => {
            need(
                best.is_none() && finalized.is_none() && state.is_none() && leaves.is_empty(),
                "empty partial metadata differs",
            )?;
            Ok(None)
        }
        Some(gen) => {
            need(gen == genesis.as_bytes(), "partial genesis differs")?;
            let best = best.ok_or("partial best metadata missing")?;
            need(
                finalized.as_ref() == Some(&best),
                "partial best/final metadata differ",
            )?;
            let (number, hash) = number_lookup(&best)?;
            need(
                number <= end,
                "partial database contains checkpoint or abandoned descendants",
            )?;
            let (header, _) = header_at(db, number)?;
            need(header.hash() == hash, "partial tip lookup differs")?;
            need(
                leaves == vec![(number, vec![hash])],
                "partial encoded leaf set differs",
            )?;
            need(
                if number == end {
                    state.as_ref() == Some(&best)
                } else {
                    state.is_none()
                },
                "partial finalized state transaction is incomplete or unexpected",
            )?;
            Ok(Some((number, hash)))
        }
    }
}

fn sdk_cursor(
    info: &sp_blockchain::Info<ChainBlock>,
    genesis: H256,
    end: u32,
) -> Result<Option<(u32, H256)>> {
    need(info.block_gap.is_none(), "partial SDK block gap")?;
    if info.genesis_hash == H256::zero() {
        need(
            info.best_hash == H256::zero()
                && info.finalized_hash == H256::zero()
                && info.best_number == 0
                && info.finalized_number == 0
                && info.finalized_state.is_none()
                && info.number_leaves == 0,
            "empty partial SDK metadata differs",
        )?;
        return Ok(None);
    }
    need(
        info.genesis_hash == genesis
            && info.best_number <= end
            && info.best_number == info.finalized_number
            && info.best_hash == info.finalized_hash
            && info.number_leaves == 1,
        "partial SDK tip is foreign, unfinalized or forked",
    )?;
    let tip = (info.best_hash, info.best_number);
    need(
        if info.best_number == end {
            info.finalized_state == Some(tip)
        } else {
            info.finalized_state.is_none()
        },
        "partial SDK finalized state differs",
    )?;
    Ok(Some((info.best_number, info.best_hash)))
}

fn column_count(db: &DB, column: &str, max: u64) -> Result<u64> {
    let cf = db.cf_handle(column).ok_or("missing partial column")?;
    let mut iter = db.raw_iterator_cf(cf);
    iter.seek_to_first();
    let mut count = 0u64;
    while iter.valid() {
        count += 1;
        need(count <= max, "partial database contains extra records")?;
        iter.next();
    }
    iter.status().map_err(|e| e.to_string())?;
    Ok(count)
}

/// Exact canonical point reads plus these column cardinalities exclude hidden
/// forks, orphan bodies/indices, local signing and offchain/transaction state.
fn prefix_shape(
    db: &DB,
    records: u64,
    justifications: u64,
    final_state: bool,
    aux_count: u64,
) -> Result<()> {
    for (column, expected) in [
        (3, records * 2),
        (4, records),
        (5, records),
        (6, justifications),
        (8, aux_count),
    ] {
        need(
            column_count(db, &format!("col{column}"), expected)? == expected,
            "partial database record coverage differs",
        )?;
    }
    for column in ["default", "col7", "col9", "col10", "col11", "col12"] {
        need(
            column_count(db, column, 0)? == 0,
            "foreign partial database state",
        )?;
    }
    if !final_state {
        need(
            column_count(db, "col1", 0)? == 0,
            "state exists before final predecessor transaction",
        )?;
    }
    let cf = db.cf_handle("col0").ok_or("missing partial metadata")?;
    let mut iter = db.raw_iterator_cf(cf);
    iter.seek_to_first();
    let mut children = 0;
    while let Some(key) = iter.key() {
        if key.starts_with(b"children") && key.len() == 40 {
            children += 1;
            need(children <= records, "extra partial child index")?;
        } else {
            need(
                [
                    b"type".as_slice(),
                    b"best",
                    b"final",
                    b"fstate",
                    b"gen",
                    b"leaf",
                ]
                .contains(&key),
                "unknown partial SDK metadata",
            )?;
        }
        iter.next();
    }
    iter.status().map_err(|e| e.to_string())?;
    need(children == records, "partial child index coverage differs")
}

#[derive(Default)]
struct HistoryProgress {
    previous: H256,
    digest: Sha256,
    bodies: u64,
    justifications: u64,
    last_justification: Option<Vec<u8>>,
}

impl HistoryProgress {
    fn observe(
        &mut self,
        number: u32,
        record: &HistoryRecord,
        genesis: H256,
        set: u64,
        keys: &AuthorityList,
        last_boundary: u32,
    ) -> Result<()> {
        let (header, body, just) = record;
        need(
            u64::from(number) == self.bodies
                && *header.number() == number
                && *header.parent_hash() == self.previous
                && (number != 0 || header.hash() == genesis),
            "canonical retained history ancestry mismatch",
        )?;
        if let Some(proofs) = just {
            self.justifications += 1;
            if number > last_boundary {
                if let Some(raw) = proofs.get(sp_consensus_grandpa::GRANDPA_ENGINE_ID) {
                    let proof: GrandpaJustification<ChainBlock> = decode(raw)?;
                    need(
                        proof.target() == (number, header.hash()),
                        "stored justification target differs",
                    )?;
                    proof.verify(set, keys).map_err(|e| e.to_string())?;
                    self.last_justification = Some(raw.clone());
                }
            }
        }
        record_digest(
            &mut self.digest,
            number,
            &header.encode(),
            &body.encode(),
            just.as_ref().map(Encode::encode).as_deref(),
        );
        self.previous = header.hash();
        self.bodies += 1;
        Ok(())
    }
}

fn compare_record(expected: &HistoryRecord, actual: &HistoryRecord) -> Result<()> {
    need(
        expected.0.encode() == actual.0.encode()
            && expected.1.encode() == actual.1.encode()
            && expected.2.as_ref().map(Encode::encode) == actual.2.as_ref().map(Encode::encode),
        "persisted canonical prefix differs from authenticated source",
    )
}

struct PrefixContext<'a> {
    genesis: H256,
    set: u64,
    authorities: &'a AuthorityList,
    last_boundary: u32,
}

fn rebuild_prefix(
    partial: &DB,
    tip: Option<(u32, H256)>,
    source_record: impl Fn(u32) -> Result<HistoryRecord>,
    context: PrefixContext<'_>,
) -> Result<HistoryProgress> {
    let mut progress = HistoryProgress::default();
    if let Some((tip_number, tip_hash)) = tip {
        for number in 0..=tip_number {
            let expected = source_record(number)?;
            let actual = history_record(partial, number)?;
            compare_record(&expected, &actual)?;
            let mut child_key = b"children".to_vec();
            child_key.extend(expected.0.parent_hash().as_bytes());
            need(
                decode::<Vec<H256>>(&get(partial, 0, &child_key)?)? == vec![expected.0.hash()],
                "partial child index differs",
            )?;
            progress.observe(
                number,
                &expected,
                context.genesis,
                context.set,
                context.authorities,
                context.last_boundary,
            )?;
            if number % 100_000 == 0 {
                eprintln!("retained predecessor resume: verified existing prefix {number}/{tip_number}; no blocks appended");
            }
        }
        need(
            progress.previous == tip_hash,
            "verified prefix tip differs from SDK metadata",
        )?;
    }
    Ok(progress)
}

fn optional(db: &DB, column: u32, key: &[u8]) -> Result<Option<Vec<u8>>> {
    let cf = db
        .cf_handle(&format!("col{column}"))
        .ok_or("missing source column")?;
    let raw = db.get_cf(cf, key).map_err(|e| e.to_string())?;
    need(
        raw.as_ref().is_none_or(|v| v.len() <= MAX_BODY),
        "bounded source record required",
    )?;
    Ok(raw)
}

fn body_decode<T: Decode + Encode>(raw: &[u8]) -> Result<T> {
    need(!raw.is_empty() && raw.len() <= MAX_BODY, "body byte bound")?;
    let mut input = raw;
    let value = T::decode_with_depth_limit(
        64,
        &mut codec::MemTrackingInput::new(&mut input, MAX_BODY * 8),
    )
    .map_err(|e| e.to_string())?;
    need(
        input.is_empty() && value.encode() == raw,
        "noncanonical body/justification encoding",
    )?;
    Ok(value)
}

// The SDK AuthoritySet and its complete encoding are decoded first. The second
// shape is restricted to EMPTY pending containers: no private PendingChange is
// manufactured, interpreted, dropped, or rewritten. The original bytes survive.
fn unchanged_parent_grandpa(raw: &[u8], state: &Storage) -> Result<(u64, AuthorityList, u32)> {
    let _: AuthoritySet<H256, u32> = decode(raw)?;
    type EmptyPendingSet = (
        AuthorityList,
        u64,
        (Vec<()>, Option<u32>),
        Vec<()>,
        Vec<(u64, u32)>,
    );
    let (keys, set, (pending, finalized), forced, changes): EmptyPendingSet = decode(raw)?;
    need(
        pending.is_empty() && forced.is_empty(),
        "pending GRANDPA transitions need a separate typed rewind",
    )?;
    need(
        !state
            .top
            .contains_key(&storage_key("Grandpa", "PendingChange")),
        "parent has a pending on-chain GRANDPA change",
    )?;
    need(
        authorities(keys.clone())?
            == storage_value::<AuthorityList>(state, "Grandpa", "Authorities")?
            && set == storage_value::<u64>(state, "Grandpa", "CurrentSetId")?,
        "parent GRANDPA authority identity differs",
    )?;
    need(
        set > 0
            && changes.len() == set as usize
            && changes.iter().enumerate().all(|(i, (id, n))| {
                *id == i as u64 && *n < CHECKPOINT - 1 && (i == 0 || *n > changes[i - 1].1)
            }),
        "parent GRANDPA historical sequence differs",
    )?;
    let last = changes.last().ok_or("missing authority boundary")?.1;
    need(
        finalized.is_some_and(|n| n >= last && n < CHECKPOINT),
        "GRANDPA pending tree was finalized beyond predecessor",
    )?;
    Ok((set, keys, last))
}

fn parent_epochs(state: &Storage, parent: &ChainHeader) -> Result<(Vec<u8>, Value)> {
    let index: u64 = storage_value(state, "Babe", "EpochIndex")?;
    let genesis: u64 = storage_value(state, "Babe", "GenesisSlot")?;
    let start = index
        .checked_mul(600)
        .and_then(|n| n.checked_add(genesis))
        .ok_or("epoch overflow")?;
    let config: BabeEpochConfiguration = storage_value(state, "Babe", "EpochConfig")?;
    let next_config = match state.top.get(&storage_key("Babe", "NextEpochConfig")) {
        Some(raw) => decode(raw)?,
        None => config.clone(),
    };
    let current = Epoch {
        epoch_index: index,
        start_slot: start.into(),
        duration: 600,
        authorities: storage_value(state, "Babe", "Authorities")?,
        randomness: storage_value(state, "Babe", "Randomness")?,
        config,
    };
    let next = Epoch {
        epoch_index: index.checked_add(1).ok_or("epoch index overflow")?,
        start_slot: start.checked_add(600).ok_or("epoch start overflow")?.into(),
        duration: 600,
        authorities: storage_value(state, "Babe", "NextAuthorities")?,
        randomness: storage_value(state, "Babe", "NextRandomness")?,
        config: next_config,
    };
    for epoch in [&current, &next] {
        let mut seen = BTreeSet::new();
        need(
            !epoch.authorities.is_empty()
                && epoch
                    .authorities
                    .iter()
                    .all(|(k, w)| *w > 0 && seen.insert(k.encode())),
            "invalid captured BABE authorities",
        )?;
        need(
            epoch.config.c == (1, 4)
                && matches!(
                    epoch.config.allowed_slots,
                    AllowedSlots::PrimaryAndSecondaryPlainSlots
                        | AllowedSlots::PrimaryAndSecondaryVRFSlots
                ),
            "unsupported captured BABE configuration",
        )?;
    }
    let mut cache = BabeCache::new();
    // This is precisely the SDK's state-sync reset operation, NOT a claim that
    // these two synthetic cache anchors were the historical announcement blocks.
    cache.reset(
        *parent.parent_hash(),
        parent.hash(),
        *parent.number(),
        current.clone().into(),
        next.clone().into(),
    );
    let raw = cache.encode();
    let _: BabeCache = decode(&raw)?;
    Ok((
        raw,
        json!({"method":"SDK EpochChanges::reset from root-verified parent current/next state",
        "currentEpoch":index,"currentScale":hex::encode(current.encode()),"nextScale":hex::encode(next.encode()),
        "syntheticStateSyncAnchors":true,"historicalAnnouncementsReconstructed":false,
        "configurationRepairApplied":false,"offlineBabeCacheRepairRequired":true}),
    ))
}

fn record_digest(sha: &mut Sha256, number: u32, header: &[u8], body: &[u8], just: Option<&[u8]>) {
    sha.update(number.to_le_bytes());
    for raw in [Some(header), Some(body), just] {
        sha.update([u8::from(raw.is_some())]);
        if let Some(raw) = raw {
            sha.update((raw.len() as u64).to_le_bytes());
            sha.update(raw);
        }
    }
}

fn state_digest<'a>(rows: impl IntoIterator<Item = (&'a Vec<u8>, &'a Vec<u8>)>) -> (u64, String) {
    let mut sha = Sha256::new();
    let mut count = 0;
    for (key, value) in rows {
        sha.update((key.len() as u64).to_le_bytes());
        sha.update(key);
        sha.update((value.len() as u64).to_le_bytes());
        sha.update(value);
        count += 1;
    }
    (count, hex::encode(sha.finalize()))
}

fn open_backend(path: &Path) -> Result<Backend<ChainBlock>> {
    Backend::new(
        DatabaseSettings {
            trie_cache_maximum_size: Some(64 * 1024 * 1024),
            state_pruning: Some(PruningMode::ArchiveAll),
            source: DatabaseSource::RocksDb {
                path: path.to_owned(),
                cache_size: 256,
            },
            blocks_pruning: BlocksPruning::KeepAll,
            pruning_filters: Vec::new(),
            metrics_registry: None,
        },
        4096,
    )
    .map_err(|e| e.to_string())
}

type HistoryRecord = (ChainHeader, Vec<OpaqueExtrinsic>, Option<Justifications>);

fn history_record(db: &DB, number: u32) -> Result<HistoryRecord> {
    let (header, _) = header_at(db, number)?;
    let mut lookup = number.to_be_bytes().to_vec();
    lookup.extend(header.hash().as_bytes());
    need(
        optional(db, 12, &lookup)?.is_none(),
        "indexed history requires a separate preservation adapter",
    )?;
    let raw = optional(db, 5, &lookup)?.ok_or_else(|| {
        format!("full historical body missing at {number}; refusing header-only reconstruction")
    })?;
    let body: Vec<OpaqueExtrinsic> = body_decode(&raw)?;
    let root = sp_io::trie::blake2_256_ordered_root(
        body.iter().map(Encode::encode).collect(),
        StateVersion::V0,
    );
    need(
        root == *header.extrinsics_root(),
        "historical body root mismatch",
    )?;
    let just: Option<Justifications> = optional(db, 6, &lookup)?
        .map(|raw| body_decode(&raw))
        .transpose()?;
    if let Some(proofs) = &just {
        let mut engines = BTreeSet::new();
        need(
            proofs.iter().all(|(engine, _)| engines.insert(*engine)),
            "duplicate historical justification engine",
        )?;
    }
    Ok((header, body, just))
}

pub(super) fn run(evidence_output: &Path, output_dir: &Path, mode: OutputMode) -> Result<Value> {
    let root = Path::new(ROOT);
    let allowed = root.join("fixtures/retained-predecessor-reconstruction-v1");
    need(
        output_dir.parent() == Some(allowed.as_path())
            && output_dir
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|s| {
                    s.strip_prefix("attempt-")
                        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                })
            && (mode == OutputMode::Resume || !output_dir.exists()),
        "fixed reconstruction attempt required (existing outputs need explicit resume mode)",
    )?;
    if mode == OutputMode::Resume {
        partial_output_layout(output_dir)?;
    }
    // Actual proof verification remains in this process. No serialized success
    // report or archived receipt authorizes any database writes.
    let evidence = inspect(evidence_output)?;
    let database = root.join(DB_REL);
    let before = inventory(&database)?;
    let admission = read(
        &root.join("fixtures/retained-client-database-admission-v1/content-inventory.json"),
        64 * 1024 * 1024,
    )?;
    need(
        hash(&admission) == "f94ba1f743cc759459a574e34f16725df72100dbb7bc95d3df9c1e5aa59c47a3"
            && serde_json::from_slice::<Value>(&admission).map_err(|e| e.to_string())?["files"]
                == json!(before),
        "source changed after evidence verification",
    )?;
    let parent_capture = root.join("history/captures/28191816");
    let parent_pins = capture_pins(&parent_capture, PARENT_MANIFEST)?;
    let checkpoint_pins = capture_pins(&root.join("checkpoint"), CHECKPOINT_MANIFEST)?;
    let (_, parent_state) = verify_capture_at(
        &parent_capture,
        CHECKPOINT - 1,
        h256(PARENT_HASH)?,
        h256(PARENT_ROOT)?,
    )?;
    need(
        parent_state.children_default.is_empty(),
        "fixed predecessor corridor requires child-free captured state",
    )?;
    let expected_state = state_digest(parent_state.top.iter());
    fs::create_dir_all(&allowed).map_err(|e| e.to_string())?;
    need(
        fs::canonicalize(&allowed).map_err(|e| e.to_string())? == allowed,
        "aliased reconstruction parent",
    )?;
    let source_bytes = before.values().try_fold(0u64, |sum, row| {
        sum.checked_add(row["size"].as_u64().ok_or("source file size")?)
            .ok_or("source size overflow")
    })?;
    let capture_bytes = parent_pins.keys().try_fold(0u64, |sum, path| {
        sum.checked_add(fs::metadata(path).map_err(|e| e.to_string())?.len())
            .ok_or("capture size overflow".to_string())
    })?;
    let required = source_bytes
        .checked_add(capture_bytes)
        .and_then(|n| n.checked_mul(2))
        .and_then(|n| n.checked_add(8 * 1024 * 1024 * 1024))
        .ok_or("space estimate overflow")?;
    let available = fs2::available_space(&allowed).map_err(|e| e.to_string())?;
    need(available>=required,&format!("reconstruction needs {required} available bytes including compaction margin; found {available}"))?;
    let mut logs = output_dir.join("source-read-only-logs");
    if mode == OutputMode::Fresh {
        fs::create_dir(output_dir).map_err(|e| e.to_string())?;
        fs::create_dir(&logs).map_err(|e| e.to_string())?;
    } else {
        // Preserve earlier read-only logs, including those of an interrupted run.
        logs = logs.join(format!(
            "resume-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        ));
        fs::create_dir(&logs).map_err(|e| e.to_string())?;
    }
    let mut options = Options::default();
    options.create_if_missing(false);
    options.create_missing_column_families(false);
    options.set_db_log_dir(&logs);
    let families = DB::list_cf(&options, &database).map_err(|e| e.to_string())?;
    let source = DB::open_cf_for_read_only(&options, &database, families, false)
        .map_err(|e| e.to_string())?;
    let (parent, _) = header_at(&source, CHECKPOINT - 1)?;
    need(
        parent.hash() == h256(PARENT_HASH)? && *parent.state_root() == h256(PARENT_ROOT)?,
        "parent anchor changed",
    )?;
    let grandpa = get(&source, 8, b"grandpa_voters")?;
    let (set, keys, last_boundary) = unchanged_parent_grandpa(&grandpa, &parent_state)?;
    let (babe, babe_report) = parent_epochs(&parent_state, &parent)?;
    let mut auxiliary = vec![
        (b"grandpa_schema_version".to_vec(), Some(3u32.encode())),
        (b"grandpa_voters".to_vec(), Some(grandpa)),
        (b"babe_epoch_changes_version".to_vec(), Some(3u32.encode())),
        (b"babe_epoch_changes".to_vec(), Some(babe)),
    ];
    // No old voter rounds, local signing state, offchain storage, recovery marker,
    // descendant best-justification or arbitrary auxiliary key is copied.
    let db_path = output_dir.join("chains/sora-nexus-recovery-predecessor/db/full");
    let genesis = h256(GENESIS)?;
    let raw_tip = if mode == OutputMode::Resume {
        let partial = readonly_output(&db_path, &logs)?;
        let cursor = raw_cursor(&partial, genesis, CHECKPOINT - 1)?;
        drop(partial);
        cursor
    } else {
        fs::create_dir_all(db_path.parent().ok_or("output database parent")?)
            .map_err(|e| e.to_string())?;
        need(!db_path.exists(), "database output already exists")?;
        None
    };
    // This obtains the SDK/RocksDB exclusive writer lock. An active reconstruction
    // cannot be resumed or interfered with by this process.
    let backend = open_backend(&db_path)?;
    let initial = backend.blockchain().info();
    let initial_tip = sdk_cursor(&initial, genesis, CHECKPOINT - 1)?;
    need(
        initial_tip == raw_tip,
        "partial metadata changed before exclusive SDK open",
    )?;
    let mut progress = HistoryProgress::default();
    if mode == OutputMode::Resume {
        // No writes occur while this read-only snapshot is open. SDK metadata is
        // checked again, then every persisted record is compared with source.
        let partial = readonly_output(&db_path, &logs)?;
        need(
            raw_cursor(&partial, genesis, CHECKPOINT - 1)? == initial_tip,
            "partial raw/SDK metadata differ",
        )?;
        if let Some((_, tip_hash)) = initial_tip {
            need(
                backend.blockchain().leaves().map_err(|e| e.to_string())? == vec![tip_hash],
                "partial leaf set differs",
            )?;
        }
        progress = rebuild_prefix(
            &partial,
            initial_tip,
            |number| history_record(&source, number),
            PrefixContext {
                genesis,
                set,
                authorities: &keys,
                last_boundary,
            },
        )?;
        let completed = initial_tip.is_some_and(|(number, _)| number == CHECKPOINT - 1);
        let mut expected_aux = auxiliary.clone();
        if let Some(raw) = progress.last_justification.as_ref() {
            expected_aux.push((b"grandpa_best_justification".to_vec(), Some(raw.clone())));
        }
        if completed {
            for (key, value) in &expected_aux {
                need(
                    optional(&partial, 8, key)? == *value,
                    "completed partial auxiliary bytes differ",
                )?;
            }
        }
        prefix_shape(
            &partial,
            progress.bodies,
            progress.justifications,
            completed,
            if completed {
                expected_aux.len() as u64
            } else {
                0
            },
        )?;
        drop(partial);
        eprintln!(
            "retained predecessor resume: full existing prefix authenticated; next block {}",
            progress.bodies
        );
    }
    let mut state = Some(parent_state);
    for number in (progress.bodies as u32)..CHECKPOINT {
        let record = history_record(&source, number)?;
        progress.observe(number, &record, genesis, set, &keys, last_boundary)?;
        let (header, body, just) = record;
        let mut operation = backend.begin_operation().map_err(|e| e.to_string())?;
        if number == CHECKPOINT - 1 {
            need(
                header.hash() == h256(PARENT_HASH)?,
                "complete retained chain does not reach pinned parent",
            )?;
            let root = operation
                .reset_storage(state.take().ok_or("parent state reused")?, StateVersion::V0)
                .map_err(|e| e.to_string())?;
            need(
                root == *header.state_root(),
                "SDK reset_storage parent root differs",
            )?;
            if let Some(raw) = progress.last_justification.as_ref() {
                auxiliary.push((b"grandpa_best_justification".to_vec(), Some(raw.clone())));
            }
            operation
                .insert_aux(auxiliary.clone())
                .map_err(|e| e.to_string())?;
        }
        operation
            .set_block_data(header, Some(body), None, just, NewBlockState::Final, true)
            .map_err(|e| e.to_string())?;
        backend
            .commit_operation(operation)
            .map_err(|e| format!("SDK import {number}: {e}"))?;
        if number % 100_000 == 0 {
            eprintln!("retained predecessor reconstruction: imported {number}/{}, no consensus or runtime replay",CHECKPOINT-1);
        }
    }
    // When the final transaction already committed, preserve it and re-run all
    // persisted-state checks below instead of resetting or silently repairing it.
    if initial_tip.is_some_and(|(number, _)| number == CHECKPOINT - 1) {
        if let Some(raw) = progress.last_justification.as_ref() {
            auxiliary.push((b"grandpa_best_justification".to_vec(), Some(raw.clone())));
        }
    }
    let bodies = progress.bodies;
    let justifications = progress.justifications;
    let imported_digest = hex::encode(progress.digest.finalize());
    drop(backend);
    // Reopen using the SDK and verify every stored canonical record. This checks
    // persisted history, not merely the import loop's in-memory inputs.
    let reopened = open_backend(&db_path)?;
    let info = reopened.blockchain().info();
    need(
        info.genesis_hash == h256(GENESIS)?
            && info.best_hash == h256(PARENT_HASH)?
            && info.best_number == CHECKPOINT - 1
            && info.finalized_hash == h256(PARENT_HASH)?
            && info.finalized_number == CHECKPOINT - 1
            && info.block_gap.is_none(),
        "reopened predecessor metadata differs",
    )?;
    need(
        reopened
            .blockchain()
            .hash(CHECKPOINT)
            .map_err(|e| e.to_string())?
            .is_none()
            && reopened
                .blockchain()
                .header(h256(CHECKPOINT_HASH)?)
                .map_err(|e| e.to_string())?
                .is_none(),
        "checkpoint already imported into output",
    )?;
    let mut persisted = Sha256::new();
    for number in 0..CHECKPOINT {
        let hash = reopened
            .blockchain()
            .hash(number)
            .map_err(|e| e.to_string())?
            .ok_or("persisted canonical gap")?;
        let h = reopened
            .blockchain()
            .header(hash)
            .map_err(|e| e.to_string())?
            .ok_or("persisted header gap")?;
        let b = reopened
            .blockchain()
            .body(hash)
            .map_err(|e| e.to_string())?
            .ok_or("persisted body gap")?;
        let j = reopened
            .blockchain()
            .justifications(hash)
            .map_err(|e| e.to_string())?
            .map(|v| v.encode());
        need(
            h.hash() == hash && *h.number() == number,
            "persisted header/hash lookup differs",
        )?;
        record_digest(
            &mut persisted,
            number,
            &h.encode(),
            &b.encode(),
            j.as_deref(),
        );
    }
    need(
        hex::encode(persisted.finalize()) == imported_digest,
        "persisted full canonical history differs",
    )?;
    let restored = reopened
        .state_at(h256(PARENT_HASH)?, TrieCacheContext::Untrusted)
        .map_err(|e| e.to_string())?;
    let mut state_sha = Sha256::new();
    let mut count = 0;
    let mut previous_key: Option<Vec<u8>> = None;
    for row in restored
        .pairs(IterArgs::default())
        .map_err(|e| e.to_string())?
    {
        let (key, value) = row.map_err(|e| e.to_string())?;
        need(
            previous_key.as_ref().is_none_or(|p| p < &key),
            "persisted state ordering differs",
        )?;
        state_sha.update((key.len() as u64).to_le_bytes());
        state_sha.update(&key);
        state_sha.update((value.len() as u64).to_le_bytes());
        state_sha.update(&value);
        previous_key = Some(key);
        count += 1;
    }
    need(
        (count, hex::encode(state_sha.finalize())) == expected_state,
        "persisted complete parent state differs",
    )?;
    drop(restored);
    for (key, value) in &auxiliary {
        need(
            AuxStore::get_aux(&reopened, key).map_err(|e| e.to_string())? == *value,
            "reopened auxiliary bytes differ",
        )?;
    }
    for key in [
        b"grandpa_completed_round".as_slice(),
        b"grandpa_concluded_rounds".as_slice(),
    ] {
        need(
            AuxStore::get_aux(&reopened, key)
                .map_err(|e| e.to_string())?
                .is_none(),
            "old local GRANDPA voting state present",
        )?;
    }
    drop(reopened);
    drop(source);
    need(
        inventory(&database)? == before,
        "source DB changed during reconstruction",
    )?;
    verify_files(&parent_pins)?;
    verify_files(&checkpoint_pins)?;
    need(
        read(
            &root.join("fixtures/retained-client-database-admission-v1/content-inventory.json"),
            64 * 1024 * 1024,
        )? == admission,
        "source admission changed",
    )?;
    let mut evidence_pins = BTreeMap::new();
    for (name, row) in evidence["artifacts"]
        .as_object()
        .ok_or("evidence artifacts")?
    {
        evidence_pins.insert(
            evidence_output.join(name),
            row["sha256"].as_str().ok_or("evidence hash")?.to_owned(),
        );
    }
    for row in evidence["headers"].as_array().ok_or("evidence headers")? {
        evidence_pins.insert(
            evidence_output.join("headers").join(
                row["header"]["file"]
                    .as_str()
                    .ok_or("header evidence path")?,
            ),
            row["header"]["sha256"]
                .as_str()
                .ok_or("header evidence hash")?
                .to_owned(),
        );
    }
    verify_files(&evidence_pins)?;
    let output_inventory = inventory(&db_path)?;
    let receipt = json!({"schema":"sora-nexus-retained-predecessor-reconstruction-v1","database":db_path,
        "resume":{"requested":mode == OutputMode::Resume,"initialBestNumber":initial_tip.map(|(n,_)|n),
            "initialBestHash":initial_tip.map(|(_,h)|format!("{h:#x}")),
            "existingPrefixComparedWithAuthenticatedSource":mode == OutputMode::Resume,
            "cursorFromPersistedSdkMetadata":mode == OutputMode::Resume,
            "finalStateTransactionReused":initial_tip.is_some_and(|(n,_)|n == CHECKPOINT-1)},
        "evidence":evidence,"sourceDatabaseUnchanged":true,"sourceDatabaseFiles":before,
        "outputDatabaseFiles":output_inventory,"canonicalHistorySha256":imported_digest,
        "historicalHeaderBodyCount":bodies,"retainedJustificationRecords":justifications,
        "parentStateKeys":expected_state.0,"parentStateSha256":expected_state.1,
        "bestNumber":CHECKPOINT-1,"bestHash":format!("0x{PARENT_HASH}"),"stateRoot":format!("0x{PARENT_ROOT}"),
        "clientDatabaseReconstructed":true,"fullCanonicalHistoryPersisted":true,
        "parentStateReopenedAndCompletelyCompared":true,"checkpointAbsent":true,
        "grandpaHistoricalAuthorityTransitionEvidenceVerified":false,
        "diskSpacePreflight":{"requiredBytes":required,"availableBytes":available},
        "grandpaCurrentSetId":set,"grandpaPendingChangesEmpty":true,"grandpaCachePreservedUnchanged":true,
        "babe":babe_report,"noOldGrandpaVoteStateCopied":true,"noOldKeysCopied":true,
        "historicalStateArchiveReconstructed":false,"genesisAuthorityChainVerified":false,
        "fullBabeVerification":false,"nodeStartupReady":false,"runtimeExecuted":false,
        "fullWarpSynchronizationVerified":false,"operatorApprovalVerified":false,"productionReady":false});
    let raw = serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?;
    let receipt_path = output_dir.join("RECONSTRUCTION.json");
    let pin = output(&receipt_path, &raw)?;
    need(
        inventory(&db_path)? == output_inventory,
        "output DB changed before receipt registration",
    )?;
    need(
        inventory(&database)? == before,
        "source DB changed during final output registration",
    )?;
    verify_files(&parent_pins)?;
    verify_files(&checkpoint_pins)?;
    verify_files(&evidence_pins)?;
    need(
        read(
            &root.join("fixtures/retained-client-database-admission-v1/content-inventory.json"),
            64 * 1024 * 1024,
        )? == admission,
        "source admission changed during output registration",
    )?;
    verify_files(&BTreeMap::from([(receipt_path, hash(&raw))]))?;
    Ok(
        json!({"schema":"sora-nexus-retained-predecessor-completion-v1","receipt":pin,"output":output_dir,
        "clientDatabaseReconstructed":true,"nodeStartupReady":false,"productionReady":false}),
    )
}

/// BABE imports a block only when its parent has a recorded chain weight. The SDK's
/// state import records weight 0 where it resets the epoch cache; the reconstruction
/// performs that reset at the parent without the weight, so the checkpoint cannot be
/// imported. Run on an owned copy of a completed reconstruction, this adds only that
/// entry. Fork choice compares descendants of the parent, so the base value never
/// changes a choice.
pub(super) fn seed_state_sync_weight(db_path: &Path) -> Result<Value> {
    // A writable SDK open creates a missing database, so require the existing one first.
    owned_directory(db_path)?;
    need(
        read(&db_path.join("db_version"), 16)? == b"4",
        "predecessor SDK schema differs",
    )?;
    let parent = h256(PARENT_HASH)?;
    let backend = open_backend(db_path)?;
    let info = backend.blockchain().info();
    need(
        info.genesis_hash == h256(GENESIS)?
            && info.best_hash == parent
            && info.best_number == CHECKPOINT - 1
            && info.finalized_hash == parent
            && info.finalized_number == CHECKPOINT - 1
            && info.block_gap.is_none()
            && backend.blockchain().leaves().map_err(|e| e.to_string())? == vec![parent],
        "database is not a completed predecessor",
    )?;
    need(
        backend
            .blockchain()
            .hash(CHECKPOINT)
            .map_err(|e| e.to_string())?
            .is_none()
            && backend
                .blockchain()
                .header(h256(CHECKPOINT_HASH)?)
                .map_err(|e| e.to_string())?
                .is_none(),
        "checkpoint already imported",
    )?;
    let header = backend
        .blockchain()
        .header(parent)
        .map_err(|e| e.to_string())?
        .ok_or("parent header missing")?;
    need(
        *header.state_root() == h256(PARENT_ROOT)?,
        "parent state root differs",
    )?;
    need(
        AuxStore::get_aux(&backend, b"babe_epoch_changes_version").map_err(|e| e.to_string())?
            == Some(3u32.encode()),
        "BABE epoch cache schema differs",
    )?;
    let cache = AuxStore::get_aux(&backend, b"babe_epoch_changes")
        .map_err(|e| e.to_string())?
        .ok_or("BABE epoch cache missing")?;
    let _: BabeCache = decode(&cache)?;
    need(
        sc_consensus_babe::block_weight(&backend, parent)
            .map_err(|e| e.to_string())?
            .is_none(),
        "parent BABE weight already recorded",
    )?;
    let key = sc_consensus_babe::aux_schema::block_weight_key(parent);
    AuxStore::insert_aux(&backend, &[(key.as_slice(), 0u32.encode().as_slice())], &[])
        .map_err(|e| e.to_string())?;
    drop(backend);
    let reopened = open_backend(db_path)?;
    let info = reopened.blockchain().info();
    need(
        sc_consensus_babe::block_weight(&reopened, parent).map_err(|e| e.to_string())? == Some(0)
            && info.best_hash == parent
            && info.finalized_hash == parent
            && AuxStore::get_aux(&reopened, b"babe_epoch_changes").map_err(|e| e.to_string())?
                == Some(cache),
        "SDK does not read the seeded parent weight beside the unchanged epoch cache",
    )?;
    Ok(
        json!({"schema":"sora-nexus-predecessor-babe-state-sync-weight-v1","database":db_path,
        "parentHash":format!("{parent:#x}"),"auxKey":format!("0x{}",hex::encode(&key)),"weight":0,
        "method":"SDK BabeBlockImport::import_state weight reset, beside the reconstruction's EpochChanges::reset",
        "readBackThroughSdk":true,"checkpointAbsent":true,"productionReady":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "sora-nexus-predecessor-resume-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&directory).unwrap();
            Self(fs::canonicalize(directory).unwrap())
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn signed_proof(
        header: &ChainHeader,
        set: u64,
        pair: &sp_consensus_grandpa::AuthorityPair,
    ) -> Vec<u8> {
        let precommit = sp_consensus_grandpa::Precommit::<ChainHeader> {
            target_hash: header.hash(),
            target_number: *header.number(),
        };
        let message = sp_consensus_grandpa::Message::<ChainHeader>::Precommit(precommit.clone());
        let signature = pair.sign(&sp_consensus_grandpa::localized_payload(5, set, &message));
        let commit = decode::<sp_consensus_grandpa::Commit<ChainHeader>>(
            &(
                header.hash(),
                *header.number(),
                vec![(precommit, signature, pair.public())],
            )
                .encode(),
        )
        .unwrap();
        sp_consensus_grandpa::GrandpaJustification::<ChainHeader> {
            round: 5,
            commit,
            votes_ancestries: Vec::new(),
        }
        .encode()
    }

    fn prefix_fixture() -> (Vec<HistoryRecord>, Storage, AuthorityList) {
        let state = Storage {
            top: BTreeMap::from([(b"key".to_vec(), b"value".to_vec())]),
            ..Default::default()
        };
        let root =
            sp_io::trie::blake2_256_root(state.top.clone().into_iter().collect(), StateVersion::V0);
        let body_root = sp_io::trie::blake2_256_ordered_root(Vec::new(), StateVersion::V0);
        let pair = sp_consensus_grandpa::AuthorityPair::from_seed(&[31; 32]);
        let mut parent = H256::zero();
        let records = (0..3)
            .map(|number| {
                let header = ChainHeader::new(
                    number,
                    body_root,
                    if number == 2 {
                        root
                    } else {
                        H256::repeat_byte(7)
                    },
                    parent,
                    Default::default(),
                );
                parent = header.hash();
                let proofs = (number > 0).then(|| {
                    Justifications::from((
                        sp_consensus_grandpa::GRANDPA_ENGINE_ID,
                        signed_proof(&header, 8, &pair),
                    ))
                });
                (header, Vec::new(), proofs)
            })
            .collect();
        (records, state, vec![(pair.public(), 1)])
    }

    fn import_fixture(
        backend: &Backend<ChainBlock>,
        record: &HistoryRecord,
        state: Option<Storage>,
        aux: Option<Vec<u8>>,
    ) {
        let mut operation = backend.begin_operation().unwrap();
        if let Some(state) = state {
            assert_eq!(
                operation.reset_storage(state, StateVersion::V0).unwrap(),
                *record.0.state_root()
            );
        }
        if let Some(raw) = aux {
            operation
                .insert_aux(vec![(b"grandpa_best_justification".to_vec(), Some(raw))])
                .unwrap();
        }
        operation
            .set_block_data(
                record.0.clone(),
                Some(record.1.clone()),
                None,
                record.2.clone(),
                NewBlockState::Final,
                true,
            )
            .unwrap();
        backend.commit_operation(operation).unwrap();
    }

    #[test]
    fn resume_rebuilds_digest_and_latest_proof_before_finishing_atomic_state_transaction() {
        let directory = TestDirectory::new();
        let db_path = directory.0.join("db");
        let logs = directory.0.join("logs");
        fs::create_dir(&logs).unwrap();
        let (records, state, keys) = prefix_fixture();
        let genesis = records[0].0.hash();
        let backend = open_backend(&db_path).unwrap();
        import_fixture(&backend, &records[0], None, None);
        import_fixture(&backend, &records[1], None, None);
        // Interrupt before committing the last state+block operation. It must not
        // advance the stored cursor or make any partial state visible.
        let mut abandoned = backend.begin_operation().unwrap();
        abandoned
            .reset_storage(state.clone(), StateVersion::V0)
            .unwrap();
        abandoned
            .set_block_data(
                records[2].0.clone(),
                Some(Vec::new()),
                None,
                records[2].2.clone(),
                NewBlockState::Final,
                true,
            )
            .unwrap();
        drop(abandoned);
        drop(backend);
        let backend = open_backend(&db_path).unwrap();
        let tip = sdk_cursor(&backend.blockchain().info(), genesis, 2).unwrap();
        assert_eq!(tip, Some((1, records[1].0.hash())));
        let raw = readonly_output(&db_path, &logs).unwrap();
        assert_eq!(raw_cursor(&raw, genesis, 2).unwrap(), tip);
        let mut progress = rebuild_prefix(
            &raw,
            tip,
            |n| Ok(records[n as usize].clone()),
            PrefixContext {
                genesis,
                set: 8,
                authorities: &keys,
                last_boundary: 0,
            },
        )
        .unwrap();
        prefix_shape(&raw, 2, 1, false, 0).unwrap();
        let first_proof = records[1]
            .2
            .as_ref()
            .unwrap()
            .get(sp_consensus_grandpa::GRANDPA_ENGINE_ID)
            .unwrap();
        assert_eq!(progress.last_justification.as_ref(), Some(first_proof));
        assert_eq!(progress.bodies, 2);
        assert!(rebuild_prefix(
            &raw,
            tip,
            |n| Ok(records[n as usize].clone()),
            PrefixContext {
                genesis,
                set: 9,
                authorities: &keys,
                last_boundary: 0
            }
        )
        .is_err());
        drop(raw);
        progress
            .observe(2, &records[2], genesis, 8, &keys, 0)
            .unwrap();
        import_fixture(
            &backend,
            &records[2],
            Some(state.clone()),
            progress.last_justification.clone(),
        );
        drop(backend);
        // An interruption after final commit is verification-only: all records,
        // final state and recomputed proof survive without another reset_storage.
        let backend = open_backend(&db_path).unwrap();
        let tip = sdk_cursor(&backend.blockchain().info(), genesis, 2).unwrap();
        assert_eq!(tip, Some((2, records[2].0.hash())));
        let raw = readonly_output(&db_path, &logs).unwrap();
        assert_eq!(raw_cursor(&raw, genesis, 2).unwrap(), tip);
        let rebuilt = rebuild_prefix(
            &raw,
            tip,
            |n| Ok(records[n as usize].clone()),
            PrefixContext {
                genesis,
                set: 8,
                authorities: &keys,
                last_boundary: 0,
            },
        )
        .unwrap();
        prefix_shape(&raw, 3, 2, true, 1).unwrap();
        assert_eq!(rebuilt.digest.finalize(), progress.digest.finalize());
        assert_eq!(
            optional(&raw, 8, b"grandpa_best_justification").unwrap(),
            rebuilt.last_justification
        );
        let restored = backend
            .state_at(records[2].0.hash(), TrieCacheContext::Untrusted)
            .unwrap();
        assert_eq!(
            restored
                .pairs(IterArgs::default())
                .unwrap()
                .collect::<std::result::Result<BTreeMap<_, _>, _>>()
                .unwrap(),
            state.top
        );
        drop(restored);
        drop(raw);
        drop(backend);
    }

    #[test]
    fn resume_rejects_gaps_foreign_rows_and_nonatomic_final_metadata() {
        let directory = TestDirectory::new();
        let db_path = directory.0.join("db");
        let logs = directory.0.join("logs");
        fs::create_dir(&logs).unwrap();
        let (records, _, keys) = prefix_fixture();
        let genesis = records[0].0.hash();
        let backend = open_backend(&db_path).unwrap();
        import_fixture(&backend, &records[0], None, None);
        import_fixture(&backend, &records[1], None, None);
        drop(backend);
        let options = Options::default();
        let families = DB::list_cf(&options, &db_path).unwrap();
        let writable = DB::open_cf(&options, &db_path, families).unwrap();
        for column in [3, 4, 5, 6, 8, 9, 11, 12] {
            let cf = writable.cf_handle(&format!("col{column}")).unwrap();
            writable.put_cf(cf, b"foreign", b"record").unwrap();
            assert!(prefix_shape(&writable, 2, 1, false, 0).is_err());
            writable.delete_cf(cf, b"foreign").unwrap();
        }
        let cf = writable.cf_handle("col0").unwrap();
        writable.put_cf(cf, b"gap", [0]).unwrap();
        assert!(raw_cursor(&writable, genesis, 2).is_err());
        writable.delete_cf(cf, b"gap").unwrap();
        assert!(raw_cursor(&writable, H256::repeat_byte(1), 2).is_err());
        assert!(raw_cursor(&writable, genesis, 0).is_err());
        let mut lookup = 1u32.to_be_bytes().to_vec();
        lookup.extend(records[1].0.hash().as_bytes());
        writable.put_cf(cf, b"fstate", &lookup).unwrap();
        assert!(raw_cursor(&writable, genesis, 2).is_err());
        writable.delete_cf(cf, b"fstate").unwrap();
        let body_cf = writable.cf_handle("col5").unwrap();
        writable.delete_cf(body_cf, &lookup).unwrap();
        assert!(rebuild_prefix(
            &writable,
            Some((1, records[1].0.hash())),
            |n| Ok(records[n as usize].clone()),
            PrefixContext {
                genesis,
                set: 8,
                authorities: &keys,
                last_boundary: 0
            }
        )
        .is_err());
        writable
            .put_cf(body_cf, &lookup, records[1].1.encode())
            .unwrap();
        drop(writable);
        let backend = open_backend(&db_path).unwrap();
        import_fixture(&backend, &records[2], None, None);
        assert!(sdk_cursor(&backend.blockchain().info(), genesis, 2).is_err());
        let raw = readonly_output(&db_path, &logs).unwrap();
        assert!(raw_cursor(&raw, genesis, 2).is_err());
        drop(raw);
        drop(backend);
    }

    #[test]
    fn prefix_comparison_does_not_trust_a_matching_header_alone() {
        let (records, _, _) = prefix_fixture();
        let expected = &records[1];
        let mut changed = expected.clone();
        changed.2 = None;
        assert!(compare_record(expected, &changed).is_err());
        changed = expected.clone();
        changed.0.set_state_root(H256::repeat_byte(92));
        assert!(compare_record(expected, &changed).is_err());
    }

    #[test]
    fn resume_layout_rejects_completion_receipts_and_foreign_or_aliased_paths() {
        let directory = TestDirectory::new();
        fs::create_dir_all(
            directory
                .0
                .join("chains/sora-nexus-recovery-predecessor/db/full"),
        )
        .unwrap();
        fs::create_dir(directory.0.join("source-read-only-logs")).unwrap();
        fs::write(
            directory
                .0
                .join("chains/sora-nexus-recovery-predecessor/db/full/db_version"),
            b"4",
        )
        .unwrap();
        partial_output_layout(&directory.0).unwrap();
        fs::write(directory.0.join("RECONSTRUCTION.json"), b"{").unwrap();
        assert!(partial_output_layout(&directory.0).is_err());
        fs::remove_file(directory.0.join("RECONSTRUCTION.json")).unwrap();
        fs::write(
            directory.0.join("untrusted-progress.json"),
            b"{\"success\":true}",
        )
        .unwrap();
        assert!(partial_output_layout(&directory.0).is_err());
        fs::remove_file(directory.0.join("untrusted-progress.json")).unwrap();
        let real = directory
            .0
            .join("chains/sora-nexus-recovery-predecessor/db/full/db_version");
        let link = directory
            .0
            .join("chains/sora-nexus-recovery-predecessor/db/full/aliased");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(partial_output_layout(&directory.0).is_err());
    }

    #[test]
    fn sdk_can_import_state_only_at_end_of_a_complete_block_prefix() {
        let directory = std::env::temp_dir().join(format!(
            "sora-nexus-predecessor-sdk-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("db");
        let backend = open_backend(&path).unwrap();
        let top = BTreeMap::from([(b"key".to_vec(), b"value".to_vec())]);
        let root =
            sp_io::trie::blake2_256_root(top.clone().into_iter().collect(), StateVersion::V0);
        let empty_body = sp_io::trie::blake2_256_ordered_root(Vec::new(), StateVersion::V0);
        let mut parent = H256::zero();
        for number in 0..3 {
            let header = ChainHeader::new(
                number,
                empty_body,
                if number == 2 {
                    root
                } else {
                    H256::repeat_byte(7)
                },
                parent,
                Default::default(),
            );
            let mut operation = backend.begin_operation().unwrap();
            if number == 2 {
                assert_eq!(
                    operation
                        .reset_storage(
                            Storage {
                                top: top.clone(),
                                ..Default::default()
                            },
                            StateVersion::V0
                        )
                        .unwrap(),
                    root
                );
            }
            parent = header.hash();
            operation
                .set_block_data(
                    header,
                    Some(Vec::new()),
                    None,
                    None,
                    NewBlockState::Final,
                    true,
                )
                .unwrap();
            backend.commit_operation(operation).unwrap();
        }
        drop(backend);
        let reopened = open_backend(&path).unwrap();
        assert_eq!(reopened.blockchain().info().best_number, 2);
        assert_eq!(reopened.blockchain().info().finalized_hash, parent);
        assert!(reopened.blockchain().info().block_gap.is_none());
        for number in 0..3 {
            let hash = reopened.blockchain().hash(number).unwrap().unwrap();
            assert!(reopened.blockchain().body(hash).unwrap().is_some());
        }
        let state = reopened
            .state_at(parent, TrieCacheContext::Untrusted)
            .unwrap();
        assert_eq!(
            state
                .pairs(IterArgs::default())
                .unwrap()
                .collect::<std::result::Result<BTreeMap<_, _>, _>>()
                .unwrap(),
            top
        );
        drop(state);
        drop(reopened);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn large_body_decoder_rejects_trailing_and_invalid_lengths() {
        assert_eq!(
            body_decode::<Vec<u8>>(&vec![1u8, 2].encode()).unwrap(),
            vec![1, 2]
        );
        let mut raw = vec![1u8, 2].encode();
        raw.push(0);
        assert!(body_decode::<Vec<u8>>(&raw).is_err());
        assert!(body_decode::<Vec<u8>>(&[]).is_err());
    }
    #[test]
    fn record_hash_separates_absent_and_empty_justifications() {
        let mut a = Sha256::new();
        let mut b = Sha256::new();
        record_digest(&mut a, 1, b"a", b"b", None);
        record_digest(&mut b, 1, b"a", b"b", Some(&[]));
        assert_ne!(a.finalize(), b.finalize());
    }
    #[test]
    fn record_hash_binds_height_and_field_boundaries() {
        let mut a = Sha256::new();
        let mut b = Sha256::new();
        let mut c = Sha256::new();
        record_digest(&mut a, 1, b"a", b"bc", None);
        record_digest(&mut b, 1, b"ab", b"c", None);
        record_digest(&mut c, 2, b"a", b"bc", None);
        let a = a.finalize();
        assert_ne!(a, b.finalize());
        assert_ne!(a, c.finalize());
    }
    #[test]
    fn state_digest_binds_keys_values_order_and_count() {
        let a = [(vec![1], vec![2, 3])];
        let b = [(vec![1, 2], vec![3])];
        assert_ne!(
            state_digest(a.iter().map(|(k, v)| (k, v))),
            state_digest(b.iter().map(|(k, v)| (k, v)))
        );
    }
    fn grandpa_case(finalized: u32) -> (Vec<u8>, Storage) {
        let key: sp_consensus_grandpa::AuthorityId = decode(&[1u8; 32]).unwrap();
        let keys = vec![(key, 1u64)];
        let set = 2u64;
        let raw = (
            keys.clone(),
            set,
            (Vec::<()>::new(), Some(finalized)),
            Vec::<()>::new(),
            vec![(0u64, 1u32), (1, 2)],
        )
            .encode();
        let mut state = Storage::default();
        state
            .top
            .insert(storage_key("Grandpa", "Authorities"), keys.encode());
        state
            .top
            .insert(storage_key("Grandpa", "CurrentSetId"), set.encode());
        (raw, state)
    }
    #[test]
    fn unchanged_grandpa_cache_requires_predecessor_bounded_tree() {
        let (raw, state) = grandpa_case(2);
        assert!(unchanged_parent_grandpa(&raw, &state).is_ok());
        let (raw, state) = grandpa_case(CHECKPOINT);
        assert!(unchanged_parent_grandpa(&raw, &state).is_err());
    }
    #[test]
    fn unconfirmed_grandpa_pending_and_identity_are_rejected() {
        let (raw, mut state) = grandpa_case(2);
        state
            .top
            .insert(storage_key("Grandpa", "PendingChange"), vec![0]);
        assert!(unchanged_parent_grandpa(&raw, &state).is_err());
        state.top.remove(&storage_key("Grandpa", "PendingChange"));
        state
            .top
            .insert(storage_key("Grandpa", "CurrentSetId"), 3u64.encode());
        assert!(unchanged_parent_grandpa(&raw, &state).is_err());
    }
    #[test]
    fn weight_seed_never_creates_or_writes_a_foreign_database() {
        let directory = TestDirectory::new();
        let missing = directory.0.join("missing");
        assert!(seed_state_sync_weight(&missing).is_err());
        assert!(!missing.exists());
        let foreign = directory.0.join("foreign");
        drop(open_backend(&foreign).unwrap());
        assert!(seed_state_sync_weight(&foreign)
            .unwrap_err()
            .contains("not a completed predecessor"));
        let reopened = open_backend(&foreign).unwrap();
        assert_eq!(
            sc_consensus_babe::block_weight(&reopened, h256(PARENT_HASH).unwrap()).unwrap(),
            None
        );
    }
}
