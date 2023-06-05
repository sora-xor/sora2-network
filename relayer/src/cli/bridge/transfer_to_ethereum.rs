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
use assets_rpc::AssetsAPIClient;
use bridge_types::H160;
use common::{AssetId32, PredefinedAssetId};

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Recipient address
    #[clap(short, long)]
    recipient: H160,
    /// Amount of tokens to transfer
    #[clap(short, long)]
    amount: u128,
    /// Asset id to transfer
    #[clap(long)]
    asset_id: AssetId32<PredefinedAssetId>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_unsigned_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;
        let network_id = eth.get_chainid().await?;
        let (_, native_asset_id, _) = sub
            .storage_fetch(&runtime::storage().eth_app().addresses(&network_id), ())
            .await?
            .expect("network not found");
        let balance = sub
            .assets()
            .total_balance(sub.account_id(), self.asset_id, None)
            .await?;
        info!("Current balance: {:?}", balance);
        if self.asset_id == native_asset_id {
            info!(
                "Call eth_app.burn({}, {}, {})",
                network_id, self.recipient, self.amount
            );
            sub.submit_extrinsic(&runtime::tx().eth_app().burn(
                network_id,
                self.recipient,
                self.amount,
            ))
            .await?;
        } else {
            info!(
                "Call erc20_app.burn({}, {}, {}, {})",
                network_id, self.asset_id, self.recipient, self.amount
            );
            sub.submit_extrinsic(&runtime::tx().erc20_app().burn(
                network_id,
                self.asset_id,
                self.recipient,
                self.amount,
            ))
            .await?;
        }
        Ok(())
    }
}
