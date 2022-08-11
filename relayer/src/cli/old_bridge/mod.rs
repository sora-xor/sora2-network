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
    /// Download registered asset list
    DumpAssets(dump_assets::Command),
    /// Send messages from Sora to Ethereum
    Relay(relay::Command),
    /// Send tokens from Ethereum to Sora
    SendToSora(send_to_sora::Command),
    /// Register assets
    RegisterAssets(register_assets::Command),
    /// Register bridge
    RegisterBridge(register_bridge::Command),
    /// Prepare bridge for migration
    PrepareForMigration(prepare_migration::Command),
    /// Migrate bridge to another contract
    Migrate(migrate::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Self::DumpAssets(cmd) => cmd.run().await,
            Self::Relay(cmd) => cmd.run().await,
            Self::SendToSora(cmd) => cmd.run().await,
            Self::RegisterAssets(cmd) => cmd.run().await,
            Self::RegisterBridge(cmd) => cmd.run().await,
            Self::Migrate(cmd) => cmd.run().await,
            Self::PrepareForMigration(cmd) => cmd.run().await,
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
