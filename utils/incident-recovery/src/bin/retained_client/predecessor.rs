//! Fixed, fresh-output predecessor reconstruction. Never rewinds or edits the source DB.
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

pub(super) fn run(evidence_output: &Path, output_dir: &Path) -> Result<Value> {
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
            && !output_dir.exists(),
        "fresh fixed reconstruction attempt required",
    )?;
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
    need(available>=required,&format!("fresh reconstruction needs {required} available bytes including compaction margin; found {available}"))?;
    fs::create_dir(output_dir).map_err(|e| e.to_string())?;
    let logs = output_dir.join("source-read-only-logs");
    fs::create_dir(&logs).map_err(|e| e.to_string())?;
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
    fs::create_dir_all(db_path.parent().ok_or("output database parent")?)
        .map_err(|e| e.to_string())?;
    need(!db_path.exists(), "database output already exists")?;
    let backend = open_backend(&db_path)?;
    let initial = backend.blockchain().info();
    need(
        initial.best_hash == H256::zero() && initial.finalized_hash == H256::zero(),
        "SDK output was not empty",
    )?;
    let mut previous = H256::zero();
    let mut digest = Sha256::new();
    let mut bodies = 0u64;
    let mut justifications = 0u64;
    let mut last_justification = None;
    let mut state = Some(parent_state);
    for number in 0..CHECKPOINT {
        let (header, body, just) = history_record(&source, number)?;
        need(
            *header.parent_hash() == previous && (number != 0 || header.hash() == h256(GENESIS)?),
            "canonical retained history ancestry mismatch",
        )?;
        if let Some(proofs) = &just {
            justifications += 1;
            if number > last_boundary {
                if let Some(raw) = proofs.get(sp_consensus_grandpa::GRANDPA_ENGINE_ID) {
                    let proof: GrandpaJustification<ChainBlock> = decode(raw)?;
                    need(
                        proof.target() == (number, header.hash()),
                        "stored justification target differs",
                    )?;
                    proof.verify(set, &keys).map_err(|e| e.to_string())?;
                    last_justification = Some(raw.clone());
                }
            }
        }
        let raw_header = header.encode();
        let raw_body = body.encode();
        let raw_just = just.as_ref().map(Encode::encode);
        record_digest(
            &mut digest,
            number,
            &raw_header,
            &raw_body,
            raw_just.as_deref(),
        );
        bodies += 1;
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
            if let Some(raw) = last_justification.take() {
                auxiliary.push((b"grandpa_best_justification".to_vec(), Some(raw)));
            }
            operation
                .insert_aux(auxiliary.clone())
                .map_err(|e| e.to_string())?;
        }
        previous = header.hash();
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
    let imported_digest = hex::encode(digest.finalize());
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
