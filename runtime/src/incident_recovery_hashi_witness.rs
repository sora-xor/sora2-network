//! Read-only preparation and native admission of committed HASHI witnesses.
//!
//! This module is a child of `incident_recovery_incoming`, so it reuses the
//! exact native decoders and operation admission rather than another call codec.
//! No key is loaded or used here. The caller must authenticate its finalized
//! recovery parent and exact executing substitute; matching a supplied header
//! to this state is not a finality proof. Existing HASHI peers still independently
//! validate the external source under their existing bridge trust model.

use super::{
    apply_encoded_with_policy, decode_bounded, hashi_call, hashi_extra, native_hashi_transfer,
    prepare as prepare_operation, IncomingChannel, IncomingOperation, MAX_HASHI_APPROVALS,
};
use crate::{Header, Runtime, System};
use alloc::vec::Vec;
use codec::{DecodeWithMemLimit, Encode};
use common::recovery::{self, RecoveryError, RecoveryPhase, RecoveryPolicy};
use eth_bridge_runtime_api::recovery::{
    RecoveryHashiAdmission, RecoveryHashiPayload, RecoveryHashiPreparation,
};
use frame_support::storage::TransactionOutcome;
use sp_runtime::{traits::Header as _, DispatchError, StateVersion};

const MAX_PARENT_HEADER_BYTES: usize = 16 * 1024;
const FORMAT_VERSION: u32 = 1;

struct Context {
    parent: Header,
    policy: RecoveryPolicy,
    operation: IncomingOperation,
    anchor: [u8; 32],
    stored_code_hash: [u8; 32],
    execution_number: u32,
}

fn supported_version(spec: u32, transaction: u32) -> Result<(), RecoveryError> {
    if spec != 134 || transaction != 131 {
        return Err(RecoveryError::BadCode);
    }
    Ok(())
}

fn supported_runtime() -> Result<(), RecoveryError> {
    let version = <Runtime as frame_system::Config>::Version::get();
    supported_version(version.spec_version, version.transaction_version)
}

/// Validate the untouched parent state before opening a storage transaction;
/// FRAME's transaction-depth bookkeeping itself changes the temporary root.
fn context(header: &[u8], operation: &[u8]) -> Result<Context, RecoveryError> {
    if header.is_empty() || header.len() > MAX_PARENT_HEADER_BYTES {
        return Err(RecoveryError::ChunkBounds);
    }
    let mut input = header;
    let parent = Header::decode_with_mem_limit(&mut input, MAX_PARENT_HEADER_BYTES * 4)
        .map_err(|_| RecoveryError::WrongPreimage)?;
    if !input.is_empty() || parent.encode() != header {
        return Err(RecoveryError::WrongPreimage);
    }
    if *parent.number() <= recovery::CHECKPOINT_NUMBER + 1
        || *parent.number() != System::block_number()
        || *parent.parent_hash() != System::parent_hash()
        || parent.state_root().as_bytes() != sp_io::storage::root(StateVersion::V0).as_slice()
        || System::block_hash(0).as_bytes() != &recovery::GENESIS_HASH
    {
        return Err(RecoveryError::WrongPreimage);
    }
    let policy = crate::incident_recovery::compiled_policy()?;
    let commitment = policy.commitment();
    let state = recovery::storage::status()?.ok_or(RecoveryError::WrongPhase)?;
    if state.format_version != 1
        || state.phase != RecoveryPhase::Applying
        || state.operation_counter != 0
        || state.failure.is_some()
        || state.policy != commitment
        || sp_io::storage::get(recovery::POLICY_STORAGE_KEY).as_deref()
            != Some(commitment.as_slice())
        || sp_io::storage::get(recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
            != Some(commitment.as_slice())
        || sp_io::storage::get(recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() != Some(&[1])
        || recovery::TRANSIENT_CONTEXT_KEYS
            .iter()
            .any(|key| sp_io::storage::exists(key))
    {
        return Err(RecoveryError::WrongPhase);
    }
    let markers: Vec<_> = parent
        .digest()
        .logs()
        .iter()
        .filter_map(|item| item.as_other().and_then(recovery::parse_lineage_marker))
        .collect();
    if markers.as_slice() != [commitment] {
        return Err(RecoveryError::WrongPolicy);
    }
    // Do not install or infer the birth anchor. Only child two may install it.
    let anchor = recovery::storage::recovery_anchor()?;
    let operation_bytes = operation;
    let operation: IncomingOperation = decode_bounded(operation_bytes)?;
    if operation.encode() != operation_bytes {
        return Err(RecoveryError::WrongPreimage);
    }
    if !matches!(
        operation.channel,
        IncomingChannel::Hashi { .. } | IncomingChannel::HashiAuthenticatedSource { .. }
    ) || operation.index != state.next_chunk
        || policy.chunk_hashes.get(operation.index as usize) != Some(&operation.commitment())
    {
        return Err(RecoveryError::WrongChunk);
    }
    let stored_code = sp_io::storage::get(b":code").ok_or(RecoveryError::BadCode)?;
    let execution_number = parent
        .number()
        .checked_add(1)
        .ok_or(RecoveryError::WrongPhase)?;
    Ok(Context {
        parent,
        policy,
        operation,
        anchor,
        stored_code_hash: sp_io::hashing::blake2_256(&stored_code),
        execution_number,
    })
}

fn rollback<T>(run: impl FnOnce() -> Result<T, RecoveryError>) -> Result<T, RecoveryError> {
    frame_support::storage::transactional::with_transaction_opaque_err(|| {
        TransactionOutcome::Rollback(run())
    })
    .map_err(|_| RecoveryError::WrongPhase)?
}

/// Populate only the next-number/parent lookup needed by native signed payloads.
/// This does not invoke consensus, timestamp or economic hooks and therefore
/// cannot certify an entire candidate block. All writes remain in `rollback`.
fn execution_context(context: &Context) {
    // FRAME keeps these aliases private and exposes setters only to std/test
    // builds. Use their exact native value keys/types, verified against the
    // native getters/setters by the focused tests, without initializing hooks.
    sp_io::storage::set(
        &frame_support::storage::storage_prefix(b"System", b"Number"),
        &context.execution_number.encode(),
    );
    sp_io::storage::set(
        &frame_support::storage::storage_prefix(b"System", b"ParentHash"),
        &context.parent.hash().encode(),
    );
    frame_system::BlockHash::<Runtime>::insert(*context.parent.number(), context.parent.hash());
}

fn prepare_native(context: &Context) -> Result<RecoveryHashiPreparation, RecoveryError> {
    execution_context(context);
    let active = prepare_operation(&context.operation, &context.policy)?;
    if active.transfers.len() != 1 || !active.controls.is_empty() {
        return Err(RecoveryError::WrongPreimage);
    }
    if !context
        .operation
        .required_weight()
        .all_lte(<Runtime as frame_system::Config>::BlockWeights::get().max_block)
    {
        return Err(RecoveryError::ChunkBounds);
    }
    let approvals = match &context.operation.channel {
        IncomingChannel::Hashi { approvals, .. }
        | IncomingChannel::HashiAuthenticatedSource { approvals, .. } => approvals,
        _ => return Err(RecoveryError::WrongPreimage),
    };
    if approvals.is_empty() || approvals.len() > MAX_HASHI_APPROVALS {
        return Err(RecoveryError::ChunkBounds);
    }
    let transfer = native_hashi_transfer(&context.operation.channel)?;
    let bridge = crate::EthBridge::bridge_account(transfer.network_id)
        .ok_or(RecoveryError::WrongPreimage)?;
    let peers = crate::EthBridge::peers(transfer.network_id);
    let members =
        bridge_multisig::Accounts::<Runtime>::get(&bridge).ok_or(RecoveryError::WrongPreimage)?;
    let current = u64::from(context.execution_number);
    let mut payloads = Vec::with_capacity(approvals.len());
    // Existing native admission permits retained loads only inside this exact
    // one-block inbound context. It is temporary and removed before apply.
    sp_io::storage::set(
        recovery::BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
        &context.execution_number.encode(),
    );
    for approval in approvals {
        if !peers.contains(&approval.signer) || !members.is_signatory(&approval.signer) {
            return Err(RecoveryError::BadQuorum);
        }
        let (call, _, _) = hashi_call(approval, &transfer)?;
        let (extra, era, nonce) = hashi_extra(&approval.extra)?;
        let birth = era.birth(current);
        if birth <= u64::from(recovery::CHECKPOINT_NUMBER)
            || birth >= current
            || nonce == crate::Index::MAX
            || System::account_nonce(&approval.signer) != nonce
        {
            return Err(RecoveryError::WrongPreimage);
        }
        frame_system::CheckNonce::<Runtime>::validate_nonce_for_account(&approval.signer, nonce)
            .map_err(|_| RecoveryError::Replay)?;
        if !crate::xor_fee_impls::validate_bridge_fee_exemption(&approval.signer, &call)
            .map_err(|_| RecoveryError::WrongPreimage)?
        {
            return Err(RecoveryError::WrongPreimage);
        }
        let birth_number: u32 = birth.try_into().map_err(|_| RecoveryError::WrongPreimage)?;
        let birth_hash = frame_system::BlockHash::<Runtime>::try_get(birth_number)
            .map_err(|_| RecoveryError::WrongPreimage)?;
        if birth_hash == sp_core::H256::zero() {
            return Err(RecoveryError::WrongPreimage);
        }
        let payload = sp_runtime::generic::SignedPayload::new(call, extra)
            .map_err(|_| RecoveryError::WrongPreimage)?
            .using_encoded(|bytes| bytes.to_vec());
        payloads.push(RecoveryHashiPayload {
            signer: *approval.signer.as_ref(),
            nonce,
            birth_number: birth,
            birth_hash: *birth_hash.as_fixed_bytes(),
            death_number: era.death(current),
            call_hash: sp_io::hashing::blake2_256(&approval.call),
            extra_hash: sp_io::hashing::blake2_256(&approval.extra),
            payload,
        });
    }
    sp_io::storage::clear(recovery::BRIDGE_INBOUND_CONTEXT_STORAGE_KEY);
    let version = <Runtime as frame_system::Config>::Version::get();
    Ok(RecoveryHashiPreparation {
        format_version: FORMAT_VERSION,
        parent_hash: *context.parent.hash().as_fixed_bytes(),
        parent_state_root: *context.parent.state_root().as_fixed_bytes(),
        execution_number: context.execution_number,
        policy_commitment: context.policy.commitment(),
        recovery_anchor: context.anchor,
        operation_index: context.operation.index,
        operation_commitment: context.operation.commitment(),
        network: transfer.network_id,
        spec_version: version.spec_version,
        transaction_version: version.transaction_version,
        stored_code_hash: context.stored_code_hash,
        source_evidence_commitment: context.operation.finalized_source_evidence,
        approvals: payloads,
        external_finality_verified: false,
    })
}

fn admit_native(context: &Context) -> Result<RecoveryHashiAdmission, RecoveryError> {
    let preparation = prepare_native(context)?;
    let operation = context.operation.encode();
    let (state, weight) = apply_encoded_with_policy(&context.policy, &operation)?;
    Ok(RecoveryHashiAdmission {
        preparation,
        operation_bytes_hash: sp_io::hashing::blake2_256(&operation),
        resulting_cursor: state.next_chunk,
        resulting_operation_counter: state.operation_counter,
        resulting_status_scale: state.encode(),
        weight_ref_time: weight.ref_time(),
        weight_proof_size: weight.proof_size(),
        native_signatures_checked: true,
        native_incoming_executed: true,
        persistent_state_changed: false,
        external_finality_verified: false,
    })
}

/// No caller-provided policy, version or domain is accepted.
pub fn prepare(
    header: Vec<u8>,
    operation: Vec<u8>,
) -> Result<RecoveryHashiPreparation, DispatchError> {
    supported_runtime().map_err(|_| DispatchError::Other("HASHI recovery executor unavailable"))?;
    let context = context(&header, &operation)
        .map_err(|_| DispatchError::Other("HASHI recovery context rejected"))?;
    rollback(|| prepare_native(&context))
        .map_err(|_| DispatchError::Other("HASHI recovery template rejected"))
}

/// Execute the existing native incoming path, always discarding every write.
pub fn admit(header: Vec<u8>, operation: Vec<u8>) -> Result<RecoveryHashiAdmission, DispatchError> {
    supported_runtime().map_err(|_| DispatchError::Other("HASHI recovery executor unavailable"))?;
    let context = context(&header, &operation)
        .map_err(|_| DispatchError::Other("HASHI recovery context rejected"))?;
    rollback(|| admit_native(&context))
        .map_err(|_| DispatchError::Other("HASHI recovery native admission rejected"))
}

#[cfg(test)]
#[path = "incident_recovery_hashi_witness_tests.rs"]
pub(crate) mod tests;
