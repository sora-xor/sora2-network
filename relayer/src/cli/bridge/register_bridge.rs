use crate::cli::prelude::*;
use crate::ethereum::make_header;
use bridge_types::H160;
use substrate_gen::runtime;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Confirmations until block is considered finalized
    #[clap(long, short)]
    descendants_until_final: u64,
    /// OutboundChannel contract address
    #[clap(long)]
    outbound_channel: H160,
    #[clap(flatten)]
    network: Network,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;

        let network_id = eth.get_chainid().await?;
        let is_light_client_registered = sub
            .api()
            .storage()
            .fetch(
                &sub_runtime::storage()
                    .ethereum_light_client()
                    .network_config(&network_id),
                None,
            )
            .await?
            .is_some();

        if !is_light_client_registered {
            let network_config = self.network.config()?;
            if network_id != network_config.chain_id() {
                return Err(anyhow!(
                    "Wrong ethereum node chain id, expected {}, actual {}",
                    network_config.chain_id(),
                    network_id
                ));
            }
            let number = eth.get_block_number().await? - self.descendants_until_final;
            let block = eth.get_block(number).await?.expect("block not found");
            let header = make_header(block);
            let call = runtime::runtime_types::framenode_runtime::Call::EthereumLightClient(
                runtime::runtime_types::ethereum_light_client::pallet::Call::register_network {
                    header,
                    network_config,
                    initial_difficulty: Default::default(),
                },
            );
            info!("Sudo call extrinsic: {:?}", call);
            let result = sub
                .api()
                .tx()
                .sign_and_submit_then_watch_default(&runtime::tx().sudo().sudo(call), &sub)
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?;
            info!("Extrinsic successful");
            sub_log_tx_events(result);
        } else {
            info!("Light client already registered");
        }

        let is_channel_registered = sub
            .api()
            .storage()
            .fetch(
                &sub_runtime::storage()
                    .bridge_inbound_channel()
                    .channel_addresses(&network_id),
                None,
            )
            .await?
            .is_some();
        if !is_channel_registered {
            let call = runtime::runtime_types::framenode_runtime::Call::BridgeInboundChannel(
                runtime::runtime_types::bridge_channel::inbound::pallet::Call::register_channel {
                    network_id,
                    channel: self.outbound_channel,
                },
            );
            info!("Sudo call extrinsic: {:?}", call);
            let result = sub
                .api()
                .tx()
                .sign_and_submit_then_watch_default(&runtime::tx().sudo().sudo(call), &sub)
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?;
            info!("Extrinsic successful");
            sub_log_tx_events(result);
        } else {
            info!("Channel already registered");
        }
        Ok(())
    }
}
