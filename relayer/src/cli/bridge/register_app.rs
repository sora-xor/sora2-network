use std::str::FromStr;

use crate::{cli::prelude::*, substrate::AssetId};
use bridge_types::{types::AssetKind, H160, U256};
use common::{AssetName, AssetSymbol, ETH};
use substrate_gen::runtime;

#[derive(Args, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    #[clap(subcommand)]
    apps: Apps,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Apps {
    /// Register ERC20App
    ERC20App {
        /// ERC20App contract address
        #[clap(long)]
        contract: H160,
    },
    /// Register NativeApp
    NativeApp {
        /// SidechainApp contract address
        #[clap(long)]
        contract: H160,
    },
    /// Register EthApp with predefined ETH asset id
    EthAppPredefined {
        #[clap(long)]
        contract: H160,
    },
    /// Register EthApp with creating new ETH asset
    EthAppNew {
        /// EthApp contract address
        #[clap(long)]
        contract: H160,
        /// ETH asset name
        #[clap(long)]
        name: String,
        /// ETH asset symbol
        #[clap(long)]
        symbol: String,
        /// ETH asset decimals
        #[clap(long)]
        decimals: u8,
    },
    /// Register EthApp with existing ETH asset
    EthAppExisting {
        /// EthApp contract address
        #[clap(long)]
        contract: H160,
        /// ETH asset id
        #[clap(long)]
        asset_id: AssetId,
    },
    /// Register MigrationApp
    MigrationApp {
        /// MigrationApp contract address
        #[clap(long)]
        contract: H160,
    },
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;
        let network_id = eth.get_chainid().await?;
        if self.check_if_registered(&sub, network_id).await? {
            return Ok(());
        }
        let call = match &self.apps {
            Apps::ERC20App { contract } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::ERC20App(
                runtime::runtime_types::erc20_app::pallet::Call::register_erc20_app {
                    network_id,
                    contract: *contract,
                }
            )
            }
            Apps::NativeApp { contract } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::ERC20App(
                runtime::runtime_types::erc20_app::pallet::Call::register_native_app {
                    network_id,
                    contract: *contract,
                }
            )
            }
            Apps::EthAppPredefined { contract } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network_with_existing_asset {
                    network_id,
                    contract: *contract,
                    asset_id: ETH
                }
            )
            }
            Apps::EthAppNew { contract, name, symbol, decimals } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network {
                    network_id,
                    contract: *contract,
                    name: AssetName::from_str(name.as_str()).map_err(|err| anyhow!(format!("{}", err)))?,
                    symbol: AssetSymbol::from_str(symbol.as_str()).map_err(|err| anyhow!(format!("{}", err)))?,
                    decimals: *decimals
                }
            )
            }
            Apps::EthAppExisting { contract, asset_id } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network_with_existing_asset {
                    network_id,
                    contract: *contract,
                    asset_id: *asset_id
                }
            )
            }
            Apps::MigrationApp { contract } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::MigrationApp(
                runtime::runtime_types::migration_app::pallet::Call::register_network {
                    network_id,
                    contract: *contract,
                }
            )
            }
        };
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
        Ok(())
    }

    async fn check_if_registered(
        &self,
        sub: &SubSignedClient,
        network_id: U256,
    ) -> AnyResult<bool> {
        let (contract, registered) = match self.apps {
            Apps::ERC20App { contract } => {
                let registered = sub
                    .api()
                    .storage()
                    .fetch(
                        &sub_runtime::storage()
                            .erc20_app()
                            .app_addresses(&network_id, &AssetKind::Sidechain),
                        None,
                    )
                    .await?;
                (contract, registered)
            }
            Apps::NativeApp { contract } => {
                let registered = sub
                    .api()
                    .storage()
                    .fetch(
                        &sub_runtime::storage()
                            .erc20_app()
                            .app_addresses(&network_id, &AssetKind::Thischain),
                        None,
                    )
                    .await?;
                (contract, registered)
            }
            Apps::EthAppPredefined { contract }
            | Apps::EthAppNew { contract, .. }
            | Apps::EthAppExisting { contract, .. } => {
                let registered = sub
                    .api()
                    .storage()
                    .fetch(
                        &sub_runtime::storage().eth_app().addresses(&network_id),
                        None,
                    )
                    .await?
                    .map(|(contract, _)| contract);
                (contract, registered)
            }
            Apps::MigrationApp { contract } => {
                let registered = sub
                    .api()
                    .storage()
                    .fetch(
                        &sub_runtime::storage()
                            .migration_app()
                            .addresses(&network_id),
                        None,
                    )
                    .await?;
                (contract, registered)
            }
        };
        if let Some(registered) = registered {
            if registered == contract {
                info!("App already registered");
            } else {
                info!(
                    "App already registered with different contract address: {} != {}",
                    contract, registered
                );
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
