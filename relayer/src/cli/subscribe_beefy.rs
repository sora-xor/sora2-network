use super::*;
use crate::prelude::*;
use clap::*;

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    #[clap(flatten)]
    url: SubstrateUrl,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub_api = SubUnsignedClient::new(Uri::from_static("ws://localhost:9944")).await?;

        let proof = sub_api.mmr_generate_proof(0, None).await?;
        info!("Proof: {:?}", proof);
        let mut beefy_sub = sub_api.subscribe_beefy().await?;
        while let Some(commitment) = beefy_sub.next().await.transpose()? {
            let justification = crate::relay::justification::BeefyJustification::create(
                sub_api.clone(),
                commitment,
                0,
            )
            .await?;
            info!("{:?}", justification);
        }
        Ok(())
    }
}
