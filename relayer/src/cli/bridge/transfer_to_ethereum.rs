use crate::cli::prelude::*;
use bridge_types::types::ChannelId;
use bridge_types::H160;
use common::{AssetId32, PredefinedAssetId};
use ethers::prelude::Middleware;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    eth: EthereumUrl,
    #[clap(flatten)]
    sub: SubstrateUrl,
    #[clap(flatten)]
    key: SubstrateKey,
    #[clap(short, long)]
    recipient: H160,
    #[clap(short, long)]
    amount: u128,
    #[clap(long)]
    asset_id: AssetId32<PredefinedAssetId>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.eth.get()).await?;
        let sub = SubUnsignedClient::new(self.sub.get())
            .await?
            .try_sign_with(&self.key.get_key_string()?)
            .await?;
        let network_id = eth.get_chainid().await?.as_u32();
        let (_, native_asset_id) = sub
            .api()
            .storage()
            .eth_app()
            .addresses(network_id, None)
            .await?
            .expect("network not found");
        let balance = sub
            .get_total_balance(self.asset_id, sub.account_id())
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
                )
                .sign_and_submit_then_watch(&sub)
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
                )
                .sign_and_submit_then_watch(&sub)
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?
        };
        info!("Result: {:?}", result);
        Ok(())
    }
}
