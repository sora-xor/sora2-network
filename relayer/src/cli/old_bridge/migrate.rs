use std::path::PathBuf;

use super::AssetInfo;
use crate::cli::prelude::*;
use bridge_types::H160;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    sub: SubstrateUrl,
    #[clap(flatten)]
    key: SubstrateKey,
    #[clap(short, long)]
    network: u32,
    #[clap(short, long)]
    contract: H160,
    #[clap(short, long)]
    input: PathBuf,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = SubUnsignedClient::new(self.sub.get())
            .await?
            .try_sign_with(&self.key.get_key_string()?)
            .await?;

        let file = std::fs::OpenOptions::new().read(true).open(&self.input)?;
        let infos: Vec<AssetInfo> = serde_json::from_reader(file)?;
        let mut addresses = vec![];
        for info in infos {
            if info.kind == "0x01" {
                if let Some(address) = info.address {
                    addresses.push(address);
                }
            }
        }

        info!("Send migrate extrinsic");

        sub.api()
            .tx()
            .sudo()
            .sudo(sub_types::framenode_runtime::Call::EthBridge(
                sub_types::eth_bridge::pallet::Call::migrate {
                    new_contract_address: self.contract,
                    erc20_native_tokens: addresses,
                    network_id: self.network,
                },
            ))
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;

        Ok(())
    }
}
