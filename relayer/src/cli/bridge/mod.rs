mod register_app;
mod register_asset;
mod register_bridge;
mod relay;
mod test_transfers;
mod transfer_to_ethereum;
mod transfer_to_sora;

use crate::prelude::*;
use clap::*;

use super::BaseArgs;

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    #[clap(subcommand)]
    Relay(relay::Commands),
    RegisterBridge(register_bridge::Command),
    #[clap(subcommand)]
    RegisterApp(register_app::Commands),
    #[clap(subcommand)]
    RegisterAsset(register_asset::Commands),
    TestTransfers(test_transfers::Command),
    TransferToSora(transfer_to_sora::Command),
    TransferToEthereum(transfer_to_ethereum::Command),
}

impl Commands {
    pub async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        match self {
            Commands::Relay(cmd) => cmd.run(args).await,
            Commands::RegisterBridge(cmd) => cmd.run(args).await,
            Commands::RegisterApp(cmd) => cmd.run(args).await,
            Commands::RegisterAsset(cmd) => cmd.run(args).await,
            Commands::TestTransfers(cmd) => cmd.run(args).await,
            Commands::TransferToSora(cmd) => cmd.run(args).await,
            Commands::TransferToEthereum(cmd) => cmd.run(args).await,
        }
    }
}
