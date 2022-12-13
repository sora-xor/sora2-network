mod bridge;
mod calc_dag_roots;
mod error;
mod fetch_ethereum_header;
mod mint_test_token;
mod subscribe_beefy;
pub mod utils;

use std::path::PathBuf;

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
    para: ParachainClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Substrate account derive URI
    #[clap(long, global = true)]
    substrate_key: Option<String>,
    /// File with Substrate account derive URI
    #[clap(long, global = true)]
    substrate_key_file: Option<String>,
    /// Substrate node endpoint
    #[clap(long, global = true)]
    substrate_url: Option<String>,
    /// Parachain account derive URI
    #[clap(long, global = true)]
    parachain_key: Option<String>,
    /// File with Parachain account derive URI
    #[clap(long, global = true)]
    parachain_key_file: Option<String>,
    /// Parachain node endpoint
    #[clap(long, global = true)]
    parachain_url: Option<String>,
    /// Ethereum private key
    #[clap(long, global = true)]
    ethereum_key: Option<String>,
    /// File with Ethereum private key
    #[clap(long, global = true)]
    ethereum_key_file: Option<String>,
    /// Ethereum node endpoint
    #[clap(long, global = true)]
    ethereum_url: Option<Url>,
    /// Path for gas estimations
    #[clap(long, global = true)]
    gas_metrics_path: Option<PathBuf>,
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
    /// Subscribe beefy to new commitments
    SubscribeBeefy(subscribe_beefy::Command),
    /// Fetch Ethereum header
    FetchEthereumHeader(fetch_ethereum_header::Command),
    /// Mint test token (work for tokens with mint method)
    MintTestToken(mint_test_token::Command),
    /// Operations with bridge
    #[clap(subcommand)]
    Bridge(bridge::Commands),
    /// Calculate DAG roots for light client
    CalcDagRoots(calc_dag_roots::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Self::SubscribeBeefy(cmd) => cmd.run().await,
            Self::FetchEthereumHeader(cmd) => cmd.run().await,
            Self::MintTestToken(cmd) => cmd.run().await,
            Self::Bridge(cmd) => cmd.run().await,
            Self::CalcDagRoots(cmd) => cmd.run().await,
        }
    }
}

pub mod prelude {
    pub use crate::cli::utils::*;
    pub use crate::prelude::*;
    pub use clap::*;
    pub use ethers::providers::Middleware;
}
