use std::path::PathBuf;

use super::*;
use crate::prelude::*;
use crate::substrate::{AccountId, AssetId};
use bridge_types::H160;
use clap::*;
use ethers::prelude::Middleware;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    eth: EthereumUrl,
    #[clap(flatten)]
    key: EthereumKey,
    #[clap(flatten)]
    sub: SubstrateUrl,
    #[clap(long)]
    token: Option<H160>,
    #[clap(long)]
    asset_id: Option<AssetId>,
    #[clap(long, short)]
    recipient: AccountId,
    #[clap(long, short)]
    amount: u128,
    #[clap(long)]
    dry_run: bool,
    #[clap(long)]
    metrics: Option<PathBuf>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.eth.get()).await?;
        let key = self.key.get_key_string()?;
        let eth = eth.sign_with_string(&key).await?;
        let balance = eth.get_balance(eth.address(), None).await?;
        let sub = SubUnsignedClient::new(self.sub.get()).await?;
        let network_id = eth.inner().get_chainid().await?.as_u32();
        if let Some(app) = sub
            .api()
            .storage()
            .migration_app()
            .addresses(&network_id, None)
            .await?
        {
            let balance = eth.inner().get_balance(app, None).await?.as_u128();
            info!("Migration balance: {}", balance);
        }
        let (asset_info, app) = match (&self.asset_id, &self.token) {
            (Some(asset_id), None) => {
                let asset_kind = sub
                    .api()
                    .storage()
                    .erc20_app()
                    .asset_kinds(&network_id, &asset_id, None)
                    .await?
                    .expect("asset not registered");
                let address = sub
                    .api()
                    .storage()
                    .erc20_app()
                    .token_addresses(&network_id, &asset_id, None)
                    .await?
                    .expect("asset not registered");
                let app = sub
                    .api()
                    .storage()
                    .erc20_app()
                    .app_addresses(&network_id, &asset_kind, None)
                    .await?
                    .expect("App not registered");
                (Some((asset_kind, address)), app)
            }
            (None, Some(token)) => {
                let asset_id = sub
                    .api()
                    .storage()
                    .erc20_app()
                    .assets_by_addresses(&network_id, token, None)
                    .await?
                    .expect("asset not registered");
                let asset_kind = sub
                    .api()
                    .storage()
                    .erc20_app()
                    .asset_kinds(&network_id, &asset_id, None)
                    .await?
                    .expect("asset not registered");
                let app = sub
                    .api()
                    .storage()
                    .erc20_app()
                    .app_addresses(&network_id, &asset_kind, None)
                    .await?
                    .expect("App not registered");
                (Some((asset_kind, *token)), app)
            }
            (None, None) => {
                let app = sub
                    .api()
                    .storage()
                    .eth_app()
                    .addresses(&network_id, None)
                    .await?
                    .expect("App not registered")
                    .0;
                (None, app)
            }
            _ => unimplemented!(),
        };
        info!("ETH {:?} balance: {}", eth.address(), balance);
        let mut call = if let Some((kind, token_address)) = asset_info {
            let token = ethereum_gen::TestToken::new(token_address, eth.inner());
            let balance = token.balance_of(eth.address()).call().await?;
            let name = token.name().call().await?;
            let symbol = token.symbol().call().await?;
            info!("Token {}({}) balance: {}", name, symbol, balance.as_u128());
            let balance = token.balance_of(app).call().await?;
            info!(
                "Token {}({}) app balance: {}",
                name,
                symbol,
                balance.as_u128()
            );
            if !self.dry_run {
                let mut call = token.mint(eth.address(), self.amount.into()).legacy();
                eth.inner()
                    .fill_transaction(&mut call.tx, call.block)
                    .await?;
                call.call().await?;
                call.send().await?.confirmations(1).await?.unwrap();

                let mut call = token.approve(app, self.amount.into()).legacy();
                eth.inner()
                    .fill_transaction(&mut call.tx, call.block)
                    .await?;
                debug!("Check {:?}", call);
                call.call().await?;
                eth.save_gas_price(&call, "transfer-to-sora::mint").await?;
                debug!("Send");
                let tx = call.send().await?.confirmations(1).await?.unwrap();
                debug!("Tx: {:?}", tx);
            }
            match kind {
                sub_types::bridge_types::types::AssetKind::Thischain => {
                    ethereum_gen::SidechainApp::new(app, eth.inner()).lock(
                        token_address,
                        *self.recipient.as_ref(),
                        self.amount.into(),
                        1,
                    )
                }
                sub_types::bridge_types::types::AssetKind::Sidechain => {
                    ethereum_gen::ERC20App::new(app, eth.inner()).lock(
                        token_address,
                        *self.recipient.as_ref(),
                        self.amount.into(),
                        1,
                    )
                }
            }
        } else {
            let balance = eth.inner().get_balance(app, None).await?.as_u128();
            let app = ethereum_gen::ETHApp::new(app, eth.inner());
            info!("EthApp balance: {}", balance);
            app.lock(*self.recipient.as_ref(), 1).value(self.amount)
        }
        .legacy();
        eth.inner()
            .fill_transaction(&mut call.tx, call.block)
            .await?;
        debug!("Check {:?}", call);
        call.call().await?;
        eth.save_gas_price(&call, "transfer-to-sora::transfer")
            .await?;
        if !self.dry_run {
            debug!("Send");
            let tx = call.send().await?.confirmations(3).await?.unwrap();
            debug!("Tx: {:?}", tx);
        }
        Ok(())
    }
}
