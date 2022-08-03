mod register_app;
mod register_asset;
mod register_bridge;
mod relay;
mod reset;
mod test_transfers;
mod transfer_to_ethereum;
mod transfer_to_sora;

use crate::cli::prelude::*;

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    #[clap(subcommand)]
    Relay(relay::Commands),
    RegisterBridge(register_bridge::Command),
    RegisterApp(register_app::Command),
    RegisterAsset(register_asset::Command),
    TestTransfers(test_transfers::Command),
    TransferToSora(transfer_to_sora::Command),
    TransferToEthereum(transfer_to_ethereum::Command),
    Reset(reset::Command),
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
        }
    }
}
