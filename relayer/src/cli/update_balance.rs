use crate::cli::prelude::*;
use crate::substrate::{AccountId, AssetId};

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    sub: SubstrateUrl,
    #[clap(flatten)]
    key: SubstrateKey,
    #[clap(short, long)]
    asset_id: AssetId,
    #[clap(short, long)]
    amount: i128,
    #[clap(short, long)]
    who: AccountId,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = SubUnsignedClient::new(self.sub.get())
            .await?
            .try_sign_with(&self.key.get_key_string()?)
            .await?;

        let progress = sub
            .api()
            .tx()
            .sudo()
            .sudo(sub_types::framenode_runtime::Call::Currencies(
                sub_types::orml_currencies::module::Call::update_balance {
                    who: self.who.clone(),
                    currency_id: self.asset_id,
                    amount: self.amount,
                },
            ))
            .sign_and_submit_then_watch(&sub)
            .await?;
        info!("Extrinsic submitted");
        let in_block = progress.wait_for_in_block().await?;
        info!("Extrinsic in block");
        in_block.wait_for_success().await?;
        info!("Extrinsic success");

        Ok(())
    }
}
