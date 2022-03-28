use super::*;
use crate::prelude::*;
use crate::relay::substrate::RelayBuilder;
use bridge_types::H160;
use clap::*;

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    #[clap(flatten)]
    ethereum: EthereumUrl,
    #[clap(flatten)]
    substrate: SubstrateUrl,
    #[clap(flatten)]
    key: EthereumKey,
    #[clap(long)]
    basic_channel: H160,
    #[clap(long)]
    incentivized_channel: H160,
    #[clap(long)]
    beefy: H160,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.ethereum.ethereum_url.clone())
            .await?
            .sign_with_string(&self.key.get_key_string()?)
            .await
            .context("sign ethereum client")?;
        let sub = SubUnsignedClient::new(self.substrate.substrate_url.clone()).await?;
        RelayBuilder::new()
            .with_substrate_client(sub)
            .with_ethereum_client(eth)
            .with_basic_contract(self.basic_channel)
            .with_incentivized_contract(self.incentivized_channel)
            .with_beefy_contract(self.beefy)
            .build()
            .await?
            .run(true)
            .await?;
        Ok(())
    }
}
