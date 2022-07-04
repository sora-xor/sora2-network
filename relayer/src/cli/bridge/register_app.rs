use std::str::FromStr;

use super::*;
use crate::{prelude::*, substrate::AssetId};
use bridge_types::H160;
use clap::*;
use common::{AssetName, AssetSymbol, ETH};
use ethers::prelude::Middleware;
use substrate_gen::runtime;

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    ERC20App {
        #[clap(long)]
        contract: H160,
    },
    NativeApp {
        #[clap(long)]
        contract: H160,
    },
    EthAppPredefined {
        #[clap(long)]
        contract: H160,
    },
    EthAppNew {
        #[clap(long)]
        contract: H160,
        #[clap(long)]
        name: String,
        #[clap(long)]
        symbol: String,
    },
    EthAppExisting {
        #[clap(long)]
        contract: H160,
        #[clap(long)]
        asset_id: AssetId,
    },
    MigrationApp {
        #[clap(long)]
        contract: H160,
    },
}

impl Commands {
    pub(super) async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        let eth = args.get_unsigned_ethereum().await?;
        let sub = args.get_signed_substrate().await?;
        let network_id = eth.get_chainid().await?;
        let call = match self {
            Self::ERC20App { contract } => {
                runtime::runtime_types::framenode_runtime::Call::ERC20App(
                runtime::runtime_types::erc20_app::pallet::Call::register_erc20_app {
                    network_id,
                    contract: *contract,
                }
            )
            }
            Self::NativeApp { contract } => {
                runtime::runtime_types::framenode_runtime::Call::ERC20App(
                runtime::runtime_types::erc20_app::pallet::Call::register_native_app {
                    network_id,
                    contract: *contract,
                }
            )
            }
            Self::EthAppPredefined { contract } => {
                runtime::runtime_types::framenode_runtime::Call::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network_with_existing_asset {
                    network_id,
                    contract: *contract,
                    asset_id: ETH
                }
            )
            }
            Self::EthAppNew { contract, name, symbol } => {
                runtime::runtime_types::framenode_runtime::Call::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network {
                    network_id,
                    contract: *contract,
                    name: AssetName::from_str(name.as_str()).map_err(|err| anyhow!(format!("{}", err)))?,
                    symbol: AssetSymbol::from_str(symbol.as_str()).map_err(|err| anyhow!(format!("{}", err)))?,
                }
            )
            }
            Self::EthAppExisting { contract, asset_id } => {
                runtime::runtime_types::framenode_runtime::Call::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network_with_existing_asset {
                    network_id,
                    contract: *contract,
                    asset_id: *asset_id
                }
            )
            }
            Self::MigrationApp { contract } => {
                runtime::runtime_types::framenode_runtime::Call::MigrationApp(
                runtime::runtime_types::migration_app::pallet::Call::register_network {
                    network_id,
                    contract: *contract,
                }
            )
            }
        };
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(false, call)?
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Result: {:?}", result.iter().collect::<Vec<_>>());
        Ok(())
    }
}
