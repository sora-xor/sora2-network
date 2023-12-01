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
use bridge_types::H256;
use futures::StreamExt;
use substrate_gen::SignatureParams;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Bridge network id
    #[clap(short, long)]
    network: u32,
    /// Relay transaction with given hash
    #[clap(long)]
    hash: Option<H256>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = self.sub.get_signed_substrate().await?;
        let eth = self.eth.get_signed_ethereum().await?;
        if let Some(hash) = self.hash {
            self.relay_request(&eth, &sub, hash).await?;
            return Ok(());
        }
        let mut blocks = sub
            .api()
            .blocks()
            .subscribe_finalized()
            .await
            .context("Subscribe")?;
        while let Some(block) = blocks.next().await.transpose().context("Events next")? {
            let events = sub.api().events().at(Some(block.hash())).await?;
            for event in events.find::<runtime::eth_bridge::events::ApprovalsCollected>() {
                if let Ok(event) = event {
                    info!("Recieved event: {:?}", event);
                    self.relay_request(&eth, &sub, event.0).await?;
                }
            }
        }
        Ok(())
    }

    async fn relay_request(
        &self,
        eth: &EthSignedClient,
        sub: &SubSignedClient<MainnetConfig>,
        hash: H256,
    ) -> AnyResult<()> {
        use mainnet_runtime::runtime_types::eth_bridge;
        let contract_address = sub
            .storage_fetch_or_default(
                &runtime::storage()
                    .eth_bridge()
                    .bridge_contract_address(&self.network),
                (),
            )
            .await?;
        let contract = ethereum_gen::Bridge::new(contract_address, eth.inner());
        let request = sub
            .storage_fetch(
                &runtime::storage()
                    .eth_bridge()
                    .requests(&self.network, &hash),
                (),
            )
            .await?
            .expect("Should exists");
        info!("Send request {}: {:?}", hash, request);
        let approvals = sub
            .storage_fetch_or_default(
                &runtime::storage()
                    .eth_bridge()
                    .request_approvals(&self.network, &hash),
                (),
            )
            .await?;

        let mut s_vec = vec![];
        let mut v_vec = vec![];
        let mut r_vec = vec![];
        for SignatureParams { s, v, r } in approvals {
            s_vec.push(s);
            v_vec.push(v + 27);
            r_vec.push(r);
        }
        let s = s_vec;
        let r = r_vec;
        let v = v_vec;

        let (call, kind) = match request {
            eth_bridge::requests::OffchainRequest::Outgoing(request, _) => {
                match request {
                    eth_bridge::requests::OutgoingRequest::PrepareForMigration(_) => {
                        let kind = Some(sub_types::eth_bridge::requests::IncomingTransactionRequestKind::PrepareForMigration);
                        let call = contract.prepare_for_migration(hash.to_fixed_bytes(), v, r, s);
                        (call, kind)
                    }
                    eth_bridge::requests::OutgoingRequest::Migrate(request) => {
                        let kind = Some(sub_types::eth_bridge::requests::IncomingTransactionRequestKind::Migrate);
                        let call = contract.shut_down_and_migrate(
                            hash.to_fixed_bytes(),
                            request.new_contract_address,
                            request.erc20_native_tokens,
                            v,
                            r,
                            s,
                        );
                        (call, kind)
                    }
                    eth_bridge::requests::OutgoingRequest::AddAsset(request) => {
                        let kind = Some(sub_types::eth_bridge::requests::IncomingTransactionRequestKind::AddAsset);
                        let (symbol, name, decimals, ..) = sub
                            .storage_fetch_or_default(
                                &runtime::storage().assets().asset_infos(&request.asset_id),
                                (),
                            )
                            .await?;
                        let call = contract.add_new_sidechain_token(
                            String::from_utf8_lossy(&name.0).to_string(),
                            String::from_utf8_lossy(&symbol.0).to_string(),
                            decimals,
                            request.asset_id.code,
                            hash.to_fixed_bytes(),
                            v,
                            r,
                            s,
                        );
                        (call, kind)
                    }
                    eth_bridge::requests::OutgoingRequest::AddToken(request) => {
                        let kind = None;
                        let call = contract.add_eth_native_token(
                            request.token_address,
                            request.symbol,
                            request.name,
                            request.decimals,
                            hash.to_fixed_bytes(),
                            v,
                            r,
                            s,
                        );
                        (call, kind)
                    }
                    _ => return Ok(()),
                }
            }
            _ => return Ok(()),
        };
        let call = call.legacy();
        info!("Static call");
        call.call().await?;
        eth.save_gas_price(&call, "").await?;
        info!("Send");
        let pending = call.send().await?;
        info!("Wait for confirmations: {:?}", pending);
        let res = pending.confirmations(30).await?;
        info!("Result: {:?}", res);
        if let (Some(kind), Some(tx)) = (kind, res) {
            sub.submit_extrinsic(&runtime::tx().eth_bridge().request_from_sidechain(
                tx.transaction_hash,
                sub_types::eth_bridge::requests::IncomingRequestKind::Transaction(kind),
                self.network,
            ))
            .await?;
        }
        Ok(())
    }
}
