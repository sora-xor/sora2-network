mod register;
mod register_app;
mod register_asset;
mod transfer_to_ethereum;
mod transfer_to_sora;

use super::prelude::*;

#[derive(Subcommand, Debug)]
pub enum Commands {
    TransferToSora(transfer_to_sora::Command),
    TransferToEthereum(transfer_to_ethereum::Command),
    Register(register::Command),
    RegisterApp(register_app::Command),
    RegisterAsset(register_asset::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Self::TransferToSora(cmd) => cmd.run().await,
            Self::TransferToEthereum(cmd) => cmd.run().await,
            Self::Register(cmd) => cmd.run().await,
            Self::RegisterApp(cmd) => cmd.run().await,
            Self::RegisterAsset(cmd) => cmd.run().await,
        }
    }
}
