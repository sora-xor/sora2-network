use super::*;
use crate::prelude::*;
use bridge_types::H160;
use clap::*;
use common::{AssetId32, PredefinedAssetId};
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
    #[clap(long)]
    is_native: bool,
    #[clap(long)]
    address: Option<H160>,
    #[clap(long)]
    asset_id: AssetId32<PredefinedAssetId>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.eth.ethereum_url.clone()).await?;
        let sub = SubUnsignedClient::new(self.sub.substrate_url.clone())
            .await?
            .try_sign_with(&self.key.get_key_string()?)
            .await?;
        let network_id = eth.get_chainid().await?.as_u32();
        let call = if self.is_native {
            runtime::runtime_types::erc20_app::pallet::Call::register_native_asset {
                network_id,
                asset_id: self.asset_id,
            }
        } else {
            runtime::runtime_types::erc20_app::pallet::Call::register_erc20_asset {
                network_id,
                address: self.address.expect("contract address is required"),
                asset_id: self.asset_id,
            }
        };
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(runtime::runtime_types::framenode_runtime::Call::ERC20App(
                call,
            ))
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
