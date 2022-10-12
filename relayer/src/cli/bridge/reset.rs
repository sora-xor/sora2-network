use crate::cli::prelude::*;
use bridge_types::H160;
use ethereum_gen::ValidatorSet;
use ethers::prelude::builders::ContractCall;

#[derive(Args, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// EthApp contract address
    #[clap(long)]
    eth_app: H160,
    #[clap(long)]
    reset_channels: bool,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_signed_ethereum().await?;
        let sub = self.sub.get_unsigned_substrate().await?;
        let eth_app = ethereum_gen::ETHApp::new(self.eth_app.clone(), eth.inner());
        let inbound_channel_address = eth_app.inbound().call().await?;
        let outbound_channel_address = eth_app.outbound().call().await?;
        let inbound_channel =
            ethereum_gen::InboundChannel::new(inbound_channel_address, eth.inner());
        let outbound_channel =
            ethereum_gen::OutboundChannel::new(outbound_channel_address, eth.inner());
        let beefy_address = inbound_channel.beefy_light_client().call().await?;
        let beefy = ethereum_gen::BeefyLightClient::new(beefy_address, eth.inner());
        if beefy.owner().call().await? == eth.address() {
            let block_number = sub.block_number::<u32>(None).await?;
            let block_hash = sub.block_hash(Some(1u32)).await?;
            let autorities = sub
                .api()
                .storage()
                .fetch_or_default(
                    &runtime::storage().mmr_leaf().beefy_authorities(),
                    Some(block_hash),
                )
                .await?;
            let next_autorities = sub
                .api()
                .storage()
                .fetch_or_default(
                    &runtime::storage().mmr_leaf().beefy_next_authorities(),
                    Some(block_hash),
                )
                .await?;
            info!("Reset beefy contract");
            let call: ContractCall<_, _> = beefy.reset(
                block_number as u64,
                ValidatorSet {
                    root: autorities.root.0,
                    length: autorities.len.into(),
                    id: autorities.id.into(),
                },
                ValidatorSet {
                    root: next_autorities.root.0,
                    length: next_autorities.len.into(),
                    id: next_autorities.id.into(),
                },
            );
            let call = call.legacy().from(eth.address());
            debug!("Static call: {:?}", call);
            call.call().await?;
            debug!("Send transaction");
            let pending = call.send().await?;
            debug!("Pending transaction: {:?}", pending);
            let result = pending.await?;
            debug!("Confirmed: {:?}", result);

            if self.reset_channels {
                for call in [inbound_channel.reset(), outbound_channel.reset()] {
                    info!("Reset {:?}", call.tx.to());
                    let call = call.legacy().from(eth.address());
                    debug!("Static call: {:?}", call);
                    call.call().await?;
                    debug!("Send transaction");
                    let pending = call.send().await?;
                    debug!("Pending transaction: {:?}", pending);
                    let result = pending.await?;
                    debug!("Confirmed: {:?}", result);
                }
            }
        }
        Ok(())
    }
}
