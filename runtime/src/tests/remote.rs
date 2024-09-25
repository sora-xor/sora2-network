use crate::*;
use frame_remote_externalities::{
    Builder, Mode, OfflineConfig, OnlineConfig, SnapshotConfig, Transport,
};
use frame_try_runtime::runtime_decl_for_TryRuntime::TryRuntime;
use std::env::var;

#[tokio::test]
async fn run_migrations() {
    sp_tracing::try_init_simple();
    let transport: Transport = var("WS")
        .unwrap_or("wss://ws.framenode-2.s1.stg1.sora2.soramitsu.co.jp:443".to_string())
        .into();
    let maybe_state_snapshot: Option<SnapshotConfig> = var("SNAP").map(|s| s.into()).ok();
    let mut ext = Builder::<Block>::default()
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
        .build()
        .await
        .unwrap();
    ext.execute_with(|| Runtime::on_runtime_upgrade(frame_try_runtime::UpgradeCheckSelect::All));
}
