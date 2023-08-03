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

// TODO #167: fix clippy warnings
#![allow(clippy::all)]

mod cli;
mod ethereum;
mod relay;
mod substrate;
use clap::StructOpt;
use prelude::*;

#[macro_use]
extern crate log;

#[macro_use]
extern crate anyhow;

#[tokio::main]
async fn main() -> AnyResult<()> {
    init_log();
    let cli = cli::Cli::parse();
    debug!("Cli: {:?}", cli);
    cli.run().await.map_err(|e| {
        error!("Relayer returned error: {:?}", e);
        e
    })?;
    Ok(())
}

fn init_log() {
    if std::env::var_os("RUST_LOG").is_none() {
        env_logger::builder().parse_filters("info").init();
    } else {
        env_logger::init();
    }
}

pub mod prelude {
    pub use crate::ethereum::{
        SignedClient as EthSignedClient, UnsignedClient as EthUnsignedClient,
    };
    pub use crate::substrate::runtime::runtime_types as sub_types;
    pub use crate::substrate::traits::{
        ConfigExt, MainnetConfig, ParachainConfig, ReceiverConfig, SenderConfig,
    };
    pub use crate::substrate::types::{mainnet_runtime, parachain_runtime};
    pub use crate::substrate::{
        event_to_string as sub_event_to_string, log_extrinsic_events as sub_log_extrinsic_events,
        SignedClient as SubSignedClient, UnsignedClient as SubUnsignedClient,
    };
    pub use anyhow::{Context, Result as AnyResult};
    pub use codec::{Decode, Encode};
    pub use hex_literal::hex;
    pub use http::Uri;
    pub use serde::{Deserialize, Serialize};
    pub use sp_core::Pair as CryptoPair;
    pub use sp_runtime::traits::Hash;
    pub use sp_runtime::traits::Header as HeaderT;
    pub use substrate_gen::runtime;
    pub use substrate_gen::runtime::runtime_types::framenode_runtime::MultiProof as VerifierMultiProof;
    pub use url::Url;
}
