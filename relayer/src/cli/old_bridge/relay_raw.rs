use std::collections::BTreeSet;
use std::time::Duration;

use crate::cli::prelude::*;
use crate::substrate::AccountId;
use bridge_types::{H160, H256};
use eth_bridge::offchain::SignatureParams;
use eth_bridge::requests::{CurrencyIdEncoded, OutgoingRequestEncoded, RequestStatus};
use ethers::abi::Detokenize;
use ethers::prelude::builders::ContractCall;
use ethers::prelude::Middleware;
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
    #[clap(short, long)]
    accounts: Vec<AccountId>,
    #[clap(short, long)]
    network: u32,
    #[clap(short, long)]
    contract: H160,
    #[clap(short, long)]
    xor_master: H160,
    #[clap(short, long)]
    val_master: H160,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let eth = EthUnsignedClient::new(self.ethereum.get())
            .await?
            .sign_with_string(&self.ethereum_key.get_key_string()?)
            .await
            .context("sign ethereum client")?;
        let url = self.substrate.get();
        let sub = {
            let (sender, receiver) = subxt::rpc::WsTransportClientBuilder::default()
                .certificate_store(jsonrpsee::core::client::CertificateStore::WebPki)
                .build(url)
                .await
                .context("connect ws")?;
            let client = subxt::rpc::RpcClientBuilder::default().build(sender, receiver);
            client
        };
        let mut used = BTreeSet::new();
        loop {
            let contract = ethereum_gen::eth_bridge::Bridge::new(self.contract, eth.inner());
            for account in self.accounts.iter() {
                let req: Result<Vec<(u32, H256)>, DispatchResult> = sub
                    .request(
                        "ethBridge_getAccountRequests",
                        rpc_params![account.clone(), RequestStatus::ApprovalsReady],
                    )
                    .await?;
                info!("{:?}", req);
                for (network_id, req_hash) in req.unwrap() {
                    if network_id == self.network {
                        if !used.contains(&req_hash) {
                            let is_used = contract.used(req_hash.0).call().await?;
                            if !is_used {
                                self.process_request(&eth, &sub, req_hash).await?;
                            }
                        }
                        used.insert(req_hash);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(6)).await;
        }
    }

    async fn process_request(
        &self,
        eth: &EthSignedClient,
        sub: &subxt::rpc::RpcClient,
        hash: H256,
    ) -> AnyResult<()> {
        info!("Process request: {}", hash);
        let req: Result<Vec<(OutgoingRequestEncoded, Vec<SignatureParams>)>, DispatchResult> = sub
            .request(
                "ethBridge_getApprovedRequests",
                rpc_params![vec![hash], self.network],
            )
            .await?;
        info!("{:?}", req);
        let (req, approvals) = req.unwrap()[0].clone();
        if approvals.len() < 2 {
            return Ok(());
        }
        let approvals_count = approvals.len();
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
        let tx_hash = hash.0;
        enum TypedCall<A, B> {
            Empty(A, String),
            Bool(B, String),
        }
        let calls = match req {
            OutgoingRequestEncoded::Transfer(req) => vec![match req.currency_id {
                CurrencyIdEncoded::AssetId(asset_id) => TypedCall::Empty(
                    contract.receive_by_sidechain_asset_id(
                        asset_id.0,
                        req.amount.into(),
                        req.to,
                        req.from,
                        tx_hash,
                        v,
                        r,
                        s,
                    ),
                    format!("approvals: {}", approvals_count),
                ),
                CurrencyIdEncoded::TokenAddress(token) => {
                    if token == contract.address_xor().call().await? {
                        let contract =
                            ethereum_gen::master::Master::new(self.xor_master, eth.inner());
                        TypedCall::Empty(
                            contract.mint_tokens_by_peers(
                                token, req.amount, req.to, tx_hash, v, r, s, req.from,
                            ),
                            format!("approvals: {}", approvals_count),
                        )
                    } else if token == contract.address_val().call().await? {
                        let contract =
                            ethereum_gen::master::Master::new(self.val_master, eth.inner());
                        TypedCall::Empty(
                            contract.mint_tokens_by_peers(
                                token, req.amount, req.to, tx_hash, v, r, s, req.from,
                            ),
                            format!("approvals: {}", approvals_count),
                        )
                    } else {
                        TypedCall::Empty(
                            contract.receive_by_ethereum_asset_address(
                                token, req.amount, req.to, req.from, tx_hash, v, r, s,
                            ),
                            format!("approvals: {}", approvals_count),
                        )
                    }
                }
            }],
            OutgoingRequestEncoded::AddAsset(req) => {
                vec![TypedCall::Empty(
                    contract.add_new_sidechain_token(
                        req.name,
                        req.symbol,
                        req.decimal,
                        req.sidechain_asset_id.try_into().unwrap(),
                        tx_hash,
                        v,
                        r,
                        s,
                    ),
                    format!("approvals: {}", approvals_count),
                )]
            }
            OutgoingRequestEncoded::AddToken(req) => {
                vec![TypedCall::Empty(
                    contract.add_eth_native_token(
                        req.token_address,
                        req.symbol,
                        req.name,
                        req.decimals,
                        tx_hash,
                        v,
                        r,
                        s,
                    ),
                    format!("approvals: {}", approvals_count),
                )]
            }
            OutgoingRequestEncoded::AddPeer(req) => {
                let call = contract.add_peer_by_peer(req.peer_address, tx_hash, v, r, s);
                if req.raw.len() > 64 {
                    vec![TypedCall::Bool(
                        call,
                        format!("approvals: {}", approvals_count),
                    )]
                } else {
                    let mut xor_call = call.clone();
                    let mut val_call = call;
                    xor_call.tx.set_to(self.xor_master);
                    val_call.tx.set_to(self.val_master);
                    vec![
                        TypedCall::Bool(xor_call, format!("approvals: {}", approvals_count)),
                        TypedCall::Bool(val_call, format!("approvals: {}", approvals_count)),
                    ]
                }
            }
            OutgoingRequestEncoded::RemovePeer(req) => {
                let call = contract.remove_peer_by_peer(req.peer_address, tx_hash, v, r, s);
                if req.raw.len() > 64 {
                    vec![TypedCall::Bool(
                        call,
                        format!("approvals: {}", approvals_count),
                    )]
                } else {
                    let mut xor_call = call.clone();
                    let mut val_call = call;
                    xor_call.tx.set_to(self.xor_master);
                    val_call.tx.set_to(self.val_master);
                    vec![
                        TypedCall::Bool(xor_call, format!("approvals: {}", approvals_count)),
                        TypedCall::Bool(val_call, format!("approvals: {}", approvals_count)),
                    ]
                }
            }
            OutgoingRequestEncoded::PrepareForMigration(req) => vec![
                TypedCall::Empty(
                    contract.prepare_for_migration(
                        req.this_contract_address,
                        tx_hash,
                        v.clone(),
                        r.clone(),
                        s.clone(),
                    ),
                    format!("approvals: {}", approvals_count),
                ),
                TypedCall::Bool(
                    contract.add_peer_by_peer(req.this_contract_address, tx_hash, v, r, s),
                    format!("approvals: {}", approvals_count),
                ),
            ],
            OutgoingRequestEncoded::Migrate(req) => {
                vec![TypedCall::Empty(
                    contract.shut_down_and_migrate(
                        req.this_contract_address,
                        tx_hash,
                        req.new_contract_address,
                        req.erc20_native_tokens,
                        v,
                        r,
                        s,
                    ),
                    format!("approvals: {}", approvals_count),
                )]
            }
        };
        for call in calls {
            let res = match call {
                TypedCall::Empty(call, additional) => self.send_call(&eth, call, &additional).await,
                TypedCall::Bool(call, additional) => self.send_call(&eth, call, &additional).await,
            };
            if let Err(e) = res {
                error!("Failed to send call: {}", e);
            }
        }
        Ok(())
    }

    async fn send_call<M, D>(
        &self,
        eth: &EthSignedClient,
        call: ContractCall<M, D>,
        additional: &str,
    ) -> AnyResult<()>
    where
        D: Detokenize + core::fmt::Debug,
        M: Middleware + 'static,
    {
        info!("Send {} to {:?}", call.function.name, call.tx.to());
        let mut call = call.from(eth.inner().address()).legacy();
        info!("Static call");
        call.call().await?;
        eth.inner()
            .fill_transaction(&mut call.tx, call.block)
            .await?;
        let gas = call.estimate_gas().await?.as_u128();
        info!("Gas: {}", gas);
        eth.save_gas_price(&call, additional).await?;
        info!("Send");
        let pending = call.send().await?;
        info!("Wait for confirmations: {:?}", pending);
        let res = pending.confirmations(1).await?;
        info!("Result: {:?}", res);
        Ok(())
    }
}
