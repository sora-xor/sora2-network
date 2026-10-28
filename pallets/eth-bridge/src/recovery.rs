//! Persistent SORA Nexus bridge maintenance latch. No call can clear it here.

use codec::{Decode, Encode};
use frame_support::ensure;
use sp_runtime::{traits::UniqueSaturatedInto, DispatchError};

pub const HASHI_RESOLUTION_KEY: &[u8] = b":sora-nexus:recovery:hashi-resolution:v1";

/// Internal, one-transfer resolution written only by the committed recovery executive.
/// The original peer-approved request remains stored and is used for replay protection.
#[derive(Clone, Encode, Decode, codec::DecodeWithMemTracking)]
pub struct HashiResolution<AccountId> {
    pub block: u32,
    pub request_hash: [u8; 32],
    pub beneficiary: AccountId,
    pub excluded: bool,
    pub consumed: bool,
}

pub(crate) fn transfer_resolution<T: crate::Config>(
    request: &crate::IncomingTransfer<T>,
) -> Result<Option<HashiResolution<T::AccountId>>, DispatchError> {
    ensure_incoming_transaction_active::<T>(request.network_id, request.tx_hash)?;
    if !halted() {
        return Ok(None);
    }
    ensure!(inbound_allowed::<T>(), crate::Error::<T>::RecoveryHalted);
    let bytes =
        sp_io::storage::get(HASHI_RESOLUTION_KEY).ok_or(crate::Error::<T>::RecoveryHalted)?;
    // AccountId is bounded by frame_system::Config. Reject extra bytes and stale contexts.
    ensure!(bytes.len() <= 1024, crate::Error::<T>::RecoveryHalted);
    let mut input = bytes.as_ref();
    let resolution = HashiResolution::<T::AccountId>::decode(&mut input)
        .map_err(|_| crate::Error::<T>::RecoveryHalted)?;
    let block: u32 = frame_system::Pallet::<T>::block_number()
        .try_into()
        .map_err(|_| crate::Error::<T>::RecoveryHalted)?;
    ensure!(
        input.is_empty()
            && resolution.block == block
            && !resolution.consumed
            && resolution.request_hash == sp_io::hashing::blake2_256(&request.encode()),
        crate::Error::<T>::RecoveryHalted
    );
    Ok(Some(resolution))
}

pub(crate) fn consume_transfer<AccountId: Encode>(mut resolution: HashiResolution<AccountId>) {
    resolution.consumed = true;
    sp_io::storage::set(HASHI_RESOLUTION_KEY, &resolution.encode());
}

pub(crate) const HALT_KEY: &[u8] = b":sora-nexus:recovery:bridge-halt:v1";
pub(crate) const INBOUND_CONTEXT_KEY: &[u8] = b":sora-nexus:recovery:bridge-inbound-context:v1";

pub(crate) fn halted() -> bool {
    sp_io::storage::exists(HALT_KEY)
}

/// The recovery executive sets this exact SCALE u32 only around a committed
/// authenticated inbound operation, and clears it transactionally before return.
pub(crate) fn inbound_allowed<T: frame_system::Config>() -> bool {
    if !halted() {
        return true;
    }
    let number: Result<u32, _> = frame_system::Pallet::<T>::block_number().try_into();
    number.ok().is_some_and(|number| {
        sp_io::storage::get(INBOUND_CONTEXT_KEY)
            .is_some_and(|bytes| bytes.as_ref() == number.to_le_bytes().as_slice())
    })
}

/// A request quarantine cannot be reopened by clearing the global maintenance latch.
pub fn ensure_outgoing_active<T: crate::Config>(
    network: T::NetworkId,
    hash: sp_core::H256,
) -> Result<(), crate::Error<T>> {
    ensure!(
        !bridge_types::recovery::hashi_quarantined(network, hash)
            && !bridge_types::recovery::message_quarantined(
                bridge_types::GenericNetworkId::EVMLegacy(network.unique_saturated_into()),
                hash,
            ),
        crate::Error::<T>::RecoveryHalted
    );
    Ok(())
}

/// Incoming BridgeProxy identity is the external transaction hash, not the
/// separately hashed native request. Match the notifier's exact domain before
/// reserving, minting, releasing custody, or recording load/replay bookkeeping.
pub(crate) fn ensure_incoming_transaction_active<T: crate::Config>(
    network: T::NetworkId,
    tx_hash: sp_core::H256,
) -> Result<(), crate::Error<T>> {
    ensure!(
        !bridge_types::recovery::message_quarantined(
            bridge_types::GenericNetworkId::EVMLegacy(network.unique_saturated_into()),
            tx_hash,
        ),
        crate::Error::<T>::RecoveryHalted
    );
    Ok(())
}

/// Resolve incoming cancellation/status work to its original outgoing identity
/// before any cancellation fallback or native status mutation can run.
pub(crate) fn ensure_incoming_active<T: crate::Config>(
    request: &crate::requests::IncomingRequest<T>,
) -> Result<(), crate::Error<T>> {
    match request {
        crate::requests::IncomingRequest::Transfer(request) => {
            ensure_incoming_transaction_active::<T>(request.network_id, request.tx_hash)
        }
        crate::requests::IncomingRequest::CancelOutgoingRequest(request) => {
            ensure_outgoing_active::<T>(request.network_id, request.outgoing_request_hash)
        }
        crate::requests::IncomingRequest::MarkAsDone(request) => {
            ensure_outgoing_active::<T>(request.network_id, request.outgoing_request_hash)
        }
        _ => Ok(()),
    }
}

pub(crate) fn ensure_request_active<T: crate::Config>(
    request: &crate::requests::OffchainRequest<T>,
) -> Result<(), crate::Error<T>> {
    match request {
        crate::requests::OffchainRequest::Outgoing(request, hash) => {
            ensure_outgoing_active::<T>(request.network_id(), *hash)
        }
        crate::requests::OffchainRequest::Incoming(request, _) => {
            ensure_incoming_active::<T>(request)
        }
        crate::requests::OffchainRequest::LoadIncoming(
            crate::requests::LoadIncomingRequest::Meta(request, _),
        ) if request.kind == crate::requests::IncomingMetaRequestKind::MarkAsDone => {
            ensure_outgoing_active::<T>(request.network_id, request.hash)
        }
        crate::requests::OffchainRequest::LoadIncoming(
            crate::requests::LoadIncomingRequest::Transaction(request),
        ) if matches!(
            request.kind,
            crate::requests::IncomingTransactionRequestKind::Transfer
                | crate::requests::IncomingTransactionRequestKind::TransferXOR
        ) =>
        {
            ensure_incoming_transaction_active::<T>(request.network_id, request.hash)
        }
        crate::requests::OffchainRequest::LoadIncoming(_) => Ok(()),
    }
}

/// Resolves an authenticated historical account without changing its authority.
/// The runtime supplies a permanent, policy-bound map; identity is useful for
/// pallets embedded in runtimes without incident recovery.
pub trait IncomingAccountResolver<AccountId> {
    fn resolve(account: &AccountId) -> Result<Option<AccountId>, DispatchError>;
}

impl<AccountId> IncomingAccountResolver<AccountId> for () {
    fn resolve(_: &AccountId) -> Result<Option<AccountId>, DispatchError> {
        Ok(None)
    }
}

/// The manifest executor owns its transient resolution and index reconciliation.
/// Ordinary native delivery may use the permanent map only after admission.
pub(crate) fn ordinary_account<T: crate::Config>(
    account: &T::AccountId,
) -> Result<T::AccountId, DispatchError> {
    if halted() {
        return Ok(account.clone());
    }
    Ok(T::IncomingAccountResolver::resolve(account)?.unwrap_or_else(|| account.clone()))
}

/// AccountRequests indexes the author, independently of a transfer beneficiary.
/// Resolve before preparation or registration writes. Never change outgoing
/// authorization or the original signed Incoming/LoadIncoming request bytes.
pub(crate) fn incoming_index_author<T: crate::Config>(
    request: &crate::OffchainRequest<T>,
) -> Result<T::AccountId, DispatchError> {
    match request {
        crate::OffchainRequest::Outgoing(_, _) => Ok(request.author().clone()),
        // The native load kind need not equal its incoming result kind. These
        // are query references only, including administrative incoming kinds;
        // native validations and signed payloads remain unchanged.
        crate::OffchainRequest::Incoming(_, _) | crate::OffchainRequest::LoadIncoming(_) => {
            ordinary_account::<T>(request.author())
        }
    }
}

/// At most six permanent lookups occur in an incoming import (author and
/// beneficiary preflight, load author, transfer author, beneficiary preflight
/// before cancellation handling, and finalization). Cover all marker/context
/// reads and 78 static-map comparisons; historical no-policy weights are unchanged.
pub(crate) fn resolution_weight<T: crate::Config>() -> frame_support::weights::Weight {
    if !sp_io::storage::exists(b":sora-nexus:recovery:policy:v1") || halted() {
        // Halted manifest work supplies its own bounded executor allowance and
        // never calls the permanent resolver. Preserve its signed max_weight.
        return frame_support::weights::Weight::zero();
    }
    use frame_support::traits::Get;
    frame_support::weights::Weight::from_parts(50_000_000, 32_768)
        .saturating_add(T::DbWeight::get().reads(32))
}
