// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use clap::Parser;
use common::prelude::SwapVariant;
use common::DEXId;
use frame_remote_externalities::{Builder, Mode, OfflineConfig, OnlineConfig, RemoteExternalities};
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use sp_runtime::{traits::Block as BlockT, DeserializeOwned};

use anyhow::Result as AnyResult;
use framenode_runtime::assets::AssetIdOf;
use framenode_runtime::order_book::WeightInfo;
use framenode_runtime::{Runtime, Weight};
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
        fn aboba(input: AssetIdOf<Runtime>, output: AssetIdOf<Runtime>) -> Weight {
            // let dex_info =
            //     dex_manager::Pallet::<Runtime>::get_dex_info(&DEXId::Polkaswap.into()).unwrap();
            // dbg!(
            //     liquidity_proxy::ExchangePath::<Runtime>::new_trivial(&dex_info, input, output,)
            //         .unwrap()
            //         .into_iter()
            //         .map(|path| path.0)
            //         .collect::<Vec<_>>()
            // );
            liquidity_proxy::Pallet::<Runtime>::swap_weight(
                &DEXId::Polkaswap.into(),
                &input,
                &output,
                SwapVariant::WithDesiredOutput,
            )
        }
        // Base -> Basic
        let path_2_weight = aboba(common::XOR, common::PSWAP);

        // Basic -> Basic
        let path_3_weight = aboba(common::VAL, common::PSWAP);

        // Synthetic -> Basic
        let path_4_weight = aboba(common::XSTUSD, common::PSWAP);

        dbg!(path_2_weight);
        dbg!(path_3_weight);
        dbg!(path_4_weight);
        let execute_order_weight =
            <Runtime as framenode_runtime::order_book::Config>::WeightInfo::execute_market_order();
        dbg!(execute_order_weight);
        Ok(())
    });
    Ok(())
}
