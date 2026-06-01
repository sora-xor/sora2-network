use crate::requests::{
    IncomingAddToken, IncomingCancelOutgoingRequest, IncomingChangePeers,
    IncomingChangePeersCompat, IncomingMarkAsDoneRequest, IncomingMigrate,
    IncomingPrepareForMigration, IncomingTransfer, OutgoingAddAsset, RequestStatus,
};
use codec::Decode;
use codec::Encode;
use frame_support::ensure;
use frame_support::sp_runtime::legacy::byte_sized_error::DispatchError as OldDispatchError;
use frame_support::sp_runtime::DispatchError;
use frame_support::sp_runtime::ModuleError;
use frame_support::traits::Get;
use frame_support::traits::GetStorageVersion;
use frame_support::traits::StorageVersion;
use frame_support::weights::Weight;
use sp_runtime::traits::BlockNumberProvider;
use sp_runtime::RuntimeDebug;

use crate::Config;
use crate::Pallet;
use crate::RequestApprovals;
use crate::RequestApprovers;
use crate::RequestStatuses;
use crate::RequestSubmissionHeight;
use crate::Requests;
use crate::{
    AccountRequests, BridgeAccount, BridgeStatus, BridgeStatuses, DeprecatedSidechainTokens, Event,
    LegacyEthereumXorDecommissioned, LegacyEthereumXorDecommissionedAt, RegisteredAsset,
    RegisteredSidechainAsset, RegisteredSidechainToken, SidechainAssetPrecision,
    LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
};
use crate::{Error, RequestsQueue};
use crate::{
    IncomingRequest, IncomingTransactionRequestKind, LoadIncomingRequest, OffchainRequest,
    OutgoingRequest,
};
use common::prelude::Balance;
use common::AssetInfoProvider;
use sp_std::vec::Vec;

#[derive(PartialEq, Eq, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
pub enum OldRequestStatus {
    Pending,
    Frozen,
    ApprovalsReady,
    Failed(OldDispatchError),
    Done,
    /// Request is broken. Tried to abort with the first error but got another one when cancelling.
    Broken(OldDispatchError, OldDispatchError),
}

pub fn migrate<T: Config>() -> Weight {
    let mut reads = 0u64;
    let mut writes = 0u64;
    let mut on_chain_storage_version = Pallet::<T>::on_chain_storage_version();

    if on_chain_storage_version < StorageVersion::new(2) {
        reads = reads.saturating_add(1);
        let mut translated = 0u64;
        RequestStatuses::<T>::translate::<OldRequestStatus, _>(|_, _, status| {
            translated = translated.saturating_add(1);
            let status = match status {
                OldRequestStatus::Pending => RequestStatus::Pending,
                OldRequestStatus::Frozen => RequestStatus::Frozen,
                OldRequestStatus::ApprovalsReady => RequestStatus::ApprovalsReady,
                OldRequestStatus::Failed(err) => RequestStatus::Failed(migrate_error(err)),
                OldRequestStatus::Done => RequestStatus::Done,
                OldRequestStatus::Broken(err1, err2) => {
                    RequestStatus::Broken(migrate_error(err1), migrate_error(err2))
                }
            };
            Some(status)
        });
        reads = reads.saturating_add(translated);
        writes = writes.saturating_add(translated);
        StorageVersion::new(2).put::<Pallet<T>>();
        writes = writes.saturating_add(1);
        on_chain_storage_version = StorageVersion::new(2);
    }

    if on_chain_storage_version < StorageVersion::new(3) {
        let (migration_reads, migration_writes) =
            migrate_legacy_incoming_change_peers_requests::<T>();
        reads = reads.saturating_add(migration_reads);
        writes = writes.saturating_add(migration_writes);
        StorageVersion::new(3).put::<Pallet<T>>();
        writes = writes.saturating_add(1);
    }

    T::DbWeight::get().reads_writes(reads, writes)
}

#[derive(Clone, Encode, Decode, PartialEq, Eq)]
struct OldIncomingChangePeers<T: Config> {
    pub peer_account_id: <T as frame_system::pallet::Config>::AccountId,
    pub peer_address: crate::EthAddress,
    pub removed: bool,
    pub author: <T as frame_system::pallet::Config>::AccountId,
    pub tx_hash: sp_core::H256,
    pub at_height: u64,
    pub timepoint: crate::BridgeTimepoint<T>,
    pub network_id: crate::BridgeNetworkId<T>,
}

#[derive(Clone, Encode, Decode, PartialEq, Eq)]
enum OldIncomingRequest<T: Config> {
    Transfer(IncomingTransfer<T>),
    AddToken(IncomingAddToken<T>),
    ChangePeers(OldIncomingChangePeers<T>),
    CancelOutgoingRequest(IncomingCancelOutgoingRequest<T>),
    MarkAsDone(IncomingMarkAsDoneRequest<T>),
    PrepareForMigration(IncomingPrepareForMigration<T>),
    Migrate(IncomingMigrate<T>),
    ChangePeersCompat(IncomingChangePeersCompat<T>),
}

#[derive(Clone, Encode, Decode, PartialEq, Eq)]
enum OldOffchainRequest<T: Config> {
    Outgoing(OutgoingRequest<T>, sp_core::H256),
    LoadIncoming(LoadIncomingRequest<T>),
    Incoming(OldIncomingRequest<T>, sp_core::H256),
}

enum DecodedOffchainRequest<T: Config> {
    Current(OffchainRequest<T>),
    Legacy(OffchainRequest<T>),
}

impl<T: Config> DecodedOffchainRequest<T> {
    fn into_request(self) -> OffchainRequest<T> {
        match self {
            Self::Current(request) | Self::Legacy(request) => request,
        }
    }
}

impl<T: Config> From<OldIncomingChangePeers<T>> for IncomingChangePeers<T> {
    fn from(request: OldIncomingChangePeers<T>) -> Self {
        Self {
            peer_account_id: Some(request.peer_account_id),
            peer_address: request.peer_address,
            removed: request.removed,
            author: request.author,
            tx_hash: request.tx_hash,
            at_height: request.at_height,
            timepoint: request.timepoint,
            network_id: request.network_id,
        }
    }
}

impl<T: Config> From<OldIncomingRequest<T>> for IncomingRequest<T> {
    fn from(request: OldIncomingRequest<T>) -> Self {
        match request {
            OldIncomingRequest::Transfer(request) => IncomingRequest::Transfer(request),
            OldIncomingRequest::AddToken(request) => IncomingRequest::AddToken(request),
            OldIncomingRequest::ChangePeers(request) => {
                IncomingRequest::ChangePeers(request.into())
            }
            OldIncomingRequest::CancelOutgoingRequest(request) => {
                IncomingRequest::CancelOutgoingRequest(request)
            }
            OldIncomingRequest::MarkAsDone(request) => IncomingRequest::MarkAsDone(request),
            OldIncomingRequest::PrepareForMigration(request) => {
                IncomingRequest::PrepareForMigration(request)
            }
            OldIncomingRequest::Migrate(request) => IncomingRequest::Migrate(request),
            OldIncomingRequest::ChangePeersCompat(request) => {
                IncomingRequest::ChangePeersCompat(request)
            }
        }
    }
}

impl<T: Config> From<OldOffchainRequest<T>> for OffchainRequest<T> {
    fn from(request: OldOffchainRequest<T>) -> Self {
        match request {
            OldOffchainRequest::Outgoing(request, hash) => OffchainRequest::Outgoing(request, hash),
            OldOffchainRequest::LoadIncoming(request) => OffchainRequest::LoadIncoming(request),
            OldOffchainRequest::Incoming(request, hash) => {
                OffchainRequest::Incoming(request.into(), hash)
            }
        }
    }
}

fn decode_exact<D: Decode>(raw: &[u8]) -> Result<D, codec::Error> {
    let mut input = raw;
    let decoded = D::decode(&mut input)?;
    if input.is_empty() {
        Ok(decoded)
    } else {
        Err(codec::Error::from("unexpected trailing bytes"))
    }
}

fn decode_offchain_request_compat<T: Config>(
    raw: &[u8],
) -> Result<DecodedOffchainRequest<T>, (codec::Error, codec::Error)> {
    match decode_exact::<OffchainRequest<T>>(raw) {
        Ok(request) => Ok(DecodedOffchainRequest::Current(request)),
        Err(current_error) => decode_exact::<OldOffchainRequest<T>>(raw)
            .map(Into::into)
            .map(DecodedOffchainRequest::Legacy)
            .map_err(|legacy_error| (current_error, legacy_error)),
    }
}

fn migrate_legacy_incoming_change_peers_requests<T: Config>() -> (u64, u64) {
    let mut reads = 0u64;
    let mut writes = 0u64;

    for (network_id, hash) in Requests::<T>::iter_keys() {
        reads = reads.saturating_add(1);
        let key = Requests::<T>::hashed_key_for(network_id, hash);
        let Some(raw) = frame_support::storage::unhashed::get_raw(&key) else {
            continue;
        };
        reads = reads.saturating_add(1);

        match decode_offchain_request_compat::<T>(&raw) {
            Ok(DecodedOffchainRequest::Current(_)) => {}
            Ok(DecodedOffchainRequest::Legacy(request)) => {
                frame_support::storage::unhashed::put_raw(&key, &request.encode());
                writes = writes.saturating_add(1);
            }
            Err((current_error, legacy_error)) => {
                frame_support::__private::log::warn!(
                    "Failed to decode eth-bridge request {:?} on network {:?} as current ({:?}) or legacy ({:?}) format",
                    hash,
                    network_id,
                    current_error,
                    legacy_error
                );
            }
        }
    }

    (reads, writes)
}

pub fn decommission_legacy_ethereum_xor<T: Config>() -> Weight {
    match common::with_transaction(decommission_legacy_ethereum_xor_inner::<T>) {
        Ok(weight) => weight,
        Err(error) => {
            frame_support::__private::log::error!(
                "Legacy Ethereum XOR decommission failed and was rolled back: {:?}",
                error
            );
            <T as frame_system::Config>::BlockWeights::get().max_block
        }
    }
}

fn decommission_legacy_ethereum_xor_inner<T: Config>() -> Result<Weight, DispatchError> {
    let mut reads = 0u64;
    let mut writes = 0u64;

    reads = reads.saturating_add(1);
    let already_decommissioned = LegacyEthereumXorDecommissioned::<T>::get();

    let network_id = T::GetEthNetworkId::get();
    let xor_asset_id = common::XOR.into();
    let reason: DispatchError = Error::<T>::DeprecatedLegacyXor.into();
    let (legacy_requests, legacy_reads) = legacy_ethereum_xor_requests::<T>(network_id);
    reads = reads.saturating_add(legacy_reads);
    let blockers =
        legacy_ethereum_xor_decommission_blocker_count_with_requests::<T>(&legacy_requests);

    reads = reads.saturating_add(1);
    let decommissioned_at = LegacyEthereumXorDecommissionedAt::<T>::get();
    if already_decommissioned && blockers == 0 {
        if decommissioned_at.is_none() {
            LegacyEthereumXorDecommissionedAt::<T>::put(
                frame_system::Pallet::<T>::current_block_number(),
            );
            writes = writes.saturating_add(1);
        }
        return Ok(T::DbWeight::get().reads_writes(reads, writes));
    }

    if blockers != 0 {
        frame_support::__private::log::warn!(
            "Quarantining {blockers} unsafe legacy Ethereum XOR outgoing transfer requests during decommission"
        );
    }

    DeprecatedSidechainTokens::<T>::insert(network_id, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS, true);
    writes = writes.saturating_add(1);

    reads = reads.saturating_add(1);
    let queue = RequestsQueue::<T>::get(network_id);
    let legacy_hashes = legacy_requests
        .iter()
        .map(|(hash, _, _)| *hash)
        .collect::<Vec<_>>();
    let mut queued_legacy_hashes = Vec::new();
    let mut retained_queue = Vec::with_capacity(queue.len());
    let mut queue_changed = false;
    for hash in queue {
        if !legacy_hashes.contains(&hash) {
            retained_queue.push(hash);
            continue;
        }
        queued_legacy_hashes.push(hash);
        queue_changed = true;
    }

    for (hash, request, status) in legacy_requests {
        let was_queued = queued_legacy_hashes.contains(&hash);
        let should_scrub = was_queued
            || !matches!(
                &status,
                Some(RequestStatus::Failed(_) | RequestStatus::Done)
            );
        if !should_scrub {
            continue;
        }
        match status {
            Some(RequestStatus::Pending) => {
                if let Err(cancel_error) = request.cancel() {
                    frame_support::__private::log::warn!(
                        "Failed to cancel legacy Ethereum XOR request {:?} during decommission; forcing deprecated failure: {:?}",
                        hash,
                        cancel_error
                    );
                }
                RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Failed(reason));
                writes = writes.saturating_add(1);
            }
            Some(
                RequestStatus::ApprovalsReady | RequestStatus::Frozen | RequestStatus::Broken(_, _),
            ) => {
                RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Failed(reason));
                writes = writes.saturating_add(1);
            }
            Some(RequestStatus::Failed(_) | RequestStatus::Done) => {}
            None => {
                RequestStatuses::<T>::insert(network_id, hash, RequestStatus::Failed(reason));
                writes = writes.saturating_add(1);
            }
        }

        RequestApprovals::<T>::remove(network_id, hash);
        RequestApprovers::<T>::remove(network_id, hash);
        writes = writes.saturating_add(2);
    }
    if queue_changed {
        RequestsQueue::<T>::insert(network_id, retained_queue);
        writes = writes.saturating_add(1);
    }

    RegisteredAsset::<T>::remove(network_id, &xor_asset_id);
    SidechainAssetPrecision::<T>::remove(network_id, &xor_asset_id);
    if let Some(token_address) = RegisteredSidechainToken::<T>::get(network_id, &xor_asset_id) {
        RegisteredSidechainAsset::<T>::remove(network_id, token_address);
        writes = writes.saturating_add(1);
    }
    RegisteredSidechainAsset::<T>::remove(network_id, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS);
    RegisteredSidechainToken::<T>::remove(network_id, &xor_asset_id);
    writes = writes.saturating_add(4);

    reads = reads.saturating_add(1);
    let bridge_account = BridgeAccount::<T>::get(network_id).ok_or(Error::<T>::UnknownNetwork)?;
    let total_balance = assets::Pallet::<T>::total_balance(&xor_asset_id, &bridge_account)?;
    reads = reads.saturating_add(1);
    if total_balance > Balance::from(0u32) {
        let _ = assets::Pallet::<T>::unreserve(&xor_asset_id, &bridge_account, total_balance)?;
        assets::Pallet::<T>::burn_from(
            &xor_asset_id,
            &bridge_account,
            &bridge_account,
            total_balance,
        )?;
        writes = writes.saturating_add(2);

        let remaining_balance = assets::Pallet::<T>::total_balance(&xor_asset_id, &bridge_account)?;
        reads = reads.saturating_add(1);
        if remaining_balance != Balance::from(0u32) {
            return Err(DispatchError::Other(
                "legacy Ethereum XOR bridge balance remains",
            ));
        }
    }

    if decommissioned_at.is_none() {
        LegacyEthereumXorDecommissionedAt::<T>::put(
            frame_system::Pallet::<T>::current_block_number(),
        );
        writes = writes.saturating_add(1);
    }
    if !already_decommissioned {
        LegacyEthereumXorDecommissioned::<T>::put(true);
        writes = writes.saturating_add(1);
    }

    Ok(T::DbWeight::get().reads_writes(reads, writes))
}

pub fn legacy_ethereum_xor_decommission_blockers<T: Config>() -> u32 {
    legacy_ethereum_xor_decommission_blocker_count::<T>().0
}

pub fn is_legacy_ethereum_xor_decommissioned<T: Config>() -> bool {
    LegacyEthereumXorDecommissioned::<T>::get()
}

pub fn legacy_ethereum_xor_decommissioned_at<T: Config>(
) -> Option<frame_system::pallet_prelude::BlockNumberFor<T>> {
    LegacyEthereumXorDecommissionedAt::<T>::get()
}

pub fn queue_ethereum_xor_thischain_add_asset_unchecked_capacity<T: Config>(
) -> Result<(), DispatchError> {
    let network_id = T::GetEthNetworkId::get();
    let asset_id = common::XOR.into();
    let from = Pallet::<T>::authority_account().ok_or(Error::<T>::AuthorityAccountNotSet)?;
    let nonce = frame_system::Pallet::<T>::account_nonce(&from);
    let timepoint = bridge_multisig::Pallet::<T>::thischain_timepoint();
    let request = OffchainRequest::outgoing(OutgoingRequest::AddAsset(OutgoingAddAsset {
        author: from.clone(),
        asset_id,
        nonce,
        network_id,
        timepoint,
    }));

    add_request_unchecked_capacity::<T>(&request)?;
    frame_system::Pallet::<T>::inc_account_nonce(&from);
    Ok(())
}

fn add_request_unchecked_capacity<T: Config>(
    request: &OffchainRequest<T>,
) -> Result<(), DispatchError> {
    let net_id = request.network_id();
    let bridge_status = BridgeStatuses::<T>::get(net_id).ok_or(Error::<T>::UnknownNetwork)?;
    let Some((outgoing_req, _)) = request.as_outgoing() else {
        return Err(Error::<T>::ExpectedOutgoingRequest.into());
    };

    ensure!(
        bridge_status != BridgeStatus::Migrating || outgoing_req.is_allowed_during_migration(),
        Error::<T>::ContractIsInMigrationStage
    );
    if outgoing_req.uses_weak_signature_domain() {
        let is_eth_peer_request = matches!(
            outgoing_req,
            OutgoingRequest::AddPeer(_)
                | OutgoingRequest::RemovePeer(_)
                | OutgoingRequest::AddPeerCompat(_)
                | OutgoingRequest::RemovePeerCompat(_)
        ) && net_id == T::GetEthNetworkId::get();
        ensure!(is_eth_peer_request, Error::<T>::WeakLegacySigningDisabled);
    }
    if let OutgoingRequest::AddAsset(add_asset_request) = outgoing_req {
        ensure!(
            !Pallet::<T>::is_add_asset_request_pending(net_id, add_asset_request.asset_id),
            Error::<T>::TokenIsAlreadyAdded
        );
    } else if let OutgoingRequest::AddToken(add_token_request) = outgoing_req {
        ensure!(
            !Pallet::<T>::is_add_token_request_pending(net_id, add_token_request.token_address),
            Error::<T>::SidechainAssetIsAlreadyRegistered
        );
    }

    let hash = request.hash();
    let can_resubmit = RequestStatuses::<T>::get(net_id, &hash)
        .map(|status| matches!(status, RequestStatus::Failed(_)))
        .unwrap_or(false);
    if !can_resubmit {
        ensure!(
            Requests::<T>::get(net_id, &hash).is_none(),
            Error::<T>::DuplicatedRequest
        );
    }
    request.validate()?;
    request.prepare()?;
    Pallet::<T>::clear_request_signatures(net_id, &hash);
    AccountRequests::<T>::mutate(request.author(), |vec| vec.push((net_id, hash)));
    Requests::<T>::insert(net_id, &hash, request);
    RequestsQueue::<T>::mutate(net_id, |queue| queue.push(hash));
    RequestStatuses::<T>::insert(net_id, &hash, RequestStatus::Pending);
    let block_number = frame_system::Pallet::<T>::current_block_number();
    RequestSubmissionHeight::<T>::insert(net_id, &hash, block_number);
    Pallet::<T>::deposit_event(Event::RequestRegistered(hash));
    Ok(())
}

fn legacy_ethereum_xor_decommission_blocker_count<T: Config>() -> (u32, u64) {
    let network_id = T::GetEthNetworkId::get();
    let (legacy_requests, reads) = legacy_ethereum_xor_requests::<T>(network_id);
    (
        legacy_ethereum_xor_decommission_blocker_count_with_requests::<T>(&legacy_requests),
        reads,
    )
}

fn legacy_ethereum_xor_requests<T: Config>(
    network_id: T::NetworkId,
) -> (
    Vec<(sp_core::H256, OffchainRequest<T>, Option<RequestStatus>)>,
    u64,
) {
    let mut requests = Vec::new();
    let mut reads = 0u64;

    for hash in Requests::<T>::iter_key_prefix(network_id) {
        reads = reads.saturating_add(1);
        let key = Requests::<T>::hashed_key_for(network_id, hash);
        let Some(raw) = frame_support::storage::unhashed::get_raw(&key) else {
            continue;
        };
        reads = reads.saturating_add(1);
        let request = match decode_offchain_request_compat::<T>(&raw) {
            Ok(decoded) => decoded.into_request(),
            Err((current_error, legacy_error)) => {
                frame_support::__private::log::warn!(
                    "Skipping undecodable eth-bridge request {:?} on network {:?}: current={:?}, legacy={:?}",
                    hash,
                    network_id,
                    current_error,
                    legacy_error
                );
                continue;
            }
        };
        if is_legacy_xor_request::<T>(network_id, &request) {
            let status = RequestStatuses::<T>::get(network_id, hash);
            reads = reads.saturating_add(1);
            requests.push((hash, request, status));
        }
    }

    (requests, reads)
}

fn legacy_ethereum_xor_decommission_blocker_count_with_requests<T: Config>(
    requests: &[(sp_core::H256, OffchainRequest<T>, Option<RequestStatus>)],
) -> u32 {
    let mut blockers = 0u32;

    for (_, request, status) in requests {
        if is_legacy_xor_outgoing_transfer::<T>(request)
            && is_unsafe_legacy_xor_outgoing_transfer_status(status)
        {
            blockers = blockers.saturating_add(1);
        }
    }

    blockers
}

fn is_unsafe_legacy_xor_outgoing_transfer_status(status: &Option<RequestStatus>) -> bool {
    matches!(
        status,
        Some(RequestStatus::ApprovalsReady | RequestStatus::Frozen | RequestStatus::Broken(_, _))
            | None
    )
}

fn is_legacy_xor_outgoing_transfer<T: Config>(request: &OffchainRequest<T>) -> bool {
    matches!(
        request,
        OffchainRequest::Outgoing(OutgoingRequest::Transfer(request), _)
            if request.asset_id == common::XOR.into()
    )
}

fn is_legacy_xor_request<T: Config>(
    network_id: T::NetworkId,
    request: &OffchainRequest<T>,
) -> bool {
    if network_id != T::GetEthNetworkId::get() {
        return false;
    }
    let xor_asset_id = common::XOR.into();
    match request {
        OffchainRequest::Outgoing(OutgoingRequest::Transfer(request), _) => {
            request.asset_id == xor_asset_id
        }
        OffchainRequest::Incoming(IncomingRequest::Transfer(request), _) => {
            request.asset_id == xor_asset_id
        }
        OffchainRequest::Outgoing(OutgoingRequest::AddAsset(request), _) => {
            request.asset_id == xor_asset_id
        }
        OffchainRequest::Outgoing(OutgoingRequest::AddToken(request), _) => {
            request.token_address == LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS
        }
        OffchainRequest::Incoming(IncomingRequest::AddToken(request), _) => {
            request.token_address == LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS
        }
        OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(request)) => {
            request.kind == IncomingTransactionRequestKind::TransferXOR
        }
        _ => false,
    }
}

pub fn migrate_error(err: OldDispatchError) -> DispatchError {
    match err {
        OldDispatchError::Other(s) => DispatchError::Other(s),
        OldDispatchError::CannotLookup => DispatchError::CannotLookup,
        OldDispatchError::BadOrigin => DispatchError::BadOrigin,
        OldDispatchError::Module(err) => DispatchError::Module(ModuleError {
            index: err.index,
            error: [err.error, 0, 0, 0],
            message: err.message,
        }),
        OldDispatchError::ConsumerRemaining => DispatchError::ConsumerRemaining,
        OldDispatchError::NoProviders => DispatchError::NoProviders,
        OldDispatchError::TooManyConsumers => DispatchError::TooManyConsumers,
        OldDispatchError::Token(err) => DispatchError::Token(err),
        OldDispatchError::Arithmetic(err) => DispatchError::Arithmetic(err),
    }
}

#[cfg(test)]
mod tests {
    use crate::migration::OldRequestStatus;
    use crate::requests::{IncomingRequest, OffchainRequest, RequestStatus};
    use crate::tests::mock::ExtBuilder;
    use crate::tests::mock::Runtime;
    use crate::Pallet;
    use crate::RequestStatuses;
    use bridge_types::H256;
    use codec::Encode;
    use frame_support::sp_runtime::legacy::byte_sized_error::DispatchError as OldDispatchError;
    use frame_support::sp_runtime::legacy::byte_sized_error::ModuleError as OldModuleError;
    use frame_support::sp_runtime::DispatchError;
    use frame_support::sp_runtime::ModuleError;
    use frame_support::traits::{GetStorageVersion, StorageVersion};

    #[test]
    fn request_statuses_migration_works() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 0);
            let key = RequestStatuses::<Runtime>::hashed_key_for(0, H256::from_low_u64_be(1));
            frame_support::storage::unhashed::put(&key, &OldRequestStatus::Done);
            let key = RequestStatuses::<Runtime>::hashed_key_for(0, H256::from_low_u64_be(2));
            frame_support::storage::unhashed::put(&key, &OldRequestStatus::ApprovalsReady);
            let key = RequestStatuses::<Runtime>::hashed_key_for(0, H256::from_low_u64_be(3));
            frame_support::storage::unhashed::put(
                &key,
                &OldRequestStatus::Failed(OldDispatchError::Module(OldModuleError {
                    index: 1,
                    error: 2,
                    message: Some("test"),
                })),
            );
            let key = RequestStatuses::<Runtime>::hashed_key_for(0, H256::from_low_u64_be(4));
            frame_support::storage::unhashed::put(
                &key,
                &OldRequestStatus::Broken(
                    OldDispatchError::Module(OldModuleError {
                        index: 3,
                        error: 4,
                        message: Some("test2"),
                    }),
                    OldDispatchError::Module(OldModuleError {
                        index: 5,
                        error: 6,
                        message: Some("test3"),
                    }),
                ),
            );
            super::migrate::<Runtime>();
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 3);
            assert_eq!(
                RequestStatuses::<Runtime>::get(0, H256::from_low_u64_be(1)),
                Some(RequestStatus::Done)
            );
            assert_eq!(
                RequestStatuses::<Runtime>::get(0, H256::from_low_u64_be(2)),
                Some(RequestStatus::ApprovalsReady)
            );
            assert_eq!(
                RequestStatuses::<Runtime>::get(0, H256::from_low_u64_be(3)),
                Some(RequestStatus::Failed(DispatchError::Module(ModuleError {
                    index: 1,
                    error: [2, 0, 0, 0],
                    message: Some("test"),
                })))
            );
            assert_eq!(
                RequestStatuses::<Runtime>::get(0, H256::from_low_u64_be(4)),
                Some(RequestStatus::Broken(
                    DispatchError::Module(ModuleError {
                        index: 3,
                        error: [4, 0, 0, 0],
                        message: Some("test2"),
                    }),
                    DispatchError::Module(ModuleError {
                        index: 5,
                        error: [6, 0, 0, 0],
                        message: Some("test3"),
                    }),
                )),
            );
        });
    }

    #[test]
    fn legacy_incoming_change_peers_requests_are_reencoded() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            StorageVersion::new(2).put::<Pallet<Runtime>>();
            let network_id = 0;
            let stored_hash = H256::from_low_u64_be(42);
            let peer_account_id = sp_runtime::AccountId32::new([7; 32]);
            let author = sp_runtime::AccountId32::new([8; 32]);
            let peer_address = sp_core::H160::from([9; 20]);
            let tx_hash = H256::from_low_u64_be(43);
            let old_request = super::OldOffchainRequest::Incoming(
                super::OldIncomingRequest::ChangePeers(super::OldIncomingChangePeers::<Runtime> {
                    peer_account_id: peer_account_id.clone(),
                    peer_address,
                    removed: false,
                    author: author.clone(),
                    tx_hash,
                    at_height: 11,
                    timepoint: Default::default(),
                    network_id,
                }),
                stored_hash,
            );
            let key = crate::Requests::<Runtime>::hashed_key_for(network_id, stored_hash);
            frame_support::storage::unhashed::put_raw(&key, &old_request.encode());

            let raw = frame_support::storage::unhashed::get_raw(&key).unwrap();
            assert!(super::decode_exact::<OffchainRequest<Runtime>>(&raw).is_err());
            assert!(super::decode_exact::<super::OldOffchainRequest<Runtime>>(&raw).is_ok());

            super::migrate::<Runtime>();

            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 3);
            let decoded = crate::Requests::<Runtime>::get(network_id, stored_hash).unwrap();
            assert!(super::decode_exact::<OffchainRequest<Runtime>>(
                &frame_support::storage::unhashed::get_raw(&key).unwrap()
            )
            .is_ok());
            match decoded {
                OffchainRequest::Incoming(IncomingRequest::ChangePeers(request), hash) => {
                    assert_eq!(hash, stored_hash);
                    assert_eq!(request.peer_account_id, Some(peer_account_id));
                    assert_eq!(request.peer_address, peer_address);
                    assert!(!request.removed);
                    assert_eq!(request.author, author);
                    assert_eq!(request.tx_hash, tx_hash);
                    assert_eq!(request.at_height, 11);
                    assert_eq!(request.network_id, network_id);
                }
                other => panic!("unexpected request after migration: {:?}", other),
            }
        });
    }

    #[test]
    fn request_reencoding_tolerates_current_legacy_and_corrupt_values() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            StorageVersion::new(2).put::<Pallet<Runtime>>();
            let network_id = 0;
            let current_hash = H256::from_low_u64_be(101);
            let legacy_hash = H256::from_low_u64_be(102);
            let corrupt_hash = H256::from_low_u64_be(103);

            let peer_address = sp_core::H160::from([9; 20]);
            let current_peer = sp_runtime::AccountId32::new([1; 32]);
            let legacy_peer = sp_runtime::AccountId32::new([7; 32]);
            let author = sp_runtime::AccountId32::new([8; 32]);

            let current_request = OffchainRequest::Incoming(
                IncomingRequest::ChangePeers(crate::IncomingChangePeers::<Runtime> {
                    peer_account_id: Some(current_peer.clone()),
                    peer_address,
                    removed: false,
                    author: author.clone(),
                    tx_hash: H256::from_low_u64_be(201),
                    at_height: 11,
                    timepoint: Default::default(),
                    network_id,
                }),
                current_hash,
            );
            let legacy_request = super::OldOffchainRequest::Incoming(
                super::OldIncomingRequest::ChangePeers(super::OldIncomingChangePeers::<Runtime> {
                    peer_account_id: legacy_peer.clone(),
                    peer_address,
                    removed: true,
                    author: author.clone(),
                    tx_hash: H256::from_low_u64_be(202),
                    at_height: 12,
                    timepoint: Default::default(),
                    network_id,
                }),
                legacy_hash,
            );
            let current_key = crate::Requests::<Runtime>::hashed_key_for(network_id, current_hash);
            let legacy_key = crate::Requests::<Runtime>::hashed_key_for(network_id, legacy_hash);
            let corrupt_key = crate::Requests::<Runtime>::hashed_key_for(network_id, corrupt_hash);
            let current_raw = current_request.encode();
            let legacy_raw = legacy_request.encode();
            let corrupt_raw = vec![0xff, 0x00, 0x01, 0x02];

            frame_support::storage::unhashed::put_raw(&current_key, &current_raw);
            frame_support::storage::unhashed::put_raw(&legacy_key, &legacy_raw);
            frame_support::storage::unhashed::put_raw(&corrupt_key, &corrupt_raw);

            assert!(super::decode_exact::<OffchainRequest<Runtime>>(&current_raw).is_ok());
            assert!(super::decode_exact::<OffchainRequest<Runtime>>(&legacy_raw).is_err());
            assert!(super::decode_exact::<super::OldOffchainRequest<Runtime>>(&legacy_raw).is_ok());
            assert!(super::decode_exact::<OffchainRequest<Runtime>>(&corrupt_raw).is_err());
            assert!(
                super::decode_exact::<super::OldOffchainRequest<Runtime>>(&corrupt_raw).is_err()
            );

            super::migrate::<Runtime>();

            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 3);
            assert_eq!(
                frame_support::storage::unhashed::get_raw(&current_key).unwrap(),
                current_raw
            );
            assert_eq!(
                frame_support::storage::unhashed::get_raw(&corrupt_key).unwrap(),
                corrupt_raw
            );

            let rewritten_legacy_raw =
                frame_support::storage::unhashed::get_raw(&legacy_key).unwrap();
            assert_ne!(rewritten_legacy_raw, legacy_raw);
            let decoded = super::decode_exact::<OffchainRequest<Runtime>>(&rewritten_legacy_raw)
                .expect("legacy request should be reencoded into the current format");
            match decoded {
                OffchainRequest::Incoming(IncomingRequest::ChangePeers(request), hash) => {
                    assert_eq!(hash, legacy_hash);
                    assert_eq!(request.peer_account_id, Some(legacy_peer));
                    assert_eq!(request.peer_address, peer_address);
                    assert!(request.removed);
                    assert_eq!(request.author, author);
                    assert_eq!(request.tx_hash, H256::from_low_u64_be(202));
                    assert_eq!(request.at_height, 12);
                    assert_eq!(request.network_id, network_id);
                }
                other => panic!("unexpected request after migration: {:?}", other),
            }

            let migrated_current_raw =
                frame_support::storage::unhashed::get_raw(&current_key).unwrap();
            let migrated_legacy_raw =
                frame_support::storage::unhashed::get_raw(&legacy_key).unwrap();
            let migrated_corrupt_raw =
                frame_support::storage::unhashed::get_raw(&corrupt_key).unwrap();

            super::migrate::<Runtime>();

            assert_eq!(
                frame_support::storage::unhashed::get_raw(&current_key).unwrap(),
                migrated_current_raw
            );
            assert_eq!(
                frame_support::storage::unhashed::get_raw(&legacy_key).unwrap(),
                migrated_legacy_raw
            );
            assert_eq!(
                frame_support::storage::unhashed::get_raw(&corrupt_key).unwrap(),
                migrated_corrupt_raw
            );
        });
    }
}
