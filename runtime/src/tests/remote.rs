use crate::*;
use frame_remote_externalities::{
    Builder, Mode, OfflineConfig, OnlineConfig, SnapshotConfig, Transport,
};
use frame_support::migrations::MultiStepMigrator;
use frame_support::traits::GetStorageVersion;
use std::env::var;

const DEFAULT_REMOTE_RPC_URL: &str = "https://ws.mof.sora.org";

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

pub(crate) async fn remote_try_runtime_upgrade_rehearsal() {
    sp_tracing::try_init_simple();
    let require_remote = env_flag("REQUIRE_REMOTE", false);

    let transport: Transport = var("REMOTE_RPC_URL")
        .or_else(|_| var("WS"))
        .unwrap_or(DEFAULT_REMOTE_RPC_URL.to_string())
        .into();
    let maybe_state_snapshot: Option<SnapshotConfig> = var("SNAP").map(|s| s.into()).ok();
    let pallets = env_csv("REMOTE_PALLETS");
    let child_trie = env_flag("REMOTE_CHILD_TRIE", true);
    let builder = Builder::<Block>::default()
        .mode(if let Some(state_snapshot) = maybe_state_snapshot {
            Mode::OfflineOrElseOnline(
                OfflineConfig {
                    state_snapshot: state_snapshot.clone(),
                },
                OnlineConfig {
                    transport,
                    state_snapshot: Some(state_snapshot),
                    pallets,
                    child_trie,
                    ..Default::default()
                },
            )
        } else {
            Mode::Online(OnlineConfig {
                transport,
                pallets,
                child_trie,
                ..Default::default()
            })
        })
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
        macro_rules! assert_storage_version {
            ($label:literal, $pallet:ty) => {{
                let on_chain = <$pallet>::on_chain_storage_version();
                let in_code = <$pallet>::in_code_storage_version();
                assert_eq!(
                    on_chain, in_code,
                    "{}: on-chain {:?} != in-code {:?}",
                    $label, on_chain, in_code
                );
            }};
        }

        assert_storage_version!("XorFee", xor_fee::Pallet<Runtime>);
        assert_storage_version!("Staking", pallet_staking::Pallet<Runtime>);
        assert_storage_version!("Offences", pallet_offences::Pallet<Runtime>);
        assert_storage_version!("Session", pallet_session::Pallet<Runtime>);
        assert_storage_version!("Grandpa", pallet_grandpa::Pallet<Runtime>);
        assert_storage_version!("ImOnline", pallet_im_online::Pallet<Runtime>);
        assert_storage_version!("PoolXYK", pool_xyk::Pallet<Runtime>);
        assert_storage_version!("PswapDistribution", pswap_distribution::Pallet<Runtime>);
        assert_storage_version!("VestedRewards", vested_rewards::Pallet<Runtime>);
        assert_storage_version!("Identity", pallet_identity::Pallet<Runtime>);
        assert_storage_version!("Farming", farming::Pallet<Runtime>);
        assert_storage_version!("Band", band::Pallet<Runtime>);
        assert_storage_version!("OracleProxy", oracle_proxy::Pallet<Runtime>);
        assert_storage_version!(
            "BridgeInboundChannel",
            bridge_channel::inbound::Pallet<Runtime>
        );
        assert_storage_version!(
            "SubstrateBridgeInboundChannel",
            substrate_bridge_channel::inbound::Pallet<Runtime>
        );
        assert_storage_version!(
            "SubstrateBridgeOutboundChannel",
            substrate_bridge_channel::outbound::Pallet<Runtime>
        );
    });
}
