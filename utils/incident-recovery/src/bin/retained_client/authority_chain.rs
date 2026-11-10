//! Bounded GRANDPA handoff proof, using the retained cache only as a lookup index.
//! This is the scheduled/zero-delay subset of the SDK warp-proof trust model, not
//! validation of all historical headers, BABE slots, or runtime execution.
use super::*;
use sp_consensus_grandpa::{ConsensusLog, GRANDPA_ENGINE_ID};
use sp_core::storage::StateVersion;
use sp_runtime::{DigestItem, Justifications};

const GENESIS: &str = "7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5";
const GENESIS_SPEC: &str =
    "/Users/takemiyamakoto/dev/sora2-network/node/chain_spec/src/bytes/chain_spec_main.json";
const GENESIS_SPEC_SHA: &str = "b7e78dcf70d190d28c662d2b5e6598dc37ff6c4f18b1c06eb48491f90f87f86c";
const MAX_PROOF_BYTES: usize = 512 * 1024 * 1024;

fn canonical_hex(value: &Value) -> Result<Vec<u8>> {
    let text = value.as_str().ok_or("hex string required")?;
    let raw =
        hex::decode(text.strip_prefix("0x").ok_or("hex prefix")?).map_err(|e| e.to_string())?;
    need(
        format!("0x{}", hex::encode(&raw)) == text,
        "noncanonical genesis hex",
    )?;
    Ok(raw)
}
fn genesis_from_top(top: BTreeMap<Vec<u8>, Vec<u8>>) -> Result<(ChainHeader, AuthorityList)> {
    let raw = top
        .get(b":grandpa_authorities".as_slice())
        .ok_or("genesis GRANDPA authorities missing")?;
    let (version, keys): (u8, AuthorityList) = decode(raw)?;
    need(version == 1, "unsupported genesis authority encoding")?;
    let keys = authorities(keys)?;
    let state_root = sp_io::trie::blake2_256_root(top.into_iter().collect(), StateVersion::V0);
    let empty = sp_io::trie::blake2_256_ordered_root(Vec::new(), StateVersion::V0);
    Ok((
        ChainHeader::new(0, empty, state_root, H256::zero(), Default::default()),
        keys,
    ))
}
fn genesis(raw: &[u8]) -> Result<(ChainHeader, AuthorityList)> {
    need(
        hash(raw) == GENESIS_SPEC_SHA,
        "pinned genesis chain spec differs",
    )?;
    let spec: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    let children = spec["genesis"]["raw"]["childrenDefault"]
        .as_object()
        .ok_or("genesis child map")?;
    need(children.is_empty(), "unsupported genesis child storage")?;
    let source = spec["genesis"]["raw"]["top"]
        .as_object()
        .ok_or("genesis top map")?;
    need(
        !source.is_empty() && source.len() <= 100_000,
        "genesis entry bound",
    )?;
    let mut top = BTreeMap::new();
    for (key, value) in source {
        let key = canonical_hex(&Value::String(key.clone()))?;
        need(
            top.insert(key, canonical_hex(value)?).is_none(),
            "duplicate genesis key",
        )?;
    }
    let (header, keys) = genesis_from_top(top)?;
    need(
        header.hash() == h256(GENESIS)?,
        "complete genesis trie/header does not match chain identity",
    )?;
    Ok((header, keys))
}
fn scheduled(header: &ChainHeader) -> Result<AuthorityList> {
    let mut next = None;
    for digest in header.digest().logs() {
        if let DigestItem::Consensus(engine, raw) = digest {
            if *engine != GRANDPA_ENGINE_ID {
                continue;
            }
            match decode::<ConsensusLog<u32>>(raw)? {
                ConsensusLog::ScheduledChange(change) => {
                    need(change.delay == 0, "delayed authority handoff needs a separate ancestry proof")?;
                    need(next.is_none(), "multiple scheduled authority digests")?;
                    next = Some(authorities(change.next_authorities)?);
                }
                // No caller-provided hardfork list can bypass the old-set signatures.
                ConsensusLog::ForcedChange(..) => return Err("forced authority change requires separately authenticated historical recovery evidence".into()),
                // stable2609 import and warp verification do not change the voting
                // authority list for an OnDisabled notice. The complete digest
                // remains bound by the old-set justification's target hash.
                ConsensusLog::OnDisabled(_) => {},
                ConsensusLog::Pause(_) | ConsensusLog::Resume(_) => return Err("pause/resume handoff requires a separate proof path".into()),
            }
        }
    }
    let next = next.ok_or("boundary lacks a zero-delay scheduled change")?;
    let sdk = sc_finality_grandpa::find_scheduled_change::<ChainBlock>(header)
        .ok_or("SDK did not recognize the scheduled handoff")?;
    need(
        sdk.delay == 0 && sdk.next_authorities == next,
        "bounded handoff differs from SDK import/warp interpretation",
    )?;
    Ok(next)
}
fn grandpa_proof(raw: &[u8]) -> Result<Vec<u8>> {
    let all: Justifications = decode(raw)?;
    let mut engines = BTreeSet::new();
    need(
        all.iter().all(|(engine, _)| engines.insert(*engine)),
        "duplicate justification engine",
    )?;
    all.get(GRANDPA_ENGINE_ID)
        .cloned()
        .ok_or_else(|| "stored boundary lacks GRANDPA justification".into())
}
fn handoff(
    header: &ChainHeader,
    proof: &[u8],
    set: u64,
    keys: &AuthorityList,
) -> Result<AuthorityList> {
    let next = scheduled(header)?;
    let proof: GrandpaJustification<ChainBlock> = decode(proof)?;
    need(
        proof.target() == (*header.number(), header.hash()),
        "boundary justification target mismatch",
    )?;
    proof.verify(set, keys).map_err(|e| e.to_string())?;
    Ok(next)
}
fn retain(
    dir: &Path,
    name: &str,
    raw: &[u8],
    pins: &mut BTreeMap<PathBuf, String>,
    total: &mut usize,
) -> Result<Value> {
    *total = total.checked_add(raw.len()).ok_or("proof byte overflow")?;
    need(
        *total <= MAX_PROOF_BYTES,
        "aggregate authority proof byte bound",
    )?;
    let path = dir.join(name);
    let row = output(&path, raw)?;
    need(
        pins.insert(path, hash(raw)).is_none(),
        "duplicate proof output",
    )?;
    Ok(row)
}
pub(super) fn run(evidence_output: &Path, output_dir: &Path) -> Result<Value> {
    let root = Path::new(ROOT);
    let allowed = root.join("fixtures/retained-authority-chain-v1");
    need(
        output_dir.parent() == Some(allowed.as_path())
            && output_dir
                .file_name()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v.starts_with("attempt-"))
            && !output_dir.exists(),
        "fresh fixed authority output required",
    )?;
    eprintln!(
        "authority proof: fresh owned-DB inventory and full parent/checkpoint capture verification"
    );
    // This function is intentionally not callable with a deserialized successful report.
    let evidence = inspect(evidence_output)?;
    eprintln!(
        "authority proof: capture roots and current-set finality verified; pinning boundary inputs"
    );
    let database = root.join(DB_REL);
    let before = inventory(&database)?;
    let admission_path =
        root.join("fixtures/retained-client-database-admission-v1/content-inventory.json");
    let admission_raw = read(&admission_path, 64 * 1024 * 1024)?;
    need(
        hash(&admission_raw) == "f94ba1f743cc759459a574e34f16725df72100dbb7bc95d3df9c1e5aa59c47a3"
            && serde_json::from_slice::<Value>(&admission_raw).map_err(|e| e.to_string())?["files"]
                == json!(before),
        "owned database changed after root verification",
    )?;
    let mut input_pins = capture_pins(&root.join("checkpoint"), CHECKPOINT_MANIFEST)?;
    input_pins.extend(capture_pins(
        &root.join("history/captures/28191816"),
        PARENT_MANIFEST,
    )?);
    input_pins.insert(admission_path, hash(&admission_raw));
    // Keep every artifact referenced by the fresh in-process evidence graph bound,
    // including the ancestry headers, not merely its current-set cache/proof.
    for row in evidence["artifacts"]
        .as_object()
        .ok_or("fresh artifact map")?
        .values()
    {
        let path = local_path(
            evidence_output,
            row["file"].as_str().ok_or("fresh artifact name")?,
        )?;
        let digest = row["sha256"]
            .as_str()
            .ok_or("fresh artifact hash")?
            .to_owned();
        need(
            input_pins.insert(path, digest).is_none(),
            "duplicate fresh evidence path",
        )?;
    }
    for row in evidence["headers"]
        .as_array()
        .ok_or("fresh ancestry headers")?
    {
        let file = &row["header"];
        let path = local_path(
            &evidence_output.join("headers"),
            file["file"].as_str().ok_or("ancestry header name")?,
        )?;
        let digest = file["sha256"]
            .as_str()
            .ok_or("ancestry header hash")?
            .to_owned();
        need(
            input_pins.insert(path, digest).is_none(),
            "duplicate fresh ancestry path",
        )?;
    }
    verify_files(&input_pins)?;
    let genesis_path = Path::new(GENESIS_SPEC);
    let spec = read(genesis_path, 32 * 1024 * 1024)?;
    let (genesis_header, mut current_keys) = genesis(&spec)?;
    input_pins.insert(genesis_path.to_owned(), hash(&spec));
    fs::create_dir_all(&allowed).map_err(|e| e.to_string())?;
    need(
        fs::canonicalize(&allowed).map_err(|e| e.to_string())? == allowed,
        "aliased authority output parent",
    )?;
    fs::create_dir(output_dir).map_err(|e| e.to_string())?;
    let logs = output_dir.join("source-read-only-logs");
    fs::create_dir(&logs).map_err(|e| e.to_string())?;
    let mut options = Options::default();
    options.create_if_missing(false);
    options.create_missing_column_families(false);
    options.set_db_log_dir(&logs);
    let families = DB::list_cf(&options, &database).map_err(|e| e.to_string())?;
    let db = DB::open_cf_for_read_only(&options, &database, families, false)
        .map_err(|e| e.to_string())?;
    need(
        header_at(&db, 0)?.0 == genesis_header,
        "owned database genesis header differs",
    )?;
    let mut pins = BTreeMap::new();
    let mut bytes = 0usize;
    let mut artifacts = BTreeMap::new();
    artifacts.insert(
        "genesis-chain-spec.json",
        retain(
            output_dir,
            "genesis-chain-spec.json",
            &spec,
            &mut pins,
            &mut bytes,
        )?,
    );
    artifacts.insert(
        "fresh-consensus-evidence.json",
        retain(
            output_dir,
            "fresh-consensus-evidence.json",
            &serde_json::to_vec(&evidence).map_err(|e| e.to_string())?,
            &mut pins,
            &mut bytes,
        )?,
    );
    let mut anchored = Vec::new();
    for name in ["grandpa-voters.scale", "grandpa-best-justification.scale"] {
        let path = evidence_output.join(name);
        let raw = read(&path, MAX_VALUE)?;
        need(
            evidence["artifacts"][name]["sha256"] == hash(&raw),
            "fresh consensus artifact changed",
        )?;
        need(
            input_pins.get(&path) == Some(&hash(&raw)),
            "fresh authority artifact pin mismatch",
        )?;
        let db_key = if name == "grandpa-voters.scale" {
            b"grandpa_voters".as_slice()
        } else {
            b"grandpa_best_justification".as_slice()
        };
        need(
            get(&db, 8, db_key)? == raw,
            "consensus cache changed between inspections",
        )?;
        artifacts.insert(name, retain(output_dir, name, &raw, &mut pins, &mut bytes)?);
        anchored.push(raw);
    }
    let cache: AuthoritySet<H256, u32> = decode(&anchored[0])?;
    let mut prefix = anchored[0].as_slice();
    let expected_keys =
        authorities(AuthorityList::decode(&mut prefix).map_err(|e| e.to_string())?)?;
    let shared: SharedAuthoritySet<H256, u32> = cache.into();
    let expected_set = shared.set_id();
    need(
        evidence["currentSetId"].as_u64() == Some(expected_set),
        "fresh set identity mismatch",
    )?;
    let history = shared
        .authority_set_changes()
        .iter_from(0)
        .ok_or("authority history missing")?
        .copied()
        .collect::<Vec<_>>();
    need(
        history.len() <= 100_000
            && history.iter().enumerate().all(|(i, (set, n))| {
                *set == i as u64 && *n < CHECKPOINT && (i == 0 || *n > history[i - 1].1)
            })
            && history
                .last()
                .is_some_and(|(set, _)| set.checked_add(1) == Some(expected_set)),
        "authority boundary index changed",
    )?;
    let mut verified_set = 0u64;
    let mut gaps = Vec::new();
    let mut boundaries = Vec::new();
    let boundary_dir = output_dir.join("boundaries");
    fs::create_dir(&boundary_dir).map_err(|e| e.to_string())?;
    for (index, (set, number)) in history.iter().enumerate() {
        if index % 100 == 0 {
            eprintln!("authority proof: boundary {index}/{}, height {number}, authenticated prefix {verified_set}",history.len());
        }
        let prefix_intact = gaps.is_empty();
        let mut row = json!({"setId":set,"number":number,"signatureVerified":false});
        let attempt = (|| -> Result<AuthorityList> {
            let (header, raw_header) = header_at(&db, *number)?;
            row["hash"] = json!(format!("{:#x}", header.hash()));
            row["header"] = retain(
                &boundary_dir,
                &format!("{set}-header.scale"),
                &raw_header,
                &mut pins,
                &mut bytes,
            )?;
            let lookup = [number.to_be_bytes().as_slice(), header.hash().as_bytes()].concat();
            let all = get(&db, 6, &lookup)?;
            row["justifications"] = retain(
                &boundary_dir,
                &format!("{set}-justifications.scale"),
                &all,
                &mut pins,
                &mut bytes,
            )?;
            let proof = grandpa_proof(&all)?;
            let decoded: GrandpaJustification<ChainBlock> = decode(&proof)?;
            need(
                decoded.target() == (*number, header.hash()),
                "boundary justification target mismatch",
            )?;
            let next = scheduled(&header)?;
            if prefix_intact {
                need(*set == verified_set, "authenticated set sequence mismatch")?;
                handoff(&header, &proof, verified_set, &current_keys)
            } else {
                Ok(next)
            }
        })();
        match attempt {
            Ok(next) if prefix_intact => {
                current_keys = next;
                verified_set = verified_set.checked_add(1).ok_or("set overflow")?;
                row["signatureVerified"] = json!(true);
            }
            Ok(_) => {
                row["blockedByEarlierGap"] = json!(true);
            }
            Err(error) => {
                row["error"] = json!(error);
                gaps.push(json!({"setId":set,"number":number,"reason":error}));
            }
        }
        boundaries.push(row);
    }
    let mut verified = gaps.is_empty() && verified_set == expected_set;
    if verified && current_keys != expected_keys {
        gaps.push(json!({"reason":"authenticated final authorities differ from root-verified parent/checkpoint state"}));
        verified = false;
    }
    if verified {
        let best: GrandpaJustification<ChainBlock> = decode(&anchored[1])?;
        if let Err(error) = best.verify(verified_set, &current_keys) {
            gaps.push(json!({"reason":format!("final proof under authenticated authority chain: {error}")}));
            verified = false;
        }
    }
    drop(db);
    eprintln!("authority proof: checking retained outputs and unchanged owned source inventory");
    need(
        inventory(&database)? == before,
        "owned database changed during authority verification",
    )?;
    verify_files(&input_pins)?;
    verify_files(&pins)?;
    let report = json!({"schema":"sora-nexus-retained-authority-chain-v1","database":database,
        "engineReadOnly":true,"genesis":GENESIS,"genesisStorageAndHeaderVerified":true,
        "proofModel":"zero-delay scheduled GRANDPA handoffs with SDK OnDisabled interpretation, complete signed digest, old-set signatures, no hardfork overrides",
        "expectedFinalSet":expected_set,"authenticatedPrefixSets":verified_set,"boundaryCount":history.len(),
        "fullGenesisAuthorityChainVerified":verified,"checkpointBoundByFreshVerifiedFinality":verified,
        "boundaries":boundaries,"gaps":gaps,"artifacts":artifacts,"inputPins":input_pins,
        "allHistoricalHeadersVerified":false,"fullBabeVerification":false,"runtimeExecuted":false,
        "recoveryConsensusHistoryAvailable":false,"operatorApprovalVerified":false,"productionReady":false});
    let raw = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    let receipt = retain(
        output_dir,
        "AUTHORITY-CHAIN.json",
        &raw,
        &mut pins,
        &mut bytes,
    )?;
    verify_files(&input_pins)?;
    verify_files(&pins)?;
    Ok(
        json!({"schema":"sora-nexus-retained-authority-chain-completion-v1","receipt":receipt,
        "output":output_dir,"fullGenesisAuthorityChainVerified":verified,"productionReady":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_consensus_grandpa::{AuthorityPair, ScheduledChange};
    fn pair(seed: u8) -> AuthorityPair {
        AuthorityPair::from_seed(&[seed; 32])
    }
    fn keys(seed: u8) -> AuthorityList {
        vec![(pair(seed).public(), 1)]
    }
    fn header(logs: Vec<ConsensusLog<u32>>) -> ChainHeader {
        ChainHeader::new(
            10,
            H256::repeat_byte(1),
            H256::repeat_byte(2),
            H256::repeat_byte(3),
            sp_runtime::Digest {
                logs: logs
                    .into_iter()
                    .map(|log| DigestItem::Consensus(GRANDPA_ENGINE_ID, log.encode()))
                    .collect(),
            },
        )
    }
    fn change(seed: u8, delay: u32) -> ConsensusLog<u32> {
        ConsensusLog::ScheduledChange(ScheduledChange {
            next_authorities: keys(seed),
            delay,
        })
    }
    fn proof(header: &ChainHeader, set: u64, seed: u8) -> Vec<u8> {
        let precommit = sp_consensus_grandpa::Precommit::<ChainHeader> {
            target_hash: header.hash(),
            target_number: *header.number(),
        };
        let message = sp_consensus_grandpa::Message::<ChainHeader>::Precommit(precommit.clone());
        let key = pair(seed);
        let signature = key.sign(&sp_consensus_grandpa::localized_payload(5, set, &message));
        // SCALE field order is identical to SDK Commit/SignedPrecommit; decode to
        // the SDK type before constructing the actual justification.
        let commit = decode::<sp_consensus_grandpa::Commit<ChainHeader>>(
            &(
                header.hash(),
                *header.number(),
                vec![(precommit, signature, key.public())],
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
    #[test]
    fn valid_old_set_signature_is_required_for_each_handoff() {
        let h = header(vec![change(2, 0)]);
        assert_eq!(handoff(&h, &proof(&h, 0, 1), 0, &keys(1)).unwrap(), keys(2));
        assert!(handoff(&h, &proof(&h, 1, 1), 0, &keys(1)).is_err());
        assert!(handoff(&h, &proof(&h, 0, 2), 0, &keys(1)).is_err());
        let altered = header(vec![change(3, 0)]);
        assert!(handoff(&altered, &proof(&h, 0, 1), 0, &keys(1)).is_err());
    }
    #[test]
    fn two_handoffs_use_new_keys_and_incremented_set_without_cache_authority_trust() {
        let first = header(vec![change(2, 0)]);
        let next = handoff(&first, &proof(&first, 0, 1), 0, &keys(1)).unwrap();
        let second = header(vec![change(3, 0)]);
        assert_eq!(
            handoff(&second, &proof(&second, 1, 2), 1, &next).unwrap(),
            keys(3)
        );
        assert!(handoff(&second, &proof(&second, 1, 1), 1, &next).is_err());
        assert!(handoff(&second, &proof(&second, 0, 2), 1, &next).is_err());
        assert!(handoff(&second, &proof(&second, 1, 2), 0, &keys(1)).is_err());
    }
    #[test]
    fn unsupported_or_ambiguous_digests_never_become_handoffs() {
        for logs in [
            vec![],
            vec![change(2, 1)],
            vec![change(2, 0), change(3, 0)],
            vec![ConsensusLog::ForcedChange(
                0,
                ScheduledChange {
                    next_authorities: keys(2),
                    delay: 0,
                },
            )],
            vec![ConsensusLog::OnDisabled(0)],
            vec![ConsensusLog::Pause(0)],
            vec![ConsensusLog::Resume(0)],
        ] {
            assert!(scheduled(&header(logs)).is_err());
        }
    }
    #[test]
    fn disabled_notices_preserve_sdk_scheduled_handoff_and_old_set_quorum() {
        for logs in [
            vec![ConsensusLog::OnDisabled(0), change(2, 0)],
            vec![change(2, 0), ConsensusLog::OnDisabled(0)],
            vec![
                ConsensusLog::OnDisabled(0),
                change(2, 0),
                ConsensusLog::OnDisabled(0),
            ],
        ] {
            let h = header(logs);
            let sdk = sc_finality_grandpa::find_scheduled_change::<ChainBlock>(&h).unwrap();
            assert_eq!(scheduled(&h).unwrap(), sdk.next_authorities);
            assert_eq!(handoff(&h, &proof(&h, 0, 1), 0, &keys(1)).unwrap(), keys(2));
            assert!(handoff(&h, &proof(&h, 0, 2), 0, &keys(1)).is_err());
            assert!(handoff(&h, &proof(&h, 1, 1), 0, &keys(1)).is_err());
        }
    }
    #[test]
    fn disabled_notice_cannot_be_added_or_removed_from_a_signed_boundary() {
        let plain = header(vec![change(2, 0)]);
        let disabled = header(vec![ConsensusLog::OnDisabled(0), change(2, 0)]);
        assert!(handoff(&disabled, &proof(&plain, 0, 1), 0, &keys(1)).is_err());
        assert!(handoff(&plain, &proof(&disabled, 0, 1), 0, &keys(1)).is_err());
        let ambiguous = header(vec![
            ConsensusLog::OnDisabled(0),
            change(2, 0),
            change(3, 0),
        ]);
        assert!(handoff(&ambiguous, &proof(&ambiguous, 0, 1), 0, &keys(1)).is_err());
        let forced = header(vec![
            ConsensusLog::OnDisabled(0),
            ConsensusLog::ForcedChange(
                0,
                ScheduledChange {
                    next_authorities: keys(2),
                    delay: 0,
                },
            ),
        ]);
        assert!(handoff(&forced, &proof(&forced, 0, 1), 0, &keys(1)).is_err());
    }
    #[test]
    fn duplicate_engine_and_missing_grandpa_proofs_are_rejected() {
        let h = header(vec![change(2, 0)]);
        let p = proof(&h, 0, 1);
        assert_eq!(
            grandpa_proof(&Justifications::new(vec![(GRANDPA_ENGINE_ID, p.clone())]).encode())
                .unwrap(),
            p
        );
        assert!(grandpa_proof(
            &Justifications::new(vec![(GRANDPA_ENGINE_ID, p.clone()), (GRANDPA_ENGINE_ID, p)])
                .encode()
        )
        .is_err());
        assert!(grandpa_proof(&Justifications::default().encode()).is_err());
    }
    #[test]
    fn full_genesis_storage_is_committed_not_only_authority_bytes() {
        let mut top = BTreeMap::from([
            (b":grandpa_authorities".to_vec(), (1u8, keys(1)).encode()),
            (b"other".to_vec(), vec![2]),
        ]);
        let (first, first_keys) = genesis_from_top(top.clone()).unwrap();
        top.insert(b"other".to_vec(), vec![3]);
        let (second, second_keys) = genesis_from_top(top.clone()).unwrap();
        assert_ne!(first.hash(), second.hash());
        assert_eq!(first_keys, second_keys);
        top.insert(b":grandpa_authorities".to_vec(), (2u8, keys(1)).encode());
        assert!(genesis_from_top(top).is_err());
        assert!(canonical_hex(&json!("0xAB")).is_err());
    }
    #[test]
    fn bounded_fixed_genesis_input_rejects_unpinned_json() {
        assert!(genesis(b"{}").is_err());
    }
}
