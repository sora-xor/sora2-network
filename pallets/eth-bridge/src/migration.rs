use crate::requests::{
    IncomingAddToken, IncomingCancelOutgoingRequest, IncomingChangePeers,
    IncomingChangePeersCompat, IncomingMarkAsDoneRequest, IncomingMigrate,
    IncomingPrepareForMigration, IncomingTransfer, OutgoingAddAsset, RequestStatus,
};
use codec::Decode;
use codec::DecodeLength;
use codec::Encode;
use core::marker::PhantomData;
use frame_support::ensure;
use frame_support::migrations::{MigrationId, SteppedMigration, SteppedMigrationError};
use frame_support::pallet_prelude::{BoundedVec, ConstU32};
use frame_support::sp_runtime::legacy::byte_sized_error::DispatchError as OldDispatchError;
use frame_support::sp_runtime::DispatchError;
use frame_support::sp_runtime::ModuleError;
use frame_support::storage::StoragePrefixedMap;
use frame_support::traits::Get;
use frame_support::traits::GetStorageVersion;
use frame_support::traits::StorageVersion;
use frame_support::weights::{Weight, WeightMeter};
use sp_runtime::traits::BlockNumberProvider;

use crate::AccountRequests;
use crate::Config;
use crate::Pallet;
use crate::RequestApprovals;
use crate::RequestApprovers;
use crate::RequestStatuses;
use crate::RequestSubmissionHeight;
use crate::Requests;
use crate::{
    BridgeAccount, BridgeStatus, BridgeStatuses, DeprecatedSidechainTokens, Event,
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

#[derive(PartialEq, Eq, Encode, Decode, Debug, scale_info::TypeInfo)]
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
    // Reading the pallet storage version is common to every migration path.
    let mut reads = 1u64;
    let mut writes = 0u64;
    let mut on_chain_storage_version = Pallet::<T>::on_chain_storage_version();

    if on_chain_storage_version < StorageVersion::new(2) {
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

const ACCOUNT_REQUESTS_CURSOR_MAX_LEN: u32 = 128;
const MAX_LEGACY_REQUESTS_PER_ACCOUNT: usize = 65_536;
const MAX_LEGACY_ACCOUNT_REQUEST_BYTES: u32 = 4 * 1024 * 1024;
// Conservative allowance for SCALE allocation/decoding/copying per encoded byte. The real
// migration step is covered by the three compiled-Wasm `migrate_account_requests_v3_to_v4_*`
// release benchmarks; 50 steps x 20 repeats across the full valid ranges retained at least 3.2x
// ref-time margin on the release benchmark host. Keep the live-state rehearsal as an enactment
// prerequisite because malformed legacy encodings can be larger than the valid benchmark range.
const ACCOUNT_REQUEST_BYTE_REF_TIME: u64 = 50_000;
const ACCOUNT_REQUEST_SCAN_REF_TIME: u64 = 5_000_000;
const ACCOUNT_REQUEST_CURSOR_REF_TIME: u64 = 1_000_000;

type AccountRequestsMigrationCursor = BoundedVec<u8, ConstU32<ACCOUNT_REQUESTS_CURSOR_MAX_LEN>>;

#[cfg(feature = "try-runtime")]
#[derive(Encode, Decode)]
struct AccountRequestsMigrationState<AccountId> {
    previous: StorageVersion,
    expected_key_count: u64,
    bounded_history_count: u64,
    bounded_history_digest: [u8; 32],
    oversized_history_fingerprints: Vec<(AccountId, u32, [u8; 32])>,
}

/// Progressively bounds the legacy per-account RPC index while retaining its newest entries.
/// Canonical requests and statuses live in separate maps, so trimming this index does not alter
/// bridge state. The storage version is advanced only after every raw value has been screened.
pub struct AccountRequestsV3ToV4<T>(PhantomData<T>);

impl<T: Config> AccountRequestsV3ToV4<T> {
    fn base_weight() -> Weight {
        T::DbWeight::get()
            .reads(1)
            .saturating_add(Weight::from_parts(ACCOUNT_REQUEST_CURSOR_REF_TIME, 0))
    }

    fn scan_weight() -> Weight {
        T::DbWeight::get()
            .reads(2)
            .saturating_add(Weight::from_parts(ACCOUNT_REQUEST_SCAN_REF_TIME, 5))
    }

    fn encoded_value_weight(encoded_len: u32, rewrite: bool) -> Weight {
        let db_weight = if rewrite {
            T::DbWeight::get().reads_writes(1, 1)
        } else {
            T::DbWeight::get().reads(1)
        };
        db_weight.saturating_add(Weight::from_parts(
            u64::from(encoded_len).saturating_mul(ACCOUNT_REQUEST_BYTE_REF_TIME),
            u64::from(encoded_len),
        ))
    }

    fn finish_weight() -> Weight {
        T::DbWeight::get().writes(1)
    }

    fn bounded_cursor(
        key: Vec<u8>,
    ) -> Result<AccountRequestsMigrationCursor, SteppedMigrationError> {
        let cursor = AccountRequestsMigrationCursor::try_from(key)
            .map_err(|_| SteppedMigrationError::InvalidCursor)?;
        Self::validate_cursor(&cursor)?;
        Ok(cursor)
    }

    fn validate_cursor(
        cursor: &AccountRequestsMigrationCursor,
    ) -> Result<(), SteppedMigrationError> {
        let prefix = AccountRequests::<T>::final_prefix();
        let cursor = cursor.as_slice();
        let account_offset = prefix.len().saturating_add(16);
        if !cursor.starts_with(&prefix) || cursor.len() <= account_offset {
            return Err(SteppedMigrationError::InvalidCursor);
        }
        let mut input = &cursor[account_offset..];
        let account_id = <T as frame_system::Config>::AccountId::decode(&mut input)
            .map_err(|_| SteppedMigrationError::InvalidCursor)?;
        if !input.is_empty() || AccountRequests::<T>::hashed_key_for(account_id) != cursor {
            return Err(SteppedMigrationError::InvalidCursor);
        }
        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn raw_key_count() -> u64 {
        let prefix = AccountRequests::<T>::final_prefix();
        let mut previous_key = prefix.to_vec();
        let mut count = 0u64;
        while let Some(next_key) = sp_io::storage::next_key(&previous_key) {
            if !next_key.starts_with(&prefix) {
                break;
            }
            count = count.saturating_add(1);
            previous_key = next_key;
        }
        count
    }
}

impl<T: Config> SteppedMigration for AccountRequestsV3ToV4<T> {
    type Cursor = AccountRequestsMigrationCursor;
    type Identifier = MigrationId<10>;

    fn id() -> Self::Identifier {
        MigrationId {
            pallet_id: *b"eth-bridge",
            version_from: 3,
            version_to: 4,
        }
    }

    fn max_steps() -> Option<u32> {
        Some(32)
    }

    fn migrating_prefixes() -> Option<impl IntoIterator<Item = Vec<u8>>> {
        Some([AccountRequests::<T>::final_prefix().to_vec()])
    }

    fn step(
        mut cursor: Option<Self::Cursor>,
        meter: &mut WeightMeter,
    ) -> Result<Option<Self::Cursor>, SteppedMigrationError> {
        let base_weight = Self::base_weight();
        if !meter.can_consume(base_weight) {
            return Err(SteppedMigrationError::InsufficientWeight {
                required: base_weight,
            });
        }
        meter.consume(base_weight);
        let on_chain = Pallet::<T>::on_chain_storage_version();
        if on_chain >= StorageVersion::new(4) {
            return Ok(None);
        }
        if on_chain != StorageVersion::new(3) {
            frame_support::__private::log::error!(
                "EthBridge account request migration expected storage version 3, found {:?}",
                on_chain
            );
            return Err(SteppedMigrationError::Failed);
        }

        let prefix = AccountRequests::<T>::final_prefix();
        if let Some(cursor) = cursor.as_ref() {
            Self::validate_cursor(cursor)?;
        }
        let max_requests = T::MaxRequestsPerAccount::get() as usize;
        let mut made_progress = false;

        loop {
            let scan_weight = Self::scan_weight();
            let scan_and_finish_weight = scan_weight.saturating_add(Self::finish_weight());
            if !meter.can_consume(scan_and_finish_weight) {
                return if made_progress {
                    Ok(cursor)
                } else {
                    Err(SteppedMigrationError::InsufficientWeight {
                        required: base_weight.saturating_add(scan_and_finish_weight),
                    })
                };
            }

            let previous_key = cursor
                .as_ref()
                .map(|cursor| cursor.as_slice())
                .unwrap_or(prefix.as_slice());
            let next_key = sp_io::storage::next_key(previous_key);
            meter.consume(scan_weight);
            let Some(next_key) = next_key.filter(|key| key.starts_with(&prefix)) else {
                StorageVersion::new(4).put::<Pallet<T>>();
                meter.consume(Self::finish_weight());
                return Ok(None);
            };
            let next_cursor = Self::bounded_cursor(next_key.clone())?;

            let mut encoded_prefix = [0u8; 5];
            let Some(encoded_len) = sp_io::storage::read(&next_key, &mut encoded_prefix, 0) else {
                return Err(SteppedMigrationError::Failed);
            };
            let prefix_len = core::cmp::min(encoded_len as usize, encoded_prefix.len());
            let request_count = <Vec<(T::NetworkId, sp_core::H256)> as DecodeLength>::len(
                &encoded_prefix[..prefix_len],
            )
            .map_err(|_| SteppedMigrationError::Failed)?;
            if request_count > MAX_LEGACY_REQUESTS_PER_ACCOUNT
                || encoded_len > MAX_LEGACY_ACCOUNT_REQUEST_BYTES
            {
                frame_support::__private::log::error!(
                    "EthBridge AccountRequests value exceeds the audited legacy migration ceiling: entries={}, encoded_bytes={}",
                    request_count,
                    encoded_len
                );
                return Err(SteppedMigrationError::Failed);
            }

            let rewrite = max_requests == 0 || request_count > max_requests;
            let value_weight = Self::encoded_value_weight(encoded_len, rewrite);
            if !meter.can_consume(value_weight) {
                return if made_progress {
                    Ok(cursor)
                } else {
                    Err(SteppedMigrationError::InsufficientWeight {
                        required: base_weight
                            .saturating_add(scan_weight)
                            .saturating_add(value_weight),
                    })
                };
            }
            let raw = sp_io::storage::get(&next_key).ok_or(SteppedMigrationError::Failed)?;
            if raw.len() != encoded_len as usize {
                return Err(SteppedMigrationError::Failed);
            }
            let mut input = &raw[..];
            let mut requests = Vec::<(T::NetworkId, sp_core::H256)>::decode(&mut input)
                .map_err(|_| SteppedMigrationError::Failed)?;
            if !input.is_empty() || requests.len() != request_count {
                return Err(SteppedMigrationError::Failed);
            }
            if rewrite {
                let retained = requests.split_off(requests.len().saturating_sub(max_requests));
                if retained.is_empty() {
                    sp_io::storage::clear(&next_key);
                } else {
                    sp_io::storage::set(&next_key, &retained.encode());
                }
            }
            meter.consume(value_weight);

            cursor = Some(next_cursor);
            made_progress = true;
        }
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
        let max_requests = T::MaxRequestsPerAccount::get() as usize;
        let raw_key_count = Self::raw_key_count();
        let mut decoded_key_count = 0u64;
        let mut bounded_history_count = 0u64;
        let mut bounded_history_digest = [0u8; 32];
        let mut oversized_history_fingerprints = Vec::new();

        for (account_id, requests) in AccountRequests::<T>::iter() {
            decoded_key_count = decoded_key_count.saturating_add(1);
            if requests.len() > MAX_LEGACY_REQUESTS_PER_ACCOUNT {
                return Err(
                    "EthBridge legacy account history exceeds the migration entry ceiling".into(),
                );
            }
            if requests.encoded_size() > MAX_LEGACY_ACCOUNT_REQUEST_BYTES as usize {
                return Err(
                    "EthBridge legacy account history exceeds the migration byte ceiling".into(),
                );
            }

            if max_requests == 0 || requests.len() > max_requests {
                let retained = &requests[requests.len().saturating_sub(max_requests)..];
                oversized_history_fingerprints.push((
                    account_id,
                    retained.len() as u32,
                    sp_io::hashing::blake2_256(&retained.encode()),
                ));
            } else {
                bounded_history_count = bounded_history_count.saturating_add(1);
                bounded_history_digest = sp_io::hashing::blake2_256(
                    &(bounded_history_digest, &account_id, &requests).encode(),
                );
            }
        }

        if decoded_key_count != raw_key_count {
            return Err("EthBridge AccountRequests contains an undecodable legacy value".into());
        }
        let expected_key_count = if max_requests == 0 { 0 } else { raw_key_count };
        Ok(
            AccountRequestsMigrationState::<<T as frame_system::Config>::AccountId> {
                previous: Pallet::<T>::on_chain_storage_version(),
                expected_key_count,
                bounded_history_count,
                bounded_history_digest,
                oversized_history_fingerprints,
            }
            .encode(),
        )
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        let state =
            AccountRequestsMigrationState::<<T as frame_system::Config>::AccountId>::decode(
                &mut &state[..],
            )
            .map_err(|_| "Failed to decode EthBridge AccountRequests migration state")?;
        let expected_version = if state.previous < StorageVersion::new(4) {
            StorageVersion::new(4)
        } else {
            state.previous
        };
        if Pallet::<T>::on_chain_storage_version() != expected_version {
            return Err(
                "EthBridge AccountRequests migration reached an unexpected storage version".into(),
            );
        }
        if Self::raw_key_count() != state.expected_key_count {
            return Err(
                "EthBridge AccountRequests migration changed the unexpected number of keys".into(),
            );
        }

        for (account_id, retained_len, retained_hash) in &state.oversized_history_fingerprints {
            let requests = AccountRequests::<T>::get(account_id);
            if requests.len() != *retained_len as usize
                || sp_io::hashing::blake2_256(&requests.encode()) != *retained_hash
            {
                return Err(
                    "EthBridge AccountRequests migration did not retain the newest entries".into(),
                );
            }
        }

        let max_requests = T::MaxRequestsPerAccount::get() as usize;
        let mut decoded_key_count = 0u64;
        let mut bounded_history_count = 0u64;
        let mut bounded_history_digest = [0u8; 32];
        for (account_id, requests) in AccountRequests::<T>::iter() {
            decoded_key_count = decoded_key_count.saturating_add(1);
            if requests.len() > max_requests {
                return Err("EthBridge AccountRequests migration left an oversized history".into());
            }
            let was_oversized = state
                .oversized_history_fingerprints
                .iter()
                .any(|(stored_account_id, _, _)| stored_account_id == &account_id);
            if !was_oversized {
                bounded_history_count = bounded_history_count.saturating_add(1);
                bounded_history_digest = sp_io::hashing::blake2_256(
                    &(bounded_history_digest, &account_id, &requests).encode(),
                );
            }
        }
        if decoded_key_count != state.expected_key_count
            || bounded_history_count != state.bounded_history_count
            || bounded_history_digest != state.bounded_history_digest
        {
            return Err("EthBridge AccountRequests migration modified bounded histories".into());
        }
        Ok(())
    }
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
    Pallet::<T>::record_account_request(request.author(), net_id, hash);
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
    use crate::AccountRequests;
    use crate::Pallet;
    use crate::RequestStatuses;
    use bridge_types::H256;
    use codec::Encode;
    use frame_support::migrations::{SteppedMigration, SteppedMigrationError};
    use frame_support::sp_runtime::legacy::byte_sized_error::DispatchError as OldDispatchError;
    use frame_support::sp_runtime::legacy::byte_sized_error::ModuleError as OldModuleError;
    use frame_support::sp_runtime::DispatchError;
    use frame_support::sp_runtime::ModuleError;
    use frame_support::traits::{GetStorageVersion, StorageVersion};
    use frame_support::weights::{Weight, WeightMeter};

    fn run_account_requests_v4_migration() {
        let mut cursor = None;
        loop {
            let mut meter = WeightMeter::with_limit(Weight::from_parts(u64::MAX, u64::MAX));
            cursor = <super::AccountRequestsV3ToV4<Runtime> as SteppedMigration>::step(
                cursor, &mut meter,
            )
            .expect("account request migration should succeed");
            if cursor.is_none() {
                break;
            }
        }
    }

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
    fn account_request_histories_keep_the_newest_bounded_entries() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            StorageVersion::new(3).put::<Pallet<Runtime>>();
            let oversized_account = sp_runtime::AccountId32::new([1; 32]);
            let bounded_account = sp_runtime::AccountId32::new([2; 32]);
            let history = (1..=5)
                .map(|value| (0, H256::from_low_u64_be(value)))
                .collect::<Vec<_>>();
            let bounded_history = vec![
                (0, H256::from_low_u64_be(10)),
                (0, H256::from_low_u64_be(11)),
            ];
            AccountRequests::<Runtime>::insert(&oversized_account, history);
            AccountRequests::<Runtime>::insert(&bounded_account, bounded_history.clone());

            run_account_requests_v4_migration();

            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 4);
            assert_eq!(
                AccountRequests::<Runtime>::get(&oversized_account),
                vec![
                    (0, H256::from_low_u64_be(3)),
                    (0, H256::from_low_u64_be(4)),
                    (0, H256::from_low_u64_be(5)),
                ]
            );
            assert_eq!(
                AccountRequests::<Runtime>::get(&bounded_account),
                bounded_history
            );

            let migrated_history = AccountRequests::<Runtime>::get(&oversized_account);
            run_account_requests_v4_migration();
            assert_eq!(
                AccountRequests::<Runtime>::get(&oversized_account),
                migrated_history
            );
        });
    }

    #[test]
    fn account_request_migration_resumes_and_only_sets_v4_at_the_end() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            StorageVersion::new(3).put::<Pallet<Runtime>>();
            let expected = vec![
                (0, H256::from_low_u64_be(2)),
                (0, H256::from_low_u64_be(3)),
                (0, H256::from_low_u64_be(4)),
            ];
            for byte in 1..=3 {
                AccountRequests::<Runtime>::insert(
                    sp_runtime::AccountId32::new([byte; 32]),
                    vec![
                        (0, H256::from_low_u64_be(1)),
                        (0, H256::from_low_u64_be(2)),
                        (0, H256::from_low_u64_be(3)),
                        (0, H256::from_low_u64_be(4)),
                    ],
                );
            }

            let encoded_len = AccountRequests::<Runtime>::get(sp_runtime::AccountId32::new([1; 32]))
                .encoded_size() as u32;
            let per_step_limit = super::AccountRequestsV3ToV4::<Runtime>::base_weight()
                .saturating_add(super::AccountRequestsV3ToV4::<Runtime>::scan_weight())
                .saturating_add(
                    super::AccountRequestsV3ToV4::<Runtime>::encoded_value_weight(
                        encoded_len,
                        true,
                    ),
                )
                .saturating_add(super::AccountRequestsV3ToV4::<Runtime>::finish_weight());
            let mut cursor = None;
            let mut steps = 0u32;
            loop {
                let mut meter = WeightMeter::with_limit(per_step_limit);
                cursor = <super::AccountRequestsV3ToV4<Runtime> as SteppedMigration>::step(
                    cursor, &mut meter,
                )
                .expect("bounded migration step should succeed");
                steps = steps.saturating_add(1);
                if cursor.is_none() {
                    break;
                }
                assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 3);
            }

            assert!(steps >= 2);
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 4);
            for byte in 1..=3 {
                assert_eq!(
                    AccountRequests::<Runtime>::get(sp_runtime::AccountId32::new([byte; 32])),
                    expected
                );
            }
        });
    }

    #[test]
    fn account_request_migration_rejects_invalid_cursor() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            StorageVersion::new(3).put::<Pallet<Runtime>>();
            let invalid_cursor =
                super::AccountRequestsMigrationCursor::try_from(vec![0u8; 33]).unwrap();
            let mut meter = WeightMeter::with_limit(Weight::from_parts(u64::MAX, u64::MAX));
            assert_eq!(
                <super::AccountRequestsV3ToV4<Runtime> as SteppedMigration>::step(
                    Some(invalid_cursor),
                    &mut meter,
                ),
                Err(SteppedMigrationError::InvalidCursor)
            );
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 3);
        });
    }

    #[test]
    fn account_request_migration_rejects_malformed_bounded_value() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            StorageVersion::new(3).put::<Pallet<Runtime>>();
            assert_eq!(AccountRequests::<Runtime>::iter_keys().count(), 0);
            let account_id = sp_runtime::AccountId32::new([9; 32]);
            let key = AccountRequests::<Runtime>::hashed_key_for(&account_id);
            let malformed = vec![0x04]; // Claims one element but omits the tuple.
            frame_support::storage::unhashed::put_raw(&key, &malformed);

            let mut meter = WeightMeter::with_limit(Weight::from_parts(u64::MAX, u64::MAX));
            assert_eq!(
                <super::AccountRequestsV3ToV4<Runtime> as SteppedMigration>::step(
                    None, &mut meter,
                ),
                Err(SteppedMigrationError::Failed)
            );
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 3);
            assert_eq!(
                frame_support::storage::unhashed::get_raw(&key),
                Some(malformed)
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
