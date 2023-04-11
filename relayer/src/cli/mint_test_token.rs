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

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    /// Token address
    #[clap(long)]
    token: H160,
    /// Amount of tokens to mint
    #[clap(long, short)]
    amount: u128,
    /// Not send transaction to Ethereum
    #[clap(long)]
    dry_run: bool,
    #[clap(flatten)]
    eth: EthereumClient,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_signed_ethereum().await?;
        let token = ethereum_gen::TestToken::new(self.token, eth.inner());
        let balance = token.balance_of(eth.address()).call().await?;
        let name = token.name().call().await?;
        let symbol = token.symbol().call().await?;
        info!(
            "Current token {}({}) balance: {}",
            name,
            symbol,
            balance.as_u128()
        );
        let mut call = token.mint(eth.address(), self.amount.into()).legacy();
        eth.inner()
            .fill_transaction(&mut call.tx, call.block)
            .await?;
        debug!("Check {:?}", call);
        call.call().await?;
        eth.save_gas_price(&call, "mint-test-token").await?;
        if !self.dry_run {
            debug!("Send");
            let tx = call.send().await?.confirmations(3).await?.unwrap();
            debug!("Tx: {:?}", tx);
        }
        Ok(())
    }
}
