mod ethereum_relay;
mod substrate_relay;

use crate::cli::prelude::*;
use clap::*;

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Relay Etheerum headers and messages to Sora
    Ethereum(ethereum_relay::Command),
    /// Relay Beefy commitments and bridge messages to Ethereum
    Substrate(substrate_relay::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Commands::Ethereum(cmd) => cmd.run().await,
            Commands::Substrate(cmd) => cmd.run().await,
        }
    }
}
