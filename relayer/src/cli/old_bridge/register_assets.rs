use super::AssetInfo;
use crate::cli::prelude::*;
use std::path::PathBuf;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(short, long)]
    input: PathBuf,
    #[clap(short, long)]
    network: u32,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = self.sub.get_signed_substrate().await?;
        let file = std::fs::OpenOptions::new().read(true).open(&self.input)?;
        let infos: Vec<AssetInfo> = serde_json::from_reader(file)?;
        let mut calls = vec![];
        for info in infos {
            if info.kind == "0x01" {
                continue;
            }
            let name = common::AssetName(info.name.as_bytes().to_vec());
            let symbol = common::AssetSymbol(info.symbol.as_bytes().to_vec());
            let call = sub_types::framenode_runtime::Call::Assets(
                sub_types::assets::pallet::Call::register {
                    symbol,
                    name,
                    is_mintable: true,
                    initial_supply: 0,
                    opt_content_src: None,
                    opt_desc: None,
                    is_indivisible: false,
                },
            );
            calls.push(call);
            let call = if info.kind == "0x00" {
                let call = sub_types::framenode_runtime::Call::Sudo(
                    sub_types::pallet_sudo::pallet::Call::sudo {
                        call: Box::new(sub_types::framenode_runtime::Call::EthBridge(
                            sub_types::eth_bridge::pallet::Call::add_asset {
                                asset_id: info.asset_id,
                                network_id: self.network,
                            },
                        )),
                    },
                );
                call
            } else if info.kind == "0x01" {
                let call = sub_types::framenode_runtime::Call::Sudo(
                    sub_types::pallet_sudo::pallet::Call::sudo {
                        call: Box::new(sub_types::framenode_runtime::Call::EthBridge(
                            sub_types::eth_bridge::pallet::Call::add_sidechain_token {
                                network_id: self.network,
                                token_address: info.address.expect("should have address"),
                                symbol: info.symbol.clone(),
                                name: info.name.clone(),
                                decimals: u8::from_str_radix(&info.precision, 10)?,
                            },
                        )),
                    },
                );
                call
            } else {
                continue;
            };
            calls.push(call);
        }

        info!("Send batch");
        sub.load_nonce().await?;
        sub.api()
            .tx()
            .utility()
            .batch(false, calls)?
            .sign_and_submit_then_watch_default(&sub)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        Ok(())
    }
}
