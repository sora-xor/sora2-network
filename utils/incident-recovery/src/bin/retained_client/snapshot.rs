//! Read-only inspection of an owned, just-imported checkpoint snapshot.
//!
//! This is deliberately narrower than arbitrary saved-voter-state decoding: a
//! distributable snapshot must contain the exact untouched fresh set-7957 state
//! written by stable2609 `aux_schema::update_authority_set`. It must never contain
//! somebody else's votes. No network, signer, SDK writable open or runtime call.
use super::*;

const GENESIS: &str = "7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5";
const POLICY_KEY: &[u8] = b"sora-nexus/recovery/checkpoint-imported/v1";
const SET_ID: u64 = 7957;

/// Exact fresh encoding, not a permissive decoder. The SDK's private
/// environment::{VoterSetState::live, CompletedRounds::encode} encode Live(0),
/// one completed round (0, genesis state, base, empty votes), set and ordered
/// voters, then current round 1 with HasVoted::No(0). Genesis State field order
/// is prevote_ghost, finalized, estimate, completable. All arbitrary voted,
/// paused, trailing or older-set bytes fail equality. Keep this reviewed mirror
/// bound to the vendored SDK; do not normalize or reset the stored value.
fn fresh_round_bytes(keys: &[sp_consensus_grandpa::AuthorityId]) -> Result<Vec<u8>> {
    let base = (h256(PARENT_HASH)?, CHECKPOINT - 1);
    let state = (Some(base), Some(base), Some(base), true);
    let completed = (0u64, state, base, Vec::<u8>::new());
    Ok((
        0u8,
        (vec![completed], SET_ID, keys.to_vec()),
        BTreeMap::from([(1u64, 0u8)]),
    )
        .encode())
}

fn file_stamps(path: &Path) -> Result<BTreeMap<String, (u64, i64, i64, u64)>> {
    let mut stamps = BTreeMap::new();
    for item in fs::read_dir(path).map_err(|e| e.to_string())? {
        let item = item.map_err(|e| e.to_string())?;
        regular(&item.path())?;
        let meta = item.metadata().map_err(|e| e.to_string())?;
        let name = item
            .file_name()
            .into_string()
            .map_err(|_| "database filename")?;
        stamps.insert(
            name,
            (meta.len(), meta.mtime(), meta.mtime_nsec(), meta.ino()),
        );
        need(stamps.len() <= 100_000, "database file count bound")?;
    }
    need(!stamps.is_empty(), "empty snapshot database")?;
    Ok(stamps)
}

pub(super) fn run(database: &Path, output_dir: &Path, policy: &str) -> Result<Value> {
    need(
        fs::symlink_metadata(database)
            .map_err(|e| e.to_string())?
            .is_dir()
            && fs::canonicalize(database).map_err(|e| e.to_string())? == database,
        "canonical existing snapshot database required",
    )?;
    let policy = h256(policy.strip_prefix("0x").ok_or("policy hex prefix")?)?;
    need(policy != H256::zero(), "empty recovery policy")?;
    need(
        read(&database.join("db_version"), 16)? == b"4",
        "snapshot database schema",
    )?;
    let before = file_stamps(database)?;
    fs::create_dir(output_dir).map_err(|e| e.to_string())?;
    let logs = output_dir.join("readonly-engine-logs");
    fs::create_dir(&logs).map_err(|e| e.to_string())?;
    let mut options = Options::default();
    options.create_if_missing(false);
    options.create_missing_column_families(false);
    options.set_db_log_dir(&logs);
    let mut families = DB::list_cf(&options, database).map_err(|e| e.to_string())?;
    families.sort();
    let mut expected = (0..13).map(|i| format!("col{i}")).collect::<Vec<_>>();
    expected.push("default".into());
    expected.sort();
    need(families == expected, "snapshot database columns")?;
    let db = DB::open_cf_for_read_only(&options, database, families, false)
        .map_err(|e| e.to_string())?;
    let checkpoint = h256(CHECKPOINT_HASH)?;
    let parent = h256(PARENT_HASH)?;
    need(
        get(&db, 0, b"type")? == b"full" && get(&db, 2, b"mode")? == b"archive",
        "snapshot database mode",
    )?;
    let meta = db.cf_handle("col0").ok_or("metadata column")?;
    need(
        db.get_pinned_cf(meta, b"gap")
            .map_err(|e| e.to_string())?
            .is_none(),
        "snapshot block gap",
    )?;
    need(
        get(&db, 0, b"gen")? == h256(GENESIS)?.as_bytes(),
        "snapshot genesis",
    )?;
    need(
        number_lookup(&get(&db, 0, b"best")?)? == (CHECKPOINT, checkpoint),
        "snapshot best head",
    )?;
    need(
        number_lookup(&get(&db, 0, b"final")?)? == (CHECKPOINT - 1, parent),
        "snapshot finalized head",
    )?;
    let leaves: Vec<(u32, Vec<H256>)> = decode(&get(&db, 0, b"leaf")?)?;
    need(
        leaves == vec![(CHECKPOINT, vec![checkpoint])],
        "snapshot leaf set",
    )?;
    let (header, header_raw) = header_at(&db, CHECKPOINT)?;
    let (parent_header, parent_raw) = header_at(&db, CHECKPOINT - 1)?;
    need(
        header.hash() == checkpoint
            && *header.parent_hash() == parent
            && parent_header.hash() == parent
            && *parent_header.state_root() == h256(PARENT_ROOT)?,
        "snapshot header anchors",
    )?;
    need(
        get(&db, 8, POLICY_KEY)? == policy.as_bytes(),
        "snapshot import provenance",
    )?;
    need(
        decode::<u32>(&get(&db, 8, b"grandpa_schema_version")?)? == 3,
        "snapshot GRANDPA schema",
    )?;
    let authority_raw = get(&db, 8, b"grandpa_voters")?;
    let native: AuthoritySet<H256, u32> = decode(&authority_raw)?;
    let shared = SharedAuthoritySet::from(native);
    need(shared.set_id() == SET_ID, "snapshot GRANDPA set")?;
    // Decode the exact native type first; this bounded projection preserves the
    // stored order and additionally rejects pending forced/standard changes.
    type EmptyPendingSet = (
        AuthorityList,
        u64,
        (Vec<()>, Option<u32>),
        Vec<()>,
        Vec<(u64, u32)>,
    );
    let (keys, set, (pending, _), forced, _): EmptyPendingSet = decode(&authority_raw)?;
    let keys = authorities(keys)?;
    need(
        set == SET_ID && pending.is_empty() && forced.is_empty(),
        "snapshot pending authority changes",
    )?;
    let round_raw = get(&db, 8, b"grandpa_completed_round")?;
    let ordered = keys.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
    need(
        round_raw == fresh_round_bytes(&ordered)?,
        "snapshot must contain exact fresh set-7957 round state without votes",
    )?;
    let aux = db.cf_handle("col8").ok_or("auxiliary column")?;
    let mut concluded = db.raw_iterator_cf(aux);
    concluded.seek(b"grandpa_concluded_rounds");
    need(
        !concluded.valid()
            || !concluded
                .key()
                .ok_or("concluded-round key")?
                .starts_with(b"grandpa_concluded_rounds"),
        "snapshot contains concluded local rounds",
    )?;
    concluded.status().map_err(|e| e.to_string())?;
    drop(concluded);
    need(
        decode::<u32>(&get(&db, 8, b"babe_epoch_changes_version")?)? == 3,
        "snapshot BABE schema",
    )?;
    let babe_raw = get(&db, 8, b"babe_epoch_changes")?;
    let _: BabeCache = decode(&babe_raw)?;
    let parent_weight: u32 = decode(&get(
        &db,
        8,
        &sc_consensus_babe::aux_schema::block_weight_key(parent),
    )?)?;
    let checkpoint_weight: u32 = decode(&get(
        &db,
        8,
        &sc_consensus_babe::aux_schema::block_weight_key(checkpoint),
    )?)?;
    let digests = header
        .digest()
        .logs()
        .iter()
        .filter_map(|d| d.as_babe_pre_digest())
        .collect::<Vec<_>>();
    need(
        digests.len() == 1 && parent_weight == 0 && checkpoint_weight == digests[0].added_weight(),
        "snapshot BABE state-sync weight",
    )?;
    drop(db);
    need(
        file_stamps(database)? == before,
        "snapshot files changed during read-only inspection",
    )?;
    let mut artifacts = BTreeMap::new();
    for (name, raw) in [
        ("checkpoint-header.scale", header_raw),
        ("parent-header.scale", parent_raw),
        ("grandpa-voters.scale", authority_raw),
        ("grandpa-rounds.scale", round_raw),
        ("babe-epochs.scale", babe_raw),
    ] {
        artifacts.insert(name, output(&output_dir.join(name), &raw)?);
    }
    let mut listed = keys
        .iter()
        .map(|(key, weight)| (format!("0x{}", hex::encode(key.encode())), *weight))
        .collect::<Vec<_>>();
    listed.sort();
    Ok(
        json!({"schema":"sora-nexus-checkpoint-snapshot-inspection-v1","database":database,
        "engineReadOnly":true,"genesisHash":format!("0x{GENESIS}"),"bestNumber":CHECKPOINT,
        "bestHash":format!("{checkpoint:#x}"),"finalizedNumber":CHECKPOINT-1,"finalizedHash":format!("{parent:#x}"),
        "checkpointImportPolicy":format!("{policy:#x}"),"grandpaSetId":set,"grandpaAuthorities":listed,
        "savedRoundSetId":SET_ID,"freshRoundStateVerified":true,"artifacts":artifacts,
        "voterHealthVerified":false,"finalityProofVerified":false,"productionReady":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "sora-nexus-snapshot-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    // Actual public canonical headers retained by the read-only checkpoint
    // evidence capture. Only the tiny RocksDB fixture is synthetic: no state,
    // history or finality execution claim is made by these inspector tests.
    const PARENT_HEADER: &str = "1c2ab8bf2527b2d7deabbe51f82dae82515daef274517968381dd7878cd32a9822b1b80651d9c70eb628e2a008906b99221b82202349d379af4d4767bf3fc27ecb3ddd2ec18cfdaaa48a83a3aa09d02ffd28e4e65c78674836c4abf6082b123a267af9c70806424142453402010000005baecf110000000005424142450101b4ea77d0d65da8e588bc3cb8d35829c533fcc6909f6212c2baf010f13156e56d8a8ce5d7d03019c9e1b4707bdd2511d12469191687e860d432ebe0078b985a82";
    const CHECKPOINT_HEADER: &str = "60b63e40ce4de9b9c1408bf7455a6f5ced796825f2437a0ee8496476d80c56d626b1b80679bda41a1db53470463653f358890f2d1fca8ad622c33a1d7095294b0b1404524a24fdf92274ca78ef10c57956d642b664287780b241a06fe45b68d2d11faa7d0806424142453402020000005caecf11000000000542414245010180149e1b922dc6fd0377e1e29b8f8277f0247ac10de60ab8339758df73fd141fa4b96671583c11a1355f072ef79e783eb2adc2242a9799eb80ee24d257defa8f";

    fn put(db: &DB, col: u32, key: &[u8], value: &[u8]) {
        db.put_cf(db.cf_handle(&format!("col{col}")).unwrap(), key, value)
            .unwrap();
    }

    fn fixture(directory: &Path, changed: Option<(&[u8], Vec<u8>)>) -> (PathBuf, H256) {
        let path = directory.join("database");
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        let db = DB::open_cf(&options, &path, (0..13).map(|i| format!("col{i}"))).unwrap();
        fs::write(path.join("db_version"), b"4").unwrap();
        put(&db, 0, b"type", b"full");
        put(&db, 2, b"mode", b"archive");
        put(&db, 0, b"gen", h256(GENESIS).unwrap().as_bytes());
        for (raw, tip) in [
            (PARENT_HEADER, b"final".as_slice()),
            (CHECKPOINT_HEADER, b"best".as_slice()),
        ] {
            let bytes = hex::decode(raw).unwrap();
            let header: ChainHeader = decode(&bytes).unwrap();
            let mut lookup = header.number().to_be_bytes().to_vec();
            lookup.extend_from_slice(header.hash().as_bytes());
            put(&db, 0, tip, &lookup);
            put(&db, 3, &header.number().to_be_bytes(), &lookup);
            put(&db, 3, header.hash().as_bytes(), &lookup);
            put(&db, 4, &lookup, &bytes);
            put(
                &db,
                8,
                &sc_consensus_babe::aux_schema::block_weight_key(header.hash()),
                &0u32.encode(),
            );
        }
        put(
            &db,
            0,
            b"leaf",
            &vec![(CHECKPOINT, vec![h256(CHECKPOINT_HASH).unwrap()])].encode(),
        );
        let key: sp_consensus_grandpa::AuthorityId = decode(&[1u8; 32]).unwrap();
        let raw = (
            vec![(key.clone(), 1u64)],
            SET_ID,
            (Vec::<()>::new(), None::<u32>),
            Vec::<()>::new(),
            Vec::<(u64, u32)>::new(),
        )
            .encode();
        let _: AuthoritySet<H256, u32> = decode(&raw).unwrap();
        put(&db, 8, b"grandpa_voters", &raw);
        put(&db, 8, b"grandpa_schema_version", &3u32.encode());
        put(
            &db,
            8,
            b"grandpa_completed_round",
            &fresh_round_bytes(&[key]).unwrap(),
        );
        put(&db, 8, b"babe_epoch_changes_version", &3u32.encode());
        put(&db, 8, b"babe_epoch_changes", &BabeCache::new().encode());
        let policy = H256::repeat_byte(9);
        put(&db, 8, POLICY_KEY, policy.as_bytes());
        if let Some((key, value)) = changed {
            put(&db, 8, key, &value);
        }
        drop(db);
        (path, policy)
    }

    #[test]
    fn actual_readonly_engine_checks_and_exports_snapshot_without_changing_database() {
        let directory = TestDirectory::new();
        let (path, policy) = fixture(&directory.0, None);
        let before = inventory(&path).unwrap();
        let report = run(
            &path,
            &directory.0.join("evidence"),
            &format!("{policy:#x}"),
        )
        .unwrap();
        assert_eq!(report["engineReadOnly"], true);
        assert_eq!(report["bestHash"], format!("0x{CHECKPOINT_HASH}"));
        assert_eq!(report["finalizedHash"], format!("0x{PARENT_HASH}"));
        assert_eq!(report["savedRoundSetId"], SET_ID);
        assert_eq!(report["freshRoundStateVerified"], true);
        assert_eq!(report["voterHealthVerified"], false);
        assert_eq!(inventory(&path).unwrap(), before);
    }

    #[test]
    fn actual_readonly_engine_rejects_old_rounds_foreign_provenance_and_concluded_rounds() {
        let key: sp_consensus_grandpa::AuthorityId = decode(&[1u8; 32]).unwrap();
        let mut old = fresh_round_bytes(&[key]).unwrap();
        let offset = old
            .windows(8)
            .position(|w| w == SET_ID.to_le_bytes())
            .unwrap();
        old[offset..offset + 8].copy_from_slice(&(SET_ID - 1).to_le_bytes());
        for (key, bytes) in [
            (b"grandpa_completed_round".as_slice(), old),
            (POLICY_KEY, vec![8; 32]),
            (b"grandpa_concluded_rounds-extra".as_slice(), vec![0]),
        ] {
            let directory = TestDirectory::new();
            let (path, policy) = fixture(&directory.0, Some((key, bytes)));
            let before = inventory(&path).unwrap();
            assert!(run(
                &path,
                &directory.0.join("evidence"),
                &format!("{policy:#x}")
            )
            .is_err());
            assert_eq!(inventory(&path).unwrap(), before);
        }
    }

    #[test]
    fn missing_database_never_created_and_existing_evidence_never_overwritten() {
        let directory = TestDirectory::new();
        let absent = directory.0.join("missing");
        assert!(run(
            &absent,
            &directory.0.join("evidence"),
            &format!("{:#x}", H256::repeat_byte(9))
        )
        .is_err());
        assert!(!absent.exists());
        let (path, policy) = fixture(&directory.0, None);
        let evidence = directory.0.join("retained");
        fs::create_dir(&evidence).unwrap();
        fs::write(evidence.join("marker"), b"preserve").unwrap();
        assert!(run(&path, &evidence, &format!("{policy:#x}")).is_err());
        assert_eq!(fs::read(evidence.join("marker")).unwrap(), b"preserve");
    }

    #[test]
    fn fresh_snapshot_rounds_bind_set_parent_keys_and_no_votes() {
        let keys = vec![sp_consensus_grandpa::AuthorityId::from(
            sp_core::ed25519::Public::from_raw([1; 32]),
        )];
        let raw = fresh_round_bytes(&keys).unwrap();
        assert_eq!(raw[0], 0); // Live
        assert_eq!(raw[1], 4); // one completed round
        assert_eq!(&raw[2..10], &[0; 8]);
        assert!(raw.windows(8).any(|w| w == SET_ID.to_le_bytes()));
        assert!(raw.windows(32).any(|w| w == [1; 32]));
        let mut other = keys;
        other.push(sp_consensus_grandpa::AuthorityId::from(
            sp_core::ed25519::Public::from_raw([2; 32]),
        ));
        assert_ne!(raw, fresh_round_bytes(&other).unwrap());
        assert_eq!(&raw[raw.len() - 10..], &[4, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
    }
}
