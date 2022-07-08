use std::str::FromStr;

use super::*;
use crate::prelude::*;
use bridge_types::H160;
use clap::*;
use common::{AssetId32, AssetName, AssetSymbol, PredefinedAssetId};
use ethers::prelude::Middleware;
use substrate_gen::runtime;

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    ExistingERC20 {
        #[clap(long)]
        asset_id: AssetId32<PredefinedAssetId>,
        #[clap(long)]
        address: H160,
    },
    ERC20 {
        #[clap(long)]
        address: H160,
        #[clap(long)]
        name: String,
        #[clap(long)]
        symbol: String,
        #[clap(long)]
        decimals: u8,
    },
    Native {
        #[clap(long)]
        asset_id: AssetId32<PredefinedAssetId>,
    },
}

impl Commands {
    pub(super) async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        let eth = args.get_unsigned_ethereum().await?;
        let sub = args.get_signed_substrate().await?;
        let network_id = eth.get_chainid().await?;
        let call = match self {
            Self::ExistingERC20 { asset_id, address } => {
                runtime::runtime_types::erc20_app::pallet::Call::register_existing_erc20_asset {
                    network_id,
                    asset_id: asset_id.clone(),
                    address: *address,
                }
            }
            Self::ERC20 {
                address,
                name,
                symbol,
                decimals,
            } => runtime::runtime_types::erc20_app::pallet::Call::register_erc20_asset {
                network_id,
                address: address.clone(),
                name: AssetName::from_str(name.as_str()).unwrap(),
                symbol: AssetSymbol::from_str(symbol.as_str()).unwrap(),
                decimals: *decimals,
            },
            Self::Native { asset_id } => {
                runtime::runtime_types::erc20_app::pallet::Call::register_native_asset {
                    network_id,
                    asset_id: asset_id.clone(),
                }
            }
        };
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(
                false,
                runtime::runtime_types::framenode_runtime::Call::ERC20App(call),
            )?
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
