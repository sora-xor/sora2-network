mod fixtures;
mod register_app;
mod register_asset;
mod register_bridge;
mod register_substrate_bridge;
mod relay;
mod reset;
mod test_transfers;
mod transfer_to_ethereum;
mod transfer_to_sora;

use crate::cli::prelude::*;

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Relay operations for bridge
    #[clap(subcommand)]
    Relay(relay::Commands),
    /// Register bridge
    RegisterBridge(register_bridge::Command),
    /// Register bridge app
    RegisterApp(register_app::Command),
    /// Register asset
    RegisterAsset(register_asset::Command),
    /// Make test transfers through bridge
    TestTransfers(test_transfers::Command),
    /// Transfer tokens from Ethereum to Sora
    TransferToSora(transfer_to_sora::Command),
    /// Transfer tokens from Sora to Ethereum
    TransferToEthereum(transfer_to_ethereum::Command),
    /// Reset bridge contracts
    Reset(reset::Command),
    Fixtures(fixtures::Command),
    RegisterSubstrateBridge(register_substrate_bridge::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Commands::Relay(cmd) => cmd.run().await,
            Commands::RegisterBridge(cmd) => cmd.run().await,
            Commands::RegisterApp(cmd) => cmd.run().await,
            Commands::RegisterAsset(cmd) => cmd.run().await,
            Commands::TestTransfers(cmd) => cmd.run().await,
            Commands::TransferToSora(cmd) => cmd.run().await,
            Commands::TransferToEthereum(cmd) => cmd.run().await,
            Commands::Reset(cmd) => cmd.run().await,
            Commands::Fixtures(cmd) => cmd.run().await,
            Commands::RegisterSubstrateBridge(cmd) => cmd.run().await,
        }
    }
}
