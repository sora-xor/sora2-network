//! Bounded, manifest-committed channel catch-up for SORA Nexus recovery.
//!
//! This is an internal recovery-executive API, never an extrinsic. The original
//! commitment and proof enter the native channel unchanged. Only its authenticated
//! application callback can resolve a replacement or extinguish an attacker claim.
//! HASHI templates carry native peer signatures collected after a recovery child.

#[cfg(test)]
#[path = "incident_recovery_incoming_channel_tests.rs"]
mod channel_tests;
#[cfg(test)]
#[path = "incident_recovery_hashi_custody_tests.rs"]
mod hashi_custody_tests;

#[path = "incident_recovery_incoming_audit.rs"]
mod audit;
#[path = "incident_recovery_incoming_balance.rs"]
mod balance_audit;
#[path = "incident_recovery_incoming_issuance.rs"]
mod issuance_journal;
pub use audit::{
    audit_dynamic_keys, audit_encoded_with_policy, audit_marker_key, audit_request_key,
    audit_targets, expected_requests, incoming_audit_weight, validate_incoming_audit,
    IncomingAudit, IncomingAuditTarget, IncomingRequestExpectation, ValidatedIncomingAudit,
};
pub use balance_audit::{
    audit_native_balances_with_policy, native_balance_audit_encoded_weight, request_balance_keys,
    AuditedIncomingSource, NativeBalanceAudit, MAX_BALANCE_SOURCES,
};

#[cfg(feature = "std")]
pub use audit::validate_completed_incoming_audit;

use alloc::{collections::BTreeSet, vec::Vec};
use bridge_types::{
    substrate::{BridgeCall, FAAppCall, JettonAppCall, ParachainAppCall, SubstrateAppCall},
    traits::{BalancePrecisionConverter, MessageDispatch, MessageDispatchOutcome},
    types::{GenericAdditionalInboundData, MessageId, MessageStatus},
    GenericAccount, GenericCommitment, GenericNetworkId, GenericTimepoint, SubNetworkId, H256,
    U256,
};
use codec::{Decode, DecodeLimit, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use common::recovery::{
    self, RecoveryError, RecoveryPhase, RecoveryPolicy, RecoveryStatus, ACTIVE_POLICY_STORAGE_KEY,
    BRIDGE_HALT_STORAGE_KEY, BRIDGE_INBOUND_CONTEXT_STORAGE_KEY, MAX_CHUNK_BYTES,
    POLICY_STORAGE_KEY, STATUS_STORAGE_KEY,
};
use frame_support::{dispatch::GetDispatchInfo, weights::Weight};

use crate::{
    AccountId, BridgeInboundChannel, BridgeMaxMessagePayloadSize, BridgeMaxMessagesPerCommit,
    Dispatch, MultiProof, Runtime, RuntimeCall, RuntimeOrigin, SubstrateBridgeInboundChannel,
    SubstrateDispatch, System, ThisNetworkId,
};

const ACTIVE_KEY: &[u8] = b":sora-nexus:recovery:incoming-active:v1";
const FAILED_KEY: &[u8] = b":sora-nexus:recovery:incoming-failed:v1";
const RECEIPT_PREFIX: &[u8] = b":sora-nexus:recovery:incoming-receipt:v1:";
const CONTROL_RECEIPT_PREFIX: &[u8] = b":sora-nexus:recovery:incoming-control:v1:";
const MAX_REGISTRY_ACCOUNTS: usize = 128;
const MAX_HASHI_APPROVALS: usize = 100;

type Commitment = GenericCommitment<BridgeMaxMessagesPerCommit, BridgeMaxMessagePayloadSize>;

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub struct HashiPeerApproval {
    pub signer: AccountId,
    /// Native BridgeMultisig call, not an arbitrary dispatch envelope.
    pub call: Vec<u8>,
    /// Native SignedExtra encoding. Only mortal, zero-tip approvals are admitted.
    pub extra: Vec<u8>,
    /// Witness excluded from the committed template; checked by the native signed payload.
    pub signature: crate::Signature,
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub struct NetworkAttackerRegistry {
    pub network: GenericNetworkId,
    pub accounts: Vec<GenericAccount>,
    /// Empty means explicitly reviewed absence, never absence of supplied evidence.
    pub no_known_attackers: bool,
    pub attestation: [u8; 32],
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub enum IncomingChannel {
    Substrate {
        network: SubNetworkId,
        commitment: Commitment,
        proof: MultiProof,
    },
    Generic {
        network: GenericNetworkId,
        commitment: Commitment,
        proof: MultiProof,
    },
    Hashi {
        network: u32,
        /// Original IncomingRequest::Transfer encoding, unchanged in native replay storage.
        request: Vec<u8>,
        approvals: Vec<HashiPeerApproval>,
    },
    /// Appended index 3: native Deposit requests retain their original zero `from`.
    /// The transaction signer is an explicitly separate source actor, never an
    /// inferred internal caller, depositor, or beneficial owner.
    HashiAuthenticatedSource {
        network: u32,
        request: Vec<u8>,
        raw_transaction: Vec<u8>,
        /// Exact signature-domain expectation, not proof of the actual source chain.
        /// None explicitly admits an unprotected v=27/28 legacy transaction.
        expected_chain_id: Option<U256>,
        approvals: Vec<HashiPeerApproval>,
    },
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub struct IncomingOperation {
    pub index: u32,
    pub channel: IncomingChannel,
    pub registries: Vec<NetworkAttackerRegistry>,
    /// Reviewed frozen source heads and delivery evidence, independently bound by release.
    pub finalized_source_evidence: [u8; 32],
}

impl IncomingOperation {
    pub fn commitment(&self) -> [u8; 32] {
        if let IncomingChannel::Hashi {
            network,
            request,
            approvals,
        } = &self.channel
        {
            // Signatures depend on a future recovery header. Commit every business field
            // and native nonce/era now, then verify proof bytes without a circular hash.
            let templates: Vec<_> = approvals
                .iter()
                .map(|approval| (&approval.signer, &approval.call, &approval.extra))
                .collect();
            return sp_io::hashing::blake2_256(
                &(
                    b"sora-nexus/hashi-incoming-operation/v1".as_slice(),
                    self.index,
                    network,
                    request,
                    templates,
                    &self.registries,
                    self.finalized_source_evidence,
                )
                    .encode(),
            );
        }
        if let IncomingChannel::HashiAuthenticatedSource {
            network,
            request,
            raw_transaction,
            expected_chain_id,
            approvals,
        } = &self.channel
        {
            let templates: Vec<_> = approvals
                .iter()
                .map(|approval| (&approval.signer, &approval.call, &approval.extra))
                .collect();
            return sp_io::hashing::blake2_256(
                &(
                    b"sora-nexus/hashi-authenticated-source-operation/v1".as_slice(),
                    self.index,
                    network,
                    request,
                    raw_transaction,
                    expected_chain_id,
                    templates,
                    &self.registries,
                    self.finalized_source_evidence,
                )
                    .encode(),
            );
        }
        sp_io::hashing::blake2_256(&(b"sora-nexus/incoming-operation/v1".as_slice(), self).encode())
    }

    fn native_call(&self) -> Option<RuntimeCall> {
        Some(match &self.channel {
            IncomingChannel::Substrate {
                network,
                commitment,
                proof,
            } => RuntimeCall::SubstrateBridgeInboundChannel(
                substrate_bridge_channel::inbound::Call::submit {
                    network_id: *network,
                    commitment: commitment.clone(),
                    proof: proof.clone(),
                },
            ),
            IncomingChannel::Generic {
                network,
                commitment,
                proof,
            } => RuntimeCall::BridgeInboundChannel(bridge_channel::inbound::Call::submit {
                network_id: *network,
                commitment: commitment.clone(),
                proof: proof.clone(),
            }),
            IncomingChannel::Hashi { .. } | IncomingChannel::HashiAuthenticatedSource { .. } => {
                return None
            }
        })
    }

    fn network(&self) -> GenericNetworkId {
        match self.channel {
            IncomingChannel::Substrate { network, .. } => network.into(),
            IncomingChannel::Generic { network, .. } => network,
            IncomingChannel::Hashi { network, .. }
            | IncomingChannel::HashiAuthenticatedSource { network, .. } => {
                GenericNetworkId::EVMLegacy(network)
            }
        }
    }

    /// Native verification/application weight plus a bounded recovery bookkeeping allowance.
    pub fn required_weight(&self) -> Weight {
        let native = match &self.channel {
            IncomingChannel::Hashi { approvals, .. }
            | IncomingChannel::HashiAuthenticatedSource { approvals, .. } => {
                approvals.iter().fold(
                    // Bounded source/destination query-index reads and the
                    // authenticated replacement move after native finalization,
                    // plus guarded reversal of an extinguished owned preparation.
                    Weight::from_parts(50_000_000, (MAX_CHUNK_BYTES / 2) as u64).saturating_add(
                        <Runtime as frame_system::Config>::DbWeight::get().reads_writes(13, 3),
                    ),
                    |weight, approval| {
                        weight
                            .saturating_add(
                                decode_runtime_call(&approval.call)
                                    .map(|call| call.get_dispatch_info().call_weight)
                                    .unwrap_or(Weight::MAX),
                            )
                            .saturating_add(Weight::from_parts(5_000_000_000, 16_384))
                            .saturating_add(
                                <Runtime as frame_system::Config>::DbWeight::get().reads(32),
                            )
                    },
                )
            }
            _ => self
                .native_call()
                .map(|call| call.get_dispatch_info().call_weight)
                .unwrap_or(Weight::MAX),
        };
        // The new variant runs one bounded 128 KiB/8192-node transaction parser,
        // three Keccak hashes and one secp256k1 recovery before any storage writes.
        // Charge a conservative fixed 50 ms even for a short envelope; old HASHI
        // weight and signed native max_weight are unchanged. Artifact rehearsals
        // must measure this bound; it is not a benchmark-generated estimate.
        let source_proof = if matches!(
            self.channel,
            IncomingChannel::HashiAuthenticatedSource { .. }
        ) {
            Weight::from_parts(50_000_000_000, 0)
        } else {
            Weight::zero()
        };
        native
            .saturating_add(source_proof)
            .saturating_add(Weight::from_parts(100_000_000, MAX_CHUNK_BYTES as u64))
            .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(64, 64))
            .saturating_add(audit::registration_preview_weight(self).saturating_mul(3))
            .saturating_add(audit::consumption_weight())
            .saturating_add(balance_audit::consumption_weight())
            .saturating_add(issuance_journal::consumption_weight())
    }
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
struct ExpectedTransfer {
    network: GenericNetworkId,
    id: H256,
    payload_hash: [u8; 32],
    origin_hash: [u8; 32],
    sender: GenericAccount,
    original: AccountId,
    beneficiary: AccountId,
    excluded: bool,
    delivered: bool,
}

/// Control messages retain their authenticated bytes. Status reports and outbound
/// configuration are deliberately journaled, never applied as refunds or sends.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub enum ControlDisposition {
    Deferred,
    VerifierPeer,
    /// Finalization writes local inbound asset metadata and sends no outbound work.
    NativeRegistration,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
enum ControlOutcome {
    Deferred,
    Applied,
    Failed,
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
struct ExpectedControl {
    network: GenericNetworkId,
    id: H256,
    payload: Vec<u8>,
    origin_hash: [u8; 32],
    disposition: ControlDisposition,
    delivered: bool,
    outcome: Option<ControlOutcome>,
}

#[derive(Clone, Encode, Decode, DecodeWithMemTracking)]
struct ActiveOperation {
    lineage: [u8; 32],
    policy: [u8; 32],
    operation: [u8; 32],
    index: u32,
    transfers: Vec<ExpectedTransfer>,
    controls: Vec<ExpectedControl>,
}

fn decode_bounded<T: DecodeWithMemTracking>(bytes: &[u8]) -> Result<T, RecoveryError> {
    if bytes.is_empty() || bytes.len() > MAX_CHUNK_BYTES {
        return Err(RecoveryError::ChunkBounds);
    }
    let mut input = bytes;
    let value = T::decode_with_mem_limit(&mut input, MAX_CHUNK_BYTES * 4)
        .map_err(|_| RecoveryError::WrongPreimage)?;
    if !input.is_empty() {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(value)
}

fn decode_call(payload: &[u8]) -> Result<BridgeCall, RecoveryError> {
    decode_bounded(payload)
}

fn decode_runtime_call(bytes: &[u8]) -> Result<RuntimeCall, RecoveryError> {
    if bytes.is_empty() || bytes.len() > MAX_CHUNK_BYTES {
        return Err(RecoveryError::ChunkBounds);
    }
    let mut input = bytes;
    let call = {
        let mut tracked = codec::MemTrackingInput::new(&mut input, MAX_CHUNK_BYTES * 4);
        RuntimeCall::decode_with_depth_limit(8, &mut tracked)
            .map_err(|_| RecoveryError::WrongPreimage)?
    };
    if !input.is_empty() || call.encode() != bytes {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(call)
}

fn hashi_transfer(
    bytes: &[u8],
) -> Result<eth_bridge::requests::IncomingTransfer<Runtime>, RecoveryError> {
    match decode_bounded::<eth_bridge::requests::IncomingRequest<Runtime>>(bytes)? {
        eth_bridge::requests::IncomingRequest::Transfer(transfer)
            if transfer.amount > 0 && transfer.from != bridge_types::H160::zero() =>
        {
            Ok(transfer)
        }
        _ => Err(RecoveryError::WrongPreimage),
    }
}

/// Resolve only the outer Ethereum transaction signer for an exact native
/// zero-sender Deposit request. Peer quorum still authenticates the original
/// network/tx hash; source-chain finality, event association and routed ownership
/// require independently bound release evidence and are not established here.
pub fn authenticated_hashi_source(
    request: &[u8],
    raw_transaction: &[u8],
    expected_chain_id: Option<U256>,
) -> Result<crate::incident_recovery_evm_source::AuthenticatedEvmSource, RecoveryError> {
    let transfer = hashi_zero_sender_transfer(request)?;
    let source = crate::incident_recovery_evm_source::authenticated_transaction_source(
        raw_transaction,
        transfer.tx_hash,
    )?;
    if source.chain_id != expected_chain_id {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(source)
}

fn hashi_zero_sender_transfer(
    bytes: &[u8],
) -> Result<eth_bridge::requests::IncomingTransfer<Runtime>, RecoveryError> {
    let original = decode_bounded::<eth_bridge::requests::IncomingRequest<Runtime>>(bytes)?;
    if original.encode() != bytes {
        return Err(RecoveryError::WrongPreimage);
    }
    match original {
        eth_bridge::requests::IncomingRequest::Transfer(transfer)
            if transfer.amount > 0 && transfer.from == bridge_types::H160::zero() =>
        {
            Ok(transfer)
        }
        _ => Err(RecoveryError::WrongPreimage),
    }
}

/// Internal decode after prepare() has authenticated the committed source proof.
/// It never changes request bytes or relaxes the existing HASHI nonzero guard.
fn native_hashi_transfer(
    channel: &IncomingChannel,
) -> Result<eth_bridge::requests::IncomingTransfer<Runtime>, RecoveryError> {
    match channel {
        IncomingChannel::Hashi { request, .. } => hashi_transfer(request),
        IncomingChannel::HashiAuthenticatedSource { request, .. } => {
            hashi_zero_sender_transfer(request)
        }
        _ => Err(RecoveryError::WrongPreimage),
    }
}

/// Parse only the stable wire fields, while decoding the full native extension separately.
/// The legacy and final tuples have identical Era/CompactNonce/CompactTip bytes.
fn hashi_extra(
    bytes: &[u8],
) -> Result<(crate::SignedExtra, sp_runtime::generic::Era, crate::Index), RecoveryError> {
    let extra: crate::SignedExtra = decode_bounded(bytes)?;
    let mut input = bytes;
    let era =
        sp_runtime::generic::Era::decode(&mut input).map_err(|_| RecoveryError::WrongPreimage)?;
    let codec::Compact(nonce) = codec::Compact::<crate::Index>::decode(&mut input)
        .map_err(|_| RecoveryError::WrongPreimage)?;
    let codec::Compact(tip) =
        codec::Compact::<u128>::decode(&mut input).map_err(|_| RecoveryError::WrongPreimage)?;
    if !input.is_empty() || tip != 0 || era.is_immortal() || extra.encode() != bytes {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok((extra, era, nonce))
}

/// Returns the exact native outer call and whether this completes an already prepared receipt.
fn hashi_call(
    approval: &HashiPeerApproval,
    transfer: &eth_bridge::requests::IncomingTransfer<Runtime>,
) -> Result<(RuntimeCall, bool, [u8; 32]), RecoveryError> {
    let call = decode_runtime_call(&approval.call)?;
    let (id, inner) = match &call {
        RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi { id, call, .. }) => {
            (id, decode_runtime_call(call)?)
        }
        RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi_threshold_1 {
            id,
            call,
            ..
        }) => (id, *call.clone()),
        _ => return Err(RecoveryError::WrongPreimage),
    };
    if crate::EthBridge::bridge_account(transfer.network_id).as_ref() != Some(id) {
        return Err(RecoveryError::WrongPreimage);
    }
    let request = eth_bridge::requests::IncomingRequest::Transfer(transfer.clone());
    let incoming_hash = eth_bridge::requests::OffchainRequest::incoming(request.clone()).hash();
    let finalize = match &inner {
        RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            load_incoming_request: eth_bridge::requests::LoadIncomingRequest::Transaction(load),
            incoming_request_result: Ok(incoming),
        }) if incoming == &request
            && load
                == &eth_bridge::requests::LoadIncomingTransactionRequest::new(
                    transfer.author.clone(),
                    transfer.tx_hash,
                    transfer.timepoint,
                    eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                    transfer.network_id,
                ) =>
        {
            false
        }
        RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
            hash,
            network_id,
        }) if *hash == incoming_hash && *network_id == transfer.network_id => true,
        _ => return Err(RecoveryError::WrongPreimage),
    };
    Ok((call, finalize, sp_io::hashing::blake2_256(&inner.encode())))
}

fn checked_hashi_approval(
    approval: &HashiPeerApproval,
    transfer: &eth_bridge::requests::IncomingTransfer<Runtime>,
) -> Result<(crate::CheckedExtrinsic, crate::Index), RecoveryError> {
    use sp_runtime::traits::Checkable;
    let (call, _, _) = hashi_call(approval, transfer)?;
    let (extra, era, nonce) = hashi_extra(&approval.extra)?;
    let current = u64::from(System::block_number());
    let birth = era.birth(current);
    if birth <= u64::from(recovery::CHECKPOINT_NUMBER)
        || birth >= current
        || nonce == crate::Index::MAX
        || System::account_nonce(&approval.signer) != nonce
    {
        return Err(RecoveryError::WrongPreimage);
    }
    let checked = crate::UncheckedExtrinsic::new_signed(
        call,
        approval.signer.clone(),
        approval.signature.clone(),
        extra,
    )
    .check(&frame_system::ChainContext::<Runtime>::default())
    .map_err(|_| RecoveryError::WrongPreimage)?;
    if !matches!(&checked.format, sp_runtime::generic::ExtrinsicFormat::Signed(signer, _) if signer == &approval.signer)
    {
        return Err(RecoveryError::WrongPreimage);
    }
    frame_system::CheckNonce::<Runtime>::validate_nonce_for_account(&approval.signer, nonce)
        .map_err(|_| RecoveryError::Replay)?;
    Ok((checked, nonce))
}

fn registry<'a>(
    operation: &'a IncomingOperation,
    network: GenericNetworkId,
) -> Result<&'a NetworkAttackerRegistry, RecoveryError> {
    operation
        .registries
        .iter()
        .find(|entry| entry.network == network)
        .ok_or(RecoveryError::WrongPreimage)
}

fn account_matches_network(network: GenericNetworkId, account: &GenericAccount) -> bool {
    matches!(
        (network, account),
        (
            GenericNetworkId::EVM(_) | GenericNetworkId::EVMLegacy(_),
            GenericAccount::EVM(_)
        ) | (GenericNetworkId::TON(_), GenericAccount::TON(_))
            | (
                GenericNetworkId::Sub(SubNetworkId::Mainnet),
                GenericAccount::Sora(_)
            )
            | (
                GenericNetworkId::Sub(SubNetworkId::Liberland),
                GenericAccount::Liberland(_)
            )
            | (
                GenericNetworkId::Sub(
                    SubNetworkId::Kusama
                        | SubNetworkId::Polkadot
                        | SubNetworkId::Rococo
                        | SubNetworkId::Alphanet
                ),
                GenericAccount::Parachain(_)
            )
    )
}

fn normalized_account(account: &GenericAccount) -> Result<GenericAccount, RecoveryError> {
    match account {
        GenericAccount::Parachain(location) => {
            let location: xcm::v4::Location = location
                .clone()
                .try_into()
                .map_err(|_| RecoveryError::WrongPreimage)?;
            Ok(GenericAccount::Parachain(xcm::VersionedLocation::V4(
                location,
            )))
        }
        GenericAccount::Unknown | GenericAccount::Root => Err(RecoveryError::WrongPreimage),
        _ => Ok(account.clone()),
    }
}

fn validate_registries(
    operation: &IncomingOperation,
    policy: &RecoveryPolicy,
) -> Result<(), RecoveryError> {
    let sora = GenericNetworkId::Sub(SubNetworkId::Mainnet);
    if operation.finalized_source_evidence == [0; 32]
        || operation.registries.is_empty()
        || operation.registries.len() > 2
        || operation.network() == sora
    {
        return Err(RecoveryError::WrongPreimage);
    }
    let mut networks = BTreeSet::new();
    for entry in &operation.registries {
        if entry.attestation == [0; 32]
            || entry.accounts.len() > MAX_REGISTRY_ACCOUNTS
            || entry.no_known_attackers != entry.accounts.is_empty()
            || !networks.insert(entry.network.encode())
            || (entry.network != sora && entry.network != operation.network())
        {
            return Err(RecoveryError::WrongPreimage);
        }
        let mut accounts = BTreeSet::new();
        for account in &entry.accounts {
            let normalized = normalized_account(account)?;
            if !account_matches_network(entry.network, account)
                || &normalized != account
                || !accounts.insert(normalized.encode())
            {
                return Err(RecoveryError::WrongPreimage);
            }
            if entry.network == sora {
                let GenericAccount::Sora(account) = account else {
                    return Err(RecoveryError::WrongPreimage);
                };
                if policy
                    .replacements
                    .iter()
                    .any(|mapping| &mapping.old == account || &mapping.new == account)
                {
                    return Err(RecoveryError::InvalidMapping);
                }
            }
        }
    }
    if !registry(operation, sora)?
        .accounts
        .contains(&GenericAccount::Sora(AccountId::new(
            recovery::ATTACKER_ACCOUNT,
        )))
    {
        return Err(RecoveryError::WrongPreimage);
    }
    let source = registry(operation, operation.network())?;
    if matches!(
        operation.network(),
        GenericNetworkId::EVM(_) | GenericNetworkId::EVMLegacy(_)
    ) {
        if !source
            .accounts
            .contains(&GenericAccount::EVM(bridge_types::H160::from(
                recovery::ATTACKER_EVM,
            )))
        {
            return Err(RecoveryError::WrongPreimage);
        }
    }
    Ok(())
}

/// Only known Substrate control variants have an explicit recovery disposition.
/// Registration requests and local signer changes need a separately reviewed
/// reopening or setup action. Journal them so they cannot issue outbound work or
/// reinterpret abandoned status reports. Inbound registration finalization and
/// verifier rotations run natively so later messages can mint/authenticate. Native
/// peer acknowledgements remain deferred by the outbound halt.
fn control_disposition(call: &BridgeCall) -> Option<ControlDisposition> {
    match call {
        BridgeCall::MultisigVerifier(_) => Some(ControlDisposition::VerifierPeer),
        BridgeCall::SubstrateApp(SubstrateAppCall::FinalizeAssetRegistration { .. })
        | BridgeCall::ParachainApp(ParachainAppCall::FinalizeAssetRegistration { .. }) => {
            Some(ControlDisposition::NativeRegistration)
        }
        BridgeCall::DataSigner(_)
        | BridgeCall::SubstrateApp(
            SubstrateAppCall::RegisterAsset { .. } | SubstrateAppCall::ReportTransferResult { .. },
        )
        | BridgeCall::ParachainApp(ParachainAppCall::ReportXCMTransferResult { .. }) => {
            Some(ControlDisposition::Deferred)
        }
        _ => None,
    }
}

/// Structural classification shared with release coverage. This does not
/// authenticate a message or authorize dispatch; only native channel execution does.
pub fn classify_substrate_control(
    payload: &[u8],
) -> Result<Option<ControlDisposition>, RecoveryError> {
    let call = decode_call(payload)?;
    if call.encode() != payload || payload.len() > BridgeMaxMessagePayloadSize::get() as usize {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(control_disposition(&call))
}

fn expected_control(
    network: GenericNetworkId,
    id: MessageId,
    payload: &[u8],
    timepoint: GenericTimepoint,
) -> Result<Option<ExpectedControl>, RecoveryError> {
    let call = decode_call(payload)?;
    if call.encode() != payload || payload.len() > BridgeMaxMessagePayloadSize::get() as usize {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(
        control_disposition(&call).map(|disposition| ExpectedControl {
            network,
            id: id.hash(),
            payload: payload.to_vec(),
            origin_hash: sp_io::hashing::blake2_256(
                &(timepoint, GenericAdditionalInboundData::Sub).encode(),
            ),
            disposition,
            delivered: false,
            outcome: None,
        }),
    )
}

fn parties(call: &BridgeCall) -> Result<(GenericAccount, AccountId), RecoveryError> {
    match call {
        BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
            sender,
            recipient: GenericAccount::Sora(recipient),
            ..
        }) => {
            if matches!(sender, GenericAccount::Unknown | GenericAccount::Root) {
                return Err(RecoveryError::WrongPreimage);
            }
            Ok((sender.clone(), recipient.clone()))
        }
        BridgeCall::ParachainApp(ParachainAppCall::Transfer {
            sender: Some(sender),
            recipient,
            ..
        }) => Ok((GenericAccount::Parachain(sender.clone()), recipient.clone())),
        BridgeCall::JettonApp(JettonAppCall::Transfer {
            sender, recipient, ..
        }) => Ok((
            GenericAccount::TON(sender.address().ok_or(RecoveryError::WrongPreimage)?),
            recipient.clone(),
        )),
        #[cfg(feature = "wip")]
        BridgeCall::FAApp(FAAppCall::Transfer {
            sender, recipient, ..
        }) => Ok((GenericAccount::EVM(*sender), recipient.clone())),
        _ => Err(RecoveryError::WrongPreimage),
    }
}

fn replace_recipient(
    mut call: BridgeCall,
    beneficiary: AccountId,
) -> Result<Vec<u8>, RecoveryError> {
    match &mut call {
        BridgeCall::SubstrateApp(SubstrateAppCall::Transfer { recipient, .. }) => {
            *recipient = GenericAccount::Sora(beneficiary)
        }
        BridgeCall::ParachainApp(ParachainAppCall::Transfer { recipient, .. })
        | BridgeCall::JettonApp(JettonAppCall::Transfer { recipient, .. }) => {
            *recipient = beneficiary
        }
        #[cfg(feature = "wip")]
        BridgeCall::FAApp(FAAppCall::Transfer { recipient, .. }) => *recipient = beneficiary,
        _ => return Err(RecoveryError::WrongPreimage),
    }
    Ok(call.encode())
}

fn expected_transfer(
    operation: &IncomingOperation,
    policy: &RecoveryPolicy,
    id: MessageId,
    payload: &[u8],
    timepoint: GenericTimepoint,
    additional: GenericAdditionalInboundData,
    substrate: bool,
) -> Result<ExpectedTransfer, RecoveryError> {
    let call = decode_call(payload)?;
    // Match the production dispatch routing before a native channel consumes its nonce.
    if substrate {
        if !matches!(
            &call,
            BridgeCall::SubstrateApp(SubstrateAppCall::Transfer { .. })
                | BridgeCall::ParachainApp(ParachainAppCall::Transfer { .. })
        ) {
            return Err(RecoveryError::WrongPreimage);
        }
    } else if !matches!(
        &call,
        BridgeCall::JettonApp(JettonAppCall::Transfer { .. })
            | BridgeCall::FAApp(FAAppCall::Transfer { .. })
    ) {
        return Err(RecoveryError::WrongPreimage);
    }
    let (sender, original) = parties(&call)?;
    let network = operation.network();
    if !account_matches_network(network, &sender) {
        return Err(RecoveryError::WrongPreimage);
    }
    let sora = registry(operation, GenericNetworkId::Sub(SubNetworkId::Mainnet))?;
    let excluded = registry(operation, network)?
        .accounts
        .contains(&normalized_account(&sender)?)
        || sora
            .accounts
            .contains(&GenericAccount::Sora(original.clone()));
    let beneficiary = policy
        .replacements
        .binary_search_by(|entry| entry.old.cmp(&original))
        .ok()
        .map(|index| policy.replacements[index].new.clone())
        .unwrap_or_else(|| original.clone());
    Ok(ExpectedTransfer {
        network,
        id: id.hash(),
        payload_hash: sp_io::hashing::blake2_256(payload),
        origin_hash: sp_io::hashing::blake2_256(&(timepoint, additional).encode()),
        sender,
        original,
        beneficiary,
        excluded,
        delivered: false,
    })
}

fn prepare(
    operation: &IncomingOperation,
    policy: &RecoveryPolicy,
) -> Result<ActiveOperation, RecoveryError> {
    validate_registries(operation, policy)?;
    if operation.encoded_size() > MAX_CHUNK_BYTES {
        return Err(RecoveryError::ChunkBounds);
    }
    let network = operation.network();
    let mut transfers = Vec::new();
    let mut controls = Vec::new();
    match &operation.channel {
        IncomingChannel::Hashi {
            network,
            request,
            approvals,
        }
        | IncomingChannel::HashiAuthenticatedSource {
            network,
            request,
            approvals,
            ..
        } => {
            if approvals.is_empty() || approvals.len() > MAX_HASHI_APPROVALS {
                return Err(RecoveryError::ChunkBounds);
            }
            let transfer = native_hashi_transfer(&operation.channel)?;
            if transfer.network_id != *network {
                return Err(RecoveryError::WrongPreimage);
            }
            let mut peers = BTreeSet::new();
            let mut inner_hash = None;
            for approval in approvals {
                if !peers.insert(approval.signer.clone())
                    || approval.signer == AccountId::new(recovery::ATTACKER_ACCOUNT)
                    || policy
                        .replacements
                        .iter()
                        .any(|entry| entry.old == approval.signer)
                {
                    return Err(RecoveryError::InvalidMapping);
                }
                hashi_extra(&approval.extra)?;
                let (_, _, hash) = hashi_call(approval, &transfer)?;
                if inner_hash
                    .replace(hash)
                    .is_some_and(|previous| previous != hash)
                {
                    return Err(RecoveryError::WrongPreimage);
                }
            }
            let original = transfer.to.clone();
            let beneficiary = policy
                .replacements
                .iter()
                .find(|entry| entry.old == original)
                .map(|entry| entry.new.clone())
                .unwrap_or_else(|| original.clone());
            let actor = match &operation.channel {
                IncomingChannel::HashiAuthenticatedSource {
                    raw_transaction,
                    expected_chain_id,
                    ..
                } => {
                    authenticated_hashi_source(request, raw_transaction, *expected_chain_id)?.signer
                }
                _ => transfer.from,
            };
            // Receipt sender means the separately authenticated transaction actor
            // for the new variant. Native request and notifier still carry from=0.
            let sender = GenericAccount::EVM(actor);
            let excluded = registry(operation, operation.network())?
                .accounts
                .contains(&sender)
                || registry(operation, ThisNetworkId::get())?
                    .accounts
                    .contains(&GenericAccount::Sora(original.clone()));
            transfers.push(ExpectedTransfer {
                network: operation.network(),
                id: transfer.tx_hash,
                payload_hash: sp_io::hashing::blake2_256(request),
                origin_hash: sp_io::hashing::blake2_256(&(*network, transfer.at_height).encode()),
                sender,
                original,
                beneficiary,
                excluded,
                delivered: false,
            });
        }
        IncomingChannel::Substrate {
            commitment: GenericCommitment::Sub(batch),
            ..
        } => {
            for (index, message) in batch.messages.iter().enumerate() {
                let id =
                    MessageId::batched(network, ThisNetworkId::get(), batch.nonce, index as u64);
                if let Some(control) =
                    expected_control(network, id.clone(), &message.payload, message.timepoint)?
                {
                    controls.push(control);
                } else {
                    transfers.push(expected_transfer(
                        operation,
                        policy,
                        id,
                        &message.payload,
                        message.timepoint,
                        GenericAdditionalInboundData::Sub,
                        true,
                    )?);
                }
            }
        }
        IncomingChannel::Generic {
            network: GenericNetworkId::TON(_),
            commitment: GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(message)),
            ..
        } => {
            let id = MessageId::basic(network, ThisNetworkId::get(), message.nonce);
            transfers.push(expected_transfer(
                operation,
                policy,
                id,
                &message.payload,
                GenericTimepoint::TON(message.transaction_id),
                bridge_types::ton::AdditionalTONInboundData {
                    source: message.source,
                }
                .into(),
                false,
            )?);
        }
        IncomingChannel::Generic {
            network: GenericNetworkId::EVM(_),
            commitment: GenericCommitment::EVM(bridge_types::evm::Commitment::Inbound(message)),
            ..
        } => {
            let id = MessageId::basic(network, ThisNetworkId::get(), message.nonce);
            transfers.push(expected_transfer(
                operation,
                policy,
                id,
                &message.payload,
                GenericTimepoint::EVM(message.block_number),
                bridge_types::evm::AdditionalEVMInboundData {
                    source: message.source,
                }
                .into(),
                false,
            )?);
        }
        _ => return Err(RecoveryError::WrongPreimage),
    }
    let count = transfers.len().saturating_add(controls.len());
    if count == 0 || count > BridgeMaxMessagesPerCommit::get() as usize {
        return Err(RecoveryError::ChunkBounds);
    }
    Ok(ActiveOperation {
        lineage: policy.commitment(),
        policy: policy.commitment(),
        operation: operation.commitment(),
        index: operation.index,
        transfers,
        controls,
    })
}

/// Bounded admission weight for an encoded operation, independent of whether its
/// committed execution succeeds or freezes recovery. No storage writes.
pub fn incoming_encoded_weight(bytes: &[u8]) -> Result<Weight, RecoveryError> {
    let operation: IncomingOperation = decode_bounded(bytes)?;
    Ok(operation.required_weight())
}

/// The bootstrap must charge incoming_encoded_weight as Mandatory for every
/// accepted operation, including frozen failures, and allow at most one operation
/// per block. Uncommitted inputs cannot freeze or modify recovery.
pub fn apply_encoded(bytes: &[u8]) -> Result<(RecoveryStatus, Weight), RecoveryError> {
    let policy = crate::incident_recovery::compiled_policy()?;
    apply_encoded_with_policy(&policy, bytes)
}

/// Repair runtimes supply their quorum-installed active work policy. The original
/// lineage remains in RecoveryStatus::policy and the immutable policy storage key.
pub fn apply_encoded_with_policy(
    policy: &RecoveryPolicy,
    bytes: &[u8],
) -> Result<(RecoveryStatus, Weight), RecoveryError> {
    let operation: IncomingOperation = decode_bounded(bytes)?;
    apply_operation(policy, &operation)
}

fn apply_operation(
    policy: &RecoveryPolicy,
    operation: &IncomingOperation,
) -> Result<(RecoveryStatus, Weight), RecoveryError> {
    policy.validate()?;
    let mut status = recovery::storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
    if sp_io::storage::get(ACTIVE_POLICY_STORAGE_KEY).as_deref()
        != Some(policy.commitment().as_slice())
        || sp_io::storage::get(POLICY_STORAGE_KEY).as_deref() != Some(status.policy.as_slice())
    {
        return Err(RecoveryError::WrongPolicy);
    }
    if status.phase != RecoveryPhase::Applying
        || System::block_number() <= recovery::CHECKPOINT_NUMBER
    {
        return Err(RecoveryError::WrongPhase);
    }
    if status.next_chunk != operation.index
        || policy.chunk_hashes.get(operation.index as usize) != Some(&operation.commitment())
    {
        return Err(RecoveryError::WrongChunk);
    }
    if !sp_io::storage::exists(BRIDGE_HALT_STORAGE_KEY)
        || [
            ACTIVE_KEY,
            FAILED_KEY,
            BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
            eth_bridge::recovery::HASHI_RESOLUTION_KEY,
        ]
        .iter()
        .any(|key| sp_io::storage::exists(key))
    {
        return Err(RecoveryError::WrongPhase);
    }
    if operation.encoded_size() > MAX_CHUNK_BYTES {
        return Err(RecoveryError::ChunkBounds);
    }
    // Witness bytes are not committed. Reject them before writes so a block
    // author cannot freeze valid committed work by corrupting a signature.
    if let IncomingChannel::Hashi { approvals, .. }
    | IncomingChannel::HashiAuthenticatedSource { approvals, .. } = &operation.channel
    {
        // Preserve the bound formerly enforced by prepare before witness checking.
        if approvals.is_empty() || approvals.len() > MAX_HASHI_APPROVALS {
            return Err(RecoveryError::ChunkBounds);
        }
        let transfer = native_hashi_transfer(&operation.channel)?;
        for approval in approvals {
            checked_hashi_approval(approval, &transfer)?;
        }
    }
    // Preserve authentication/registry admission as a read-only rejection. Only
    // the new consumption-state preflight joins the transactional failure path.
    let mut active = prepare(operation, policy)?;
    active.lineage = status.policy;
    let weight = operation.required_weight();
    if !weight.all_lte(<Runtime as frame_system::Config>::BlockWeights::get().max_block) {
        return Err(RecoveryError::ChunkBounds);
    }
    let result: Result<(), RecoveryError> =
        frame_support::storage::transactional::with_transaction_opaque_err(|| {
            // Consumption-state preflight errors follow native rollback/freeze
            // semantics, after read-only authentication admission has succeeded.
            let consumption = match audit::prepare_consumption(operation, &active, &status) {
                Ok(consumption) => consumption,
                Err(error) => {
                    return frame_support::storage::TransactionOutcome::Rollback(Err(error));
                }
            };
            let balance_journal = match balance_audit::prepare(operation, &active, &status, policy)
            {
                Ok(value) => value,
                Err(error) => {
                    return frame_support::storage::TransactionOutcome::Rollback(Err(error))
                }
            };
            let issuance = match issuance_journal::prepare(operation, &active, policy) {
                Ok(value) => value,
                Err(error) => {
                    return frame_support::storage::TransactionOutcome::Rollback(Err(error));
                }
            };
            sp_io::storage::set(ACTIVE_KEY, &active.encode());
            sp_io::storage::set(
                BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
                &System::block_number().encode(),
            );
            let dispatched = match &operation.channel {
                IncomingChannel::Hashi {
                    network, approvals, ..
                }
                | IncomingChannel::HashiAuthenticatedSource {
                    network, approvals, ..
                } => native_hashi_transfer(&operation.channel)
                    .and_then(|transfer| dispatch_hashi(policy, *network, &transfer, approvals))
                    .is_ok(),
                IncomingChannel::Substrate {
                    network,
                    commitment,
                    proof,
                } => SubstrateBridgeInboundChannel::submit(
                    RuntimeOrigin::none(),
                    *network,
                    commitment.clone(),
                    proof.clone(),
                )
                .is_ok(),
                IncomingChannel::Generic {
                    network,
                    commitment,
                    proof,
                } => BridgeInboundChannel::submit(
                    RuntimeOrigin::none(),
                    *network,
                    commitment.clone(),
                    proof.clone(),
                )
                .is_ok(),
            };
            let completed = sp_io::storage::get(ACTIVE_KEY)
                .and_then(|bytes| decode_bounded::<ActiveOperation>(&bytes).ok())
                .is_some_and(|active| {
                    active.transfers.iter().all(|entry| entry.delivered)
                        && active.controls.iter().all(|entry| entry.delivered)
                });
            if !dispatched || sp_io::storage::exists(FAILED_KEY) || !completed {
                return frame_support::storage::TransactionOutcome::Rollback(Err(
                    RecoveryError::WrongPreimage,
                ));
            }
            let consumed = sp_io::storage::get(ACTIVE_KEY)
                .ok_or(RecoveryError::WrongPreimage)
                .and_then(|bytes| decode_bounded::<ActiveOperation>(&bytes))
                .and_then(|active| audit::record_consumption(consumption, &active));
            if let Err(error) = consumed {
                return frame_support::storage::TransactionOutcome::Rollback(Err(error));
            }
            if let Err(error) = balance_audit::record(balance_journal) {
                return frame_support::storage::TransactionOutcome::Rollback(Err(error));
            }
            if let Err(error) = issuance_journal::record(policy, issuance) {
                return frame_support::storage::TransactionOutcome::Rollback(Err(error));
            }
            sp_io::storage::clear(ACTIVE_KEY);
            sp_io::storage::clear(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY);
            sp_io::storage::clear(eth_bridge::recovery::HASHI_RESOLUTION_KEY);
            status.next_chunk += 1;
            if status.next_chunk as usize == policy.chunk_hashes.len() {
                status.phase = RecoveryPhase::Auditing;
            }
            sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
            frame_support::storage::TransactionOutcome::Commit(Ok(()))
        })
        .map_err(|_| RecoveryError::WrongPhase)?;
    if let Err(error) = result {
        status.phase = RecoveryPhase::Failed;
        status.failure = Some(error);
        sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
        return Err(error);
    }
    Ok((status, weight))
}

fn control_receipt_key(network: GenericNetworkId, id: H256) -> Vec<u8> {
    let mut key = CONTROL_RECEIPT_PREFIX.to_vec();
    key.extend((network, id).encode());
    key
}

fn receipt_key(network: GenericNetworkId, id: H256) -> Vec<u8> {
    let mut key = RECEIPT_PREFIX.to_vec();
    key.extend((network, id).encode());
    key
}

fn dispatch_hashi(
    policy: &RecoveryPolicy,
    network: u32,
    transfer: &eth_bridge::requests::IncomingTransfer<Runtime>,
    approvals: &[HashiPeerApproval],
) -> Result<(), RecoveryError> {
    use sp_runtime::traits::Dispatchable;
    let mut active: ActiveOperation =
        decode_bounded(&sp_io::storage::get(ACTIVE_KEY).ok_or(RecoveryError::WrongPhase)?)?;
    let entry = active
        .transfers
        .first_mut()
        .ok_or(RecoveryError::WrongPreimage)?;
    let receipt = receipt_key(GenericNetworkId::EVMLegacy(network), transfer.tx_hash);
    if entry.delivered || sp_io::storage::exists(&receipt) {
        return Err(RecoveryError::Replay);
    }
    let original = eth_bridge::requests::IncomingRequest::Transfer(transfer.clone());
    let incoming_hash = eth_bridge::requests::OffchainRequest::incoming(original.clone()).hash();
    let index_owner = if transfer.author == AccountId::new(recovery::ATTACKER_ACCOUNT) {
        None
    } else {
        Some(
            policy
                .replacements
                .iter()
                .find(|entry| entry.old == transfer.author)
                .map_or_else(|| transfer.author.clone(), |entry| entry.new.clone()),
        )
    };
    let remap_index = index_owner.as_ref() != Some(&transfer.author);
    if remap_index {
        // Reject an unresolved checkpoint index before native registration can
        // apply its rolling history cap. Only this authenticated receipt may
        // be transferred here; unrelated entries need their typed compiler.
        validate_hashi_source_index(&transfer.author, network, transfer.tx_hash, incoming_hash)?;
    }
    let (_, was_prepared, _) = hashi_call(&approvals[0], transfer)?;
    if was_prepared {
        if eth_bridge::Requests::<Runtime>::get(network, incoming_hash)
            != Some(eth_bridge::requests::OffchainRequest::incoming(
                original.clone(),
            ))
            || eth_bridge::RequestStatuses::<Runtime>::get(network, incoming_hash)
                != Some(eth_bridge::requests::RequestStatus::Pending)
            || eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(network, transfer.tx_hash)
                != incoming_hash
        {
            return Err(RecoveryError::WrongPreimage);
        }
    }
    let resolution = eth_bridge::recovery::HashiResolution {
        block: System::block_number(),
        request_hash: sp_io::hashing::blake2_256(&transfer.encode()),
        beneficiary: entry.beneficiary.clone(),
        excluded: entry.excluded,
        consumed: false,
    };
    sp_io::storage::set(
        eth_bridge::recovery::HASHI_RESOLUTION_KEY,
        &resolution.encode(),
    );
    for approval in approvals {
        // Native SignedPayload binds spec/tx/genesis and the post-fork birth hash.
        let (checked, nonce) = checked_hashi_approval(approval, transfer)?;
        let sp_runtime::generic::ExtrinsicFormat::Signed(signer, _) = checked.format else {
            return Err(RecoveryError::WrongPreimage);
        };
        if signer != approval.signer
            || !crate::xor_fee_impls::validate_bridge_fee_exemption(&signer, &checked.function)
                .map_err(|_| RecoveryError::WrongPreimage)?
        {
            return Err(RecoveryError::WrongPreimage);
        }
        // Use native authentication/nonce semantics, without economic payment hooks or
        // ordinary executive scheduling. The committed operation pays mandatory weight.
        frame_system::CheckNonce::<Runtime>::validate_nonce_for_account(&signer, nonce)
            .map_err(|_| RecoveryError::Replay)?;
        frame_system::CheckNonce::<Runtime>::prepare_nonce_for_account(&signer, nonce)
            .map_err(|_| RecoveryError::Replay)?;
        checked
            .function
            .dispatch(RuntimeOrigin::signed(signer))
            .map_err(|_| RecoveryError::WrongPreimage)?;
    }
    let consumed: eth_bridge::recovery::HashiResolution<AccountId> = decode_bounded(
        &sp_io::storage::get(eth_bridge::recovery::HASHI_RESOLUTION_KEY)
            .ok_or(RecoveryError::WrongPreimage)?,
    )?;
    if !consumed.consumed
        || eth_bridge::RequestStatuses::<Runtime>::get(network, incoming_hash)
            != Some(eth_bridge::requests::RequestStatus::Done)
        || eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(network, transfer.tx_hash)
            != incoming_hash
        || eth_bridge::RequestsQueue::<Runtime>::get(network).contains(&incoming_hash)
    {
        return Err(RecoveryError::WrongPreimage);
    }
    if entry.excluded && transfer.asset_kind.is_owned() {
        // Both fresh and retained prepared owned claims reached native finalize:
        // its unreserve preserves pooled custody. Undo exactly the associated
        // before_asset_unlock decrement after the consumed/Done checks above.
        crate::BridgeProxy::recovery_restore_extinguished_hashi_owned(
            GenericNetworkId::EVMLegacy(network),
            transfer.asset_id,
            transfer.amount,
        )
        .map_err(|_| RecoveryError::WrongPreimage)?;
    }
    if was_prepared && entry.excluded && !transfer.asset_kind.is_owned() {
        crate::BridgeProxy::recovery_extinguish_prepared_sidechain(
            GenericNetworkId::EVMLegacy(network),
            transfer.asset_id,
            transfer.amount,
        )
        .map_err(|_| RecoveryError::WrongPreimage)?;
    }
    if remap_index {
        let source = validate_hashi_source_index(
            &transfer.author,
            network,
            transfer.tx_hash,
            incoming_hash,
        )?;
        if let Some(owner) = index_owner {
            let mut target = read_hashi_account_index(&owner)?;
            if source.iter().any(|id| target.contains(id)) {
                return Err(RecoveryError::WrongPreimage);
            }
            target.extend(source);
            if target.encoded_size() > MAX_CHUNK_BYTES / 4 {
                return Err(RecoveryError::ChunkBounds);
            }
            eth_bridge::AccountRequests::<Runtime>::insert(owner, target);
        }
        eth_bridge::AccountRequests::<Runtime>::remove(&transfer.author);
    }
    entry.delivered = true;
    sp_io::storage::set(
        &receipt,
        &(active.lineage, active.policy, active.operation, &*entry).encode(),
    );
    sp_io::storage::set(ACTIVE_KEY, &active.encode());
    Ok(())
}

/// Query attribution may follow a proved replacement; signed native requests
/// remain exact historical evidence. This is called only inside the committed,
/// transactional incoming executive, after native authentication for writes.
fn read_hashi_account_index(owner: &AccountId) -> Result<Vec<(u32, H256)>, RecoveryError> {
    let key = eth_bridge::AccountRequests::<Runtime>::hashed_key_for(owner);
    let Some(length) = sp_io::storage::read(&key, &mut [], 0) else {
        return Ok(Vec::new());
    };
    if length as usize > MAX_CHUNK_BYTES / 4 {
        return Err(RecoveryError::ChunkBounds);
    }
    let value = sp_io::storage::get(&key).ok_or(RecoveryError::WrongPreimage)?;
    decode_bounded(&value)
}

fn validate_hashi_source_index(
    owner: &AccountId,
    network: u32,
    transaction: H256,
    incoming: H256,
) -> Result<Vec<(u32, H256)>, RecoveryError> {
    let source = read_hashi_account_index(owner)?;
    let mut unique = BTreeSet::new();
    if source.iter().any(|id| {
        ![(network, transaction), (network, incoming)].contains(id) || !unique.insert(*id)
    }) {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(source)
}

fn authenticated_application_origin(
    network: GenericNetworkId,
    call: &BridgeCall,
    additional: &GenericAdditionalInboundData,
) -> Result<(), RecoveryError> {
    // Exclusion still requires the native application's source, token, precision
    // and positive-amount checks. It cannot turn arbitrary signed data into an ACK.
    let valid_amount = match (network, call) {
        (
            GenericNetworkId::Sub(network),
            BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
                asset_id, amount, ..
            }),
        ) => {
            let asset: crate::AssetId = asset_id
                .clone()
                .try_into()
                .map_err(|_| RecoveryError::WrongPreimage)?;
            crate::SubstrateBridgeApp::asset_kind(network, asset)
                .ok_or(RecoveryError::WrongPreimage)?;
            let precision = crate::SubstrateBridgeApp::sidechain_precision(network, asset)
                .ok_or(RecoveryError::WrongPreimage)?;
            <Runtime as substrate_bridge_app::Config>::BalancePrecisionConverter::from_sidechain(
                &asset,
                precision,
                amount.clone(),
            )
            .is_some_and(|(value, _)| value > 0)
        }
        (
            GenericNetworkId::Sub(network),
            BridgeCall::ParachainApp(ParachainAppCall::Transfer {
                asset_id, amount, ..
            }),
        ) => {
            let asset: crate::AssetId = (*asset_id).into();
            crate::ParachainBridgeApp::asset_kind(network, asset)
                .ok_or(RecoveryError::WrongPreimage)?;
            let precision = crate::ParachainBridgeApp::sidechain_precision(network, asset)
                .ok_or(RecoveryError::WrongPreimage)?;
            <Runtime as parachain_bridge_app::Config>::BalancePrecisionConverter::from_sidechain(
                &asset, precision, *amount,
            )
            .is_some_and(|(value, _)| value > 0)
        }
        (
            GenericNetworkId::TON(_),
            BridgeCall::JettonApp(JettonAppCall::Transfer { token, amount, .. }),
        ) => {
            let token = token.address().ok_or(RecoveryError::WrongPreimage)?;
            let asset =
                crate::JettonApp::asset_by_address(token).ok_or(RecoveryError::WrongPreimage)?;
            crate::JettonApp::asset_kind(asset).ok_or(RecoveryError::WrongPreimage)?;
            let precision =
                crate::JettonApp::sidechain_precision(asset).ok_or(RecoveryError::WrongPreimage)?;
            <Runtime as jetton_app::Config>::BalancePrecisionConverter::from_sidechain(
                &asset,
                precision,
                amount.clone(),
            )
            .is_some_and(|(value, _)| value > 0)
        }
        #[cfg(feature = "wip")]
        (
            GenericNetworkId::EVM(network),
            BridgeCall::FAApp(FAAppCall::Transfer { token, amount, .. }),
        ) => {
            if crate::EVMFungibleApp::is_deprecated_token_address(network, *token) {
                return Err(RecoveryError::WrongPreimage);
            }
            let asset = crate::EVMFungibleApp::asset_by_address(network, *token)
                .ok_or(RecoveryError::WrongPreimage)?;
            crate::EVMFungibleApp::asset_kind(network, asset)
                .ok_or(RecoveryError::WrongPreimage)?;
            let precision = crate::EVMFungibleApp::sidechain_precision(network, asset)
                .ok_or(RecoveryError::WrongPreimage)?;
            <Runtime as evm_fungible_app::Config>::BalancePrecisionConverter::from_sidechain(
                &asset,
                precision,
                bridge_types::U256::from_big_endian(amount.as_bytes()),
            )
            .is_some_and(|(value, _)| value > 0)
        }
        _ => false,
    };
    if !valid_amount {
        return Err(RecoveryError::WrongPreimage);
    }
    match (network, call, additional) {
        (
            GenericNetworkId::Sub(_),
            BridgeCall::SubstrateApp(_) | BridgeCall::ParachainApp(_),
            GenericAdditionalInboundData::Sub,
        ) => Ok(()),
        (
            GenericNetworkId::TON(network),
            BridgeCall::JettonApp(_),
            GenericAdditionalInboundData::TON(additional),
        ) => {
            if crate::JettonApp::app_info() == Some((network, additional.source)) {
                Ok(())
            } else {
                Err(RecoveryError::WrongPreimage)
            }
        }
        #[cfg(feature = "wip")]
        (
            GenericNetworkId::EVM(network),
            BridgeCall::FAApp(_),
            GenericAdditionalInboundData::EVM(additional),
        ) => {
            if crate::EVMFungibleApp::app_address(network) == Some(additional.source) {
                Ok(())
            } else {
                Err(RecoveryError::WrongPreimage)
            }
        }
        _ => Err(RecoveryError::WrongPreimage),
    }
}

fn dispatch_authenticated(
    network: GenericNetworkId,
    id: MessageId,
    payload: &[u8],
    timepoint: GenericTimepoint,
    additional: &GenericAdditionalInboundData,
    forward: impl FnOnce(&[u8]) -> MessageDispatchOutcome,
) -> MessageDispatchOutcome {
    // Tombstones are permanent, independent of the temporary global worker halt.
    // A direct authenticated-origin dispatch must not bypass channel admission.
    if bridge_types::recovery::message_quarantined(network, id.hash()) {
        if sp_io::storage::exists(BRIDGE_HALT_STORAGE_KEY) {
            sp_io::storage::set(FAILED_KEY, &RecoveryError::Replay.encode());
        }
        return MessageDispatchOutcome::Failed;
    }
    if !sp_io::storage::exists(BRIDGE_HALT_STORAGE_KEY) {
        if !sp_io::storage::exists(POLICY_STORAGE_KEY) {
            // Preserve deployed pre-recovery dispatch bytes and behavior.
            return forward(payload);
        }
        // Native channels have already authenticated the original commitment and
        // consumed its nonce. Resolve only the application recipient here, so a
        // delayed authenticated message cannot recreate a retired beneficiary
        // after recovery completes. Channel IDs, proof bytes, sender, amount and
        // native application origin/token checks are unchanged.
        let resolved = (|| -> Result<Option<Vec<u8>>, RecoveryError> {
            if payload.len() > BridgeMaxMessagePayloadSize::get() as usize {
                return Err(RecoveryError::ChunkBounds);
            }
            let call = decode_call(payload)?;
            let original = match &call {
                BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
                    recipient: GenericAccount::Sora(recipient),
                    ..
                })
                | BridgeCall::ParachainApp(ParachainAppCall::Transfer { recipient, .. })
                | BridgeCall::JettonApp(JettonAppCall::Transfer { recipient, .. }) => recipient,
                #[cfg(feature = "wip")]
                BridgeCall::FAApp(FAAppCall::Transfer { recipient, .. }) => recipient,
                // Administrative messages and unsupported destination types keep
                // the native dispatcher semantics; they are not transfer remaps.
                _ => return Ok(None),
            };
            crate::incident_recovery::replacement(original)?
                .map(|beneficiary| replace_recipient(call, beneficiary))
                .transpose()
        })();
        return match resolved {
            Ok(Some(bytes)) => forward(&bytes),
            Ok(None) => forward(payload),
            Err(_) => MessageDispatchOutcome::Failed,
        };
    }
    let result = (|| -> Result<MessageDispatchOutcome, RecoveryError> {
        if sp_io::storage::get(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY).as_deref()
            != Some(System::block_number().encode().as_slice())
        {
            return Err(RecoveryError::WrongPhase);
        }
        let bytes = sp_io::storage::get(ACTIVE_KEY).ok_or(RecoveryError::WrongPhase)?;
        let mut active: ActiveOperation = decode_bounded(&bytes)?;
        let status = recovery::storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
        if status.phase != RecoveryPhase::Applying
            || status.policy != active.lineage
            || status.next_chunk != active.index
            || sp_io::storage::get(POLICY_STORAGE_KEY).as_deref() != Some(active.lineage.as_slice())
            || sp_io::storage::get(ACTIVE_POLICY_STORAGE_KEY).as_deref()
                != Some(active.policy.as_slice())
        {
            return Err(RecoveryError::WrongPolicy);
        }
        if let Some(entry) = active.controls.iter_mut().find(|entry| {
            entry.network == network
                && entry.id == id.hash()
                && entry.payload == payload
                && entry.origin_hash
                    == sp_io::hashing::blake2_256(&(timepoint, additional).encode())
        }) {
            let key = control_receipt_key(network, id.hash());
            if !matches!(network, GenericNetworkId::Sub(_))
                || !matches!(additional, GenericAdditionalInboundData::Sub)
                || entry.delivered
                || sp_io::storage::exists(&key)
                || control_disposition(&decode_call(payload)?) != Some(entry.disposition.clone())
            {
                return Err(RecoveryError::WrongPreimage);
            }
            let outcome = match entry.disposition {
                ControlDisposition::Deferred => MessageDispatchOutcome::Applied,
                ControlDisposition::VerifierPeer | ControlDisposition::NativeRegistration => {
                    forward(payload)
                }
            };
            // Native Substrate delivery consumes authenticated control messages
            // even when the local application rejects them (e.g. PeerExists).
            // Preserve that outcome and continue the batch; only admission,
            // authentication or financial-delivery failures freeze recovery.
            entry.outcome = Some(match (&entry.disposition, outcome) {
                (ControlDisposition::Deferred, _) => ControlOutcome::Deferred,
                (_, MessageDispatchOutcome::Applied) => ControlOutcome::Applied,
                (_, MessageDispatchOutcome::Failed) => ControlOutcome::Failed,
            });
            entry.delivered = true;
            sp_io::storage::set(
                &key,
                &(active.lineage, active.policy, active.operation, &*entry).encode(),
            );
            sp_io::storage::set(ACTIVE_KEY, &active.encode());
            return Ok(outcome);
        }
        let entry = active
            .transfers
            .iter_mut()
            .find(|entry| {
                entry.network == network
                    && entry.id == id.hash()
                    && entry.payload_hash == sp_io::hashing::blake2_256(payload)
                    && entry.origin_hash
                        == sp_io::hashing::blake2_256(&(timepoint, additional).encode())
            })
            .ok_or(RecoveryError::WrongPreimage)?;
        let key = receipt_key(network, id.hash());
        if entry.delivered || sp_io::storage::exists(&key) {
            return Err(RecoveryError::Replay);
        }
        let call = decode_call(payload)?;
        authenticated_application_origin(network, &call, additional)?;
        let outcome = if entry.excluded {
            if matches!(
                &call,
                BridgeCall::SubstrateApp(SubstrateAppCall::Transfer { .. })
            ) {
                let GenericNetworkId::Sub(network) = network else {
                    return Err(RecoveryError::WrongPreimage);
                };
                if substrate_bridge_app::DeferredTransferResults::<Runtime>::contains_key(
                    network,
                    id.hash(),
                ) {
                    return Err(RecoveryError::Replay);
                }
                // The source debit remains consumed. Failed would authorize a refund.
                substrate_bridge_app::DeferredTransferResults::<Runtime>::insert(
                    network,
                    id.hash(),
                    MessageStatus::Done,
                );
            }
            MessageDispatchOutcome::Applied
        } else {
            forward(&replace_recipient(call, entry.beneficiary.clone())?)
        };
        if outcome == MessageDispatchOutcome::Failed {
            return Err(RecoveryError::WrongPreimage);
        }
        entry.delivered = true;
        sp_io::storage::set(
            &key,
            &(active.lineage, active.policy, active.operation, &*entry).encode(),
        );
        sp_io::storage::set(ACTIVE_KEY, &active.encode());
        Ok(outcome)
    })();
    result.unwrap_or_else(|error| {
        sp_io::storage::set(FAILED_KEY, &error.encode());
        MessageDispatchOutcome::Failed
    })
}

pub struct RecoveryGenericMessageDispatch;
impl MessageDispatch<Runtime, GenericNetworkId, MessageId, GenericAdditionalInboundData>
    for RecoveryGenericMessageDispatch
{
    fn dispatch(
        network: GenericNetworkId,
        id: MessageId,
        timepoint: GenericTimepoint,
        payload: &[u8],
        additional: GenericAdditionalInboundData,
    ) -> MessageDispatchOutcome {
        dispatch_authenticated(
            network,
            id.clone(),
            payload,
            timepoint,
            &additional,
            |resolved| Dispatch::dispatch(network, id, timepoint, resolved, additional.clone()),
        )
    }
    fn dispatch_weight(payload: &[u8]) -> Weight {
        let original = Dispatch::dispatch_weight(payload);
        if sp_io::storage::exists(POLICY_STORAGE_KEY) {
            // Includes permanent tombstones and the authenticated recipient
            // resolver: at most 256 payload bytes and thirteen static-map comparisons
            // for 4,096 replacements, with three extra policy reads. Historical
            // no-policy weights remain byte-for-byte compatible.
            original
                .saturating_add(Weight::from_parts(8_000_000, 20_480))
                .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads(15))
        } else {
            original
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn successful_dispatch_event(id: MessageId) -> Option<crate::RuntimeEvent> {
        Dispatch::successful_dispatch_event(id)
    }
}

pub struct RecoverySubstrateMessageDispatch;
impl MessageDispatch<Runtime, SubNetworkId, MessageId, ()> for RecoverySubstrateMessageDispatch {
    fn dispatch(
        network: SubNetworkId,
        id: MessageId,
        timepoint: GenericTimepoint,
        payload: &[u8],
        _: (),
    ) -> MessageDispatchOutcome {
        dispatch_authenticated(
            network.into(),
            id.clone(),
            payload,
            timepoint,
            &GenericAdditionalInboundData::Sub,
            |resolved| SubstrateDispatch::dispatch(network, id, timepoint, resolved, ()),
        )
    }
    fn dispatch_weight(payload: &[u8]) -> Weight {
        let original = SubstrateDispatch::dispatch_weight(payload);
        if sp_io::storage::exists(POLICY_STORAGE_KEY) {
            // Includes permanent tombstones and the authenticated recipient
            // resolver: at most 256 payload bytes and thirteen static-map comparisons
            // for 4,096 replacements, with three extra policy reads. Historical
            // no-policy weights remain byte-for-byte compatible.
            original
                .saturating_add(Weight::from_parts(8_000_000, 20_480))
                .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads(15))
        } else {
            original
        }
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn successful_dispatch_event(id: MessageId) -> Option<crate::RuntimeEvent> {
        SubstrateDispatch::successful_dispatch_event(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bridge_types::traits::BridgeOriginOutput;
    use bridge_types::{
        substrate::BridgeMessage,
        types::{AssetKind, AuxiliaryDigest, AuxiliaryDigestItem, CallOriginOutput},
        GenericAssetId, GenericBalance,
    };
    use common::recovery::AccountReplacement;
    use frame_support::assert_ok;
    use sp_core::{ecdsa, sr25519, Pair};
    use sp_runtime::traits::{Hash, Keccak256};

    pub(super) fn fixture(recipient: AccountId) -> (RecoveryPolicy, IncomingOperation, Vec<u8>) {
        let network = SubNetworkId::Liberland;
        let call = BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
            sender: GenericAccount::Liberland(AccountId::new([6; 32])),
            recipient: GenericAccount::Sora(recipient),
            asset_id: GenericAssetId::Sora(common::XOR.into()),
            amount: GenericBalance::Substrate(1_000_000_000_000_000_000),
        });
        let payload = call.encode();
        let commitment: Commitment = GenericCommitment::Sub(bridge_types::substrate::Commitment {
            nonce: 1,
            messages: vec![BridgeMessage {
                payload: payload.clone().try_into().unwrap(),
                timepoint: GenericTimepoint::Sora(1),
            }]
            .try_into()
            .unwrap(),
        });
        let digest = AuxiliaryDigest {
            logs: vec![AuxiliaryDigestItem::Commitment(
                ThisNetworkId::get(),
                commitment.hash(),
            )],
        };
        let keys: Vec<_> = (1u8..=4)
            .map(|seed| ecdsa::Pair::from_seed(&[seed; 32]))
            .collect();
        let peers: BTreeSet<_> = keys.iter().map(Pair::public).collect();
        multisig_verifier::PeerKeys::<Runtime>::insert(GenericNetworkId::Sub(network),
            sp_runtime::BoundedBTreeSet::<_, <Runtime as multisig_verifier::Config>::MaxPeers>::try_from(peers).unwrap());
        let proof = MultiProof::Multisig(multisig_verifier::Proof {
            digest: digest.clone(),
            proof: keys
                .iter()
                .map(|key| key.sign_prehashed(Keccak256::hash_of(&digest).as_fixed_bytes()))
                .collect(),
        });
        let operation = IncomingOperation {
            index: 0,
            channel: IncomingChannel::Substrate {
                network,
                commitment,
                proof,
            },
            finalized_source_evidence: [3; 32],
            registries: vec![
                NetworkAttackerRegistry {
                    network: network.into(),
                    accounts: vec![],
                    no_known_attackers: true,
                    attestation: [1; 32],
                },
                NetworkAttackerRegistry {
                    network: ThisNetworkId::get(),
                    accounts: vec![GenericAccount::Sora(AccountId::new(
                        recovery::ATTACKER_ACCOUNT,
                    ))],
                    no_known_attackers: false,
                    attestation: [2; 32],
                },
            ],
        };
        let policy = RecoveryPolicy {
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
        };
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 1);
        let origin: RuntimeOrigin = dispatch::RawOrigin::new(CallOriginOutput::new(
            network,
            H256::zero(),
            GenericTimepoint::Unknown,
            (),
        ))
        .into();
        assert_ok!(crate::SubstrateBridgeApp::finalize_asset_registration(
            origin,
            common::XOR,
            GenericAssetId::Sora(common::XOR.into()),
            AssetKind::Sidechain,
            18
        ));
        (policy, operation, payload)
    }

    pub(super) fn sign_hashi(approval: &mut HashiPeerApproval, key: &sr25519::Pair) {
        let call = decode_runtime_call(&approval.call).unwrap();
        let (extra, _, _) = hashi_extra(&approval.extra).unwrap();
        approval.signature = sp_runtime::generic::SignedPayload::new(call, extra)
            .unwrap()
            .using_encoded(|payload| key.sign(payload))
            .into();
    }

    pub(super) fn hashi_fixture_with_kind(
        attacker: bool,
        kind: eth_bridge::requests::AssetKind,
    ) -> (RecoveryPolicy, IncomingOperation, Vec<sr25519::Pair>) {
        let (mut policy, mut operation, _) = fixture(AccountId::new([7; 32]));
        let network = crate::GetEthNetworkId::get();
        let bridge = AccountId::new([55; 32]);
        let keys: Vec<_> = (10u8..14)
            .map(|seed| sr25519::Pair::from_seed(&[seed; 32]))
            .collect();
        let peers: Vec<AccountId> = keys.iter().map(|key| key.public().into()).collect();
        for account in peers.iter().chain(core::iter::once(&bridge)) {
            System::inc_providers(account);
        }
        eth_bridge::BridgeAccount::<Runtime>::insert(network, bridge.clone());
        let mut status_key =
            frame_support::storage::storage_prefix(b"EthBridge", b"BridgeStatuses").to_vec();
        status_key.extend(sp_io::hashing::twox_64(&network.encode()));
        status_key.extend(network.encode());
        sp_io::storage::set(&status_key, &eth_bridge::BridgeStatus::Initialized.encode());
        eth_bridge::Peers::<Runtime>::insert(
            network,
            peers.iter().cloned().collect::<BTreeSet<_>>(),
        );
        bridge_multisig::Accounts::<Runtime>::insert(
            &bridge,
            bridge_multisig::MultisigAccount::new(peers.clone()),
        );
        assert_ok!(crate::Assets::register_asset_id(
            bridge.clone(),
            common::DAI,
            common::AssetSymbol(b"DAI".to_vec()),
            common::AssetName(b"Dai".to_vec()),
            18,
            if kind.is_owned() {
                2_000_000_000_000_000_000
            } else {
                0
            },
            true,
            common::AssetType::Regular,
            None,
            None,
        ));
        let mut asset_key =
            frame_support::storage::storage_prefix(b"EthBridge", b"RegisteredAsset").to_vec();
        asset_key.extend(sp_io::hashing::twox_64(&network.encode()));
        asset_key.extend(network.encode());
        asset_key.extend(common::DAI.encode());
        sp_io::storage::set(&asset_key, &kind.encode());
        if kind.is_owned() {
            assert_ok!(
                <crate::BridgeProxy as bridge_types::traits::BridgeAssetLockChecker<
                    crate::AssetId,
                    u128,
                >>::before_asset_lock(
                    GenericNetworkId::EVMLegacy(network),
                    bridge_types::types::AssetKind::Thischain,
                    &common::DAI,
                    &2_000_000_000_000_000_000,
                )
            );
        }
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 2);
        frame_system::BlockHash::<Runtime>::insert(0, H256::repeat_byte(77));
        frame_system::BlockHash::<Runtime>::insert(
            recovery::CHECKPOINT_NUMBER + 1,
            H256::repeat_byte(44),
        );
        let timepoint = crate::BridgeMultisig::thischain_timepoint();
        let transfer = eth_bridge::requests::IncomingTransfer::<Runtime> {
            from: if attacker {
                bridge_types::H160::from(recovery::ATTACKER_EVM)
            } else {
                bridge_types::H160::repeat_byte(2)
            },
            to: AccountId::new([7; 32]),
            asset_id: common::DAI,
            asset_kind: kind,
            amount: 1_000_000_000_000_000_000,
            author: peers[0].clone(),
            tx_hash: H256::repeat_byte(66),
            at_height: 123,
            timepoint,
            network_id: network,
            should_take_fee: false,
        };
        let request = eth_bridge::requests::IncomingRequest::Transfer(transfer.clone());
        let inner = RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            load_incoming_request: eth_bridge::requests::LoadIncomingRequest::Transaction(
                eth_bridge::requests::LoadIncomingTransactionRequest::new(
                    transfer.author.clone(),
                    transfer.tx_hash,
                    timepoint,
                    eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                    network,
                ),
            ),
            incoming_request_result: Ok(request.clone()),
        });
        let era = sp_runtime::generic::Era::mortal(64, u64::from(recovery::CHECKPOINT_NUMBER + 1));
        let approvals: Vec<_> = keys
            .iter()
            .take(3)
            .enumerate()
            .map(|(index, key)| {
                let call = RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
                    id: bridge.clone(),
                    maybe_timepoint: Some(timepoint),
                    call: inner.encode(),
                    store_call: false,
                    max_weight: inner.get_dispatch_info().total_weight(),
                });
                let mut approval = HashiPeerApproval {
                    signer: peers[index].clone(),
                    call: call.encode(),
                    extra: (era, codec::Compact(0u32), codec::Compact(0u128)).encode(),
                    signature: sr25519::Signature::from_raw([0; 64]).into(),
                };
                sign_hashi(&mut approval, key);
                approval
            })
            .collect();
        operation.channel = IncomingChannel::Hashi {
            network,
            request: request.encode(),
            approvals,
        };
        operation.registries[0] = NetworkAttackerRegistry {
            network: GenericNetworkId::EVMLegacy(network),
            accounts: vec![GenericAccount::EVM(bridge_types::H160::from(
                recovery::ATTACKER_EVM,
            ))],
            no_known_attackers: false,
            attestation: [8; 32],
        };
        policy.chunk_hashes = vec![operation.commitment()];
        (policy, operation, keys)
    }

    pub(super) fn hashi_fixture(
        attacker: bool,
    ) -> (RecoveryPolicy, IncomingOperation, Vec<sr25519::Pair>) {
        hashi_fixture_with_kind(attacker, eth_bridge::requests::AssetKind::Sidechain)
    }

    #[test]
    fn recovery_hashi_offline_codec_golden() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, _) = hashi_fixture(false);
            let hex = |bytes: &[u8]| format!("0x{}", bytes.iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<alloc::string::String>());
            let IncomingChannel::Hashi { network, request, approvals } = &operation.channel else { unreachable!() };
            let template = serde_json::json!({
                "format": "sora-nexus-hashi-template-v1", "index": operation.index,
                "network": network, "request": hex(request),
                "approvals": approvals.iter().map(|approval| serde_json::json!({
                    "signer": hex(approval.signer.as_ref()), "call": hex(&approval.call), "extra": hex(&approval.extra),
                })).collect::<Vec<_>>(),
                "registries": [
                    { "network": { "evmLegacy": network }, "accounts": [{ "evm": hex(&recovery::ATTACKER_EVM) }], "noKnownAttackers": false, "attestation": hex(&[8u8;32]) },
                    { "network": { "sub": "Mainnet" }, "accounts": [{ "sora": hex(&recovery::ATTACKER_ACCOUNT) }], "noKnownAttackers": false, "attestation": hex(&[2u8;32]) },
                ],
                "finalizedSourceEvidence": hex(&operation.finalized_source_evidence),
            });
            let payloads: Vec<_> = approvals.iter().map(|approval| {
                let (extra, _, _) = hashi_extra(&approval.extra).unwrap();
                sp_runtime::generic::SignedPayload::new(decode_runtime_call(&approval.call).unwrap(), extra)
                    .unwrap().using_encoded(|payload| hex(payload))
            }).collect();
            let fixture = serde_json::json!({
                "template": template,
                "signatures": approvals.iter().map(|approval| hex(&approval.signature.encode())).collect::<Vec<_>>(),
                "operationScale": hex(&operation.encode()), "commitment": hex(&operation.commitment()),
                "signingPayloads": payloads,
                "nativeContext": { "specVersion": crate::VERSION.spec_version,
                    "transactionVersion": crate::VERSION.transaction_version,
                    "genesisHash": hex(H256::repeat_byte(77).as_bytes()),
                    "birthHash": hex(H256::repeat_byte(44).as_bytes()) },
                "fixturePolicyHash": hex(&policy.commitment()),
            });
            if let Some(path) = std::env::var_os("SORA_RECOVERY_HASHI_GOLDEN_PATH") {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();
                file.write_all(serde_json::to_string_pretty(&fixture).unwrap().as_bytes()).unwrap();
            }
            let committed = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../misc/incident_recovery/hashi_codec_fixture.json");
            if committed.exists() {
                let expected: serde_json::Value = serde_json::from_slice(&std::fs::read(committed).unwrap()).unwrap();
                // Signatures use randomized native signing; compare immutable templates,
                // native payloads and commitments while excluding witness bytes.
                for field in ["template", "commitment", "signingPayloads", "nativeContext"] {
                    assert_eq!(fixture[field], expected[field]);
                }
            }
        });
    }

    #[test]
    fn recovery_hashi_native_quorum_maps_credit_and_consumes_original_replay_identity() {
        use common::AssetInfoProvider;
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, keys) = hashi_fixture(false);
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &AccountId::new([8; 32])).unwrap(),
                    1_000_000_000_000_000_000
                );
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &AccountId::new([7; 32])).unwrap(),
                    0
                );
                assert_eq!(System::account_nonce(AccountId::from(keys[0].public())), 1);
                assert!(sp_io::storage::exists(&receipt_key(
                    GenericNetworkId::EVMLegacy(0),
                    H256::repeat_byte(66)
                )));
                assert!(!sp_io::storage::exists(
                    eth_bridge::recovery::HASHI_RESOLUTION_KEY
                ));
                assert_eq!(
                    apply_operation(&policy, &operation).unwrap_err(),
                    RecoveryError::WrongPhase
                );
            });
        });
    }

    fn hashi_fixture_with_author(
        author: AccountId,
    ) -> (RecoveryPolicy, IncomingOperation, Vec<sr25519::Pair>) {
        hashi_fixture_with_parties(author, AccountId::new([7; 32]))
    }

    fn hashi_fixture_with_parties(
        author: AccountId,
        recipient: AccountId,
    ) -> (RecoveryPolicy, IncomingOperation, Vec<sr25519::Pair>) {
        let (mut policy, mut operation, keys) = hashi_fixture(false);
        let IncomingChannel::Hashi {
            network,
            request,
            approvals,
        } = &mut operation.channel
        else {
            unreachable!()
        };
        let mut transfer = hashi_transfer(request).unwrap();
        transfer.author = author;
        transfer.to = recipient;
        let original = eth_bridge::requests::IncomingRequest::Transfer(transfer.clone());
        *request = original.encode();
        let inner = RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            load_incoming_request: eth_bridge::requests::LoadIncomingRequest::Transaction(
                eth_bridge::requests::LoadIncomingTransactionRequest::new(
                    transfer.author,
                    transfer.tx_hash,
                    transfer.timepoint,
                    eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                    *network,
                ),
            ),
            incoming_request_result: Ok(original),
        });
        for (approval, key) in approvals.iter_mut().zip(&keys) {
            let mut call = decode_runtime_call(&approval.call).unwrap();
            let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
                call: opaque,
                max_weight,
                ..
            }) = &mut call
            else {
                unreachable!()
            };
            *opaque = inner.encode();
            *max_weight = inner.get_dispatch_info().total_weight();
            approval.call = call.encode();
            sign_hashi(approval, key);
        }
        policy.chunk_hashes = vec![operation.commitment()];
        (policy, operation, keys)
    }

    #[test]
    fn recovery_hashi_mapped_author_index_moves_only_after_authentication() {
        sp_io::TestExternalities::default().execute_with(|| {
            let old = AccountId::new([7; 32]);
            let new = AccountId::new([8; 32]);
            let (policy, operation, _) = hashi_fixture_with_author(old.clone());
            let IncomingChannel::Hashi { request, .. } = &operation.channel else {
                unreachable!()
            };
            let transfer = hashi_transfer(request).unwrap();
            let original = eth_bridge::requests::OffchainRequest::incoming(
                eth_bridge::requests::IncomingRequest::Transfer(transfer.clone()),
            );
            let retained = (0u32, H256::repeat_byte(39));
            eth_bridge::AccountRequests::<Runtime>::insert(&new, vec![retained]);
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
                assert_eq!(
                    eth_bridge::AccountRequests::<Runtime>::get(&new),
                    vec![retained, (0, transfer.tx_hash), (0, original.hash())]
                );
                assert_eq!(
                    eth_bridge::Requests::<Runtime>::get(0, original.hash()),
                    Some(original)
                );
                let load = eth_bridge::Requests::<Runtime>::get(0, transfer.tx_hash).unwrap();
                let eth_bridge::requests::OffchainRequest::LoadIncoming(load) = load else {
                    unreachable!()
                };
                assert_eq!(load.author(), &old);
                let indexes = eth_bridge::AccountRequests::<Runtime>::get(&new);
                assert!(apply_operation(&policy, &operation).is_err());
                assert_eq!(eth_bridge::AccountRequests::<Runtime>::get(&new), indexes);
                assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
            });
        });
    }

    fn checkpoint_pending_hashi_load(
        operation: &IncomingOperation,
        migrated_index: bool,
    ) -> (eth_bridge::requests::OffchainRequest<Runtime>, AccountId) {
        let IncomingChannel::Hashi {
            request, network, ..
        } = &operation.channel
        else {
            unreachable!()
        };
        let transfer = hashi_transfer(request).unwrap();
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
        // Exact native nonfinancial load state retained from the checkpoint.
        eth_bridge::Requests::<Runtime>::insert(*network, load.hash(), &load);
        eth_bridge::RequestStatuses::<Runtime>::insert(
            *network,
            load.hash(),
            eth_bridge::requests::RequestStatus::Pending,
        );
        eth_bridge::RequestSubmissionHeight::<Runtime>::insert(
            *network,
            load.hash(),
            recovery::CHECKPOINT_NUMBER,
        );
        eth_bridge::RequestsQueue::<Runtime>::insert(*network, vec![load.hash()]);
        let owner = if migrated_index {
            AccountId::new([8; 32])
        } else {
            transfer.author
        };
        eth_bridge::AccountRequests::<Runtime>::insert(&owner, vec![(*network, load.hash())]);
        (load, owner)
    }

    #[test]
    fn recovery_hashi_checkpoint_pending_load_credits_replacement_once_without_rewriting_payload() {
        use common::AssetInfoProvider;
        for migrated_index in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let old = AccountId::new([7; 32]);
                let new = AccountId::new([8; 32]);
                let (policy, operation, _) = hashi_fixture_with_author(old.clone());
                let (load, _) = checkpoint_pending_hashi_load(&operation, migrated_index);
                let IncomingChannel::Hashi { request, .. } = &operation.channel else {
                    unreachable!()
                };
                let original = eth_bridge::requests::OffchainRequest::incoming(
                    decode_bounded(request).unwrap(),
                );
                crate::incident_recovery::with_test_policy(&policy, || {
                    recovery::storage::initialize(&policy).unwrap();
                    assert_ok!(apply_operation(&policy, &operation));
                    assert_eq!(
                        crate::Assets::free_balance(&common::DAI, &new).unwrap(),
                        1_000_000_000_000_000_000
                    );
                    assert_eq!(crate::Assets::free_balance(&common::DAI, &old).unwrap(), 0);
                    assert_eq!(
                        eth_bridge::Requests::<Runtime>::get(0, load.hash()),
                        Some(load.clone())
                    );
                    assert_eq!(
                        eth_bridge::Requests::<Runtime>::get(0, original.hash()),
                        Some(original.clone())
                    );
                    assert_eq!(
                        eth_bridge::RequestSubmissionHeight::<Runtime>::get(0, load.hash()),
                        recovery::CHECKPOINT_NUMBER
                    );
                    assert_eq!(
                        eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(0, load.hash()),
                        original.hash()
                    );
                    assert_eq!(
                        eth_bridge::RequestStatuses::<Runtime>::get(0, original.hash()),
                        Some(eth_bridge::requests::RequestStatus::Done)
                    );
                    assert!(eth_bridge::RequestsQueue::<Runtime>::get(0).is_empty());
                    assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
                    assert_eq!(
                        eth_bridge::AccountRequests::<Runtime>::get(&new),
                        vec![(0, load.hash()), (0, original.hash())]
                    );
                    assert!(apply_operation(&policy, &operation).is_err());
                    assert_eq!(
                        crate::Assets::free_balance(&common::DAI, &new).unwrap(),
                        1_000_000_000_000_000_000
                    );
                });
            });
        }
    }

    #[test]
    fn recovery_hashi_checkpoint_pending_load_reuse_requires_internal_authenticated_context() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, _) = hashi_fixture_with_author(AccountId::new([7; 32]));
            let (load, _) = checkpoint_pending_hashi_load(&operation, true);
            let IncomingChannel::Hashi { request, .. } = &operation.channel else {
                unreachable!()
            };
            let eth_bridge::requests::OffchainRequest::LoadIncoming(load_request) = load.clone()
            else {
                unreachable!()
            };
            let protocol = eth_bridge::Call::import_incoming_request {
                load_incoming_request: load_request.clone(),
                incoming_request_result: Ok(decode_bounded(request).unwrap()),
            };
            let bridge = AccountId::new([55; 32]);
            assert!(crate::EthBridge::validate_peer_protocol_call(&bridge, &protocol).is_err());
            assert!(crate::EthBridge::import_incoming_request(
                RuntimeOrigin::signed(bridge.clone()),
                load_request.clone(),
                Ok(decode_bounded(request).unwrap())
            )
            .is_err());
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert!(crate::EthBridge::validate_peer_protocol_call(&bridge, &protocol).is_err());
                assert!(crate::EthBridge::import_incoming_request(
                    RuntimeOrigin::signed(bridge),
                    load_request,
                    Ok(decode_bounded(request).unwrap())
                )
                .is_err());
                assert_eq!(
                    eth_bridge::Requests::<Runtime>::get(0, load.hash()),
                    Some(load)
                );
                assert!(!sp_io::storage::exists(
                    eth_bridge::recovery::HASHI_RESOLUTION_KEY
                ));
                // The same original payload succeeds only through committed
                // recovery admission and fresh, fully verified native quorum.
                assert_ok!(apply_operation(&policy, &operation));
            });
        });
    }

    #[test]
    fn recovery_hashi_checkpoint_pending_load_rejects_stale_identity_queue_replay_and_signature() {
        use common::AssetInfoProvider;
        for invalid in 0..6 {
            sp_io::TestExternalities::default().execute_with(|| {
                let old = AccountId::new([7; 32]);
                let new = AccountId::new([8; 32]);
                let (mut policy, mut operation, keys) = hashi_fixture_with_author(old.clone());
                let (load, _) = checkpoint_pending_hashi_load(&operation, true);
                match invalid {
                    0 => eth_bridge::RequestsQueue::<Runtime>::insert(0, Vec::<H256>::new()),
                    1 => eth_bridge::RequestsQueue::<Runtime>::insert(0, vec![load.hash(); 2]),
                    2 => eth_bridge::RequestStatuses::<Runtime>::insert(
                        0,
                        load.hash(),
                        eth_bridge::requests::RequestStatus::Done,
                    ),
                    3 => eth_bridge::LoadToIncomingRequestHash::<Runtime>::insert(
                        0,
                        load.hash(),
                        H256::repeat_byte(99),
                    ),
                    4 => {
                        let mut changed = load.clone();
                        let eth_bridge::requests::OffchainRequest::LoadIncoming(ref mut record) =
                            changed
                        else {
                            unreachable!()
                        };
                        *record = eth_bridge::requests::LoadIncomingRequest::Transaction(
                            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                                AccountId::new([88; 32]),
                                record.hash(),
                                record.timepoint(),
                                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                                record.network_id(),
                            ),
                        );
                        eth_bridge::Requests::<Runtime>::insert(0, load.hash(), changed);
                    }
                    5 => {
                        let IncomingChannel::Hashi { approvals, .. } = &mut operation.channel
                        else {
                            unreachable!()
                        };
                        approvals[2].signature = sr25519::Signature::from_raw([0; 64]).into();
                    }
                    _ => unreachable!(),
                }
                policy.chunk_hashes = vec![operation.commitment()];
                let retained = eth_bridge::Requests::<Runtime>::get(0, load.hash());
                let queue = eth_bridge::RequestsQueue::<Runtime>::get(0);
                let replay = eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(0, load.hash());
                crate::incident_recovery::with_test_policy(&policy, || {
                    recovery::storage::initialize(&policy).unwrap();
                    assert!(apply_operation(&policy, &operation).is_err());
                    assert_eq!(
                        eth_bridge::Requests::<Runtime>::get(0, load.hash()),
                        retained
                    );
                    assert_eq!(eth_bridge::RequestsQueue::<Runtime>::get(0), queue);
                    assert_eq!(
                        eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(0, load.hash()),
                        replay
                    );
                    assert_eq!(eth_bridge::Requests::<Runtime>::iter().count(), 1);
                    assert_eq!(
                        eth_bridge::AccountRequests::<Runtime>::get(&new),
                        vec![(0, load.hash())]
                    );
                    assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
                    assert_eq!(crate::Assets::free_balance(&common::DAI, &new).unwrap(), 0);
                    for key in keys.iter().take(3) {
                        assert_eq!(System::account_nonce(AccountId::from(key.public())), 0);
                    }
                    assert_eq!(bridge_multisig::Multisigs::<Runtime>::iter().count(), 0);
                });
            });
        }
    }

    #[test]
    fn recovery_hashi_checkpoint_pending_author_and_recipient_resolve_independently() {
        use common::AssetInfoProvider;
        for author_is_mapped in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let old = AccountId::new([7; 32]);
                let new = AccountId::new([8; 32]);
                let innocent = AccountId::new([19; 32]);
                let (author, recipient) = if author_is_mapped {
                    (old.clone(), innocent.clone())
                } else {
                    (innocent.clone(), old.clone())
                };
                let (policy, operation, _) = hashi_fixture_with_parties(author.clone(), recipient);
                let (load, _) = checkpoint_pending_hashi_load(&operation, author_is_mapped);
                let beneficiary = if author_is_mapped { &innocent } else { &new };
                let index_owner = if author_is_mapped { &new } else { &innocent };
                crate::incident_recovery::with_test_policy(&policy, || {
                    recovery::storage::initialize(&policy).unwrap();
                    assert_ok!(apply_operation(&policy, &operation));
                    assert_eq!(
                        crate::Assets::free_balance(&common::DAI, beneficiary).unwrap(),
                        1_000_000_000_000_000_000
                    );
                    assert_eq!(crate::Assets::free_balance(&common::DAI, &old).unwrap(), 0);
                    assert_eq!(
                        eth_bridge::Requests::<Runtime>::get(0, load.hash()),
                        Some(load.clone())
                    );
                    assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
                    assert!(eth_bridge::AccountRequests::<Runtime>::get(index_owner)
                        .contains(&(0, load.hash())));
                });
            });
        }
    }

    #[test]
    fn recovery_hashi_mapped_author_invalid_signature_leaves_no_index_or_credit() {
        use common::AssetInfoProvider;
        sp_io::TestExternalities::default().execute_with(|| {
            let old = AccountId::new([7; 32]);
            let new = AccountId::new([8; 32]);
            let (policy, mut operation, _) = hashi_fixture_with_author(old.clone());
            let IncomingChannel::Hashi { approvals, .. } = &mut operation.channel else {
                unreachable!()
            };
            approvals[2].signature = sr25519::Signature::from_raw([0; 64]).into();
            // Witness bytes do not change the committed template: this must
            // reject native authentication, not an unrelated policy hash.
            assert_eq!(policy.chunk_hashes, vec![operation.commitment()]);
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert!(apply_operation(&policy, &operation).is_err());
                assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
                assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&new));
                assert_eq!(crate::Assets::free_balance(&common::DAI, &new).unwrap(), 0);
                assert_eq!(eth_bridge::Requests::<Runtime>::iter().count(), 0);
            });
        });
    }

    #[test]
    fn recovery_hashi_unresolved_author_history_rolls_back_without_pruning_or_credit() {
        use common::AssetInfoProvider;
        sp_io::TestExternalities::default().execute_with(|| {
            let old = AccountId::new([7; 32]);
            let new = AccountId::new([8; 32]);
            let (policy, operation, _) = hashi_fixture_with_author(old.clone());
            let unrelated = vec![(0u32, H256::repeat_byte(39))];
            eth_bridge::AccountRequests::<Runtime>::insert(&old, &unrelated);
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert!(apply_operation(&policy, &operation).is_err());
                assert_eq!(eth_bridge::AccountRequests::<Runtime>::get(&old), unrelated);
                assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&new));
                assert_eq!(crate::Assets::free_balance(&common::DAI, &new).unwrap(), 0);
                assert_eq!(eth_bridge::Requests::<Runtime>::iter().count(), 0);
            });
        });
    }

    #[test]
    fn recovery_hashi_attacker_author_does_not_recreate_index_or_lose_innocent_credit() {
        use common::AssetInfoProvider;
        sp_io::TestExternalities::default().execute_with(|| {
            let author = AccountId::new(recovery::ATTACKER_ACCOUNT);
            let (policy, operation, _) = hashi_fixture_with_author(author.clone());
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(
                    &author
                ));
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &AccountId::new([8; 32])).unwrap(),
                    1_000_000_000_000_000_000
                );
            });
        });
    }

    #[test]
    fn recovery_hashi_destination_index_overlap_or_bound_rolls_back_native_effects() {
        use common::AssetInfoProvider;
        for oversized in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let old = AccountId::new([7; 32]);
                let new = AccountId::new([8; 32]);
                let (policy, operation, keys) = hashi_fixture_with_author(old.clone());
                let IncomingChannel::Hashi { request, .. } = &operation.channel else {
                    unreachable!()
                };
                let transfer = hashi_transfer(request).unwrap();
                let existing = if oversized {
                    vec![(0u32, H256::repeat_byte(39)); MAX_CHUNK_BYTES / 4 / 36 + 1]
                } else {
                    vec![(0u32, transfer.tx_hash)]
                };
                eth_bridge::AccountRequests::<Runtime>::insert(&new, &existing);
                if oversized {
                    assert_eq!(
                        read_hashi_account_index(&new),
                        Err(RecoveryError::ChunkBounds)
                    );
                }
                crate::incident_recovery::with_test_policy(&policy, || {
                    recovery::storage::initialize(&policy).unwrap();
                    assert!(apply_operation(&policy, &operation).is_err());
                    assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
                    assert_eq!(eth_bridge::AccountRequests::<Runtime>::get(&new), existing);
                    assert_eq!(crate::Assets::free_balance(&common::DAI, &new).unwrap(), 0);
                    assert_eq!(eth_bridge::Requests::<Runtime>::iter().count(), 0);
                    for key in keys.iter().take(3) {
                        assert_eq!(System::account_nonce(AccountId::from(key.public())), 0);
                    }
                    assert_eq!(bridge_multisig::Multisigs::<Runtime>::iter().count(), 0);
                });
            });
        }
    }

    #[test]
    fn recovery_hashi_prepared_mapped_author_preserves_already_replaced_index() {
        sp_io::TestExternalities::default().execute_with(|| {
            let old = AccountId::new([7; 32]);
            let new = AccountId::new([8; 32]);
            let (mut policy, mut operation, keys) = hashi_fixture_with_author(old.clone());
            let IncomingChannel::Hashi {
                request, approvals, ..
            } = &mut operation.channel
            else {
                unreachable!()
            };
            let original =
                decode_bounded::<eth_bridge::requests::IncomingRequest<Runtime>>(request).unwrap();
            let native = eth_bridge::requests::OffchainRequest::incoming(original.clone());
            assert_ok!(crate::EthBridge::register_incoming_request(
                RuntimeOrigin::signed(AccountId::new([55; 32])),
                original
            ));
            // Model the exact prior typed ownership move; the authenticated
            // request itself still contains its historical author and recipient.
            let index = eth_bridge::AccountRequests::<Runtime>::take(&old);
            assert_eq!(index, vec![(0, native.hash())]);
            eth_bridge::AccountRequests::<Runtime>::insert(&new, &index);
            let inner = RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
                hash: native.hash(),
                network_id: 0,
            });
            for (approval, key) in approvals.iter_mut().zip(&keys) {
                let mut call = decode_runtime_call(&approval.call).unwrap();
                let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
                    call: opaque,
                    max_weight,
                    ..
                }) = &mut call
                else {
                    unreachable!()
                };
                *opaque = inner.encode();
                *max_weight = inner.get_dispatch_info().total_weight();
                approval.call = call.encode();
                sign_hashi(approval, key);
            }
            policy.chunk_hashes = vec![operation.commitment()];
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
                assert_eq!(eth_bridge::AccountRequests::<Runtime>::get(&new), index);
                assert_eq!(
                    eth_bridge::Requests::<Runtime>::get(0, native.hash()),
                    Some(native)
                );
            });
        });
    }

    #[test]
    fn recovery_hashi_attacker_claim_is_consumed_without_mint_or_refund() {
        use common::AssetInfoProvider;
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, _) = hashi_fixture(true);
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                let supply = crate::Assets::total_issuance(&common::DAI).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), supply);
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &AccountId::new([8; 32])).unwrap(),
                    0
                );
                let IncomingChannel::Hashi { request, .. } = &operation.channel else {
                    unreachable!()
                };
                let original =
                    decode_bounded::<eth_bridge::requests::IncomingRequest<Runtime>>(request)
                        .unwrap();
                let hash = eth_bridge::requests::OffchainRequest::incoming(original).hash();
                assert_eq!(
                    crate::EthBridge::request_status(0, hash),
                    Some(eth_bridge::requests::RequestStatus::Done)
                );
            });
        });
    }

    #[test]
    fn recovery_incoming_preflight_failure_freezes_only_after_valid_hashi_witnesses() {
        for corrupt_signature in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, mut operation, _) = hashi_fixture(false);
                let (load, _) = checkpoint_pending_hashi_load(&operation, false);
                crate::incident_recovery::with_test_policy(&policy, || {
                    recovery::storage::initialize(&policy).unwrap();
                    let initial = recovery::storage::status().unwrap().unwrap();
                    let active = prepare(&operation, &policy).unwrap();
                    // Establish a valid descriptor/retained-height control before
                    // corrupting the committed state's consumption preimage.
                    assert!(audit::prepare_consumption(&operation, &active, &initial).is_ok());
                    let height_key = eth_bridge::RequestSubmissionHeight::<Runtime>::hashed_key_for(
                        0,
                        load.hash(),
                    );
                    sp_io::storage::set(&height_key, &[0]);
                    assert!(matches!(
                        audit::prepare_consumption(&operation, &active, &initial),
                        Err(RecoveryError::WrongPreimage)
                    ));
                    if corrupt_signature {
                        let IncomingChannel::Hashi { approvals, .. } = &mut operation.channel
                        else {
                            unreachable!()
                        };
                        approvals[2].signature = sr25519::Signature::from_raw([0; 64]).into();
                    }
                    assert_eq!(operation.commitment(), policy.chunk_hashes[0]);
                    let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
                    assert_eq!(
                        apply_operation(&policy, &operation),
                        Err(RecoveryError::WrongPreimage)
                    );
                    let status = recovery::storage::status().unwrap().unwrap();
                    assert_eq!(status.next_chunk, initial.next_chunk);
                    if corrupt_signature {
                        assert_eq!(status, initial);
                    } else {
                        let mut expected = initial.clone();
                        expected.phase = RecoveryPhase::Failed;
                        expected.failure = Some(RecoveryError::WrongPreimage);
                        assert_eq!(status, expected);
                        sp_io::storage::set(STATUS_STORAGE_KEY, &initial.encode());
                    }
                    // The complete state is unchanged apart from the allowed
                    // failure status: no credit, nonce, companion, receipt or
                    // pending-counter mutation can escape failed preparation.
                    assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
                });
            });
        }
    }

    #[test]
    fn recovery_incoming_encoded_weight_rejects_noncanonical_envelopes() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (_, operation, _) = hashi_fixture(false);
            let mut bytes = operation.encode();
            assert_eq!(
                incoming_encoded_weight(&bytes),
                Ok(operation.required_weight())
            );
            bytes.push(0);
            assert_eq!(
                incoming_encoded_weight(&bytes),
                Err(RecoveryError::WrongPreimage)
            );
            assert_eq!(
                incoming_encoded_weight(&[]),
                Err(RecoveryError::ChunkBounds)
            );
            assert_eq!(
                incoming_encoded_weight(&vec![0; MAX_CHUNK_BYTES + 1]),
                Err(RecoveryError::ChunkBounds)
            );
        });
    }

    #[test]
    fn recovery_hashi_bad_last_signature_rolls_back_prior_nonce_and_multisig_votes() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, mut operation, keys) = hashi_fixture(false);
            recovery::storage::initialize(&policy).unwrap();
            let original_commitment = operation.commitment();
            let IncomingChannel::Hashi { approvals, .. } = &mut operation.channel else {
                unreachable!()
            };
            approvals[2].signature = sr25519::Signature::from_raw([0; 64]).into();
            assert_eq!(operation.commitment(), original_commitment);
            assert!(apply_operation(&policy, &operation).is_err());
            for key in keys.iter().take(3) {
                assert_eq!(System::account_nonce(AccountId::from(key.public())), 0);
            }
            assert_eq!(bridge_multisig::Multisigs::<Runtime>::iter().count(), 0);
            assert_eq!(eth_bridge::Requests::<Runtime>::iter().count(), 0);
            assert_eq!(
                recovery::storage::status().unwrap().unwrap().phase,
                RecoveryPhase::Applying
            );
            assert!(!sp_io::storage::exists(
                eth_bridge::recovery::HASHI_RESOLUTION_KEY
            ));
        });
    }

    #[test]
    fn recovery_hashi_owned_attacker_claim_preserves_pooled_assets_and_issuance() {
        use common::AssetInfoProvider;
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, _) =
                hashi_fixture_with_kind(true, eth_bridge::requests::AssetKind::Thischain);
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert_eq!(
                    crate::Assets::total_issuance(&common::DAI).unwrap(),
                    2_000_000_000_000_000_000
                );
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &AccountId::new([55; 32])).unwrap(),
                    2_000_000_000_000_000_000
                );
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &AccountId::new([8; 32])).unwrap(),
                    0
                );
                assert_eq!(
                    crate::BridgeProxy::locked_assets(GenericNetworkId::EVMLegacy(0), common::DAI),
                    2_000_000_000_000_000_000
                );
            });
        });
    }

    #[test]
    fn recovery_hashi_prepared_claim_exclusion_reverses_preparation_without_refund() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation, keys) = hashi_fixture(true);
            let IncomingChannel::Hashi {
                request, approvals, ..
            } = &mut operation.channel
            else {
                unreachable!()
            };
            let request =
                decode_bounded::<eth_bridge::requests::IncomingRequest<Runtime>>(request).unwrap();
            let incoming_hash =
                eth_bridge::requests::OffchainRequest::incoming(request.clone()).hash();
            assert_ok!(crate::EthBridge::register_incoming_request(
                RuntimeOrigin::signed(AccountId::new([55; 32])),
                request
            ));
            assert_eq!(
                crate::BridgeProxy::locked_assets(GenericNetworkId::EVMLegacy(0), common::DAI),
                1_000_000_000_000_000_000
            );
            let inner = RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
                hash: incoming_hash,
                network_id: 0,
            });
            for (approval, key) in approvals.iter_mut().zip(&keys) {
                let mut call = decode_runtime_call(&approval.call).unwrap();
                let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
                    call: opaque,
                    max_weight,
                    ..
                }) = &mut call
                else {
                    unreachable!()
                };
                *opaque = inner.encode();
                *max_weight = inner.get_dispatch_info().total_weight();
                approval.call = call.encode();
                sign_hashi(approval, key);
            }
            policy.chunk_hashes = vec![operation.commitment()];
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert_eq!(
                    crate::BridgeProxy::locked_assets(GenericNetworkId::EVMLegacy(0), common::DAI),
                    0
                );
                assert_eq!(
                    crate::EthBridge::request_status(0, incoming_hash),
                    Some(eth_bridge::requests::RequestStatus::Done)
                );
            });
        });
    }

    #[test]
    fn recovery_hashi_native_failure_rolls_back_quorum_nonce_and_request_bookkeeping() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, keys) = hashi_fixture(false);
            crate::incident_recovery::with_test_policy(&policy, || {
                assets::AssetInfosV2::<Runtime>::mutate(common::DAI, |info| {
                    info.is_mintable = false
                });
                recovery::storage::initialize(&policy).unwrap();
                assert!(apply_operation(&policy, &operation).is_err());
                for key in keys.iter().take(3) {
                    assert_eq!(System::account_nonce(AccountId::from(key.public())), 0);
                }
                assert_eq!(bridge_multisig::Multisigs::<Runtime>::iter().count(), 0);
                assert_eq!(eth_bridge::Requests::<Runtime>::iter().count(), 0);
                assert_eq!(
                    recovery::storage::status().unwrap().unwrap().phase,
                    RecoveryPhase::Failed
                );
                assert!(!sp_io::storage::exists(
                    eth_bridge::recovery::HASHI_RESOLUTION_KEY
                ));
            });
        });
    }

    #[test]
    fn recovery_hashi_nonce_and_era_are_committed_and_native_checked() {
        for wrong_nonce in [true, false] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (mut policy, mut operation, keys) = hashi_fixture(false);
                let previous = operation.commitment();
                let IncomingChannel::Hashi { approvals, .. } = &mut operation.channel else {
                    unreachable!()
                };
                let birth = if wrong_nonce {
                    recovery::CHECKPOINT_NUMBER + 1
                } else {
                    recovery::CHECKPOINT_NUMBER - 1
                };
                frame_system::BlockHash::<Runtime>::insert(birth, H256::repeat_byte(44));
                approvals[0].extra = (
                    sp_runtime::generic::Era::mortal(64, u64::from(birth)),
                    codec::Compact(u32::from(wrong_nonce)),
                    codec::Compact(0u128),
                )
                    .encode();
                sign_hashi(&mut approvals[0], &keys[0]);
                assert_ne!(operation.commitment(), previous);
                policy.chunk_hashes = vec![operation.commitment()];
                recovery::storage::initialize(&policy).unwrap();
                assert!(apply_operation(&policy, &operation).is_err());
                assert_eq!(System::account_nonce(AccountId::from(keys[0].public())), 0);
                assert_eq!(bridge_multisig::Multisigs::<Runtime>::iter().count(), 0);
            });
        }
    }

    #[test]
    fn recovery_incoming_preserves_signed_payload_and_maps_only_authenticated_beneficiary() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, payload) = fixture(AccountId::new([7; 32]));
            let active = prepare(&operation, &policy).unwrap();
            let entry = &active.transfers[0];
            assert_eq!(entry.original, AccountId::new([7; 32]));
            assert_eq!(entry.beneficiary, AccountId::new([8; 32]));
            assert!(!entry.excluded);
            let before = operation.encode();
            let resolved =
                replace_recipient(decode_call(&payload).unwrap(), entry.beneficiary.clone())
                    .unwrap();
            assert_eq!(operation.encode(), before);
            let (sender, recipient) = parties(&decode_call(&resolved).unwrap()).unwrap();
            assert_eq!(sender, entry.sender);
            assert_eq!(recipient, entry.beneficiary);
        });
    }

    #[test]
    fn recovery_incoming_wrappers_preserve_exact_historical_dispatch_weights() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, _, payload) = fixture(AccountId::new([7; 32]));
            assert_eq!(
                RecoveryGenericMessageDispatch::dispatch_weight(&payload),
                Dispatch::dispatch_weight(&payload)
            );
            assert_eq!(
                RecoverySubstrateMessageDispatch::dispatch_weight(&payload),
                SubstrateDispatch::dispatch_weight(&payload)
            );
            recovery::storage::initialize(&policy).unwrap();
            assert!(SubstrateDispatch::dispatch_weight(&payload)
                .all_lte(RecoverySubstrateMessageDispatch::dispatch_weight(&payload)));
        });
    }

    #[test]
    fn recovery_incoming_attacker_registry_is_network_scoped_and_canonical() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, mut operation, _) = fixture(AccountId::new([7; 32]));
            operation.registries[0].no_known_attackers = false;
            operation.registries[0].accounts = vec![GenericAccount::EVM(bridge_types::H160::from(
                recovery::ATTACKER_EVM,
            ))];
            assert_eq!(
                validate_registries(&operation, &policy),
                Err(RecoveryError::WrongPreimage)
            );
            operation.registries[0].accounts =
                vec![GenericAccount::Liberland(AccountId::new([6; 32]))];
            assert_ok!(validate_registries(&operation, &policy));
            assert!(prepare(&operation, &policy).unwrap().transfers[0].excluded);
        });
        let old =
            GenericAccount::Parachain(xcm::VersionedLocation::V3(xcm::v3::MultiLocation::here()));
        let canonical =
            GenericAccount::Parachain(xcm::VersionedLocation::V4(xcm::v4::Location::here()));
        assert_eq!(normalized_account(&old).unwrap(), canonical);
        assert_eq!(normalized_account(&canonical).unwrap(), canonical);
    }

    #[test]
    fn recovery_incoming_exclusion_uses_native_authentication_nonce_and_done_ack() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, _) = fixture(AccountId::new(recovery::ATTACKER_ACCOUNT));
            recovery::storage::initialize(&policy).unwrap();
            let (status, _) = apply_operation(&policy, &operation).unwrap();
            assert_eq!(status.phase, RecoveryPhase::Auditing);
            assert_eq!(status.next_chunk, 1);
            let network = SubNetworkId::Liberland;
            let id = MessageId::batched(network.into(), ThisNetworkId::get(), 1, 0).hash();
            assert_eq!(
                substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(network),
                1
            );
            assert_eq!(
                substrate_bridge_app::DeferredTransferResults::<Runtime>::get(network, id),
                Some(MessageStatus::Done)
            );
            assert!(sp_io::storage::exists(&receipt_key(network.into(), id)));
            assert!(!sp_io::storage::exists(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY));
            assert!(!sp_io::storage::exists(ACTIVE_KEY));
            assert!(!sp_io::storage::exists(FAILED_KEY));
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert_eq!(
                apply_operation(&policy, &operation).unwrap_err(),
                RecoveryError::WrongPhase
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        });
    }

    #[test]
    fn recovery_incoming_bad_committed_proof_rolls_back_nonce_and_ack_then_freezes() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation, _) =
                fixture(AccountId::new(recovery::ATTACKER_ACCOUNT));
            let IncomingChannel::Substrate {
                proof: MultiProof::Multisig(proof),
                ..
            } = &mut operation.channel
            else {
                unreachable!()
            };
            proof.proof.clear();
            policy.chunk_hashes[0] = operation.commitment();
            recovery::storage::initialize(&policy).unwrap();
            assert_eq!(
                apply_operation(&policy, &operation).unwrap_err(),
                RecoveryError::WrongPreimage
            );
            assert_eq!(
                recovery::storage::status().unwrap().unwrap().phase,
                RecoveryPhase::Failed
            );
            assert_eq!(
                substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(
                    SubNetworkId::Liberland
                ),
                0
            );
            assert!(!sp_io::storage::exists(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY));
            assert!(!sp_io::storage::exists(ACTIVE_KEY));
            assert!(
                substrate_bridge_app::DeferredTransferResults::<Runtime>::iter()
                    .next()
                    .is_none()
            );
        });
    }

    #[test]
    fn recovery_incoming_uncommitted_registry_or_payload_cannot_mutate_or_freeze() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, mut operation, _) = fixture(AccountId::new([7; 32]));
            recovery::storage::initialize(&policy).unwrap();
            operation.registries[0].attestation = [8; 32];
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert_eq!(
                apply_operation(&policy, &operation).unwrap_err(),
                RecoveryError::WrongChunk
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
            operation.registries[0].no_known_attackers = false;
            assert!(validate_registries(&operation, &policy).is_err());
        });
    }

    fn opaque_anomaly_marker(network: GenericNetworkId, id: H256, malformed: bool) {
        let marker = if malformed {
            vec![0xff]
        } else {
            (
                *b"snxanom1",
                recovery::CHECKPOINT_HASH,
                [71u8; 32],
                [72u8; 32],
                [73u8; 32],
                [74u8; 32],
            )
                .encode()
        };
        sp_io::storage::set(
            &bridge_types::recovery::historical_message_key(network, id),
            &marker,
        );
    }

    #[test]
    fn recovery_hashi_quarantined_external_tx_cannot_reserve_mint_release_or_recreate() {
        for kind in [
            eth_bridge::requests::AssetKind::Sidechain,
            eth_bridge::requests::AssetKind::Thischain,
        ] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, operation, _) = hashi_fixture_with_kind(false, kind);
                let IncomingChannel::Hashi { request, .. } = &operation.channel else {
                    unreachable!()
                };
                let transfer = hashi_transfer(request).unwrap();
                recovery::storage::initialize(&policy).unwrap();
                opaque_anomaly_marker(
                    GenericNetworkId::EVMLegacy(transfer.network_id),
                    transfer.tx_hash,
                    false,
                );
                sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);
                let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
                frame_support::assert_noop!(
                    transfer.validate(),
                    eth_bridge::Error::<Runtime>::RecoveryHalted
                );
                frame_support::assert_noop!(
                    transfer.prepare(),
                    eth_bridge::Error::<Runtime>::RecoveryHalted
                );
                frame_support::assert_noop!(
                    transfer.finalize(),
                    eth_bridge::Error::<Runtime>::RecoveryHalted
                );
                frame_support::assert_noop!(
                    transfer.cancel(),
                    eth_bridge::Error::<Runtime>::RecoveryHalted
                );
                frame_support::assert_noop!(
                    transfer.unreserve(),
                    eth_bridge::Error::<Runtime>::RecoveryHalted
                );
                frame_support::assert_noop!(
                    crate::EthBridge::request_from_sidechain(
                        RuntimeOrigin::signed(transfer.author.clone()),
                        transfer.tx_hash,
                        eth_bridge::requests::IncomingRequestKind::Transaction(
                            eth_bridge::requests::IncomingTransactionRequestKind::Transfer
                        ),
                        transfer.network_id
                    ),
                    eth_bridge::Error::<Runtime>::RecoveryHalted
                );
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
                // The same authenticated peer approvals cannot bypass the marker
                // when submitted as committed recovery work under the global halt.
                sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
                assert_eq!(
                    apply_operation(&policy, &operation).unwrap_err(),
                    RecoveryError::WrongPreimage
                );
                assert_eq!(
                    recovery::storage::status().unwrap().unwrap().phase,
                    RecoveryPhase::Failed
                );
                assert!(!sp_io::storage::exists(ACTIVE_KEY));
                assert!(!sp_io::storage::exists(
                    eth_bridge::recovery::HASHI_RESOLUTION_KEY
                ));
            });
        }
    }

    #[test]
    fn recovery_incoming_quarantined_message_rolls_back_authenticated_batch_before_credit() {
        for malformed in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, operation, _) = fixture(AccountId::new([7; 32]));
                recovery::storage::initialize(&policy).unwrap();
                let network = SubNetworkId::Liberland;
                let id = MessageId::batched(network.into(), ThisNetworkId::get(), 1, 0).hash();
                opaque_anomaly_marker(network.into(), id, malformed);
                assert_eq!(
                    apply_operation(&policy, &operation).unwrap_err(),
                    RecoveryError::WrongPreimage
                );
                let status = recovery::storage::status().unwrap().unwrap();
                assert_eq!(status.phase, RecoveryPhase::Failed);
                assert_eq!(status.next_chunk, 0);
                assert_eq!(
                    substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(network),
                    0
                );
                assert_eq!(crate::Balances::free_balance(AccountId::new([8; 32])), 0);
                assert!(
                    substrate_bridge_app::DeferredTransferResults::<Runtime>::iter()
                        .next()
                        .is_none()
                );
                assert!(!sp_io::storage::exists(&receipt_key(network.into(), id)));
                assert!(!sp_io::storage::exists(ACTIVE_KEY));
                assert!(!sp_io::storage::exists(FAILED_KEY));
                assert!(!sp_io::storage::exists(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY));
            });
        }
    }

    #[test]
    fn recovery_incoming_quarantined_message_rejects_native_next_nonce_after_halt_clear() {
        for malformed in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, operation, payload) = fixture(AccountId::new([7; 32]));
                recovery::storage::initialize(&policy).unwrap();
                sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);
                let IncomingChannel::Substrate {
                    network,
                    commitment,
                    proof,
                } = &operation.channel
                else {
                    unreachable!()
                };
                // This is a correct next-nonce batch and a valid peer signature set.
                assert_ok!(SubstrateBridgeInboundChannel::validate_submission(
                    *network, commitment, proof
                ));
                assert_ok!(BridgeInboundChannel::validate_submission(
                    (*network).into(),
                    commitment,
                    proof
                ));
                let id = MessageId::batched((*network).into(), ThisNetworkId::get(), 1, 0);
                opaque_anomaly_marker((*network).into(), id.hash(), malformed);
                let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
                frame_support::assert_noop!(
                    SubstrateBridgeInboundChannel::submit(
                        RuntimeOrigin::none(),
                        *network,
                        commitment.clone(),
                        proof.clone()
                    ),
                    substrate_bridge_channel::inbound::Error::<Runtime>::RecoveryHalted
                );
                frame_support::assert_noop!(
                    BridgeInboundChannel::submit(
                        RuntimeOrigin::none(),
                        (*network).into(),
                        commitment.clone(),
                        proof.clone()
                    ),
                    bridge_channel::inbound::Error::<Runtime>::RecoveryHalted
                );
                assert_eq!(
                    dispatch_authenticated(
                        (*network).into(),
                        id,
                        &payload,
                        GenericTimepoint::Sora(1),
                        &GenericAdditionalInboundData::Sub,
                        |_| panic!("quarantined message reached forwarding")
                    ),
                    MessageDispatchOutcome::Failed
                );
                assert_eq!(
                    substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(*network),
                    0
                );
                assert_eq!(
                    bridge_channel::inbound::ChannelNonces::<Runtime>::get(GenericNetworkId::Sub(
                        *network
                    )),
                    0
                );
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
                assert!(Dispatch::dispatch_weight(&payload)
                    .all_lte(RecoveryGenericMessageDispatch::dispatch_weight(&payload)));
            });
        }
    }

    #[test]
    fn recovery_incoming_quarantined_message_rejects_direct_native_apps_before_mint_or_ack() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, _, _) = fixture(AccountId::new([7; 32]));
            recovery::storage::initialize(&policy).unwrap();
            sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);
            let network = SubNetworkId::Liberland;
            let id = H256::repeat_byte(91);
            opaque_anomaly_marker(network.into(), id, false);
            let origin: RuntimeOrigin = dispatch::RawOrigin::new(CallOriginOutput::new(
                network,
                id,
                GenericTimepoint::Sora(1),
                (),
            ))
            .into();
            let recipient = AccountId::new([8; 32]);
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            frame_support::assert_noop!(
                crate::SubstrateBridgeApp::mint(
                    origin.clone(),
                    common::XOR,
                    GenericAccount::Root,
                    recipient.clone(),
                    GenericBalance::Substrate(1_000_000_000_000_000_000)
                ),
                substrate_bridge_app::Error::<Runtime>::RecoveryHalted
            );
            frame_support::assert_noop!(
                crate::SubstrateBridgeApp::mint_inner(
                    common::XOR,
                    GenericAccount::Root,
                    recipient.clone(),
                    GenericBalance::Substrate(1_000_000_000_000_000_000),
                    network,
                    id,
                    GenericTimepoint::Sora(1)
                ),
                substrate_bridge_app::Error::<Runtime>::RecoveryHalted
            );
            frame_support::assert_noop!(
                crate::ParachainBridgeApp::mint(
                    origin,
                    common::XOR,
                    None,
                    recipient.clone(),
                    1_000_000_000_000_000_000
                ),
                parachain_bridge_app::Error::<Runtime>::RecoveryHalted
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
            let network = bridge_types::ton::TonNetworkId::Mainnet;
            opaque_anomaly_marker(network.into(), id, false);
            let address = bridge_types::ton::TonAddress::new(0, H256::repeat_byte(92));
            let origin: RuntimeOrigin = dispatch::RawOrigin::new(CallOriginOutput::new(
                GenericNetworkId::TON(network),
                id,
                GenericTimepoint::Unknown,
                GenericAdditionalInboundData::TON(bridge_types::ton::AdditionalTONInboundData {
                    source: address,
                }),
            ))
            .into();
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            frame_support::assert_noop!(
                crate::JettonApp::mint(
                    origin,
                    address.into(),
                    address.into(),
                    recipient.clone(),
                    1u128.into()
                ),
                jetton_app::Error::<Runtime>::RecoveryHalted
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
            #[cfg(feature = "wip")]
            {
                let network = bridge_types::EVMChainId::from([1u8; 32]);
                opaque_anomaly_marker(network.into(), id, false);
                let origin: RuntimeOrigin = dispatch::RawOrigin::new(CallOriginOutput::new(
                    GenericNetworkId::EVM(network),
                    id,
                    GenericTimepoint::Unknown,
                    GenericAdditionalInboundData::EVM(
                        bridge_types::evm::AdditionalEVMInboundData {
                            source: bridge_types::H160::repeat_byte(93),
                        },
                    ),
                ))
                .into();
                let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
                frame_support::assert_noop!(
                    crate::EVMFungibleApp::mint(
                        origin,
                        bridge_types::H160::repeat_byte(94),
                        bridge_types::H160::repeat_byte(95),
                        recipient,
                        1u128.into()
                    ),
                    evm_fungible_app::Error::<Runtime>::RecoveryHalted
                );
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
            }
        });
    }

    #[test]
    fn recovery_incoming_block_context_alone_cannot_authorize_a_payload() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, _, payload) = fixture(AccountId::new([7; 32]));
            recovery::storage::initialize(&policy).unwrap();
            sp_io::storage::set(
                BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
                &System::block_number().encode(),
            );
            let id = MessageId::batched(SubNetworkId::Liberland.into(), ThisNetworkId::get(), 1, 0);
            let outcome = dispatch_authenticated(
                SubNetworkId::Liberland.into(),
                id,
                &payload,
                GenericTimepoint::Sora(1),
                &GenericAdditionalInboundData::Sub,
                |_| panic!("uncommitted payload must not reach the application"),
            );
            assert_eq!(outcome, MessageDispatchOutcome::Failed);
        });
    }
    include!("incident_recovery_incoming_hashi_delayed_tests.rs");
    include!("incident_recovery_incoming_hashi_source_tests.rs");
}

/// Native HASHI ordinary delivery shares runtime138's permanent static mapping.
/// This resolves ownership only; it never changes a signed caller or opens a bridge.
pub struct PersistentAccountResolver;

impl eth_bridge::recovery::IncomingAccountResolver<AccountId> for PersistentAccountResolver {
    fn resolve(account: &AccountId) -> Result<Option<AccountId>, sp_runtime::DispatchError> {
        crate::incident_recovery::replacement(account)
            .map_err(|_| sp_runtime::DispatchError::Other("Invalid recovery account mapping"))
    }
}
