#[macro_use]
extern crate log;

use clap::Parser;
use common::prelude::QuoteAmount;
use common::{balance, DEXId, LiquiditySourceFilter};
use frame_remote_externalities::{Builder, Mode, OfflineConfig, OnlineConfig, RemoteExternalities};
use frame_support::traits::OnRuntimeUpgrade;
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use sp_runtime::{traits::Block as BlockT, DeserializeOwned};

use anyhow::Result as AnyResult;
use framenode_runtime::Runtime;
use std::sync::Arc;

async fn create_ext<B>(client: Arc<WsClient>) -> AnyResult<RemoteExternalities<B>>
where
    B: DeserializeOwned + BlockT,
    <B as BlockT>::Header: DeserializeOwned,
{
    let res = Builder::<B>::new()
        .mode(Mode::OfflineOrElseOnline(
            OfflineConfig {
                state_snapshot: "state_snapshot".to_string().into(),
            },
            OnlineConfig {
                transport: client.into(),
                state_snapshot: Some("state_snapshot".to_string().into()),
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
    uri: String,
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
    let mut ext = create_ext::<framenode_runtime::Block>(client.clone()).await?;
    let _res: AnyResult<()> = ext.execute_with(|| {
        framenode_runtime::migrations::Migrations::on_runtime_upgrade();
        let input = QuoteAmount::with_desired_input(balance!(1000.0));
        let res = liquidity_proxy::Pallet::<Runtime>::inner_quote(
            DEXId::Polkaswap.into(),
            &common::DAI.into(),
            &common::XSTUSD.into(),
            input.clone(),
            LiquiditySourceFilter::empty(DEXId::Polkaswap.into()),
            true,
            true,
        )
        .unwrap()
        .0
        .outcome;
        info!("quote(0, DAI, XSTUSD, {input:?}) = {res:?}");
        Ok(())
    });
    Ok(())
}
