mod bridge;
mod common;
mod error;
mod estimate_gas;
mod fetch_ethereum_header;
mod mint_test_token;
mod old_bridge;
mod relay;
mod subscribe_beefy;
mod update_balance;
mod substrate_relay;
mod test_transfers;
mod transfer_to_ethereum;
mod transfer_to_sora;

use prelude::*;

/// App struct
#[derive(Parser, Debug)]
#[clap(version, author)]
pub struct Cli {
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
    EstimateGas(estimate_gas::Command),
    MintTestToken(mint_test_token::Command),
    UpdateBalance(update_balance::Command),
    #[clap(subcommand)]
    Bridge(bridge::Commands),
    #[clap(subcommand)]
    Relay(relay::Commands),
    #[clap(subcommand)]
    OldBridge(old_bridge::Commands),
    #[clap(subcommand)]
    TestTransfers(test_transfers::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Self::SubscribeBeefy(cmd) => cmd.run().await,
            Self::FetchEthereumHeader(cmd) => cmd.run().await,
            Self::EstimateGas(cmd) => cmd.run().await,
            Self::MintTestToken(cmd) => cmd.run().await,
            Self::Bridge(cmd) => cmd.run().await,
            Self::Relay(cmd) => cmd.run().await,
            Self::OldBridge(cmd) => cmd.run().await,
            Self::UpdateBalance(cmd) => cmd.run().await,
            Self::TestTransfers(cmd) => cmd.run().await,
        }
    }
}

mod prelude {
    pub use super::common::*;
    pub use super::error::*;
    pub use crate::prelude::*;
    pub use clap::*;
}
