use crate::cli::prelude::*;
use crate::ethereum::proof_loader::ProofLoader;
use crate::relay::ethereum::Relay;
use crate::relay::ethereum_messages::SubstrateMessagesRelay;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Ethereum DAG cache dir
    #[clap(long)]
    base_path: PathBuf,
    /// Not send messages from Ethereum to Substrate
    #[clap(long)]
    disable_message_relay: bool,
}

impl Command {
    pub async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;
        let chain_id = eth.get_chainid().await?;
        loop {
            let has_light_client = sub
                .api()
                .storage()
                .fetch(
                    &runtime::storage()
                        .ethereum_light_client()
                        .network_config(&chain_id),
                    None,
                )
                .await?
                .is_some();
            let has_channel = sub
                .api()
                .storage()
                .fetch(
                    &runtime::storage()
                        .bridge_inbound_channel()
                        .channel_addresses(&chain_id),
                    None,
                )
                .await?
                .is_some();
            if has_channel && has_light_client {
                break;
            }
            debug!("Waiting for bridge to be available");
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        let proof_loader = ProofLoader::new(eth.clone(), self.base_path.clone());
        let relay = Relay::new(sub.clone(), eth.clone(), proof_loader.clone()).await?;
        if self.disable_message_relay {
            relay.run().await?;
        } else {
            let messages_relay = SubstrateMessagesRelay::new(sub, eth, proof_loader).await?;
            tokio::try_join!(relay.run(), messages_relay.run())?;
        }
        Ok(())
    }
}
