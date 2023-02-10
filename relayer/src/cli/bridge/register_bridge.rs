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
use crate::ethereum::make_header;
use bridge_types::H160;
use substrate_gen::runtime;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Confirmations until block is considered finalized
    #[clap(long, short)]
    descendants_until_final: u64,
    /// InboundChannel contract address
    #[clap(long)]
    inbound_channel: H160,
    /// OutboundChannel contract address
    #[clap(long)]
    outbound_channel: H160,
    #[clap(flatten)]
    network: Network,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;

        let network_id = eth.get_chainid().await?;
        let is_light_client_registered = sub
            .storage_fetch(
                &mainnet_runtime::storage()
                    .ethereum_light_client()
                    .network_config(&network_id),
                (),
            )
            .await?
            .is_some();

        if !is_light_client_registered {
            let network_config = self.network.config()?;
            if network_id != network_config.chain_id() {
                return Err(anyhow!(
                    "Wrong ethereum node chain id, expected {}, actual {}",
                    network_config.chain_id(),
                    network_id
                ));
            }
            let number = eth.get_block_number().await? - self.descendants_until_final;
            let block = eth.get_block(number).await?.expect("block not found");
            let header = make_header(block);
            let call = runtime::runtime_types::framenode_runtime::RuntimeCall::EthereumLightClient(
                runtime::runtime_types::ethereum_light_client::pallet::Call::register_network {
                    header,
                    network_config,
                    initial_difficulty: Default::default(),
                },
            );
            info!("Sudo call extrinsic: {:?}", call);
            sub.submit_extrinsic(&runtime::tx().sudo().sudo(call))
                .await?;
        } else {
            info!("Light client already registered");
        }

        let is_channel_registered = sub
            .storage_fetch(
                &mainnet_runtime::storage()
                    .bridge_inbound_channel()
                    .channel_addresses(&network_id),
                (),
            )
            .await?
            .is_some();
        if !is_channel_registered {
            let call = runtime::runtime_types::framenode_runtime::RuntimeCall::BridgeInboundChannel(
                runtime::runtime_types::bridge_inbound_channel::pallet::Call::register_channel {
                    network_id,
                    inbound_channel: self.inbound_channel,
                    outbound_channel: self.outbound_channel,
                },
            );
            info!("Sudo call extrinsic: {:?}", call);
            sub.submit_extrinsic(&runtime::tx().sudo().sudo(call))
                .await?;
        } else {
            info!("Channel already registered");
        }
        Ok(())
    }
}
