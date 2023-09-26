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
use crate::substrate::traits::KeyPair;
use crate::substrate::AssetId;
use bridge_types::types::AssetKind;
use std::collections::HashMap;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
}

#[derive(Debug, Default)]
struct Stats {
    eth_fail: u64,
    sub_fail: u64,
    eth_succ: u64,
    sub_succ: u64,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let mut stats = HashMap::<AssetId, Stats>::new();
        let eth = self.eth.get_signed_ethereum().await?;
        let sub = self.sub.get_signed_substrate().await?;
        let network_id = eth.get_chainid().await?;

        let sidechain_app = sub
            .storage_fetch(
                &runtime::storage()
                    .erc20_app()
                    .app_addresses(&network_id, &AssetKind::Thischain),
                (),
            )
            .await?
            .unwrap();
        let erc20_app = sub
            .storage_fetch(
                &runtime::storage()
                    .erc20_app()
                    .app_addresses(&network_id, &AssetKind::Sidechain),
                (),
            )
            .await?
            .unwrap();
        let (eth_app, native_asset, _) = sub
            .storage_fetch(&runtime::storage().eth_app().addresses(&network_id), ())
            .await?
            .unwrap();

        let mut assets = vec![];
        assets.push((native_asset, None));
        let mut assets_iter = sub
            .api()
            .storage()
            .iter(
                runtime::storage().erc20_app().assets_by_addresses_root(),
                32,
                None,
            )
            .await?;
        while let Some((_, asset)) = assets_iter.next().await? {
            let asset_kind = sub
                .storage_fetch(
                    &runtime::storage()
                        .erc20_app()
                        .asset_kinds(&network_id, &asset),
                    (),
                )
                .await?
                .unwrap();
            let address = sub
                .storage_fetch(
                    &runtime::storage()
                        .erc20_app()
                        .token_addresses(&network_id, &asset),
                    (),
                )
                .await?
                .unwrap();
            match asset_kind {
                AssetKind::Thischain => {
                    let acc = sub.account_id();
                    let sub = sub
                        .clone()
                        .unsigned()
                        .signed(subxt::tx::PairSigner::new(
                            KeyPair::from_string("//Alice", None).unwrap(),
                        ))
                        .await?;
                    sub.submit_extrinsic(&runtime::tx().sudo().sudo(
                        sub_types::framenode_runtime::RuntimeCall::Assets(
                            sub_types::assets::pallet::Call::force_mint {
                                asset_id: asset,
                                to: acc,
                                amount: 1000000000000000000000,
                            },
                        ),
                    ))
                    .await?;
                    let token = ethereum_gen::IERC20Metadata::new(address, eth.inner());
                    let call = token
                        .approve(sidechain_app, 1000000000000000000000000000u128.into())
                        .legacy();
                    call.call().await?;
                    call.send().await?.confirmations(1).await?;
                }
                AssetKind::Sidechain => {
                    let token = ethereum_gen::TestToken::new(address, eth.inner());
                    let call = token
                        .mint(eth.address(), 1000000000000000000u64.into())
                        .legacy();
                    call.call().await?;
                    call.send().await?.confirmations(1).await?;
                    let call = token
                        .approve(erc20_app, 1000000000000000000000000000u128.into())
                        .legacy();
                    call.call().await?;
                    call.send().await?.confirmations(1).await?;
                }
            }
            assets.push((asset, Some((asset_kind, address))));
        }

        let eth_app = ethereum_gen::ETHApp::new(eth_app, eth.inner());

        let sidechain_app = ethereum_gen::SidechainApp::new(sidechain_app, eth.inner());

        let erc20_app = ethereum_gen::SidechainApp::new(erc20_app, eth.inner());
        loop {
            for (asset, info) in assets.iter() {
                let entry = stats.entry(*asset).or_default();
                let mut call = if let Some((kind, address)) = info {
                    match kind {
                        AssetKind::Thischain => {
                            sidechain_app.lock(*address, sub.account_id().into(), 11u128.into())
                        }
                        AssetKind::Sidechain => {
                            erc20_app.lock(*address, sub.account_id().into(), 1100u128.into())
                        }
                    }
                } else {
                    eth_app.lock(sub.account_id().into()).value(100000u128)
                }
                .legacy();
                let eth_res = eth.fill_transaction(&mut call.tx, call.block).await;
                if eth_res.is_ok() {
                    entry.eth_succ += 1;
                    let res = call.send().await?.confirmations(1).await?.unwrap();
                    debug!("Tx {:?}", res);
                } else {
                    debug!(
                        "Failed to send eth {:?} {}: {:?}",
                        call.tx.to(),
                        call.function.name,
                        eth_res
                    );
                    entry.eth_fail += 1;
                }
                if let Some((_, token)) = info {
                    let token = ethereum_gen::IERC20Metadata::new(*token, eth.inner());
                    info!(
                        "{} balance: {}",
                        asset,
                        token.balance_of(eth.address()).call().await?.as_u128()
                    );
                    let in_block = sub
                        .api()
                        .tx()
                        .sign_and_submit_then_watch_default(
                            &runtime::tx()
                                .erc20_app()
                                .burn(network_id, *asset, eth.address(), 110),
                            &sub,
                        )
                        .await?
                        .wait_for_in_block()
                        .await?;
                    let sub_res = in_block.wait_for_success().await;
                    if sub_res.is_ok() {
                        entry.sub_succ += 1;
                    } else {
                        entry.sub_fail += 1;
                        debug!("Failed to send sub: {:?}", sub_res);
                    }
                } else {
                    info!(
                        "{} balance: {}",
                        asset,
                        eth.get_balance(eth.address(), None).await?.as_u128()
                    );
                    let in_block = sub
                        .api()
                        .tx()
                        .sign_and_submit_then_watch_default(
                            &runtime::tx().eth_app().burn(network_id, eth.address(), 9),
                            &sub,
                        )
                        .await?
                        .wait_for_in_block()
                        .await?;
                    let sub_res = in_block.wait_for_success().await;
                    if sub_res.is_ok() {
                        entry.sub_succ += 1;
                    } else {
                        entry.sub_fail += 1;
                        debug!("Failed to send sub: {:?}", sub_res);
                    }
                }
            }
            info!("{:?}", stats);
        }
    }
}
