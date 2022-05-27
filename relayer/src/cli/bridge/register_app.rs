use super::*;
use crate::prelude::*;
use bridge_types::H160;
use clap::*;
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
}

impl Commands {
    pub(super) async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        let eth = args.get_unsigned_ethereum().await?;
        let sub = args.get_signed_substrate().await?;
        let network_id = eth.get_chainid().await?.as_u32();
        let call = match self {
            Self::ERC20App { contract } => {
                runtime::runtime_types::erc20_app::pallet::Call::register_native_app {
                    network_id,
                    contract: *contract,
                }
            }
            Self::NativeApp { contract } => {
                runtime::runtime_types::erc20_app::pallet::Call::register_erc20_app {
                    network_id,
                    contract: *contract,
                }
            }
        };
        let result = sub
            .api()
            .tx()
            .sudo()
            .sudo(runtime::runtime_types::framenode_runtime::Call::ERC20App(
                call,
            ))?
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
