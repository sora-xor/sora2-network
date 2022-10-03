use crate::cli::prelude::*;
use assets_rpc::AssetsAPIClient;
use bridge_types::H160;
use common::{AssetId32, PredefinedAssetId};

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Recipient address
    #[clap(short, long)]
    recipient: H160,
    /// Amount of tokens to transfer
    #[clap(short, long)]
    amount: u128,
    /// Asset id to transfer
    #[clap(long)]
    asset_id: AssetId32<PredefinedAssetId>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;
        let network_id = eth.get_chainid().await?;
        let (_, native_asset_id) = sub
            .api()
            .storage()
            .fetch(&runtime::storage().eth_app().addresses(&network_id), None)
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
                .sign_and_submit_then_watch_default(
                    &runtime::tx()
                        .eth_app()
                        .burn(network_id, self.recipient, self.amount),
                    &sub,
                )
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?
        } else {
            sub.api()
                .tx()
                .sign_and_submit_then_watch_default(
                    &runtime::tx().erc20_app().burn(
                        network_id,
                        self.asset_id,
                        self.recipient,
                        self.amount,
                    ),
                    &sub,
                )
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
