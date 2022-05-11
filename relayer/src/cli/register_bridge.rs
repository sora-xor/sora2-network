use super::*;
use crate::ethereum::make_header;
use crate::prelude::*;
use bridge_types::H160;
use clap::*;
use ethers::prelude::Middleware;
use substrate_gen::runtime;

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    #[clap(flatten)]
    eth: EthereumUrl,
    #[clap(flatten)]
    sub: SubstrateUrl,
    #[clap(flatten)]
    key: SubstrateKey,
    #[clap(long, short)]
    descendants_until_final: u64,
    #[clap(long)]
    basic_channel_inbound: H160,
    #[clap(long)]
    incentivized_channel_inbound: H160,
    #[clap(long)]
    eth_app: H160,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.eth.ethereum_url.clone()).await?;
        let sub = SubUnsignedClient::new(self.sub.substrate_url.clone())
            .await?
            .try_sign_with(&self.key.get_key_string()?)
            .await?;
        let network_id = eth.get_chainid().await?.as_u32();
        let number = eth.get_block_number().await? - self.descendants_until_final;
        let block = eth.get_block(number).await?.expect("block not found");
        let header = make_header(block);
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(
                runtime::runtime_types::framenode_runtime::Call::EthereumLightClient(
                    runtime::runtime_types::ethereum_light_client::pallet::Call::register_network {
                        header,
                        network_id,
                        initial_difficulty: Default::default(),
                    },
                ),
            )
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Result: {:?}", result);
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(
                runtime::runtime_types::framenode_runtime::Call::BasicInboundChannel(
                    runtime::runtime_types::basic_channel::inbound::pallet::Call::register_channel {
                        network_id,
                        channel: self.basic_channel_inbound
                    },
                ),
            )
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Result: {:?}", result);
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(
                runtime::runtime_types::framenode_runtime::Call::IncentivizedInboundChannel(
                    runtime::runtime_types::incentivized_channel::inbound::pallet::Call::register_channel {
                        network_id,
                        channel: self.incentivized_channel_inbound
                    },
                ),
            )
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Result: {:?}", result);
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(runtime::runtime_types::framenode_runtime::Call::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network {
                    network_id,
                    contract: self.eth_app,
                    asset_id: common::ETH,
                },
            ))
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Result: {:?}", result);
        Ok(())
    }
}
