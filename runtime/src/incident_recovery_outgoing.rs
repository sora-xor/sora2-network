//! Manifest-bound, permanent outgoing quarantine. No refund or balance movement.
//!
//! A tombstone stops native execution even if a later release reopens the bridge.
//! Original requests, signatures, statuses, nonces and backing remain untouched.
//! Observed amounts are evidence, never an authorization to create a refund.

use crate::{AccountId, AssetId, BridgeProxy, Runtime, System, ThisNetworkId};
use alloc::vec::Vec;
use bridge_types::{
    recovery as tombstone,
    types::{MessageDirection, MessageId},
    GenericAccount, GenericBridgeMessage, GenericCommitment, GenericNetworkId, H256,
};
use codec::{Decode, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use common::recovery::{self, RecoveryError, RecoveryPhase, RecoveryPolicy, RecoveryStatus};
use frame_support::weights::Weight;

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub struct FinancialWitness {
    pub owner: AccountId,
    pub asset: AssetId,
    pub amount: u128,
    /// Hash of the complete native BridgeProxy transaction, including its status.
    pub record_hash: [u8; 32],
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub struct MessageWitness {
    pub message_id: H256,
    /// Hash of the SCALE native message, including timestamp/gas/recipient fields.
    pub message_hash: [u8; 32],
    /// None requires absence of the native sender index. Such a record conveys no
    /// financial entitlement; it only quarantines this exact protocol message.
    pub financial: Option<FinancialWitness>,
}

#[derive(Clone, Copy, Debug, Encode, Decode, DecodeWithMemTracking)]
pub enum ChannelKind {
    Generic,
    Substrate,
}
impl ChannelKind {
    fn tag(self) -> u8 {
        match self {
            Self::Generic => 0,
            Self::Substrate => 1,
        }
    }
}

/// Historic commitment authenticated by the exact retained native signing index.
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub struct ArchivedCommitmentWitness {
    pub preimage: Vec<u8>,
    /// Hash of SCALE (PendingApprovals value, full Approvals map).
    pub signer_state_hash: [u8; 32],
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub enum OutgoingTarget {
    Hashi {
        network: u32,
        request: H256,
        /// Complete OffchainRequest and RequestStatus encoding hashes.
        request_hash: [u8; 32],
        status_hash: [u8; 32],
        author: AccountId,
        /// Transfer requests must supply exact native asset and amount; control
        /// requests must supply None. This does not prove refund eligibility.
        transfer: Option<(AssetId, u128)>,
    },
    Channel {
        channel: ChannelKind,
        network: GenericNetworkId,
        nonce: u64,
        /// None means the exact current uncommitted queue. Some requires the
        /// native latest outbound commitment with this hash and this nonce.
        commitment: Option<H256>,
        /// Needed only when the commitment is older than native LatestCommitment.
        /// Absent native pending/signature evidence cannot be replaced by a claim.
        archived: Option<ArchivedCommitmentWitness>,
        /// Every native message, in order; subsets and reordered entries fail.
        messages: Vec<MessageWitness>,
    },
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub struct OutgoingOperation {
    pub index: u32,
    pub target: OutgoingTarget,
    /// Bound review of external authorization/redemption uncertainty. A nonzero
    /// commitment is required, but this adapter never interprets it as a refund.
    pub reconciliation_evidence: [u8; 32],
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, PartialEq, Eq)]
pub enum ClaimDisposition {
    /// Contingent rights, not a new debt or refund authorization. Failed/Broken
    /// native records remain uncertain until separate external reconciliation.
    Quarantined {
        effective_owner: AccountId,
    },
    /// Only claim rights are extinguished here. Assets/collateral require a
    /// separate proved accounting operation and cannot be burned by this API.
    ExtinguishedAttacker,
    NoFinancialClaim,
}

#[derive(Encode)]
struct QuarantineRecord<'a> {
    lineage: [u8; 32],
    policy: [u8; 32],
    operation: [u8; 32],
    evidence: [u8; 32],
    index: u32,
    disposition: &'a ClaimDisposition,
    /// Always false in this adapter; future accounting cannot infer eligibility
    /// from a quarantine record or the absence of retained local signatures.
    refund_authorized: bool,
}

impl OutgoingOperation {
    pub fn commitment(&self) -> [u8; 32] {
        sp_io::hashing::blake2_256(
            &(b"sora-nexus/outgoing-quarantine/v1".as_slice(), self).encode(),
        )
    }
    pub fn required_weight(&self) -> Weight {
        let count = match &self.target {
            OutgoingTarget::Hashi { .. } => 1,
            OutgoingTarget::Channel { messages, .. } => messages.len() as u64,
        };
        Weight::from_parts(
            150_000_000u64.saturating_add(count.saturating_mul(10_000_000)),
            recovery::MAX_CHUNK_BYTES as u64,
        )
        .saturating_add(
            <Runtime as frame_system::Config>::DbWeight::get()
                .reads_writes(16 + count * 4, 4 + count),
        )
    }
}

fn disposition(policy: &RecoveryPolicy, owner: &AccountId) -> ClaimDisposition {
    if owner == &AccountId::new(recovery::ATTACKER_ACCOUNT) {
        ClaimDisposition::ExtinguishedAttacker
    } else {
        ClaimDisposition::Quarantined {
            effective_owner: policy
                .replacements
                .iter()
                .find(|pair| &pair.old == owner)
                .map_or_else(|| owner.clone(), |pair| pair.new.clone()),
        }
    }
}

type Prepared = Vec<(Vec<u8>, ClaimDisposition)>;

fn message_financial(
    policy: &RecoveryPolicy,
    network: GenericNetworkId,
    message: &MessageWitness,
) -> Result<ClaimDisposition, RecoveryError> {
    match (
        &message.financial,
        BridgeProxy::sender(network, message.message_id),
    ) {
        (None, None) => Ok(ClaimDisposition::NoFinancialClaim),
        (Some(expected), Some(owner)) if owner == expected.owner => {
            let record = BridgeProxy::transactions((network, owner.clone()), message.message_id)
                .ok_or(RecoveryError::WrongPreimage)?;
            if record.direction() != MessageDirection::Outbound
                || record.source() != &GenericAccount::Sora(owner.clone())
                || record.asset_id() != &expected.asset
                || record.amount() != expected.amount
                || sp_io::hashing::blake2_256(&record.encode()) != expected.record_hash
            {
                return Err(RecoveryError::WrongPreimage);
            }
            if matches!(
                record.status(),
                bridge_types::types::MessageStatus::Done
                    | bridge_types::types::MessageStatus::Refunded
            ) {
                Ok(ClaimDisposition::NoFinancialClaim)
            } else {
                Ok(disposition(policy, &owner))
            }
        }
        _ => Err(RecoveryError::WrongPreimage),
    }
}

fn native_messages(
    channel: ChannelKind,
    network: GenericNetworkId,
    nonce: u64,
    expected_commitment: Option<H256>,
    archived: Option<&ArchivedCommitmentWitness>,
) -> Result<Vec<[u8; 32]>, RecoveryError> {
    let hash = |message: &[u8]| sp_io::hashing::blake2_256(message);
    if let Some(expected) = expected_commitment {
        let commitment = if let Some(archived) = archived {
            let pending = crate::BridgeDataSigner::pending_approval(network, expected);
            let approvals = crate::BridgeDataSigner::approvals(network, expected);
            if (!pending && approvals.is_empty())
                || sp_io::hashing::blake2_256(&(pending, &approvals).encode())
                    != archived.signer_state_hash
            {
                return Err(RecoveryError::WrongPreimage);
            }
            let mut input = archived.preimage.as_slice();
            let decoded =
                GenericCommitment::<
                    crate::BridgeMaxMessagesPerCommit,
                    crate::BridgeMaxMessagePayloadSize,
                >::decode_with_mem_limit(&mut input, recovery::MAX_CHUNK_BYTES * 4)
                .map_err(|_| RecoveryError::WrongPreimage)?;
            if !input.is_empty() || decoded.encode() != archived.preimage {
                return Err(RecoveryError::WrongPreimage);
            }
            decoded
        } else {
            match channel {
                ChannelKind::Generic => {
                    bridge_channel::outbound::LatestCommitment::<Runtime>::get(network)
                }
                ChannelKind::Substrate => {
                    substrate_bridge_channel::outbound::LatestCommitment::<Runtime>::get(
                        network.sub().ok_or(RecoveryError::WrongPreimage)?,
                    )
                }
            }
            .ok_or(RecoveryError::WrongPreimage)?
            .commitment
        };
        if commitment.hash() != expected || commitment.nonce() != nonce {
            return Err(RecoveryError::WrongPreimage);
        }
        return match (&commitment, network) {
            (GenericCommitment::Sub(commitment), GenericNetworkId::Sub(_)) => Ok(commitment
                .messages
                .iter()
                .map(|message| hash(&message.encode()))
                .collect()),
            (
                GenericCommitment::EVM(bridge_types::evm::Commitment::Outbound(commitment)),
                GenericNetworkId::EVM(_),
            ) if matches!(channel, ChannelKind::Generic) => Ok(commitment
                .messages
                .iter()
                .map(|message| hash(&message.encode()))
                .collect()),
            _ => Err(RecoveryError::WrongPreimage),
        };
    }
    if archived.is_some() {
        return Err(RecoveryError::WrongPreimage);
    }
    match channel {
        ChannelKind::Generic => {
            if bridge_channel::outbound::ChannelNonces::<Runtime>::get(network).checked_add(1)
                != Some(nonce)
            {
                return Err(RecoveryError::WrongPreimage);
            }
            bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(network)
                .iter()
                .map(|message| match (message, network) {
                    (GenericBridgeMessage::Sub(message), GenericNetworkId::Sub(_)) => {
                        Ok(hash(&message.encode()))
                    }
                    (GenericBridgeMessage::EVM(message), GenericNetworkId::EVM(_)) => {
                        Ok(hash(&message.encode()))
                    }
                    _ => Err(RecoveryError::WrongPreimage),
                })
                .collect()
        }
        ChannelKind::Substrate => {
            let network = network.sub().ok_or(RecoveryError::WrongPreimage)?;
            if substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(network)
                .checked_add(1)
                != Some(nonce)
            {
                return Err(RecoveryError::WrongPreimage);
            }
            Ok(
                substrate_bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(network)
                    .iter()
                    .map(|message| hash(&message.encode()))
                    .collect(),
            )
        }
    }
}

fn prepare(
    policy: &RecoveryPolicy,
    operation: &OutgoingOperation,
) -> Result<Prepared, RecoveryError> {
    if operation.reconciliation_evidence == [0; 32] {
        return Err(RecoveryError::WrongPreimage);
    }
    let mut prepared = Vec::new();
    match &operation.target {
        OutgoingTarget::Hashi {
            network,
            request,
            request_hash,
            status_hash,
            author,
            transfer,
        } => {
            let native = eth_bridge::Requests::<Runtime>::get(network, request)
                .ok_or(RecoveryError::WrongPreimage)?;
            let status = eth_bridge::RequestStatuses::<Runtime>::get(network, request)
                .ok_or(RecoveryError::WrongPreimage)?;
            let (outgoing, stored_hash) =
                native.as_outgoing().ok_or(RecoveryError::WrongPreimage)?;
            if stored_hash != *request
                || native.hash()
                    != eth_bridge::requests::OffchainRequest::outgoing(outgoing.clone()).hash()
                || outgoing.network_id() != *network
                || outgoing.author() != author
                || sp_io::hashing::blake2_256(&native.encode()) != *request_hash
                || sp_io::hashing::blake2_256(&status.encode()) != *status_hash
            {
                return Err(RecoveryError::WrongPreimage);
            }
            let financial = match (outgoing, transfer) {
                (
                    eth_bridge::requests::OutgoingRequest::Transfer(native),
                    Some((asset, amount)),
                ) if native.asset_id == *asset && native.amount == *amount => {
                    disposition(policy, author)
                }
                (eth_bridge::requests::OutgoingRequest::Transfer(_), _) | (_, Some(_)) => {
                    return Err(RecoveryError::WrongPreimage)
                }
                (_, None) => ClaimDisposition::NoFinancialClaim,
            };
            let financial = if matches!(status, eth_bridge::requests::RequestStatus::Done) {
                ClaimDisposition::NoFinancialClaim
            } else {
                financial
            };
            prepared.push((tombstone::hashi_key(*network, *request), financial.clone()));
            prepared.push((
                tombstone::message_key(GenericNetworkId::EVMLegacy(*network), *request),
                financial,
            ));
        }
        OutgoingTarget::Channel {
            channel,
            network,
            nonce,
            commitment,
            archived,
            messages,
        } => {
            if messages.is_empty() || messages.len() > 256 {
                return Err(RecoveryError::ChunkBounds);
            }
            let native =
                native_messages(*channel, *network, *nonce, *commitment, archived.as_ref())?;
            if native.len() != messages.len() {
                return Err(RecoveryError::WrongPreimage);
            }
            for (index, (expected, native_hash)) in messages.iter().zip(native).enumerate() {
                let id =
                    MessageId::batched(ThisNetworkId::get(), *network, *nonce, index as u64).hash();
                if expected.message_id != id || expected.message_hash != native_hash {
                    return Err(RecoveryError::WrongPreimage);
                }
                prepared.push((
                    tombstone::message_key(*network, id),
                    message_financial(policy, *network, expected)?,
                ));
            }
            prepared.push((
                tombstone::batch_key(channel.tag(), *network, *nonce),
                ClaimDisposition::NoFinancialClaim,
            ));
            if let Some(commitment) = commitment {
                prepared.push((
                    tombstone::commitment_key(*network, *commitment),
                    ClaimDisposition::NoFinancialClaim,
                ));
            }
        }
    }
    if prepared.iter().any(|(key, _)| sp_io::storage::exists(key)) {
        return Err(RecoveryError::Replay);
    }
    Ok(prepared)
}

/// Share the exact bounded decoder between executive weight admission and work.
fn decode_operation(bytes: &[u8]) -> Result<OutgoingOperation, RecoveryError> {
    if bytes.is_empty() || bytes.len() > recovery::MAX_CHUNK_BYTES {
        return Err(RecoveryError::ChunkBounds);
    }
    let mut input = bytes;
    let operation =
        OutgoingOperation::decode_with_mem_limit(&mut input, recovery::MAX_CHUNK_BYTES * 4)
            .map_err(|_| RecoveryError::WrongPreimage)?;
    if !input.is_empty() {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(operation)
}

/// No storage writes. Accepted committed failures must pay this same weight.
pub fn outgoing_encoded_weight(bytes: &[u8]) -> Result<Weight, RecoveryError> {
    Ok(decode_operation(bytes)?.required_weight())
}

pub fn apply_encoded_with_policy(
    policy: &RecoveryPolicy,
    bytes: &[u8],
) -> Result<(RecoveryStatus, Weight), RecoveryError> {
    apply_operation(policy, &decode_operation(bytes)?)
}

fn apply_operation(
    policy: &RecoveryPolicy,
    operation: &OutgoingOperation,
) -> Result<(RecoveryStatus, Weight), RecoveryError> {
    policy.validate()?;
    let mut status = recovery::storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
    if sp_io::storage::get(recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
        != Some(policy.commitment().as_slice())
        || sp_io::storage::get(recovery::POLICY_STORAGE_KEY).as_deref()
            != Some(status.policy.as_slice())
    {
        return Err(RecoveryError::WrongPolicy);
    }
    if status.phase != RecoveryPhase::Applying
        || System::block_number() <= recovery::CHECKPOINT_NUMBER
        || !sp_io::storage::exists(recovery::BRIDGE_HALT_STORAGE_KEY)
    {
        return Err(RecoveryError::WrongPhase);
    }
    if status.next_chunk != operation.index
        || policy.chunk_hashes.get(operation.index as usize) != Some(&operation.commitment())
    {
        return Err(RecoveryError::WrongChunk);
    }
    // The template is now authenticated by the active work policy. A native
    // preimage mismatch freezes the cursor without changing bridge state, so the
    // separately authenticated escape remains executable in a valid next block.
    let prepared = match prepare(policy, operation) {
        Ok(prepared) => prepared,
        Err(error) => {
            status.phase = RecoveryPhase::Failed;
            status.failure = Some(error);
            sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &status.encode());
            return Err(error);
        }
    };
    let weight = operation.required_weight();
    if !weight.all_lte(<Runtime as frame_system::Config>::BlockWeights::get().max_block) {
        return Err(RecoveryError::ChunkBounds);
    }
    frame_support::storage::transactional::with_transaction_opaque_err(|| {
        for (key, disposition) in &prepared {
            sp_io::storage::set(
                key,
                &QuarantineRecord {
                    lineage: status.policy,
                    policy: policy.commitment(),
                    operation: operation.commitment(),
                    evidence: operation.reconciliation_evidence,
                    index: operation.index,
                    disposition,
                    refund_authorized: false,
                }
                .encode(),
            );
        }
        status.next_chunk += 1;
        if status.next_chunk as usize == policy.chunk_hashes.len() {
            status.phase = RecoveryPhase::Auditing;
        }
        sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &status.encode());
        frame_support::storage::TransactionOutcome::Commit(Ok::<_, RecoveryError>(()))
    })
    .map_err(|_| RecoveryError::WrongPhase)??;
    Ok((status, weight))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bridge_types::{
        traits::{MessageStatusNotifier, OutboundChannel},
        types::MessageStatus,
        SubNetworkId,
    };
    use common::recovery::AccountReplacement;
    use frame_support::assert_ok;

    #[test]
    fn recovery_outgoing_weight_uses_the_execution_decoder() {
        let operation = OutgoingOperation {
            index: 0,
            target: OutgoingTarget::Hashi {
                network: 0,
                request: H256::repeat_byte(1),
                request_hash: [2; 32],
                status_hash: [3; 32],
                author: AccountId::new([7; 32]),
                transfer: None,
            },
            reconciliation_evidence: [4; 32],
        };
        let mut bytes = operation.encode();
        assert_eq!(
            outgoing_encoded_weight(&bytes),
            Ok(operation.required_weight())
        );
        bytes.push(0);
        assert_eq!(
            outgoing_encoded_weight(&bytes),
            Err(RecoveryError::WrongPreimage)
        );
        assert_eq!(
            outgoing_encoded_weight(&[]),
            Err(RecoveryError::ChunkBounds)
        );
        assert_eq!(
            outgoing_encoded_weight(&vec![0; recovery::MAX_CHUNK_BYTES + 1]),
            Err(RecoveryError::ChunkBounds)
        );
    }

    fn policy(operation: &OutgoingOperation) -> RecoveryPolicy {
        RecoveryPolicy {
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![AccountReplacement {
                old: AccountId::new([7; 32]),
                new: AccountId::new([8; 32]),
            }],
            council: vec![AccountId::new([5; 32])],
            chunk_hashes: vec![operation.commitment()],
            audit_chunk_hashes: vec![[4; 32]],
            evidence_commitment: [5; 32],
            terminal_audit_commitment: [6; 32],
        }
    }

    fn initialize(policy: &RecoveryPolicy) {
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 1);
        sp_io::storage::set(recovery::POLICY_STORAGE_KEY, &policy.commitment());
        sp_io::storage::set(recovery::ACTIVE_POLICY_STORAGE_KEY, &policy.commitment());
        sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
        sp_io::storage::set(
            recovery::STATUS_STORAGE_KEY,
            &RecoveryStatus {
                format_version: 1,
                policy: policy.commitment(),
                phase: RecoveryPhase::Applying,
                next_chunk: 0,
                operation_counter: 0,
                failure: None,
            }
            .encode(),
        );
    }

    fn queue_fixture() -> OutgoingOperation {
        let network = SubNetworkId::Liberland;
        let mut messages = Vec::new();
        for (index, owner) in [
            AccountId::new([7; 32]),
            AccountId::new(recovery::ATTACKER_ACCOUNT),
        ]
        .into_iter()
        .enumerate()
        {
            let id = crate::SubstrateBridgeOutboundChannel::submit(
                network,
                &frame_system::RawOrigin::Signed(owner.clone()),
                &[index as u8],
                (),
            )
            .unwrap();
            BridgeProxy::outbound_request(
                network.into(),
                id,
                owner.clone(),
                GenericAccount::Sora(AccountId::new([9; 32])),
                common::XOR,
                100 + index as u128,
                MessageStatus::InQueue,
            );
            let native =
                substrate_bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(network);
            let record =
                BridgeProxy::transactions((GenericNetworkId::Sub(network), owner.clone()), id)
                    .unwrap();
            messages.push(MessageWitness {
                message_id: id,
                message_hash: sp_io::hashing::blake2_256(&native[index].encode()),
                financial: Some(FinancialWitness {
                    owner,
                    asset: common::XOR,
                    amount: 100 + index as u128,
                    record_hash: sp_io::hashing::blake2_256(&record.encode()),
                }),
            });
        }
        OutgoingOperation {
            index: 0,
            target: OutgoingTarget::Channel {
                channel: ChannelKind::Substrate,
                network: network.into(),
                nonce: 1,
                commitment: None,
                archived: None,
                messages,
            },
            reconciliation_evidence: [1; 32],
        }
    }

    #[test]
    fn exact_batch_quarantine_is_atomic_and_preserves_native_backing_and_evidence() {
        sp_io::TestExternalities::default().execute_with(|| {
            let operation = queue_fixture();
            let policy = policy(&operation);
            initialize(&policy);
            let queue_before =
                substrate_bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(
                    SubNetworkId::Liberland,
                )
                .encode();
            let prepared = prepare(&policy, &operation).unwrap();
            assert_eq!(
                prepared[0].1,
                ClaimDisposition::Quarantined {
                    effective_owner: AccountId::new([8; 32])
                }
            );
            assert_eq!(prepared[1].1, ClaimDisposition::ExtinguishedAttacker);
            let (status, _) = apply_encoded_with_policy(&policy, &operation.encode()).unwrap();
            assert_eq!(status.next_chunk, 1);
            assert_eq!(status.phase, RecoveryPhase::Auditing);
            assert_eq!(
                substrate_bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(
                    SubNetworkId::Liberland
                )
                .encode(),
                queue_before
            );
            assert_eq!(
                substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(
                    SubNetworkId::Liberland
                ),
                0
            );
            if let OutgoingTarget::Channel {
                network, messages, ..
            } = &operation.target
            {
                for message in messages {
                    assert!(tombstone::message_quarantined(*network, message.message_id));
                    let witness = message.financial.as_ref().unwrap();
                    assert_eq!(
                        sp_io::hashing::blake2_256(
                            &BridgeProxy::transactions(
                                (*network, witness.owner.clone()),
                                message.message_id
                            )
                            .unwrap()
                            .encode()
                        ),
                        witness.record_hash
                    );
                }
            }
            sp_io::storage::clear(recovery::BRIDGE_HALT_STORAGE_KEY);
            assert!(crate::SubstrateBridgeOutboundChannel::submit(
                SubNetworkId::Liberland,
                &frame_system::RawOrigin::Signed(AccountId::new([7; 32])),
                &[9],
                ()
            )
            .is_err());
            assert_eq!(
                substrate_bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(
                    SubNetworkId::Liberland
                )
                .encode(),
                queue_before
            );
        });
    }

    #[test]
    fn historical_message_only_stop_preserves_queued_batch_and_nonce_after_halt_clear() {
        use frame_support::traits::Hooks;
        sp_io::TestExternalities::default().execute_with(|| {
            let operation = queue_fixture();
            initialize(&policy(&operation));
            let network = SubNetworkId::Liberland;
            let OutgoingTarget::Channel { messages, .. } = &operation.target else {
                unreachable!()
            };
            sp_io::storage::set(
                &tombstone::historical_message_key(network.into(), messages[0].message_id),
                b"snxpend1",
            );
            assert!(!tombstone::batch_quarantined(1, network.into(), 1));
            sp_io::storage::clear(recovery::BRIDGE_HALT_STORAGE_KEY);
            assert_ok!(crate::SubstrateBridgeOutboundChannel::update_interval(
                crate::RuntimeOrigin::root(),
                1
            ));
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            <crate::SubstrateBridgeOutboundChannel as Hooks<u32>>::on_initialize(10);
            <crate::SubstrateBridgeOutboundChannel as Hooks<u32>>::on_finalize(10);
            frame_support::assert_noop!(
                crate::SubstrateBridgeOutboundChannel::submit(
                    network,
                    &frame_system::RawOrigin::Signed(AccountId::new([7; 32])),
                    &[9],
                    ()
                ),
                substrate_bridge_channel::outbound::Error::<Runtime>::RecoveryHalted
            );
            assert_eq!(
                substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(network),
                0
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        });
        sp_io::TestExternalities::default().execute_with(|| {
            let network = SubNetworkId::Liberland;
            let id = <crate::BridgeOutboundChannel as OutboundChannel<SubNetworkId,AccountId,()>>::submit(network,&frame_system::RawOrigin::Root,&[1],()).unwrap();
            sp_io::storage::set(&tombstone::historical_message_key(network.into(),id),b"snxpend1");
            sp_io::storage::set(recovery::POLICY_STORAGE_KEY,&[1;32]);
            sp_io::storage::set(&frame_support::storage::storage_prefix(b"BridgeOutboundChannel",b"Interval"),&1u32.encode());
            assert!(!tombstone::batch_quarantined(0,network.into(),1));
            let root=sp_io::storage::root(sp_runtime::StateVersion::V1);
            <crate::BridgeOutboundChannel as Hooks<u32>>::on_initialize(10);
            frame_support::assert_noop!(<crate::BridgeOutboundChannel as OutboundChannel<SubNetworkId,AccountId,()>>::submit(network,&frame_system::RawOrigin::Root,&[2],()),bridge_channel::outbound::Error::<Runtime>::RecoveryHalted);
            assert_eq!(bridge_channel::outbound::ChannelNonces::<Runtime>::get(GenericNetworkId::Sub(network)),0);
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1),root);
        });
    }

    #[test]
    fn recovery_queue_submit_weights_preserve_history_and_cover_every_bounded_position() {
        use bridge_types::{evm::AdditionalEVMOutboundData, EVMChainId};
        type Generic = crate::BridgeOutboundChannel;
        type Substrate = crate::SubstrateBridgeOutboundChannel;
        sp_io::TestExternalities::default().execute_with(|| {
            let generic_base = <<Runtime as bridge_channel::outbound::Config>::WeightInfo as bridge_channel::outbound::WeightInfo>::submit();
            let substrate_base = <<Runtime as substrate_bridge_channel::outbound::Config>::WeightInfo as substrate_bridge_channel::outbound::WeightInfo>::submit();
            let weights = || [
                <Generic as OutboundChannel<SubNetworkId, AccountId, ()>>::submit_weight(),
                <Generic as OutboundChannel<EVMChainId, AccountId, AdditionalEVMOutboundData>>::submit_weight(),
                <Substrate as OutboundChannel<SubNetworkId, AccountId, ()>>::submit_weight(),
            ];
            assert_eq!(weights(), [generic_base, generic_base, substrate_base]);
            sp_io::storage::set(recovery::POLICY_STORAGE_KEY, &[1; 32]);
            let reads = <Runtime as frame_system::Config>::DbWeight::get()
                .reads(u64::from(crate::BridgeMaxMessagesPerCommit::get()) * 2);
            for (weight, base) in weights().into_iter().zip([generic_base, generic_base, substrate_base]) {
                assert!(base.saturating_add(reads).all_lte(weight));
                assert!(weight.proof_size() > base.proof_size());
            }
            sp_io::storage::clear(recovery::POLICY_STORAGE_KEY);
            assert_eq!(weights(), [generic_base, generic_base, substrate_base]);
        });
    }

    #[test]
    fn historical_message_only_stop_preserves_evm_queue_gas_on_rejected_append() {
        use bridge_types::{evm::AdditionalEVMOutboundData, EVMChainId, U256};
        type Channel = crate::BridgeOutboundChannel;
        sp_io::TestExternalities::default().execute_with(|| {
            let network = EVMChainId::from([1u8; 32]);
            let origin = frame_system::RawOrigin::Root;
            let additional = || AdditionalEVMOutboundData {
                target: bridge_types::H160::repeat_byte(9),
                max_gas: U256::from(1u32),
            };
            let id = <Channel as OutboundChannel<EVMChainId, AccountId, AdditionalEVMOutboundData>>::submit(
                network, &origin, &[1], additional(),
            ).unwrap();
            sp_io::storage::set(recovery::POLICY_STORAGE_KEY, &[1; 32]);
            sp_io::storage::set(&tombstone::historical_message_key(network.into(), id), b"snxpend1");
            assert!(!sp_io::storage::exists(recovery::BRIDGE_HALT_STORAGE_KEY));
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            frame_support::assert_noop!(
                <Channel as OutboundChannel<EVMChainId, AccountId, AdditionalEVMOutboundData>>::submit(
                    network, &origin, &[2], additional(),
                ),
                bridge_channel::outbound::Error::<Runtime>::RecoveryHalted
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        });
    }

    #[test]
    fn historical_message_only_stop_blocks_hashi_request_without_native_outgoing_tombstone() {
        sp_io::TestExternalities::default().execute_with(|| {
            let outgoing = eth_bridge::requests::OutgoingRequest::<Runtime>::Transfer(
                eth_bridge::requests::OutgoingTransfer {
                    from: AccountId::new([7; 32]),
                    to: bridge_types::H160::repeat_byte(9),
                    asset_id: common::XOR,
                    amount: 300,
                    nonce: 4,
                    network_id: 0,
                    timepoint: Default::default(),
                },
            );
            let native = eth_bridge::requests::OffchainRequest::outgoing(outgoing.clone());
            let id = native.hash();
            eth_bridge::Requests::<Runtime>::insert(0, id, &native);
            eth_bridge::RequestStatuses::<Runtime>::insert(
                0,
                id,
                eth_bridge::requests::RequestStatus::ApprovalsReady,
            );
            sp_io::storage::set(
                &tombstone::historical_message_key(GenericNetworkId::EVMLegacy(0), id),
                b"snxpend1",
            );
            assert!(!tombstone::hashi_quarantined(0u32, id));
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            frame_support::assert_noop!(
                outgoing.cancel(),
                eth_bridge::Error::<Runtime>::RecoveryHalted
            );
            assert!(eth_bridge::recovery::ensure_outgoing_active::<Runtime>(0, id).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        });
    }

    #[test]
    fn historical_native_hashi_stop_blocks_unindexed_request_without_global_halt() {
        sp_io::TestExternalities::default().execute_with(|| {
            let outgoing = eth_bridge::requests::OutgoingRequest::<Runtime>::Transfer(
                eth_bridge::requests::OutgoingTransfer {
                    from: AccountId::new([7; 32]),
                    to: bridge_types::H160::repeat_byte(9),
                    asset_id: common::XOR,
                    amount: 300,
                    nonce: 4,
                    network_id: 0,
                    timepoint: Default::default(),
                },
            );
            let native = eth_bridge::requests::OffchainRequest::outgoing(outgoing.clone());
            let id = native.hash();
            eth_bridge::Requests::<Runtime>::insert(0, id, &native);
            eth_bridge::RequestStatuses::<Runtime>::insert(
                0,
                id,
                eth_bridge::requests::RequestStatus::ApprovalsReady,
            );
            sp_io::storage::set(&tombstone::historical_hashi_key(0u32, id), b"snxhash1");
            assert!(tombstone::hashi_quarantined(0u32, id));
            assert!(!sp_io::storage::exists(recovery::BRIDGE_HALT_STORAGE_KEY));
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            frame_support::assert_noop!(
                outgoing.cancel(),
                eth_bridge::Error::<Runtime>::RecoveryHalted
            );
            assert!(eth_bridge::recovery::ensure_outgoing_active::<Runtime>(0, id).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        });
    }

    #[test]
    fn historical_signing_hash_blocks_valid_signature_reregistration_and_clear_without_global_halt()
    {
        use sp_core::Pair;
        sp_io::TestExternalities::default().execute_with(|| {
            System::set_block_number(1);
            let network = GenericNetworkId::Sub(SubNetworkId::Liberland);
            let keys: Vec<_> = (3u8..=6)
                .map(|seed| sp_core::ecdsa::Pair::from_seed(&[seed; 32]))
                .collect();
            assert_ok!(crate::BridgeDataSigner::register_network(
                crate::RuntimeOrigin::root(),
                network,
                keys.iter()
                    .map(|key| key.public())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            ));
            let hash = H256::repeat_byte(99);
            assert_ok!(crate::BridgeDataSigner::register_pending_approval(
                crate::RuntimeOrigin::root(),
                network,
                hash
            ));
            assert_ok!(crate::BridgeDataSigner::approve(
                crate::RuntimeOrigin::none(),
                network,
                hash,
                keys[0].sign_prehashed(hash.as_fixed_bytes())
            ));
            sp_io::storage::set(
                &tombstone::historical_commitment_key(network, hash),
                b"snxsign1",
            );
            assert!(!sp_io::storage::exists(recovery::BRIDGE_HALT_STORAGE_KEY));
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            frame_support::assert_noop!(
                crate::BridgeDataSigner::approve(
                    crate::RuntimeOrigin::none(),
                    network,
                    hash,
                    keys[1].sign_prehashed(hash.as_fixed_bytes())
                ),
                bridge_data_signer::Error::<Runtime>::RecoveryHalted
            );
            frame_support::assert_noop!(
                crate::BridgeDataSigner::clear_pending_approval(
                    crate::RuntimeOrigin::root(),
                    network,
                    hash
                ),
                bridge_data_signer::Error::<Runtime>::RecoveryHalted
            );
            frame_support::assert_noop!(
                crate::BridgeDataSigner::register_pending_approval(
                    crate::RuntimeOrigin::root(),
                    network,
                    hash
                ),
                bridge_data_signer::Error::<Runtime>::RecoveryHalted
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        });
    }

    #[test]
    fn omitted_batch_member_and_wrong_financial_owner_fail_without_partial_tombstones() {
        for wrong_owner in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let mut operation = queue_fixture();
                if let OutgoingTarget::Channel { messages, .. } = &mut operation.target {
                    if wrong_owner {
                        messages[0].financial.as_mut().unwrap().owner = AccountId::new([4; 32]);
                    } else {
                        messages.pop();
                    }
                }
                let policy = policy(&operation);
                initialize(&policy);
                assert_eq!(
                    apply_encoded_with_policy(&policy, &operation.encode()),
                    Err(RecoveryError::WrongPreimage)
                );
                let status = recovery::storage::status().unwrap().unwrap();
                assert_eq!(status.next_chunk, 0);
                assert_eq!(status.phase, RecoveryPhase::Failed);
                assert!(!tombstone::batch_quarantined(
                    1,
                    SubNetworkId::Liberland.into(),
                    1
                ));
                if let OutgoingTarget::Channel {
                    messages, network, ..
                } = &operation.target
                {
                    for message in messages {
                        assert!(!tombstone::message_quarantined(
                            *network,
                            message.message_id
                        ));
                    }
                }
            });
        }
    }

    #[test]
    fn committed_channel_requires_exact_native_latest_commitment() {
        sp_io::TestExternalities::default().execute_with(|| {
            let mut operation = queue_fixture();
            let native = substrate_bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(
                SubNetworkId::Liberland,
            );
            let commitment = GenericCommitment::Sub(bridge_types::substrate::Commitment {
                messages: native,
                nonce: 1,
            });
            substrate_bridge_channel::outbound::LatestCommitment::<Runtime>::insert(
                SubNetworkId::Liberland,
                bridge_types::types::GenericCommitmentWithBlock {
                    commitment: commitment.clone(),
                    block_number: 1,
                },
            );
            if let OutgoingTarget::Channel {
                commitment: expected,
                ..
            } = &mut operation.target
            {
                *expected = Some(commitment.hash());
            }
            let policy = policy(&operation);
            initialize(&policy);
            assert_ok!(apply_encoded_with_policy(&policy, &operation.encode()));
            assert!(tombstone::commitment_quarantined(
                SubNetworkId::Liberland.into(),
                commitment.hash()
            ));
            // Clearing the global latch cannot turn a retained commitment into a new approval window.
            sp_io::storage::clear(recovery::BRIDGE_HALT_STORAGE_KEY);
            assert!(crate::BridgeDataSigner::register_pending_approval(
                crate::RuntimeOrigin::root(),
                SubNetworkId::Liberland.into(),
                commitment.hash()
            )
            .is_err());
        });
    }

    #[test]
    fn hashi_native_record_quarantine_preserves_redemption_evidence_and_blocks_refund() {
        sp_io::TestExternalities::default().execute_with(|| {
            let outgoing = eth_bridge::requests::OutgoingRequest::<Runtime>::Transfer(
                eth_bridge::requests::OutgoingTransfer {
                    from: AccountId::new([7; 32]),
                    to: bridge_types::H160::repeat_byte(9),
                    asset_id: common::XOR,
                    amount: 300,
                    nonce: 4,
                    network_id: 0,
                    timepoint: Default::default(),
                },
            );
            let native = eth_bridge::requests::OffchainRequest::outgoing(outgoing.clone());
            let id = native.hash();
            let native_status = eth_bridge::requests::RequestStatus::ApprovalsReady;
            eth_bridge::Requests::<Runtime>::insert(0, id, &native);
            eth_bridge::RequestStatuses::<Runtime>::insert(0, id, &native_status);
            let operation = OutgoingOperation {
                index: 0,
                target: OutgoingTarget::Hashi {
                    network: 0,
                    request: id,
                    request_hash: sp_io::hashing::blake2_256(&native.encode()),
                    status_hash: sp_io::hashing::blake2_256(&native_status.encode()),
                    author: AccountId::new([7; 32]),
                    transfer: Some((common::XOR, 300)),
                },
                reconciliation_evidence: [2; 32],
            };
            let policy = policy(&operation);
            initialize(&policy);
            assert_ok!(apply_encoded_with_policy(&policy, &operation.encode()));
            assert_eq!(
                eth_bridge::Requests::<Runtime>::get(0, id)
                    .unwrap()
                    .encode(),
                native.encode()
            );
            assert_eq!(
                eth_bridge::RequestStatuses::<Runtime>::get(0, id),
                Some(native_status)
            );
            sp_io::storage::clear(recovery::BRIDGE_HALT_STORAGE_KEY);
            assert!(outgoing.cancel().is_err());
            assert!(eth_bridge::recovery::ensure_outgoing_active::<Runtime>(0, id).is_err());
        });
    }

    #[test]
    fn archived_commitment_requires_native_signing_index_and_preserves_signatures() {
        use sp_core::Pair;
        for authenticated in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let mut operation = queue_fixture();
                let network: GenericNetworkId = SubNetworkId::Liberland.into();
                let key = sp_core::ecdsa::Pair::from_seed(&[3; 32]);
                assert_ok!(crate::BridgeDataSigner::register_network(
                    crate::RuntimeOrigin::root(),
                    network,
                    (3u8..=6)
                        .map(|seed| sp_core::ecdsa::Pair::from_seed(&[seed; 32]).public())
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap()
                ));
                let commitment: GenericCommitment<
                    crate::BridgeMaxMessagesPerCommit,
                    crate::BridgeMaxMessagePayloadSize,
                > = GenericCommitment::Sub(bridge_types::substrate::Commitment {
                    messages: substrate_bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(
                        SubNetworkId::Liberland,
                    ),
                    nonce: 1,
                });
                let hash = commitment.hash();
                if authenticated {
                    assert_ok!(crate::BridgeDataSigner::register_pending_approval(
                        crate::RuntimeOrigin::root(),
                        network,
                        hash
                    ));
                    assert_ok!(crate::BridgeDataSigner::approve(
                        crate::RuntimeOrigin::none(),
                        network,
                        hash,
                        key.sign_prehashed(hash.as_fixed_bytes())
                    ));
                }
                let signer_state = (
                    crate::BridgeDataSigner::pending_approval(network, hash),
                    crate::BridgeDataSigner::approvals(network, hash),
                );
                let signer_state_hash = sp_io::hashing::blake2_256(&signer_state.encode());
                if let OutgoingTarget::Channel {
                    commitment: expected,
                    archived,
                    ..
                } = &mut operation.target
                {
                    *expected = Some(hash);
                    *archived = Some(ArchivedCommitmentWitness {
                        preimage: commitment.encode(),
                        signer_state_hash,
                    });
                }
                let policy = policy(&operation);
                initialize(&policy);
                let result = apply_encoded_with_policy(&policy, &operation.encode());
                if authenticated {
                    assert_ok!(result);
                } else {
                    assert_eq!(result, Err(RecoveryError::WrongPreimage));
                }
                assert_eq!(
                    (
                        crate::BridgeDataSigner::pending_approval(network, hash),
                        crate::BridgeDataSigner::approvals(network, hash)
                    )
                        .encode(),
                    signer_state.encode()
                );
                assert_eq!(
                    tombstone::commitment_quarantined(network, hash),
                    authenticated
                );
            });
        }
    }

    #[test]
    fn uncommitted_payload_cannot_freeze_or_advance_the_cursor() {
        sp_io::TestExternalities::default().execute_with(|| {
            let mut operation = queue_fixture();
            let policy = policy(&operation);
            initialize(&policy);
            operation.reconciliation_evidence = [2; 32];
            let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
            assert_eq!(
                apply_encoded_with_policy(&policy, &operation.encode()),
                Err(RecoveryError::WrongChunk)
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
        });
    }
}
