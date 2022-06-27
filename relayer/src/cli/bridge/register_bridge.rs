use crate::cli::prelude::*;
use crate::ethereum::make_header;
use bridge_types::network_params::NetworkConfig;
use bridge_types::H160;
use std::path::PathBuf;
use substrate_gen::runtime;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    #[clap(long, short)]
    descendants_until_final: u64,
    #[clap(long)]
    eth_app: H160,
    #[clap(long)]
    migration_app: Option<H160>,
    #[clap(subcommand)]
    network: Network,
}

#[derive(Subcommand, Clone, Debug)]
enum Network {
    Mainnet,
    Ropsten,
    Sepolia,
    Rinkeby,
    Goerli,
    Custom {
        #[clap(long)]
        path: PathBuf,
    },
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;

        let eth_app = ethereum_gen::ETHApp::new(self.eth_app, eth.inner());
        let basic_outbound_channel = eth_app.channels(0).call().await?.1;
        let incentivized_outbound_channel = eth_app.channels(1).call().await?.1;

        let network_id = eth.get_chainid().await?;
        let network_config = match &self.network {
            Network::Mainnet => NetworkConfig::Mainnet,
            Network::Ropsten => NetworkConfig::Ropsten,
            Network::Sepolia => NetworkConfig::Sepolia,
            Network::Rinkeby => NetworkConfig::Rinkeby,
            Network::Goerli => NetworkConfig::Goerli,
            Network::Custom { path } => {
                let bytes = std::fs::read(path)?;
                serde_json::de::from_slice(&bytes)?
            }
        };
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
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(
                false,
                runtime::runtime_types::framenode_runtime::Call::EthereumLightClient(
                    runtime::runtime_types::ethereum_light_client::pallet::Call::register_network {
                        header,
                        network_config,
                        initial_difficulty: Default::default(),
                    },
                ),
            )?
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Result: {:?}", result.iter().collect::<Vec<_>>());
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(false,
                runtime::runtime_types::framenode_runtime::Call::BasicInboundChannel(
                    runtime::runtime_types::basic_channel::inbound::pallet::Call::register_channel {
                        network_id,
                        channel: basic_outbound_channel
                    },
                ),
            )?
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Result: {:?}", result.iter().collect::<Vec<_>>());
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(false,
                runtime::runtime_types::framenode_runtime::Call::IncentivizedInboundChannel(
                    runtime::runtime_types::incentivized_channel::inbound::pallet::Call::register_channel {
                        network_id,
                        channel: incentivized_outbound_channel
                    },
                ),
            )?
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Result: {:?}", result.iter().collect::<Vec<_>>());
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(false, runtime::runtime_types::framenode_runtime::Call::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network_with_existing_asset {
                    network_id,
                    contract: self.eth_app,
                    asset_id: common::ETH,
                },
            ))?
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Result: {:?}", result.iter().collect::<Vec<_>>());
        if let Some(migration_app) = self.migration_app {
            let result = sub
                .api()
                .tx()
                .sudo()
                .sudo(
                    false,
                    runtime::runtime_types::framenode_runtime::Call::MigrationApp(
                        runtime::runtime_types::migration_app::pallet::Call::register_network {
                            network_id,
                            contract: migration_app,
                        },
                    ),
                )?
                .sign_and_submit_then_watch_default(&sub)
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?;
            info!("Result: {:?}", result.iter().collect::<Vec<_>>());
        }
        Ok(())
    }
}
