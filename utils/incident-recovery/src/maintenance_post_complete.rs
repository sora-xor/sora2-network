//! PUBLIC FIXTURE ONLY: ordinary HASHI admission under exact final-138 Wasm.
//! The disposable setup overlay and synthetic parent are NOT canonical recovery
//! history. No production bridge halt, source snapshot or release is modified.

use super::*;
use common::AssetInfoProvider;
use frame_support::{dispatch::DispatchClass, weights::Weight};
use framenode_runtime::{AccountId, Runtime, RuntimeCall, System};
use serde_json::{json, Value};
use sp_core::{ed25519, Pair, H160};
use sp_runtime::{
    generic::Era,
    transaction_validity::{InvalidTransaction, TransactionValidityError},
    MultiSignature,
};
use std::{io::Write, path::PathBuf};

const SCHEMA: &str = "sora-nexus-PUBLIC-FIXTURE-post-complete-hashi-v1";
const SOURCE_MANIFEST_SHA: &str =
    "bac24ade6ce4c3522bf2af699ff56998b01458ae0b2d4773c7137e737ea47b89";
const SOURCE_REPORT_SHA: &str = "6a5a22eaf2090b1ec27935f5c9ed1b8e1a485e842bc1bbfd32597c32fbe92544";
const SOURCE_CODE_SHA: &str = "26b724485a5b5d597358a59289776d68efde6f6f640b7ae1dc1a110140e35754";
const DOMAIN: &[u8] = b"sora-nexus/PUBLIC-FIXTURE-ONLY/post-complete-hashi/v1/";
const MINT: u128 = 1_000_000_000_000_000_000;
const MAX_INPUT: usize = 64 * 1024 * 1024;

type Transfer = eth_bridge::requests::IncomingTransfer<Runtime>;

/// Match the client's BlockBuilder::push transaction boundary. The Executive
/// may record inherents/extrinsic bookkeeping before a validity error; rejected
/// candidates must not leave those writes in the authored diagnostic block.
fn with_candidate_transaction(
    ext: &mut sp_io::TestExternalities,
    apply: impl FnOnce(&mut sp_io::TestExternalities) -> Result<ApplyExtrinsicResult>,
) -> Result<ApplyExtrinsicResult> {
    let prior_depth = ext.overlayed_changes().transaction_depth();
    ext.execute_with(sp_io::storage::start_transaction);
    let result = apply(ext);
    let depth = ext.overlayed_changes().transaction_depth();
    if matches!(result, Ok(Ok(_))) && depth == prior_depth + 1 {
        // An included extrinsic with a dispatch error still consumes its nonce
        // and records its events, just as an included successful dispatch does.
        ext.execute_with(sp_io::storage::commit_transaction);
        return result;
    }
    // A host failure can interrupt a runtime-owned nested transaction. Discard
    // all layers opened by this candidate, while preserving the caller's layer.
    while ext.overlayed_changes().transaction_depth() > prior_depth {
        ext.execute_with(sp_io::storage::rollback_transaction);
    }
    if depth != prior_depth + 1 && result.is_ok() {
        return Err("diagnostic candidate left an unbalanced storage transaction".into());
    }
    if depth <= prior_depth {
        return Err("diagnostic candidate closed the client storage transaction".into());
    }
    result
}

pub(super) fn apply_candidate(
    executor: &replay::Executor,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
    extrinsic: &OpaqueExtrinsic,
) -> Result<ApplyExtrinsicResult> {
    with_candidate_transaction(ext, |ext| super::apply(executor, ext, code, extrinsic))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub input_root: PathBuf,
    pub source_manifest: replay::Artifact,
    pub source_report: replay::Artifact,
    pub source_execution: replay::Artifact,
    pub source_last_block: replay::Artifact,
    pub snapshot_directory: String,
    pub snapshot_manifest: String,
    pub snapshot_sha256: String,
    pub candidate_runtime: replay::Artifact,
    /// Hash of the exact policy input used for candidate Wasm compilation.
    /// The positive original-recipient branch must also prove that binding in execution.
    pub candidate_policy: replay::Artifact,
}

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut f| f.write_all(bytes))
        .map_err(|e| e.to_string())
}
fn write_json(path: &Path, value: &Value) -> Result<()> {
    write_new(
        path,
        &serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
}
fn require(condition: bool, error: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(error.into())
    }
}
fn h256(text: &str) -> Result<H256> {
    let bytes = hex_bytes(text)?;
    require(bytes.len() == 32, "expected a 32-byte hash")?;
    Ok(H256::from_slice(&bytes))
}

struct Source {
    header: Header,
    baseline: Vec<u8>,
    candidate: Vec<u8>,
    policy: RecoveryPolicy,
    snapshot_directory: PathBuf,
    manifest: Manifest,
    slot: u64,
    timestamp: u64,
    authority_index: u32,
    evidence: Value,
}

impl Source {
    fn restore(&self) -> Result<sp_io::TestExternalities> {
        snapshot::restore_pinned(
            &self.snapshot_directory,
            &self.manifest.snapshot_manifest,
            &self.manifest.snapshot_sha256,
            *self.header.state_root(),
        )
    }
}

fn load(path: &Path) -> Result<Source> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_INPUT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    require(
        bytes.len() <= MAX_INPUT,
        "diagnostic manifest exceeds bound",
    )?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    require(
        manifest.schema == SCHEMA && manifest.input_root.is_absolute(),
        "invalid public diagnostic schema/root",
    )?;
    let root = manifest
        .input_root
        .canonicalize()
        .map_err(|e| e.to_string())?;
    require(root == manifest.input_root, "input root must be canonical")?;
    require(
        manifest.source_manifest.sha256 == SOURCE_MANIFEST_SHA
            && manifest.source_report.sha256 == SOURCE_REPORT_SHA,
        "diagnostic accepts only the reviewed HASHI38 source cohort",
    )?;
    let source_bytes = replay::artifact(&root, &manifest.source_manifest, MAX_INPUT)?;
    let source: super::Manifest =
        serde_json::from_slice(&source_bytes).map_err(|e| e.to_string())?;
    let report_bytes = replay::artifact(&root, &manifest.source_report, MAX_INPUT)?;
    let report: Value = serde_json::from_slice(&report_bytes).map_err(|e| e.to_string())?;
    let execution_bytes = replay::artifact(&root, &manifest.source_execution, MAX_INPUT)?;
    let execution: Value = serde_json::from_slice(&execution_bytes).map_err(|e| e.to_string())?;
    require(
        execution["exitCode"] == 0
            && execution["status"] == "passed"
            && execution["productionReady"] == false
            && execution["fixtureOnly"] == true
            && execution["inputChanges"] == json!([])
            && execution["report"]["sha256"] == manifest.source_report.sha256,
        "source execution receipt is incomplete or differs",
    )?;
    let blocks = report["blocks"]
        .as_array()
        .ok_or("source report has no block list")?;
    require(
        blocks.len() == 38
            && source.steps.len() == 38
            && report["manifestSha256"] == manifest.source_manifest.sha256
            && report["publicTestHashiFixture"] == true
            && report["automaticActivationObserved"] == true
            && report["ordinaryRuntimeBlocks"] == 2
            && report["bridgeHaltRetained"] == true
            && report["nativeExecution"] == false
            && report["productionReady"] == false
            && report["operatorApprovalVerified"] == false
            && report["consensusSealsVerified"] == false
            && source.schema == hashi_fixture::SCHEMA,
        "source report is not the completed exact-Wasm HASHI38 fixture",
    )?;
    let last = blocks.last().ok_or("source report is empty")?;
    require(
        last["number"] == 28_191_855
            && last["phase"] == "Complete"
            && last["resultingSpec"] == 138
            && last["independentImportRootMatches"] == true
            && last["diskRestartRootMatches"] == true
            && last["blockSha256"] == manifest.source_last_block.sha256
            && last["diskSnapshotManifestSha256"] == manifest.snapshot_sha256
            && manifest.snapshot_manifest == "snapshots/000038.json",
        "source terminal block/snapshot binding differs",
    )?;
    let block_bytes = replay::artifact(&root, &manifest.source_last_block, MAX_INPUT)?;
    let block = RuntimeBlock::decode_all(&mut block_bytes.as_slice()).map_err(|e| e.to_string())?;
    require(
        block.header.hash() == h256(last["hash"].as_str().ok_or("missing source hash")?)?
            && *block.header.state_root()
                == h256(last["stateRoot"].as_str().ok_or("missing source root")?)?
            && *block.header.extrinsics_root()
                == sp_io::trie::blake2_256_ordered_root(
                    block.extrinsics.iter().map(Encode::encode).collect(),
                    StateVersion::V0,
                ),
        "source terminal header/extrinsics are not bound",
    )?;
    let policy_bytes = replay::artifact(&root, &source.policy, recovery::MAX_INHERENT_BYTES * 3)?;
    let mut input = policy_bytes.as_slice();
    let policy =
        RecoveryPolicy::decode_with_mem_limit(&mut input, recovery::MAX_INHERENT_BYTES * 6)
            .map_err(|e| e.to_string())?;
    require(
        input.is_empty() && policy.encode() == policy_bytes,
        "noncanonical source policy",
    )?;
    hashi_fixture::validate(
        &root,
        source
            .fixture_hashi_peers
            .as_ref()
            .ok_or("source peer fixture missing")?,
        &policy,
    )?;
    require(
        report["policyCommitment"] == hex::encode(policy.commitment())
            && source.final_runtime.sha256 == SOURCE_CODE_SHA
            && report["artifactSha256"]["final-138"] == source.final_runtime.sha256,
        "source final runtime/policy binding differs",
    )?;
    let baseline = replay::artifact(&root, &source.final_runtime, MAX_INPUT)?;
    let candidate = replay::artifact(&root, &manifest.candidate_runtime, MAX_INPUT)?;
    let candidate_policy = replay::artifact(
        &root,
        &manifest.candidate_policy,
        recovery::MAX_INHERENT_BYTES * 3,
    )?;
    require(
        candidate_policy == policy_bytes && candidate != baseline,
        "candidate must use exact HASHI38 policy and changed runtime bytes",
    )?;
    let snapshot_directory = local_path(&root, &manifest.snapshot_directory)?;
    require(
        snapshot_directory == root.join(&manifest.snapshot_directory),
        "snapshot directory must have a canonical non-aliased path",
    )?;
    let last_step = source.steps.last().ok_or("source steps absent")?;
    let evidence = json!({"manifestSha256":sha(&bytes),"sourceExecutionSha256":sha(&execution_bytes),
        "sourceManifestSha256":sha(&source_bytes),"sourceReportSha256":sha(&report_bytes),
        "sourceBlockSha256":sha(&block_bytes),"sourceHeaderScale":hx(&block.header.encode()),
        "sourceHeaderHash":format!("{:#x}",block.header.hash()),"sourceStateRoot":format!("{:#x}",block.header.state_root()),
        "sourceSnapshotManifestSha256":manifest.snapshot_sha256,"sourcePolicySha256":sha(&policy_bytes),
        "policyCommitment":hx(&policy.commitment()),"baselineCodeSha256":sha(&baseline),"candidateCodeSha256":sha(&candidate)});
    Ok(Source {
        header: block.header,
        baseline,
        candidate,
        policy,
        snapshot_directory,
        manifest,
        slot: last_step.slot.checked_add(1).ok_or("slot overflow")?,
        timestamp: last_step
            .timestamp
            .checked_add(6_000)
            .ok_or("timestamp overflow")?,
        authority_index: last_step.authority_index,
        evidence,
    })
}

fn assert_complete(
    ext: &mut sp_io::TestExternalities,
    policy: &RecoveryPolicy,
    halted: bool,
) -> Result<()> {
    let current = status(ext)?;
    require(
        current.phase == RecoveryPhase::Complete
            && current.policy == policy.commitment()
            && value(ext, recovery::POLICY_STORAGE_KEY).as_deref()
                == Some(policy.commitment().as_slice())
            && value(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
                == Some(policy.commitment().as_slice())
            && value(ext, recovery::BRIDGE_HALT_STORAGE_KEY) == halted.then(|| vec![1])
            && recovery::TRANSIENT_CONTEXT_KEYS
                .iter()
                .all(|key| value(ext, key).is_none()),
        "diagnostic phase/policy/halt/transient context differs",
    )
}

fn financial(ext: &mut sp_io::TestExternalities) -> Result<[u128; 3]> {
    ext.execute_with(|| {
        Ok([
            framenode_runtime::Assets::free_balance(
                &common::DAI,
                &hashi_fixture::owner(b"owner-old"),
            )
            .map_err(|e| format!("old balance: {e:?}"))?,
            framenode_runtime::Assets::free_balance(
                &common::DAI,
                &hashi_fixture::owner(b"owner-new"),
            )
            .map_err(|e| format!("new balance: {e:?}"))?,
            framenode_runtime::Assets::total_issuance(&common::DAI)
                .map_err(|e| format!("issuance: {e:?}"))?,
        ])
    })
}

fn setup(
    source: &Source,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
    branch: &Branch,
    transfer: &Transfer,
) -> Result<Value> {
    let reopen = branch.reopen;
    let executor = replay::Executor::builder().build();
    require(
        replay::code(ext)? == source.baseline,
        "restored snapshot has different code",
    )?;
    assert_complete(ext, &source.policy, true)?;
    let version = replay::version(&executor, ext, &source.baseline)?;
    require(
        (version.spec_version, version.transaction_version) == (138, 134),
        "source is not runtime138/tx134",
    )?;
    ext.commit_all().map_err(|e| e.to_string())?;
    let before_root = replay::root(ext);
    let before_financial = financial(ext)?;
    let peers: Vec<_> = hashi_fixture::pairs().into_iter().map(|(a, _)| a).collect();
    let (network, bridge) = ext.execute_with(|| {
        let network = framenode_runtime::GetEthNetworkId::get();
        (network, eth_bridge::BridgeAccount::<Runtime>::get(network))
    });
    let bridge = bridge.ok_or("HASHI bridge absent")?;
    let mut edits = Vec::<(Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>)>::new();
    for peer in &peers {
        let key = frame_system::Account::<Runtime>::hashed_key_for(peer);
        require(
            value(ext, &key).is_none(),
            "restored public peer account unexpectedly exists",
        )?;
        let account = framenode_runtime::AccountData {
            providers: 1,
            ..Default::default()
        };
        edits.push((key, None, Some(account.encode())));
    }
    for (key, after) in [
        (
            eth_bridge::Peers::<Runtime>::hashed_key_for(network),
            peers.iter().cloned().collect::<BTreeSet<_>>().encode(),
        ),
        (
            bridge_multisig::Accounts::<Runtime>::hashed_key_for(&bridge),
            bridge_multisig::MultisigAccount::new(peers).encode(),
        ),
    ] {
        let before = value(ext, &key);
        edits.push((key, before, Some(after)));
    }
    if code != source.baseline {
        edits.push((
            b":code".to_vec(),
            Some(source.baseline.clone()),
            Some(code.to_vec()),
        ));
    }
    if reopen {
        edits.push((
            recovery::BRIDGE_HALT_STORAGE_KEY.to_vec(),
            Some(vec![1]),
            None,
        ));
    }
    if branch.pending {
        let load = eth_bridge::requests::OffchainRequest::<Runtime>::LoadIncoming(
            eth_bridge::requests::LoadIncomingRequest::Transaction(
                eth_bridge::requests::LoadIncomingTransactionRequest::new(
                    transfer.author.clone(),
                    transfer.tx_hash,
                    transfer.timepoint,
                    eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                    network,
                ),
            ),
        );
        for (key, after) in [
            (
                eth_bridge::Requests::<Runtime>::hashed_key_for(network, transfer.tx_hash),
                load.encode(),
            ),
            (
                eth_bridge::RequestStatuses::<Runtime>::hashed_key_for(network, transfer.tx_hash),
                eth_bridge::requests::RequestStatus::Pending.encode(),
            ),
            (
                eth_bridge::RequestSubmissionHeight::<Runtime>::hashed_key_for(
                    network,
                    transfer.tx_hash,
                ),
                source.header.number().encode(),
            ),
        ] {
            require(
                value(ext, &key).is_none(),
                "retained fixture load key collides with authenticated source",
            )?;
            edits.push((key, None, Some(after)));
        }
        let queue_key = eth_bridge::RequestsQueue::<Runtime>::hashed_key_for(network);
        let query_key = eth_bridge::AccountRequests::<Runtime>::hashed_key_for(
            hashi_fixture::owner(b"owner-new"),
        );
        let (mut queue, mut query) = ext.execute_with(|| {
            (
                eth_bridge::RequestsQueue::<Runtime>::get(network),
                eth_bridge::AccountRequests::<Runtime>::get(hashi_fixture::owner(b"owner-new")),
            )
        });
        require(
            !queue.contains(&transfer.tx_hash) && !query.contains(&(network, transfer.tx_hash)),
            "duplicate public pending load",
        )?;
        queue.push(transfer.tx_hash);
        query.push((network, transfer.tx_hash));
        for (key, after) in [(queue_key, queue.encode()), (query_key, query.encode())] {
            let before = value(ext, &key);
            edits.push((key, before, Some(after)));
        }
    }
    let mut rows = Vec::new();
    for (key, before, after) in &edits {
        require(
            value(ext, key) == *before,
            "diagnostic setup preimage differs",
        )?;
        ext.execute_with(|| match after {
            Some(v) => sp_io::storage::set(key, v),
            None => sp_io::storage::clear(key),
        });
        rows.push(json!({"key":hx(key),"valueEncoding":if key==b":code" {"bound-runtime-artifact"} else {"inline-hex"},
            "before":if key==b":code" {None} else {before.as_ref().map(|v|hx(v))},
            "after":if key==b":code" {None} else {after.as_ref().map(|v|hx(v))},
            "beforeBytes":before.as_ref().map(Vec::len),"afterBytes":after.as_ref().map(Vec::len),
            "beforeSha256":before.as_ref().map(|v|sha(v)),"afterSha256":after.as_ref().map(|v|sha(v))}));
    }
    let changed: BTreeSet<_> = ext
        .overlayed_changes()
        .changes()
        .map(|(k, _)| k.clone())
        .collect();
    require(
        changed == edits.iter().map(|(k, _, _)| k.clone()).collect(),
        "setup changed a non-allowlisted key",
    )?;
    require(
        financial(ext)? == before_financial,
        "setup changed balances or issuance",
    )?;
    assert_complete(ext, &source.policy, !reopen)?;
    let version = replay::version(&executor, ext, code)?;
    require(
        (version.spec_version, version.transaction_version) == (138, 134),
        "candidate is not runtime138/tx134",
    )?;
    Ok(
        json!({"sourceStateRoot":format!("{before_root:#x}"),"overlayStateRoot":format!("{:#x}",replay::root(ext)),
        "writes":rows,"reopenedOnlyInDisposableOverlay":reopen,"syntheticRetainedPendingLoad":branch.pending,"nativeEconomicDispatch":false}),
    )
}

fn transfer(ext: &mut sp_io::TestExternalities, to: AccountId) -> Result<Transfer> {
    ext.execute_with(|| {
        let network = framenode_runtime::GetEthNetworkId::get();
        require(
            framenode_runtime::EthBridge::registered_asset(network, common::DAI)
                == Some(eth_bridge::requests::AssetKind::Sidechain),
            "fixture asset no longer sidechain DAI",
        )?;
        let author = to.clone();
        let transfer = Transfer {
            // The native Deposit constructor does not encode an EVM sender.
            from: H160::zero(),
            to,
            asset_id: common::DAI,
            asset_kind: eth_bridge::requests::AssetKind::Sidechain,
            amount: MINT,
            author,
            tx_hash: H256::from(sp_io::hashing::blake2_256(
                &[DOMAIN, b"fresh-transfer"].concat(),
            )),
            at_height: 123,
            timepoint: framenode_runtime::BridgeMultisig::thischain_timepoint(),
            network_id: network,
            should_take_fee: false,
        };
        let incoming = eth_bridge::requests::OffchainRequest::<Runtime>::incoming(
            eth_bridge::requests::IncomingRequest::Transfer(transfer.clone()),
        );
        for id in [transfer.tx_hash, incoming.hash()] {
            require(
                !eth_bridge::Requests::<Runtime>::contains_key(network, id)
                    && !eth_bridge::RequestStatuses::<Runtime>::contains_key(network, id)
                    && !eth_bridge::LoadToIncomingRequestHash::<Runtime>::contains_key(network, id)
                    && !eth_bridge::RequestsQueue::<Runtime>::get(network).contains(&id),
                "fresh fixture request collides with existing bridge state",
            )?;
        }
        Ok(transfer)
    })
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Import,
    Register,
    Finalize,
}
fn original_load(transfer: &Transfer) -> eth_bridge::requests::OffchainRequest<Runtime> {
    eth_bridge::requests::OffchainRequest::LoadIncoming(
        eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                transfer.author.clone(),
                transfer.tx_hash,
                transfer.timepoint,
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                transfer.network_id,
            ),
        ),
    )
}

fn inner_call(transfer: &Transfer, stage: Stage) -> RuntimeCall {
    if matches!(stage, Stage::Register) {
        return RuntimeCall::EthBridge(eth_bridge::Call::register_incoming_request {
            incoming_request: eth_bridge::requests::IncomingRequest::Transfer(transfer.clone()),
        });
    }
    if matches!(stage, Stage::Finalize) {
        let incoming = eth_bridge::requests::OffchainRequest::<Runtime>::incoming(
            eth_bridge::requests::IncomingRequest::Transfer(transfer.clone()),
        );
        return RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
            hash: incoming.hash(),
            network_id: transfer.network_id,
        });
    }
    RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
        load_incoming_request: eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                transfer.author.clone(),
                transfer.tx_hash,
                transfer.timepoint,
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                transfer.network_id,
            ),
        ),
        incoming_request_result: Ok(eth_bridge::requests::IncomingRequest::Transfer(
            transfer.clone(),
        )),
    })
}

fn queried_weight(
    executor: &replay::Executor,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
    call: &RuntimeCall,
) -> Result<(Weight, Value)> {
    let bare = framenode_runtime::UncheckedExtrinsic::new_bare(call.clone());
    let bytes = bare.encode();
    let before = replay::root(ext);
    let args = [bytes.clone(), (bytes.len() as u32).encode()].concat();
    let response = replay::call(
        executor,
        ext,
        code,
        "TransactionPaymentApi_query_info",
        &args,
    )?;
    require(
        replay::root(ext) == before,
        "weight runtime API changed state",
    )?;
    let (weight, class, fee) =
        <(Weight, DispatchClass, u128)>::decode_all(&mut response.as_slice())
            .map_err(|e| e.to_string())?;
    require(weight.ref_time() > 0, "native inner call weight is zero")?;
    Ok((
        weight,
        json!({"method":"TransactionPaymentApi_query_info","inputSha256":sha(&args),"outputScale":hx(&response),
        "weightRefTime":weight.ref_time(),"weightProofSize":weight.proof_size(),"class":format!("{class:?}"),"partialFee":fee.to_string(),
        "executingCodeSha256":sha(code),"stateRoot":format!("{before:#x}")}),
    ))
}

fn outer_call(
    ext: &mut sp_io::TestExternalities,
    transfer: &Transfer,
    stage: Stage,
    max_weight: Weight,
) -> Result<RuntimeCall> {
    let inner = inner_call(transfer, stage).encode();
    let call_hash = sp_io::hashing::blake2_256(&inner);
    let (bridge, when) = ext.execute_with(|| {
        let bridge = eth_bridge::BridgeAccount::<Runtime>::get(transfer.network_id);
        let when = bridge
            .as_ref()
            .and_then(|b| bridge_multisig::Multisigs::<Runtime>::get(b, call_hash))
            .map(|m| m.when);
        (bridge, when)
    });
    Ok(RuntimeCall::BridgeMultisig(
        bridge_multisig::Call::as_multi {
            id: bridge.ok_or("bridge absent")?,
            maybe_timepoint: when,
            call: inner,
            store_call: false,
            max_weight,
        },
    ))
}

#[derive(Clone, Copy)]
enum SignatureCase {
    Valid,
    BadSignature,
    WrongNonce,
    ChangedPayload,
}
fn signed(
    ext: &mut sp_io::TestExternalities,
    call: RuntimeCall,
    pair: &ed25519::Pair,
    parent: &Header,
    case: SignatureCase,
) -> Result<(OpaqueExtrinsic, Value)> {
    let signer = AccountId::from(pair.public().0);
    let (genesis, birth) =
        ext.execute_with(|| (System::block_hash(0), System::block_hash(*parent.number())));
    require(
        genesis.as_bytes() == recovery::GENESIS_HASH.as_slice() && birth == parent.hash(),
        "actual signed genesis/synthetic birth context differs",
    )?;
    let nonce = ext.execute_with(|| System::account_nonce(&signer));
    let nonce = if matches!(case, SignatureCase::WrongNonce) {
        nonce.checked_add(1).ok_or("nonce overflow")?
    } else {
        nonce
    };
    let era = Era::mortal(64, u64::from(*parent.number()));
    require(
        era.birth(u64::from(*parent.number() + 1)) == u64::from(*parent.number()),
        "mortal birth differs from synthetic parent",
    )?;
    let extra_bytes = (era, Compact(nonce), Compact(0u128)).encode();
    let extra = framenode_runtime::SignedExtra::decode_all(&mut extra_bytes.as_slice())
        .map_err(|e| e.to_string())?;
    let mut signed_call = call.encode();
    if matches!(case, SignatureCase::ChangedPayload) {
        signed_call.push(1);
    }
    let preimage = [
        signed_call,
        extra_bytes.clone(),
        138u32.encode(),
        134u32.encode(),
        recovery::GENESIS_HASH.to_vec(),
        parent.hash().encode(),
    ]
    .concat();
    let payload = if preimage.len() > 256 {
        sp_io::hashing::blake2_256(&preimage).to_vec()
    } else {
        preimage.clone()
    };
    let signature = if matches!(case, SignatureCase::BadSignature) {
        ed25519::Signature::from_raw([0; 64])
    } else {
        pair.sign(&payload)
    };
    let tx = framenode_runtime::UncheckedExtrinsic::new_signed(
        call.clone(),
        signer.clone(),
        MultiSignature::Ed25519(signature),
        extra,
    );
    let bytes = tx.encode();
    let opaque = OpaqueExtrinsic::decode_all(&mut bytes.as_slice()).map_err(|e| e.to_string())?;
    Ok((
        opaque,
        json!({"signer":hx(signer.as_ref()),"nonce":nonce,"era":hx(&era.encode()),"callScale":hx(&call.encode()),
        "callSha256":sha(&call.encode()),"extraScale":hx(&extra_bytes),"signingPreimageSha256":sha(&preimage),
        "birthHeaderHash":format!("{:#x}",parent.hash()),"birthHeaderNumber":parent.number(),"extrinsicSha256":sha(&bytes)}),
    ))
}

fn bridge_view(ext: &mut sp_io::TestExternalities, transfer: &Transfer) -> Value {
    ext.execute_with(|| {
        let incoming=eth_bridge::requests::OffchainRequest::<Runtime>::incoming(eth_bridge::requests::IncomingRequest::Transfer(transfer.clone()));
        let old=hashi_fixture::owner(b"owner-old");let new=hashi_fixture::owner(b"owner-new");
        json!({"incomingHash":format!("{:#x}",incoming.hash()),"externalTxHash":format!("{:#x}",transfer.tx_hash),
            "loadRequest":eth_bridge::Requests::<Runtime>::get(transfer.network_id,transfer.tx_hash).map(|r|hx(&r.encode())),
            "expectedOriginalLoadRequest":hx(&original_load(transfer).encode()),
            "loadSubmissionHeight":eth_bridge::RequestSubmissionHeight::<Runtime>::get(transfer.network_id,transfer.tx_hash),
            "loadQueueOccurrences":eth_bridge::RequestsQueue::<Runtime>::get(transfer.network_id).iter().filter(|h|**h==transfer.tx_hash).count(),
            "incomingQueueOccurrences":eth_bridge::RequestsQueue::<Runtime>::get(transfer.network_id).iter().filter(|h|**h==incoming.hash()).count(),
            "incomingRequest":eth_bridge::Requests::<Runtime>::get(transfer.network_id,incoming.hash()).map(|r|hx(&r.encode())),
            "expectedOriginalIncomingRequest":hx(&incoming.encode()),
            "loadStatus":format!("{:?}",eth_bridge::RequestStatuses::<Runtime>::get(transfer.network_id,transfer.tx_hash)),
            "incomingStatus":format!("{:?}",eth_bridge::RequestStatuses::<Runtime>::get(transfer.network_id,incoming.hash())),
            "loadLink":format!("{:#x}",eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(transfer.network_id,transfer.tx_hash)),
            "oldQuery":eth_bridge::AccountRequests::<Runtime>::get(old).iter().map(|(n,h)|json!([n,format!("{h:#x}")])).collect::<Vec<_>>(),
            "newQuery":eth_bridge::AccountRequests::<Runtime>::get(new).iter().map(|(n,h)|json!([n,format!("{h:#x}")])).collect::<Vec<_>>()})
    })
}

struct Branch {
    name: &'static str,
    candidate: bool,
    reopen: bool,
    mapped: bool,
    success: bool,
    pending: bool,
}

fn branch(source: &Source, branch: &Branch, output: &Path) -> Result<Value> {
    fs::create_dir(output).map_err(|e| e.to_string())?;
    let code = if branch.candidate {
        &source.candidate
    } else {
        &source.baseline
    };
    let executor = replay::Executor::builder().build();
    let mut ext = source.restore()?;
    let to = hashi_fixture::owner(if branch.mapped {
        b"owner-old"
    } else {
        b"owner-new"
    });
    let transfer = transfer(&mut ext, to)?;
    let setup = setup(source, &mut ext, code, branch, &transfer)?;
    let overlay_root = replay::root(&mut ext);
    let mut parent = source.header.clone();
    parent.set_state_root(overlay_root);
    parent.digest_mut().push(DigestItem::Other(
        [DOMAIN, b"synthetic-parent-not-canonical"].concat(),
    ));
    let mut journal = snapshot::Journal::new(output)?;
    let (initial, overlay_snapshot_sha) = journal.persist(ext)?;
    ext = initial;
    write_new(&output.join("synthetic-parent.scale"), &parent.encode())?;
    write_json(&output.join("setup.json"), &setup)?;
    let header = Header::new(
        *parent.number() + 1,
        H256::zero(),
        H256::zero(),
        parent.hash(),
        Digest {
            logs: vec![DigestItem::PreRuntime(
                *b"BABE",
                (2u8, source.authority_index, source.slot).encode(),
            )],
        },
    );
    let initialized = replay::call(
        &executor,
        &mut ext,
        code,
        "Core_initialize_block",
        &header.encode(),
    )?;
    require(
        ExtrinsicInclusionMode::decode_all(&mut initialized.as_slice())
            .map_err(|e| e.to_string())?
            == ExtrinsicInclusionMode::AllExtrinsics,
        "ordinary final runtime does not admit economic extrinsics",
    )?;
    let time = timestamp(source.timestamp)?;
    require(
        apply_candidate(&executor, &mut ext, code, &time)? == Ok(Ok(())),
        "ordinary timestamp rejected",
    )?;
    let mut extrinsics = vec![time];
    let before = financial(&mut ext)?;
    let before_view = bridge_view(&mut ext, &transfer);
    let peers = hashi_fixture::pairs();
    let mut negatives = Vec::new();
    let mut approvals = Vec::new();
    let mut weight_witnesses = Vec::new();
    let stages: Vec<_> = if branch.pending {
        vec![Stage::Register, Stage::Finalize]
    } else {
        vec![Stage::Import]
    };
    for stage in &stages {
        let (weight, weight_witness) =
            queried_weight(&executor, &mut ext, code, &inner_call(&transfer, *stage))?;
        weight_witnesses.push(json!({"stage":format!("{stage:?}"),"weight":weight_witness}));
        for (label, case) in [
            ("invalid-signature", SignatureCase::BadSignature),
            ("future-nonce", SignatureCase::WrongNonce),
            ("changed-signed-payload", SignatureCase::ChangedPayload),
        ] {
            // CheckNonce validates future transactions for pool ordering; its
            // prepare rejection runs only after protocol/payment validation.
            // A retained halt intentionally rejects the protocol earlier.
            if !branch.reopen && matches!(case, SignatureCase::WrongNonce) {
                continue;
            }
            let call = outer_call(&mut ext, &transfer, *stage, weight)?;
            let (tx, authorization) = signed(&mut ext, call, &peers[0].1, &parent, case)?;
            let prior = replay::root(&mut ext);
            let result = apply_candidate(&executor, &mut ext, code, &tx)?;
            let expected = if matches!(case, SignatureCase::WrongNonce) {
                InvalidTransaction::Future
            } else {
                InvalidTransaction::BadProof
            };
            require(
                result == Err(TransactionValidityError::Invalid(expected))
                    && replay::root(&mut ext) == prior,
                "invalid ordinary authorization was admitted or mutated state",
            )?;
            negatives.push(json!({"stage":format!("{stage:?}"),"case":label,"authorization":authorization,"result":format!("{result:?}"),"stateRootUnchanged":true}));
        }
        for (index, (account, pair)) in peers.iter().take(3).enumerate() {
            let call = outer_call(&mut ext, &transfer, *stage, weight)?;
            let (tx, authorization) = signed(&mut ext, call, pair, &parent, SignatureCase::Valid)?;
            let nonce_before = ext.execute_with(|| System::account_nonce(account));
            let result = apply_candidate(&executor, &mut ext, code, &tx)?;
            let nonce_after = ext.execute_with(|| System::account_nonce(account));
            if result.is_ok() {
                extrinsics.push(tx);
            }
            if branch.success {
                require(result == Ok(Ok(())), "expected public peer approval failed")?;
            }
            if branch.reopen && index < 2 {
                require(
                    result == Ok(Ok(()))
                        && nonce_after == nonce_before.checked_add(1).ok_or("nonce overflow")?,
                    "reopened branch did not admit the first two genuine peer approvals",
                )?;
            }
            let balances = financial(&mut ext)?;
            if index < 2 || !branch.success || matches!(stage, Stage::Register) {
                require(
                    balances == before,
                    "financial credit occurred without successful finalization quorum",
                )?;
            }
            approvals.push(json!({"stage":format!("{stage:?}"),"authorization":authorization,"result":format!("{result:?}"),"nonceBefore":nonce_before,"nonceAfter":nonce_after,
                "financial":balances.map(|v|v.to_string()),"bridge":bridge_view(&mut ext,&transfer),"nativeEvents":ext.execute_with(||format!("{:?}",System::events()))}));
        }
    }
    let after = financial(&mut ext)?;
    let after_view = bridge_view(&mut ext, &transfer);
    if branch.success {
        require(
            after
                == [
                    before[0],
                    before[1].checked_add(MINT).ok_or("balance overflow")?,
                    before[2].checked_add(MINT).ok_or("issuance overflow")?,
                ],
            "native quorum did not credit only the replacement with exact issuance",
        )?;
        let incoming = eth_bridge::requests::OffchainRequest::<Runtime>::incoming(
            eth_bridge::requests::IncomingRequest::Transfer(transfer.clone()),
        );
        let consumed = ext.execute_with(|| {
            eth_bridge::Requests::<Runtime>::get(transfer.network_id, incoming.hash())
                .map(|r| r.encode())
                == Some(incoming.encode())
                && eth_bridge::RequestStatuses::<Runtime>::get(
                    transfer.network_id,
                    transfer.tx_hash,
                ) == Some(eth_bridge::requests::RequestStatus::Done)
                && eth_bridge::RequestStatuses::<Runtime>::get(transfer.network_id, incoming.hash())
                    == Some(eth_bridge::requests::RequestStatus::Done)
                && eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(
                    transfer.network_id,
                    transfer.tx_hash,
                ) == incoming.hash()
                && !eth_bridge::AccountRequests::<Runtime>::get(hashi_fixture::owner(b"owner-old"))
                    .iter()
                    .any(|(_, h)| *h == transfer.tx_hash || *h == incoming.hash())
                && [transfer.tx_hash, incoming.hash()].iter().all(|h| {
                    eth_bridge::AccountRequests::<Runtime>::get(hashi_fixture::owner(b"owner-new"))
                        .iter()
                        .filter(|(n, id)| *n == transfer.network_id && id == h)
                        .count()
                        == 1
                })
                && eth_bridge::Requests::<Runtime>::get(transfer.network_id, transfer.tx_hash)
                    .map(|r| r.encode())
                    == Some(original_load(&transfer).encode())
                && eth_bridge::RequestSubmissionHeight::<Runtime>::get(
                    transfer.network_id,
                    transfer.tx_hash,
                ) == if branch.pending {
                    *source.header.number()
                } else {
                    *source.header.number() + 1
                }
                && !eth_bridge::RequestsQueue::<Runtime>::get(transfer.network_id)
                    .iter()
                    .any(|h| *h == transfer.tx_hash || *h == incoming.hash())
        });
        require(
            consumed,
            "native payload/replay consumption/replacement query index differs",
        )?;
    } else {
        require(after == before, "negative branch credited funds")?;
        if branch.reopen {
            let incoming = eth_bridge::requests::OffchainRequest::<Runtime>::incoming(
                eth_bridge::requests::IncomingRequest::Transfer(transfer.clone()),
            );
            let terminal = ext.execute_with(|| {
                eth_bridge::RequestStatuses::<Runtime>::get(transfer.network_id, incoming.hash())
            });
            require(
                matches!(
                    terminal,
                    Some(eth_bridge::requests::RequestStatus::Failed(_))
                        | Some(eth_bridge::requests::RequestStatus::Broken(_, _))
                ),
                "baseline mapping failure lacks a native failed/broken incoming outcome",
            )?;
        }
    }
    let mut replay_results = Vec::new();
    if branch.success {
        let stage = *stages.last().ok_or("no diagnostic stage")?;
        let (weight, _) = queried_weight(&executor, &mut ext, code, &inner_call(&transfer, stage))?;
        for (account, pair) in peers.iter().take(3) {
            let call = outer_call(&mut ext, &transfer, stage, weight)?;
            let (tx, authorization) = signed(&mut ext, call, pair, &parent, SignatureCase::Valid)?;
            let nonce_before = ext.execute_with(|| System::account_nonce(account));
            let result = apply_candidate(&executor, &mut ext, code, &tx)?;
            require(result==Err(TransactionValidityError::Invalid(InvalidTransaction::Call)),
                "authenticated replay must fail native protocol validation, not payment or stale nonce")?;
            if result.is_ok() {
                extrinsics.push(tx);
            }
            require(
                financial(&mut ext)? == after && bridge_view(&mut ext, &transfer) == after_view,
                "authenticated redelivery changed financial/replay state",
            )?;
            replay_results.push(json!({"authorization":authorization,"result":format!("{result:?}"),"nonceBefore":nonce_before,
                "nonceAfter":ext.execute_with(||System::account_nonce(account)),"noSecondMint":true,"nativeValidationReason":"InvalidTransaction::Call; existing consumed native identity is bound above, no fee-payment error accepted"}));
        }
    }
    let encoded = replay::call(
        &executor,
        &mut ext,
        code,
        "BlockBuilder_finalize_block",
        &[],
    )?;
    let finalized = Header::decode_all(&mut encoded.as_slice()).map_err(|e| e.to_string())?;
    require(
        *finalized.number() == *parent.number() + 1
            && *finalized.parent_hash() == parent.hash()
            && *finalized.state_root() == replay::root(&mut ext)
            && *finalized.extrinsics_root()
                == sp_io::trie::blake2_256_ordered_root(
                    extrinsics.iter().map(Encode::encode).collect(),
                    StateVersion::V0,
                ),
        "diagnostic authored header/extrinsics root differs",
    )?;
    assert_complete(&mut ext, &source.policy, !branch.reopen)?;
    let block = RuntimeBlock {
        header: finalized.clone(),
        extrinsics,
    };
    let block_bytes = block.encode();
    write_new(&output.join("block.scale"), &block_bytes)?;
    let mut imported = journal.restore()?;
    let import_executor = replay::Executor::builder().build();
    require(
        replay::call(
            &import_executor,
            &mut imported,
            code,
            "Core_execute_block",
            &block_bytes,
        )?
        .is_empty()
            && replay::root(&mut imported) == *finalized.state_root(),
        "independent execution from exact setup overlay differs",
    )?;
    drop(imported);
    let (persisted, terminal_snapshot_sha) = journal.persist(ext)?;
    drop(persisted);
    drop(journal);
    let mut reopened = snapshot::restore_pinned(
        output,
        "snapshots/000001.json",
        &terminal_snapshot_sha,
        *finalized.state_root(),
    )?;
    require(
        financial(&mut reopened)? == after && bridge_view(&mut reopened, &transfer) == after_view,
        "fresh disk restore differs from financial/replay result",
    )?;
    assert_complete(&mut reopened, &source.policy, !branch.reopen)?;
    let result = json!({"name":branch.name,"candidateRuntime":branch.candidate,"runtimeSha256":sha(code),
        "sourceStateRoot":format!("{:#x}",source.header.state_root()),"overlayRoot":format!("{overlay_root:#x}"),
        "syntheticParentHeader":hx(&parent.encode()),"syntheticParentHash":format!("{:#x}",parent.hash()),
        "canonicalHistoryReplay":false,"setup":setup,"overlaySnapshotSha256":overlay_snapshot_sha,
        "originalRecipient":hx(transfer.to.as_ref()),"originalAuthor":hx(transfer.author.as_ref()),
        "effectiveOwner":hx(hashi_fixture::owner(b"owner-new").as_ref()),"originalNativeCalls":stages.iter().map(|stage|json!({"stage":format!("{stage:?}"),"scale":hx(&inner_call(&transfer,*stage).encode()),"sha256":sha(&inner_call(&transfer,*stage).encode())})).collect::<Vec<_>>(),
        "weightQueries":weight_witnesses,"syntheticRetainedPendingLoad":branch.pending,
        "negativeAuthorizations":negatives,"approvals":approvals,"authenticatedReplayAttempts":replay_results,
        "financialBefore":before.map(|v|v.to_string()),"financialAfter":after.map(|v|v.to_string()),
        "bridgeBefore":before_view,"bridgeAfter":after_view,"expectedNativeCreditObserved":branch.success,
        "blockSha256":sha(&block_bytes),"blockHash":format!("{:#x}",finalized.hash()),"stateRoot":format!("{:#x}",finalized.state_root()),
        "terminalSnapshotSha256":terminal_snapshot_sha,"independentOverlayImportRootMatches":true,"freshDiskRootMatches":true,
        "productionReady":false});
    write_json(&output.join("result.json"), &result)?;
    Ok(result)
}

pub fn run(manifest: &Path, output: &Path) -> Result<Value> {
    let source = load(manifest)?;
    fs::create_dir(output).map_err(|e| format!("diagnostic output must be new: {e}"))?;
    write_json(&output.join("bound-inputs.json"), &source.evidence)?;
    let mut results = Vec::new();
    for case in [
        Branch {
            name: "baseline-halted",
            candidate: false,
            reopen: false,
            mapped: true,
            success: false,
            pending: false,
        },
        Branch {
            name: "baseline-reopened-original-recipient",
            candidate: false,
            reopen: true,
            mapped: true,
            success: false,
            pending: false,
        },
        Branch {
            name: "baseline-reopened-live-recipient-control",
            candidate: false,
            reopen: true,
            mapped: false,
            success: true,
            pending: false,
        },
        Branch {
            name: "candidate-halted",
            candidate: true,
            reopen: false,
            mapped: true,
            success: false,
            pending: false,
        },
        Branch {
            name: "candidate-reopened-original-recipient",
            candidate: true,
            reopen: true,
            mapped: true,
            success: true,
            pending: false,
        },
        Branch {
            name: "baseline-pending-reopened-original-recipient",
            candidate: false,
            reopen: true,
            mapped: true,
            success: false,
            pending: true,
        },
        Branch {
            name: "candidate-pending-halted",
            candidate: true,
            reopen: false,
            mapped: true,
            success: false,
            pending: true,
        },
        Branch {
            name: "candidate-pending-reopened-original-recipient",
            candidate: true,
            reopen: true,
            mapped: true,
            success: true,
            pending: true,
        },
    ] {
        eprintln!("PUBLIC FIXTURE post-Complete HASHI branch {}", case.name);
        results.push(branch(&source, &case, &output.join(case.name))?);
    }
    let result = json!({"schema":"sora-nexus-PUBLIC-FIXTURE-post-complete-hashi-report-v1","inputs":source.evidence,
        "branches":results,"fixtureOnly":true,"nativeEconomicExecution":false,"productionBridgeHaltChanged":false,
        "canonicalRecoveryHistoryReplayed":false,"consensusFinalityVerified":false,"operatorApprovalVerified":false,"productionReady":false,
        "exactOldOwnerSignedSpendTested":false,"exactOldOwnerSignedSpendLimitation":"HASHI38 owner addresses are synthetic hashes without known signing seeds; no valid retired-owner signature can be fabricated. The separate key-backed native regression covers signed retired-key rejection.",
        "retainedPendingLoadAdmissionTested":true,"pendingLoadScope":"Synthetic delayed load regression; actual checkpoint has zero Pending financial HASHI loads. Registration and finalization use their ordinary native calls, never recovery-only retained import.","channelDeliveryTested":false,
        "scope":"Fresh and explicitly seeded pending original-payload HASHI delivery after Complete, retaining the native Deposit from=zero shape, from an explicitly recorded disposable setup overlay; no source-actor attribution, external event/finality proof, or production reopening authorization."});
    write_json(&output.join("report.json"), &result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_candidate_transaction_commits_admitted_dispatch_outcomes() {
        for dispatch in [
            Ok(()),
            Err(sp_runtime::DispatchError::Other("dispatch failed")),
        ] {
            let mut ext = sp_io::TestExternalities::default();
            let expected = dispatch;
            let result = with_candidate_transaction(&mut ext, |ext| {
                ext.execute_with(|| sp_io::storage::set(b"included-nonce", &[1]));
                Ok(Ok(dispatch))
            });
            assert_eq!(result.unwrap().unwrap(), expected);
            assert_eq!(value(&mut ext, b"included-nonce"), Some(vec![1]));
            assert_eq!(ext.overlayed_changes().transaction_depth(), 0);
            ext.commit_all().unwrap();
        }
    }

    #[test]
    fn client_candidate_transaction_rolls_back_validity_bookkeeping() {
        let mut ext = sp_io::TestExternalities::default();
        let initial = replay::root(&mut ext);
        ext.execute_with(|| {
            sp_io::storage::start_transaction();
            sp_io::storage::set(b"caller-layer", &[7]);
        });
        let before = replay::root(&mut ext);
        let result = with_candidate_transaction(&mut ext, |ext| {
            // Executive can write these before an extension rejects a candidate.
            ext.execute_with(|| {
                sp_io::storage::set(b"inherents-applied", &[1]);
                sp_io::storage::set(b"extrinsic-data", &[9]);
                sp_io::storage::set(b"caller-layer", &[8]);
            });
            Ok(Err(InvalidTransaction::Call.into()))
        });
        assert_eq!(result.unwrap(), Err(InvalidTransaction::Call.into()));
        assert_eq!(replay::root(&mut ext), before);
        assert_eq!(value(&mut ext, b"caller-layer"), Some(vec![7]));
        assert_eq!(ext.overlayed_changes().transaction_depth(), 1);
        ext.execute_with(sp_io::storage::rollback_transaction);
        assert_eq!(replay::root(&mut ext), initial);
        ext.commit_all().unwrap();
    }

    #[test]
    fn client_candidate_transaction_rolls_back_host_failure_and_nested_writes() {
        let mut ext = sp_io::TestExternalities::default();
        let before = replay::root(&mut ext);
        let result = with_candidate_transaction(&mut ext, |ext| {
            ext.execute_with(|| {
                sp_io::storage::set(b"extrinsic-data", &[9]);
                sp_io::storage::start_transaction();
                sp_io::storage::set(b"interrupted-runtime-write", &[1]);
            });
            Err("test-only host failure".into())
        });
        assert_eq!(result.unwrap_err(), "test-only host failure");
        assert_eq!(replay::root(&mut ext), before);
        assert_eq!(ext.overlayed_changes().transaction_depth(), 0);
        ext.commit_all().unwrap();
    }

    #[test]
    fn client_candidate_transaction_rejects_unbalanced_success() {
        let mut ext = sp_io::TestExternalities::default();
        let before = replay::root(&mut ext);
        let result = with_candidate_transaction(&mut ext, |ext| {
            ext.execute_with(|| {
                sp_io::storage::start_transaction();
                sp_io::storage::set(b"uncommitted-runtime-write", &[1]);
            });
            Ok(Ok(Ok(())))
        });
        assert!(result
            .unwrap_err()
            .contains("unbalanced storage transaction"));
        assert_eq!(replay::root(&mut ext), before);
        assert_eq!(ext.overlayed_changes().transaction_depth(), 0);
        ext.commit_all().unwrap();
    }
}
