use crate::cli::prelude::*;
use bridge_types::H160;
use ethers::prelude::*;

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    #[clap(flatten)]
    url: EthereumUrl,
    #[clap(flatten)]
    key: EthereumKey,
    #[clap(long)]
    sidechain_app: H160,
    #[clap(long)]
    inbound_channel: H160,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.url.get()).await?;
        let key = self.key.get_key_string()?;
        let eth = eth.sign_with_string(&key).await?;
        let sidechain_app = ethereum_gen::SidechainApp::new(self.sidechain_app, eth.inner());
        let mut call = sidechain_app.register_asset(
            "XOR".to_string(),
            "XOR".to_string(),
            hex!("0002000000000000000000000000000000000000000000000000000000000000"),
        );
        // call.tx.set_from(self.inbound_channel);

        info!("Fill");
        eth.inner()
            .fill_transaction(&mut call.tx, call.block)
            .await?;
        info!("Call: {:?}", call);
        call.tx.set_gas(4000000u128);
        call.call().await?;
        info!("Send");
        let res = call.send().await?;
        let res = res.confirmations(3);
        info!("Confirm");
        let res = res.await?;
        println!("{:?}", res);

        let block = eth.inner().get_block(360).await?;
        info!("Block {:?}", block);
        let gas = call.estimate_gas().await?;
        info!("Register asset gas: {}", gas.as_u128());
        Ok(())
    }
}
