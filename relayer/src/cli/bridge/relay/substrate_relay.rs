use crate::cli::prelude::*;
use crate::relay::substrate::RelayBuilder;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    #[clap(short, long)]
    send_unneeded_commitments: bool,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_signed_ethereum().await?;
        let sub = self.sub.get_unsigned_substrate().await?;
        let network_id = eth.inner().get_chainid().await.context("fetch chain id")?;
        let eth_app = sub
            .api()
            .storage()
            .eth_app()
            .addresses(false, &network_id, None)
            .await?
            .ok_or(anyhow!("Network is not registered"))?
            .0;
        let eth_app = ethereum_gen::ETHApp::new(eth_app, eth.inner());
        let inbound_channel_address = eth_app
            .inbound()
            .call()
            .await
            .context("fetch outbound channel address")?;
        let channel = ethereum_gen::InboundChannel::new(inbound_channel_address, eth.inner());
        let beefy = channel
            .beefy_light_client()
            .call()
            .await
            .context("fetch beefy light client address")?;
        RelayBuilder::new()
            .with_substrate_client(sub)
            .with_ethereum_client(eth)
            .with_inbound_channel_contract(inbound_channel_address)
            .with_beefy_contract(beefy)
            .build()
            .await
            .context("build substrate relay")?
            .run(!self.send_unneeded_commitments)
            .await
            .context("run substrate relay")?;
        Ok(())
    }
}
