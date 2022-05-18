use super::*;
use crate::prelude::*;
use crate::relay::justification::BeefyJustification;
use clap::*;

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    #[clap(flatten)]
    url: SubstrateUrl,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub_api = SubUnsignedClient::new(self.url.get()).await?;
        let beefy_start_block = sub_api.beefy_start_block().await?;

        let proof = sub_api.mmr_generate_proof(0, None).await?;
        info!("Proof: {:#?}", proof);
        let mut beefy_sub = sub_api.subscribe_beefy().await?;
        while let Some(commitment) = beefy_sub.next().await.transpose()? {
            let justification = BeefyJustification::create(
                sub_api.clone(),
                commitment.decode()?,
                beefy_start_block as u32,
            )
            .await?;
            println!("{:#?}", justification);
        }
        Ok(())
    }
}
