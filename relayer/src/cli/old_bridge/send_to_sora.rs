use crate::cli::prelude::*;
use bridge_types::{H160, H256};
use eth_bridge::offchain::SignatureParams;
use eth_bridge::requests::{CurrencyIdEncoded, OutgoingRequestEncoded};
use ethers::prelude::Middleware;
use futures::StreamExt;
use jsonrpsee::rpc_params;
use subxt::rpc::ClientT;
use subxt::sp_runtime::DispatchResult;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    ethereum: EthereumUrl,
    #[clap(flatten)]
    substrate: SubstrateUrl,
    #[clap(flatten)]
    ethereum_key: EthereumKey,
    #[clap(flatten)]
    substrate_key: SubstrateKey,
    #[clap(short, long)]
    network: u32,
    #[clap(long)]
    hash: H256,
    #[clap(short, long)]
    contract: H160,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.ethereum.get())
            .await?
            .sign_with_string(&self.ethereum_key.get_key_string()?)
            .await
            .context("sign ethereum client")?;
        let url = self.substrate.get();
        let client = {
            let (sender, receiver) = subxt::rpc::WsTransportClientBuilder::default()
                .certificate_store(jsonrpsee::core::client::CertificateStore::WebPki)
                .build(url)
                .await
                .context("connect ws")?;
            let client = subxt::rpc::RpcClientBuilder::default().build(sender, receiver);
            client
        };
        let req: Result<Vec<(OutgoingRequestEncoded, Vec<SignatureParams>)>, DispatchResult> =
            client
                .request(
                    "ethBridge_getApprovedRequests",
                    rpc_params![vec![self.hash], self.network],
                )
                .await?;
        info!("{:?}", req);
        let (req, approvals) = req.unwrap()[0].clone();
        let mut s_vec = vec![];
        let mut v_vec = vec![];
        let mut r_vec = vec![];
        for SignatureParams { s, v, r } in approvals {
            s_vec.push(s);
            v_vec.push(v + 27);
            r_vec.push(r);
        }
        let s = s_vec;
        let r = r_vec;
        let v = v_vec;
        let contract = ethereum_gen::eth_bridge::Bridge::new(self.contract, eth.inner());
        let tx_hash = self.hash.0;
        let call = match req {
            OutgoingRequestEncoded::Transfer(req) => match req.currency_id {
                CurrencyIdEncoded::AssetId(asset_id) => contract.receive_by_sidechain_asset_id(
                    asset_id.0, req.amount, req.to, req.from, tx_hash, v, r, s,
                ),
                CurrencyIdEncoded::TokenAddress(token) => contract
                    .receive_by_ethereum_asset_address(
                        token, req.amount, req.to, req.from, tx_hash, v, r, s,
                    ),
            },
            OutgoingRequestEncoded::AddAsset(req) => contract.add_new_sidechain_token(
                req.name,
                req.symbol,
                req.decimal,
                req.sidechain_asset_id,
                tx_hash,
                v,
                r,
                s,
            ),
            OutgoingRequestEncoded::AddToken(req) => contract.add_eth_native_token(
                req.token_address,
                req.symbol,
                req.name,
                req.decimals,
                tx_hash,
                v,
                r,
                s,
            ),
            OutgoingRequestEncoded::AddPeer(req) => {
                contract.add_peer_by_peer(req.peer_address, tx_hash, v, r, s)
            }
            OutgoingRequestEncoded::RemovePeer(req) => {
                contract.remove_peer_by_peer(req.peer_address, tx_hash, v, r, s)
            }
            OutgoingRequestEncoded::PrepareForMigration(req) => {
                contract.prepare_for_migration(req.this_contract_address, tx_hash, v, r, s)
            }
            OutgoingRequestEncoded::Migrate(req) => contract.shut_down_and_migrate(
                req.this_contract_address,
                tx_hash,
                req.new_contract_address,
                req.erc20_native_tokens,
                v,
                r,
                s,
            ),
        };
        let call = call;
        let mut call = call.from(eth.inner().address());
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
