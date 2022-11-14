mod ethereum_relay;
mod parachain_to_parachain_relay;
mod parachain_to_sora_relay;
mod sora_to_parachain_relay;
mod sora_to_sora_relay;
mod substrate_relay;

use crate::cli::prelude::*;
use clap::*;

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Relay Etheerum headers and messages to Sora
    Ethereum(ethereum_relay::Command),
    /// Relay Beefy commitments and bridge messages to Ethereum
    Substrate(substrate_relay::Command),
    /// Relay Beefy commitments and bridge messages from Sora to Parachain
    SoraToParachain(sora_to_parachain_relay::Command),
    /// Relay Beefy commitments and bridge messages from Parachain to Sora
    ParachainToSora(parachain_to_sora_relay::Command),
    /// Relay Beefy commitments and bridge messages from Sora to Sora
    SoraToSora(sora_to_sora_relay::Command),
    /// Relay Beefy commitments and bridge messages from Parachain to Parachain
    ParachainToParachain(parachain_to_parachain_relay::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Commands::Ethereum(cmd) => cmd.run().await,
            Commands::Substrate(cmd) => cmd.run().await,
            Commands::SoraToParachain(cmd) => cmd.run().await,
            Commands::ParachainToSora(cmd) => cmd.run().await,
            Commands::SoraToSora(cmd) => cmd.run().await,
            Commands::ParachainToParachain(cmd) => cmd.run().await,
        }
    }
}
