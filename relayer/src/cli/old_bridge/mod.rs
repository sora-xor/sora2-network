mod dump_assets;
mod migrate;
mod prepare_migration;
mod register_assets;
mod register_bridge;
mod relay;
mod send_to_sora;

use bridge_types::H160;

use crate::cli::prelude::*;
use crate::substrate::AssetId;

#[derive(Subcommand, Debug)]
pub enum Commands {
    DumpAssets(dump_assets::Command),
    Relay(relay::Command),
    SendToSora(send_to_sora::Command),
    RegisterAssets(register_assets::Command),
    RegisterBridge(register_bridge::Command),
    PrepareForMigration(prepare_migration::Command),
    Migrate(migrate::Command),
}

impl Commands {
    pub async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        match self {
            Self::DumpAssets(cmd) => cmd.run(args).await,
            Self::Relay(cmd) => cmd.run(args).await,
            Self::SendToSora(cmd) => cmd.run(args).await,
            Self::RegisterAssets(cmd) => cmd.run(args).await,
            Self::RegisterBridge(cmd) => cmd.run(args).await,
            Self::Migrate(cmd) => cmd.run(args).await,
            Self::PrepareForMigration(cmd) => cmd.run(args).await,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInfo {
    asset_id: AssetId,
    is_mintable: String,
    name: String,
    precision: String,
    symbol: String,
    kind: String,
    address: Option<H160>,
}
