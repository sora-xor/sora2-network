use crate::cli::prelude::*;
use bridge_types::H160;
use substrate_gen::runtime;

#[derive(Args, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    #[clap(subcommand)]
    apps: App,
}

#[derive(Subcommand, Debug)]
pub(crate) enum App {
    ERC20App {
        #[clap(long)]
        contract: H160,
    },
    NativeApp {
        #[clap(long)]
        contract: H160,
    },
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;
        let network_id = eth.get_chainid().await?;
        let call = match &self.apps {
            App::ERC20App { contract } => {
                runtime::runtime_types::erc20_app::pallet::Call::register_erc20_app {
                    network_id,
                    contract: *contract,
                }
            }
            App::NativeApp { contract } => {
                runtime::runtime_types::erc20_app::pallet::Call::register_native_app {
                    network_id,
                    contract: *contract,
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
