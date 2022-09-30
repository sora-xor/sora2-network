mod bridge;
mod calc_dag_roots;
mod error;
mod fetch_ethereum_header;
mod mint_test_token;
mod old_bridge;
mod subscribe_beefy;
pub mod utils;

pub use utils::*;

use crate::prelude::*;
use clap::*;

/// App struct
#[derive(Parser, Debug)]
#[clap(version, author)]
pub struct Cli {
    #[clap(flatten)]
    base_args: BaseArgs,
    #[clap(subcommand)]
    commands: Commands,
}

impl Cli {
    pub async fn run(&self) -> AnyResult<()> {
        self.commands.run(&self.base_args).await
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    SubscribeBeefy(subscribe_beefy::Command),
    FetchEthereumHeader(fetch_ethereum_header::Command),
    MintTestToken(mint_test_token::Command),
    #[clap(subcommand)]
    Bridge(bridge::Commands),
    #[clap(subcommand)]
    OldBridge(old_bridge::Commands),
    CalcDagRoots(calc_dag_roots::Command),
}

impl Commands {
    pub async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        match self {
            Self::SubscribeBeefy(cmd) => cmd.run(args).await,
            Self::FetchEthereumHeader(cmd) => cmd.run(args).await,
            Self::MintTestToken(cmd) => cmd.run(args).await,
            Self::Bridge(cmd) => cmd.run(args).await,
            Self::OldBridge(cmd) => cmd.run(args).await,
            Self::CalcDagRoots(cmd) => cmd.run(args).await,
        }
    }
}

pub mod prelude {
    pub use crate::cli::utils::*;
    pub use crate::prelude::*;
    pub use clap::*;
}
