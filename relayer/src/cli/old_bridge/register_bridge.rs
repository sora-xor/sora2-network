use crate::cli::prelude::*;
use crate::substrate::AccountId;
use bridge_types::H160;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    /// Bridge network id
    #[clap(short, long)]
    network: u32,
    /// Bridge peers
    #[clap(short, long)]
    peers: Vec<AccountId>,
    /// Bridge contract address
    #[clap(short, long)]
    contract: H160,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = self.sub.get_signed_substrate().await?;

        sub.api()
            .tx()
            .eth_bridge()
            .register_bridge(false, self.contract, self.peers.clone())?
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        Ok(())
    }
}
