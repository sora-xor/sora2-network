use super::*;
use crate::prelude::*;
use bridge_types::H160;
use clap::*;
use ethers::prelude::Middleware;
use substrate_gen::runtime;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    eth: EthereumUrl,
    #[clap(flatten)]
    sub: SubstrateUrl,
    #[clap(flatten)]
    key: SubstrateKey,
    #[clap(long)]
    is_native: bool,
    #[clap(long)]
    contract: H160,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.eth.get()).await?;
        let sub = SubUnsignedClient::new(self.sub.get())
            .await?
            .try_sign_with(&self.key.get_key_string()?)
            .await?;
        let network_id = eth.get_chainid().await?.as_u32();
        let call = if self.is_native {
            runtime::runtime_types::erc20_app::pallet::Call::register_native_app {
                network_id,
                contract: self.contract,
            }
        } else {
            runtime::runtime_types::erc20_app::pallet::Call::register_erc20_app {
                network_id,
                contract: self.contract,
            }
        };
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(runtime::runtime_types::framenode_runtime::Call::ERC20App(
                call,
            ))
            .sign_and_submit_then_watch(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Result: {:?}", result);
        Ok(())
    }
}
