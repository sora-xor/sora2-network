use crate::cli::prelude::*;
use crate::substrate::{AccountId, AssetId};
use bridge_types::H160;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    eth: EthereumClient,
    #[clap(short, long)]
    contract: H160,
    #[clap(short, long)]
    token: Option<H160>,
    #[clap(short, long)]
    asset_id: Option<AssetId>,
    #[clap(long)]
    approval: bool,
    #[clap(long)]
    mint: bool,
    #[clap(short, long)]
    to: AccountId,
    #[clap(short, long)]
    amount: u128,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = self.eth.get_signed_ethereum().await?;
        let contract = ethereum_gen::eth_bridge::Bridge::new(self.contract, eth.inner());
        let to: &[u8] = self.to.as_ref();
        let to: [u8; 32] = to.to_vec().try_into().unwrap();
        let token = if let Some(asset_id) = self.asset_id {
            contract
                .sidechain_tokens(asset_id.code)
                .legacy()
                .call()
                .await?
        } else if let Some(token) = self.token {
            token
        } else {
            H160::zero()
        };
        let call = if token.is_zero() {
            contract.send_eth_to_sidechain(to).value(self.amount)
        } else {
            if self.mint && self.asset_id.is_none() {
                let test_token = ethereum_gen::test_token::TestToken::new(token, eth.inner());
                let call = test_token
                    .mint(eth.inner().address(), self.amount.into())
                    .legacy();
                let res = call.send().await?.confirmations(1).await?;
                info!("Minted: {:?}", res);
            }
            if self.approval {
                let ierc20 = ethereum_gen::ierc20::IERC20Metadata::new(token, eth.inner());
                let call = ierc20.approve(self.contract, self.amount.into()).legacy();
                let res = call.send().await?.confirmations(1).await?;
                info!("Approved: {:?}", res);
            }
            contract.send_erc20_to_sidechain(to, self.amount.into(), token)
        };
        let mut call = call.legacy().from(eth.inner().address());
        info!("Static call");
        call.call().await?;
        info!("Call: {:?}", call);
        eth.inner()
            .fill_transaction(&mut call.tx, call.block)
            .await?;
        let gas = call.estimate_gas().await?.as_u128();
        info!("Gas: {}", gas);
        info!("Send");
        let pending = call.send().await?;
        info!("Wait for confirmations: {:?}", pending);
        let res = pending.confirmations(1).await?;
        info!("Result: {:?}", res);
        Ok(())
    }
}
