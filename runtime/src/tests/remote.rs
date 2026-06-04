use crate::*;
use frame_remote_externalities::{Builder, Mode, OfflineConfig, OnlineConfig, SnapshotConfig};
use frame_support::migrations::MultiStepMigrator;
use frame_support::storage::storage_prefix;
use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
use sp_runtime::traits::Block as BlockT;
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

fn remote_mode(
    transport_uri: String,
    maybe_state_snapshot: Option<SnapshotConfig>,
    pallets: Vec<String>,
    hashed_prefixes: Vec<Vec<u8>>,
    hashed_keys: Vec<Vec<u8>>,
    child_trie: bool,
) -> Mode<<Block as BlockT>::Hash> {
    if let Some(state_snapshot) = maybe_state_snapshot {
        Mode::OfflineOrElseOnline(
            OfflineConfig {
                state_snapshot: state_snapshot.clone(),
            },
            OnlineConfig {
                transport_uris: vec![transport_uri],
                state_snapshot: Some(state_snapshot),
                pallets,
                hashed_prefixes,
                hashed_keys,
                child_trie,
                ..Default::default()
            },
        )
    } else {
        Mode::Online(OnlineConfig {
            transport_uris: vec![transport_uri],
            pallets,
            hashed_prefixes,
            hashed_keys,
            child_trie,
            ..Default::default()
        })
    }
}

fn eth_bridge_migration_hashed_prefixes() -> Vec<Vec<u8>> {
    vec![
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

fn assert_eth_bridge_rehearsal_starts_from_live_4_8_7() {
    let on_chain = eth_bridge::Pallet::<Runtime>::on_chain_storage_version();
    assert!(
        [StorageVersion::new(2), StorageVersion::new(3)].contains(&on_chain),
        "EthBridge live storage version should match a supported 4.8.7 release state (v2 or v3), found {on_chain:?}"
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
        || var("REMOTE_HASHED_KEYS").is_ok();

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
    let builder = Builder::<Block>::default()
        .mode(remote_mode(
            transport_uri,
            maybe_state_snapshot,
            pallets,
            hashed_prefixes,
            hashed_keys,
            child_trie,
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
    ext.execute_with(|| {
        Executive::execute_on_runtime_upgrade();

        let mut steps = 0u32;
        while <Runtime as frame_system::Config>::MultiBlockMigrator::ongoing() {
            <Runtime as frame_system::Config>::MultiBlockMigrator::step();
            steps = steps.saturating_add(1);
            assert!(
                steps <= 4096,
                "multi-block migrations did not finish after {steps} steps"
            );
        }
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
    });
}

pub(crate) async fn remote_eth_bridge_migration_rehearsal() {
    sp_tracing::try_init_simple();
    let require_remote = env_flag("REQUIRE_REMOTE", false);
    let remote_requested =
        require_remote || var("REMOTE_RPC_URL").is_ok() || var("WS").is_ok() || var("SNAP").is_ok();

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
    let builder = Builder::<Block>::default()
        .mode(remote_mode(
            transport_uri,
            maybe_state_snapshot,
            Vec::new(),
            hashed_prefixes,
            hashed_keys,
            false,
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

    ext.execute_with(|| {
        let request_count = eth_bridge::Requests::<Runtime>::iter_keys().count();
        let status_count = eth_bridge::RequestStatuses::<Runtime>::iter_keys().count();
        assert_ne!(
            request_count, 0,
            "EthBridge::Requests live-state prefix loaded no keys"
        );
        assert_ne!(
            status_count, 0,
            "EthBridge::RequestStatuses live-state prefix loaded no keys"
        );
        assert_eth_bridge_rehearsal_starts_from_live_4_8_7();

        let state = crate::migrations::EthBridgeStorageVersionV3::pre_upgrade()
            .expect("EthBridge pre-upgrade should encode live storage version");
        crate::migrations::EthBridgeStorageVersionV3::on_runtime_upgrade();
        crate::migrations::EthBridgeStorageVersionV3::post_upgrade(state)
            .expect("EthBridge post-upgrade should validate the v3 transition");

        assert_eq!(
            eth_bridge::Pallet::<Runtime>::on_chain_storage_version(),
            StorageVersion::new(3)
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
    fn live_4_8_7_guard_accepts_supported_versions() {
        for version in [2, 3] {
            sp_io::TestExternalities::new_empty().execute_with(|| {
                StorageVersion::new(version).put::<eth_bridge::Pallet<Runtime>>();

                assert_eth_bridge_rehearsal_starts_from_live_4_8_7();
            });
        }
    }

    #[test]
    #[should_panic(
        expected = "EthBridge live storage version should match a supported 4.8.7 release state"
    )]
    fn live_4_8_7_guard_rejects_missing_storage_version() {
        sp_io::TestExternalities::new_empty()
            .execute_with(assert_eth_bridge_rehearsal_starts_from_live_4_8_7);
    }

    #[test]
    #[should_panic(
        expected = "EthBridge live storage version should match a supported 4.8.7 release state"
    )]
    fn live_4_8_7_guard_rejects_legacy_v1() {
        sp_io::TestExternalities::new_empty().execute_with(|| {
            StorageVersion::new(1).put::<eth_bridge::Pallet<Runtime>>();

            assert_eth_bridge_rehearsal_starts_from_live_4_8_7();
        });
    }

    #[test]
    #[should_panic(
        expected = "EthBridge live storage version should match a supported 4.8.7 release state"
    )]
    fn live_4_8_7_guard_rejects_future_v4() {
        sp_io::TestExternalities::new_empty().execute_with(|| {
            StorageVersion::new(4).put::<eth_bridge::Pallet<Runtime>>();

            assert_eth_bridge_rehearsal_starts_from_live_4_8_7();
        });
    }
}
