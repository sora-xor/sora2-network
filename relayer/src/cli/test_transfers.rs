use std::collections::HashMap;

use super::*;
use crate::prelude::*;
use crate::substrate::AssetId;
use bridge_types::types::ChannelId;
use clap::*;
use ethers::prelude::Middleware;
use substrate_gen::runtime::runtime_types::bridge_types::types::AssetKind;

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    #[clap(flatten)]
    ethereum: EthereumUrl,
    #[clap(flatten)]
    ethereum_key: EthereumKey,
    #[clap(flatten)]
    substrate: SubstrateUrl,
    #[clap(flatten)]
    substrate_key: SubstrateKey,
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
        let eth = EthUnsignedClient::new(self.ethereum.ethereum_url.clone())
            .await?
            .sign_with_string(&self.ethereum_key.get_key_string()?)
            .await?;
        let sub = SubUnsignedClient::new(self.substrate.substrate_url.clone())
            .await?
            .try_sign_with(&self.substrate_key.get_key_string()?)
            .await?;
        let network_id = eth.get_chainid().await?.as_u32();

        let sidechain_app = sub
            .api()
            .storage()
            .erc20_app()
            .app_addresses(&network_id, &AssetKind::Thischain, None)
            .await?
            .unwrap();
        let erc20_app = sub
            .api()
            .storage()
            .erc20_app()
            .app_addresses(&network_id, &AssetKind::Sidechain, None)
            .await?
            .unwrap();
        let (eth_app, native_asset) = sub
            .api()
            .storage()
            .eth_app()
            .addresses(&network_id, None)
            .await?
            .unwrap();

        let mut assets = vec![];
        assets.push((native_asset, None));
        let mut assets_iter = sub
            .api()
            .storage()
            .erc20_app()
            .assets_by_addresses_iter(None)
            .await?;
        while let Some((_, asset)) = assets_iter.next().await? {
            let asset_kind = sub
                .api()
                .storage()
                .erc20_app()
                .asset_kinds(&network_id, &asset, None)
                .await?
                .unwrap();
            let address = sub
                .api()
                .storage()
                .erc20_app()
                .token_addresses(&network_id, &asset, None)
                .await?
                .unwrap();
            match asset_kind {
                AssetKind::Thischain => {
                    let acc = sub.account_id();
                    let sub = sub.clone().unsigned().try_sign_with("//Alice").await?;
                    sub.api()
                        .tx()
                        .sudo()
                        .sudo(sub_types::framenode_runtime::Call::Currencies(
                            sub_types::orml_currencies::module::Call::update_balance {
                                who: acc,
                                currency_id: asset,
                                amount: 1000000000000000000000,
                            },
                        ))
                        .sign_and_submit_then_watch_default(&sub)
                        .await?
                        .wait_for_in_block()
                        .await?
                        .wait_for_success()
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
                            sidechain_app.lock(*address, sub.account_id().into(), 11.into(), 1)
                        }
                        AssetKind::Sidechain => {
                            erc20_app.lock(*address, sub.account_id().into(), 1100.into(), 1)
                        }
                    }
                } else {
                    eth_app.lock(sub.account_id().into(), 1).value(100000)
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
                        .erc20_app()
                        .burn(
                            network_id,
                            ChannelId::Incentivized,
                            *asset,
                            eth.address(),
                            110,
                        )
                        .sign_and_submit_then_watch_default(&sub)
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
                        .eth_app()
                        .burn(network_id, ChannelId::Incentivized, eth.address(), 9)
                        .sign_and_submit_then_watch_default(&sub)
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
