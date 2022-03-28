use std::path::PathBuf;

use super::*;
use crate::prelude::*;
use bridge_types::{H160, H256};
use clap::*;
use ethers::prelude::Middleware;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    url: EthereumUrl,
    #[clap(flatten)]
    key: EthereumKey,
    #[clap(long)]
    eth_app: Option<H160>,
    #[clap(long)]
    sidechain_app: Option<H160>,
    #[clap(long)]
    erc20_app: Option<H160>,
    #[clap(long)]
    token: Option<H160>,
    #[clap(long, short)]
    recipient: H256,
    #[clap(long, short)]
    amount: u128,
    #[clap(long)]
    dry_run: bool,
    #[clap(long)]
    metrics: Option<PathBuf>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.url.get()).await?;
        let key = self.key.get_key_string()?;
        let eth = eth.sign_with_string(&key).await?;
        let balance = eth.get_balance(eth.address(), None).await?;
        info!("ETH {:?} balance: {}", eth.address(), balance);
        if let Some(token_address) = self.token {
            let token = ethereum_gen::IERC20Metadata::new(token_address, eth.inner());
            let balance = token.balance_of(eth.address()).call().await?;
            let name = token.name().call().await?;
            let symbol = token.symbol().call().await?;
            info!("Token {}({}) balance: {}", name, symbol, balance.as_u128());
            if !self.dry_run {
                let mut call = token
                    .approve(
                        self.erc20_app.or(self.sidechain_app).unwrap(),
                        self.amount.into(),
                    )
                    .legacy();
                eth.inner()
                    .fill_transaction(&mut call.tx, call.block)
                    .await?;
                debug!("Check {:?}", call);
                call.call().await?;
                eth.save_gas_price(&call, "transfer-to-sora::mint").await?;
                debug!("Send");
                let tx = call.send().await?.confirmations(3).await?.unwrap();
                debug!("Tx: {:?}", tx);
            }
        }
        let mut call = match (self.eth_app, self.erc20_app, self.sidechain_app, self.token) {
            (Some(eth_app_address), None, None, None) => {
                let eth_app = ethereum_gen::ETHApp::new(eth_app_address, eth.inner());
                let balance = eth_app.balance().call().await?;
                info!("EthApp balance: {}", balance);
                eth_app
                    .lock(*self.recipient.as_fixed_bytes(), 1)
                    .value(self.amount)
            }
            (None, Some(erc20_app_address), None, Some(token_address)) => {
                let erc20_app = ethereum_gen::ERC20App::new(erc20_app_address, eth.inner());
                let registered = erc20_app.tokens(token_address).call().await?;
                if !registered {
                    warn!("Token not registered");
                }
                erc20_app.lock(
                    token_address,
                    *self.recipient.as_fixed_bytes(),
                    self.amount.into(),
                    1,
                )
            }
            (None, None, Some(sidechain_app_address), Some(token_address)) => {
                let sidechain_app =
                    ethereum_gen::SidechainApp::new(sidechain_app_address, eth.inner());
                sidechain_app.lock(
                    token_address,
                    *self.recipient.as_fixed_bytes(),
                    self.amount.into(),
                    1,
                )
            }
            _ => panic!("invalid arguments"),
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
