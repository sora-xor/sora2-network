//! Native receipt-to-request association before any beneficiary replacement.
//!
//! The peer supplies an ordered projection of its independently fetched
//! successful receipt. This method does not prove receipt inclusion, complete
//! logs or external finality; those remain the existing peers' responsibility.
//! No provider, keystore, offchain extension or signed transaction is used here.

use super::super::{
    authenticated_hashi_source, decode_bounded, IncomingChannel, IncomingOperation,
};
use super::{context, prepare_native, rollback, supported_runtime};
use crate::{BridgeMultisig, EthBridge, Runtime};
use alloc::{vec, vec::Vec};
use bridge_types::{H160, H256, U256};
use codec::Encode;
use common::recovery::{self, RecoveryError};
use eth_bridge::requests::{
    IncomingRequest, IncomingTransactionRequestKind as Kind, LoadIncomingRequest,
    LoadIncomingTransactionRequest, OffchainRequest, RequestStatus,
};
use eth_bridge_runtime_api::recovery::{
    RecoveryHashiLoadProvenance, RecoveryHashiSource, RecoveryHashiSourceAssociation,
    RecoveryHashiSourcePreparation,
};
use sp_runtime::DispatchError;

const ETHEREUM_GENESIS: [u8; 32] = [
    0xd4, 0xe5, 0x67, 0x40, 0xf8, 0x76, 0xae, 0xf8, 0xc0, 0x10, 0xb8, 0x6a, 0x40, 0xd5, 0xf5, 0x67,
    0x45, 0xa1, 0x18, 0xd0, 0x90, 0x6a, 0x34, 0xe6, 0x9a, 0xec, 0x8c, 0x0d, 0xb1, 0xcb, 0x8f, 0xa3,
];
const MAX_LOGS: usize = 4096;
const MAX_LOG_DATA: usize = 64 * 1024;

fn bad() -> RecoveryError {
    RecoveryError::WrongPreimage
}

fn source_commitment(
    source: &RecoveryHashiSource,
    selected: usize,
    raw_transaction: &[u8],
) -> [u8; 32] {
    // Identical to the existing offline native association verifier's domain.
    // Native network 0 is HASHI Ethereum, not an external EVM chain identifier.
    let mut domain = b"sora-nexus/evm-required-source/v1\0".to_vec();
    domain.extend(0u32.to_le_bytes());
    domain.extend(U256::one().to_little_endian());
    domain.extend(ETHEREUM_GENESIS);
    let log = &source.logs[selected];
    let events = vec![(
        log.log_index,
        selected as u32,
        log.address,
        log.topics[0],
        log.data.clone(),
    )];
    sp_io::hashing::blake2_256(
        &(
            b"sora-nexus/verified-hashi-source-record/v1".to_vec(),
            sp_io::hashing::sha2_256(&domain),
            source.block_hash,
            source.block_number,
            source.transaction_hash,
            source.transaction_index,
            raw_transaction.to_vec(),
            events,
        )
            .encode(),
    )
}

fn associate(
    operation: &IncomingOperation,
    source_bytes: &[u8],
) -> Result<RecoveryHashiSourceAssociation, RecoveryError> {
    let source: RecoveryHashiSource = decode_bounded(source_bytes)?;
    if source.encode() != source_bytes
        || source.format_version != 1
        || source.status != 1
        || source.block_hash == [0; 32]
        || source.block_number == 0
        || source.logs.is_empty()
        || source.logs.len() > MAX_LOGS
    {
        return Err(bad());
    }
    let IncomingChannel::HashiAuthenticatedSource {
        network,
        request,
        raw_transaction,
        expected_chain_id,
        ..
    } = &operation.channel
    else {
        return Err(bad());
    };
    if *network != 0 || expected_chain_id.is_some_and(|id| id != U256::one()) {
        return Err(bad());
    }
    let original: IncomingRequest<Runtime> = decode_bounded(request)?;
    let IncomingRequest::Transfer(transfer) = &original else {
        return Err(bad());
    };
    if original.encode() != *request
        || transfer.from != H160::zero()
        || transfer.amount == 0
        || transfer.should_take_fee
        || transfer.network_id != *network
        || transfer.tx_hash != H256(source.transaction_hash)
        || transfer.at_height != source.block_number
    {
        return Err(bad());
    }
    let actor = authenticated_hashi_source(request, raw_transaction, *expected_chain_id)?;
    let load: LoadIncomingRequest<Runtime> = decode_bounded(&source.load_scale)?;
    let native_load = LoadIncomingTransactionRequest::new(
        transfer.author.clone(),
        transfer.tx_hash,
        transfer.timepoint,
        Kind::Transfer,
        *network,
    );
    if load.encode() != source.load_scale
        || load != LoadIncomingRequest::Transaction(native_load.clone())
    {
        return Err(bad());
    }
    let mut logs = Vec::with_capacity(source.logs.len());
    let mut selected = None;
    for (index, row) in source.logs.iter().enumerate() {
        if row.topics.len() > 4
            || row.data.len() > MAX_LOG_DATA
            || (index > 0 && source.logs[index - 1].log_index.checked_add(1) != Some(row.log_index))
        {
            return Err(bad());
        }
        if row.topics.first() == Some(eth_bridge::DEPOSIT_TOPIC.as_fixed_bytes())
            && EthBridge::ensure_known_contract(H160(row.address), *network).is_ok()
        {
            if selected.replace(index).is_some()
                || row.topics.len() != 1
                || row.data.len() != 128
                || row.data[64..76].iter().any(|byte| *byte != 0)
            {
                return Err(bad());
            }
        }
        logs.push(eth_bridge::types::Log {
            address: eth_bridge::types::H160(row.address),
            topics: row
                .topics
                .iter()
                .copied()
                .map(eth_bridge::types::H256)
                .collect(),
            data: eth_bridge::types::Bytes(row.data.clone()),
            block_hash: Some(eth_bridge::types::H256(source.block_hash)),
            block_number: Some(source.block_number.into()),
            transaction_hash: Some(eth_bridge::types::H256(source.transaction_hash)),
            transaction_index: Some(source.transaction_index.into()),
            log_index: Some(row.log_index.into()),
            transaction_log_index: Some((index as u32).into()),
            log_type: None,
            removed: Some(false),
        });
    }
    let selected = selected.ok_or_else(bad)?;
    if source_commitment(&source, selected, raw_transaction) != operation.finalized_source_evidence
    {
        return Err(bad());
    }
    let tx_hash = transfer.tx_hash;
    let queue = eth_bridge::RequestsQueue::<Runtime>::get(*network);
    let provenance = if let Some(stored) = eth_bridge::Requests::<Runtime>::get(*network, tx_hash) {
        // Retained user-load HTTP processing admitted the outer call target.
        // Authenticate it from the raw signed transaction; never a JSON `to`.
        EthBridge::ensure_known_contract(actor.destination.ok_or_else(bad)?, *network)
            .map_err(|_| bad())?;
        if stored.encode() != OffchainRequest::<Runtime>::LoadIncoming(load.clone()).encode()
            || eth_bridge::RequestStatuses::<Runtime>::get(*network, tx_hash)
                != Some(RequestStatus::Pending)
            || !eth_bridge::RequestSubmissionHeight::<Runtime>::contains_key(*network, tx_hash)
            || eth_bridge::RequestSubmissionHeight::<Runtime>::get(*network, tx_hash)
                > recovery::CHECKPOINT_NUMBER
            || queue.iter().filter(|id| **id == tx_hash).count() != 1
        {
            return Err(bad());
        }
        RecoveryHashiLoadProvenance::CheckpointPending {
            submission_height: eth_bridge::RequestSubmissionHeight::<Runtime>::get(
                *network, tx_hash,
            ),
        }
    } else {
        // Native handle_logs accepts routed outer calls. Only the emitting
        // contract is constrained on this automatic-event path.
        let expected = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
            transfer.to.clone(),
            tx_hash,
            BridgeMultisig::sidechain_timepoint(source.block_number, source.transaction_index),
            Kind::Transfer,
            *network,
        ));
        if load != expected
            || eth_bridge::RequestStatuses::<Runtime>::contains_key(*network, tx_hash)
            || eth_bridge::RequestSubmissionHeight::<Runtime>::contains_key(*network, tx_hash)
            || queue.contains(&tx_hash)
            || EthBridge::account_requests(load.author()).contains(&(*network, tx_hash))
        {
            return Err(bad());
        }
        RecoveryHashiLoadProvenance::AutomaticEventImport
    };
    if eth_bridge::LoadToIncomingRequestHash::<Runtime>::contains_key(*network, tx_hash) {
        return Err(RecoveryError::Replay);
    }
    let event = EthBridge::parse_main_event(*network, &logs, Kind::Transfer).map_err(|_| bad())?;
    let expected = IncomingRequest::<Runtime>::try_from_contract_event(
        event,
        native_load,
        source.block_number,
    )
    .map_err(|_| bad())?;
    if expected.encode() != *request {
        return Err(bad());
    }
    let incoming_hash = OffchainRequest::incoming(original).hash();
    if eth_bridge::Requests::<Runtime>::contains_key(*network, incoming_hash)
        || eth_bridge::RequestStatuses::<Runtime>::contains_key(*network, incoming_hash)
    {
        return Err(RecoveryError::Replay);
    }
    Ok(RecoveryHashiSourceAssociation {
        source_scale_hash: sp_io::hashing::blake2_256(source_bytes),
        original_request_hash: sp_io::hashing::blake2_256(request),
        native_incoming_hash: *incoming_hash.as_fixed_bytes(),
        load_scale_hash: sp_io::hashing::blake2_256(&source.load_scale),
        load_provenance: provenance,
        source_contract: source.logs[selected].address,
        source_block_hash: source.block_hash,
        source_block_number: source.block_number,
        source_transaction_hash: source.transaction_hash,
        source_transaction_index: source.transaction_index,
        source_log_index: source.logs[selected].log_index,
        receipt_log_index: selected as u32,
        authenticated_outer_actor: *actor.signer.as_fixed_bytes(),
        authenticated_outer_destination: actor.destination.map(|value| *value.as_fixed_bytes()),
        complete_original_request_matches: true,
        source_inclusion_verified: false,
        external_finality_verified: false,
    })
}

pub fn prepare_source(
    header: Vec<u8>,
    operation: Vec<u8>,
    source: Vec<u8>,
) -> Result<RecoveryHashiSourcePreparation, DispatchError> {
    supported_runtime().map_err(|_| DispatchError::Other("HASHI recovery executor unavailable"))?;
    let context = context(&header, &operation)
        .map_err(|_| DispatchError::Other("HASHI recovery context rejected"))?;
    rollback(|| {
        let association = associate(&context.operation, &source)?;
        let preparation = prepare_native(&context)?;
        Ok(RecoveryHashiSourcePreparation {
            preparation,
            association,
        })
    })
    .map_err(|_| DispatchError::Other("HASHI recovery source association rejected"))
}

#[cfg(test)]
#[path = "incident_recovery_hashi_source_tests.rs"]
pub(crate) mod tests;
