// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use clap::Parser;
use codec::Decode;
use frame_remote_externalities::{
    Builder, Mode, OfflineConfig, OnlineConfig, RemoteExternalities, SnapshotConfig,
};
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use sp_core::H256;
use sp_runtime::{traits::Block as BlockT, DeserializeOwned};

use anyhow::Result as AnyResult;
use sp_runtime::traits::Dispatchable;
use std::path::PathBuf;
use std::sync::Arc;

async fn create_ext<B>(
    client: Arc<WsClient>,
    at: Option<H256>,
    snapshot_path: Option<PathBuf>,
) -> AnyResult<RemoteExternalities<B>>
where
    B: DeserializeOwned + BlockT<Hash = H256>,
    <B as BlockT>::Header: DeserializeOwned,
{
    let snapshot_path = snapshot_path.unwrap_or_else(|| match at {
        Some(at) => format!("state_snapshot_{}", at).into(),
        None => "state_snapshot".to_string().into(),
    });
    let state_snapshot = SnapshotConfig::new(&snapshot_path);
    let res = Builder::<B>::new()
        .mode(Mode::OfflineOrElseOnline(
            OfflineConfig {
                state_snapshot: state_snapshot.clone(),
            },
            OnlineConfig {
                transport: client.into(),
                state_snapshot: Some(state_snapshot),
                at,
                ..Default::default()
            },
        ))
        .build()
        .await
        .unwrap();
    Ok(res)
}

#[derive(Debug, Clone, Parser)]
struct Cli {
    /// Sora node endpoint.
    #[clap(long)]
    uri: String,
    /// Sora block hash.
    #[clap(long)]
    at: Option<H256>,
    /// Sora snapshot path.
    #[clap(long)]
    snapshot_path: Option<PathBuf>,
    /// Encoded extrinsic
    #[clap(long)]
    xt: String,
}

#[tokio::main]
async fn main() -> AnyResult<()> {
    env_logger::init();
    let cli = Cli::parse();
    let client = WsClientBuilder::default()
        .max_request_body_size(u32::MAX)
        .build(cli.uri)
        .await?;
    let client = Arc::new(client);
    let mut ext =
        create_ext::<framenode_runtime::Block>(client.clone(), cli.at, cli.snapshot_path).await?;
    let _res: AnyResult<()> = ext.execute_with(|| {
        let xt_encoded = hex::decode(&cli.xt).unwrap();
        let xt = framenode_runtime::UncheckedExtrinsic::decode(&mut &xt_encoded[..]).unwrap();
        if let Some((account, _signature, _extra)) = xt.signature {
            xt.function
                .dispatch(framenode_runtime::RuntimeOrigin::signed(account))
                .unwrap();
        }
        Ok(())
    });
    Ok(())
}
