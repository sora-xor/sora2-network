use super::*;
use crate::prelude::*;
use crate::relay::substrate::RelayBuilder;
use clap::*;
use ethers::prelude::Middleware;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(short, long)]
    send_unneeded_commitments: bool,
}

impl Command {
    pub(super) async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        let eth = args.get_signed_ethereum().await?;
        let sub = args.get_unsigned_substrate().await?;
        let network_id = eth.inner().get_chainid().await?.as_u32();
        let eth_app = sub
            .api()
            .storage()
            .eth_app()
            .addresses(&network_id, None)
            .await?
            .ok_or(anyhow!("Network is not registered"))?
            .0;
        let eth_app = ethereum_gen::ETHApp::new(eth_app, eth.inner());
        let basic_inbound_address = eth_app.channels(0).call().await?.0;
        let incentivized_inbound_address = eth_app.channels(1).call().await?.0;
        let basic = ethereum_gen::BasicInboundChannel::new(basic_inbound_address, eth.inner());
        let beefy = basic.beefy_light_client().call().await?;
        RelayBuilder::new()
            .with_substrate_client(sub)
            .with_ethereum_client(eth)
            .with_basic_contract(basic_inbound_address)
            .with_incentivized_contract(incentivized_inbound_address)
            .with_beefy_contract(beefy)
            .build()
            .await?
            .run(!self.send_unneeded_commitments)
            .await?;
        Ok(())
    }
}
