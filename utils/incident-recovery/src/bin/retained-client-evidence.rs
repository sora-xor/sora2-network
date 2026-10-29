//! Read-only, capture-anchored consensus evidence from one owned SORA Nexus DB copy.
//! Default evidence mode never opens a writable backend. Optional explicitly selected
//! predecessor reconstruction writes a fresh owned SDK database, or explicitly
//! resumes an owned partial output after full canonical-prefix verification; no node/service,
//! signing, network, or runtime execution is started.
#[path = "retained_client/authority_chain.rs"]
mod authority_chain;
#[cfg(feature = "retained-predecessor")]
#[path = "retained_client/predecessor.rs"]
mod predecessor;
use codec::{Decode, DecodeLimit, Encode};
use incident_recovery::{verify_capture, verify_capture_at, Result};
use rocksdb::{Options, DB};
use sc_consensus_babe::Epoch as CachedEpoch;
use sc_consensus_epochs::EpochChanges;
use sc_finality_grandpa::{AuthoritySet, GrandpaJustification, SharedAuthoritySet};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use sp_consensus_babe::{
    digests::{CompatibleDigestItem, PreDigest},
    AuthorityPair,
};
use sp_consensus_grandpa::AuthorityList;
use sp_core::{storage::Storage, Pair, H256};
use sp_runtime::{
    generic::{Block, Header},
    traits::{BlakeTwo256, Header as HeaderT},
    OpaqueExtrinsic,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

type ChainHeader = Header<u32, BlakeTwo256>;
type ChainBlock = Block<ChainHeader, OpaqueExtrinsic>;
type BabeCache = EpochChanges<H256, u32, CachedEpoch>;
const ROOT: &str = "/Users/takemiyamakoto/dev/sora2-network/artifacts/recovery-28191817";
const DB_REL: &str = "fixtures/retained-client-database-admission-v1/inspection-base/chains/sora-substrate-main-net/db/full";
const CHECKPOINT: u32 = 28_191_817;
const CHECKPOINT_HASH: &str = "7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0";
const PARENT_HASH: &str = "60b63e40ce4de9b9c1408bf7455a6f5ced796825f2437a0ee8496476d80c56d6";
const PARENT_ROOT: &str = "51d9c70eb628e2a008906b99221b82202349d379af4d4767bf3fc27ecb3ddd2e";
const PARENT_MANIFEST: &str = "3af84b1cb4ea6f91c38c4ead65fcb4464691a0c2f2decdbb81dae3b6b7f42a7c";
const CHECKPOINT_MANIFEST: &str =
    "24d0ea108f2adfbed6e543b7ce6bdb8146ce0e1ecf2b733af35ee9b9a315cc0b";
const MAX_VALUE: usize = 2 * 1024 * 1024;
const MAX_RANGE: u32 = 100_000;

fn need(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn hash(raw: &[u8]) -> String {
    hex::encode(Sha256::digest(raw))
}
fn h256(value: &str) -> Result<H256> {
    let raw = hex::decode(value).map_err(|e| e.to_string())?;
    need(raw.len() == 32, "hash width")?;
    Ok(H256::from_slice(&raw))
}
fn regular(path: &Path) -> Result<()> {
    need(
        path.is_absolute() && fs::canonicalize(path).map_err(|e| e.to_string())? == path,
        "canonical path required",
    )?;
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    need(
        meta.is_file() && meta.nlink() == 1,
        "independent regular file required",
    )
}
fn read(path: &Path, max: usize) -> Result<Vec<u8>> {
    regular(path)?;
    let mut raw = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(max as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    need(raw.len() <= max, "file byte bound")?;
    Ok(raw)
}
fn decode<T: Decode + Encode>(raw: &[u8]) -> Result<T> {
    need(
        !raw.is_empty() && raw.len() <= MAX_VALUE,
        "typed value byte bound",
    )?;
    let mut input = raw;
    let value = T::decode_with_depth_limit(
        64,
        &mut codec::MemTrackingInput::new(&mut input, MAX_VALUE * 8),
    )
    .map_err(|e| e.to_string())?;
    need(
        input.is_empty() && value.encode() == raw,
        "noncanonical or trailing typed bytes",
    )?;
    Ok(value)
}
fn storage_key(pallet: &str, item: &str) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(pallet.as_bytes()).as_slice(),
        sp_io::hashing::twox_128(item.as_bytes()).as_slice(),
    ]
    .concat()
}
fn storage_value<T: Decode + Encode>(storage: &Storage, pallet: &str, item: &str) -> Result<T> {
    decode(
        storage
            .top
            .get(&storage_key(pallet, item))
            .ok_or("missing anchored storage item")?,
    )
}
fn get(db: &DB, column: u32, key: &[u8]) -> Result<Vec<u8>> {
    let name = format!("col{column}");
    let cf = db.cf_handle(&name).ok_or("missing database column")?;
    let value = db
        .get_pinned_cf(cf, key)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("missing column{column} key {}", hex::encode(key)))?;
    need(value.len() <= MAX_VALUE, "database value byte bound")?;
    Ok(value.to_vec())
}
fn number_lookup(raw: &[u8]) -> Result<(u32, H256)> {
    need(raw.len() == 36, "lookup must be number+hash")?;
    let number = u32::from_be_bytes(raw[..4].try_into().map_err(|_| "lookup number")?);
    Ok((number, H256::from_slice(&raw[4..])))
}
fn header_at(db: &DB, number: u32) -> Result<(ChainHeader, Vec<u8>)> {
    let lookup = get(db, 3, &number.to_be_bytes())?;
    let (found_number, expected_hash) = number_lookup(&lookup)?;
    let raw = get(db, 4, &lookup)?;
    let header: ChainHeader = decode(&raw)?;
    need(
        found_number == number && *header.number() == number && header.hash() == expected_hash,
        "canonical header lookup/hash mismatch",
    )?;
    need(
        get(db, 3, expected_hash.as_bytes())? == lookup,
        "hash lookup mismatch",
    )?;
    Ok((header, raw))
}
fn local_path(root: &Path, name: &str) -> Result<PathBuf> {
    let relative = Path::new(name);
    need(
        !relative.is_absolute()
            && !name.is_empty()
            && relative
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            && !name.contains('\\'),
        "canonical local evidence name required",
    )?;
    let path = root.join(relative);
    regular(&path)?;
    Ok(path)
}
fn capture_pins(root: &Path, expected_manifest: &str) -> Result<BTreeMap<PathBuf, String>> {
    let manifest = read(&root.join("capture-complete.json"), MAX_VALUE)?;
    need(
        hash(&manifest) == expected_manifest,
        "capture manifest pin differs",
    )?;
    let value: Value = serde_json::from_slice(&manifest).map_err(|e| e.to_string())?;
    need(
        value["children"].as_array().is_some_and(|x| x.is_empty()),
        "this fixed evidence corridor expects retained child-free captures",
    )?;
    let mut pins = BTreeMap::from([(
        root.join("capture-complete.json"),
        expected_manifest.to_owned(),
    )]);
    for (name, row) in value["artifacts"].as_object().ok_or("capture artifacts")? {
        let path = local_path(root, name)?;
        let digest = row["sha256"].as_str().ok_or("artifact digest")?.to_owned();
        need(
            pins.insert(path, digest).is_none(),
            "duplicate capture artifact",
        )?;
    }
    for row in value["top"]["pages"]
        .as_array()
        .ok_or("capture top pages")?
    {
        let path = local_path(root, row["file"].as_str().ok_or("page path")?)?;
        let digest = row["sha256"].as_str().ok_or("page digest")?.to_owned();
        need(
            pins.insert(path, digest).is_none(),
            "duplicate capture page",
        )?;
    }
    verify_files(&pins)?;
    Ok(pins)
}
fn verify_files(pins: &BTreeMap<PathBuf, String>) -> Result<()> {
    for (path, digest) in pins {
        regular(path)?;
        let mut stream = File::open(path).map_err(|e| e.to_string())?;
        let mut sha = Sha256::new();
        let mut buffer = [0u8; 1024 * 1024];
        let mut count = 0u64;
        loop {
            let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            count += n as u64;
            need(count <= 128 * 1024 * 1024 * 1024, "pinned file byte bound")?;
            sha.update(&buffer[..n]);
        }
        need(
            hex::encode(sha.finalize()) == *digest,
            "pinned evidence file changed",
        )?;
    }
    Ok(())
}
fn inventory(path: &Path) -> Result<BTreeMap<String, Value>> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let file = entry.path();
        regular(&file)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "nonutf8 database filename")?;
        let mut stream = File::open(&file).map_err(|e| e.to_string())?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 1024 * 1024];
        let mut size = 0u64;
        loop {
            let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            size = size
                .checked_add(n as u64)
                .ok_or("database length overflow")?;
            need(size <= 128 * 1024 * 1024 * 1024, "database file bound")?;
            digest.update(&buffer[..n]);
        }
        result.insert(
            name,
            json!({"size":size,"sha256":hex::encode(digest.finalize())}),
        );
        need(result.len() <= 100_000, "database file count bound")?;
    }
    need(!result.is_empty(), "empty database")?;
    Ok(result)
}
fn output(path: &Path, raw: &[u8]) -> Result<Value> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(raw).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(
        json!({"file":path.file_name().ok_or("output name")?.to_string_lossy(),
        "sha256":hash(raw),"bytes":raw.len()}),
    )
}
fn authorities(raw: AuthorityList) -> Result<AuthorityList> {
    let mut seen = BTreeSet::new();
    need(
        !raw.is_empty()
            && raw.len() <= 10_000
            && raw
                .iter()
                .all(|(key, weight)| *weight > 0 && seen.insert(key.encode())),
        "invalid anchored authority list",
    )?;
    Ok(raw)
}
fn checkpoint_babe_signature(header: &ChainHeader, parent: &Storage) -> Result<Value> {
    let authorities: Vec<(sp_consensus_babe::AuthorityId, u64)> =
        storage_value(parent, "Babe", "Authorities")?;
    let mut pre = header.clone();
    let seal = pre
        .digest_mut()
        .pop()
        .ok_or("checkpoint BABE seal missing")?;
    let signature = seal
        .as_babe_seal()
        .ok_or("last checkpoint digest is not a BABE seal")?;
    let pre_digests = pre
        .digest()
        .logs()
        .iter()
        .filter_map(|item| item.as_babe_pre_digest())
        .collect::<Vec<_>>();
    need(pre_digests.len() == 1, "checkpoint BABE pre-digest count")?;
    let predigest = &pre_digests[0];
    let genesis: u64 = storage_value(parent, "Babe", "GenesisSlot")?;
    let epoch: u64 = storage_value(parent, "Babe", "EpochIndex")?;
    let start = epoch
        .checked_mul(600)
        .and_then(|n| n.checked_add(genesis))
        .ok_or("epoch slot overflow")?;
    let slot = u64::from(predigest.slot());
    need(
        start <= slot && slot < start.checked_add(600).ok_or("epoch end overflow")?,
        "checkpoint crossed a BABE epoch; explicit transition proof is required",
    )?;
    let author = authorities
        .get(predigest.authority_index() as usize)
        .ok_or("checkpoint BABE authority index")?;
    need(
        author.1 > 0 && AuthorityPair::verify(&signature, pre.hash().as_bytes(), &author.0),
        "checkpoint BABE cryptographic seal differs from parent-state authority",
    )?;
    Ok(
        json!({"authority":format!("0x{}",hex::encode(author.0.encode())),
        "authorityIndex":predigest.authority_index(),"slot":u64::from(predigest.slot()),
        "preDigestKind":match predigest { PreDigest::Primary(_)=>"Primary",PreDigest::SecondaryPlain(_)=>"SecondaryPlain",PreDigest::SecondaryVRF(_)=>"SecondaryVRF" },
        "signatureVerifiedAgainstParentAuthorities":true,"slotAssignmentVerified":false,
        "fullBabeHeaderVerification":false,"epochConfigurationRepairApplied":false}),
    )
}
fn inspect(output_dir: &Path) -> Result<Value> {
    let root = Path::new(ROOT);
    let database = root.join(DB_REL);
    need(
        fs::canonicalize(&database).map_err(|e| e.to_string())? == database,
        "owned database path changed",
    )?;
    let execution_root = root.join("fixtures/retained-consensus-evidence-execution-v1");
    need(
        output_dir.parent() == Some(execution_root.as_path())
            && output_dir
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v.starts_with("attempt-"))
            && !output_dir.exists(),
        "fresh fixed evidence output required",
    )?;
    fs::create_dir_all(&execution_root).map_err(|e| e.to_string())?;
    need(
        fs::canonicalize(&execution_root).map_err(|e| e.to_string())? == execution_root,
        "aliased output parent",
    )?;
    eprintln!("retained consensus evidence: hashing owned database inventory");
    let before = inventory(&database)?;
    let admission_path =
        root.join("fixtures/retained-client-database-admission-v1/content-inventory.json");
    let admission_raw = read(&admission_path, 64 * 1024 * 1024)?;
    need(
        hash(&admission_raw) == "f94ba1f743cc759459a574e34f16725df72100dbb7bc95d3df9c1e5aa59c47a3",
        "retained DB inventory pin changed",
    )?;
    let admission: Value = serde_json::from_slice(&admission_raw).map_err(|e| e.to_string())?;
    need(
        admission["copy"] == database.to_string_lossy().as_ref()
            && admission["files"] == json!(before),
        "owned copy differs from retained inventory",
    )?;
    fs::create_dir(output_dir).map_err(|e| e.to_string())?;
    let log_dir = output_dir.join("rocksdb-read-only-logs");
    fs::create_dir(&log_dir).map_err(|e| e.to_string())?;
    let mut options = Options::default();
    options.create_if_missing(false);
    options.create_missing_column_families(false);
    options.set_db_log_dir(&log_dir);
    let mut families = DB::list_cf(&options, &database).map_err(|e| e.to_string())?;
    families.sort();
    let mut expected = (0..13).map(|i| format!("col{i}")).collect::<Vec<_>>();
    expected.push("default".into());
    expected.sort();
    need(
        families == expected && read(&database.join("db_version"), 16)? == b"4",
        "exact SDK database schema4 required",
    )?;
    let db = DB::open_cf_for_read_only(&options, &database, families, false)
        .map_err(|e| e.to_string())?;
    need(
        get(&db, 0, b"type")? == b"full"
            && get(&db, 0, b"gen")?
                == hex::decode("7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5")
                    .map_err(|e| e.to_string())?,
        "retained database type/genesis differs",
    )?;
    need(
        get(&db, 8, b"grandpa_schema_version")? == 3u32.encode()
            && get(&db, 8, b"babe_epoch_changes_version")? == 3u32.encode(),
        "typed consensus cache schema3 required",
    )?;
    let checkpoint_capture = root.join("checkpoint");
    let parent_capture = root.join("history/captures/28191816");
    let checkpoint_pins = capture_pins(&checkpoint_capture, CHECKPOINT_MANIFEST)?;
    let parent_pins = capture_pins(&parent_capture, PARENT_MANIFEST)?;
    let mut written_pins = BTreeMap::new();
    eprintln!("retained consensus evidence: verifying complete predecessor state root");
    let (parent_report, parent_state) = verify_capture_at(
        &parent_capture,
        CHECKPOINT - 1,
        h256(PARENT_HASH)?,
        h256(PARENT_ROOT)?,
    )?;
    eprintln!("retained consensus evidence: verifying complete checkpoint state root");
    let (checkpoint_report, checkpoint_state) = verify_capture(&checkpoint_capture)?;
    need(
        parent_state.top.get(b":code".as_slice()) == checkpoint_state.top.get(b":code".as_slice()),
        "typed predecessor constants require the exact checkpoint spec134 code",
    )?;
    let (parent, parent_raw) = header_at(&db, CHECKPOINT - 1)?;
    let (checkpoint, checkpoint_raw) = header_at(&db, CHECKPOINT)?;
    need(
        parent.hash() == h256(PARENT_HASH)?
            && *parent.state_root() == h256(PARENT_ROOT)?
            && checkpoint.hash() == h256(CHECKPOINT_HASH)?
            && *checkpoint.parent_hash() == parent.hash(),
        "owned history disagrees with captured checkpoint/predecessor",
    )?;
    let parent_set: u64 = storage_value(&parent_state, "Grandpa", "CurrentSetId")?;
    let checkpoint_set: u64 = storage_value(&checkpoint_state, "Grandpa", "CurrentSetId")?;
    let parent_authorities = authorities(storage_value(&parent_state, "Grandpa", "Authorities")?)?;
    let checkpoint_authorities =
        authorities(storage_value(&checkpoint_state, "Grandpa", "Authorities")?)?;
    need(
        parent_set == checkpoint_set && parent_authorities == checkpoint_authorities,
        "checkpoint authority transition requires a separate proof path",
    )?;
    let authority_raw = get(&db, 8, b"grandpa_voters")?;
    let decoded: AuthoritySet<H256, u32> = decode(&authority_raw)?;
    // Validate the typed prefix before SharedAuthoritySet's voter-set invariant can assert.
    let mut prefix = authority_raw.as_slice();
    let observed = AuthorityList::decode(&mut prefix).map_err(|e| e.to_string())?;
    need(
        authorities(observed)? == parent_authorities,
        "cached authorities differ from root-verified state",
    )?;
    let shared: SharedAuthoritySet<H256, u32> = decoded.into();
    need(
        shared.set_id() == parent_set,
        "cache current set differs from root-verified predecessor",
    )?;
    let changes = shared.authority_set_changes();
    let history = changes
        .iter_from(0)
        .ok_or("authority history lacks genesis prefix")?
        .copied()
        .collect::<Vec<_>>();
    need(
        history.len() <= 100_000
            && history.iter().enumerate().all(|(i, (set, n))| {
                *set == i as u64 && *n < CHECKPOINT && (i == 0 || *n > history[i - 1].1)
            }),
        "authority history order/set sequence/checkpoint interval differs",
    )?;
    need(
        history
            .last()
            .is_some_and(|(set, _)| set.checked_add(1) == Some(parent_set)),
        "authority history does not reach the anchored current set",
    )?;
    eprintln!("retained consensus evidence: verified roots; checking current-set proof and bounded ancestry");
    let best_raw = get(&db, 8, b"grandpa_best_justification")?;
    let best: GrandpaJustification<ChainBlock> = decode(&best_raw)?;
    let (target_number, target_hash) = best.target();
    need(
        target_number >= CHECKPOINT && target_number - CHECKPOINT <= MAX_RANGE,
        "best justification outside bounded retained branch",
    )?;
    best.verify(parent_set, &parent_authorities)
        .map_err(|e| e.to_string())?;
    let finalized = number_lookup(&get(&db, 0, b"final")?)?;
    need(
        (target_number, target_hash) == finalized,
        "best justification does not match finalized head",
    )?;
    let mut headers = Vec::new();
    let mut previous = parent.hash();
    let headers_dir = output_dir.join("headers");
    fs::create_dir(&headers_dir).map_err(|e| e.to_string())?;
    for number in CHECKPOINT..=target_number {
        let (header, raw) = header_at(&db, number)?;
        need(
            *header.parent_hash() == previous,
            "retained canonical ancestry gap",
        )?;
        previous = header.hash();
        let path = headers_dir.join(format!("{number}.scale"));
        let file = output(&path, &raw)?;
        written_pins.insert(path, hash(&raw));
        headers.push(json!({"number":number,"hash":format!("{previous:#x}"),"header":file}));
    }
    need(
        previous == target_hash,
        "justification target not descended from pinned checkpoint",
    )?;
    let babe_raw = get(&db, 8, b"babe_epoch_changes")?;
    let babe: BabeCache = decode(&babe_raw)?;
    let mut epochs = Vec::new();
    let _mapped: BabeCache=babe.map(|hash,number,epoch|{
        epochs.push(json!({"importHash":format!("{hash:#x}"),"importNumber":number,
            "epochIndex":epoch.epoch_index,"startSlot":u64::from(epoch.start_slot),"duration":epoch.duration,
            "authorities":epoch.authorities.iter().map(|(key,weight)|json!({"key":format!("0x{}",hex::encode(key.encode())),"weight":weight})).collect::<Vec<_>>(),
            "randomness":format!("0x{}",hex::encode(epoch.randomness)),"configurationScale":format!("0x{}",hex::encode(epoch.config.encode()))}));epoch
    });
    need(
        !epochs.is_empty() && epochs.len() <= 128,
        "bounded BABE cache epochs required",
    )?;
    let checkpoint_seal = checkpoint_babe_signature(&checkpoint, &parent_state)?;
    let mut artifacts = BTreeMap::new();
    for (name, raw) in [
        ("parent-header.scale", parent_raw),
        ("checkpoint-header.scale", checkpoint_raw),
        ("grandpa-voters.scale", authority_raw),
        ("grandpa-best-justification.scale", best_raw),
        ("babe-epoch-changes.scale", babe_raw),
    ] {
        let path = output_dir.join(name);
        artifacts.insert(name, output(&path, &raw)?);
        written_pins.insert(path, hash(&raw));
    }
    drop(db);
    need(
        inventory(&database)? == before,
        "read-only inspection changed owned database files",
    )?;
    verify_files(&checkpoint_pins)?;
    verify_files(&parent_pins)?;
    verify_files(&written_pins)?;
    need(
        read(&admission_path, 64 * 1024 * 1024)? == admission_raw,
        "retained DB inventory changed during verification",
    )?;
    Ok(
        json!({"schema":"sora-nexus-retained-consensus-evidence-v1","database":database,"engineReadOnly":true,
        "parentCapture":parent_report,"checkpointCapture":checkpoint_report,
        "authoritySetFullyDecoded":true,"authoritySetAnchoredToVerifiedState":true,"currentSetId":parent_set,
        "historicalAuthorityBoundariesFromDecodedCache":history,"grandpaJustificationSignatureVerified":true,
        "grandpaJustificationTarget":{"number":target_number,"hash":format!("{target_hash:#x}")},
        "checkpointAncestryToVerifiedJustification":true,"headers":headers,"babeEpochCacheFullyDecoded":true,
        "babeEpochs":epochs,"checkpointBabeSeal":checkpoint_seal,"artifacts":artifacts,
        "fullGenesisAuthorityChainVerified":false,"allHistoricalHeadersVerified":false,
        "predecessorAuxiliaryStateReconstructed":false,"clientDatabaseReconstructed":false,
        "fullBabeVerification":false,"recoveryConsensusHistoryAvailable":false,
        "operatorApprovalVerified":false,"runtimeExecuted":false,"productionReady":false}),
    )
}
fn main() {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let result = if args.len() == 2 && args[0] == "--output" {
        inspect(&PathBuf::from(&args[1]))
    } else if args.len() == 3 && args[0] == "--verify-authority-chain" {
        authority_chain::run(&PathBuf::from(&args[1]), &PathBuf::from(&args[2]))
    } else {
        #[cfg(feature = "retained-predecessor")]
        {
            if args.len() == 3 && args[0] == "--reconstruct-predecessor" {
                predecessor::run(
                    &PathBuf::from(&args[1]),
                    &PathBuf::from(&args[2]),
                    predecessor::OutputMode::Fresh,
                )
            } else if args.len() == 3 && args[0] == "--resume-predecessor" {
                predecessor::run(
                    &PathBuf::from(&args[1]),
                    &PathBuf::from(&args[2]),
                    predecessor::OutputMode::Resume,
                )
            } else {
                Err("expected --output <fresh evidence attempt>, or --verify-authority-chain <fresh evidence attempt> <fresh authority attempt>, or --reconstruct-predecessor <fresh evidence attempt> <fresh reconstruction attempt>, or --resume-predecessor <fresh evidence attempt> <owned partial reconstruction attempt>".into())
            }
        }
        #[cfg(not(feature = "retained-predecessor"))]
        {
            Err("expected --output <fresh fixed evidence attempt directory>, or --verify-authority-chain <fresh evidence attempt> <fresh authority attempt>".into())
        }
    };
    match result {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("retained consensus evidence refused: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_decode_rejects_trailing_and_noncanonical_bytes() {
        assert_eq!(decode::<u64>(&5u64.encode()).unwrap(), 5);
        let mut raw = 5u64.encode();
        raw.push(0);
        assert!(decode::<u64>(&raw).is_err());
        assert!(decode::<Vec<u8>>(&[1, 0]).is_err());
    }
    #[test]
    fn lookup_requires_exact_number_hash_width() {
        let mut raw = 7u32.to_be_bytes().to_vec();
        raw.extend([8; 32]);
        assert_eq!(number_lookup(&raw).unwrap(), (7, H256::repeat_byte(8)));
        raw.push(0);
        assert!(number_lookup(&raw).is_err());
    }
    #[test]
    fn source_storage_keys_are_separate_for_set_and_authorities() {
        assert_eq!(storage_key("Grandpa", "Authorities").len(), 32);
        assert_ne!(
            storage_key("Grandpa", "Authorities"),
            storage_key("Grandpa", "CurrentSetId")
        );
    }
    #[test]
    fn invalid_authority_weights_and_duplicates_are_rejected_before_shared_invariant() {
        let key: sp_consensus_grandpa::AuthorityId = decode(&[1u8; 32]).unwrap();
        assert!(authorities(Vec::new()).is_err());
        assert!(authorities(vec![(key.clone(), 0)]).is_err());
        assert!(authorities(vec![(key.clone(), 1), (key.clone(), 1)]).is_err());
        assert!(authorities(vec![(key, 1)]).is_ok());
    }
    #[test]
    fn unsigned_justification_cannot_establish_finality() {
        let key: sp_consensus_grandpa::AuthorityId = decode(&[1u8; 32]).unwrap();
        let proof: GrandpaJustification<ChainBlock> = sp_consensus_grandpa::GrandpaJustification {
            round: 1,
            commit: sp_consensus_grandpa::Commit::<ChainHeader> {
                target_hash: H256::repeat_byte(2),
                target_number: CHECKPOINT,
                precommits: Vec::new(),
            },
            votes_ancestries: Vec::new(),
        }
        .into();
        assert!(proof.verify(7, &vec![(key, 1)]).is_err());
    }
}
