// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use super::AssetInfo;
use crate::cli::prelude::*;
use std::path::PathBuf;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    /// Assets to register
    #[clap(short, long)]
    input: PathBuf,
    /// Bridge network id
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
            let call = sub_types::framenode_runtime::RuntimeCall::Assets(
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
                let call = sub_types::framenode_runtime::RuntimeCall::Sudo(
                    sub_types::pallet_sudo::pallet::Call::sudo {
                        call: Box::new(sub_types::framenode_runtime::RuntimeCall::EthBridge(
                            sub_types::eth_bridge::pallet::Call::add_asset {
                                asset_id: info.asset_id,
                                network_id: self.network,
                            },
                        )),
                    },
                );
                call
            } else if info.kind == "0x01" {
                let call = sub_types::framenode_runtime::RuntimeCall::Sudo(
                    sub_types::pallet_sudo::pallet::Call::sudo {
                        call: Box::new(sub_types::framenode_runtime::RuntimeCall::EthBridge(
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
        sub.submit_extrinsic(&runtime::tx().utility().batch(calls))
            .await?;
        Ok(())
    }
}
