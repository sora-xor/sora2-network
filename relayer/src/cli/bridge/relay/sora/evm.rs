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
use crate::ethereum::proof_loader::ProofLoader;
use crate::relay::ethereum::Relay;
use crate::relay::ethereum_messages::SubstrateMessagesRelay;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Ethereum DAG cache dir
    #[clap(long)]
    base_path: PathBuf,
    /// Not send messages from Ethereum to Substrate
    #[clap(long)]
    disable_message_relay: bool,
}

impl Command {
    pub async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;
        let chain_id = eth.get_chainid().await?;
        debug!("Eth chain id = {}", chain_id);
        loop {
            let has_light_client = sub
                .storage_fetch(
                    &runtime::storage()
                        .ethereum_light_client()
                        .network_config(&chain_id),
                    (),
                )
                .await?
                .is_some();
            let has_channel = sub
                .storage_fetch(
                    &runtime::storage()
                        .bridge_inbound_channel()
                        .channel_addresses(&chain_id),
                    (),
                )
                .await?
                .is_some();
            if has_channel && has_light_client {
                break;
            }
            debug!(
                "Waiting for bridge to be available. Channel status = {}, light client status = {}",
                has_channel, has_light_client
            );
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        let proof_loader = ProofLoader::new(eth.clone(), self.base_path.clone());
        let relay = Relay::new(sub.clone(), eth.clone(), proof_loader.clone()).await?;
        if self.disable_message_relay {
            relay.run().await?;
        } else {
            let messages_relay = SubstrateMessagesRelay::new(sub, eth, proof_loader).await?;
            tokio::try_join!(relay.run(), messages_relay.run())?;
        }
        Ok(())
    }
}
