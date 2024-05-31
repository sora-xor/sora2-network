// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use clap::{Args, Parser, Subcommand};
use codec::Decode;
use frame_remote_externalities::{
    Builder, Mode, OfflineConfig, OnlineConfig, RemoteExternalities, SnapshotConfig,
};
use framenode_runtime::AccountId;
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use sp_core::H256;
use sp_runtime::{traits::Block as BlockT, DeserializeOwned};

use anyhow::Result as AnyResult;
use frame_support::traits::OnRuntimeUpgrade;
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
    /// Run migrations
    #[clap(long)]
    run_migrations: bool,
    /// Command type
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Args)]
#[group(multiple = false, required = true)]
pub struct AccountArg {
    #[clap(long)]
    account: Option<AccountId>,
    #[clap(long)]
    root: bool,
    #[clap(long)]
    unsigned: bool,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    Call {
        #[clap(long)]
        call: String,
        #[clap(flatten)]
        account: AccountArg,
    },
    Xt {
        xt: String,
    },
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
        if cli.run_migrations {
            framenode_runtime::migrations::Migrations::on_runtime_upgrade();
        }
        match cli.command {
            Command::Call { call, account } => {
                let origin = if account.unsigned {
                    framenode_runtime::RuntimeOrigin::none()
                } else if account.root {
                    framenode_runtime::RuntimeOrigin::root()
                } else {
                    framenode_runtime::RuntimeOrigin::signed(account.account.unwrap())
                };
                let call_encoded = hex::decode(call.strip_prefix("0x").unwrap_or(&call)).unwrap();
                let call = framenode_runtime::RuntimeCall::decode(&mut &call_encoded[..]).unwrap();
                call.dispatch(origin).unwrap();
            }
            Command::Xt { xt } => {
                let xt_encoded = hex::decode(xt.strip_prefix("0x").unwrap_or(&xt)).unwrap();
                let xt =
                    framenode_runtime::UncheckedExtrinsic::decode(&mut &xt_encoded[..]).unwrap();
                if let Some((account, _signature, _extra)) = xt.signature {
                    xt.function
                        .dispatch(framenode_runtime::RuntimeOrigin::signed(account))
                        .unwrap();
                }
            }
        }
        Ok(())
    });
    Ok(())
}
