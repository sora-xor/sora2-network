use crate::*;
use codec::Encode;
use frame_remote_externalities::{Builder, Mode, OfflineConfig, OnlineConfig, SnapshotConfig};
use frame_support::migrations::{MultiStepMigrator, SteppedMigration};
use frame_support::storage::storage_prefix;
use frame_support::traits::{GetStorageVersion, StorageVersion};
use frame_support::weights::{Weight, WeightMeter};
use sp_runtime::traits::{Block as BlockT, Header as HeaderT};
use std::collections::BTreeMap;
use std::env::var;

const DEFAULT_REMOTE_RPC_URL: &str = "wss://ws.mof.sora.org";

fn env_flag(name: &str, default: bool) -> bool {
    var(name)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(default)
}

fn env_csv(name: &str) -> Vec<String> {
    var(name)
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn decode_hex_prefixed(value: &str) -> Vec<u8> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    assert!(
        value.len() % 2 == 0,
        "hex input must contain an even number of digits"
    );

    value
        .as_bytes()
        .chunks_exact(2)
        .map(|byte| {
            let hex = std::str::from_utf8(byte).expect("hex input should be valid UTF-8");
            u8::from_str_radix(hex, 16).expect("hex input should contain only hex digits")
        })
        .collect()
}

fn env_hex_csv(name: &str) -> Vec<Vec<u8>> {
    env_csv(name)
        .into_iter()
        .map(|value| decode_hex_prefixed(&value))
        .collect()
}

fn remote_block_hash() -> Option<<Block as BlockT>::Hash> {
    var("REMOTE_BLOCK_HASH").ok().map(|value| {
        value
            .trim()
            .parse()
            .expect("REMOTE_BLOCK_HASH must be a 32-byte hex block hash")
    })
}

fn assert_requested_remote_block(
    requested: Option<<Block as BlockT>::Hash>,
    actual: <Block as BlockT>::Hash,
) {
    if let Some(requested) = requested {
        // OfflineOrElseOnline loads an existing snapshot before consulting OnlineConfig.at.
        // Reject stale snapshots rather than silently rehearsing a different release state.
        assert_eq!(
            actual, requested,
            "remote snapshot block does not match REMOTE_BLOCK_HASH"
        );
    }
}

fn remote_mode(
    transport_uri: String,
    maybe_state_snapshot: Option<SnapshotConfig>,
    pallets: Vec<String>,
    hashed_prefixes: Vec<Vec<u8>>,
    hashed_keys: Vec<Vec<u8>>,
    child_trie: bool,
    at: Option<<Block as BlockT>::Hash>,
) -> Mode<<Block as BlockT>::Hash> {
    if let Some(state_snapshot) = maybe_state_snapshot {
        Mode::OfflineOrElseOnline(
            OfflineConfig {
                state_snapshot: state_snapshot.clone(),
            },
            OnlineConfig {
                at,
                transport_uris: vec![transport_uri],
                state_snapshot: Some(state_snapshot),
                pallets,
                hashed_prefixes,
                hashed_keys,
                child_trie,
            },
        )
    } else {
        Mode::Online(OnlineConfig {
            at,
            transport_uris: vec![transport_uri],
            pallets,
            hashed_prefixes,
            hashed_keys,
            child_trie,
            ..Default::default()
        })
    }
}

fn replay_multi_block_migrations() -> u32 {
    let mut steps = 0u32;
    while <Runtime as frame_system::Config>::MultiBlockMigrator::ongoing() {
        // pallet-migrations enforces max_steps through elapsed block numbers. Repeatedly
        // stepping at one block number would bypass the production migration deadline.
        System::set_block_number(
            System::block_number()
                .checked_add(1)
                .expect("remote migration rehearsal exhausted block numbers"),
        );
        <Runtime as frame_system::Config>::MultiBlockMigrator::step();
        steps = steps.saturating_add(1);
        assert!(
            steps <= 4096,
            "multi-block migrations did not finish after {steps} steps"
        );
    }
    steps
}

fn eth_bridge_migration_hashed_prefixes() -> Vec<Vec<u8>> {
    vec![
        storage_prefix(b"EthBridge", b"LoadToIncomingRequestHash").to_vec(),
        storage_prefix(b"EthBridge", b"AccountRequests").to_vec(),
        storage_prefix(b"EthBridge", b"Peers").to_vec(),
        storage_prefix(b"EthBridge", b"RequestApprovals").to_vec(),
        storage_prefix(b"EthBridge", b"RequestApprovers").to_vec(),
        storage_prefix(b"EthBridge", b"RequestsQueue").to_vec(),
        storage_prefix(b"EthBridge", b"RequestStatuses").to_vec(),
        storage_prefix(b"EthBridge", b"Requests").to_vec(),
    ]
}

fn eth_bridge_storage_version_key() -> Vec<u8> {
    StorageVersion::storage_key::<eth_bridge::Pallet<Runtime>>().to_vec()
}

fn eth_bridge_migration_hashed_keys() -> Vec<Vec<u8>> {
    vec![eth_bridge_storage_version_key()]
}

fn assert_eth_bridge_rehearsal_starts_from_live_4_8_8() {
    let on_chain = eth_bridge::Pallet::<Runtime>::on_chain_storage_version();
    assert_eq!(
        on_chain,
        StorageVersion::new(3),
        "EthBridge live storage version should match the 4.8.8 release state (v3)"
    );
}

fn incoming_replay_key(request: &eth_bridge::requests::IncomingRequest<Runtime>) -> sp_core::H256 {
    use eth_bridge::requests::IncomingRequest;

    match request {
        IncomingRequest::Transfer(request) => request.tx_hash,
        IncomingRequest::AddToken(request) => request.tx_hash,
        IncomingRequest::ChangePeers(request) => request.tx_hash,
        IncomingRequest::CancelOutgoingRequest(request) => request.initial_request_hash,
        IncomingRequest::MarkAsDone(request) => request.initial_request_hash,
        IncomingRequest::PrepareForMigration(request) => request.tx_hash,
        IncomingRequest::Migrate(request) => request.tx_hash,
        IncomingRequest::ChangePeersCompat(request) => request.tx_hash,
    }
}

fn assert_eth_bridge_replay_index_is_consistent() {
    let mut incoming_by_replay_key =
        BTreeMap::<(NetworkId, sp_core::H256), Vec<sp_core::H256>>::new();
    let mut replay_key_by_incoming_hash =
        BTreeMap::<(NetworkId, sp_core::H256), sp_core::H256>::new();

    for (network_id, storage_hash, request) in eth_bridge::Requests::<Runtime>::iter() {
        let Some((incoming, embedded_hash)) = request.as_incoming() else {
            continue;
        };
        assert_eq!(
            storage_hash, embedded_hash,
            "EthBridge incoming request storage key does not match its embedded hash"
        );
        assert_eq!(
            network_id,
            incoming.network_id(),
            "EthBridge incoming request is stored under the wrong network"
        );
        let replay_key = incoming_replay_key(incoming);
        incoming_by_replay_key
            .entry((network_id, replay_key))
            .or_default()
            .push(storage_hash);
        replay_key_by_incoming_hash.insert((network_id, storage_hash), replay_key);
    }

    let duplicate_requests = incoming_by_replay_key
        .iter()
        .filter(|(_, hashes)| hashes.len() > 1)
        .map(|((network_id, replay_key), hashes)| {
            format!("network={network_id:?}, replay_key={replay_key:?}, incoming_hashes={hashes:?}")
        })
        .collect::<Vec<_>>();
    assert!(
        duplicate_requests.is_empty(),
        "EthBridge has multiple incoming requests for one replay key:\n{}",
        duplicate_requests.join("\n")
    );

    let mut inconsistent_mappings = Vec::new();
    for (network_id, replay_key, incoming_hash) in
        eth_bridge::LoadToIncomingRequestHash::<Runtime>::iter()
    {
        match replay_key_by_incoming_hash.get(&(network_id, incoming_hash)) {
            Some(indexed_replay_key) if *indexed_replay_key == replay_key => {}
            indexed_replay_key => inconsistent_mappings.push(format!(
                "network={network_id:?}, replay_key={replay_key:?}, incoming_hash={incoming_hash:?}, indexed_replay_key={indexed_replay_key:?}, status={:?}",
                eth_bridge::RequestStatuses::<Runtime>::get(network_id, incoming_hash)
            )),
        }
    }
    for ((network_id, replay_key), incoming_hashes) in &incoming_by_replay_key {
        let mapping_exists =
            eth_bridge::LoadToIncomingRequestHash::<Runtime>::contains_key(network_id, replay_key);
        let mapped_hash =
            eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(network_id, replay_key);
        if !mapping_exists || incoming_hashes.as_slice() != [mapped_hash] {
            inconsistent_mappings.push(format!(
                "network={network_id:?}, replay_key={replay_key:?}, incoming_hashes={incoming_hashes:?}, mapped_hash={mapped_hash:?}, mapping_exists={mapping_exists}"
            ));
        }
    }
    assert!(
        inconsistent_mappings.is_empty(),
        "EthBridge replay index is incomplete or stale and requires a bounded migration:\n{}",
        inconsistent_mappings.join("\n")
    );
}

fn assert_eth_bridge_collections_are_bounded(allow_legacy_account_histories: bool) {
    let max_requests = <Runtime as eth_bridge::Config>::MaxRequestsPerAccount::get() as usize;
    let max_queue = <Runtime as eth_bridge::Config>::MaxRequestsPerQueue::get() as usize;
    let max_peers = eth_bridge::requests::MAX_PEERS;
    let mut maximum_account_requests = 0usize;
    let mut maximum_queue = 0usize;
    let mut maximum_peers = 0usize;
    let mut maximum_approvals = 0usize;
    let mut maximum_approvers = 0usize;
    let mut total_account_request_entries = 0usize;
    let mut total_account_request_encoded_bytes = 0usize;
    let mut oversized = Vec::new();

    for (account_id, requests) in eth_bridge::AccountRequests::<Runtime>::iter() {
        maximum_account_requests = maximum_account_requests.max(requests.len());
        total_account_request_entries =
            total_account_request_entries.saturating_add(requests.len());
        total_account_request_encoded_bytes =
            total_account_request_encoded_bytes.saturating_add(requests.encoded_size());
        if requests.len() > max_requests && !allow_legacy_account_histories {
            oversized.push(format!(
                "account={account_id:?}, request_count={}, max_requests={max_requests}",
                requests.len()
            ));
        }
    }
    for (network_id, queue) in eth_bridge::RequestsQueue::<Runtime>::iter() {
        maximum_queue = maximum_queue.max(queue.len());
        if queue.len() > max_queue {
            oversized.push(format!(
                "network={network_id:?}, queue_count={}, max_queue={max_queue}",
                queue.len()
            ));
        }
    }
    for (network_id, peers) in eth_bridge::Peers::<Runtime>::iter() {
        maximum_peers = maximum_peers.max(peers.len());
        if peers.len() > max_peers {
            oversized.push(format!(
                "network={network_id:?}, peer_count={}, max_peers={max_peers}",
                peers.len()
            ));
        }
    }
    for (network_id, request_hash, approvals) in eth_bridge::RequestApprovals::<Runtime>::iter() {
        maximum_approvals = maximum_approvals.max(approvals.len());
        if approvals.len() > max_peers {
            oversized.push(format!(
                "network={network_id:?}, request_hash={request_hash:?}, approval_count={}, max_approvals={max_peers}",
                approvals.len()
            ));
        }
    }
    for (network_id, request_hash, approvers) in eth_bridge::RequestApprovers::<Runtime>::iter() {
        maximum_approvers = maximum_approvers.max(approvers.len());
        if approvers.len() > max_peers {
            oversized.push(format!(
                "network={network_id:?}, request_hash={request_hash:?}, approver_count={}, max_approvers={max_peers}",
                approvers.len()
            ));
        }
    }

    eprintln!(
        "EthBridge live maxima: account_requests={maximum_account_requests}/{max_requests}, queue={maximum_queue}/{max_queue}, peers={maximum_peers}/{max_peers}, approvals={maximum_approvals}/{max_peers}, approvers={maximum_approvers}/{max_peers}; account_request_entries={total_account_request_entries}, encoded_bytes={total_account_request_encoded_bytes}, allow_legacy_account_histories={allow_legacy_account_histories}"
    );
    assert!(
        oversized.is_empty(),
        "EthBridge has oversized collections that require a bounded migration:\n{}",
        oversized.join("\n")
    );
}

pub(crate) async fn remote_try_runtime_upgrade_rehearsal() {
    sp_tracing::try_init_simple();
    let require_remote = env_flag("REQUIRE_REMOTE", false);
    let remote_requested = require_remote
        || var("REMOTE_RPC_URL").is_ok()
        || var("WS").is_ok()
        || var("SNAP").is_ok()
        || var("REMOTE_PALLETS").is_ok()
        || var("REMOTE_HASHED_PREFIXES").is_ok()
        || var("REMOTE_HASHED_KEYS").is_ok()
        || var("REMOTE_BLOCK_HASH").is_ok();

    if !remote_requested {
        eprintln!("Skipping remote migration test: set REQUIRE_REMOTE=1 to run it");
        return;
    }

    let transport_uri = var("REMOTE_RPC_URL")
        .or_else(|_| var("WS"))
        .unwrap_or(DEFAULT_REMOTE_RPC_URL.to_string());
    let maybe_state_snapshot: Option<SnapshotConfig> = var("SNAP").map(|s| s.into()).ok();
    let pallets = env_csv("REMOTE_PALLETS");
    let hashed_prefixes = env_hex_csv("REMOTE_HASHED_PREFIXES");
    let hashed_keys = env_hex_csv("REMOTE_HASHED_KEYS");
    let child_trie = env_flag("REMOTE_CHILD_TRIE", true);
    let requested_at = remote_block_hash();
    let builder = Builder::<Block>::default()
        .mode(remote_mode(
            transport_uri,
            maybe_state_snapshot,
            pallets,
            hashed_prefixes,
            hashed_keys,
            child_trie,
            requested_at,
        ))
        .build();

    let mut ext = match builder.await {
        Ok(ext) => ext,
        Err(err) => {
            if require_remote {
                panic!("failed to build remote externalities: {err}");
            }
            eprintln!(
                "Skipping remote migration test: failed to build remote externalities: {err}"
            );
            return;
        }
    };
    let replay_hash = ext.header.hash();
    let replay_block = *ext.header.number();
    assert_requested_remote_block(requested_at, replay_hash);
    eprintln!("Runtime upgrade rehearsal state: block={replay_block}, hash={replay_hash:?}");
    ext.execute_with(|| {
        // Filtered snapshots may omit System::Number; the captured header is authoritative.
        System::set_block_number(replay_block);
        assert_eth_bridge_replay_index_is_consistent();
        assert_eth_bridge_collections_are_bounded(true);
        let upgrade_weight = Executive::execute_on_runtime_upgrade();
        let steps = replay_multi_block_migrations();
        eprintln!(
            "Runtime upgrade rehearsal replay: source_block={replay_block}, final_block={}, migration_steps={steps}, upgrade_weight={upgrade_weight:?}",
            System::block_number()
        );
        let mut storage_version_mismatches = std::vec::Vec::new();
        macro_rules! check_storage_version {
            ($label:literal, $pallet:ty) => {{
                let on_chain = <$pallet>::on_chain_storage_version();
                let in_code = <$pallet>::in_code_storage_version();
                if on_chain != in_code {
                    storage_version_mismatches.push(format!(
                        "{}: on-chain {:?} != in-code {:?}",
                        $label, on_chain, in_code
                    ));
                }
            }};
        }

        check_storage_version!("XorFee", xor_fee::Pallet<Runtime>);
        check_storage_version!("Staking", pallet_staking::Pallet<Runtime>);
        check_storage_version!("Offences", pallet_offences::Pallet<Runtime>);
        check_storage_version!("Session", pallet_session::Pallet<Runtime>);
        check_storage_version!("Grandpa", pallet_grandpa::Pallet<Runtime>);
        check_storage_version!("ImOnline", pallet_im_online::Pallet<Runtime>);
        check_storage_version!("PoolXYK", pool_xyk::Pallet<Runtime>);
        check_storage_version!("PswapDistribution", pswap_distribution::Pallet<Runtime>);
        check_storage_version!("VestedRewards", vested_rewards::Pallet<Runtime>);
        check_storage_version!("Identity", pallet_identity::Pallet<Runtime>);
        check_storage_version!("Farming", farming::Pallet<Runtime>);
        check_storage_version!("Kensetsu", kensetsu::Pallet<Runtime>);
        check_storage_version!("Band", band::Pallet<Runtime>);
        check_storage_version!("Polkamarkt", pallet_polkamarkt::Pallet<Runtime>);
        check_storage_version!("EthBridge", eth_bridge::Pallet<Runtime>);
        check_storage_version!("OracleProxy", oracle_proxy::Pallet<Runtime>);
        check_storage_version!(
            "BridgeInboundChannel",
            bridge_channel::inbound::Pallet<Runtime>
        );
        check_storage_version!(
            "SubstrateBridgeInboundChannel",
            substrate_bridge_channel::inbound::Pallet<Runtime>
        );
        check_storage_version!(
            "SubstrateBridgeOutboundChannel",
            substrate_bridge_channel::outbound::Pallet<Runtime>
        );

        assert!(
            storage_version_mismatches.is_empty(),
            "storage version mismatches after remote runtime upgrade:\n{}",
            storage_version_mismatches.join("\n")
        );
        assert_eth_bridge_replay_index_is_consistent();
        assert_eth_bridge_collections_are_bounded(false);
    });
}

pub(crate) async fn remote_eth_bridge_migration_rehearsal() {
    sp_tracing::try_init_simple();
    let require_remote = env_flag("REQUIRE_REMOTE", false);
    let remote_requested = require_remote
        || var("REMOTE_RPC_URL").is_ok()
        || var("WS").is_ok()
        || var("SNAP").is_ok()
        || var("REMOTE_BLOCK_HASH").is_ok();

    if !remote_requested {
        eprintln!("Skipping EthBridge remote migration test: set REQUIRE_REMOTE=1 to run it");
        return;
    }

    let transport_uri = var("REMOTE_RPC_URL")
        .or_else(|_| var("WS"))
        .unwrap_or(DEFAULT_REMOTE_RPC_URL.to_string());
    let maybe_state_snapshot: Option<SnapshotConfig> = var("SNAP").map(|s| s.into()).ok();
    let hashed_prefixes = eth_bridge_migration_hashed_prefixes();
    let hashed_keys = eth_bridge_migration_hashed_keys();
    let requested_at = remote_block_hash();
    let builder = Builder::<Block>::default()
        .mode(remote_mode(
            transport_uri,
            maybe_state_snapshot,
            Vec::new(),
            hashed_prefixes,
            hashed_keys,
            false,
            requested_at,
        ))
        .build();

    let mut ext = match builder.await {
        Ok(ext) => ext,
        Err(err) => {
            if require_remote {
                panic!("failed to build EthBridge remote externalities: {err}");
            }
            eprintln!(
                "Skipping EthBridge remote migration test: failed to build remote externalities: {err}"
            );
            return;
        }
    };

    let replay_hash = ext.header.hash();
    let replay_block = *ext.header.number();
    assert_requested_remote_block(requested_at, replay_hash);
    eprintln!("EthBridge rehearsal state: block={replay_block}, hash={replay_hash:?}");
    ext.execute_with(|| {
        System::set_block_number(replay_block);
        let replay_mapping_count =
            eth_bridge::LoadToIncomingRequestHash::<Runtime>::iter_keys().count();
        let account_request_count = eth_bridge::AccountRequests::<Runtime>::iter_keys().count();
        let peer_network_count = eth_bridge::Peers::<Runtime>::iter_keys().count();
        let approval_count = eth_bridge::RequestApprovals::<Runtime>::iter_keys().count();
        let approver_count = eth_bridge::RequestApprovers::<Runtime>::iter_keys().count();
        let queue_network_count = eth_bridge::RequestsQueue::<Runtime>::iter_keys().count();
        let request_count = eth_bridge::Requests::<Runtime>::iter_keys().count();
        let status_count = eth_bridge::RequestStatuses::<Runtime>::iter_keys().count();
        assert_ne!(
            replay_mapping_count, 0,
            "EthBridge::LoadToIncomingRequestHash live-state prefix loaded no keys"
        );
        assert_ne!(
            account_request_count, 0,
            "EthBridge::AccountRequests live-state prefix loaded no keys"
        );
        assert_ne!(
            peer_network_count, 0,
            "EthBridge::Peers live-state prefix loaded no keys"
        );
        assert_ne!(
            queue_network_count, 0,
            "EthBridge::RequestsQueue live-state prefix loaded no keys"
        );
        assert_ne!(
            request_count, 0,
            "EthBridge::Requests live-state prefix loaded no keys"
        );
        assert_ne!(
            status_count, 0,
            "EthBridge::RequestStatuses live-state prefix loaded no keys"
        );
        assert_eth_bridge_rehearsal_starts_from_live_4_8_8();
        assert_eth_bridge_replay_index_is_consistent();
        assert_eth_bridge_collections_are_bounded(true);

        let max_requests =
            <Runtime as eth_bridge::Config>::MaxRequestsPerAccount::get() as usize;
        let expected_account_request_tails = eth_bridge::AccountRequests::<Runtime>::iter()
            .filter(|(_, requests)| requests.len() > max_requests)
            .map(|(account_id, requests)| {
                let retained = requests[requests.len().saturating_sub(max_requests)..].to_vec();
                (account_id, retained)
            })
            .collect::<BTreeMap<_, _>>();

        type AccountRequestsMigration =
            eth_bridge::migration::AccountRequestsV3ToV4<Runtime>;
        let state = <AccountRequestsMigration as SteppedMigration>::pre_upgrade()
            .expect("EthBridge pre-upgrade should fingerprint live account histories");
        let migration_started = std::time::Instant::now();
        let service_weight = MigrationMaxServiceWeight::get();
        let mut migration_weight = Weight::zero();
        let mut migration_steps = 0u32;
        let mut cursor = None;
        loop {
            let mut meter = WeightMeter::with_limit(service_weight);
            cursor = <AccountRequestsMigration as SteppedMigration>::step(cursor, &mut meter)
                .expect("EthBridge account history migration step should succeed");
            assert!(
                meter.consumed().all_lte(service_weight),
                "EthBridge migration step exceeded its service budget"
            );
            migration_weight.saturating_accrue(meter.consumed());
            migration_steps = migration_steps.saturating_add(1);
            assert!(
                migration_steps <= 32,
                "EthBridge account history migration exceeded its maximum step count"
            );
            if cursor.is_none() {
                break;
            }
        }
        let migration_elapsed = migration_started.elapsed();
        <AccountRequestsMigration as SteppedMigration>::post_upgrade(state)
            .expect("EthBridge post-upgrade should validate retained account history suffixes");
        eprintln!(
            "EthBridge v3->v4 migration: steps={migration_steps}, total_weight={migration_weight:?}, per_block_limit={service_weight:?}, elapsed={migration_elapsed:?}, truncated_accounts={}",
            expected_account_request_tails.len()
        );

        for (account_id, expected_tail) in expected_account_request_tails {
            assert_eq!(
                eth_bridge::AccountRequests::<Runtime>::get(&account_id),
                expected_tail,
                "EthBridge migration did not retain the newest account request entries"
            );
        }
        assert_eth_bridge_replay_index_is_consistent();
        assert_eth_bridge_collections_are_bounded(false);

        assert_eq!(
            eth_bridge::Pallet::<Runtime>::on_chain_storage_version(),
            StorageVersion::new(4)
        );
        assert_eq!(
            eth_bridge::LoadToIncomingRequestHash::<Runtime>::iter_keys().count(),
            replay_mapping_count,
            "EthBridge::LoadToIncomingRequestHash key count changed during migration"
        );
        assert_eq!(
            eth_bridge::AccountRequests::<Runtime>::iter_keys().count(),
            account_request_count,
            "EthBridge::AccountRequests key count changed during migration"
        );
        assert_eq!(
            eth_bridge::Peers::<Runtime>::iter_keys().count(),
            peer_network_count,
            "EthBridge::Peers key count changed during migration"
        );
        assert_eq!(
            eth_bridge::RequestApprovals::<Runtime>::iter_keys().count(),
            approval_count,
            "EthBridge::RequestApprovals key count changed during migration"
        );
        assert_eq!(
            eth_bridge::RequestApprovers::<Runtime>::iter_keys().count(),
            approver_count,
            "EthBridge::RequestApprovers key count changed during migration"
        );
        assert_eq!(
            eth_bridge::RequestsQueue::<Runtime>::iter_keys().count(),
            queue_network_count,
            "EthBridge::RequestsQueue key count changed during migration"
        );
        assert_eq!(
            eth_bridge::Requests::<Runtime>::iter_keys().count(),
            request_count,
            "EthBridge::Requests key count changed during migration"
        );
        assert_eq!(
            eth_bridge::RequestStatuses::<Runtime>::iter_keys().count(),
            status_count,
            "EthBridge::RequestStatuses key count changed during migration"
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::storage::unhashed;

    #[test]
    fn remote_mode_pins_online_and_snapshot_fallback_to_requested_block() {
        let requested = sp_core::H256::repeat_byte(7);
        for snapshot in [None, Some(SnapshotConfig::new("unused-test-snapshot"))] {
            let mode = remote_mode(
                "ws://127.0.0.1:1".into(),
                snapshot,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                false,
                Some(requested),
            );
            let online = match mode {
                Mode::Online(online) | Mode::OfflineOrElseOnline(_, online) => online,
                _ => panic!("rehearsal mode must have an online configuration"),
            };
            assert_eq!(online.at, Some(requested));
        }
    }

    #[test]
    #[should_panic(expected = "remote snapshot block does not match REMOTE_BLOCK_HASH")]
    fn pinned_rehearsal_rejects_snapshot_from_another_block() {
        assert_requested_remote_block(
            Some(sp_core::H256::repeat_byte(7)),
            sp_core::H256::repeat_byte(8),
        );
    }

    #[test]
    #[cfg(not(feature = "runtime-benchmarks"))]
    fn migration_replay_advances_chain_block_number() {
        use frame_support::traits::Hooks;

        sp_io::TestExternalities::new_empty().execute_with(|| {
            System::set_block_number(100);
            StorageVersion::new(3).put::<eth_bridge::Pallet<Runtime>>();
            <pallet_migrations::Pallet<Runtime> as Hooks<BlockNumber>>::on_runtime_upgrade();

            let steps = replay_multi_block_migrations();

            assert!(
                steps > 0,
                "migration should have advanced at least one block"
            );
            assert_eq!(System::block_number(), 100 + steps);
            assert_eq!(
                eth_bridge::Pallet::<Runtime>::on_chain_storage_version(),
                StorageVersion::new(4)
            );
        });
    }

    fn legacy_eth_bridge_storage_version_key() -> Vec<u8> {
        storage_prefix(b"EthBridge", b"StorageVersion").to_vec()
    }

    #[test]
    fn eth_bridge_focused_filter_loads_frame_storage_version_key() {
        let frame_key = eth_bridge_storage_version_key();

        assert_eq!(eth_bridge_migration_hashed_keys(), vec![frame_key.clone()]);
        assert_eq!(
            frame_key,
            StorageVersion::storage_key::<eth_bridge::Pallet<Runtime>>().to_vec()
        );
        assert_ne!(frame_key, legacy_eth_bridge_storage_version_key());
    }

    #[test]
    fn eth_bridge_focused_filter_excludes_legacy_storage_version_key() {
        let keys = eth_bridge_migration_hashed_keys();

        assert_eq!(keys.len(), 1);
        assert!(!keys.contains(&legacy_eth_bridge_storage_version_key()));
    }

    #[test]
    fn eth_bridge_focused_filter_only_loads_migrated_maps_as_prefixes() {
        assert_eq!(
            eth_bridge_migration_hashed_prefixes(),
            vec![
                storage_prefix(b"EthBridge", b"LoadToIncomingRequestHash").to_vec(),
                storage_prefix(b"EthBridge", b"AccountRequests").to_vec(),
                storage_prefix(b"EthBridge", b"Peers").to_vec(),
                storage_prefix(b"EthBridge", b"RequestApprovals").to_vec(),
                storage_prefix(b"EthBridge", b"RequestApprovers").to_vec(),
                storage_prefix(b"EthBridge", b"RequestsQueue").to_vec(),
                storage_prefix(b"EthBridge", b"RequestStatuses").to_vec(),
                storage_prefix(b"EthBridge", b"Requests").to_vec(),
            ]
        );
    }

    #[test]
    fn legacy_storage_version_key_does_not_seed_frame_on_chain_version() {
        sp_io::TestExternalities::new_empty().execute_with(|| {
            unhashed::put(
                &legacy_eth_bridge_storage_version_key(),
                &StorageVersion::new(2),
            );

            assert_eq!(
                eth_bridge::Pallet::<Runtime>::on_chain_storage_version(),
                StorageVersion::new(0)
            );
        });
    }

    #[test]
    fn frame_storage_version_key_seeds_frame_on_chain_version() {
        sp_io::TestExternalities::new_empty().execute_with(|| {
            unhashed::put(&eth_bridge_storage_version_key(), &StorageVersion::new(2));

            assert_eq!(
                eth_bridge::Pallet::<Runtime>::on_chain_storage_version(),
                StorageVersion::new(2)
            );
        });
    }

    #[test]
    fn live_4_8_8_guard_accepts_v3() {
        sp_io::TestExternalities::new_empty().execute_with(|| {
            StorageVersion::new(3).put::<eth_bridge::Pallet<Runtime>>();

            assert_eth_bridge_rehearsal_starts_from_live_4_8_8();
        });
    }

    #[test]
    #[should_panic(
        expected = "EthBridge live storage version should match the 4.8.8 release state"
    )]
    fn live_4_8_8_guard_rejects_missing_storage_version() {
        sp_io::TestExternalities::new_empty()
            .execute_with(assert_eth_bridge_rehearsal_starts_from_live_4_8_8);
    }

    #[test]
    #[should_panic(
        expected = "EthBridge live storage version should match the 4.8.8 release state"
    )]
    fn live_4_8_8_guard_rejects_v2() {
        sp_io::TestExternalities::new_empty().execute_with(|| {
            StorageVersion::new(2).put::<eth_bridge::Pallet<Runtime>>();

            assert_eth_bridge_rehearsal_starts_from_live_4_8_8();
        });
    }

    #[test]
    #[should_panic(
        expected = "EthBridge live storage version should match the 4.8.8 release state"
    )]
    fn live_4_8_8_guard_rejects_future_v4() {
        sp_io::TestExternalities::new_empty().execute_with(|| {
            StorageVersion::new(4).put::<eth_bridge::Pallet<Runtime>>();

            assert_eth_bridge_rehearsal_starts_from_live_4_8_8();
        });
    }
}
