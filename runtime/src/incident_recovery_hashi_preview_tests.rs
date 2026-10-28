//! Native-only public HASHI preview. No policy or signing override is compiled
//! into a node or Wasm. Public fixture keys prove no production authorization.
use crate::incident_recovery_preview_audits as preview_audits;

#[path = "incident_recovery_hashi_preview_codec.rs"]
mod wire;
use crate::{incident_recovery_incoming, AccountId, Runtime, System};
use codec::{Decode, DecodeAll, DecodeLimit, DecodeWithMemLimit, Encode};
use common::{recovery, AssetInfoProvider};
use sp_core::{
    ed25519,
    storage::{StateVersion, Storage},
    Pair, H256,
};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::Path,
};

const PEER_SCHEME: &str = "sora-nexus/PUBLIC-FIXTURE-ONLY/hashi-peer/v1/";
const OWNER_DOMAIN: &[u8] = b"sora-nexus/PUBLIC-FIXTURE-ONLY/hashi-bridge/v1/";

fn owner(label: &[u8]) -> AccountId {
    AccountId::new(sp_io::hashing::blake2_256(&[OWNER_DOMAIN, label].concat()))
}
fn prefix(pallet: &[u8], item: &[u8]) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(pallet),
        sp_io::hashing::twox_128(item),
    ]
    .concat()
}
fn consensus(key: &[u8]) -> bool {
    key == b":extrinsic_index"
        || (!key.starts_with(&prefix(b"System", b"Account"))
            && [
                b"System".as_slice(),
                b"Babe",
                b"Grandpa",
                b"Session",
                b"Timestamp",
            ]
            .iter()
            .any(|p| key.starts_with(&sp_io::hashing::twox_128(p))))
}
fn read(ext: &mut sp_io::TestExternalities, key: &[u8]) -> Option<Vec<u8>> {
    ext.execute_with(|| sp_io::storage::get(key).map(|value| value.to_vec()))
}
fn observed(ext: &mut sp_io::TestExternalities) -> wire::Values {
    let keys: Vec<_> = ext
        .overlayed_changes()
        .changes()
        .map(|(key, _)| key.clone())
        .filter(|key| !consensus(key))
        .collect();
    keys.into_iter()
        .map(|key| {
            let value = read(ext, &key);
            (key, value)
        })
        .collect()
}
fn decode_operation(bytes: &[u8]) -> Result<incident_recovery_incoming::IncomingOperation, String> {
    if bytes.len() > recovery::MAX_CHUNK_BYTES {
        return Err("HASHI fixture operation bound".into());
    }
    let mut input = bytes;
    let operation = incident_recovery_incoming::IncomingOperation::decode_with_mem_limit(
        &mut input,
        recovery::MAX_CHUNK_BYTES * 3,
    )
    .map_err(|e| e.to_string())?;
    if !input.is_empty() || operation.encode() != bytes {
        return Err("noncanonical HASHI operation".into());
    }
    Ok(operation)
}

fn decode_runtime_call(bytes: &[u8]) -> Result<crate::RuntimeCall, String> {
    if bytes.is_empty() || bytes.len() > recovery::MAX_CHUNK_BYTES {
        return Err("public HASHI call bound".into());
    }
    let mut input = bytes;
    let call = {
        let mut tracked = codec::MemTrackingInput::new(&mut input, recovery::MAX_CHUNK_BYTES * 4);
        crate::RuntimeCall::decode_with_depth_limit(8, &mut tracked).map_err(|e| e.to_string())?
    };
    if !input.is_empty() || call.encode() != bytes {
        return Err("noncanonical public HASHI call".into());
    }
    Ok(call)
}

/// Native preview signs against an explicitly synthetic local birth hash.
/// Exact-Wasm execution must independently sign against its actual persisted
/// post-fork header. Signature witnesses do not affect the operation commitment.
fn sign_operation(
    operation: &mut incident_recovery_incoming::IncomingOperation,
) -> Result<(), String> {
    let before = operation.commitment();
    let incident_recovery_incoming::IncomingChannel::HashiAuthenticatedSource { approvals, .. } =
        &mut operation.channel
    else {
        return Err("public HASHI exporter admits only HASHI operations".into());
    };
    if approvals.len() != 3 {
        return Err("public HASHI fixture requires three of four peers".into());
    }
    let keys: Vec<_> = (0..4)
        .map(|index| {
            ed25519::Pair::from_seed(&sp_io::hashing::sha2_256(
                format!("{PEER_SCHEME}{index}").as_bytes(),
            ))
        })
        .collect();
    let mut seen = BTreeSet::new();
    for approval in approvals {
        if !seen.insert(approval.signer.clone()) {
            return Err("duplicate public peer approval".into());
        }
        let key = keys
            .iter()
            .find(|key| AccountId::from(key.public()) == approval.signer)
            .ok_or("unrecognized public fixture peer")?;
        let mut wire_extra = approval.extra.as_slice();
        let era = sp_runtime::generic::Era::decode(&mut wire_extra).map_err(|e| e.to_string())?;
        let codec::Compact(nonce) =
            codec::Compact::<u32>::decode(&mut wire_extra).map_err(|e| e.to_string())?;
        let codec::Compact(tip) =
            codec::Compact::<u128>::decode(&mut wire_extra).map_err(|e| e.to_string())?;
        if !wire_extra.is_empty()
            || era.is_immortal()
            || tip != 0
            || era.birth(u64::from(System::block_number()))
                != u64::from(recovery::CHECKPOINT_NUMBER + 1)
            || nonce != System::account_nonce(&approval.signer)
        {
            return Err("unsupported public HASHI nonce/era/tip".into());
        }
        let extra = crate::SignedExtra::decode_all(&mut approval.extra.as_slice())
            .map_err(|e| e.to_string())?;
        let call = decode_runtime_call(&approval.call)?;
        approval.signature = sp_runtime::generic::SignedPayload::new(call, extra)
            .map_err(|e| format!("native HASHI payload: {e:?}"))?
            .using_encoded(|payload| key.sign(payload))
            .into();
    }
    if operation.commitment() != before {
        return Err("signature changed public operation commitment".into());
    }
    Ok(())
}

fn run(input: &wire::Input) -> Result<(wire::Values, Vec<(u32, wire::Values)>), String> {
    if input.tag != *b"snxhain3"
        || input.checkpoint_top.len() > 2_000_000
        || input.stable_keys.len() > 4096
        || input.incoming.len() != wire::INCOMING_COUNT as usize
        || input.setup.index != 0
        || input.restore.index as usize != input.incoming.len() + 1
        || input.old != owner(b"owner-old")
        || input.replacement != owner(b"owner-new")
        || input.mint != wire::CREDITED_COUNT
        || input.network != crate::GetEthNetworkId::get()
    {
        return Err("public HASHI fixture bounds/identity/tag".into());
    }
    let mut incoming = input
        .incoming
        .iter()
        .map(|bytes| decode_operation(bytes))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, operation) in incoming.iter().enumerate() {
        if operation.registries.encode() != incoming[0].registries.encode() {
            return Err("public source registries must be identical across all operations".into());
        }
        if operation.index as usize != index + 1 {
            return Err("public HASHI operation index".into());
        }
    }
    let storage = Storage {
        top: input.checkpoint_top.clone(),
        children_default: Default::default(),
    };
    let code = storage
        .top
        .get(b":code".as_slice())
        .ok_or("checkpoint code absent")?
        .clone();
    let mut ext =
        sp_io::TestExternalities::new_with_code_and_state(&code, storage.clone(), StateVersion::V0);
    if ext.execute_with(|| sp_io::storage::root(StateVersion::V0))
        != recovery::CHECKPOINT_STATE_ROOT
    {
        return Err("public HASHI checkpoint state root mismatch".into());
    }
    crate::incident_recovery::with_test_policy(&input.policy, || {
        let issuance = ext.execute_with(|| -> Result<u128, String> {
            if crate::Assets::free_balance(&input.asset, &input.old)
                .map_err(|e| format!("old balance {e:?}"))?
                != 0
                || crate::Assets::free_balance(&input.asset, &input.replacement)
                    .map_err(|e| format!("new balance {e:?}"))?
                    != 0
            {
                return Err("synthetic HASHI owners already funded".into());
            }
            let issuance = crate::Assets::total_issuance(&input.asset)
                .map_err(|e| format!("issuance {e:?}"))?;
            System::set_block_number(recovery::CHECKPOINT_NUMBER + 1);
            frame_system::BlockHash::<Runtime>::insert(0, H256::from(recovery::GENESIS_HASH));
            frame_system::BlockHash::<Runtime>::insert(
                recovery::CHECKPOINT_NUMBER + 1,
                H256::from(sp_io::hashing::blake2_256(
                    b"SORA Nexus PUBLIC FIXTURE ONLY: native preview synthetic birth hash",
                )),
            );
            recovery::storage::initialize(&input.policy)
                .map_err(|e| format!("initialize {e:?}"))?;
            recovery::storage::apply_chunk(&input.policy, &input.setup)
                .map_err(|e| format!("setup {e:?}"))?;
            Ok(issuance)
        })?;
        let mut prior = observed(&mut ext);
        let mut operations = Vec::new();
        let sources = wire::source_transactions();
        for operation in &mut incoming {
            ext.execute_with(|| -> Result<(), String> {
                System::set_block_number(recovery::CHECKPOINT_NUMBER + 1 + operation.index);
                sign_operation(operation)?;
                let incident_recovery_incoming::IncomingChannel::HashiAuthenticatedSource {
                    network, request, raw_transaction, expected_chain_id, ..
                } = &operation.channel
                else {
                    unreachable!()
                };
                let original = eth_bridge::requests::IncomingRequest::<Runtime>::decode_all(
                    &mut request.as_slice(),
                )
                .map_err(|e| e.to_string())?;
                let eth_bridge::requests::IncomingRequest::Transfer(transfer) = &original else {
                    return Err("fixture transfer required".into());
                };
                if *network != input.network
                    || transfer.author != input.old
                    || transfer.to != input.old
                    || transfer.asset_id != input.asset
                    || transfer.amount != 1
                    || transfer.from != bridge_types::H160::zero()
                    || raw_transaction != &sources[operation.index as usize - 1]
                    || *expected_chain_id != Some(bridge_types::U256::one())
                {
                    return Err("fixture HASHI ownership/asset differs".into());
                }
                let native = eth_bridge::requests::OffchainRequest::incoming(original.clone());
                let load = eth_bridge::requests::OffchainRequest::LoadIncoming(
                    eth_bridge::requests::LoadIncomingRequest::Transaction(
                        eth_bridge::requests::LoadIncomingTransactionRequest::new(
                            transfer.author.clone(),
                            transfer.tx_hash,
                            transfer.timepoint,
                            eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                            *network,
                        ),
                    ),
                );
                let source = incident_recovery_incoming::authenticated_hashi_source(
                    request, raw_transaction, *expected_chain_id
                ).map_err(|e| format!("public source proof: {e:?}"))?;
                let excluded = operation.index == 3;
                let registry = operation.registries.iter().find(|r| r.network == bridge_types::GenericNetworkId::EVMLegacy(*network))
                    .ok_or("missing public source registry")?;
                if source.signer != bridge_types::H160::from(if excluded { wire::EXCLUDED_SOURCE_ACTOR } else { wire::SOURCE_ACTOR })
                    || registry.accounts.contains(&bridge_types::GenericAccount::EVM(source.signer)) != excluded
                {
                    return Err("public source-actor exclusion differs".into());
                }
                if operation.index == 1 {
                    if eth_bridge::Requests::<Runtime>::get(*network, transfer.tx_hash) != Some(load.clone())
                        || eth_bridge::RequestStatuses::<Runtime>::get(*network, transfer.tx_hash) != Some(eth_bridge::requests::RequestStatus::Pending)
                        || eth_bridge::RequestSubmissionHeight::<Runtime>::get(*network, transfer.tx_hash) != recovery::CHECKPOINT_NUMBER
                        || eth_bridge::AccountRequests::<Runtime>::get(&input.replacement) != vec![(*network, transfer.tx_hash)]
                    {
                        return Err("first source fixture must retain exact Pending load and migrated index".into());
                    }
                } else if eth_bridge::Requests::<Runtime>::contains_key(*network, transfer.tx_hash)
                    || eth_bridge::RequestStatuses::<Runtime>::contains_key(*network, transfer.tx_hash)
                    || eth_bridge::AccountRequests::<Runtime>::get(&input.replacement).contains(&(*network,transfer.tx_hash))
                {
                    return Err("later public source imports must be fresh".into());
                }
                if eth_bridge::LoadToIncomingRequestHash::<Runtime>::contains_key(*network, transfer.tx_hash)
                    || eth_bridge::AccountRequests::<Runtime>::contains_key(&input.old)
                {
                    return Err("public source replay or old query index present before admission".into());
                }
                let balance_before = crate::Assets::free_balance(&input.asset, &input.replacement).map_err(|e|format!("balance {e:?}"))?;
                let issuance_before = crate::Assets::total_issuance(&input.asset).map_err(|e|format!("issuance {e:?}"))?;
                incident_recovery_incoming::apply_encoded_with_policy(
                    &input.policy,
                    &operation.encode(),
                )
                .map_err(|e| format!("incoming {}: {e:?}", operation.index))?;
                if eth_bridge::Requests::<Runtime>::get(*network, native.hash())
                    != Some(native.clone())
                    || eth_bridge::Requests::<Runtime>::get(*network, transfer.tx_hash)
                        != Some(load)
                    || eth_bridge::RequestSubmissionHeight::<Runtime>::get(
                        *network,
                        transfer.tx_hash,
                    ) != if operation.index == 1 { recovery::CHECKPOINT_NUMBER } else { System::block_number() }
                    || eth_bridge::RequestStatuses::<Runtime>::get(*network, native.hash())
                        != Some(eth_bridge::requests::RequestStatus::Done)
                    || eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(
                        *network,
                        transfer.tx_hash,
                    ) != native.hash()
                    || eth_bridge::AccountRequests::<Runtime>::contains_key(&input.old)
                    || !eth_bridge::AccountRequests::<Runtime>::get(&input.replacement)
                        .contains(&(*network, transfer.tx_hash))
                    || !eth_bridge::AccountRequests::<Runtime>::get(&input.replacement)
                        .contains(&(*network, native.hash()))
                {
                    return Err("native HASHI payload/replay/query owner mismatch".into());
                }
                let expected_credit = u128::from(!excluded);
                let index = eth_bridge::AccountRequests::<Runtime>::get(&input.replacement);
                if index.iter().filter(|id| **id == (*network, transfer.tx_hash)).count() != 1
                    || index.iter().filter(|id| **id == (*network, native.hash())).count() != 1
                    || eth_bridge::RequestsQueue::<Runtime>::get(*network).contains(&transfer.tx_hash)
                    || eth_bridge::RequestsQueue::<Runtime>::get(*network).contains(&native.hash())
                    || crate::Assets::free_balance(&input.asset,&input.replacement).map_err(|e|format!("post balance {e:?}"))? != balance_before + expected_credit
                    || crate::Assets::total_issuance(&input.asset).map_err(|e|format!("post issuance {e:?}"))? != issuance_before + expected_credit
                {
                    return Err("source fixture credit/exclusion/issuance/index consumption differs".into());
                }
                let root = sp_io::storage::root(StateVersion::V0);
                if incident_recovery_incoming::apply_encoded_with_policy(&input.policy, &operation.encode()).is_ok()
                    || sp_io::storage::root(StateVersion::V0) != root {
                    return Err("source fixture replay was accepted or mutated state".into());
                }
                Ok(())
            })?;
            let now = observed(&mut ext);
            let delta = now
                .iter()
                .filter(|(key, value)| {
                    let before = prior
                        .get(*key)
                        .cloned()
                        .unwrap_or_else(|| storage.top.get(*key).cloned());
                    &before != *value
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            operations.push((operation.index, delta));
            prior = now;
        }
        ext.execute_with(|| -> Result<(), String> {
            System::set_block_number(recovery::CHECKPOINT_NUMBER + 1 + input.restore.index);
            recovery::storage::apply_chunk(&input.policy, &input.restore)
                .map_err(|e| format!("restore {e:?}"))?;
            if crate::Assets::free_balance(&input.asset, &input.replacement)
                .map_err(|e| format!("new balance {e:?}"))?
                != input.mint
                || crate::Assets::free_balance(&input.asset, &input.old)
                    .map_err(|e| format!("old balance {e:?}"))?
                    != 0
                || crate::Assets::total_issuance(&input.asset)
                    .map_err(|e| format!("final issuance {e:?}"))?
                    != issuance
                        .checked_add(input.mint)
                        .ok_or("issuance overflow")?
                || eth_bridge::AccountRequests::<Runtime>::contains_key(&input.old)
            {
                return Err("HASHI replacement balance/issuance/index mismatch".into());
            }
            Ok(())
        })?;
        ext.execute_with(|| preview_audits::run(&input.policy, &input.audits))?;
        prior = observed(&mut ext);
        let keys: BTreeSet<_> = input
            .stable_keys
            .iter()
            .cloned()
            .chain([
                frame_system::Account::<Runtime>::hashed_key_for(&input.old),
                frame_system::Account::<Runtime>::hashed_key_for(&input.replacement),
                eth_bridge::AccountRequests::<Runtime>::hashed_key_for(&input.old),
                eth_bridge::AccountRequests::<Runtime>::hashed_key_for(&input.replacement),
            ])
            .collect();
        for key in keys {
            prior.insert(key.clone(), read(&mut ext, &key));
        }
        for key in [
            recovery::STATUS_STORAGE_KEY,
            recovery::AUDIT_CURSOR_STORAGE_KEY,
        ] {
            prior.remove(key);
        }
        Ok((prior, operations))
    })
}

#[test]
fn public_hashi_preview_rejects_nested_or_noncanonical_signing_calls() {
    let mut call = crate::RuntimeCall::System(frame_system::Call::remark {
        remark: b"public fixture".to_vec(),
    });
    assert!(decode_runtime_call(&call.encode()).is_ok());
    let mut trailing = call.encode();
    trailing.push(0);
    assert!(decode_runtime_call(&trailing).is_err());
    for _ in 0..16 {
        call = crate::RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![call] });
    }
    assert!(decode_runtime_call(&call.encode()).is_err());
}

#[test]
fn public_hashi_source_vectors_have_distinct_hashes_and_exact_actor_domain() {
    let raw = wire::source_transactions();
    let mut hashes = BTreeSet::new();
    assert_eq!(raw.len(), wire::INCOMING_COUNT as usize);
    for (index, bytes) in raw.into_iter().enumerate() {
        let hash = H256(sp_io::hashing::keccak_256(&bytes));
        assert!(hashes.insert(hash));
        let source =
            crate::incident_recovery_evm_source::authenticated_transaction_source(&bytes, hash)
                .unwrap();
        assert_eq!(
            source.signer,
            bridge_types::H160::from(if index == 2 {
                wire::EXCLUDED_SOURCE_ACTOR
            } else {
                wire::SOURCE_ACTOR
            })
        );
        assert_eq!(source.chain_id, Some(bridge_types::U256::one()));
    }
}

#[test]
#[ignore = "explicit public HASHI fixture input/output paths and pinned native test binary required"]
fn export_public_hashi_preview() {
    let input = std::env::var_os("SORA_RECOVERY_PUBLIC_HASHI_PREVIEW_INPUT")
        .expect("explicit public fixture input");
    let output = std::env::var_os("SORA_RECOVERY_PUBLIC_HASHI_PREVIEW_OUTPUT")
        .expect("explicit public fixture output");
    assert!(Path::new(&input).is_absolute() && Path::new(&output).is_absolute());
    let mut file = fs::File::open(input).unwrap();
    assert!(file.metadata().unwrap().len() <= 1024 * 1024 * 1024);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    let input_hash = sp_io::hashing::blake2_256(&bytes);
    let mut encoded = bytes.as_slice();
    let decoded =
        wire::Input::decode_with_mem_limit(&mut encoded, 3 * 1024 * 1024 * 1024usize).unwrap();
    assert!(encoded.is_empty() && decoded.encode() == bytes);
    let (values, operations) = run(&decoded).unwrap();
    let result = wire::Output {
        tag: *b"snxhao03",
        input_hash,
        values,
        operations,
    }
    .encode();
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    output.write_all(&result).unwrap();
    output.sync_all().unwrap();
}
