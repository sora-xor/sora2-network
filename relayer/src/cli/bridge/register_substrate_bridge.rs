use crate::{cli::prelude::*, substrate::BlockNumber};

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    para: ParachainClient,
    #[clap(long)]
    parachain: bool,
    #[clap(long)]
    sora: bool,
    #[clap(long)]
    both: bool,
    #[clap(long)]
    mainnet_block: Option<BlockNumber<MainnetConfig>>,
    #[clap(long)]
    parachain_block: Option<BlockNumber<ParachainConfig>>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = self.sub.get_signed_substrate().await?;
        let para = self.para.get_signed_substrate().await?;

        if self.parachain || self.both {
            let (block_number, block_hash) = if let Some(block) = self.mainnet_block {
                let hash = sub
                    .api()
                    .rpc()
                    .block_hash(Some(block.into()))
                    .await?
                    .ok_or(anyhow!("Block {} not found on mainnet", block))?;
                (block, hash)
            } else {
                let hash = sub.api().rpc().finalized_head().await?;
                let number = sub.block_number(Some(hash)).await?;
                (number, hash)
            };
            let authorities = sub
                .api()
                .storage()
                .fetch(
                    &mainnet_runtime::storage().mmr_leaf().beefy_authorities(),
                    Some(block_hash),
                )
                .await?
                .ok_or(anyhow!("Beefy authorities not found"))?;
            let next_authorities = sub
                .api()
                .storage()
                .fetch(
                    &mainnet_runtime::storage()
                        .mmr_leaf()
                        .beefy_next_authorities(),
                    Some(block_hash),
                )
                .await?
                .ok_or(anyhow!("Beefy authorities not found"))?;

            let call = parachain_runtime::runtime_types::parachain_template_runtime::RuntimeCall::BeefyLightClient(parachain_runtime::runtime_types::beefy_light_client::pallet::Call::initialize {
                latest_beefy_block: block_number.into(),
                validator_set: authorities,
                next_validator_set: next_authorities });
            info!("Submit call: {call:?}");
            let call = parachain_runtime::tx().sudo().sudo(call);
            let events = para
                .api()
                .tx()
                .sign_and_submit_then_watch_default(&call, &para)
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?;
            sub_log_tx_events::<parachain_runtime::Event, _>(events);
        }

        if self.sora || self.both {
            let (block_number, block_hash) = if let Some(block) = self.parachain_block {
                let hash = para
                    .api()
                    .rpc()
                    .block_hash(Some(block.into()))
                    .await?
                    .ok_or(anyhow!("Block {} not found on mainnet", block))?;
                (block, hash)
            } else {
                let hash = para.api().rpc().finalized_head().await?;
                let number = para.block_number(Some(hash)).await?;
                (number, hash)
            };
            let authorities = para
                .api()
                .storage()
                .fetch(
                    &parachain_runtime::storage().beefy_mmr().beefy_authorities(),
                    Some(block_hash),
                )
                .await?
                .ok_or(anyhow!("Beefy authorities not found"))?;
            let next_authorities = para
                .api()
                .storage()
                .fetch(
                    &parachain_runtime::storage()
                        .beefy_mmr()
                        .beefy_next_authorities(),
                    Some(block_hash),
                )
                .await?
                .ok_or(anyhow!("Beefy authorities not found"))?;

            let call =
                mainnet_runtime::runtime_types::framenode_runtime::RuntimeCall::BeefyLightClient(
                    mainnet_runtime::runtime_types::beefy_light_client::pallet::Call::initialize {
                        latest_beefy_block: block_number.into(),
                        validator_set: authorities,
                        next_validator_set: next_authorities,
                    },
                );
            info!("Submit call: {call:?}");
            let call = mainnet_runtime::tx().sudo().sudo(call);
            let events = sub
                .api()
                .tx()
                .sign_and_submit_then_watch_default(&call, &sub)
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?;
            sub_log_tx_events::<mainnet_runtime::Event, _>(events);
        }

        Ok(())
    }
}
