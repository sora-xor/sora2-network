use crate::cli::prelude::*;
use bridge_types::H256;
use futures::StreamExt;
use substrate_gen::SignatureParams;

#[derive(Args, Clone, Debug)]
pub struct Command {
    #[clap(flatten)]
    sub: SubstrateClient,
    #[clap(flatten)]
    eth: EthereumClient,
    /// Bridge network id
    #[clap(short, long)]
    network: u32,
    /// Relay transaction with given hash
    #[clap(long)]
    hash: Option<H256>,
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        let sub = self.sub.get_signed_substrate().await?;
        let eth = self.eth.get_signed_ethereum().await?;
        if let Some(hash) = self.hash {
            self.relay_request(&eth, &sub, hash).await?;
            return Ok(());
        }
        let mut events = sub
            .api()
            .events()
            .subscribe_finalized()
            .await
            .context("Subscribe")?;
        while let Some(events) = events.next().await.transpose().context("Events next")? {
            for event in events.iter() {
                info!("Recieved event: {:?}", event);
                use sub_runtime::runtime_types::eth_bridge;
                if let Ok(event) = event {
                    match event.event {
                        sub_runtime::Event::EthBridge(
                            eth_bridge::pallet::Event::ApprovalsCollected(hash),
                        ) => self.relay_request(&eth, &sub, hash).await?,
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    async fn relay_request(
        &self,
        eth: &EthSignedClient,
        sub: &SubSignedClient,
        hash: H256,
    ) -> AnyResult<()> {
        use sub_runtime::runtime_types::eth_bridge;
        let contract_address = sub
            .api()
            .storage()
            .eth_bridge()
            .bridge_contract_address(false, &self.network, None)
            .await?;
        let contract = ethereum_gen::eth_bridge::Bridge::new(contract_address, eth.inner());
        let request = sub
            .api()
            .storage()
            .eth_bridge()
            .requests(false, &self.network, &hash, None)
            .await?
            .expect("Should exists");
        info!("Send request {}: {:?}", hash, request);
        let approvals = sub
            .api()
            .storage()
            .eth_bridge()
            .request_approvals(false, &self.network, &hash, None)
            .await?;

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

        let (call, kind) = match request {
            eth_bridge::requests::OffchainRequest::Outgoing(request, _) => {
                match request {
                    eth_bridge::requests::OutgoingRequest::PrepareForMigration(_) => {
                        let kind = Some(sub_types::eth_bridge::requests::IncomingTransactionRequestKind::PrepareForMigration);
                        let call = contract.prepare_for_migration(
                            contract_address,
                            hash.to_fixed_bytes(),
                            v,
                            r,
                            s,
                        );
                        (call, kind)
                    }
                    eth_bridge::requests::OutgoingRequest::Migrate(request) => {
                        let kind = Some(sub_types::eth_bridge::requests::IncomingTransactionRequestKind::Migrate);
                        let call = contract.shut_down_and_migrate(
                            contract_address,
                            hash.to_fixed_bytes(),
                            request.new_contract_address,
                            request.erc20_native_tokens,
                            v,
                            r,
                            s,
                        );
                        (call, kind)
                    }
                    eth_bridge::requests::OutgoingRequest::AddAsset(request) => {
                        let kind = Some(sub_types::eth_bridge::requests::IncomingTransactionRequestKind::AddAsset);
                        let (symbol, name, decimals, ..) = sub
                            .api()
                            .storage()
                            .assets()
                            .asset_infos(false, &request.asset_id, None)
                            .await?;
                        let call = contract.add_new_sidechain_token(
                            String::from_utf8_lossy(&name.0).to_string(),
                            String::from_utf8_lossy(&symbol.0).to_string(),
                            decimals,
                            request.asset_id.code,
                            hash.to_fixed_bytes(),
                            v,
                            r,
                            s,
                        );
                        (call, kind)
                    }
                    eth_bridge::requests::OutgoingRequest::AddToken(request) => {
                        let kind = None;
                        let call = contract.add_eth_native_token(
                            request.token_address,
                            request.symbol,
                            request.name,
                            request.decimals,
                            hash.to_fixed_bytes(),
                            v,
                            r,
                            s,
                        );
                        (call, kind)
                    }
                    _ => return Ok(()),
                }
            }
            _ => return Ok(()),
        };
        let call = call.legacy();
        info!("Static call");
        call.call().await?;
        eth.save_gas_price(&call, "").await?;
        info!("Send");
        let pending = call.send().await?;
        info!("Wait for confirmations: {:?}", pending);
        let res = pending.confirmations(30).await?;
        info!("Result: {:?}", res);
        if let (Some(kind), Some(tx)) = (kind, res) {
            sub.api()
                .tx()
                .eth_bridge()
                .request_from_sidechain(
                    false,
                    tx.transaction_hash,
                    sub_types::eth_bridge::requests::IncomingRequestKind::Transaction(kind),
                    self.network,
                )?
                .sign_and_submit_then_watch_default(sub)
                .await?
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?;
        }
        Ok(())
    }
}
