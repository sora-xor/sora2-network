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
use crate::substrate::{AccountId, AssetId};
use bridge_types::H160;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    eth: EthereumClient,
    /// Bridge contract address
    #[clap(short, long)]
    contract: H160,
    /// Token address to transfer
    #[clap(short, long)]
    token: Option<H160>,
    /// Asset id to transfer
    #[clap(short, long)]
    asset_id: Option<AssetId>,
    /// Approve ERC20 token transfer
    #[clap(long)]
    approval: bool,
    /// Mint ERC20 token
    #[clap(long)]
    mint: bool,
    /// Recipient account id
    #[clap(short, long)]
    to: AccountId<MainnetConfig>,
    /// Amount of tokens to transfer
    #[clap(short, long)]
    amount: u128,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_signed_ethereum().await?;
        let contract = ethereum_gen::Bridge::new(self.contract, eth.inner());
        let to: &[u8] = self.to.as_ref();
        let to: [u8; 32] = to.to_vec().try_into().unwrap();
        let token = if let Some(asset_id) = self.asset_id {
            contract
                .sidechain_tokens(asset_id.code)
                .legacy()
                .call()
                .await?
        } else if let Some(token) = self.token {
            token
        } else {
            H160::zero()
        };
        let call = if token.is_zero() {
            contract.send_eth_to_sidechain(to).value(self.amount)
        } else {
            if self.mint && self.asset_id.is_none() {
                let test_token = ethereum_gen::test_token::TestToken::new(token, eth.inner());
                let call = test_token
                    .mint(eth.inner().address(), self.amount.into())
                    .legacy();
                let res = call.send().await?.confirmations(1).await?;
                info!("Minted: {:?}", res);
            }
            if self.approval {
                let ierc20 = ethereum_gen::IERC20Metadata::new(token, eth.inner());
                let call = ierc20.approve(self.contract, self.amount.into()).legacy();
                let res = call.send().await?.confirmations(1).await?;
                info!("Approved: {:?}", res);
            }
            contract.send_erc20_to_sidechain(to, self.amount.into(), token)
        };
        let mut call = call.legacy().from(eth.inner().address());
        info!("Static call");
        call.call().await?;
        info!("Call: {:?}", call);
        eth.inner()
            .fill_transaction(&mut call.tx, call.block)
            .await?;
        let gas = call.estimate_gas().await?.as_u128();
        info!("Gas: {}", gas);
        info!("Send");
        let pending = call.send().await?;
        info!("Wait for confirmations: {:?}", pending);
        let res = pending.confirmations(1).await?;
        info!("Result: {:?}", res);
        Ok(())
    }
}
