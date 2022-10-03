use crate::cli::prelude::*;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    /// Bridge network id
    #[clap(short, long)]
    network: u32,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = self.sub.get_signed_substrate().await?;

        sub.api()
            .tx()
            .sign_and_submit_then_watch_default(
                &runtime::tx()
                    .sudo()
                    .sudo(sub_types::framenode_runtime::Call::EthBridge(
                        sub_types::eth_bridge::pallet::Call::prepare_for_migration {
                            network_id: self.network,
                        },
                    )),
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
