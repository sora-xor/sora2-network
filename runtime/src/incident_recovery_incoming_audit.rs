//! Typed, delay-independent checks of already authenticated maintenance deliveries.
//! Business fields are derived from the original native payload at consumption;
//! only the end timepoint comes from the actual consuming recovery block.
use super::*;
use bridge_types::types::MessageDirection;

const COMPANION_PREFIX: &[u8] = b":sora-nexus:recovery:incoming-consumption:v1:";
const AUDITED_PREFIX: &[u8] = b":sora-nexus:recovery:incoming-audited:v1:";

pub(super) fn consumption_weight() -> Weight {
    // Per transfer: up to three native registry reads, bounded receipt/sender/
    // current+old request length+value reads (eight), one ACK read, and margin.
    // Fixed reads cover context, duplicate keys, pending, native HASHI status/
    // queue/load; two new writes persist the companion and pending count.
    Weight::from_parts(
        5_000_000_000 + 100_000_000 * u64::from(BridgeMaxMessagesPerCommit::get()),
        (MAX_CHUNK_BYTES * 2) as u64 + 8_192 * u64::from(BridgeMaxMessagesPerCommit::get()),
    )
    .saturating_add(
        <Runtime as frame_system::Config>::DbWeight::get()
            .reads_writes(24 + 14 * u64::from(BridgeMaxMessagesPerCommit::get()), 2),
    )
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct IncomingRequestExpectation {
    pub source: GenericAccount,
    pub beneficiary: AccountId,
    pub asset_id: crate::AssetId,
    /// Native notifier amount (HASHI deliberately records gross amount).
    pub amount: crate::Balance,
    pub start_timepoint: GenericTimepoint,
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
pub struct IncomingAudit {
    pub index: u32,
    pub operation_index: u32,
    pub operation_commitment: [u8; 32],
    pub requests: Vec<Option<IncomingRequestExpectation>>,
    /// None selects active work without embedding a circular policy hash.
    pub prior_policy: Option<[u8; 32]>,
}
impl IncomingAudit {
    pub fn validate(&self) -> Result<(), RecoveryError> {
        // A control-only batch has no financial requests. Its nonempty, bounded
        // authenticated control journal is required by the consumption companion.
        if self.requests.len() > BridgeMaxMessagesPerCommit::get() as usize
            || self.operation_index as usize >= recovery::MAX_WORK_CHUNKS
            || self.operation_commitment == [0; 32]
            || self.prior_policy == Some([0; 32])
            || self.requests.iter().flatten().any(|r| r.amount == 0)
            || self.encoded_size() > MAX_CHUNK_BYTES
        {
            return Err(RecoveryError::ChunkBounds);
        }
        Ok(())
    }
    pub fn commitment(&self) -> [u8; 32] {
        sp_io::hashing::blake2_256(&(b"sora-nexus/incoming-audit/v1".as_slice(), self).encode())
    }
}

#[derive(Clone, Debug)]
pub struct IncomingAuditTarget {
    pub network: GenericNetworkId,
    pub id: H256,
    pub beneficiary: AccountId,
    pub excluded: bool,
}

/// Structural targets only; this function does not authenticate channel proofs.
pub fn audit_targets(
    operation: &IncomingOperation,
    policy: &RecoveryPolicy,
) -> Result<Vec<IncomingAuditTarget>, RecoveryError> {
    Ok(prepare(operation, policy)?
        .transfers
        .into_iter()
        .map(|t| IncomingAuditTarget {
            network: t.network,
            id: t.id,
            beneficiary: t.beneficiary,
            excluded: t.excluded,
        })
        .collect())
}

fn keyed(prefix: &[u8], policy: [u8; 32], operation: [u8; 32]) -> Vec<u8> {
    let mut key = prefix.to_vec();
    key.extend((policy, operation).encode());
    key
}
pub fn audit_marker_key(policy: [u8; 32], operation: [u8; 32]) -> Vec<u8> {
    keyed(AUDITED_PREFIX, policy, operation)
}
fn companion_key(policy: [u8; 32], operation: [u8; 32]) -> Vec<u8> {
    keyed(COMPANION_PREFIX, policy, operation)
}

#[derive(Encode, Decode, DecodeWithMemTracking)]
struct ConsumedTransfer {
    transfer: ExpectedTransfer,
    request: Option<IncomingRequestExpectation>,
    receipt_hash: [u8; 32],
    request_hash: Option<[u8; 32]>,
    deferred_substrate_ack: bool,
}
#[derive(Encode, Decode, DecodeWithMemTracking)]
enum NativeConsumption {
    Substrate(SubNetworkId, u64),
    Generic(GenericNetworkId, u64),
    Hashi(u32, H256, H256, LoadHeight), // network, transaction, native incoming request
}
#[derive(Encode, Decode, DecodeWithMemTracking)]
enum LoadHeight {
    Retained(Option<u32>),
    ExecutionBlock,
}
#[derive(Encode, Decode, DecodeWithMemTracking)]
pub(super) struct Consumption {
    format_version: u8,
    lineage: [u8; 32],
    policy: [u8; 32],
    operation: [u8; 32],
    index: u32,
    operation_counter: u64,
    execution_block: u32,
    native: NativeConsumption,
    transfers: Vec<ConsumedTransfer>,
    controls: Vec<ExpectedControl>,
}

fn read(key: &[u8], budget: &mut usize) -> Result<Option<Vec<u8>>, RecoveryError> {
    let Some(length) = sp_io::storage::read(key, &mut [], 0) else {
        return Ok(None);
    };
    *budget = budget
        .checked_sub(length as usize)
        .ok_or(RecoveryError::ChunkBounds)?;
    Ok(sp_io::storage::get(key).map(|v| v.to_vec()))
}
fn required<T: DecodeWithMemTracking>(key: &[u8], budget: &mut usize) -> Result<T, RecoveryError> {
    decode_bounded(&read(key, budget)?.ok_or(RecoveryError::IncompleteAudit)?)
}
fn pending() -> Result<u32, RecoveryError> {
    required(recovery::INCOMING_AUDIT_PENDING_KEY, &mut 4usize)
}

/// Use native asset registration and precision conversion while consumption-time
/// configuration is active. Later cleanup may legitimately remove that registry.
fn payload_request(
    network: GenericNetworkId,
    payload: &[u8],
    start: GenericTimepoint,
    beneficiary: AccountId,
) -> Result<IncomingRequestExpectation, RecoveryError> {
    let call = decode_call(payload)?;
    let (source, _) = parties(&call)?;
    let (asset_id, amount) = match (network, &call) {
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
            let (amount, _) = <Runtime as substrate_bridge_app::Config>::BalancePrecisionConverter::from_sidechain(&asset, precision, amount.clone()).ok_or(RecoveryError::WrongPreimage)?;
            (asset, amount)
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
            let (amount, _) = <Runtime as parachain_bridge_app::Config>::BalancePrecisionConverter::from_sidechain(&asset, precision, *amount).ok_or(RecoveryError::WrongPreimage)?;
            (asset, amount)
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
            let (amount, _) =
                <Runtime as jetton_app::Config>::BalancePrecisionConverter::from_sidechain(
                    &asset,
                    precision,
                    amount.clone(),
                )
                .ok_or(RecoveryError::WrongPreimage)?;
            (asset, amount)
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
            let (amount, _) =
                <Runtime as evm_fungible_app::Config>::BalancePrecisionConverter::from_sidechain(
                    &asset,
                    precision,
                    U256::from_big_endian(amount.as_bytes()),
                )
                .ok_or(RecoveryError::WrongPreimage)?;
            (asset, amount)
        }
        _ => return Err(RecoveryError::WrongPreimage),
    };
    if amount == 0 {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(IncomingRequestExpectation {
        source,
        beneficiary,
        asset_id,
        amount,
        start_timepoint: start,
    })
}

/// Three runtime preflight consumers derive request expectations before delivery.
/// Charge each rollback-only registration preview in addition to native dispatch.
pub(super) fn registration_preview_weight(operation: &IncomingOperation) -> Weight {
    let IncomingChannel::Substrate {
        commitment: GenericCommitment::Sub(batch),
        ..
    } = &operation.channel
    else {
        return Weight::zero();
    };
    batch
        .messages
        .iter()
        .fold(Weight::zero(), |weight, message| {
            if matches!(
                classify_substrate_control(&message.payload),
                Ok(Some(ControlDisposition::NativeRegistration))
            ) {
                weight
                    .saturating_add(SubstrateDispatch::dispatch_weight(&message.payload))
                    .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().writes(1))
            } else {
                weight
            }
        })
}

/// Derived from original payloads, never from observed BridgeProxy request values.
/// Authentication remains the native channel/HASHI execution's responsibility.
pub fn expected_requests(
    operation: &IncomingOperation,
    policy: &RecoveryPolicy,
) -> Result<Vec<Option<IncomingRequestExpectation>>, RecoveryError> {
    let active = prepare(operation, policy)?;
    derive_requests(operation, &active)
}
pub(super) fn derive_requests(
    operation: &IncomingOperation,
    active: &ActiveOperation,
) -> Result<Vec<Option<IncomingRequestExpectation>>, RecoveryError> {
    if let IncomingChannel::Substrate {
        network,
        commitment: GenericCommitment::Sub(batch),
        ..
    } = &operation.channel
    {
        // Preview only the native, non-financial finalization handlers, in the
        // original batch order. This supports finalize→mint in one commitment;
        // using checkpoint registration or the batch's final precision would be
        // wrong. All writes (including events/context) roll back. Authentication
        // and committed effects still happen only in the native channel below.
        return frame_support::storage::transactional::with_transaction_opaque_err(|| {
            let result = (|| {
                sp_io::storage::set(
                    BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
                    &System::block_number().encode(),
                );
                let mut requests = Vec::with_capacity(active.transfers.len());
                for (index, message) in batch.messages.iter().enumerate() {
                    let id = MessageId::batched(
                        (*network).into(),
                        ThisNetworkId::get(),
                        batch.nonce,
                        index as u64,
                    );
                    match control_disposition(&decode_call(&message.payload)?) {
                        Some(ControlDisposition::NativeRegistration) => {
                            // Failed finalization also has native consume-and-continue
                            // semantics; a later transfer still must pass its own checks.
                            let _ = SubstrateDispatch::dispatch(
                                *network,
                                id,
                                message.timepoint,
                                &message.payload,
                                (),
                            );
                        }
                        Some(_) => {}
                        None => {
                            let entry = active
                                .transfers
                                .get(requests.len())
                                .ok_or(RecoveryError::WrongPreimage)?;
                            if entry.id != id.hash() || entry.network != (*network).into() {
                                return Err(RecoveryError::WrongPreimage);
                            }
                            requests.push(if entry.excluded {
                                None
                            } else {
                                Some(payload_request(
                                    entry.network,
                                    &message.payload,
                                    message.timepoint,
                                    entry.beneficiary.clone(),
                                )?)
                            });
                        }
                    }
                }
                if requests.len() != active.transfers.len() {
                    return Err(RecoveryError::WrongPreimage);
                }
                Ok(requests)
            })();
            frame_support::storage::TransactionOutcome::Rollback(result)
        })
        .map_err(|_| RecoveryError::WrongPhase)?;
    }
    let mut result = Vec::with_capacity(active.transfers.len());
    for entry in &active.transfers {
        // Excluded callbacks authenticate assets/amounts in the native path, but
        // deliberately create no BridgeProxy request or credited entitlement.
        if entry.excluded {
            result.push(None);
            continue;
        }
        let request = match &operation.channel {
            IncomingChannel::Hashi { .. } | IncomingChannel::HashiAuthenticatedSource { .. } => {
                let transfer = native_hashi_transfer(&operation.channel)?;
                IncomingRequestExpectation {
                    source: GenericAccount::EVM(transfer.from),
                    beneficiary: entry.beneficiary.clone(),
                    asset_id: transfer.asset_id,
                    amount: transfer.amount,
                    start_timepoint: GenericTimepoint::EVM(transfer.at_height),
                }
            }
            IncomingChannel::Generic {
                commitment: GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(m)),
                ..
            } => payload_request(
                entry.network,
                &m.payload,
                GenericTimepoint::TON(m.transaction_id),
                entry.beneficiary.clone(),
            )?,
            IncomingChannel::Generic {
                commitment: GenericCommitment::EVM(bridge_types::evm::Commitment::Inbound(m)),
                ..
            } => payload_request(
                entry.network,
                &m.payload,
                GenericTimepoint::EVM(m.block_number),
                entry.beneficiary.clone(),
            )?,
            _ => return Err(RecoveryError::WrongPreimage),
        };
        result.push(Some(request));
    }
    Ok(result)
}
fn native(operation: &IncomingOperation) -> Result<NativeConsumption, RecoveryError> {
    Ok(match &operation.channel {
        IncomingChannel::Substrate {
            network,
            commitment: GenericCommitment::Sub(batch),
            ..
        } => NativeConsumption::Substrate(*network, batch.nonce),
        IncomingChannel::Generic {
            network,
            commitment: GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(m)),
            ..
        } => NativeConsumption::Generic(*network, m.nonce),
        IncomingChannel::Generic {
            network,
            commitment: GenericCommitment::EVM(bridge_types::evm::Commitment::Inbound(m)),
            ..
        } => NativeConsumption::Generic(*network, m.nonce),
        IncomingChannel::Hashi { .. } | IncomingChannel::HashiAuthenticatedSource { .. } => {
            let transfer = native_hashi_transfer(&operation.channel)?;
            let hash = eth_bridge::requests::OffchainRequest::incoming(
                eth_bridge::requests::IncomingRequest::Transfer(transfer.clone()),
            )
            .hash();
            NativeConsumption::Hashi(
                transfer.network_id,
                transfer.tx_hash,
                hash,
                load_height(operation)?,
            )
        }
        _ => return Err(RecoveryError::WrongPreimage),
    })
}
/// Mirror the native distinction between an exact retained load and add_request.
/// A different failed row may be resubmitted; presence alone is not retention.
fn load_height(operation: &IncomingOperation) -> Result<LoadHeight, RecoveryError> {
    let transfer = native_hashi_transfer(&operation.channel)?;
    let approvals = match &operation.channel {
        IncomingChannel::Hashi { approvals, .. }
        | IncomingChannel::HashiAuthenticatedSource { approvals, .. } => approvals,
        _ => return Err(RecoveryError::WrongPreimage),
    };
    let (_, finalize, _) = hashi_call(
        approvals.first().ok_or(RecoveryError::WrongPreimage)?,
        &transfer,
    )?;
    let expected_load = eth_bridge::requests::OffchainRequest::<Runtime>::LoadIncoming(
        eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                transfer.author.clone(),
                transfer.tx_hash,
                transfer.timepoint,
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                transfer.network_id,
            ),
        ),
    );
    let mut budget = MAX_CHUNK_BYTES;
    let existing = read(
        &eth_bridge::Requests::<Runtime>::hashed_key_for(transfer.network_id, transfer.tx_hash),
        &mut budget,
    )?;
    if finalize || existing.as_deref() == Some(expected_load.encode().as_slice()) {
        let height = read(
            &eth_bridge::RequestSubmissionHeight::<Runtime>::hashed_key_for(
                transfer.network_id,
                transfer.tx_hash,
            ),
            &mut 4usize,
        )?
        .map(|bytes| decode_bounded::<u32>(&bytes))
        .transpose()?;
        Ok(LoadHeight::Retained(height))
    } else {
        Ok(LoadHeight::ExecutionBlock)
    }
}

/// Exact fields with consumption-block-dependent values, each checked by the
/// companion audit. Does not authorize broad storage-prefix witness exclusions.
/// Must run under the guarded consumption configuration before native dispatch.
pub fn audit_dynamic_keys(
    operation: &IncomingOperation,
    policy: &RecoveryPolicy,
) -> Result<Vec<Vec<u8>>, RecoveryError> {
    let targets = audit_targets(operation, policy)?;
    let mut keys = BTreeSet::new();
    for target in targets {
        if !target.excluded {
            keys.insert(audit_request_key(
                target.network,
                &target.beneficiary,
                target.id,
            ));
        }
    }
    if matches!(
        &operation.channel,
        IncomingChannel::Hashi { .. } | IncomingChannel::HashiAuthenticatedSource { .. }
    ) && matches!(load_height(operation)?, LoadHeight::ExecutionBlock)
    {
        let transfer = native_hashi_transfer(&operation.channel)?;
        keys.insert(
            eth_bridge::RequestSubmissionHeight::<Runtime>::hashed_key_for(
                transfer.network_id,
                transfer.tx_hash,
            ),
        );
    }
    Ok(keys.into_iter().collect())
}

fn native_check(
    native: &NativeConsumption,
    block: u32,
    budget: &mut usize,
) -> Result<(), RecoveryError> {
    let valid = match native {
        NativeConsumption::Substrate(network, nonce) => {
            substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(network) >= *nonce
                && *nonce > 0
        }
        NativeConsumption::Generic(network, nonce) => {
            bridge_channel::inbound::ChannelNonces::<Runtime>::get(network) >= *nonce && *nonce > 0
        }
        NativeConsumption::Hashi(network, tx, hash, height) => {
            let queue: Vec<H256> = match read(
                &eth_bridge::RequestsQueue::<Runtime>::hashed_key_for(network),
                budget,
            )? {
                Some(bytes) => decode_bounded(&bytes)?,
                None => Vec::new(),
            };
            let expected_height = match height {
                LoadHeight::Retained(height) => *height,
                LoadHeight::ExecutionBlock => Some(block),
            };
            let actual_height = read(
                &eth_bridge::RequestSubmissionHeight::<Runtime>::hashed_key_for(network, tx),
                budget,
            )?;
            let expected_height = expected_height.map(|height| height.encode());
            actual_height == expected_height
                && eth_bridge::RequestStatuses::<Runtime>::get(network, hash)
                    == Some(eth_bridge::requests::RequestStatus::Done)
                && eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(network, tx) == *hash
                && !queue.contains(hash)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(RecoveryError::WrongPreimage)
    }
}
fn request_bytes(request: &IncomingRequestExpectation, block: u32) -> Vec<u8> {
    (
        &request.source,
        GenericAccount::Sora(request.beneficiary.clone()),
        request.asset_id,
        request.amount,
        MessageStatus::Done,
        request.start_timepoint,
        GenericTimepoint::Sora(block),
        MessageDirection::Inbound,
    )
        .encode()
}
pub fn audit_request_key(network: GenericNetworkId, owner: &AccountId, id: H256) -> Vec<u8> {
    use frame_support::StorageHasher;
    let mut key = frame_support::storage::storage_prefix(b"BridgeProxy", b"Transactions").to_vec();
    key.extend(frame_support::Blake2_128Concat::hash(
        &(network, owner).encode(),
    ));
    key.extend(frame_support::Blake2_128Concat::hash(&id.encode()));
    key
}
fn sender_key(network: GenericNetworkId, id: H256) -> Vec<u8> {
    use frame_support::StorageHasher;
    let mut key = frame_support::storage::storage_prefix(b"BridgeProxy", b"Senders").to_vec();
    key.extend(frame_support::Twox64Concat::hash(&network.encode()));
    key.extend(frame_support::Blake2_128Concat::hash(&id.encode()));
    key
}
fn request_check(
    transfer: &ExpectedTransfer,
    request: Option<&IncomingRequestExpectation>,
    block: u32,
    budget: &mut usize,
) -> Result<Option<[u8; 32]>, RecoveryError> {
    let sender = read(&sender_key(transfer.network, transfer.id), budget)?;
    let bytes = read(
        &audit_request_key(transfer.network, &transfer.beneficiary, transfer.id),
        budget,
    )?;
    if transfer.original != transfer.beneficiary
        && read(
            &audit_request_key(transfer.network, &transfer.original, transfer.id),
            budget,
        )?
        .is_some()
    {
        return Err(RecoveryError::WrongPreimage);
    }
    match request {
        None if transfer.excluded && sender.is_none() && bytes.is_none() => Ok(None),
        Some(request)
            if !transfer.excluded
                && request.beneficiary == transfer.beneficiary
                && sender.as_deref() == Some(transfer.beneficiary.encode().as_slice())
                && bytes.as_deref() == Some(request_bytes(request, block).as_slice()) =>
        {
            Ok(bytes.map(|v| sp_io::hashing::blake2_256(&v)))
        }
        _ => Err(RecoveryError::WrongPreimage),
    }
}

pub(super) fn prepare_consumption(
    operation: &IncomingOperation,
    active: &ActiveOperation,
    status: &RecoveryStatus,
) -> Result<Consumption, RecoveryError> {
    let requests = derive_requests(operation, active)?;
    // Check maximum wrapper encodability before native authentication mutates state.
    IncomingAudit {
        index: u32::MAX,
        operation_index: active.index,
        operation_commitment: active.operation,
        requests: requests.clone(),
        prior_policy: Some(active.policy),
    }
    .validate()?;
    let mut transfers = Vec::with_capacity(active.transfers.len());
    for (transfer, request) in active.transfers.iter().cloned().zip(requests) {
        let deferred_substrate_ack = if let IncomingChannel::Substrate {
            commitment: GenericCommitment::Sub(batch),
            ..
        } = &operation.channel
        {
            let message = batch
                .messages
                .iter()
                .enumerate()
                .find_map(|(index, message)| {
                    (MessageId::batched(
                        transfer.network,
                        ThisNetworkId::get(),
                        batch.nonce,
                        index as u64,
                    )
                    .hash()
                        == transfer.id)
                        .then_some(message)
                })
                .ok_or(RecoveryError::WrongPreimage)?;
            matches!(
                decode_call(&message.payload)?,
                BridgeCall::SubstrateApp(SubstrateAppCall::Transfer { .. })
            )
        } else {
            false
        };
        transfers.push(ConsumedTransfer {
            transfer,
            request,
            receipt_hash: [0; 32],
            request_hash: None,
            deferred_substrate_ack,
        });
    }
    let value = Consumption {
        format_version: 3,
        lineage: active.lineage,
        policy: active.policy,
        operation: active.operation,
        index: active.index,
        operation_counter: status.operation_counter,
        execution_block: System::block_number(),
        native: native(operation)?,
        transfers,
        controls: active.controls.clone(),
    };
    // Reserve every optional request hash in this preflight, not its currently None size.
    if value
        .encoded_size()
        .saturating_add(value.transfers.len().saturating_mul(32))
        .saturating_add(value.controls.len())
        > MAX_CHUNK_BYTES
    {
        return Err(RecoveryError::ChunkBounds);
    }
    Ok(value)
}

/// Called only inside apply_operation's existing transaction, after native proof,
/// nonce, callback and receipt completion. Never called by ordinary deliveries.
pub(super) fn record_consumption(
    mut value: Consumption,
    active: &ActiveOperation,
) -> Result<(), RecoveryError> {
    if active.lineage != value.lineage
        || active.policy != value.policy
        || active.operation != value.operation
        || active.index != value.index
        || active.transfers.len() != value.transfers.len()
        || active.controls.len() != value.controls.len()
        || System::block_number() != value.execution_block
        || sp_io::storage::get(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY).as_deref()
            != Some(value.execution_block.encode().as_slice())
    {
        return Err(RecoveryError::WrongPreimage);
    }
    let key = companion_key(value.policy, value.operation);
    if sp_io::storage::exists(&key)
        || sp_io::storage::exists(&audit_marker_key(value.policy, value.operation))
    {
        return Err(RecoveryError::Replay);
    }
    let next = pending()?
        .checked_add(1)
        .ok_or(RecoveryError::ChunkBounds)?;
    let mut budget = MAX_CHUNK_BYTES;
    native_check(&value.native, value.execution_block, &mut budget)?;
    for (stored, actual) in value.transfers.iter_mut().zip(&active.transfers) {
        stored.transfer.delivered = true;
        if stored.transfer.encode() != actual.encode() {
            return Err(RecoveryError::WrongPreimage);
        }
        let bytes = read(&receipt_key(actual.network, actual.id), &mut budget)?
            .ok_or(RecoveryError::WrongPreimage)?;
        let expected = (active.lineage, active.policy, active.operation, actual).encode();
        if bytes != expected {
            return Err(RecoveryError::WrongPreimage);
        }
        stored.receipt_hash = sp_io::hashing::blake2_256(&bytes);
        stored.request_hash = request_check(
            actual,
            stored.request.as_ref(),
            value.execution_block,
            &mut budget,
        )?;
        ack_check(stored)?;
    }
    for (stored, actual) in value.controls.iter_mut().zip(&active.controls) {
        stored.delivered = true;
        stored.outcome = actual.outcome.clone();
        if stored.encode() != actual.encode() {
            return Err(RecoveryError::WrongPreimage);
        }
    }
    validate_controls(&value, &mut budget)?;
    let bytes = value.encode();
    if bytes.len() > MAX_CHUNK_BYTES {
        return Err(RecoveryError::ChunkBounds);
    }
    sp_io::storage::set(&key, &bytes);
    sp_io::storage::set(recovery::INCOMING_AUDIT_PENDING_KEY, &next.encode());
    Ok(())
}
fn validate_controls(value: &Consumption, budget: &mut usize) -> Result<(), RecoveryError> {
    let count = value.transfers.len().saturating_add(value.controls.len());
    if count == 0 || count > BridgeMaxMessagesPerCommit::get() as usize {
        return Err(RecoveryError::ChunkBounds);
    }
    let mut ids = BTreeSet::new();
    for transfer in &value.transfers {
        if !ids.insert((transfer.transfer.network.encode(), transfer.transfer.id)) {
            return Err(RecoveryError::DuplicateEdit);
        }
    }
    for control in &value.controls {
        let NativeConsumption::Substrate(network, _) = &value.native else {
            return Err(RecoveryError::WrongPreimage);
        };
        let call = decode_call(&control.payload)?;
        if !control.delivered
            || control.network != GenericNetworkId::Sub(*network)
            || control.payload.len() > BridgeMaxMessagePayloadSize::get() as usize
            || call.encode() != control.payload
            || control_disposition(&call) != Some(control.disposition.clone())
            || !matches!(
                (&control.disposition, &control.outcome),
                (ControlDisposition::Deferred, Some(ControlOutcome::Deferred))
                    | (
                        ControlDisposition::VerifierPeer | ControlDisposition::NativeRegistration,
                        Some(ControlOutcome::Applied | ControlOutcome::Failed)
                    )
            )
            || !ids.insert((control.network.encode(), control.id))
        {
            return Err(RecoveryError::WrongPreimage);
        }
        let receipt = read(&control_receipt_key(control.network, control.id), budget)?
            .ok_or(RecoveryError::IncompleteAudit)?;
        if receipt != (value.lineage, value.policy, value.operation, control).encode() {
            return Err(RecoveryError::WrongPreimage);
        }
        if let BridgeCall::MultisigVerifier(peer) = call {
            use bridge_types::substrate::{DataSignerCall, MultisigVerifierCall};
            let acknowledgement = match peer {
                MultisigVerifierCall::AddPeer { peer } => DataSignerCall::AddPeer { peer },
                MultisigVerifierCall::RemovePeer { peer } => DataSignerCall::RemovePeer { peer },
            };
            let expected_ack = if control.outcome == Some(ControlOutcome::Applied) {
                Some(acknowledgement.encode())
            } else {
                None
            };
            if read(
                &multisig_verifier::recovery_peer_ack_key(*network, control.id),
                budget,
            )? != expected_ack
            {
                return Err(RecoveryError::WrongPreimage);
            }
        }
    }
    Ok(())
}

fn ack_check(transfer: &ConsumedTransfer) -> Result<(), RecoveryError> {
    if transfer.deferred_substrate_ack {
        let GenericNetworkId::Sub(network) = transfer.transfer.network else {
            return Err(RecoveryError::WrongPreimage);
        };
        if substrate_bridge_app::DeferredTransferResults::<Runtime>::get(
            network,
            transfer.transfer.id,
        ) != Some(MessageStatus::Done)
        {
            return Err(RecoveryError::WrongPreimage);
        }
    }
    Ok(())
}

/// Fixed bounded charge also used before execution by the maintenance executive,
/// so a committed failing audit is charged as well as a successful audit.
pub fn incoming_audit_weight() -> Weight {
    let n = u64::from(BridgeMaxMessagesPerCommit::get());
    Weight::from_parts(
        5_000_000_000 + 100_000_000 * n,
        (MAX_CHUNK_BYTES * 2) as u64 + 8_192 * n,
    )
    .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(24 + 12 * n, 3))
}

/// Read-only validation against the actual recovery storage context. This is
/// not a finality certificate; callers must authenticate the state and code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedIncomingAudit {
    pub lineage: [u8; 32],
    pub active_policy: [u8; 32],
    pub consumed_policy: [u8; 32],
    pub operation_index: u32,
    pub operation_commitment: [u8; 32],
    pub operation_counter: u64,
    pub consumption_counter: u64,
    pub execution_block: u32,
    pub audit_index: u32,
    pub pending_before: u32,
}

fn audit_context(
    policy: &RecoveryPolicy,
    bytes: &[u8],
) -> Result<(IncomingAudit, RecoveryStatus, u32, Weight), RecoveryError> {
    let audit: IncomingAudit = decode_bounded(bytes)?;
    audit.validate()?;
    policy.validate()?;
    let status = recovery::storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
    let active_policy = policy.commitment();
    if status.format_version != 1
        || sp_io::storage::get(ACTIVE_POLICY_STORAGE_KEY).as_deref()
            != Some(active_policy.as_slice())
        || sp_io::storage::get(POLICY_STORAGE_KEY).as_deref() != Some(status.policy.as_slice())
    {
        return Err(RecoveryError::WrongPolicy);
    }
    if status.phase != RecoveryPhase::Auditing
        || System::block_number() <= recovery::CHECKPOINT_NUMBER
        || sp_io::storage::get(BRIDGE_HALT_STORAGE_KEY).as_deref() != Some(&[1])
        || recovery::TRANSIENT_CONTEXT_KEYS
            .iter()
            .any(|key| sp_io::storage::exists(key))
    {
        return Err(RecoveryError::WrongPhase);
    }
    let cursor: u32 = required(recovery::AUDIT_CURSOR_STORAGE_KEY, &mut 4usize)?;
    if audit.index != cursor
        || policy.audit_chunk_hashes.get(cursor as usize) != Some(&audit.commitment())
    {
        return Err(RecoveryError::WrongChunk);
    }
    let weight = incoming_audit_weight();
    if !weight.all_lte(<Runtime as frame_system::Config>::BlockWeights::get().max_block) {
        return Err(RecoveryError::ChunkBounds);
    }
    Ok((audit, status, cursor, weight))
}

// Read-only companion/receipt checks shared by accepted-audit inspection and native balance audits.
fn validate_consumption_at(
    consumed_policy: [u8; 32],
    audit: &IncomingAudit,
    status: &RecoveryStatus,
    latest_consumption_block: u32,
) -> Result<Consumption, RecoveryError> {
    let mut budget = MAX_CHUNK_BYTES * 2;
    let value: Consumption = required(
        &companion_key(consumed_policy, audit.operation_commitment),
        &mut budget,
    )?;
    if value.format_version != 3
        || value.lineage != status.policy
        || value.policy != consumed_policy
        || value.operation != audit.operation_commitment
        || value.index != audit.operation_index
        || value.operation_counter > status.operation_counter
        || (audit.prior_policy.is_none() && value.operation_counter != status.operation_counter)
        || value.execution_block <= recovery::CHECKPOINT_NUMBER
        || value.execution_block > latest_consumption_block
        || value.transfers.len() != audit.requests.len()
    {
        return Err(RecoveryError::WrongPreimage);
    }
    native_check(&value.native, value.execution_block, &mut budget)?;
    validate_controls(&value, &mut budget)?;
    for (entry, expected) in value.transfers.iter().zip(&audit.requests) {
        if &entry.request != expected || !entry.transfer.delivered {
            return Err(RecoveryError::WrongPreimage);
        }
        let receipt = read(
            &receipt_key(entry.transfer.network, entry.transfer.id),
            &mut budget,
        )?
        .ok_or(RecoveryError::IncompleteAudit)?;
        if sp_io::hashing::blake2_256(&receipt) != entry.receipt_hash
            || receipt
                != (
                    value.lineage,
                    value.policy,
                    value.operation,
                    &entry.transfer,
                )
                    .encode()
            || request_check(
                &entry.transfer,
                expected.as_ref(),
                value.execution_block,
                &mut budget,
            )? != entry.request_hash
        {
            return Err(RecoveryError::WrongPreimage);
        }
        ack_check(entry)?;
    }
    Ok(value)
}

#[cfg(feature = "std")]
fn validate_completed_consumption(
    policy: &RecoveryPolicy,
    audit: &IncomingAudit,
    status: &RecoveryStatus,
    latest_consumption_block: u32,
) -> Result<Consumption, RecoveryError> {
    validate_consumption_at(
        audit.prior_policy.unwrap_or(policy.commitment()),
        audit,
        status,
        latest_consumption_block,
    )
}

/// A permanent accepted marker, never caller flags, authorizes reuse of this native receipt.
/// Read-only and restricted to the current original lineage and monotonic repair counter.
pub(super) fn validate_audited_balance_source(
    policy: &RecoveryPolicy,
    audit: &IncomingAudit,
    historical_audit_policy: Option<[u8; 32]>,
) -> Result<ValidatedIncomingAudit, RecoveryError> {
    audit.validate()?;
    let status = recovery::storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
    let audited_policy = historical_audit_policy.unwrap_or(policy.commitment());
    let consumed_policy = audit.prior_policy.unwrap_or(audited_policy);
    if historical_audit_policy == Some(policy.commitment())
        || audit.prior_policy == Some(audited_policy)
    {
        return Err(RecoveryError::WrongPolicy);
    }
    let marker: ([u8; 32], [u8; 32], [u8; 32], u64) = required(
        &audit_marker_key(consumed_policy, audit.operation_commitment),
        &mut 104usize,
    )?;
    let cursor: u32 = required(recovery::AUDIT_CURSOR_STORAGE_KEY, &mut 4usize)?;
    if marker.0 != status.policy
        || marker.1 != audited_policy
        || marker.2 != audit.commitment()
        || marker.3 > status.operation_counter
        || (historical_audit_policy.is_some() && marker.3 >= status.operation_counter)
        || (historical_audit_policy.is_none()
            && (marker.3 != status.operation_counter
                || audit.index >= cursor
                || policy.audit_chunk_hashes.get(audit.index as usize)
                    != Some(&audit.commitment())))
    {
        return Err(RecoveryError::WrongPreimage);
    }
    let mut audited_status = status.clone();
    audited_status.operation_counter = marker.3;
    let value = validate_consumption_at(
        consumed_policy,
        audit,
        &audited_status,
        System::block_number(),
    )?;
    Ok(ValidatedIncomingAudit {
        lineage: status.policy,
        active_policy: policy.commitment(),
        consumed_policy,
        operation_index: value.index,
        operation_commitment: value.operation,
        operation_counter: status.operation_counter,
        consumption_counter: value.operation_counter,
        execution_block: value.execution_block,
        audit_index: audit.index,
        pending_before: 0,
    })
}

fn validate_companion(
    policy: &RecoveryPolicy,
    audit: &IncomingAudit,
    status: &RecoveryStatus,
    cursor: u32,
) -> Result<ValidatedIncomingAudit, RecoveryError> {
    let active_policy = policy.commitment();
    let consumed_policy = audit.prior_policy.unwrap_or(active_policy);
    if audit.prior_policy == Some(active_policy)
        || (audit.prior_policy.is_none()
            && policy.chunk_hashes.get(audit.operation_index as usize)
                != Some(&audit.operation_commitment))
    {
        return Err(RecoveryError::WrongPolicy);
    }
    let marker = audit_marker_key(consumed_policy, audit.operation_commitment);
    if sp_io::storage::exists(&marker) {
        return Err(RecoveryError::Replay);
    }
    let mut budget = MAX_CHUNK_BYTES * 2;
    let value: Consumption = required(
        &companion_key(consumed_policy, audit.operation_commitment),
        &mut budget,
    )?;
    if value.format_version != 3
        || value.lineage != status.policy
        || value.policy != consumed_policy
        || value.operation != audit.operation_commitment
        || value.index != audit.operation_index
        || value.operation_counter > status.operation_counter
        || (audit.prior_policy.is_none() && value.operation_counter != status.operation_counter)
        || value.execution_block <= recovery::CHECKPOINT_NUMBER
        || value.execution_block > System::block_number()
        || value.transfers.len() != audit.requests.len()
    {
        return Err(RecoveryError::WrongPreimage);
    }
    native_check(&value.native, value.execution_block, &mut budget)?;
    validate_controls(&value, &mut budget)?;
    for (entry, expected) in value.transfers.iter().zip(&audit.requests) {
        if &entry.request != expected || !entry.transfer.delivered {
            return Err(RecoveryError::WrongPreimage);
        }
        let receipt = read(
            &receipt_key(entry.transfer.network, entry.transfer.id),
            &mut budget,
        )?
        .ok_or(RecoveryError::IncompleteAudit)?;
        if sp_io::hashing::blake2_256(&receipt) != entry.receipt_hash
            || receipt
                != (
                    value.lineage,
                    value.policy,
                    value.operation,
                    &entry.transfer,
                )
                    .encode()
            || request_check(
                &entry.transfer,
                expected.as_ref(),
                value.execution_block,
                &mut budget,
            )? != entry.request_hash
        {
            return Err(RecoveryError::WrongPreimage);
        }
        ack_check(entry)?;
    }
    let pending_before = pending()?;
    pending_before
        .checked_sub(1)
        .ok_or(RecoveryError::IncompleteAudit)?;
    cursor.checked_add(1).ok_or(RecoveryError::ChunkBounds)?;
    Ok(ValidatedIncomingAudit {
        lineage: status.policy,
        active_policy,
        consumed_policy,
        operation_index: value.index,
        operation_commitment: value.operation,
        operation_counter: status.operation_counter,
        consumption_counter: value.operation_counter,
        execution_block: value.execution_block,
        audit_index: cursor,
        pending_before,
    })
}

/// Shares every context and companion check with execution, including marker
/// absence, bounded native/request/receipt checks and nonzero pending count.
/// Neither success nor failure writes storage or freezes recovery.
pub fn validate_incoming_audit(
    policy: &RecoveryPolicy,
    bytes: &[u8],
) -> Result<ValidatedIncomingAudit, RecoveryError> {
    let (audit, status, cursor, _) = audit_context(policy, bytes)?;
    validate_companion(policy, &audit, &status, cursor)
}

/// Recheck an already accepted audit against completed recovery state. This
/// host-only inspector never executes an audit, changes its cursor, or permits
/// an on-chain replay. The caller must supply the audit policy, counter and
/// block from the same authenticated replay, not from a serialized assertion.
#[cfg(feature = "std")]
pub fn validate_completed_incoming_audit(
    final_policy: &RecoveryPolicy,
    audited_policy: &RecoveryPolicy,
    bytes: &[u8],
    audited_counter: u64,
    audited_block: u32,
) -> Result<ValidatedIncomingAudit, RecoveryError> {
    final_policy.validate()?;
    audited_policy.validate()?;
    let audit: IncomingAudit = decode_bounded(bytes)?;
    audit.validate()?;
    if final_policy.checkpoint_hash != audited_policy.checkpoint_hash
        || final_policy.checkpoint_state_root != audited_policy.checkpoint_state_root
        || final_policy.replacements != audited_policy.replacements
        || final_policy.council != audited_policy.council
        || audited_policy.audit_chunk_hashes.get(audit.index as usize) != Some(&audit.commitment())
        || audit.prior_policy == Some(audited_policy.commitment())
        || (audit.prior_policy.is_none()
            && audited_policy
                .chunk_hashes
                .get(audit.operation_index as usize)
                != Some(&audit.operation_commitment))
    {
        return Err(RecoveryError::WrongPolicy);
    }
    let status = recovery::storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
    if status.format_version != 1
        || status.phase != RecoveryPhase::Complete
        || status.failure.is_some()
        || status.operation_counter < audited_counter
        || sp_io::storage::get(ACTIVE_POLICY_STORAGE_KEY).as_deref()
            != Some(final_policy.commitment().as_slice())
        || sp_io::storage::get(POLICY_STORAGE_KEY).as_deref() != Some(status.policy.as_slice())
        || sp_io::storage::get(BRIDGE_HALT_STORAGE_KEY).as_deref() != Some(&[1])
        || pending()? != 0
        || required::<u32>(recovery::AUDIT_CURSOR_STORAGE_KEY, &mut 4usize)? as usize
            != final_policy.audit_chunk_hashes.len()
        || audited_block <= recovery::CHECKPOINT_NUMBER
        || audited_block > System::block_number()
        || recovery::TRANSIENT_CONTEXT_KEYS
            .iter()
            .any(|key| sp_io::storage::exists(key))
    {
        return Err(RecoveryError::WrongPhase);
    }
    let consumed_policy = audit.prior_policy.unwrap_or(audited_policy.commitment());
    let marker: ([u8; 32], [u8; 32], [u8; 32], u64) = required(
        &audit_marker_key(consumed_policy, audit.operation_commitment),
        &mut 104usize,
    )?;
    if marker
        != (
            status.policy,
            audited_policy.commitment(),
            audit.commitment(),
            audited_counter,
        )
    {
        return Err(RecoveryError::WrongPreimage);
    }
    // Use the actual accepted audit's counter and block as upper bounds, even
    // after later council repairs advance the final state counter.
    let mut audited_status = status.clone();
    audited_status.operation_counter = audited_counter;
    let value =
        validate_completed_consumption(audited_policy, &audit, &audited_status, audited_block)?;
    Ok(ValidatedIncomingAudit {
        lineage: status.policy,
        active_policy: final_policy.commitment(),
        consumed_policy,
        operation_index: value.index,
        operation_commitment: value.operation,
        operation_counter: status.operation_counter,
        consumption_counter: value.operation_counter,
        execution_block: value.execution_block,
        audit_index: audit.index,
        pending_before: 0,
    })
}

pub fn audit_encoded_with_policy(
    policy: &RecoveryPolicy,
    bytes: &[u8],
) -> Result<(RecoveryStatus, Weight), RecoveryError> {
    let (audit, mut status, cursor, weight) = audit_context(policy, bytes)?;
    let verified = match validate_companion(policy, &audit, &status, cursor) {
        Ok(verified) => verified,
        Err(error) => {
            status.phase = RecoveryPhase::Failed;
            status.failure = Some(error);
            sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
            return Err(error);
        }
    };
    // All fallible validation precedes the same three protected writes.
    let marker = audit_marker_key(verified.consumed_policy, audit.operation_commitment);
    sp_io::storage::set(
        &marker,
        &(
            status.policy,
            verified.active_policy,
            audit.commitment(),
            status.operation_counter,
        )
            .encode(),
    );
    sp_io::storage::set(
        recovery::INCOMING_AUDIT_PENDING_KEY,
        &(verified.pending_before - 1).encode(),
    );
    sp_io::storage::set(recovery::AUDIT_CURSOR_STORAGE_KEY, &(cursor + 1).encode());
    Ok((status, weight))
}

#[cfg(test)]
include!("incident_recovery_incoming_audit_tests.rs");

#[cfg(test)]
mod readonly_tests {
    use super::*;
    fn setup() -> (RecoveryPolicy, IncomingOperation, IncomingAudit) {
        let (mut policy, operation, _) =
            crate::incident_recovery_incoming::tests::fixture(AccountId::new([7; 32]));
        let owner = AccountId::new([45; 32]);
        System::inc_providers(&owner);
        crate::Assets::register_asset_id(
            owner,
            common::XOR,
            common::AssetSymbol(b"XOR".to_vec()),
            common::AssetName(b"SORA Nexus".to_vec()),
            18,
            0,
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        <crate::BridgeProxy as bridge_types::traits::BridgeAssetRegistry<
            AccountId,
            crate::AssetId,
        >>::manage_asset(SubNetworkId::Liberland.into(), common::XOR)
        .unwrap();
        let audit = IncomingAudit {
            index: 0,
            operation_index: operation.index,
            operation_commitment: operation.commitment(),
            requests: expected_requests(&operation, &policy).unwrap(),
            prior_policy: None,
        };
        policy.audit_chunk_hashes = vec![audit.commitment()];
        recovery::storage::initialize(&policy).unwrap();
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 10);
        crate::incident_recovery::with_test_policy(&policy, || {
            crate::incident_recovery_incoming::apply_operation(&policy, &operation)
        })
        .unwrap();
        (policy, operation, audit)
    }
    fn root() -> Vec<u8> {
        sp_io::storage::root(sp_runtime::StateVersion::V0)
    }
    #[test]
    fn readonly_companion_validation_matches_dispatch_and_never_writes() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, audit) = setup();
            let before = root();
            let proof = validate_incoming_audit(&policy, &audit.encode()).unwrap();
            assert_eq!(root(), before);
            assert_eq!(proof.pending_before, 1);
            assert_eq!(proof.lineage, policy.commitment());
            assert_eq!(proof.active_policy, policy.commitment());
            assert_eq!(proof.operation_commitment, operation.commitment());
            assert_eq!(proof.execution_block, recovery::CHECKPOINT_NUMBER + 10);
            audit_encoded_with_policy(&policy, &audit.encode()).unwrap();
            assert_eq!(pending().unwrap(), 0);
            let after = root();
            assert_eq!(
                validate_incoming_audit(&policy, &audit.encode()).unwrap_err(),
                RecoveryError::WrongChunk
            );
            assert_eq!(root(), after);
        });
    }
    #[test]
    fn readonly_prior_companion_binds_consuming_counter_and_preserves_failure_partition() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, operation, mut audit) = setup();
            let original = policy.commitment();
            audit.prior_policy = Some(original);
            policy.audit_chunk_hashes = vec![audit.commitment()];
            policy.evidence_commitment = [93; 32];
            sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &policy.commitment());
            let mut state = recovery::storage::status().unwrap().unwrap();
            state.operation_counter = 2;
            sp_io::storage::set(STATUS_STORAGE_KEY, &state.encode());
            System::set_block_number(System::block_number() + 100);
            let before = root();
            let proof = validate_incoming_audit(&policy, &audit.encode()).unwrap();
            assert_eq!(root(), before);
            assert_eq!(proof.consumed_policy, original);
            assert_eq!(proof.lineage, original);
            assert_eq!(proof.operation_counter, 2);
            assert_eq!(proof.consumption_counter, 0);
            let marker = companion_key(original, operation.commitment());
            let bytes = sp_io::storage::get(&marker).unwrap();
            sp_io::storage::clear(&marker);
            let broken = root();
            assert_eq!(
                validate_incoming_audit(&policy, &audit.encode()).unwrap_err(),
                RecoveryError::IncompleteAudit
            );
            assert_eq!(root(), broken);
            assert_eq!(recovery::storage::status().unwrap().unwrap(), state);
            sp_io::storage::set(&marker, &bytes);
            assert!(validate_incoming_audit(&policy, &audit.encode()).is_ok());
            sp_io::storage::clear(&marker);
            assert_eq!(
                audit_encoded_with_policy(&policy, &audit.encode()).unwrap_err(),
                RecoveryError::IncompleteAudit
            );
            assert_eq!(
                recovery::storage::status().unwrap().unwrap().phase,
                RecoveryPhase::Failed
            );
            assert_eq!(pending().unwrap(), 1);
        });
    }
    fn completed_prior() -> (RecoveryPolicy, RecoveryPolicy, IncomingAudit, u32) {
        let (mut audited, _, mut audit) = setup();
        let original = audited.commitment();
        audit.prior_policy = Some(original);
        audited.audit_chunk_hashes = vec![audit.commitment()];
        audited.evidence_commitment = [93; 32];
        sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &audited.commitment());
        let mut state = recovery::storage::status().unwrap().unwrap();
        state.operation_counter = 2;
        sp_io::storage::set(STATUS_STORAGE_KEY, &state.encode());
        let block = System::block_number();
        audit_encoded_with_policy(&audited, &audit.encode()).unwrap();
        let mut final_policy = audited.clone();
        final_policy.evidence_commitment = [94; 32];
        final_policy.audit_chunk_hashes = vec![[95; 32]];
        sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &final_policy.commitment());
        state.phase = RecoveryPhase::Complete;
        state.operation_counter = 3;
        sp_io::storage::set(STATUS_STORAGE_KEY, &state.encode());
        System::set_block_number(block + 100);
        (final_policy, audited, audit, block)
    }

    #[test]
    fn completed_prior_audit_survives_later_policy_without_replaying_or_writing() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (final_policy, audited, audit, block) = completed_prior();
            let before = root();
            let value = validate_completed_incoming_audit(
                &final_policy,
                &audited,
                &audit.encode(),
                2,
                block,
            )
            .unwrap();
            assert_eq!(value.active_policy, final_policy.commitment());
            assert_eq!(value.operation_counter, 3);
            assert_eq!(value.consumption_counter, 0);
            assert_eq!(value.pending_before, 0);
            assert_eq!(root(), before);
            // The inspector neither removes the permanent marker nor permits dispatch.
            assert!(audit_encoded_with_policy(&audited, &audit.encode()).is_err());
            assert_eq!(root(), before);
            assert_eq!(pending().unwrap(), 0);
        });
    }

    #[test]
    fn completed_prior_audit_rejects_changed_final_context_marker_and_native_state() {
        for mode in 0..10 {
            sp_io::TestExternalities::default().execute_with(|| {
                let (mut final_policy, mut audited, audit, mut block) = completed_prior();
                let mut counter = 2;
                match mode {
                    0 => final_policy.evidence_commitment = [99; 32],
                    1 => {
                        final_policy.council = vec![AccountId::new([99; 32])];
                        sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &final_policy.commitment());
                    }
                    2 => audited.evidence_commitment = [99; 32],
                    3 => counter = 1,
                    4 => block -= 1,
                    5 => sp_io::storage::set(recovery::INCOMING_AUDIT_PENDING_KEY, &1u32.encode()),
                    6 => sp_io::storage::clear(&audit_marker_key(
                        audit.prior_policy.unwrap(),
                        audit.operation_commitment,
                    )),
                    7 => substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::insert(
                        SubNetworkId::Liberland,
                        0,
                    ),
                    8 => {
                        let key =
                            companion_key(audit.prior_policy.unwrap(), audit.operation_commitment);
                        let mut budget = MAX_CHUNK_BYTES;
                        let mut value: Consumption = required(&key, &mut budget).unwrap();
                        value.operation_counter = 3;
                        sp_io::storage::set(&key, &value.encode());
                    }
                    _ => {
                        let mut budget = MAX_CHUNK_BYTES;
                        let value: Consumption = required(
                            &companion_key(audit.prior_policy.unwrap(), audit.operation_commitment),
                            &mut budget,
                        )
                        .unwrap();
                        let entry = &value.transfers[0];
                        sp_io::storage::set(
                            &audit_request_key(
                                entry.transfer.network,
                                &entry.transfer.beneficiary,
                                entry.transfer.id,
                            ),
                            &[0],
                        );
                    }
                }
                let before = root();
                assert!(
                    validate_completed_incoming_audit(
                        &final_policy,
                        &audited,
                        &audit.encode(),
                        counter,
                        block
                    )
                    .is_err(),
                    "mode {mode}"
                );
                assert_eq!(root(), before, "mode {mode}");
            });
        }
    }
    #[test]
    fn completed_same_policy_audit_can_be_rechecked_after_a_later_repair() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (audited, _, audit) = setup();
            assert!(audit.prior_policy.is_none());
            let block = System::block_number();
            audit_encoded_with_policy(&audited, &audit.encode()).unwrap();
            let mut final_policy = audited.clone();
            final_policy.evidence_commitment = [96; 32];
            sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &final_policy.commitment());
            let mut state = recovery::storage::status().unwrap().unwrap();
            state.phase = RecoveryPhase::Complete;
            state.operation_counter = 1;
            sp_io::storage::set(STATUS_STORAGE_KEY, &state.encode());
            let before = root();
            let proof = validate_completed_incoming_audit(
                &final_policy,
                &audited,
                &audit.encode(),
                0,
                block,
            )
            .unwrap();
            assert_eq!(proof.consumed_policy, audited.commitment());
            assert_eq!(proof.consumption_counter, 0);
            assert_eq!(root(), before);
            assert!(validate_completed_incoming_audit(
                &final_policy,
                &audited,
                &audit.encode(),
                1,
                block
            )
            .is_err());
            assert_eq!(root(), before);
            final_policy.replacements[0].new = AccountId::new([97; 32]);
            sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &final_policy.commitment());
            let changed = root();
            assert!(validate_completed_incoming_audit(
                &final_policy,
                &audited,
                &audit.encode(),
                0,
                block
            )
            .is_err());
            assert_eq!(root(), changed);
        });
    }
}
