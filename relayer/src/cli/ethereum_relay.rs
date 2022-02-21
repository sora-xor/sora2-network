use super::*;
use crate::relay::ethereum::Relay;
use clap::*;
use std::path::PathBuf;

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    #[clap(flatten)]
    ethereum: EthereumUrl,
    #[clap(flatten)]
    substrate: SubstrateUrl,
    #[clap(long)]
    base_path: PathBuf,
    #[clap(flatten)]
    key: SubstrateKey,
}

impl Command {
    pub async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.ethereum.ethereum_url.clone()).await?;
        let sub = SubUnsignedClient::new(self.substrate.substrate_url.clone())
            .await?
            .try_sign_with(&self.key.get_key_string()?)
            .await?;
        Relay::new(self.base_path.clone(), sub, eth)
            .await?
            .run()
            .await?;
        Ok(())
    }
}
