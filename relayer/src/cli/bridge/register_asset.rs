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

use crate::cli::prelude::*;
use bridge_types::{H160, U256};
use common::{AssetId32, AssetName, AssetSymbol, PredefinedAssetId};
use std::str::FromStr;
use substrate_gen::runtime;

#[derive(Args, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    para: ParachainClient,
    #[clap(flatten)]
    eth: EthereumClient,
    #[clap(subcommand)]
    asset_kind: AssetKind,
}

#[derive(Subcommand, Debug)]
pub(crate) enum AssetKind {
    /// Register ERC20 asset with given asset id
    ExistingERC20 {
        /// ERC20 asset id
        #[clap(long)]
        asset_id: AssetId32<PredefinedAssetId>,
        /// ERC20 token address
        #[clap(long)]
        address: H160,
    },
    /// Register ERC20 asset with creating new asset
    ERC20 {
        /// ERC20 token address
        #[clap(long)]
        address: H160,
        /// ERC20 asset name
        #[clap(long)]
        name: String,
        /// ERC20 asset symbol
        #[clap(long)]
        symbol: String,
        /// ERC20 asset decimals
        #[clap(long)]
        decimals: u8,
    },
    /// Register native asset with given asset id
    Native {
        /// Native asset id
        #[clap(long)]
        asset_id: AssetId32<PredefinedAssetId>,
    },
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;
        let network_id = eth.get_chainid().await?;
        if self.check_if_registered(&sub, network_id).await? {
            return Ok(());
        }
        let call = match &self.asset_kind {
            AssetKind::ExistingERC20 { asset_id, address } => {
                runtime::runtime_types::erc20_app::pallet::Call::register_existing_erc20_asset {
                    network_id,
                    asset_id: asset_id.clone(),
                    address: *address,
                }
            }
            AssetKind::ERC20 {
                address,
                name,
                symbol,
                decimals,
            } => runtime::runtime_types::erc20_app::pallet::Call::register_erc20_asset {
                network_id,
                address: address.clone(),
                name: AssetName::from_str(name.as_str()).unwrap(),
                symbol: AssetSymbol::from_str(symbol.as_str()).unwrap(),
                decimals: *decimals,
            },
            AssetKind::Native { asset_id } => {
                runtime::runtime_types::erc20_app::pallet::Call::register_native_asset {
                    network_id,
                    asset_id: asset_id.clone(),
                }
            }
        };
        let call = runtime::runtime_types::framenode_runtime::RuntimeCall::ERC20App(call);
        info!("Sudo call extrinsic: {:?}", call);
        sub.submit_extrinsic(&runtime::tx().sudo().sudo(call))
            .await?;
        Ok(())
    }

    pub async fn check_if_registered(
        &self,
        sub: &SubSignedClient<MainnetConfig>,
        network_id: U256,
    ) -> AnyResult<bool> {
        let is_registered = match &self.asset_kind {
            AssetKind::ExistingERC20 { asset_id, .. } | AssetKind::Native { asset_id } => {
                let is_registered = sub
                    .api()
                    .storage()
                    .fetch(
                        &mainnet_runtime::storage()
                            .erc20_app()
                            .asset_kinds(&network_id, asset_id),
                        None,
                    )
                    .await?
                    .is_some();
                is_registered
            }
            AssetKind::ERC20 { address, .. } => {
                let is_registered = sub
                    .api()
                    .storage()
                    .fetch(
                        &mainnet_runtime::storage()
                            .erc20_app()
                            .assets_by_addresses(&network_id, address),
                        None,
                    )
                    .await?
                    .is_some();
                is_registered
            }
        };
        if is_registered {
            info!("Asset is already registered");
        }
        Ok(is_registered)
    }
}
