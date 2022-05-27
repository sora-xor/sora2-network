use super::*;
use crate::prelude::*;
use assets_rpc::AssetsAPIClient;
use bridge_types::types::ChannelId;
use bridge_types::H160;
use clap::*;
use common::{AssetId32, PredefinedAssetId};
use ethers::prelude::Middleware;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(short, long)]
    recipient: H160,
    #[clap(short, long)]
    amount: u128,
    #[clap(long)]
    asset_id: AssetId32<PredefinedAssetId>,
}

impl Command {
    pub(super) async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        let eth = args.get_unsigned_ethereum().await?;
        let sub = args.get_signed_substrate().await?;
        let network_id = eth.get_chainid().await?.as_u32();
        let (_, native_asset_id) = sub
            .api()
            .storage()
            .eth_app()
            .addresses(&network_id, None)
            .await?
            .expect("network not found");
        let balance = sub
            .assets()
            .total_balance(sub.account_id(), self.asset_id, None)
            .await?;
        info!("Current balance: {:?}", balance);
        let result = if self.asset_id == native_asset_id {
            sub.api()
                .tx()
                .eth_app()
                .burn(
                    network_id,
                    ChannelId::Incentivized,
                    self.recipient,
                    self.amount,
                )?
                .sign_and_submit_then_watch_default(&sub)
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?
        } else {
            sub.api()
                .tx()
                .erc20_app()
                .burn(
                    network_id,
                    ChannelId::Incentivized,
                    self.asset_id,
                    self.recipient,
                    self.amount,
                )?
                .sign_and_submit_then_watch_default(&sub)
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?
        };
        info!("Result: {:?}", result.iter().collect::<Vec<_>>());
        Ok(())
    }
}
