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
use bridge_types::H160;
use ethereum_gen::ValidatorSet;
use ethers::prelude::builders::ContractCall;

#[derive(Args, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// EthApp contract address
    #[clap(long)]
    eth_app: H160,
    #[clap(long)]
    reset_channels: bool,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_signed_ethereum().await?;
        let sub = self.sub.get_unsigned_substrate().await?;
        let eth_app = ethereum_gen::ETHApp::new(self.eth_app.clone(), eth.inner());
        let inbound_channel_address = eth_app.inbound().call().await?;
        let outbound_channel_address = eth_app.outbound().call().await?;
        let inbound_channel =
            ethereum_gen::InboundChannel::new(inbound_channel_address, eth.inner());
        let outbound_channel =
            ethereum_gen::OutboundChannel::new(outbound_channel_address, eth.inner());
        let beefy_address = inbound_channel.beefy_light_client().call().await?;
        let beefy = ethereum_gen::BeefyLightClient::new(beefy_address, eth.inner());
        if beefy.owner().call().await? == eth.address() {
            let block_number = sub.block_number(()).await?;
            let block_hash = sub
                .api()
                .rpc()
                .block_hash(Some(block_number.into()))
                .await?
                .expect("block hash not found");
            let autorities = sub
                .storage_fetch_or_default(
                    &runtime::storage().mmr_leaf().beefy_authorities(),
                    block_hash,
                )
                .await?;
            let next_autorities = sub
                .storage_fetch_or_default(
                    &runtime::storage().mmr_leaf().beefy_next_authorities(),
                    block_hash,
                )
                .await?;
            info!("Reset beefy contract");
            let call: ContractCall<_, _> = beefy.reset(
                block_number as u64,
                ValidatorSet {
                    root: autorities.root.0,
                    length: autorities.len.into(),
                    id: autorities.id.into(),
                },
                ValidatorSet {
                    root: next_autorities.root.0,
                    length: next_autorities.len.into(),
                    id: next_autorities.id.into(),
                },
            );
            let call = call.legacy().from(eth.address());
            debug!("Static call: {:?}", call);
            call.call().await?;
            debug!("Send transaction");
            let pending = call.send().await?;
            debug!("Pending transaction: {:?}", pending);
            let result = pending.await?;
            debug!("Confirmed: {:?}", result);

            if self.reset_channels {
                for call in [inbound_channel.reset(), outbound_channel.reset()] {
                    info!("Reset {:?}", call.tx.to());
                    let call = call.legacy().from(eth.address());
                    debug!("Static call: {:?}", call);
                    call.call().await?;
                    debug!("Send transaction");
                    let pending = call.send().await?;
                    debug!("Pending transaction: {:?}", pending);
                    let result = pending.await?;
                    debug!("Confirmed: {:?}", result);
                }
            }
        }
        Ok(())
    }
}
