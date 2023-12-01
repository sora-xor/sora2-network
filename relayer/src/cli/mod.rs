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

mod bridge;
mod calc_dag_roots;
mod copy_liquidity;
mod error;
mod fetch_ethereum_header;
mod mint_test_token;
mod old_bridge;
mod subscribe_beefy;
pub mod utils;

use std::{net::SocketAddr, path::PathBuf};

pub use utils::*;

use crate::prelude::*;
use clap::*;

/// App struct
#[derive(Parser, Debug)]
#[clap(version, author)]
pub struct Cli {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    para: ParachainClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Substrate account derive URI
    #[clap(long, global = true)]
    substrate_key: Option<String>,
    /// File with Substrate account derive URI
    #[clap(long, global = true)]
    substrate_key_file: Option<String>,
    /// Substrate node endpoint
    #[clap(long, global = true)]
    substrate_url: Option<String>,
    /// Parachain account derive URI
    #[clap(long, global = true)]
    parachain_key: Option<String>,
    /// File with Parachain account derive URI
    #[clap(long, global = true)]
    parachain_key_file: Option<String>,
    /// Parachain node endpoint
    #[clap(long, global = true)]
    parachain_url: Option<String>,
    /// Ethereum private key
    #[clap(long, global = true)]
    ethereum_key: Option<String>,
    /// File with Ethereum private key
    #[clap(long, global = true)]
    ethereum_key_file: Option<String>,
    /// Ethereum node endpoint
    #[clap(long, global = true)]
    ethereum_url: Option<Url>,
    /// Path for gas estimations
    #[clap(long, global = true)]
    gas_metrics_path: Option<PathBuf>,
    #[clap(long, global = true)]
    enable_metrics: bool,
    #[clap(long)]
    prometheus_address: Option<SocketAddr>,
    #[clap(subcommand)]
    commands: Commands,
}

impl Cli {
    pub async fn run(&self) -> AnyResult<()> {
        if self.enable_metrics {
            let mut builder = metrics_exporter_prometheus::PrometheusBuilder::new();
            if let Some(address) = &self.prometheus_address {
                builder = builder.with_http_listener(*address);
            }
            builder.install()?;
            crate::metrics::describe_metrics();
        }
        self.commands.run().await
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Subscribe beefy to new commitments
    SubscribeBeefy(subscribe_beefy::Command),
    /// Fetch Ethereum header
    FetchEthereumHeader(fetch_ethereum_header::Command),
    /// Mint test token (work for tokens with mint method)
    MintTestToken(mint_test_token::Command),
    /// Operations with bridge
    #[clap(subcommand)]
    Bridge(bridge::Commands),
    /// Operations with old bridge
    #[clap(subcommand)]
    OldBridge(old_bridge::Commands),
    /// Calculate DAG roots for light client
    CalcDagRoots(calc_dag_roots::Command),
    CopyLiquidity(copy_liquidity::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Self::SubscribeBeefy(cmd) => cmd.run().await,
            Self::FetchEthereumHeader(cmd) => cmd.run().await,
            Self::MintTestToken(cmd) => cmd.run().await,
            Self::Bridge(cmd) => cmd.run().await,
            Self::OldBridge(cmd) => cmd.run().await,
            Self::CalcDagRoots(cmd) => cmd.run().await,
            Self::CopyLiquidity(cmd) => cmd.run().await,
        }
    }
}

pub mod prelude {
    pub use crate::cli::utils::*;
    pub use crate::prelude::*;
    pub use clap::*;
    pub use ethers::providers::Middleware;
}
