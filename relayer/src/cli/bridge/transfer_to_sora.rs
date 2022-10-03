use crate::cli::prelude::*;
use crate::substrate::{AccountId, AssetId};
use substrate_gen::runtime::runtime_types::bridge_types::types::AssetKind;

#[derive(Args, Clone, Debug)]
pub(crate) struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Asset id to transfer
    #[clap(long)]
    asset_id: AssetId,
    /// Recipient account id
    #[clap(long, short)]
    recipient: AccountId,
    /// Amount of tokens to transfer
    #[clap(long, short)]
    amount: u128,
    /// Not send transaction to Ethereum
    #[clap(long)]
    dry_run: bool,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_signed_ethereum().await?;
        let sub = self.sub.get_unsigned_substrate().await?;
        let recipient: [u8; 32] = *self.recipient.as_ref();
        let network_id = eth.get_chainid().await?;
        let (eth_app_address, eth_asset) = sub
            .api()
            .storage()
            .fetch(&runtime::storage().eth_app().addresses(&network_id), None)
            .await?
            .ok_or(anyhow!("Network not registered"))?;
        let balance = eth.get_balance(eth.address(), None).await?;
        info!("ETH {:?} balance: {}", eth.address(), balance);
        let mut call = if self.asset_id == eth_asset {
            info!(
                "Transfer native eth token to {} ({})",
                self.recipient,
                hex::encode(recipient)
            );
            let eth_app = ethereum_gen::ETHApp::new(eth_app_address, eth.inner());
            let balance = eth.get_balance(eth_app_address, None).await?;
            info!("EthApp balance: {}", balance);
            eth_app.lock(recipient).value(self.amount)
        } else {
            let asset_kind = sub
                .api()
                .storage()
                .fetch(
                    &runtime::storage()
                        .erc20_app()
                        .asset_kinds(&network_id, &self.asset_id),
                    None,
                )
                .await?
                .ok_or(anyhow!("Asset is not registered"))?;
            let app_address = sub
                .api()
                .storage()
                .fetch(
                    &runtime::storage()
                        .erc20_app()
                        .app_addresses(&network_id, &asset_kind),
                    None,
                )
                .await?
                .expect("should be registered");
            let token_address = sub
                .api()
                .storage()
                .fetch(
                    &runtime::storage()
                        .erc20_app()
                        .token_addresses(&network_id, &self.asset_id),
                    None,
                )
                .await?
                .expect("should be registered");
            match asset_kind {
                AssetKind::Thischain => {
                    info!("Approve");
                    let token = ethereum_gen::TestToken::new(token_address, eth.inner());
                    let mut call = token.approve(app_address, self.amount.into()).legacy();
                    eth.inner()
                        .fill_transaction(&mut call.tx, call.block)
                        .await?;
                    debug!("Check {:?}", call);
                    call.call().await?;
                    if !self.dry_run {
                        debug!("Send");
                        let tx = call.send().await?.confirmations(1).await?.unwrap();
                        debug!("Tx: {:?}", tx);
                    }
                    info!("Transfer native Sora token");
                    let sidechain_app = ethereum_gen::SidechainApp::new(app_address, eth.inner());
                    sidechain_app.lock(token_address, recipient, self.amount.into())
                }
                AssetKind::Sidechain => {
                    info!("Transfer native ERC20 token");
                    let token = ethereum_gen::TestToken::new(token_address, eth.inner());
                    let balance = token.balance_of(eth.address()).call().await?;
                    let name = token.name().call().await?;
                    let symbol = token.symbol().call().await?;
                    info!("Token {}({}) balance: {}", name, symbol, balance.as_u128());
                    if !self.dry_run {
                        let mut call = token.mint(eth.address(), self.amount.into()).legacy();
                        eth.inner()
                            .fill_transaction(&mut call.tx, call.block)
                            .await?;
                        call.call().await?;
                        call.send().await?.confirmations(1).await?.unwrap();

                        let mut call = token.approve(app_address, self.amount.into()).legacy();
                        eth.inner()
                            .fill_transaction(&mut call.tx, call.block)
                            .await?;
                        debug!("Check {:?}", call);
                        call.call().await?;
                        debug!("Send");
                        let tx = call.send().await?.confirmations(1).await?.unwrap();
                        debug!("Tx: {:?}", tx);
                    }
                    let erc20_app = ethereum_gen::ERC20App::new(app_address, eth.inner());
                    let registered = erc20_app.tokens(token_address).call().await?;
                    if !registered {
                        warn!("Token not registered");
                    }
                    erc20_app.lock(token_address, recipient, self.amount.into())
                }
            }
        }
        .legacy()
        .from(eth.address());
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
