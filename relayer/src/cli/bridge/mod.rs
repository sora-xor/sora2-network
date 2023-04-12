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

mod register_app;
mod register_asset;
mod register_bridge;
mod register_substrate_bridge;
mod relay;
mod reset;
mod test_transfers;
mod transfer_to_ethereum;
mod transfer_to_sora;

use crate::cli::prelude::*;

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Relay operations for bridge
    #[clap(subcommand)]
    Relay(relay::Commands),
    /// Register bridge
    RegisterBridge(register_bridge::Command),
    /// Register bridge app
    RegisterApp(register_app::Command),
    /// Register asset
    RegisterAsset(register_asset::Command),
    /// Make test transfers through bridge
    TestTransfers(test_transfers::Command),
    /// Transfer tokens from Ethereum to Sora
    TransferToSora(transfer_to_sora::Command),
    /// Transfer tokens from Sora to Ethereum
    TransferToEthereum(transfer_to_ethereum::Command),
    /// Reset bridge contracts
    Reset(reset::Command),

    RegisterSubstrateBridge(register_substrate_bridge::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Commands::Relay(cmd) => cmd.run().await,
            Commands::RegisterBridge(cmd) => cmd.run().await,
            Commands::RegisterApp(cmd) => cmd.run().await,
            Commands::RegisterAsset(cmd) => cmd.run().await,
            Commands::TestTransfers(cmd) => cmd.run().await,
            Commands::TransferToSora(cmd) => cmd.run().await,
            Commands::TransferToEthereum(cmd) => cmd.run().await,
            Commands::Reset(cmd) => cmd.run().await,
            Commands::RegisterSubstrateBridge(cmd) => cmd.run().await,
        }
    }
}
