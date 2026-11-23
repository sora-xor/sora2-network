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

/// This alternative is reachable only from the explicit signed public fixture schema.
pub fn validate_funded(base: &Path, config: &Config, policy: &RecoveryPolicy) -> Result<()> {
    #[cfg(not(feature = "bridge-fixture"))]
    {
        let _ = (base, config, policy);
        Err("funded fixture feature is absent".into())
    }
    #[cfg(feature = "bridge-fixture")]
    {
        use crate::funded_staking_fixture as funding;
        funded_owner(policy)?;
        let evidence = replay::artifact(base, &config.provenance, 64 * 1024)?;
        let evidence: serde_json::Value =
            serde_json::from_slice(&evidence).map_err(|e| e.to_string())?;
        let peers: Vec<_> = pairs()
            .iter()
            .map(|(a, _)| format!("0x{}", hex::encode(a.as_ref() as &[u8])))
            .collect();
        if config.scheme != SCHEME
            || evidence["schema"] != funding::PROVENANCE_SCHEMA
            || evidence["fixtureOnly"] != true
            || evidence["productionReady"] != false
            || evidence["peerScheme"] != SCHEME
            || evidence["publicPeers"] != serde_json::json!(peers)
            || evidence["policyCommitment"] != format!("0x{}", hex::encode(policy.commitment()))
            || evidence["checkpointHash"]
                != format!("0x{}", hex::encode(common::recovery::CHECKPOINT_HASH))
            || evidence["localCredit"] != funding::CREDIT.to_string()
            || evidence["creditCount"] != funding::CREDIT_COUNT
            || evidence["contract"] != format!("{:#x}", funding::contract())
            || evidence["token"] != format!("{:#x}", funding::token())
            || evidence["asset"] != format!("0x{}", hex::encode(common::XOR.encode()))
            || evidence["network"]
                .as_u64()
                .is_none_or(|n| n >= u64::from(u32::MAX))
        {
            return Err("funded HASHI provenance or exact public binding differs".into());
        }
        Ok(())
    }
}
#[cfg(feature = "bridge-fixture")]
fn funded_owner(policy: &RecoveryPolicy) -> Result<AccountId32> {
    let public_signers: Vec<_> = super::fixture::pairs()
        .into_iter()
        .map(|(a, _)| a)
        .collect();
    if policy.replacements.len() != 1
        || !public_signers.contains(&policy.replacements[0].new)
        || public_signers.contains(&policy.replacements[0].old)
        || pairs()
            .iter()
            .any(|(a, _)| a == &policy.replacements[0].old)
    {
        return Err("funded HASHI requires one exact published replacement signer".into());
    }
    Ok(policy.replacements[0].old.clone())
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
    prepare_owner(
        ext,
        policy,
        version,
        executing_code,
        template,
        headers,
        &owner(b"owner-old"),
    )
}

#[cfg(feature = "bridge-fixture")]
pub fn prepare_funded(
    ext: &mut sp_io::TestExternalities,
    policy: &RecoveryPolicy,
    version: &sp_version::RuntimeVersion,
    executing_code: &[u8],
    template: &[u8],
    headers: &std::collections::BTreeMap<u32, super::Header>,
) -> Result<Prepared> {
    use crate::funded_staking_fixture as funding;
    use codec::{Decode, DecodeWithMemLimit};
    use framenode_runtime::{
        incident_recovery_incoming::{IncomingChannel, IncomingOperation},
        Runtime,
    };
    let old = funded_owner(policy)?;
    let envelope = common::recovery::RecoveryInherent::decode_remark(template)
        .map_err(|e| format!("funding envelope: {e:?}"))?;
    let common::recovery::RecoveryAction::Incoming(bytes) = envelope.action else {
        return Err("funding requires incoming".into());
    };
    let mut cursor = bytes.as_slice();
    let operation = IncomingOperation::decode_with_mem_limit(
        &mut cursor,
        common::recovery::MAX_CHUNK_BYTES * 4,
    )
    .map_err(|e| e.to_string())?;
    let IncomingChannel::HashiAuthenticatedSource {
        network,
        request,
        raw_transaction,
        expected_chain_id,
        ..
    } = &operation.channel
    else {
        return Err("funding requires authenticated source transaction".into());
    };
    let mut encoded = request.as_slice();
    let eth_bridge::requests::IncomingRequest::<Runtime>::Transfer(transfer) =
        eth_bridge::requests::IncomingRequest::<Runtime>::decode(&mut encoded)
            .map_err(|e| e.to_string())?
    else {
        return Err("funding transfer missing".into());
    };
    let valid = ext.execute_with(|| {
        *network != framenode_runtime::GetEthNetworkId::get()
            && framenode_runtime::EthBridge::bridge_contract_address(network) == funding::contract()
            && framenode_runtime::EthBridge::registered_asset(network, common::XOR)
                == Some(eth_bridge::requests::AssetKind::Sidechain)
            && framenode_runtime::EthBridge::registered_sidechain_token(network, common::XOR)
                == Some(funding::token())
    });
    if !cursor.is_empty()
        || operation.encode() != bytes
        || !encoded.is_empty()
        || !valid
        || transfer.amount != funding::CREDIT
        || transfer.asset_id != common::XOR
        || transfer.asset_kind != eth_bridge::requests::AssetKind::Sidechain
        || transfer.from != sp_core::H160::zero()
        || transfer.should_take_fee
        || *expected_chain_id != Some(sp_core::U256::one())
        || !funding::public_raw_transactions().contains(raw_transaction)
    {
        return Err("funding must use exact native non-Ethereum XOR deposit/public source".into());
    }
    let offset = funding::public_raw_transactions()
        .iter()
        .position(|raw| raw == raw_transaction)
        .ok_or("funding source transaction index missing")?;
    let number = ext.execute_with(frame_system::Pallet::<Runtime>::block_number);
    validate_funded_timing(&operation, offset, number)?;
    prepare_owner(
        ext,
        policy,
        version,
        executing_code,
        template,
        headers,
        &old,
    )
}

#[cfg(feature = "bridge-fixture")]
fn validate_funded_timing(
    operation: &framenode_runtime::incident_recovery_incoming::IncomingOperation,
    offset: usize,
    number: u32,
) -> Result<()> {
    use codec::{Compact, Decode};
    use framenode_runtime::incident_recovery_incoming::IncomingChannel;
    use sp_runtime::generic::Era;
    if offset >= crate::funded_staking_fixture::CREDIT_COUNT as usize {
        return Err("funding offset differs".into());
    }
    let base = operation
        .index
        .checked_sub(offset as u32 + 1)
        .ok_or("funding base cursor underflow")?;
    let birth = crate::funded_staking_fixture::funding_birth(base as usize)?;
    if number
        != birth
            .checked_add(1 + offset as u32)
            .ok_or("funding block overflow")?
    {
        return Err(
            "funding execution differs from committed linear setup/deposit schedule".into(),
        );
    }
    let IncomingChannel::HashiAuthenticatedSource { approvals, .. } = &operation.channel else {
        return Err("funding channel differs".into());
    };
    for approval in approvals {
        let mut input = approval.extra.as_slice();
        let era = Era::decode(&mut input).map_err(|e| e.to_string())?;
        let nonce = Compact::<u32>::decode(&mut input).map_err(|e| e.to_string())?;
        let tip = Compact::<u128>::decode(&mut input).map_err(|e| e.to_string())?;
        if !input.is_empty()
            || (era, nonce, tip).encode() != approval.extra
            || era != Era::mortal(64, u64::from(birth))
            || era.birth(u64::from(number)) != u64::from(birth)
            || nonce.0 != offset as u32
            || tip.0 != 0
        {
            return Err(
                "funding approval does not commit to actual setup birth and deposit nonce".into(),
            );
        }
    }
    Ok(())
}

#[cfg(feature = "bridge-fixture")]
#[allow(clippy::too_many_arguments)]
fn prepare_owner(
    ext: &mut sp_io::TestExternalities,
    policy: &RecoveryPolicy,
    version: &sp_version::RuntimeVersion,
    executing_code: &[u8],
    template: &[u8],
    headers: &std::collections::BTreeMap<u32, super::Header>,
    expected_owner: &AccountId32,
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
        || &transfer.author != expected_owner
        || &transfer.to != expected_owner
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
    #[cfg(feature = "bridge-fixture")]
    #[test]
    fn complete_funding_schedule_rejects_delay_old_birth_and_mutated_nonce() {
        use codec::Compact;
        use common::recovery;
        use framenode_runtime::incident_recovery_incoming::{
            HashiPeerApproval, IncomingChannel, IncomingOperation,
        };
        use sp_runtime::{generic::Era, MultiSignature};
        let birth = crate::funded_staking_fixture::funding_birth(739).unwrap();
        for offset in 0..2usize {
            let extra = (
                Era::mortal(64, u64::from(birth)),
                Compact(offset as u32),
                Compact(0u128),
            )
                .encode();
            let mut operation = IncomingOperation {
                index: 740 + offset as u32,
                channel: IncomingChannel::HashiAuthenticatedSource {
                    network: 1,
                    request: vec![],
                    raw_transaction: vec![],
                    expected_chain_id: None,
                    approvals: pairs()
                        .into_iter()
                        .take(3)
                        .map(|(signer, _)| HashiPeerApproval {
                            signer,
                            call: vec![],
                            extra: extra.clone(),
                            signature: MultiSignature::Ed25519(ed25519::Signature::from_raw(
                                [0; 64],
                            )),
                        })
                        .collect(),
                },
                registries: vec![],
                finalized_source_evidence: [0; 32],
            };
            let number = birth + 1 + offset as u32;
            assert!(validate_funded_timing(&operation, offset, number).is_ok());
            assert!(validate_funded_timing(&operation, offset, number + 1).is_err());
            assert!(validate_funded_timing(&operation, 2, number).is_err());
            let IncomingChannel::HashiAuthenticatedSource { approvals, .. } =
                &mut operation.channel
            else {
                unreachable!()
            };
            approvals[0].extra = (
                Era::mortal(64, u64::from(recovery::CHECKPOINT_NUMBER + 1)),
                Compact(offset as u32),
                Compact(0u128),
            )
                .encode();
            assert!(validate_funded_timing(&operation, offset, number).is_err());
            let IncomingChannel::HashiAuthenticatedSource { approvals, .. } =
                &mut operation.channel
            else {
                unreachable!()
            };
            approvals[0].extra = (
                Era::mortal(64, u64::from(birth)),
                Compact(offset as u32 + 1),
                Compact(0u128),
            )
                .encode();
            assert!(validate_funded_timing(&operation, offset, number).is_err());
            let IncomingChannel::HashiAuthenticatedSource { approvals, .. } =
                &mut operation.channel
            else {
                unreachable!()
            };
            approvals[0].extra = extra.clone();
            approvals[0].extra.push(0);
            assert!(validate_funded_timing(&operation, offset, number).is_err());
        }
    }
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

    #[cfg(feature = "bridge-fixture")]
    #[test]
    fn funded_owner_is_public_replacement_not_council_and_rejects_public_old_keys() {
        let signers = super::super::fixture::pairs();
        let old = AccountId32::new([91; 32]);
        let mut policy = RecoveryPolicy {
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: common::recovery::CHECKPOINT_HASH,
            checkpoint_state_root: common::recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![common::recovery::AccountReplacement {
                old: old.clone(),
                new: signers[0].0.clone(),
            }],
            council: vec![AccountId32::new([31; 32]), AccountId32::new([32; 32])],
            chunk_hashes: vec![[7; 32]],
            audit_chunk_hashes: vec![[8; 32]],
            evidence_commitment: [9; 32],
            terminal_audit_commitment: [10; 32],
        };
        assert_eq!(funded_owner(&policy).unwrap(), old);
        let baseline = policy.clone();
        policy.replacements[0].new = AccountId32::new([55; 32]);
        assert!(funded_owner(&policy).is_err());
        for public_old in signers
            .into_iter()
            .map(|(a, _)| a)
            .chain(pairs().into_iter().map(|(a, _)| a))
        {
            policy = baseline.clone();
            policy.replacements[0].old = public_old;
            assert!(funded_owner(&policy).is_err());
        }
        policy = baseline.clone();
        policy.replacements.clear();
        assert!(funded_owner(&policy).is_err());
        policy = baseline;
        policy.replacements.push(policy.replacements[0].clone());
        assert!(funded_owner(&policy).is_err());
    }
}
