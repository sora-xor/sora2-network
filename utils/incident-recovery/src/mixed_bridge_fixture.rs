//! PUBLIC-FIXTURE-ONLY native bridge work for exact-Wasm rehearsals.
//!
//! Synthetic peer keys and transfer evidence deliberately have no production
//! authority. Every setup byte is guarded by the verified checkpoint preimage.
//! Native accounting determines the expected effects; the Wasm harness must
//! reproduce them independently before this fixture can report execution success.

use crate::{economics, mixed_work, release_checks, Result, VerifiedCheckpoint};
use bridge_types::{
    substrate::{BridgeCall, BridgeMessage, SubstrateAppCall},
    traits::{
        BalancePrecisionConverter, BridgeAssetRegistry, MessageStatusNotifier, OutboundChannel,
    },
    types::{AssetKind, AuxiliaryDigest, AuxiliaryDigestItem, MessageStatus},
    GenericAccount, GenericAssetId, GenericBalance, GenericCommitment, GenericNetworkId,
    GenericTimepoint, SubNetworkId,
};
use codec::{DecodeWithMemLimit, Encode};
use common::recovery::{
    self, invariants::TerminalAudit, AccountReplacement, GuardedStorageEdit, RecoveryAction,
    RecoveryAuditChunk, RecoveryChunk, RecoveryInherent, RecoveryPolicy, StorageScope,
    StorageWitness,
};
use framenode_runtime::{
    incident_recovery_incoming::{
        audit_dynamic_keys, audit_targets, expected_requests, IncomingAudit, IncomingChannel,
        IncomingOperation, NetworkAttackerRegistry,
    },
    incident_recovery_outgoing::{
        ChannelKind, FinancialWitness, MessageWitness, OutgoingOperation, OutgoingTarget,
    },
    AccountId, AssetId, Balances, BridgeMaxMessagePayloadSize, BridgeMaxMessagesPerCommit,
    BridgeProxy, MultiProof, Runtime, SubstrateBridgeOutboundChannel, System, ThisNetworkId,
};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use sp_core::{ecdsa, storage::Storage, Pair};
use sp_runtime::{
    traits::{Hash, Keccak256},
    BoundedBTreeSet,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
    process::Command,
};
#[path = "../../../runtime/src/incident_recovery_bridge_preview_codec.rs"]
mod preview_codec;

const DOMAIN: &[u8] = b"sora-nexus/PUBLIC-FIXTURE-ONLY/mixed-bridge/v1/";
// The checkpoint's XOR denomination factor is 10^38. One local base unit
// remains representable by the native u128 sidechain amount; one whole XOR does
// not. Exercise a positive mint without rewriting denomination or precision.
const MINT: u128 = 1;
const NETWORK: SubNetworkId = SubNetworkId::Liberland;
pub(crate) type Values = BTreeMap<Vec<u8>, Option<Vec<u8>>>;
type Commitment = GenericCommitment<BridgeMaxMessagesPerCommit, BridgeMaxMessagePayloadSize>;

/// Derive fixed business descriptors under the guarded consumption configuration.
/// The native preview independently executes the operation and verifies the
/// resulting request. Only exact nonexcluded request keys and authenticated
/// fresh/retried HASHI load heights lose their brittle whole-value witnesses.
/// Preserved load heights, monetary and every other state witness remain.
pub(crate) fn semantic_audits(
    storage: &Storage,
    setup: &RecoveryChunk,
    policy: &RecoveryPolicy,
    operations: &[IncomingOperation],
) -> Result<(Vec<IncomingAudit>, BTreeSet<Vec<u8>>)> {
    let mut ext = sp_io::TestExternalities::new(storage.clone());
    ext.execute_with(|| {
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 1);
        recovery::storage::initialize(policy)
            .map_err(|error| format!("audit setup initialize: {error:?}"))?;
        recovery::storage::apply_chunk(policy, setup)
            .map_err(|error| format!("audit setup preimage: {error:?}"))?;
        let mut audits = Vec::new();
        let mut keys = BTreeSet::new();
        for operation in operations {
            let requests = expected_requests(operation, policy)
                .map_err(|error| format!("native incoming descriptor: {error:?}"))?;
            let targets = audit_targets(operation, policy)
                .map_err(|error| format!("native incoming targets: {error:?}"))?;
            if requests.len() != targets.len() {
                return Err("native incoming descriptor/target coverage mismatch".into());
            }
            for (target, request) in targets.iter().zip(&requests) {
                if target.excluded != request.is_none() {
                    return Err("native incoming descriptor exclusion mismatch".into());
                }
            }
            for key in audit_dynamic_keys(operation, policy)
                .map_err(|error| format!("native incoming dynamic audit keys: {error:?}"))?
            {
                if !keys.insert(key) {
                    return Err("duplicate native incoming dynamic audit target".into());
                }
            }
            let audit = IncomingAudit {
                index: 0,
                operation_index: operation.index,
                operation_commitment: operation.commitment(),
                requests,
                prior_policy: None,
            };
            audit
                .validate()
                .map_err(|error| format!("native incoming audit bounds: {error:?}"))?;
            audits.push(audit);
        }
        Ok((audits, keys))
    })
}

fn external_amount(local: u128) -> Result<GenericBalance> {
    type Converter = <Runtime as substrate_bridge_app::Config>::BalancePrecisionConverter;
    let (_, external) = Converter::to_sidechain(&common::XOR, 18, local)
        .ok_or("fixture mint is not representable by the native sidechain converter")?;
    let (roundtrip, _) = Converter::from_sidechain(&common::XOR, 18, external.clone())
        .ok_or("fixture sidechain amount does not convert back")?;
    if local == 0 || roundtrip != local {
        return Err("fixture amount has zero value or loses precision".into());
    }
    Ok(external)
}

pub(crate) fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
pub(crate) fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn id(label: &[u8]) -> AccountId {
    AccountId::new(hash(&[DOMAIN, label].concat()))
}
pub(crate) fn key(pallet: &[u8], item: &[u8]) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(pallet),
        sp_io::hashing::twox_128(item),
    ]
    .concat()
}
fn envelope(action: RecoveryAction) -> String {
    hx(&RecoveryInherent {
        format_version: 1,
        action,
    }
    .encode_remark())
}
pub(crate) fn read(ext: &mut sp_io::TestExternalities, key: &[u8]) -> Option<Vec<u8>> {
    ext.execute_with(|| sp_io::storage::get(key).map(|b| b.to_vec()))
}
fn consensus(key_bytes: &[u8]) -> bool {
    key_bytes == b":extrinsic_index"
        || (!key_bytes.starts_with(&key(b"System", b"Account"))
            && [
                b"System".as_slice(),
                b"Babe",
                b"Grandpa",
                b"Session",
                b"Timestamp",
            ]
            .iter()
            .any(|name| key_bytes.starts_with(&sp_io::hashing::twox_128(name))))
}
pub(crate) fn observed(ext: &mut sp_io::TestExternalities) -> Values {
    let keys: Vec<_> = ext
        .overlayed_changes()
        .changes()
        .map(|(key, _)| key.clone())
        .collect();
    keys.into_iter()
        .filter(|key| !consensus(key))
        .map(|key| {
            let value = read(ext, &key);
            (key, value)
        })
        .collect()
}
pub(crate) fn differences(before: &Values, after: &Values, storage: &Storage) -> Values {
    after
        .iter()
        .filter(|(key, value)| {
            let prior = before
                .get(*key)
                .cloned()
                .unwrap_or_else(|| storage.top.get(*key).cloned());
            &prior != *value
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
pub(crate) fn rows(values: &Values) -> Vec<Value> {
    values
        .iter()
        .map(|(key, value)| json!({"key":hx(key),"valueHash":value.as_ref().map(|v| hx(&hash(v)))}))
        .collect()
}
pub(crate) fn apply_edits(storage: &Storage, values: &Values, index: u32) -> Result<RecoveryChunk> {
    let edits = values
        .iter()
        .filter(|(key, value)| storage.top.get(*key) != value.as_ref())
        .map(|(key, after)| GuardedStorageEdit {
            scope: StorageScope::Top,
            key: key.clone(),
            before: storage.top.get(key).map(|v| hash(v)),
            after: after.clone(),
        })
        .collect();
    let chunk = RecoveryChunk { index, edits };
    chunk
        .validate()
        .map_err(|e| format!("fixture setup bounds: {e:?}"))?;
    Ok(chunk)
}

struct Setup {
    chunk: RecoveryChunk,
    restore: RecoveryChunk,
    incoming: IncomingOperation,
    outgoing: OutgoingOperation,
    stable_keys: BTreeSet<Vec<u8>>,
    incoming_nonce: u64,
    outgoing_nonce: u64,
    external_amount: GenericBalance,
}

/// Setup invokes only native configuration and queue-record constructors. It
/// never mints fixture funds, debits real accounts or creates a refundable claim.
fn setup(storage: &Storage) -> Result<Setup> {
    for who in [id(b"old-owner"), id(b"replacement-owner")] {
        if storage
            .top
            .contains_key(&frame_system::Account::<Runtime>::hashed_key_for(&who))
        {
            return Err("public synthetic fixture account is occupied at checkpoint".into());
        }
    }
    if storage
        .top
        .keys()
        .any(|k| k.starts_with(recovery::STORAGE_PREFIX))
    {
        return Err("fixture must start at the unrecovered checkpoint".into());
    }
    let mut ext = sp_io::TestExternalities::new(storage.clone());
    let (incoming_nonce, outgoing_nonce, messages, external_amount) =
        ext.execute_with(|| -> Result<_> {
            System::set_block_number(recovery::CHECKPOINT_NUMBER + 1);
            if !substrate_bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(NETWORK)
                .is_empty()
            {
                return Err("fixture refuses to replace a real pending Liberland queue".into());
            }
            let before_issuance = Balances::total_issuance();
            let incoming_nonce =
                substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(NETWORK)
                    .checked_add(1)
                    .ok_or("inbound nonce overflow")?;
            let outgoing_nonce =
                substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(NETWORK)
                    .checked_add(1)
                    .ok_or("outbound nonce overflow")?;
            let peers = (0u8..4)
                .map(|n| ecdsa::Pair::from_seed(&hash(&[DOMAIN, b"peer/", &[n]].concat())).public())
                .collect::<BTreeSet<_>>();
            multisig_verifier::PeerKeys::<Runtime>::insert(
                GenericNetworkId::Sub(NETWORK),
                BoundedBTreeSet::<_, <Runtime as multisig_verifier::Config>::MaxPeers>::try_from(
                    peers,
                )
                .map_err(|_| "fixture peer bound")?,
            );
            <BridgeProxy as BridgeAssetRegistry<AccountId, AssetId>>::manage_asset(
                NETWORK.into(),
                common::XOR,
            )
            .map_err(|e| format!("native fixture asset management: {e:?}"))?;
            // These four private native maps have Identity keys. Values use the exact
            // runtime types, and the original configuration is restored after work.
            for (item, suffix, value) in [
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
            ] {
                sp_io::storage::set(
                    &[key(b"SubstrateBridgeApp", item), NETWORK.encode(), suffix].concat(),
                    &value,
                );
            }
            let mut messages = Vec::new();
            for (position, owner) in [id(b"old-owner"), AccountId::new(recovery::ATTACKER_ACCOUNT)]
                .into_iter()
                .enumerate()
            {
                let message_id = SubstrateBridgeOutboundChannel::submit(
                    NETWORK,
                    &frame_system::RawOrigin::Signed(owner.clone()),
                    &[0xf0, position as u8],
                    (),
                )
                .map_err(|e| format!("native fixture queue: {e:?}"))?;
                BridgeProxy::outbound_request(
                    NETWORK.into(),
                    message_id,
                    owner.clone(),
                    GenericAccount::Liberland(id(b"destination")),
                    common::XOR,
                    100 + position as u128,
                    MessageStatus::InQueue,
                );
                let queue =
                    substrate_bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(NETWORK);
                let record = BridgeProxy::transactions(
                    (GenericNetworkId::Sub(NETWORK), owner.clone()),
                    message_id,
                )
                .ok_or("native outgoing fixture record missing")?;
                messages.push(MessageWitness {
                    message_id,
                    message_hash: hash(&queue[position].encode()),
                    financial: Some(FinancialWitness {
                        owner,
                        asset: common::XOR,
                        amount: 100 + position as u128,
                        record_hash: hash(&record.encode()),
                    }),
                });
            }
            if Balances::total_issuance() != before_issuance {
                return Err("fixture setup unexpectedly changed issuance".into());
            }
            Ok((
                incoming_nonce,
                outgoing_nonce,
                messages,
                external_amount(MINT)?,
            ))
        })?;
    let changed = differences(&BTreeMap::new(), &observed(&mut ext), storage);
    let allowed: Vec<_> = [
        b"MultisigVerifier".as_slice(),
        b"Technical",
        b"Permissions",
        b"SubstrateBridgeApp",
        b"SubstrateBridgeOutboundChannel",
        b"BridgeProxy",
    ]
    .iter()
    .map(|name| sp_io::hashing::twox_128(name))
    .collect();
    if changed.keys().any(|k| {
        !k.starts_with(&key(b"System", b"Account"))
            && !allowed.iter().any(|prefix| k.starts_with(prefix))
    }) {
        return Err("unreviewed native fixture setup write".into());
    }
    let chunk = apply_edits(storage, &changed, 0)?;
    // Retain the exact quarantined queue and request evidence. Restore temporary
    // peer/configuration/permission/custody-registration changes verbatim.
    let restore = RecoveryChunk {
        index: 3,
        edits: chunk
            .edits
            .iter()
            .filter(|e| {
                !e.key
                    .starts_with(&sp_io::hashing::twox_128(b"SubstrateBridgeOutboundChannel"))
                    && !e.key.starts_with(&sp_io::hashing::twox_128(b"BridgeProxy"))
            })
            .map(|e| GuardedStorageEdit {
                scope: e.scope.clone(),
                key: e.key.clone(),
                before: e.after.as_ref().map(|v| hash(v)),
                after: storage.top.get(&e.key).cloned(),
            })
            .collect(),
    };
    restore
        .validate()
        .map_err(|e| format!("fixture restoration bounds: {e:?}"))?;
    let commitment: Commitment = GenericCommitment::Sub(bridge_types::substrate::Commitment {
        nonce: incoming_nonce,
        messages: [id(b"old-owner"), AccountId::new(recovery::ATTACKER_ACCOUNT)]
            .into_iter()
            .map(|recipient| {
                let payload = BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
                    sender: GenericAccount::Liberland(id(b"external-sender")),
                    recipient: GenericAccount::Sora(recipient),
                    asset_id: GenericAssetId::Sora(common::XOR.into()),
                    amount: external_amount.clone(),
                })
                .encode()
                .try_into()
                .expect("bounded fixed transfer");
                BridgeMessage {
                    payload,
                    timepoint: GenericTimepoint::Sora(recovery::CHECKPOINT_NUMBER),
                }
            })
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "fixture commitment bound")?,
    });
    let digest = AuxiliaryDigest {
        logs: vec![AuxiliaryDigestItem::Commitment(
            ThisNetworkId::get(),
            commitment.hash(),
        )],
    };
    let proof = MultiProof::Multisig(multisig_verifier::Proof {
        digest: digest.clone(),
        proof: (0u8..4)
            .map(|n| {
                ecdsa::Pair::from_seed(&hash(&[DOMAIN, b"peer/", &[n]].concat()))
                    .sign_prehashed(Keccak256::hash_of(&digest).as_fixed_bytes())
            })
            .collect(),
    });
    let incoming = IncomingOperation {
        index: 1,
        channel: IncomingChannel::Substrate {
            network: NETWORK,
            commitment,
            proof,
        },
        finalized_source_evidence: hash(&[DOMAIN, b"SYNTHETIC-NOT-SOURCE-FINALITY"].concat()),
        registries: vec![
            NetworkAttackerRegistry {
                network: NETWORK.into(),
                accounts: vec![],
                no_known_attackers: true,
                attestation: hash(&[DOMAIN, b"synthetic-source-registry"].concat()),
            },
            NetworkAttackerRegistry {
                network: ThisNetworkId::get(),
                accounts: vec![GenericAccount::Sora(AccountId::new(
                    recovery::ATTACKER_ACCOUNT,
                ))],
                no_known_attackers: false,
                attestation: hash(&[DOMAIN, b"synthetic-destination-registry"].concat()),
            },
        ],
    };
    let outgoing = OutgoingOperation {
        index: 2,
        target: OutgoingTarget::Channel {
            channel: ChannelKind::Substrate,
            network: NETWORK.into(),
            nonce: outgoing_nonce,
            commitment: None,
            archived: None,
            messages,
        },
        reconciliation_evidence: hash(&[DOMAIN, b"NO-REFUND-NO-EXTERNAL-RECONCILIATION"].concat()),
    };
    Ok(Setup {
        stable_keys: chunk.edits.iter().map(|e| e.key.clone()).collect(),
        chunk,
        restore,
        incoming,
        outgoing,
        incoming_nonce,
        outgoing_nonce,
        external_amount,
    })
}

struct Preview {
    values: Values,
    operations: Vec<Value>,
    evidence: Value,
}
pub(crate) fn file_sha(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(hex::encode(hash.finalize()))
}
fn preview(
    storage: &Storage,
    setup: &Setup,
    policy: &RecoveryPolicy,
    audits: &[RecoveryInherent],
) -> Result<Preview> {
    if !storage.children_default.is_empty() {
        return Err("public preview transport does not omit child tries".into());
    }
    let binary=std::env::var_os("SORA_RECOVERY_FIXTURE_RUNTIME_TEST_BINARY").ok_or("explicit pinned native runtime test exporter required; unbound production runtime cannot preview recovery minting")?;
    let pin = std::env::var("SORA_RECOVERY_FIXTURE_RUNTIME_TEST_SHA256")
        .map_err(|_| "native test binary SHA256 pin required")?;
    let binary = Path::new(&binary);
    if !binary.is_absolute() || pin.len() != 64 || file_sha(binary)? != pin {
        return Err("native fixture test binary hash/path mismatch".into());
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "sora-nexus-PUBLIC-FIXTURE-ONLY-bridge-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let input = preview_codec::Input {
        tag: *b"snxbrin2",
        checkpoint_top: storage.top.clone(),
        audits: audits.iter().map(RecoveryInherent::encode_remark).collect(),
        setup: setup.chunk.clone(),
        restore: setup.restore.clone(),
        incoming: setup.incoming.encode(),
        outgoing: setup.outgoing.encode(),
        policy: policy.clone(),
        stable_keys: setup.stable_keys.iter().cloned().collect(),
        old: id(b"old-owner"),
        replacement: id(b"replacement-owner"),
        mint: MINT,
        incoming_nonce: setup.incoming_nonce,
        outgoing_nonce: setup.outgoing_nonce,
    };
    let encoded = input.encode();
    let input_hash = hash(&encoded);
    let input_path = directory.join("public-input.scale");
    let output_path = directory.join("public-output.scale");
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&input_path)
        .map_err(|e| e.to_string())?;
    output.write_all(&encoded).map_err(|e| e.to_string())?;
    output.sync_all().map_err(|e| e.to_string())?;
    drop(output);
    drop(input);
    drop(encoded);
    let run = Command::new(binary)
        .args([
            "--ignored",
            "--exact",
            "incident_recovery_bridge_preview_tests::export_public_mixed_bridge_preview",
            "--nocapture",
        ])
        .env("SORA_RECOVERY_PUBLIC_PREVIEW_INPUT", &input_path)
        .env("SORA_RECOVERY_PUBLIC_PREVIEW_OUTPUT", &output_path)
        .output()
        .map_err(|e| e.to_string())?;
    fs::write(directory.join("test-stdout.txt"), &run.stdout).map_err(|e| e.to_string())?;
    fs::write(directory.join("test-stderr.txt"), &run.stderr).map_err(|e| e.to_string())?;
    if !run.status.success() {
        return Err(format!(
            "native public preview exporter rejected; retained evidence {}",
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
        return Err("native preview output bound".into());
    }
    let mut cursor = output.as_slice();
    let decoded = preview_codec::Output::decode_with_mem_limit(&mut cursor, 256 * 1024 * 1024)
        .map_err(|e| e.to_string())?;
    if !cursor.is_empty()
        || decoded.encode() != output
        || decoded.tag != *b"snxbro02"
        || decoded.input_hash != input_hash
        || decoded.values.len() > 4096
        || decoded.operations.len() != 2
        || decoded
            .operations
            .iter()
            .map(|(i, _)| *i)
            .collect::<Vec<_>>()
            != [1, 2]
    {
        return Err("native preview response is not bound to this exact request".into());
    }
    let evidence = json!({"fixtureOnly":true,"input":input_path,"inputBlake2":hx(&input_hash),"output":output_path,
        "outputSha256":hex::encode(Sha256::digest(&output)),"nativeTestBinary":binary,"nativeTestBinarySha256":pin,
        "exporterSourceSha256":hex::encode(Sha256::digest(include_bytes!("../../../runtime/src/incident_recovery_bridge_preview_tests.rs")))});
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
    let council = release_checks::checkpoint_council(&verified.schema, &verified.storage)?;
    let mut policy = RecoveryPolicy {
        format_version: 2,
        authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
        checkpoint_hash: recovery::CHECKPOINT_HASH,
        checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
        replacements: vec![AccountReplacement {
            old: id(b"old-owner"),
            new: id(b"replacement-owner"),
        }],
        council,
        chunk_hashes: vec![
            setup.chunk.commitment(),
            setup.incoming.commitment(),
            setup.outgoing.commitment(),
            setup.restore.commitment(),
        ],
        audit_chunk_hashes: vec![[1; 32]],
        evidence_commitment: hash(&[DOMAIN, b"synthetic-public-inputs-not-production"].concat()),
        terminal_audit_commitment: [2; 32],
    };
    let (mut incoming_audits, dynamic_keys) = semantic_audits(
        &verified.storage,
        &setup.chunk,
        &policy,
        core::slice::from_ref(&setup.incoming),
    )?;
    let first = preview(&verified.storage, &setup, &policy, &[])?;
    if dynamic_keys
        .iter()
        .any(|key| first.values.get(key).and_then(Option::as_ref).is_none())
    {
        return Err("native preview omitted an exact dynamic incoming audit target".into());
    }
    let stable: Values = first
        .values
        .iter()
        .filter(|(k, _)| !k.starts_with(recovery::STORAGE_PREFIX) && !dynamic_keys.contains(*k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let audits: Vec<_> = stable
        .iter()
        .map(|(key, value)| StorageWitness {
            scope: StorageScope::Top,
            key: key.clone(),
            value_hash: value.as_ref().map(|v| hash(v)),
        })
        .collect::<Vec<_>>()
        .chunks(recovery::MAX_EDITS_PER_CHUNK)
        .enumerate()
        .map(|(index, witnesses)| RecoveryAuditChunk {
            index: index as u32,
            witnesses: witnesses.to_vec(),
        })
        .collect();
    for audit in &audits {
        audit
            .validate()
            .map_err(|e| format!("fixture audit bounds: {e:?}"))?;
    }
    let issuance = economics::issuance_baseline(
        &verified.schema,
        &verified.storage,
        common::XOR
            .encode()
            .try_into()
            .map_err(|_| "native asset codec")?,
    )?;
    let terminal = TerminalAudit {
        issuance: vec![issuance],
        collateral: vec![],
        zero_claims: vec![],
        absent: vec![StorageWitness {
            scope: StorageScope::Top,
            key: frame_system::Account::<Runtime>::hashed_key_for(id(b"old-owner")),
            value_hash: None,
        }],
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
        .map_err(|e| format!("fixture policy: {e:?}"))?;
    let final_preview = preview(&verified.storage, &setup, &policy, &audit_actions)?;
    let final_stable: Values = final_preview
        .values
        .iter()
        .filter(|(key, _)| {
            !key.starts_with(recovery::STORAGE_PREFIX) && !dynamic_keys.contains(*key)
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if final_stable != stable {
        return Err("policy binding changed native business accounting".into());
    }
    let work_envelopes = vec![
        envelope(RecoveryAction::Apply(setup.chunk.clone())),
        envelope(RecoveryAction::Incoming(setup.incoming.encode())),
        envelope(RecoveryAction::Outgoing(setup.outgoing.encode())),
        envelope(RecoveryAction::Apply(setup.restore.clone())),
    ];
    let audit_envelopes: Vec<_> = audit_actions
        .iter()
        .map(|audit| hx(&audit.encode_remark()))
        .collect();
    let terminal_envelope = envelope(RecoveryAction::TerminalAudit(terminal));
    let input = json!({"schema":"sora-nexus-mixed-work-input-v1","policyTemplateScale":hx(&policy.encode()),
        "workEnvelopes":work_envelopes,"auditEnvelopes":audit_envelopes,"terminalEnvelope":terminal_envelope});
    let work =
        mixed_work::compile(serde_json::from_value(input.clone()).map_err(|e| e.to_string())?)?;
    Ok(
        json!({"schema":"sora-nexus-PUBLIC-FIXTURE-ONLY-mixed-bridge-v1","fixtureOnly":true,"productionReady":false,
            "warning":"PUBLIC-FIXTURE-ONLY. NEVER DEPLOY. Synthetic peer keys, signed messages and outgoing records prove no production authorization, external finality, refundable liability or operator approval.",
            "checkpoint":verified.state,"metadataWitness":verified.metadata_witness,
            "councilSource":"observed checkpoint council, diagnostic only; governance state unchanged",
            "publicKeyDerivation":"Blake2-256(domain || peer/ || one-byte index), secp256k1; domain is sora-nexus/PUBLIC-FIXTURE-ONLY/mixed-bridge/v1/",
            "oldOwner":hx(id(b"old-owner").as_ref()),"replacementOwner":hx(id(b"replacement-owner").as_ref()),
            "minted":MINT.to_string(),"externalAmountScale":hx(&setup.external_amount.encode()),"attackerMinted":"0","outgoingRefunds":"0",
            "setupChangedKeys":setup.chunk.edits.len(),"restoredConfigurationKeys":setup.restore.edits.len(),
            "nativePreviewCompleted":true,"nativePreviewEvidence":[first.evidence,final_preview.evidence],"exactWasmRehearsed":false,"input":input,"work":work,
            "fixtureExpectations":{"schema":"sora-nexus-public-fixture-storage-expectations-v1","fixtureOnly":true,
                "policyCommitment":hx(&policy.commitment()),"records":rows(&final_preview.values),"operations":final_preview.operations}
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoint_denomination_uses_exact_positive_native_roundtrip() {
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            let factor = 10u128.pow(38);
            sp_io::storage::set(&key(b"Denomination", b"Denominator"), &factor.encode());
            let asset_key = [
                key(b"Assets", b"AssetInfosV2"),
                sp_io::hashing::twox_64(&common::XOR.encode()).to_vec(),
                common::XOR.encode(),
            ]
            .concat();
            sp_io::storage::set(
                &asset_key,
                &common::AssetInfo {
                    precision: 18,
                    ..Default::default()
                }
                .encode(),
            );
            assert_eq!(
                external_amount(MINT).unwrap(),
                GenericBalance::Substrate(factor)
            );
            assert!(external_amount(1_000_000_000_000_000_000).is_err());
            assert!(external_amount(0).is_err());
            type Converter = <Runtime as substrate_bridge_app::Config>::BalancePrecisionConverter;
            assert_eq!(
                Converter::from_sidechain(
                    &common::XOR,
                    18,
                    GenericBalance::Substrate(1_000_000_000_000_000_000)
                )
                .unwrap()
                .0,
                0,
            );
        });
    }
    #[test]
    fn complete_diff_keeps_accounts_and_recovery_records_but_excludes_consensus() {
        assert!(!consensus(
            &frame_system::Account::<Runtime>::hashed_key_for(id(b"x"))
        ));
        assert!(!consensus(recovery::STATUS_STORAGE_KEY));
        assert!(consensus(&key(b"System", b"Events")));
        let storage = Storage::default();
        let before = [(vec![1], Some(vec![2]))].into_iter().collect();
        let after = [
            (vec![1], Some(vec![2])),
            (vec![3], Some(vec![4])),
            (vec![5], None),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            differences(&before, &after, &storage),
            [(vec![3], Some(vec![4]))].into_iter().collect()
        );
    }
    #[test]
    fn setup_refuses_occupied_synthetic_owner() {
        let mut storage = Storage::default();
        storage.top.insert(
            frame_system::Account::<Runtime>::hashed_key_for(id(b"old-owner")),
            vec![0],
        );
        assert!(setup(&storage).is_err());
    }
}
