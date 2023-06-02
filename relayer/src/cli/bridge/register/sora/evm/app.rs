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

use std::str::FromStr;

use crate::{cli::prelude::*, substrate::AssetId};
use bridge_types::{types::AssetKind, H160, U256};
use common::{AssetName, AssetSymbol, ETH};
use substrate_gen::runtime;

#[derive(Args, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    #[clap(subcommand)]
    apps: Apps,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Apps {
    /// Register ERC20App
    ERC20App {
        /// ERC20App contract address
        #[clap(long)]
        contract: H160,
    },
    /// Register NativeApp
    NativeApp {
        /// SidechainApp contract address
        #[clap(long)]
        contract: H160,
    },
    /// Register EthApp with predefined ETH asset id
    EthAppPredefined {
        /// EthApp contract address
        #[clap(long)]
        contract: H160,
        /// ETH precision
        #[clap(long)]
        precision: u8,
    },
    /// Register EthApp with creating new ETH asset
    EthAppNew {
        /// EthApp contract address
        #[clap(long)]
        contract: H160,
        /// ETH asset name
        #[clap(long)]
        name: String,
        /// ETH asset symbol
        #[clap(long)]
        symbol: String,
        /// ETH precision
        #[clap(long)]
        precision: u8,
    },
    /// Register EthApp with existing ETH asset
    EthAppExisting {
        /// EthApp contract address
        #[clap(long)]
        contract: H160,
        /// ETH asset id
        #[clap(long)]
        asset_id: AssetId,
        /// ETH precision
        #[clap(long)]
        precision: u8,
    },
    /// Register MigrationApp
    MigrationApp {
        /// MigrationApp contract address
        #[clap(long)]
        contract: H160,
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
        let call = match &self.apps {
            Apps::ERC20App { contract } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::ERC20App(
                runtime::runtime_types::erc20_app::pallet::Call::register_erc20_app {
                    network_id,
                    contract: *contract,
                }
            )
            }
            Apps::NativeApp { contract } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::ERC20App(
                runtime::runtime_types::erc20_app::pallet::Call::register_native_app {
                    network_id,
                    contract: *contract,
                }
            )
            }
            Apps::EthAppPredefined { contract, precision } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network_with_existing_asset {
                    network_id,
                    contract: *contract,
                    asset_id: ETH,
                    sidechain_precision: *precision
                }
            )
            }
            Apps::EthAppNew { contract, name, symbol, precision } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network {
                    network_id,
                    contract: *contract,
                    name: AssetName::from_str(name.as_str()).map_err(|err| anyhow!(format!("{}", err)))?,
                    symbol: AssetSymbol::from_str(symbol.as_str()).map_err(|err| anyhow!(format!("{}", err)))?,
                    sidechain_precision: *precision
                }
            )
            }
            Apps::EthAppExisting { contract, asset_id, precision } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::EthApp(
                runtime::runtime_types::eth_app::pallet::Call::register_network_with_existing_asset {
                    network_id,
                    contract: *contract,
                    asset_id: *asset_id,
                    sidechain_precision: *precision
                }
            )
            }
            Apps::MigrationApp { contract } => {
                runtime::runtime_types::framenode_runtime::RuntimeCall::MigrationApp(
                runtime::runtime_types::migration_app::pallet::Call::register_network {
                    network_id,
                    contract: *contract,
                }
            )
            }
        };
        info!("Sudo call extrinsic: {:?}", call);
        sub.submit_extrinsic(&runtime::tx().sudo().sudo(call))
            .await?;
        Ok(())
    }

    async fn check_if_registered(
        &self,
        sub: &SubSignedClient<MainnetConfig>,
        network_id: U256,
    ) -> AnyResult<bool> {
        let (contract, registered) = match self.apps {
            Apps::ERC20App { contract } => {
                let registered = sub
                    .storage_fetch(
                        &mainnet_runtime::storage()
                            .erc20_app()
                            .app_addresses(&network_id, &AssetKind::Sidechain),
                        (),
                    )
                    .await?;
                (contract, registered)
            }
            Apps::NativeApp { contract } => {
                let registered = sub
                    .storage_fetch(
                        &mainnet_runtime::storage()
                            .erc20_app()
                            .app_addresses(&network_id, &AssetKind::Thischain),
                        (),
                    )
                    .await?;
                (contract, registered)
            }
            Apps::EthAppPredefined { contract, .. }
            | Apps::EthAppNew { contract, .. }
            | Apps::EthAppExisting { contract, .. } => {
                let registered = sub
                    .storage_fetch(
                        &mainnet_runtime::storage().eth_app().addresses(&network_id),
                        (),
                    )
                    .await?
                    .map(|(contract, _, _)| contract);
                (contract, registered)
            }
            Apps::MigrationApp { contract } => {
                let registered = sub
                    .storage_fetch(
                        &mainnet_runtime::storage()
                            .migration_app()
                            .addresses(&network_id),
                        (),
                    )
                    .await?;
                (contract, registered)
            }
        };
        if let Some(registered) = registered {
            if registered == contract {
                info!("App already registered");
            } else {
                info!(
                    "App already registered with different contract address: {} != {}",
                    contract, registered
                );
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
