mod bridge;
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
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    #[clap(long, global = true)]
    substrate_key: Option<String>,
    #[clap(long, global = true)]
    substrate_key_file: Option<String>,
    #[clap(long, global = true)]
    substrate_url: Option<String>,
    #[clap(long, global = true)]
    ethereum_key: Option<String>,
    #[clap(long, global = true)]
    ethereum_key_file: Option<String>,
    #[clap(long, global = true)]
    ethereum_url: Option<Url>,
    #[clap(subcommand)]
    commands: Commands,
}

impl Cli {
    pub async fn run(&self) -> AnyResult<()> {
        self.commands.run().await
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
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Self::SubscribeBeefy(cmd) => cmd.run().await,
            Self::FetchEthereumHeader(cmd) => cmd.run().await,
            Self::MintTestToken(cmd) => cmd.run().await,
            Self::Bridge(cmd) => cmd.run().await,
            Self::OldBridge(cmd) => cmd.run().await,
        }
    }
}

pub mod prelude {
    pub use crate::cli::utils::*;
    pub use crate::prelude::*;
    pub use clap::*;
    pub use ethers::providers::Middleware;
}
