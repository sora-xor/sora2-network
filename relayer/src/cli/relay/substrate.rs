use crate::cli::prelude::*;
use crate::relay::substrate::RelayBuilder;
use bridge_types::H160;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    ethereum: EthereumUrl,
    #[clap(flatten)]
    substrate: SubstrateUrl,
    #[clap(flatten)]
    key: EthereumKey,
    #[clap(long)]
    basic_channel_inbound: H160,
    #[clap(long)]
    incentivized_channel_inbound: H160,
    #[clap(long)]
    beefy: H160,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.ethereum.get())
            .await?
            .sign_with_string(&self.key.get_key_string()?)
            .await
            .context("sign ethereum client")?;
        let sub = SubUnsignedClient::new(self.substrate.get()).await?;
        RelayBuilder::new()
            .with_substrate_client(sub)
            .with_ethereum_client(eth)
            .with_basic_contract(self.basic_channel_inbound)
            .with_incentivized_contract(self.incentivized_channel_inbound)
            .with_beefy_contract(self.beefy)
            .build()
            .await?
            .run(true)
            .await?;
        Ok(())
    }
}
