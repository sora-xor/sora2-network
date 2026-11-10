//! Bounded real-replay bridge state equations. No imported certificate or state writes.
use super::{replay, storage_key, value, BlockWitness, Result};
use bridge_types::{GenericNetworkId, SubNetworkId};
use codec::{DecodeAll, DecodeWithMemLimit, Encode};
use common::recovery::{self, RecoveryAction, RecoveryPhase, RecoveryPolicy};
use framenode_runtime::incident_recovery_incoming::{IncomingChannel, IncomingOperation};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

type Rows = BTreeMap<Vec<u8>, Vec<u8>>;
const SUB_IN: &str = "SubstrateBridgeInboundChannel.ChannelNonces";
const GENERIC_IN: &str = "BridgeInboundChannel.ChannelNonces";
const REPORTED: &str = "BridgeInboundChannel.ReportedChannelNonces";
const ACK: &str = "SubstrateBridgeApp.DeferredTransferResults";
const PEER_ACK: &str = ":sora-nexus:recovery:peer-ack:v1:";
const UNCHANGED_NONCES: &[&str] = &[
    "SubstrateBridgeOutboundChannel.ChannelNonces",
    "BridgeOutboundChannel.ChannelNonces",
    REPORTED,
];
const COMMITMENTS: &[&str] = &[
    "SubstrateBridgeOutboundChannel.LatestCommitment",
    "BridgeOutboundChannel.LatestCommitment",
];
const MAX_ROWS: usize = 65_536;
const MAX_BYTES: usize = 64 * 1024 * 1024;

fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(why.into())
    }
}
fn prefix(name: &str) -> Vec<u8> {
    if name == PEER_ACK {
        return PEER_ACK.as_bytes().to_vec();
    }
    let (pallet, item) = name
        .split_once('.')
        .expect("fixed bridge storage descriptor");
    storage_key(pallet.as_bytes(), item.as_bytes())
}
fn decode<T: DecodeAll + Encode>(raw: &[u8]) -> Result<T> {
    let mut input = raw;
    let v = T::decode_all(&mut input).map_err(|e| e.to_string())?;
    need(v.encode() == raw, "noncanonical bridge storage encoding")?;
    Ok(v)
}
fn network_key(name: &str, key: &[u8]) -> Result<()> {
    if name.starts_with("Substrate") {
        let _: SubNetworkId = decode(key)?;
    } else {
        let _: GenericNetworkId = decode(key)?;
    }
    Ok(())
}
fn scan(ext: &mut sp_io::TestExternalities, name: &str) -> Result<Rows> {
    let prefix = prefix(name);
    ext.execute_with(|| {
        let mut rows = Rows::new();
        let mut bytes = 0usize;
        let mut next = if sp_io::storage::exists(&prefix) {
            Some(prefix.clone())
        } else {
            sp_io::storage::next_key(&prefix)
        };
        while let Some(key) = next.filter(|key| key.starts_with(&prefix)) {
            let len =
                sp_io::storage::read(&key, &mut [], 0).ok_or("bridge row disappeared")? as usize;
            need(
                len <= recovery::MAX_CHUNK_BYTES && key.len() <= 256,
                "bridge row bound",
            )?;
            bytes = bytes
                .checked_add(key.len())
                .and_then(|n| n.checked_add(len))
                .ok_or("bridge scan overflow")?;
            need(
                rows.len() < MAX_ROWS && bytes <= MAX_BYTES,
                "bridge prefix scan bound",
            )?;
            let raw = sp_io::storage::get(&key)
                .ok_or("bridge row disappeared")?
                .to_vec();
            need(raw.len() == len, "bridge row length changed")?;
            let suffix = &key[prefix.len()..];
            if name == ACK {
                let _: (SubNetworkId, bridge_types::H256) = decode(suffix)?;
                let _: bridge_types::types::MessageStatus = decode(&raw)?;
            } else if name == PEER_ACK {
                let _: (SubNetworkId, bridge_types::H256) = decode(suffix)?;
                let _: bridge_types::substrate::DataSignerCall = decode(&raw)?;
            } else {
                network_key(name, suffix)?;
                if name.ends_with("Nonces") {
                    let _: u64 = decode(&raw)?;
                }
            }
            rows.insert(key.clone(), raw);
            next = sp_io::storage::next_key(&key);
            need(
                next.as_ref().is_none_or(|new| new > &key),
                "nonmonotonic bridge scan",
            )?;
        }
        Ok(rows)
    })
}
fn advance(rows: &mut Rows, name: &str, network: &[u8], nonce: u64) -> Result<()> {
    network_key(name, network)?;
    let key = [prefix(name), network.to_vec()].concat();
    let before = rows
        .get(&key)
        .map(|raw| decode::<u64>(raw))
        .transpose()?
        .unwrap_or(0);
    need(
        before.checked_add(1) == Some(nonce),
        "authenticated incoming nonce gap or duplicate",
    )?;
    rows.insert(key, nonce.encode());
    Ok(())
}
fn digest(rows: &Rows) -> String {
    hex::encode(Sha256::digest(rows.encode()))
}

fn halted_prefixes(
    ext: &mut sp_io::TestExternalities,
    checkpoint: &BTreeMap<String, Rows>,
) -> Result<()> {
    need(
        value(ext, recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() == Some(&[1])
            && recovery::TRANSIENT_CONTEXT_KEYS
                .iter()
                .all(|key| value(ext, key).is_none()),
        "bridge halt/transient context differs",
    )?;
    for name in UNCHANGED_NONCES {
        need(
            scan(ext, name)? == checkpoint[*name],
            "outbound/reported nonce changed while halted",
        )?;
    }
    for name in COMMITMENTS {
        // Explicit cancellation may remove a retained commitment; it may not add/replace one.
        need(
            scan(ext, name)?
                .iter()
                .all(|(key, raw)| checkpoint[*name].get(key) == Some(raw)),
            "new outbound commitment while halted",
        )?;
    }
    Ok(())
}

pub(super) struct Track {
    policy: [u8; 32],
    checkpoint: BTreeMap<String, Rows>,
    incoming: BTreeMap<u32, [u8; 32]>,
    blocks: usize,
}
impl Track {
    pub fn checkpoint(ext: &mut sp_io::TestExternalities, policy: &RecoveryPolicy) -> Result<Self> {
        need(
            replay::root(ext) == sp_core::H256::from(recovery::CHECKPOINT_STATE_ROOT),
            "bridge checkpoint root differs",
        )?;
        let mut checkpoint = BTreeMap::new();
        for name in [SUB_IN, GENERIC_IN, ACK, PEER_ACK]
            .into_iter()
            .chain(UNCHANGED_NONCES.iter().copied())
            .chain(COMMITMENTS.iter().copied())
        {
            checkpoint.insert(name.to_string(), scan(ext, name)?);
        }
        need(
            checkpoint[ACK].is_empty() && checkpoint[PEER_ACK].is_empty(),
            "checkpoint already contains recovery deferred ACKs",
        )?;
        Ok(Self {
            policy: policy.commitment(),
            checkpoint,
            incoming: BTreeMap::new(),
            blocks: 0,
        })
    }
    pub fn accepted(
        &mut self,
        ext: &mut sp_io::TestExternalities,
        action: Option<&RecoveryAction>,
        witness: &BlockWitness,
    ) -> Result<()> {
        let root = replay::root(ext);
        need(
            witness.state_root == format!("{root:#x}")
                && witness.independent_import_root_matches
                && witness.disk_restart_root_matches,
            "bridge observation lacks actual restored block",
        )?;
        halted_prefixes(ext, &self.checkpoint)?;
        if let Some(RecoveryAction::Incoming(raw)) = action {
            need(
                raw.len() <= recovery::MAX_CHUNK_BYTES,
                "observed incoming bound",
            )?;
            let mut input = raw.as_slice();
            let operation =
                IncomingOperation::decode_with_mem_limit(&mut input, recovery::MAX_CHUNK_BYTES * 4)
                    .map_err(|e| e.to_string())?;
            need(
                input.is_empty() && operation.encode() == *raw,
                "observed incoming canonical encoding",
            )?;
            need(
                self.incoming
                    .insert(operation.index, operation.commitment())
                    .is_none(),
                "duplicate observed incoming operation",
            )?;
        }
        need(replay::root(ext) == root, "bridge observer mutated state")?;
        self.blocks += 1;
        Ok(())
    }
    pub fn finish(
        self,
        ext: &mut sp_io::TestExternalities,
        policy: &RecoveryPolicy,
        evidence: &crate::incoming_replay::VerifiedReplay,
        last: &BlockWitness,
    ) -> Result<Value> {
        let root = replay::root(ext);
        need(
            self.policy == policy.commitment()
                && self.blocks > 0
                && last.state_root == format!("{root:#x}")
                && last.resulting_spec == 138
                && last.phase == "Complete",
            "bridge final identity differs",
        )?;
        let state = ext
            .execute_with(recovery::storage::status)
            .map_err(|e| format!("bridge status: {e:?}"))?
            .ok_or("bridge status absent")?;
        need(
            state.phase == RecoveryPhase::Complete
                && state.policy == self.policy
                && state.failure.is_none()
                && state.operation_counter == 0
                && value(ext, recovery::INCOMING_AUDIT_PENDING_KEY) == Some(0u32.encode()),
            "bridge final phase/counter/pending differs",
        )?;
        let operations = evidence.normal_bridge_operations(policy, root, &self.incoming)?;
        let mut sub = self.checkpoint[SUB_IN].clone();
        let mut generic = self.checkpoint[GENERIC_IN].clone();
        let mut acks = Rows::new();
        let mut peer_acks = Rows::new();
        let mut deferred_controls = 0usize;
        let mut hashi = 0usize;
        for operation in operations {
            let (network, commitment) = match &operation.channel {
                IncomingChannel::Substrate {
                    network,
                    commitment,
                    ..
                } => {
                    advance(&mut sub, SUB_IN, &network.encode(), commitment.nonce())?;
                    (GenericNetworkId::Sub(*network), Some(commitment))
                }
                IncomingChannel::Generic {
                    network,
                    commitment,
                    ..
                } => {
                    advance(
                        &mut generic,
                        GENERIC_IN,
                        &network.encode(),
                        commitment.nonce(),
                    )?;
                    (*network, Some(commitment))
                }
                IncomingChannel::Hashi { network, .. }
                | IncomingChannel::HashiAuthenticatedSource { network, .. } => {
                    hashi += 1;
                    (GenericNetworkId::EVMLegacy(*network), None)
                }
            };
            if let Some(bridge_types::GenericCommitment::Sub(batch)) = commitment {
                let targets = ext
                    .execute_with(|| {
                        framenode_runtime::incident_recovery_incoming::audit_targets(
                            operation, policy,
                        )
                    })
                    .map_err(|e| format!("bridge targets: {e:?}"))?;
                let mut matched_targets = 0usize;
                for (index, message) in batch.messages.iter().enumerate() {
                    let id = bridge_types::types::MessageId::batched(
                        network,
                        GenericNetworkId::Sub(SubNetworkId::Mainnet),
                        batch.nonce,
                        index as u64,
                    )
                    .hash();
                    let call: bridge_types::substrate::BridgeCall = decode(&message.payload)?;
                    let target = targets
                        .iter()
                        .find(|target| target.network == network && target.id == id);
                    if target.is_some() {
                        matched_targets += 1;
                    }
                    if matches!(
                        call,
                        bridge_types::substrate::BridgeCall::SubstrateApp(
                            bridge_types::substrate::SubstrateAppCall::Transfer { .. }
                        )
                    ) {
                        let GenericNetworkId::Sub(subnet) = network else {
                            return Err("Substrate ACK on wrong network".into());
                        };
                        need(
                            target.is_some(),
                            "transfer has no authenticated financial target",
                        )?;
                        let key = [prefix(ACK), (subnet, id).encode()].concat();
                        need(
                            acks.insert(key, bridge_types::types::MessageStatus::Done.encode())
                                .is_none(),
                            "duplicate deferred ACK",
                        )?;
                    } else if let bridge_types::substrate::BridgeCall::MultisigVerifier(peer) = call
                    {
                        use bridge_types::substrate::{DataSignerCall, MultisigVerifierCall};
                        let GenericNetworkId::Sub(subnet) = network else {
                            return Err("peer ACK on wrong network".into());
                        };
                        let ack = match peer {
                            MultisigVerifierCall::AddPeer { peer } => {
                                DataSignerCall::AddPeer { peer }
                            }
                            MultisigVerifierCall::RemovePeer { peer } => {
                                DataSignerCall::RemovePeer { peer }
                            }
                        };
                        need(
                            peer_acks
                                .insert(
                                    multisig_verifier::recovery_peer_ack_key(subnet, id),
                                    ack.encode(),
                                )
                                .is_none(),
                            "duplicate peer ACK",
                        )?;
                    } else if target.is_none() {
                        // The opaque accepted incoming audit binds the exact known
                        // deferred control variant and immutable payload journal.
                        deferred_controls += 1;
                    }
                }
                need(
                    matched_targets == targets.len(),
                    "bridge financial target/message coverage differs",
                )?;
            }
        }
        need(
            scan(ext, SUB_IN)? == sub && scan(ext, GENERIC_IN)? == generic,
            "final inbound nonce prefix differs from exact authenticated sequence",
        )?;
        need(
            scan(ext, ACK)? == acks && scan(ext, PEER_ACK)? == peer_acks,
            "deferred ACK prefix is incomplete, duplicated or contains an unapproved outcome",
        )?;
        need(
            replay::root(ext) == root,
            "bridge final checker mutated state",
        )?;
        Ok(
            json!({"schema":"sora-nexus-real-bridge-state-equations-v1","fixtureOnly":false,
            "policyCommitment":hex::encode(self.policy),"finalStateRoot":format!("{root:#x}"),"finalBlockHash":last.hash,
            "observedBlocks":self.blocks,"authenticatedIncomingOperations":self.incoming.len(),"hashiOperations":hashi,"deferredAcknowledgements":acks.len(),"deferredPeerAcknowledgements":peer_acks.len(),"deferredControlMessages":deferred_controls,
            "deferredControlExecutionVerified":false,"registrationPrerequisitesReconciled":false,
            "exactInboundNonceSequenceVerified":true,"outboundAndReportedNoncesUnchanged":true,"noNewOutboundCommitments":true,
            "haltRetainedAtEveryObservedBlock":true,"transientContextsAbsent":true,"pendingIncomingAudits":0,
            "opaqueAcceptedAuditFinalStateVerified":true,"deferredAcknowledgementsExactlyMatchAuthenticatedMessages":true,"readOnlyStateRootMatches":true,
            "substrateNonceRowsSha256":digest(&sub),"genericNonceRowsSha256":digest(&generic),"deferredAckRowsSha256":digest(&acks),"deferredPeerAckRowsSha256":digest(&peer_acks),
            "outboundQueueDispositionVerified":false,"workerSigningSuppressionExecuted":false,"counterpartReconciliationVerified":false,
            "externalFinalityVerified":false,"bridgeHaltNonceAndAcknowledgementsVerified":false,"productionReady":false}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn halt_transients_outbound_counters_and_new_commitments_are_enforced() {
        let mut ext = sp_io::TestExternalities::default();
        let checkpoint = UNCHANGED_NONCES
            .iter()
            .chain(COMMITMENTS)
            .map(|name| (name.to_string(), Rows::new()))
            .collect();
        assert!(halted_prefixes(&mut ext, &checkpoint).is_err());
        ext.execute_with(|| sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[1]));
        halted_prefixes(&mut ext, &checkpoint).unwrap();
        for transient in recovery::TRANSIENT_CONTEXT_KEYS {
            ext.execute_with(|| sp_io::storage::set(transient, &[0]));
            assert!(halted_prefixes(&mut ext, &checkpoint).is_err());
            ext.execute_with(|| sp_io::storage::clear(transient));
        }
        for name in UNCHANGED_NONCES.iter().chain(COMMITMENTS) {
            let network = if name.starts_with("Substrate") {
                SubNetworkId::Liberland.encode()
            } else {
                GenericNetworkId::Sub(SubNetworkId::Liberland).encode()
            };
            let key = [prefix(name), network].concat();
            ext.execute_with(|| sp_io::storage::set(&key, &1u64.encode()));
            assert!(halted_prefixes(&mut ext, &checkpoint).is_err());
            ext.execute_with(|| sp_io::storage::clear(&key));
        }
        let before = replay::root(&mut ext);
        halted_prefixes(&mut ext, &checkpoint).unwrap();
        assert_eq!(replay::root(&mut ext), before);
    }
    #[test]
    fn nonce_sequence_never_skips_duplicates_or_overflows() {
        let network = SubNetworkId::Liberland.encode();
        let mut rows = Rows::new();
        assert!(advance(&mut rows, SUB_IN, &network, 2).is_err());
        advance(&mut rows, SUB_IN, &network, 1).unwrap();
        assert!(advance(&mut rows, SUB_IN, &network, 1).is_err());
        advance(&mut rows, SUB_IN, &network, 2).unwrap();
        rows.insert(
            [prefix(SUB_IN), network.clone()].concat(),
            u64::MAX.encode(),
        );
        assert!(advance(&mut rows, SUB_IN, &network, 0).is_err());
    }
    #[test]
    fn full_prefix_scan_rejects_malformed_nonce_keys_and_values() {
        let mut ext = sp_io::TestExternalities::default();
        let key = [prefix(SUB_IN), SubNetworkId::Liberland.encode()].concat();
        ext.execute_with(|| sp_io::storage::set(&key, &7u64.encode()));
        assert_eq!(scan(&mut ext, SUB_IN).unwrap().len(), 1);
        ext.execute_with(|| sp_io::storage::set(&key, &[7]));
        assert!(scan(&mut ext, SUB_IN).is_err());
        ext.execute_with(|| {
            sp_io::storage::clear(&key);
            sp_io::storage::set(&prefix(SUB_IN), &7u64.encode());
        });
        assert!(scan(&mut ext, SUB_IN).is_err());
    }
    #[test]
    fn extra_or_failed_ack_cannot_equal_the_authenticated_done_set() {
        let key = [
            prefix(ACK),
            (SubNetworkId::Liberland, bridge_types::H256::repeat_byte(3)).encode(),
        ]
        .concat();
        let expected = Rows::from([(
            key.clone(),
            bridge_types::types::MessageStatus::Done.encode(),
        )]);
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            sp_io::storage::set(&key, &bridge_types::types::MessageStatus::Failed.encode())
        });
        assert_ne!(scan(&mut ext, ACK).unwrap(), expected);
        ext.execute_with(|| {
            sp_io::storage::set(&key, &bridge_types::types::MessageStatus::Done.encode())
        });
        assert_eq!(scan(&mut ext, ACK).unwrap(), expected);
        let extra = [
            prefix(ACK),
            (SubNetworkId::Liberland, bridge_types::H256::repeat_byte(4)).encode(),
        ]
        .concat();
        ext.execute_with(|| {
            sp_io::storage::set(&extra, &bridge_types::types::MessageStatus::Done.encode())
        });
        assert_ne!(scan(&mut ext, ACK).unwrap(), expected);
    }
}
