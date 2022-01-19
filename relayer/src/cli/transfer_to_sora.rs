use super::*;
use crate::prelude::*;
use bridge_types::{H160, H256};
use clap::*;
use ethers::prelude::Middleware;

#[derive(Args, Clone, Debug)]
pub(super) struct TransferToSora {
    #[clap(flatten)]
    url: EthereumUrl,
    #[clap(flatten)]
    key: EthereumKey,
    #[clap(long)]
    eth_app: H160,
    #[clap(long, short)]
    recipient: H256,
    #[clap(long, short)]
    amount: u128,
}

impl TransferToSora {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.url.ethereum_url.clone()).await?;
        let key = self.key.get_key_string()?;
        let eth = eth.sign_with_string(&key).await?;
        let balance = eth.get_balance(eth.address(), None).await?;
        info!("{:?} balance: {}", eth.address(), balance);
        let eth_app = ethereum_gen::ETHApp::new(self.eth_app, eth.inner());
        let balance = eth_app.balance().call().await?;
        info!("EthApp balance: {}", balance);
        let mut call = eth_app
            .lock(*self.recipient.as_fixed_bytes(), 1)
            .value(self.amount)
            .legacy();
        eth.inner()
            .fill_transaction(&mut call.tx, call.block)
            .await?;
        debug!("Check {:?}", call);
        call.call().await?;
        debug!("Send");
        let tx = call.send().await?.confirmations(3).await?.unwrap();
        debug!("Tx: {:?}", tx);
        let balance = eth_app.balance().call().await?;
        info!("EthApp balance: {}", balance);
        Ok(())
    }
}
