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

use std::time::Duration;

use crate::cli::prelude::*;
use crate::relay::beefy_syncer::BeefySyncer;
use crate::relay::substrate::RelayBuilder;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Send all Beefy commitments
    #[clap(short, long)]
    send_unneeded_commitments: bool,
    /// Not send messages from Substrate to Ethereum
    #[clap(long)]
    disable_message_relay: bool,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_signed_ethereum().await?;
        let sub = self.sub.get_unsigned_substrate().await?;
        let syncer = BeefySyncer::new();
        let network_id = eth.inner().get_chainid().await.context("fetch chain id")?;
        let eth_app = loop {
            let eth_app = sub
                .storage_fetch(&runtime::storage().eth_app().addresses(&network_id), ())
                .await?;
            if let Some((eth_app, _, _)) = eth_app {
                break eth_app;
            }
            debug!("Waiting for bridge to be available");
            tokio::time::sleep(Duration::from_secs(10)).await;
        };
        let eth_app = ethereum_gen::ETHApp::new(eth_app, eth.inner());
        let inbound_channel_address = eth_app
            .inbound()
            .call()
            .await
            .context("fetch outbound channel address")?;
        let channel = ethereum_gen::InboundChannel::new(inbound_channel_address, eth.inner());
        let beefy = channel
            .beefy_light_client()
            .call()
            .await
            .context("fetch beefy light client address")?;
        let relay = RelayBuilder::new()
            .with_substrate_client(sub.clone())
            .with_ethereum_client(eth.clone())
            .with_beefy_contract(beefy)
            .with_syncer(syncer.clone())
            .build()
            .await
            .context("build substrate relay")?;
        EthMetricsCollectorBuilder::default()
            .with_beefy(beefy)
            .with_inbound_channel(inbound_channel_address)
            .build()
            .await?
            .spawn();
        SoraMetricsCollectorBuilder::default()
            .with_client(sub.clone())
            .with_network_id(network_id.into())
            .build()
            .await?
            .spawn();
        if self.disable_message_relay {
            relay.run(!self.send_unneeded_commitments).await?;
        } else {
            let messages_relay = crate::relay::substrate_messages::RelayBuilder::new()
                .with_inbound_channel_contract(inbound_channel_address)
                .with_receiver_client(eth)
                .with_sender_client(sub)
                .with_syncer(syncer)
                .build()
                .await?;
            tokio::try_join!(
                relay.run(!self.send_unneeded_commitments),
                messages_relay.run()
            )?;
        }
        Ok(())
    }
}
