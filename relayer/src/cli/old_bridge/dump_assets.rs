use crate::cli::prelude::*;
use crate::substrate::AssetId;
use std::path::PathBuf;
use substrate_gen::AssetKind;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    /// Output file path
    #[clap(long, short)]
    output: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AssetInfo {
    name: String,
    symbol: String,
    decimals: u8,
    asset_id: AssetId,
    asset_kind: AssetKind,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct AssetsDump {
    assets: Vec<AssetInfo>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = self.sub.get_unsigned_substrate().await?;
        let mut asset_iter = sub
            .api()
            .storage()
            .eth_bridge()
            .registered_asset_iter(false, None)
            .await?;
        let mut assets = AssetsDump::default();
        while let Some((asset_id, asset_kind)) = asset_iter.next().await? {
            let asset_id = crate::substrate::AssetId::from_bytes(asset_id.0.try_into().unwrap());
            let (asset_symbol, asset_name, decimals, _, _, _) = sub
                .api()
                .storage()
                .assets()
                .asset_infos(false, &asset_id, None)
                .await?;
            let asset_info = AssetInfo {
                asset_id,
                name: String::from_utf8(asset_name.0)?,
                symbol: String::from_utf8(asset_symbol.0)?,
                decimals,
                asset_kind,
            };
            log::info!("Retrieved asset data: {:?}", asset_info);
            assets.assets.push(asset_info);
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&self.output)?;
        serde_json::to_writer_pretty(file, &assets)?;
        Ok(())
    }
}
