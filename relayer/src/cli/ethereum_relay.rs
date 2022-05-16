use super::*;
use crate::ethereum::proof_loader::ProofLoader;
use crate::relay::ethereum::Relay;
use crate::relay::ethereum_messages::SubstrateMessagesRelay;
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
        let proof_loader = ProofLoader::new(eth.clone(), self.base_path.clone());
        let relay = Relay::new(sub.clone(), eth.clone(), proof_loader.clone()).await?;
        let messages_relay = SubstrateMessagesRelay::new(sub, eth, proof_loader).await?;
        tokio::try_join!(relay.run(), messages_relay.run())?;
        Ok(())
    }
}
