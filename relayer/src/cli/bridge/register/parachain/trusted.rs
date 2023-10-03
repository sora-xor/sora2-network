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

use sp_core::{crypto::Ss58Codec, ecdsa};

use crate::cli::prelude::*;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    para: ParachainClient,
    #[clap(long)]
    peers: Vec<String>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = self.sub.get_unsigned_substrate().await?;
        let para = self.para.get_signed_substrate().await?;

        let peers = self
            .peers
            .iter()
            .map(|peer| ecdsa::Public::from_string(peer.as_str()))
            .try_fold(vec![], |mut acc, peer| -> AnyResult<Vec<ecdsa::Public>> {
                acc.push(peer?);
                Ok(acc)
            })?;

        let network_id = sub.constant_fetch_or_default(
            &mainnet_runtime::constants()
                .substrate_bridge_outbound_channel()
                .this_network_id(),
        )?;

        let call = parachain_runtime::runtime_types::sora2_parachain_runtime::RuntimeCall::BridgeDataSigner(
            parachain_runtime::runtime_types::bridge_data_signer::pallet::Call::register_network {
                network_id,
                peers: peers.clone(),
            },
        );
        info!("Submit sudo call: {call:?}");
        let call = parachain_runtime::tx().sudo().sudo(call);
        para.submit_extrinsic(&call).await?;

        let call =
            parachain_runtime::runtime_types::sora2_parachain_runtime::RuntimeCall::MultisigVerifier(
                parachain_runtime::runtime_types::multisig_verifier::pallet::Call::initialize {
                    network_id,
                    peers,
                },
            );
        info!("Submit sudo call: {call:?}");
        let call = parachain_runtime::tx().sudo().sudo(call);
        para.submit_extrinsic(&call).await?;

        Ok(())
    }
}
