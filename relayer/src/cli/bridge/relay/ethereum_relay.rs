use crate::cli::prelude::*;
use crate::ethereum::proof_loader::ProofLoader;
use crate::relay::ethereum::Relay;
use crate::relay::ethereum_messages::SubstrateMessagesRelay;
use std::path::PathBuf;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    #[clap(long)]
    base_path: PathBuf,
    #[clap(long)]
    disable_incentivized: bool,
    #[clap(long)]
    disable_basic: bool,
}

impl Command {
    pub async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;
        let proof_loader = ProofLoader::new(eth.clone(), self.base_path.clone());
        let relay = Relay::new(sub.clone(), eth.clone(), proof_loader.clone()).await?;
        let messages_relay = SubstrateMessagesRelay::new(
            sub,
            eth,
            proof_loader,
            self.disable_basic,
            self.disable_incentivized,
        )
        .await?;
        tokio::try_join!(relay.run(), messages_relay.run())?;
        Ok(())
    }
}
