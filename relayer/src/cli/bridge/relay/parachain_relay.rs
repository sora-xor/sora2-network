use crate::cli::prelude::*;
use crate::relay::parachain::RelayBuilder;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    para: SubstrateClient,
    /// Send all Beefy commitments
    #[clap(short, long)]
    send_unneeded_commitments: bool,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sender = self.sub.get_signed_substrate().await?;
        let receiver = self.para.get_signed_substrate().await?;
        RelayBuilder::new()
            .with_sender_client(sender)
            .with_receiver_client(receiver)
            .build()
            .await
            .context("build substrate relay")?
            .run(!self.send_unneeded_commitments)
            .await
            .context("run substrate relay")?;
        Ok(())
    }
}
