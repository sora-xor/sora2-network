//! PUBLIC FIXTURE ONLY HASHI work from the exact observed checkpoint.
//! Temporary public peers are restored. No production signing keys or transactions are used.
use crate::{
    economics, maintenance::hashi_fixture, mixed_bridge_fixture as helpers, mixed_work,
    release_checks, Result, VerifiedCheckpoint,
};
use bridge_types::{GenericAccount, GenericNetworkId, SubNetworkId};
use codec::{Decode, DecodeWithMemLimit, Encode};
use common::{
    recovery::{
        self, invariants::TerminalAudit, AccountReplacement, GuardedStorageEdit, RecoveryAction,
        RecoveryAuditChunk, RecoveryChunk, RecoveryInherent, RecoveryPolicy, StorageScope,
        StorageWitness,
    },
    AssetInfoProvider,
};
use frame_support::dispatch::GetDispatchInfo;
use framenode_runtime::{
    incident_recovery_incoming::{
        HashiPeerApproval, IncomingAudit, IncomingChannel, IncomingOperation,
        NetworkAttackerRegistry,
    },
    AccountId, Runtime, RuntimeCall, System,
};
use helpers::{apply_edits, differences, file_sha, hash, hx, key, observed, rows, Values};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use sp_core::{ed25519, storage::Storage, H160, H256, U256};
use sp_runtime::{generic::Era, MultiSignature};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::Path,
    process::Command,
};

#[path = "../../../runtime/src/incident_recovery_hashi_preview_codec.rs"]
pub(crate) mod wire;
const MINT: u128 = 1;

fn envelope(action: RecoveryAction) -> String {
    hx(&RecoveryInherent {
        format_version: 1,
        action,
    }
    .encode_remark())
}

struct Setup {
    chunk: RecoveryChunk,
    restore: RecoveryChunk,
    incoming: Vec<IncomingOperation>,
    stable_keys: BTreeSet<Vec<u8>>,
    network: u32,
}

fn setup(storage: &Storage) -> Result<Setup> {
    let old = hashi_fixture::owner(b"owner-old");
    let replacement = hashi_fixture::owner(b"owner-new");
    let peers: Vec<_> = hashi_fixture::pairs().into_iter().map(|(a, _)| a).collect();
    let raw_transactions = wire::source_transactions();
    for account in [&old, &replacement].into_iter().chain(peers.iter()) {
        if storage
            .top
            .contains_key(&frame_system::Account::<Runtime>::hashed_key_for(account))
        {
            return Err("public HASHI test account is already present at checkpoint".into());
        }
    }
    if storage
        .top
        .keys()
        .any(|k| k.starts_with(recovery::STORAGE_PREFIX))
        || !storage.children_default.is_empty()
    {
        return Err(
            "HASHI public fixture requires the unrecovered complete top-only checkpoint".into(),
        );
    }
    let mut ext = sp_io::TestExternalities::new(storage.clone());
    let (network, bridge, transfer, approval_timepoint) = ext.execute_with(|| -> Result<_> {
        let network = framenode_runtime::GetEthNetworkId::get();
        let bridge = framenode_runtime::EthBridge::bridge_account(network)
            .ok_or("observed HASHI bridge account absent")?;
        if framenode_runtime::EthBridge::registered_asset(network, common::DAI)
            != Some(eth_bridge::requests::AssetKind::Sidechain)
            || framenode_runtime::Assets::free_balance(&common::DAI, &old)
                .map_err(|e| format!("old asset balance: {e:?}"))?
                != 0
            || framenode_runtime::Assets::free_balance(&common::DAI, &replacement)
                .map_err(|e| format!("new asset balance: {e:?}"))?
                != 0
        {
            return Err(
                "fixture requires observed sidechain DAI and unused synthetic owners".into(),
            );
        }
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 2);
        let approval_timepoint = framenode_runtime::BridgeMultisig::thischain_timepoint();
        for account in &peers {
            System::inc_providers(account);
        }
        eth_bridge::Peers::<Runtime>::insert(
            network,
            peers.iter().cloned().collect::<BTreeSet<_>>(),
        );
        bridge_multisig::Accounts::<Runtime>::insert(
            &bridge,
            bridge_multisig::MultisigAccount::new(peers.clone()),
        );
        System::set_block_number(recovery::CHECKPOINT_NUMBER);
        let transfer = eth_bridge::requests::IncomingTransfer::<Runtime> {
            from: H160::zero(),
            to: old.clone(),
            asset_id: common::DAI,
            asset_kind: eth_bridge::requests::AssetKind::Sidechain,
            amount: MINT,
            author: old.clone(),
            tx_hash: H256::from(sp_io::hashing::keccak_256(&raw_transactions[0])),
            at_height: 123,
            timepoint: framenode_runtime::BridgeMultisig::thischain_timepoint(),
            network_id: network,
            should_take_fee: false,
        };
        if eth_bridge::Requests::<Runtime>::contains_key(network, transfer.tx_hash)
            || eth_bridge::RequestStatuses::<Runtime>::contains_key(network, transfer.tx_hash)
            || eth_bridge::RequestSubmissionHeight::<Runtime>::contains_key(
                network,
                transfer.tx_hash,
            )
            || eth_bridge::LoadToIncomingRequestHash::<Runtime>::contains_key(
                network,
                transfer.tx_hash,
            )
        {
            return Err("public fixture transaction collides with checkpoint replay state".into());
        }
        if eth_bridge::AccountRequests::<Runtime>::contains_key(&old)
            || eth_bridge::AccountRequests::<Runtime>::contains_key(&replacement)
        {
            return Err("public fixture query owner already exists".into());
        }
        let load = eth_bridge::requests::OffchainRequest::<Runtime>::LoadIncoming(
            eth_bridge::requests::LoadIncomingRequest::Transaction(
                eth_bridge::requests::LoadIncomingTransactionRequest::new(
                    old.clone(),
                    transfer.tx_hash,
                    transfer.timepoint,
                    eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                    network,
                ),
            ),
        );
        eth_bridge::Requests::<Runtime>::insert(network, transfer.tx_hash, load);
        eth_bridge::RequestStatuses::<Runtime>::insert(
            network,
            transfer.tx_hash,
            eth_bridge::requests::RequestStatus::Pending,
        );
        eth_bridge::RequestSubmissionHeight::<Runtime>::insert(
            network,
            transfer.tx_hash,
            recovery::CHECKPOINT_NUMBER,
        );
        eth_bridge::RequestsQueue::<Runtime>::mutate(network, |queue| queue.push(transfer.tx_hash));
        // Model the typed compiler's query-index move before authenticated
        // catch-up. The retained signed load still names the original author.
        eth_bridge::AccountRequests::<Runtime>::insert(
            &replacement,
            vec![(network, transfer.tx_hash)],
        );
        Ok((network, bridge, transfer, approval_timepoint))
    })?;
    let changed = differences(&Values::new(), &observed(&mut ext), storage);
    let configuration_keys: BTreeSet<_> = peers
        .iter()
        .map(frame_system::Account::<Runtime>::hashed_key_for)
        .chain([
            eth_bridge::Peers::<Runtime>::hashed_key_for(network),
            bridge_multisig::Accounts::<Runtime>::hashed_key_for(&bridge),
        ])
        .collect();
    let expected_keys: BTreeSet<_> = configuration_keys
        .iter()
        .cloned()
        .chain([
            eth_bridge::Requests::<Runtime>::hashed_key_for(network, transfer.tx_hash),
            eth_bridge::RequestStatuses::<Runtime>::hashed_key_for(network, transfer.tx_hash),
            eth_bridge::RequestSubmissionHeight::<Runtime>::hashed_key_for(
                network,
                transfer.tx_hash,
            ),
            eth_bridge::RequestsQueue::<Runtime>::hashed_key_for(network),
            eth_bridge::AccountRequests::<Runtime>::hashed_key_for(&replacement),
        ])
        .collect();
    if changed.keys().cloned().collect::<BTreeSet<_>>() != expected_keys {
        return Err("unexpected HASHI fixture setup writes".into());
    }
    let chunk = apply_edits(storage, &changed, 0)?;
    let mut restore_edits = Vec::new();
    for edit in &chunk.edits {
        if !configuration_keys.contains(&edit.key) {
            // Native incoming processing consumes the load and preserves its
            // exact historical evidence. Never undo receipt/replay/query state.
            continue;
        }
        let mut before = edit
            .after
            .clone()
            .ok_or("HASHI setup cannot delete state")?;
        if peers
            .iter()
            .take(3)
            .any(|a| frame_system::Account::<Runtime>::hashed_key_for(a) == edit.key)
        {
            let mut account = framenode_runtime::AccountData::decode(&mut before.as_slice())
                .map_err(|e| e.to_string())?;
            account.nonce = account
                .nonce
                .checked_add(wire::INCOMING_COUNT)
                .ok_or("fixture peer nonce overflow")?;
            before = account.encode();
        }
        restore_edits.push(GuardedStorageEdit {
            scope: StorageScope::Top,
            key: edit.key.clone(),
            before: Some(hash(&before)),
            after: storage.top.get(&edit.key).cloned(),
        });
    }
    let restore = RecoveryChunk {
        index: wire::INCOMING_COUNT + 1,
        edits: restore_edits,
    };
    restore
        .validate()
        .map_err(|e| format!("HASHI restore bounds: {e:?}"))?;
    let mut incoming = Vec::new();
    let mut transaction_ids = BTreeSet::new();
    for (offset, raw_transaction) in raw_transactions.into_iter().enumerate() {
        let index = offset as u32 + 1;
        let mut transfer = transfer.clone();
        transfer.tx_hash = H256::from(sp_io::hashing::keccak_256(&raw_transaction));
        if !transaction_ids.insert(transfer.tx_hash) {
            return Err("duplicate public EVM transaction".into());
        }
        // Each later import is fresh, but preserves the same original owner and
        // checkpoint timepoint in its native immutable request.
        if offset != 0
            && [
                eth_bridge::Requests::<Runtime>::hashed_key_for(network, transfer.tx_hash),
                eth_bridge::RequestStatuses::<Runtime>::hashed_key_for(network, transfer.tx_hash),
                eth_bridge::RequestSubmissionHeight::<Runtime>::hashed_key_for(
                    network,
                    transfer.tx_hash,
                ),
                eth_bridge::LoadToIncomingRequestHash::<Runtime>::hashed_key_for(
                    network,
                    transfer.tx_hash,
                ),
            ]
            .iter()
            .any(|key| storage.top.contains_key(key))
        {
            return Err("fresh public source transaction collides with checkpoint".into());
        }
        let original = eth_bridge::requests::IncomingRequest::Transfer(transfer.clone());
        let source = framenode_runtime::incident_recovery_incoming::authenticated_hashi_source(
            &original.encode(),
            &raw_transaction,
            Some(U256::one()),
        )
        .map_err(|e| format!("public source vector: {e:?}"))?;
        if source.signer
            != H160::from(if offset == 2 {
                wire::EXCLUDED_SOURCE_ACTOR
            } else {
                wire::SOURCE_ACTOR
            })
        {
            return Err("independent public source actor differs".into());
        }
        let inner = RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            load_incoming_request: eth_bridge::requests::LoadIncomingRequest::Transaction(
                eth_bridge::requests::LoadIncomingTransactionRequest::new(
                    old.clone(),
                    transfer.tx_hash,
                    transfer.timepoint,
                    eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                    network,
                ),
            ),
            incoming_request_result: Ok(original.clone()),
        });
        let mut timepoint = approval_timepoint;
        timepoint.height =
            bridge_multisig::MultiChainHeight::Thischain(recovery::CHECKPOINT_NUMBER + 1 + index);
        let call = RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
            id: bridge.clone(),
            maybe_timepoint: Some(timepoint),
            call: inner.encode(),
            store_call: false,
            max_weight: ext.execute_with(|| inner.get_dispatch_info().total_weight()),
        })
        .encode();
        let extra = (
            Era::mortal(64, u64::from(recovery::CHECKPOINT_NUMBER + 1)),
            codec::Compact(offset as u32),
            codec::Compact(0u128),
        )
            .encode();
        let actors = vec![
            GenericAccount::EVM(H160::from(recovery::ATTACKER_EVM)),
            GenericAccount::EVM(H160::from(wire::EXCLUDED_SOURCE_ACTOR)),
        ];
        incoming.push(IncomingOperation {
            index,
            channel: IncomingChannel::HashiAuthenticatedSource {
                network,
                request: original.encode(),
                raw_transaction,
                expected_chain_id: Some(U256::one()),
                approvals: peers
                    .iter()
                    .take(3)
                    .map(|signer| HashiPeerApproval {
                        signer: signer.clone(),
                        call: call.clone(),
                        extra: extra.clone(),
                        signature: MultiSignature::Ed25519(ed25519::Signature::from_raw([0; 64])),
                    })
                    .collect(),
            },
            registries: vec![
                NetworkAttackerRegistry {
                    network: GenericNetworkId::EVMLegacy(network),
                    accounts: actors,
                    no_known_attackers: false,
                    attestation: hash(b"PUBLIC HASHI FIXTURE external registry"),
                },
                NetworkAttackerRegistry {
                    network: GenericNetworkId::Sub(SubNetworkId::Mainnet),
                    accounts: vec![GenericAccount::Sora(AccountId::new(
                        recovery::ATTACKER_ACCOUNT,
                    ))],
                    no_known_attackers: false,
                    attestation: hash(b"PUBLIC HASHI FIXTURE local registry"),
                },
            ],
            finalized_source_evidence: hash(b"PUBLIC HASHI FIXTURE ONLY no external finality"),
        });
    }
    let mut stable_keys = expected_keys;
    stable_keys.insert(key(b"Balances", b"TotalIssuance"));
    Ok(Setup {
        chunk,
        restore,
        incoming,
        stable_keys,
        network,
    })
}

struct Preview {
    values: Values,
    operations: Vec<Value>,
    evidence: Value,
}

fn preview(
    storage: &Storage,
    setup: &Setup,
    policy: &RecoveryPolicy,
    audits: &[RecoveryInherent],
) -> Result<Preview> {
    let binary = std::env::var_os("SORA_RECOVERY_FIXTURE_RUNTIME_TEST_BINARY")
        .ok_or("explicit native test exporter required")?;
    let pin = std::env::var("SORA_RECOVERY_FIXTURE_RUNTIME_TEST_SHA256")
        .map_err(|_| "native exporter SHA256 pin required")?;
    let binary = Path::new(&binary);
    if !binary.is_absolute() || pin.len() != 64 || file_sha(binary)? != pin {
        return Err("native HASHI exporter path/hash differs".into());
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "sora-nexus-PUBLIC-FIXTURE-ONLY-hashi-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let input = wire::Input {
        tag: *b"snxhain3",
        checkpoint_top: storage.top.clone(),
        audits: audits.iter().map(RecoveryInherent::encode_remark).collect(),
        setup: setup.chunk.clone(),
        restore: setup.restore.clone(),
        incoming: setup.incoming.iter().map(Encode::encode).collect(),
        policy: policy.clone(),
        stable_keys: setup.stable_keys.iter().cloned().collect(),
        old: hashi_fixture::owner(b"owner-old"),
        replacement: hashi_fixture::owner(b"owner-new"),
        asset: common::DAI,
        mint: MINT * wire::CREDITED_COUNT,
        network: setup.network,
    };
    let encoded = input.encode();
    let input_hash = hash(&encoded);
    let input_path = directory.join("public-input.scale");
    let output_path = directory.join("public-output.scale");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&input_path)
        .map_err(|e| e.to_string())?;
    file.write_all(&encoded).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    drop(encoded);
    drop(input);
    let run = Command::new(binary)
        .args([
            "--ignored",
            "--exact",
            "incident_recovery_hashi_preview_tests::export_public_hashi_preview",
            "--nocapture",
        ])
        .env("SORA_RECOVERY_PUBLIC_HASHI_PREVIEW_INPUT", &input_path)
        .env("SORA_RECOVERY_PUBLIC_HASHI_PREVIEW_OUTPUT", &output_path)
        .output()
        .map_err(|e| e.to_string())?;
    fs::write(directory.join("test-stdout.txt"), run.stdout).map_err(|e| e.to_string())?;
    fs::write(directory.join("test-stderr.txt"), run.stderr).map_err(|e| e.to_string())?;
    if !run.status.success() {
        return Err(format!(
            "native HASHI fixture rejected; evidence {}",
            directory.display()
        ));
    }
    let mut output = Vec::new();
    fs::File::open(&output_path)
        .map_err(|e| e.to_string())?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut output)
        .map_err(|e| e.to_string())?;
    if output.len() > 64 * 1024 * 1024 {
        return Err("HASHI native preview output bound".into());
    }
    let mut cursor = output.as_slice();
    let decoded = wire::Output::decode_with_mem_limit(&mut cursor, 256 * 1024 * 1024)
        .map_err(|e| e.to_string())?;
    if !cursor.is_empty()
        || decoded.encode() != output
        || decoded.tag != *b"snxhao03"
        || decoded.input_hash != input_hash
        || decoded.values.len() > 4096
        || decoded
            .operations
            .iter()
            .map(|(i, _)| *i)
            .collect::<Vec<_>>()
            != [1, 2, 3]
    {
        return Err("native HASHI preview response is not bound to exact input".into());
    }
    let evidence = json!({"fixtureOnly":true,"input":input_path,"inputBlake2":hx(&input_hash),"output":output_path,"outputSha256":hex::encode(Sha256::digest(output)),
        "nativeTestBinary":binary,"nativeTestBinarySha256":pin,"exporterSourceSha256":hex::encode(Sha256::digest(include_bytes!("../../../runtime/src/incident_recovery_hashi_preview_tests.rs"))),
        "signatureContext":"native preview uses disclosed synthetic birth; exact Wasm signer independently binds its actual postfork birth header"});
    fs::write(
        directory.join("PUBLIC-FIXTURE-PROVENANCE.json"),
        serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(Preview {
        values: decoded.values,
        operations: decoded
            .operations
            .into_iter()
            .map(|(index, values)| json!({"index":index,"records":rows(&values)}))
            .collect(),
        evidence,
    })
}

pub fn generate(verified: VerifiedCheckpoint) -> Result<Value> {
    let setup = setup(&verified.storage)?;
    let mut policy = RecoveryPolicy {
        format_version: 2,
        authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
        checkpoint_hash: recovery::CHECKPOINT_HASH,
        checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
        replacements: vec![AccountReplacement {
            old: hashi_fixture::owner(b"owner-old"),
            new: hashi_fixture::owner(b"owner-new"),
        }],
        council: release_checks::checkpoint_council(&verified.schema, &verified.storage)?,
        chunk_hashes: core::iter::once(setup.chunk.commitment())
            .chain(setup.incoming.iter().map(IncomingOperation::commitment))
            .chain(core::iter::once(setup.restore.commitment()))
            .collect(),
        audit_chunk_hashes: vec![[1; 32]],
        evidence_commitment: hash(&[hashi_fixture::DOMAIN, b"public-diagnostic-only"].concat()),
        terminal_audit_commitment: [2; 32],
    };
    let (mut incoming_audits, dynamic_keys) =
        helpers::semantic_audits(&verified.storage, &setup.chunk, &policy, &setup.incoming)?;
    let first = preview(&verified.storage, &setup, &policy, &[])?;
    if dynamic_keys
        .iter()
        .any(|key| first.values.get(key).and_then(Option::as_ref).is_none())
    {
        return Err("HASHI preview omitted an exact dynamic incoming audit target".into());
    }
    let stable: Values = first
        .values
        .iter()
        .filter(|(k, _)| !k.starts_with(recovery::STORAGE_PREFIX) && !dynamic_keys.contains(*k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let witnesses: Vec<_> = stable
        .iter()
        .map(|(key, value)| StorageWitness {
            scope: StorageScope::Top,
            key: key.clone(),
            value_hash: value.as_ref().map(|v| hash(v)),
        })
        .collect();
    let audits: Vec<_> = witnesses
        .chunks(recovery::MAX_AUDIT_WITNESSES)
        .enumerate()
        .map(|(index, witnesses)| RecoveryAuditChunk {
            index: index as u32,
            witnesses: witnesses.to_vec(),
        })
        .collect();
    for audit in &audits {
        audit
            .validate()
            .map_err(|e| format!("HASHI audit: {e:?}"))?;
    }
    let mut issuance = Vec::new();
    for asset in [common::XOR, common::DAI] {
        let invariant = economics::issuance_baseline(
            &verified.schema,
            &verified.storage,
            asset.encode().try_into().map_err(|_| "asset codec")?,
        )?;
        // Native execution journals exact issuance deltas and final record hashes;
        // never hardcode a gross transfer amount into terminal conservation.
        issuance.push(invariant);
    }
    let terminal = TerminalAudit {
        issuance,
        collateral: vec![],
        zero_claims: vec![],
        absent: vec![
            StorageWitness {
                scope: StorageScope::Top,
                key: frame_system::Account::<Runtime>::hashed_key_for(hashi_fixture::owner(
                    b"owner-old",
                )),
                value_hash: None,
            },
            StorageWitness {
                scope: StorageScope::Top,
                key: eth_bridge::AccountRequests::<Runtime>::hashed_key_for(hashi_fixture::owner(
                    b"owner-old",
                )),
                value_hash: None,
            },
        ],
    };
    for (offset, audit) in incoming_audits.iter_mut().enumerate() {
        audit.index = (audits.len() + offset) as u32;
    }
    policy.audit_chunk_hashes = audits
        .iter()
        .map(RecoveryAuditChunk::commitment)
        .chain(incoming_audits.iter().map(IncomingAudit::commitment))
        .collect();
    let audit_actions: Vec<_> = audits
        .into_iter()
        .map(RecoveryAction::Audit)
        .chain(
            incoming_audits
                .into_iter()
                .map(|audit| RecoveryAction::IncomingAudit(audit.encode())),
        )
        .map(|action| RecoveryInherent {
            format_version: 1,
            action,
        })
        .collect();
    policy.terminal_audit_commitment = terminal.commitment();
    policy
        .validate()
        .map_err(|e| format!("HASHI fixture policy: {e:?}"))?;
    let final_preview = preview(&verified.storage, &setup, &policy, &audit_actions)?;
    if final_preview
        .values
        .iter()
        .filter(|(k, _)| !k.starts_with(recovery::STORAGE_PREFIX) && !dynamic_keys.contains(*k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Values>()
        != stable
    {
        return Err("HASHI policy rebinding changed native accounting".into());
    }
    let input = json!({"schema":"sora-nexus-mixed-work-input-v1","policyTemplateScale":hx(&policy.encode()),
        "workEnvelopes":core::iter::once(envelope(RecoveryAction::Apply(setup.chunk)))
            .chain(setup.incoming.iter().map(|op|envelope(RecoveryAction::Incoming(op.encode()))))
            .chain(core::iter::once(envelope(RecoveryAction::Apply(setup.restore)))).collect::<Vec<_>>(),
        "auditEnvelopes":audit_actions.iter().map(|audit|hx(&audit.encode_remark())).collect::<Vec<_>>(),"terminalEnvelope":envelope(RecoveryAction::TerminalAudit(terminal))});
    let work =
        mixed_work::compile(serde_json::from_value(input.clone()).map_err(|e| e.to_string())?)?;
    let public_peers: Vec<_> = hashi_fixture::pairs()
        .iter()
        .map(|(a, _)| hx(a.as_ref()))
        .collect();
    Ok(
        json!({"schema":"sora-nexus-PUBLIC-FIXTURE-ONLY-hashi-bridge-v2","fixtureOnly":true,"productionReady":false,
        "warning":"PUBLIC HASHI TEST PEERS. NEVER DEPLOY. No external transfer, validator, victim or release authorization is proven.",
        "checkpoint":verified.state,"metadataWitness":verified.metadata_witness,"nativePreviewCompleted":true,"nativePreviewEvidence":[first.evidence,final_preview.evidence],"exactWasmRehearsed":false,
        "syntheticRetainedPendingLoad":true,"syntheticLoadSubmissionHeight":recovery::CHECKPOINT_NUMBER,"queryIndexMigratedBeforeAuthentication":true,
        "minted":(MINT * wire::CREDITED_COUNT).to_string(),
        "nativeZeroSenderPreserved":true,"sourceActor":hx(&wire::SOURCE_ACTOR),"excludedSourceActor":hx(&wire::EXCLUDED_SOURCE_ACTOR),"identicalSourceRegistryAcrossOperations":true,"signatureChainDomain":"0x01",
        "incomingScenarios":["retained-pending-credit","fresh-credit","fresh-source-actor-exclusion"],
        "sourceActorScope":"Outer transaction signer only; synthetic native peer-approved Deposit association; no receipt, external finality, routed caller or economic ownership is proven.","asset":hx(&common::DAI.encode()),"input":input,"work":work,
        "fixtureHashiProvenance":{"schema":"sora-nexus-public-fixture-hashi-inputs-v1","fixtureOnly":true,"productionReady":false,"peerScheme":hashi_fixture::SCHEME,"publicPeers":public_peers,
            "policyCommitment":hx(&policy.commitment()),"checkpointHash":hx(&recovery::CHECKPOINT_HASH)},
        "fixtureExpectations":{"schema":"sora-nexus-public-fixture-storage-expectations-v1","fixtureOnly":true,"policyCommitment":hx(&policy.commitment()),"records":rows(&final_preview.values),"operations":final_preview.operations}}),
    )
}
