use crate::*;
use frame_remote_externalities::{
    Builder, Mode, OfflineConfig, OnlineConfig, SnapshotConfig, Transport,
};
use frame_try_runtime::runtime_decl_for_try_runtime::TryRuntimeV1;
use std::env::var;

#[tokio::test]
async fn run_migrations() {
    sp_tracing::try_init_simple();
    let require_remote = var("REQUIRE_REMOTE")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);

    let transport: Transport = var("WS")
        .unwrap_or("https://ws.framenode-0.a1.sora2.soramitsu.co.jp:443".to_string())
        .into();
    let maybe_state_snapshot: Option<SnapshotConfig> = var("SNAP").map(|s| s.into()).ok();
    let builder = Builder::<Block>::default()
        .mode(if let Some(state_snapshot) = maybe_state_snapshot {
            Mode::OfflineOrElseOnline(
                OfflineConfig {
                    state_snapshot: state_snapshot.clone(),
                },
                OnlineConfig {
                    transport,
                    state_snapshot: Some(state_snapshot),
                    ..Default::default()
                },
            )
        } else {
            Mode::Online(OnlineConfig {
                transport,
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
    ext.execute_with(|| Runtime::on_runtime_upgrade(frame_try_runtime::UpgradeCheckSelect::All));
}
