//! PUBLIC FIXTURE ONLY: ordinary Substrate-family admission under exact final-138 Wasm.
//! The disposable setup overlay and synthetic parent are NOT canonical recovery
//! history. No production bridge halt, source snapshot or release is modified.

use super::post_complete::apply_candidate as apply;
use super::*;
use bridge_types::{
    substrate::{BridgeCall, BridgeMessage, SubstrateAppCall},
    traits::{BalancePrecisionConverter, BridgeAssetRegistry},
    types::{
        AssetKind, AuxiliaryDigest, AuxiliaryDigestItem, MessageDirection, MessageId, MessageStatus,
    },
    GenericAccount, GenericAssetId, GenericBalance, GenericCommitment, GenericNetworkId,
    GenericTimepoint, SubNetworkId,
};
use common::AssetInfoProvider;
use framenode_runtime::{
    AccountId, AssetId, BridgeMaxMessagePayloadSize, BridgeMaxMessagesPerCommit, BridgeProxy,
    Runtime, RuntimeCall, RuntimeEvent, System, ThisNetworkId,
};
use serde_json::{json, Value};
use sp_core::{ecdsa, ed25519, Pair};
use sp_runtime::{
    generic::Era,
    transaction_validity::{InvalidTransaction, TransactionValidityError},
    MultiSignature,
};
use sp_runtime::{
    traits::{Hash as _, Keccak256},
    BoundedBTreeSet,
};
use std::{io::Write, path::PathBuf};

const SCHEMA: &str = "sora-nexus-PUBLIC-FIXTURE-post-complete-substrate-v1";
const SOURCE_MANIFEST_SHA: &str =
    "d695c781f27137a01b981c558be1739a05006eb7946ae8ea930de4500b0e29c7";
const SOURCE_EXECUTION_SHA: &str =
    "d29e8832babfc3830b24331444071b94265e9a619a9273279a72dd081a3e6347";
const SOURCE_REPORT_SHA: &str = "77823a126a3909dcfad7855bc1374aa7620db8e845353ceb3bfd5cd0742652b4";
const SOURCE_CODE_SHA: &str = "ffd4728d7122edfc72e309ef862c730a324132ddc290b03ad521bf097c02397a";
const DOMAIN: &[u8] = b"sora-nexus/PUBLIC-FIXTURE-ONLY/post-complete-substrate/v1/";
const MINT: u128 = 1;
const NETWORK: SubNetworkId = SubNetworkId::Rococo;
const MIXED_DOMAIN: &[u8] = b"sora-nexus/PUBLIC-FIXTURE-ONLY/mixed-bridge/v1/";
const POLICY_COMMITMENT: &str = "cccbdc43eba9d8c794d99451476deb49a307280d1bf7785b2783b5444afe80d6";
const MAX_INPUT: usize = 64 * 1024 * 1024;

type Commitment = GenericCommitment<BridgeMaxMessagesPerCommit, BridgeMaxMessagePayloadSize>;

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
            && manifest.source_report.sha256 == SOURCE_REPORT_SHA
            && manifest.source_execution.sha256 == SOURCE_EXECUTION_SHA,
        "diagnostic accepts only the reviewed mixed39 source cohort",
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
        blocks.len() == 39
            && source.steps.len() == 39
            && report["manifestSha256"] == manifest.source_manifest.sha256
            && report["publicTestHashiFixture"] == false
            && report["publicTestCouncilFixture"] == false
            && report["automaticActivationObserved"] == true
            && report["ordinaryRuntimeBlocks"] == 2
            && report["bridgeHaltRetained"] == true
            && report["nativeExecution"] == false
            && report["productionReady"] == false
            && report["operatorApprovalVerified"] == false
            && report["consensusSealsVerified"] == false
            && source.schema == "sora-nexus-maintenance-replay-v1",
        "source report is not the completed exact-Wasm mixed39 fixture",
    )?;
    let last = blocks.last().ok_or("source report is empty")?;
    require(
        last["number"] == 28_191_856
            && last["phase"] == "Complete"
            && last["resultingSpec"] == 138
            && last["independentImportRootMatches"] == true
            && last["diskRestartRootMatches"] == true
            && last["blockSha256"] == manifest.source_last_block.sha256
            && last["diskSnapshotManifestSha256"] == manifest.snapshot_sha256
            && manifest.snapshot_manifest == "snapshots/000039.json",
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
    require(
        hex::encode(policy.commitment()) == POLICY_COMMITMENT
            && policy.replacements.iter().any(|pair| {
                pair.old == owner(b"old-owner") && pair.new == owner(b"replacement-owner")
            }),
        "mixed39 replacement policy differs",
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
        "candidate must use exact mixed39 policy and changed runtime bytes",
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

fn owner(label: &[u8]) -> AccountId {
    AccountId::new(sp_io::hashing::blake2_256(&[MIXED_DOMAIN, label].concat()))
}
fn peer(n: u8) -> ecdsa::Pair {
    ecdsa::Pair::from_seed(&sp_io::hashing::blake2_256(
        &[MIXED_DOMAIN, b"peer/", &[n]].concat(),
    ))
}
fn relayer() -> ed25519::Pair {
    ed25519::Pair::from_seed(&sp_io::hashing::blake2_256(
        &[DOMAIN, b"public-relayer"].concat(),
    ))
}
fn key(pallet: &[u8], item: &[u8]) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(pallet),
        sp_io::hashing::twox_128(item),
    ]
    .concat()
}
fn app_keys() -> Vec<(Vec<u8>, Vec<u8>)> {
    [
        (
            b"AssetKinds".as_slice(),
            common::XOR.encode(),
            AssetKind::Sidechain.encode(),
        ),
        (b"SidechainPrecision", common::XOR.encode(), 18u8.encode()),
        (
            b"SidechainAssetId",
            common::XOR.encode(),
            GenericAssetId::Sora(common::XOR.into()).encode(),
        ),
        (
            b"ThischainAssetId",
            GenericAssetId::Sora(common::XOR.into()).encode(),
            common::XOR.encode(),
        ),
    ]
    .into_iter()
    .map(|(item, suffix, value)| {
        (
            [key(b"SubstrateBridgeApp", item), NETWORK.encode(), suffix].concat(),
            value,
        )
    })
    .collect()
}
fn amount() -> Result<GenericBalance> {
    type Converter = <Runtime as substrate_bridge_app::Config>::BalancePrecisionConverter;
    let (_, external) = Converter::to_sidechain(&common::XOR, 18, MINT)
        .ok_or("public XOR mint does not fit native external amount")?;
    let (local, _) = Converter::from_sidechain(&common::XOR, 18, external.clone())
        .ok_or("public XOR mint does not roundtrip")?;
    require(
        local == MINT && local > 0,
        "public XOR amount lost precision",
    )?;
    Ok(external)
}
fn financial(ext: &mut sp_io::TestExternalities) -> Result<[u128; 4]> {
    ext.execute_with(|| {
        Ok([
            framenode_runtime::Assets::free_balance(&common::XOR, &owner(b"old-owner"))
                .map_err(|e| format!("old balance: {e:?}"))?,
            framenode_runtime::Assets::free_balance(&common::XOR, &owner(b"replacement-owner"))
                .map_err(|e| format!("new balance: {e:?}"))?,
            framenode_runtime::Assets::total_issuance(&common::XOR)
                .map_err(|e| format!("issuance: {e:?}"))?,
            BridgeProxy::locked_assets(GenericNetworkId::Sub(NETWORK), common::XOR),
        ])
    })
}

/// Preserve the real Liberland queue/counters and every associated permanent
/// message/batch marker; the diagnostic never clears a recovery quarantine.
fn protected_liberland(ext: &mut sp_io::TestExternalities) -> Value {
    ext.execute_with(|| {
        let network = SubNetworkId::Liberland;
        let nonce = substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(network);
        let queue = framenode_runtime::SubstrateBridgeOutboundChannel::recovery_queue(network);
        let batch = nonce
            .checked_add(1)
            .expect("pinned fixture nonce is bounded");
        let mut keys = vec![
            substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::hashed_key_for(network),
            substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::hashed_key_for(network),
            substrate_bridge_channel::outbound::LatestCommitment::<Runtime>::hashed_key_for(
                network,
            ),
            [
                key(b"SubstrateBridgeOutboundChannel", b"MessageQueues"),
                network.encode(),
            ]
            .concat(),
            bridge_types::recovery::batch_key(1, network.into(), batch),
        ];
        for index in 0..queue.len() {
            let id = MessageId::batched(ThisNetworkId::get(), network.into(), batch, index as u64)
                .hash();
            keys.push(bridge_types::recovery::message_key(network.into(), id));
            keys.push(bridge_types::recovery::historical_message_key(
                network.into(),
                id,
            ));
        }
        json!(keys
            .into_iter()
            .map(|k| json!({"key":hx(&k),
            "value":sp_io::storage::get(&k).map(|v|hx(&v))}))
            .collect::<Vec<_>>())
    })
}

fn setup(
    source: &Source,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
    reopen: bool,
) -> Result<Value> {
    let executor = replay::Executor::builder().build();
    require(
        replay::code(ext)? == source.baseline,
        "source snapshot runtime differs",
    )?;
    assert_complete(ext, &source.policy, true)?;
    let version = replay::version(&executor, ext, &source.baseline)?;
    require(
        (version.spec_version, version.transaction_version) == (138, 134),
        "source version differs",
    )?;
    ext.commit_all().map_err(|e| e.to_string())?;
    let before_financial = financial(ext)?;
    let before_liberland = protected_liberland(ext);
    let before_root = replay::root(ext);
    let signer = AccountId::from(relayer().public().0);
    let peers_key =
        multisig_verifier::PeerKeys::<Runtime>::hashed_key_for(GenericNetworkId::Sub(NETWORK));
    let relayer_key = frame_system::Account::<Runtime>::hashed_key_for(&signer);
    let configs = app_keys();
    require(
        value(ext, &relayer_key).is_none()
            && value(ext, &peers_key).is_none()
            && configs.iter().all(|(k, _)| value(ext, k).is_none()),
        "public fixture network or relayer is occupied",
    )?;
    require(
        ext.execute_with(|| {
            !substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::contains_key(NETWORK)
                && !substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::contains_key(
                    NETWORK,
                )
                && !substrate_bridge_channel::outbound::LatestCommitment::<Runtime>::contains_key(
                    NETWORK,
                )
                && framenode_runtime::SubstrateBridgeOutboundChannel::recovery_queue(NETWORK)
                    .is_empty()
        }) && before_financial[3] == 0,
        "Rococo diagnostic network has existing protocol state",
    )?;
    let interval = ext.execute_with(framenode_runtime::SubstrateBridgeOutboundChannel::interval);
    require(
        interval > 0 && !(*source.header.number() + 1).is_multiple_of(interval),
        "diagnostic child must not be an outbound commitment boundary",
    )?;
    let manager = BridgeProxy::bridge_account(GenericNetworkId::Sub(NETWORK))
        .map_err(|e| format!("manager: {e:?}"))?;
    let fee = framenode_runtime::Technical::tech_account_id_to_account_id(
        &BridgeProxy::bridge_fee_tech_account(GenericNetworkId::Sub(NETWORK)),
    )
    .map_err(|e| format!("fee account: {e:?}"))?;
    let custody_financial_before = ext.execute_with(|| {
        [&manager, &fee].map(|account| frame_system::Account::<Runtime>::get(account).data.encode())
    });
    // Exact deployed permissions::Scope::Limited(H512) encoding. The native
    // manage_asset call below must write only these enumerated double-map keys.
    let scope = (0u8, common::hash(&common::XOR)).encode();
    let permission_owner = |permission: common::permissions::PermissionId| {
        [
            key(b"Permissions", b"Owners"),
            sp_io::hashing::blake2_256(&permission.encode()).to_vec(),
            sp_io::hashing::blake2_256(&scope).to_vec(),
        ]
        .concat()
    };
    let permission_holder = [
        key(b"Permissions", b"Permissions"),
        sp_io::hashing::blake2_256(&manager.encode()).to_vec(),
        sp_io::hashing::blake2_256(&scope).to_vec(),
    ]
    .concat();
    // Exact native manage_asset write set, plus explicit public fixture config.
    // No broad pallet-prefix exemption and no incoming/outgoing counter edits.
    let mut allowed: BTreeSet<Vec<u8>> = configs.iter().map(|(k, _)| k.clone()).collect();
    allowed.extend([
        peers_key.clone(),
        relayer_key.clone(),
        permission_owner(common::permissions::MINT),
        permission_owner(common::permissions::BURN),
        permission_holder,
        key(b"System", b"Events"),
        key(b"System", b"EventCount"),
    ]);
    for account in [&manager, &fee] {
        allowed.insert(frame_system::Account::<Runtime>::hashed_key_for(account));
        allowed.insert(
            [
                key(b"Technical", b"TechAccounts"),
                sp_io::hashing::blake2_128(&account.encode()).to_vec(),
                account.encode(),
            ]
            .concat(),
        );
    }
    if code != source.baseline {
        allowed.insert(b":code".to_vec());
    }
    if reopen {
        allowed.insert(recovery::BRIDGE_HALT_STORAGE_KEY.to_vec());
    }
    let before: BTreeMap<_, _> = allowed.iter().map(|k| (k.clone(), value(ext, k))).collect();
    ext.execute_with(|| -> Result<()> {
        <BridgeProxy as BridgeAssetRegistry<AccountId, AssetId>>::manage_asset(
            GenericNetworkId::Sub(NETWORK),
            common::XOR,
        )
        .map_err(|e| format!("native diagnostic asset management: {e:?}"))?;
        let peers = (0u8..4).map(|n| peer(n).public()).collect::<BTreeSet<_>>();
        multisig_verifier::PeerKeys::<Runtime>::insert(
            GenericNetworkId::Sub(NETWORK),
            BoundedBTreeSet::<_, <Runtime as multisig_verifier::Config>::MaxPeers>::try_from(peers)
                .map_err(|_| "public peer bound")?,
        );
        for (k, v) in &configs {
            sp_io::storage::set(k, v);
        }
        sp_io::storage::set(
            &relayer_key,
            &framenode_runtime::AccountData {
                providers: 1,
                ..Default::default()
            }
            .encode(),
        );
        if code != source.baseline {
            sp_io::storage::set(b":code", code);
        }
        if reopen {
            sp_io::storage::clear(recovery::BRIDGE_HALT_STORAGE_KEY);
        }
        Ok(())
    })?;
    let changed: BTreeSet<_> = ext
        .overlayed_changes()
        .changes()
        .map(|(k, _)| k.clone())
        .collect();
    require(
        changed.is_subset(&allowed),
        "unlisted diagnostic setup write",
    )?;
    require(
        financial(ext)? == before_financial && protected_liberland(ext) == before_liberland,
        "setup changed financial state or protected Liberland protocol records",
    )?;
    require(
        ext.execute_with(|| {
            [&manager, &fee]
                .map(|account| frame_system::Account::<Runtime>::get(account).data.encode())
                == custody_financial_before
                && frame_system::Account::<Runtime>::get(&signer).data
                    == framenode_runtime::AccountData::default().data
        }),
        "setup changed custody balances or funded the public relayer",
    )?;
    let rows = changed.into_iter().map(|k| {
        let prior=before.get(&k).expect("enumerated key");let after=value(ext,&k);
        json!({"key":hx(&k),"before":if k==b":code" {None} else {prior.as_ref().map(|v|hx(v))},
            "after":if k==b":code" {None} else {after.as_ref().map(|v|hx(v))},
            "beforeSha256":prior.as_ref().map(|v|sha(v)),"afterSha256":after.as_ref().map(|v|sha(v)),
            "beforeBytes":prior.as_ref().map(Vec::len),"afterBytes":after.as_ref().map(Vec::len)})
    }).collect::<Vec<_>>();
    assert_complete(ext, &source.policy, !reopen)?;
    let version = replay::version(&executor, ext, code)?;
    require(
        (version.spec_version, version.transaction_version) == (138, 134),
        "candidate version differs",
    )?;
    Ok(
        json!({"sourceStateRoot":format!("{before_root:#x}"),"overlayStateRoot":format!("{:#x}",replay::root(ext)),
        "writes":rows,"allowedExactKeys":allowed.iter().map(|k|hx(k)).collect::<Vec<_>>(),
        "fixtureNetwork":"Rococo","nativeOutboundInterval":interval,"outboundCommitmentBoundary":false,
        "realLiberlandStateUnchanged":true,"protectedLiberland":before_liberland,
        "publicRelayer":hx(signer.as_ref()),"relayerBalanceFunded":false,
        "custodyAccountFinancialDataUnchanged":true,
        "nativeEconomicDispatch":false,"nativeConfigurationOnly":true,
        "bridgeReopenedOnlyInDisposableOverlay":reopen,"counterOrQuarantineWrites":false}),
    )
}

fn make_commitment(
    recipient: AccountId,
    nonce: u64,
    external_amount: GenericBalance,
) -> Result<Commitment> {
    let payload = BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
        sender: GenericAccount::Liberland(owner(b"external-sender")),
        recipient: GenericAccount::Sora(recipient),
        asset_id: GenericAssetId::Sora(common::XOR.into()),
        amount: external_amount,
    })
    .encode()
    .try_into()
    .map_err(|_| "bounded public payload")?;
    Ok(GenericCommitment::Sub(
        bridge_types::substrate::Commitment {
            nonce,
            messages: vec![BridgeMessage {
                payload,
                timepoint: GenericTimepoint::Sora(recovery::CHECKPOINT_NUMBER),
            }]
            .try_into()
            .map_err(|_| "bounded public message")?,
        },
    ))
}
fn proof(commitment: &Commitment) -> framenode_runtime::MultiProof {
    let digest = AuxiliaryDigest {
        logs: vec![AuxiliaryDigestItem::Commitment(
            ThisNetworkId::get(),
            commitment.hash(),
        )],
    };
    framenode_runtime::MultiProof::Multisig(multisig_verifier::Proof {
        digest: digest.clone(),
        proof: (0u8..4)
            .map(|n| peer(n).sign_prehashed(Keccak256::hash_of(&digest).as_fixed_bytes()))
            .collect(),
    })
}
fn native_call(commitment: Commitment, proof: framenode_runtime::MultiProof) -> RuntimeCall {
    RuntimeCall::SubstrateBridgeInboundChannel(substrate_bridge_channel::inbound::Call::submit {
        network_id: NETWORK,
        commitment,
        proof,
    })
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

fn bridge_view(ext: &mut sp_io::TestExternalities, id: H256) -> Value {
    ext.execute_with(||json!({
        "nativeInboundNonce":substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(NETWORK),
        "nativeOutboundNonce":substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(NETWORK),
        "oldQuery":BridgeProxy::transactions((GenericNetworkId::Sub(NETWORK),owner(b"old-owner")),id).map(|v|hx(&v.encode())),
        "effectiveQuery":BridgeProxy::transactions((GenericNetworkId::Sub(NETWORK),owner(b"replacement-owner")),id).map(|v|hx(&v.encode())),
        "senderQuery":BridgeProxy::sender(GenericNetworkId::Sub(NETWORK),id).map(|v|hx(v.as_ref())),
        "acknowledgementQueue":framenode_runtime::SubstrateBridgeOutboundChannel::recovery_queue(NETWORK)
            .iter().map(|v|hx(&v.encode())).collect::<Vec<_>>(),
        "oldSystemAccountExists":frame_system::Account::<Runtime>::contains_key(owner(b"old-owner")),
    }))
}
fn assert_acknowledgements_uncommitted(ext: &mut sp_io::TestExternalities) -> Result<()> {
    require(
        ext.execute_with(|| {
            substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(NETWORK) == 0
                && !substrate_bridge_channel::outbound::LatestCommitment::<Runtime>::contains_key(
                    NETWORK,
                )
        }),
        "disposable acknowledgement was committed or advanced the outbound counter",
    )
}
struct Branch {
    name: &'static str,
    candidate: bool,
    reopen: bool,
    mapped: bool,
    success: bool,
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
    let protected = protected_liberland(&mut ext);
    let setup = setup(source, &mut ext, code, branch.reopen)?;
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
        "ordinary final runtime rejected economic admission",
    )?;
    let time = timestamp(source.timestamp)?;
    require(
        apply(&executor, &mut ext, code, &time)? == Ok(Ok(())),
        "ordinary timestamp rejected",
    )?;
    let mut extrinsics = vec![time];
    let before = financial(&mut ext)?;
    let nonce = ext
        .execute_with(|| substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(NETWORK))
        .checked_add(1)
        .ok_or("inbound nonce overflow")?;
    require(nonce == 1, "unused diagnostic network nonce differs")?;
    let recipient = owner(if branch.mapped {
        b"old-owner"
    } else {
        b"replacement-owner"
    });
    let external_amount = ext.execute_with(amount)?;
    let commitment = make_commitment(recipient.clone(), nonce, external_amount.clone())?;
    let original_commitment = commitment.encode();
    let original_proof = proof(&commitment);
    let id = MessageId::batched(
        GenericNetworkId::Sub(NETWORK),
        ThisNetworkId::get(),
        nonce,
        0,
    )
    .hash();
    let before_view = bridge_view(&mut ext, id);
    let original_call = native_call(commitment.clone(), original_proof.clone());
    let public_relayer = relayer();
    let signer = AccountId::from(public_relayer.public().0);
    let mut negatives = Vec::new();
    for (label, case) in [
        ("invalid-outer-signature", SignatureCase::BadSignature),
        ("future-outer-nonce", SignatureCase::WrongNonce),
        ("changed-outer-signed-call", SignatureCase::ChangedPayload),
    ] {
        if !branch.reopen && matches!(case, SignatureCase::WrongNonce) {
            continue;
        }
        let (tx, witness) = signed(
            &mut ext,
            original_call.clone(),
            &public_relayer,
            &parent,
            case,
        )?;
        let prior = replay::root(&mut ext);
        let result = apply(&executor, &mut ext, code, &tx)?;
        let expected = if matches!(case, SignatureCase::WrongNonce) {
            InvalidTransaction::Future
        } else {
            InvalidTransaction::BadProof
        };
        require(
            result == Err(TransactionValidityError::Invalid(expected))
                && replay::root(&mut ext) == prior,
            "invalid outer authorization mutated/admitted state",
        )?;
        negatives.push(json!({"case":label,"outerAuthorization":witness,"result":format!("{result:?}"),"stateRootUnchanged":true}));
    }
    if branch.reopen {
        let mut bad_proof = original_proof.clone();
        match &mut bad_proof {
            framenode_runtime::MultiProof::Multisig(p) => p.proof[0].0[64] = 255,
            _ => return Err("unexpected public proof variant".into()),
        }
        let changed = make_commitment(owner(b"external-sender"), nonce, external_amount.clone())?;
        let future = make_commitment(
            recipient.clone(),
            nonce.checked_add(1).ok_or("nonce overflow")?,
            external_amount.clone(),
        )?;
        let cases = [
            (
                "invalid-inner-proof",
                native_call(commitment.clone(), bad_proof),
            ),
            (
                "changed-recipient-original-proof",
                native_call(changed, original_proof.clone()),
            ),
            (
                "future-native-channel-nonce",
                native_call(future.clone(), proof(&future)),
            ),
        ];
        for (label, call) in cases {
            let (tx, witness) = signed(
                &mut ext,
                call,
                &public_relayer,
                &parent,
                SignatureCase::Valid,
            )?;
            let prior = replay::root(&mut ext);
            let result = apply(&executor, &mut ext, code, &tx)?;
            require(
                result == Err(TransactionValidityError::Invalid(InvalidTransaction::Call))
                    && replay::root(&mut ext) == prior,
                "invalid native proof/nonce did not fail protocol validation atomically",
            )?;
            negatives.push(json!({"case":label,"outerAuthorization":witness,"result":format!("{result:?}"),
                "stateRootUnchanged":true,"nativeValidationReason":"InvalidTransaction::Call; no fee error accepted"}));
        }
    }
    let (tx, authorization) = signed(
        &mut ext,
        original_call.clone(),
        &public_relayer,
        &parent,
        SignatureCase::Valid,
    )?;
    let before_nonce = ext.execute_with(|| System::account_nonce(&signer));
    let prior = replay::root(&mut ext);
    let result = apply(&executor, &mut ext, code, &tx)?;
    if branch.reopen {
        require(
            result == Ok(Ok(()))
                && ext.execute_with(|| System::account_nonce(&signer)) == before_nonce + 1,
            "authenticated ordinary submit did not pass native fee-exempt admission",
        )?;
        extrinsics.push(tx);
        require(
            ext.execute_with(|| {
                substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(NETWORK)
            }) == nonce,
            "authenticated native delivery did not consume exactly one channel nonce",
        )?;
    } else {
        require(
            result == Err(TransactionValidityError::Invalid(InvalidTransaction::Call))
                && replay::root(&mut ext) == prior,
            "retained bridge halt did not reject atomically through protocol validation",
        )?;
    }
    let after = financial(&mut ext)?;
    let after_view = bridge_view(&mut ext, id);
    let events = ext.execute_with(System::events);
    if branch.success {
        require(
            after
                == [
                    before[0],
                    before[1].checked_add(MINT).ok_or("balance overflow")?,
                    before[2].checked_add(MINT).ok_or("issuance overflow")?,
                    before[3].checked_add(MINT).ok_or("locked overflow")?,
                ],
            "native transfer did not credit only replacement with exact issuance/locked accounting",
        )?;
        require(
            ext.execute_with(|| {
                BridgeProxy::sender(GenericNetworkId::Sub(NETWORK), id)
                    == Some(owner(b"replacement-owner"))
                    && BridgeProxy::transactions(
                        (GenericNetworkId::Sub(NETWORK), owner(b"old-owner")),
                        id,
                    )
                    .is_none()
                    && BridgeProxy::transactions(
                        (GenericNetworkId::Sub(NETWORK), owner(b"replacement-owner")),
                        id,
                    )
                    .is_some_and(|r| {
                        r.source() == &GenericAccount::Liberland(owner(b"external-sender"))
                            && r.amount() == MINT
                            && r.asset_id() == &common::XOR
                            && r.status() == MessageStatus::Done
                            && r.direction() == MessageDirection::Inbound
                    })
                    && !frame_system::Account::<Runtime>::contains_key(owner(b"old-owner"))
            }),
            "native effective-owner query/source/status or retired account creation differs",
        )?;
    } else {
        require(after == before, "negative branch changed financial state")?;
        if branch.reopen {
            require(events.iter().any(|record|matches!(&record.event,
                RuntimeEvent::SubstrateBridgeApp(substrate_bridge_app::Event::FailedToMint(message,_)) if *message==id)),
                "baseline old-recipient failure lacks the exact native application failure event")?;
        }
    }
    require(
        commitment.encode() == original_commitment,
        "signed commitment bytes changed",
    )?;
    let acknowledgements = ext.execute_with(|| {
        framenode_runtime::SubstrateBridgeOutboundChannel::recovery_queue(NETWORK)
    });
    if branch.reopen {
        let expected = BridgeCall::SubstrateApp(SubstrateAppCall::ReportTransferResult {
            message_id: id,
            message_status: if branch.success {
                MessageStatus::Done
            } else {
                MessageStatus::Failed
            },
        })
        .encode();
        require(
            acknowledgements.len() == 1
                && acknowledgements[0].payload.as_slice() == expected.as_slice(),
            "disposable native acknowledgement differs",
        )?;
    } else {
        require(
            acknowledgements.is_empty(),
            "halted path queued an acknowledgement",
        )?;
    }
    let mut replay_witness = None;
    if branch.reopen {
        let (tx, witness) = signed(
            &mut ext,
            original_call,
            &public_relayer,
            &parent,
            SignatureCase::Valid,
        )?;
        let prior = replay::root(&mut ext);
        let result = apply(&executor, &mut ext, code, &tx)?;
        require(
            result == Err(TransactionValidityError::Invalid(InvalidTransaction::Call))
                && replay::root(&mut ext) == prior
                && financial(&mut ext)? == after
                && bridge_view(&mut ext, id) == after_view,
            "native consumed-identity replay was not rejected without side effects",
        )?;
        replay_witness = Some(
            json!({"authorization":witness,"result":format!("{result:?}"),"stateRootUnchanged":true,"noSecondCredit":true}),
        );
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
        "authored header/extrinsics differ",
    )?;
    assert_complete(&mut ext, &source.policy, !branch.reopen)?;
    assert_acknowledgements_uncommitted(&mut ext)?;
    require(
        protected_liberland(&mut ext) == protected,
        "diagnostic changed real Liberland queue/counters/tombstones",
    )?;
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
        "independent overlay import differs",
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
        financial(&mut reopened)? == after
            && bridge_view(&mut reopened, id) == after_view
            && protected_liberland(&mut reopened) == protected,
        "fresh disk state differs",
    )?;
    assert_complete(&mut reopened, &source.policy, !branch.reopen)?;
    assert_acknowledgements_uncommitted(&mut reopened)?;
    let result = json!({"name":branch.name,"candidateRuntime":branch.candidate,"runtimeSha256":sha(code),
        "sourceStateRoot":format!("{:#x}",source.header.state_root()),"overlayRoot":format!("{overlay_root:#x}"),
        "syntheticParentHeader":hx(&parent.encode()),"syntheticParentHash":format!("{:#x}",parent.hash()),
        "canonicalHistoryReplay":false,"setup":setup,"overlaySnapshotSha256":overlay_snapshot_sha,
        "network":"Rococo PUBLIC FIXTURE ONLY","syntheticSenderEncoding":"GenericAccount::Liberland; not counterpart identity evidence","originalRecipient":hx(recipient.as_ref()),
        "effectiveOwner":hx(owner(b"replacement-owner").as_ref()),"localMintBaseUnits":MINT.to_string(),
        "externalAmountScale":hx(&external_amount.encode()),"originalCommitmentScale":hx(&original_commitment),
        "originalCommitmentHash":format!("{:#x}",commitment.hash()),"originalProofScale":hx(&original_proof.encode()),
        "messageId":format!("{id:#x}"),"negativeAuthorizations":negatives,"ordinaryAdmission":{"authorization":authorization,
        "result":format!("{result:?}"),"events":format!("{events:?}")},"authenticatedReplay":replay_witness,
        "financialBefore":before.map(|v|v.to_string()),"financialAfter":after.map(|v|v.to_string()),
        "bridgeBefore":before_view,"bridgeAfter":after_view,"expectedReplacementCreditObserved":branch.success,
        "blockSha256":sha(&block_bytes),"blockHash":format!("{:#x}",finalized.hash()),"stateRoot":format!("{:#x}",finalized.state_root()),
        "terminalSnapshotSha256":terminal_snapshot_sha,"independentOverlayImportRootMatches":true,"freshDiskRootMatches":true,
        "realLiberlandStateUnchanged":true,"disposableOutboundCounterUnchanged":true,
        "disposableLatestCommitmentAbsent":true,"productionReady":false});
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
        },
        Branch {
            name: "baseline-original-recipient",
            candidate: false,
            reopen: true,
            mapped: true,
            success: false,
        },
        Branch {
            name: "baseline-live-recipient-control",
            candidate: false,
            reopen: true,
            mapped: false,
            success: true,
        },
        Branch {
            name: "candidate-halted",
            candidate: true,
            reopen: false,
            mapped: true,
            success: false,
        },
        Branch {
            name: "candidate-original-recipient",
            candidate: true,
            reopen: true,
            mapped: true,
            success: true,
        },
    ] {
        results.push(branch(&source, &case, &output.join(case.name))?);
    }
    let report = json!({"schema":SCHEMA,"source":source.evidence,"branches":results,
        "nativeExecution":false,"nativeConfigurationSetupOnly":true,"exactWasmOrdinaryChannelExecution":true,
        "fixtureOnly":true,"productionReady":false,"sourceHistoryModified":false,"canonicalHistoryReplay":false,
        "productionBridgeReopeningAuthorized":false,"realLiberlandReopeningVerified":false,
        "consensusSealsVerified":false,"operatorApprovalVerified":false,
        "limits":["Disposable unused Rococo SubNetworkId with deterministic public multisig peers; no real counterpart authentication or finality.",
        "The synthetic payload preserves GenericAccount::Liberland sender encoding while transport uses unused Rococo; neither network is asserted to be a real authenticated counterpart.",
        "Only the native Substrate-family ordinary dispatcher is exercised. TON, EVM and Parachain proof/application variants are not covered by this diagnostic.",
        "Real Liberland queues, counters and quarantine markers are preserved. Acknowledgements are disposable native local effects, not reconciled external authorization.",
        "Original mixed fixture owner IDs have no supplied signing secret; no old-owner signed-proof claim is made.",
        "Candidate Wasm uses the exact mixed39 replacement policy, not the distinct HASHI38 policy."]});
    write_json(&output.join("report.json"), &report)?;
    Ok(report)
}

#[path = "maintenance_post_complete_families.rs"]
pub mod families;
