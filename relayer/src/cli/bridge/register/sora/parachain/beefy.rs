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

use bridge_types::GenericNetworkId;

use crate::{cli::prelude::*, substrate::BlockNumber};

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    para: ParachainClient,
    #[clap(long)]
    block: Option<BlockNumber<MainnetConfig>>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = self.sub.get_signed_substrate().await?;
        let para = self.para.get_unsigned_substrate().await?;

        let (block_number, block_hash) = if let Some(block) = self.block {
            let hash = para
                .api()
                .rpc()
                .block_hash(Some(block.into()))
                .await?
                .ok_or(anyhow!("Block {} not found on mainnet", block))?;
            (block, hash)
        } else {
            let hash = para.api().rpc().finalized_head().await?;
            let number = para.block_number(hash).await?;
            (number, hash)
        };
        let authorities = para
            .storage_fetch(
                &parachain_runtime::storage().beefy_mmr().beefy_authorities(),
                block_hash,
            )
            .await?
            .ok_or(anyhow!("Beefy authorities not found"))?;
        let next_authorities = para
            .storage_fetch(
                &parachain_runtime::storage()
                    .beefy_mmr()
                    .beefy_next_authorities(),
                block_hash,
            )
            .await?
            .ok_or(anyhow!("Beefy authorities not found"))?;
        let GenericNetworkId::Sub(network_id) = para.constant_fetch_or_default(
            &parachain_runtime::constants()
                .substrate_bridge_outbound_channel()
                .this_network_id().unvalidated(),
        )? else {
            return Err(anyhow!("Network id not found"));
        };

        let call = mainnet_runtime::runtime_types::framenode_runtime::RuntimeCall::BeefyLightClient(
            mainnet_runtime::runtime_types::beefy_light_client::pallet::Call::initialize {
                network_id,
                latest_beefy_block: block_number.into(),
                validator_set: authorities,
                next_validator_set: next_authorities,
            },
        );
        info!("Submit call: {call:?}");
        let call = mainnet_runtime::tx().sudo().sudo(call);
        sub.submit_extrinsic(&call).await?;

        Ok(())
    }
}
