use crate::cli::prelude::*;
use crate::relay::justification::BeefyJustification;
use beefy_gadget_rpc::BeefyApiClient;

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub_api = self.sub.get_unsigned_substrate().await?;

        // let proof = sub_api.mmr_generate_proof(1, None).await?;
        // info!("Proof: {:#?}", proof);
        let mut beefy_sub = sub_api.beefy().subscribe_justifications().await?;
        while let Some(commitment) = beefy_sub.next().await.transpose()? {
            let justification =
                BeefyJustification::create(sub_api.clone(), commitment.decode()?).await?;
            println!("{:#?}", justification);
        }
        Ok(())
    }
}
