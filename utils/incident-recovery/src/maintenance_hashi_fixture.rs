//! Deterministic PUBLIC FIXTURE ONLY HASHI witnesses for actual recovery headers.
//! No supplied secrets, bypassed signature checks or native execution of work.

use crate::{replay, Result};
#[cfg(any(feature = "bridge-fixture", test))]
use codec::Encode;
use common::recovery::{AccountReplacement, RecoveryPolicy};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use sp_core::{ed25519, Pair};
use sp_runtime::AccountId32;
use std::path::Path;

pub const SCHEMA: &str = "sora-nexus-public-fixture-hashi-replay-v1";
pub const SCHEME: &str = "sora-nexus/PUBLIC-FIXTURE-ONLY/hashi-peer/v1/";
pub const DOMAIN: &[u8] = b"sora-nexus/PUBLIC-FIXTURE-ONLY/hashi-bridge/v1/";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub scheme: String,
    pub provenance: replay::Artifact,
}

pub fn owner(label: &[u8]) -> AccountId32 {
    AccountId32::new(sp_io::hashing::blake2_256(&[DOMAIN, label].concat()))
}

pub fn pairs() -> Vec<(AccountId32, ed25519::Pair)> {
    let mut pairs: Vec<_> = (0..4)
        .map(|index| {
            let seed: [u8; 32] = Sha256::digest(format!("{SCHEME}{index}").as_bytes()).into();
            let pair = ed25519::Pair::from_seed(&seed);
            (AccountId32::from(pair.public().0), pair)
        })
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}

pub fn validate(base: &Path, config: &Config, policy: &RecoveryPolicy) -> Result<()> {
    if !cfg!(feature = "bridge-fixture")
        || config.scheme != SCHEME
        || policy.replacements
            != [AccountReplacement {
                old: owner(b"owner-old"),
                new: owner(b"owner-new"),
            }]
    {
        return Err("HASHI rehearsal requires the compiled public fixture feature and exact public ownership fixture".into());
    }
    let evidence = replay::artifact(base, &config.provenance, 64 * 1024)?;
    let evidence: serde_json::Value =
        serde_json::from_slice(&evidence).map_err(|e| e.to_string())?;
    let peers: Vec<_> = pairs()
        .iter()
        .map(|(a, _)| format!("0x{}", hex::encode(a.as_ref() as &[u8])))
        .collect();
    if evidence["schema"] != "sora-nexus-public-fixture-hashi-inputs-v1"
        || evidence["fixtureOnly"] != true
        || evidence["productionReady"] != false
        || evidence["peerScheme"] != SCHEME
        || evidence["publicPeers"] != serde_json::json!(peers)
        || evidence["policyCommitment"] != format!("0x{}", hex::encode(policy.commitment()))
        || evidence["checkpointHash"]
            != format!("0x{}", hex::encode(common::recovery::CHECKPOINT_HASH))
    {
        return Err("HASHI fixture provenance or exact public peer binding differs".into());
    }
    Ok(())
}

#[cfg(any(feature = "bridge-fixture", test))]
fn signing_payload(
    call: &[u8],
    extra: &[u8],
    spec: u32,
    transaction: u32,
    genesis: [u8; 32],
    birth: [u8; 32],
) -> (Vec<u8>, Vec<u8>) {
    // These are the native stable wire fields of both retained134 and final138
    // SignedExtra. Actual Wasm still performs UncheckedExtrinsic::check.
    let preimage = [
        call,
        extra,
        &spec.encode(),
        &transaction.encode(),
        &genesis,
        &birth,
    ]
    .concat();
    let payload = if preimage.len() > 256 {
        sp_io::hashing::blake2_256(&preimage).to_vec()
    } else {
        preimage.clone()
    };
    (preimage, payload)
}

#[cfg(feature = "bridge-fixture")]
pub struct Prepared {
    pub envelope: Vec<u8>,
    pub negatives: Vec<(&'static str, Vec<u8>)>,
    pub context: serde_json::Value,
}

#[cfg(feature = "bridge-fixture")]
fn decode_call(bytes: &[u8]) -> Result<framenode_runtime::RuntimeCall> {
    use codec::DecodeLimit;
    use common::recovery;
    if bytes.is_empty() || bytes.len() > recovery::MAX_CHUNK_BYTES {
        return Err("HASHI fixture call exceeds bound".into());
    }
    let mut input = bytes;
    let call = {
        let mut tracked = codec::MemTrackingInput::new(&mut input, recovery::MAX_CHUNK_BYTES * 4);
        framenode_runtime::RuntimeCall::decode_with_depth_limit(8, &mut tracked)
            .map_err(|e| e.to_string())?
    };
    if !input.is_empty() || call.encode() != bytes {
        return Err("noncanonical HASHI fixture call".into());
    }
    Ok(call)
}

#[cfg(feature = "bridge-fixture")]
pub fn prepare(
    ext: &mut sp_io::TestExternalities,
    policy: &RecoveryPolicy,
    version: &sp_version::RuntimeVersion,
    executing_code: &[u8],
    template: &[u8],
    headers: &std::collections::BTreeMap<u32, super::Header>,
) -> Result<Prepared> {
    use codec::{Compact, Decode, DecodeWithMemLimit};
    use common::recovery::{self, RecoveryAction, RecoveryInherent};
    use framenode_runtime::{
        incident_recovery_incoming::{IncomingChannel, IncomingOperation},
        Runtime, RuntimeCall,
    };
    use sp_runtime::{generic::Era, traits::Header as _, MultiSignature};
    let envelope = RecoveryInherent::decode_remark(template)
        .map_err(|e| format!("HASHI fixture envelope: {e:?}"))?;
    let RecoveryAction::Incoming(bytes) = envelope.action else {
        return Err("HASHI fixture must be an incoming operation".into());
    };
    let mut input = bytes.as_slice();
    let mut operation =
        IncomingOperation::decode_with_mem_limit(&mut input, recovery::MAX_CHUNK_BYTES * 4)
            .map_err(|e| e.to_string())?;
    if !input.is_empty()
        || operation.encode() != bytes
        || policy.chunk_hashes.get(operation.index as usize) != Some(&operation.commitment())
    {
        return Err("HASHI fixture template differs from committed work".into());
    }
    let commitment = operation.commitment();
    let observed = ext
        .execute_with(recovery::storage::status)
        .map_err(|e| format!("HASHI fixture recovery status: {e:?}"))?
        .ok_or("HASHI fixture recovery status missing")?;
    let active = ext.execute_with(|| sp_io::storage::get(recovery::ACTIVE_POLICY_STORAGE_KEY));
    if observed.phase != recovery::RecoveryPhase::Applying
        || observed.next_chunk != operation.index
        || observed.policy != policy.commitment()
        || active.as_deref() != Some(policy.commitment().as_slice())
    {
        return Err("HASHI fixture phase/cursor/policy differs from persisted state".into());
    }
    let (network, request, approvals) = match &mut operation.channel {
        IncomingChannel::Hashi {
            network,
            request,
            approvals,
        } => (network, request, approvals),
        IncomingChannel::HashiAuthenticatedSource {
            network,
            request,
            raw_transaction,
            expected_chain_id,
            approvals,
        } => {
            framenode_runtime::incident_recovery_incoming::authenticated_hashi_source(
                request,
                raw_transaction,
                *expected_chain_id,
            )
            .map_err(|error| format!("HASHI fixture authenticated source: {error:?}"))?;
            (network, request, approvals)
        }
        _ => return Err("HASHI fixture requires native HASHI channel".into()),
    };
    let mut request_input = request.as_slice();
    let eth_bridge::requests::IncomingRequest::<Runtime>::Transfer(transfer) =
        eth_bridge::requests::IncomingRequest::<Runtime>::decode(&mut request_input)
            .map_err(|e| e.to_string())?
    else {
        return Err("HASHI fixture supports only transfer requests".into());
    };
    if !request_input.is_empty()
        || transfer.author != owner(b"owner-old")
        || transfer.to != owner(b"owner-old")
        || transfer.network_id != *network
        || approvals.len() != 3
    {
        return Err("HASHI fixture requires exact mapped-author/beneficiary and three public peer approvals".into());
    }
    let public = pairs();
    let expected_peers: std::collections::BTreeSet<_> =
        public.iter().map(|(a, _)| a.clone()).collect();
    let (number, genesis, bridge, peers) = ext.execute_with(|| {
        (
            frame_system::Pallet::<Runtime>::block_number(),
            frame_system::Pallet::<Runtime>::block_hash(0),
            eth_bridge::BridgeAccount::<Runtime>::get(*network),
            eth_bridge::Peers::<Runtime>::get(*network),
        )
    });
    let genesis_expected =
        crate::hex_bytes("0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5")?;
    let bridge = bridge.ok_or("HASHI fixture bridge account missing")?;
    let registered = ext.execute_with(|| bridge_multisig::Accounts::<Runtime>::get(&bridge));
    if number <= recovery::CHECKPOINT_NUMBER + 1
        || genesis.as_bytes() != genesis_expected
        || peers != expected_peers
        || registered.map(|m| m.encode())
            != Some(
                bridge_multisig::MultisigAccount::new(expected_peers.iter().cloned().collect())
                    .encode(),
            )
        || !matches!(
            (version.spec_version, version.transaction_version),
            (134, 131) | (135, 132) | (136, 132) | (137, 132)
        )
    {
        return Err("HASHI public peer or actual runtime context differs".into());
    }
    let mut context = Vec::new();
    let mut bad_birth = Vec::new();
    for (approval, (account, pair)) in approvals.iter_mut().zip(public.iter().take(3)) {
        if &approval.signer != account
            || approval.signature.encode()
                != MultiSignature::Ed25519(ed25519::Signature::from_raw([0; 64])).encode()
        {
            return Err(
                "HASHI fixture accepts only its fixed peer order and empty witnesses".into(),
            );
        }
        let call = decode_call(&approval.call)?;
        let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
            id, call: inner, ..
        }) = &call
        else {
            return Err("HASHI fixture cannot sign an arbitrary outer call".into());
        };
        let inner_call = decode_call(inner)?;
        let expected_load = eth_bridge::requests::LoadIncomingTransactionRequest::new(
            transfer.author.clone(),
            transfer.tx_hash,
            transfer.timepoint,
            eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
            *network,
        );
        if id != &bridge
            || !matches!(inner_call, RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
                load_incoming_request: eth_bridge::requests::LoadIncomingRequest::Transaction(ref load),
                incoming_request_result: Ok(eth_bridge::requests::IncomingRequest::Transfer(ref actual)),
            }) if load == &expected_load && actual == &transfer)
        {
            return Err("HASHI fixture cannot change or sign unrelated business calls".into());
        }
        let mut extra = approval.extra.as_slice();
        let era = Era::decode(&mut extra).map_err(|e| e.to_string())?;
        let Compact(nonce) = Compact::<u32>::decode(&mut extra).map_err(|e| e.to_string())?;
        let Compact(tip) = Compact::<u128>::decode(&mut extra).map_err(|e| e.to_string())?;
        let birth = era.birth(u64::from(number));
        let birth_number = u32::try_from(birth).map_err(|e| e.to_string())?;
        let header = headers
            .get(&birth_number)
            .ok_or("HASHI fixture birth header was not authored in this rehearsal")?;
        let birth_hash =
            ext.execute_with(|| frame_system::Pallet::<Runtime>::block_hash(birth_number));
        let observed_nonce =
            ext.execute_with(|| frame_system::Pallet::<Runtime>::account_nonce(account));
        let lineages: Vec<_> = header
            .digest()
            .logs
            .iter()
            .filter_map(|item| match item {
                sp_runtime::DigestItem::Other(bytes) => recovery::parse_lineage_marker(bytes),
                _ => None,
            })
            .collect();
        if !extra.is_empty()
            || (era, Compact(nonce), Compact(tip)).encode() != approval.extra
            || era.is_immortal()
            || tip != 0
            || birth <= u64::from(recovery::CHECKPOINT_NUMBER)
            || birth >= u64::from(number)
            || nonce != observed_nonce
            || birth_hash != header.hash()
            || *header.number() != birth_number
            || lineages != [policy.commitment()]
        {
            return Err("HASHI fixture nonce, era or actual recovery birth header differs".into());
        }
        let (preimage, payload) = signing_payload(
            &approval.call,
            &approval.extra,
            version.spec_version,
            version.transaction_version,
            genesis.0,
            birth_hash.0,
        );
        let (_, wrong) = signing_payload(
            &approval.call,
            &approval.extra,
            version.spec_version,
            version.transaction_version,
            genesis.0,
            recovery::CHECKPOINT_HASH,
        );
        approval.signature = MultiSignature::Ed25519(pair.sign(&payload));
        bad_birth.push(MultiSignature::Ed25519(pair.sign(&wrong)));
        context.push(serde_json::json!({"signer":format!("0x{}",hex::encode(account.as_ref() as &[u8])),
            "nonce":nonce,"era":"0x".to_owned()+&hex::encode(era.encode()),"birthBlock":birth_number,"birthHash":format!("{birth_hash:#x}"),
            "signingPreimage":"0x".to_owned()+&hex::encode(preimage),"signingPayload":"0x".to_owned()+&hex::encode(payload),
            "signature":"0x".to_owned()+&hex::encode(approval.signature.encode())}));
    }
    if operation.commitment() != commitment {
        return Err("HASHI witness generation changed business commitment".into());
    }
    let wrap = |operation: &IncomingOperation| {
        RecoveryInherent {
            format_version: 1,
            action: RecoveryAction::Incoming(operation.encode()),
        }
        .encode_remark()
    };
    let mut invalid_signature = operation.clone();
    let (IncomingChannel::Hashi { approvals, .. }
    | IncomingChannel::HashiAuthenticatedSource { approvals, .. }) = &mut invalid_signature.channel
    else {
        unreachable!()
    };
    approvals[0].signature = MultiSignature::Ed25519(ed25519::Signature::from_raw([0; 64]));
    let mut invalid_birth = operation.clone();
    let (IncomingChannel::Hashi { approvals, .. }
    | IncomingChannel::HashiAuthenticatedSource { approvals, .. }) = &mut invalid_birth.channel
    else {
        unreachable!()
    };
    for (approval, signature) in approvals.iter_mut().zip(bad_birth) {
        approval.signature = signature;
    }
    Ok(Prepared {
        envelope: wrap(&operation),
        negatives: vec![
            ("invalid-signature", wrap(&invalid_signature)),
            ("checkpoint-birth-signature", wrap(&invalid_birth)),
        ],
        context: serde_json::json!({"schema":"sora-nexus-public-fixture-hashi-authorization-v1","fixtureOnly":true,"productionReady":false,
            "executionBlock":number,"specVersion":version.spec_version,"transactionVersion":version.transaction_version,
            "executingCodeBlake2":hex::encode(sp_io::hashing::blake2_256(executing_code)),"genesisHash":format!("{genesis:#x}"),
            "policyCommitment":hex::encode(policy.commitment()),"operationCommitment":hex::encode(commitment),"approvals":context}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_hashi_payload_matches_native_golden_and_is_context_bound() {
        let golden: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../misc/incident_recovery/hashi_codec_fixture.json"
        ))
        .unwrap();
        let context = &golden["nativeContext"];
        for (i, approval) in golden["template"]["approvals"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let call = crate::hex_bytes(approval["call"].as_str().unwrap()).unwrap();
            let extra = crate::hex_bytes(approval["extra"].as_str().unwrap()).unwrap();
            let genesis = crate::hex_bytes(context["genesisHash"].as_str().unwrap())
                .unwrap()
                .try_into()
                .unwrap();
            let birth = crate::hex_bytes(context["birthHash"].as_str().unwrap())
                .unwrap()
                .try_into()
                .unwrap();
            let (_, actual) = signing_payload(
                &call,
                &extra,
                context["specVersion"].as_u64().unwrap() as u32,
                context["transactionVersion"].as_u64().unwrap() as u32,
                genesis,
                birth,
            );
            assert_eq!(
                format!("0x{}", hex::encode(actual)),
                golden["signingPayloads"][i]
            );
        }
        assert_eq!(pairs().len(), 4);
        assert!(pairs().windows(2).all(|w| w[0].0 < w[1].0));
    }
}
