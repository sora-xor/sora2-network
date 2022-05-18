use crate::cli::prelude::*;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    sub: SubstrateUrl,
    #[clap(flatten)]
    key: SubstrateKey,
    #[clap(short, long)]
    network: u32,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = SubUnsignedClient::new(self.sub.get())
            .await?
            .try_sign_with(&self.key.get_key_string()?)
            .await?;

        sub.api()
            .tx()
            .sudo()
            .sudo(sub_types::framenode_runtime::Call::EthBridge(
                sub_types::eth_bridge::pallet::Call::prepare_for_migration {
                    network_id: self.network,
                },
            ))
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        Ok(())
    }
}
