use crate::cli::prelude::*;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(short, long)]
    network: u32,
}

impl Command {
    pub(super) async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        let sub = args.get_signed_substrate().await?;

        sub.api()
            .tx()
            .sudo()
            .sudo(
                false,
                sub_types::framenode_runtime::Call::EthBridge(
                    sub_types::eth_bridge::pallet::Call::prepare_for_migration {
                        network_id: self.network,
                    },
                ),
            )?
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        Ok(())
    }
}
