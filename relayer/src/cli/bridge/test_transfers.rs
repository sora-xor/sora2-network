use crate::cli::prelude::*;
use crate::substrate::AssetId;
use std::collections::HashMap;
use substrate_gen::runtime::runtime_types::bridge_types::types::AssetKind;

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
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .erc20_app()
                    .app_addresses(&network_id, &AssetKind::Thischain),
                None,
            )
            .await?
            .unwrap();
        let erc20_app = sub
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .erc20_app()
                    .app_addresses(&network_id, &AssetKind::Sidechain),
                None,
            )
            .await?
            .unwrap();
        let (eth_app, native_asset) = sub
            .api()
            .storage()
            .fetch(&runtime::storage().eth_app().addresses(&network_id), None)
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
                .api()
                .storage()
                .fetch(
                    &runtime::storage()
                        .erc20_app()
                        .asset_kinds(&network_id, &asset),
                    None,
                )
                .await?
                .unwrap();
            let address = sub
                .api()
                .storage()
                .fetch(
                    &runtime::storage()
                        .erc20_app()
                        .token_addresses(&network_id, &asset),
                    None,
                )
                .await?
                .unwrap();
            match asset_kind {
                AssetKind::Thischain => {
                    let acc = sub.account_id();
                    let sub = sub.clone().unsigned().try_sign_with("//Alice").await?;
                    sub.api()
                        .tx()
                        .sign_and_submit_then_watch_default(
                            &runtime::tx().sudo().sudo(
                                sub_types::framenode_runtime::Call::Currencies(
                                    sub_types::orml_currencies::module::Call::update_balance {
                                        who: acc,
                                        currency_id: asset,
                                        amount: 1000000000000000000000,
                                    },
                                ),
                            ),
                            &sub,
                        )
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
