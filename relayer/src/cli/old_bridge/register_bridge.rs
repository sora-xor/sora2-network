use crate::cli::prelude::*;
use crate::substrate::AccountId;
use bridge_types::H160;
use substrate_gen::BridgeSignatureVersion;

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
            .sign_and_submit_then_watch_default(
                &runtime::tx().eth_bridge().register_bridge(
                    self.contract,
                    self.peers.clone(),
                    BridgeSignatureVersion::V2,
                ),
                &sub,
            )
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        Ok(())
    }
}
