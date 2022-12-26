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

mod ethereum_relay;
mod parachain_to_parachain_relay;
mod parachain_to_sora_relay;
mod sora_to_parachain_relay;
mod sora_to_sora_relay;
mod substrate_relay;

use crate::cli::prelude::*;
use clap::*;

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Relay Etheerum headers and messages to Sora
    Ethereum(ethereum_relay::Command),
    /// Relay Beefy commitments and bridge messages to Ethereum
    Substrate(substrate_relay::Command),
    /// Relay Beefy commitments and bridge messages from Sora to Parachain
    SoraToParachain(sora_to_parachain_relay::Command),
    /// Relay Beefy commitments and bridge messages from Parachain to Sora
    ParachainToSora(parachain_to_sora_relay::Command),
    /// Relay Beefy commitments and bridge messages from Sora to Sora
    SoraToSora(sora_to_sora_relay::Command),
    /// Relay Beefy commitments and bridge messages from Parachain to Parachain
    ParachainToParachain(parachain_to_parachain_relay::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Commands::Ethereum(cmd) => cmd.run().await,
            Commands::Substrate(cmd) => cmd.run().await,
            Commands::SoraToParachain(cmd) => cmd.run().await,
            Commands::ParachainToSora(cmd) => cmd.run().await,
            Commands::SoraToSora(cmd) => cmd.run().await,
            Commands::ParachainToParachain(cmd) => cmd.run().await,
        }
    }
}
