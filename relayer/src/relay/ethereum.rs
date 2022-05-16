use crate::ethereum::make_header;
use crate::ethereum::proof_loader::ProofLoader;
use crate::prelude::*;
use bridge_types::EthNetworkId;
use ethers::prelude::*;
use futures::stream::FuturesOrdered;
use substrate_gen::{runtime, DefaultConfig};
use subxt::TransactionProgress;

#[derive(Clone)]
pub struct Relay {
    sub: SubSignedClient,
    eth: EthUnsignedClient,
    proof_loader: ProofLoader,
    chain_id: EthNetworkId,
}

impl Relay {
    pub async fn new(
        sub: SubSignedClient,
        eth: EthUnsignedClient,
        proof_loader: ProofLoader,
    ) -> AnyResult<Self> {
        let chain_id = eth.get_chainid().await?.as_u32();
        Ok(Self {
            sub,
            eth,
            chain_id,
            proof_loader,
        })
    }

    pub async fn run(&self) -> AnyResult<()> {
        let finalized_block = self
            .sub
            .api()
            .storage()
            .ethereum_light_client()
            .finalized_block(&self.chain_id, None)
            .await?
            .ok_or(anyhow::anyhow!("Network is not registered"))?;

        let latest_block = self
            .eth
            .get_block_number()
            .await
            .context("get block number")?
            .as_u64();

        let mut futures = FuturesOrdered::new();

        let mut current = finalized_block.number + 1;

        debug!("Latest Ethereum block {}", latest_block);
        loop {
            if let Some(block) = self
                .eth
                .get_block(current)
                .await
                .context("get eth block by number")?
            {
                debug!("Preimport block {}", current);
                while futures.len() > 10 {
                    if let Some(result) = futures.next().await {
                        // Rust can't infer type here for some reason
                        let result: Result<(), _> = result;
                        result.context("finalize import header transaction")?;
                    }
                }
                let number = block.number.unwrap_or_default().as_u64();
                let progress = self
                    .process_block(block)
                    .await
                    .context("send import header transaction")?;
                if let Some(progress) = progress {
                    futures.push(self.finalize_transaction(progress, number));
                }
                current += 1;
            } else {
                break;
            }
        }

        let mut watch = self.eth.watch_blocks().await.context("watch blocks")?;
        while let Some(block) = watch.next().await {
            if let Some(block) = self
                .eth
                .get_block(block)
                .await
                .context("get block by hash")?
            {
                debug!("Import block {}", block.number.unwrap_or_default().as_u64());
                while futures.len() > 10 {
                    if let Some(result) = futures.next().await {
                        let result: Result<(), _> = result;
                        result.context("finalize import header transaction")?;
                    }
                }
                let number = block.number.unwrap_or_default().as_u64();
                let progress = self
                    .process_block(block)
                    .await
                    .context("send import header transaction")?;
                if let Some(progress) = progress {
                    futures.push(self.finalize_transaction(progress, number));
                }
            }
        }

        Ok(())
    }

    async fn finalize_transaction<'a>(
        &'a self,
        progress: TransactionProgress<'a, DefaultConfig, runtime::DispatchError, runtime::Event>,
        block_number: u64,
    ) -> AnyResult<()> {
        match progress.wait_for_finalized_success().await {
            Err(
                subxt::Error::Runtime(subxt::RuntimeError(runtime::DispatchError::Module(
                    runtime::runtime_types::sp_runtime::ModuleError { index, error, .. },
                )))
                | subxt::Error::Module(subxt::ModuleError {
                    error_data:
                        subxt::ModuleErrorData {
                            pallet_index: index,
                            error,
                            ..
                        },
                    ..
                }),
            ) if index == 93 && error == 3u32.to_le_bytes() => {
                warn!("DublicateHeader {}", block_number);
                return Ok(());
            }
            Err(subxt::Error::Rpc(subxt::rpc::RpcError::RequestTimeout)) => {
                warn!("Request timeout {}", block_number);
                return Ok(());
            }
            Err(err) => {
                error!(
                    "Failed to import header {}: {}, {:?}",
                    block_number, err, err
                );
                return Err(err.into());
            }
            _ => {}
        };
        Ok(())
    }

    async fn process_block<'a>(
        &'a self,
        block: Block<H256>,
    ) -> AnyResult<
        Option<TransactionProgress<'a, DefaultConfig, runtime::DispatchError, runtime::Event>>,
    > {
        let nonce = block.nonce.unwrap_or_default();
        let header = make_header(block);
        debug!("Process ethereum header: {:?}", header);
        let has_block = self
            .sub
            .api()
            .storage()
            .ethereum_light_client()
            .headers(&self.chain_id, &header.compute_hash(), None)
            .await;
        if let Ok(Some(_)) = has_block {
            return Ok(None);
        }
        let proof = self
            .proof_loader
            .header_proof(header.clone(), nonce)
            .await
            .context("generate header proof")?;
        let result = self
            .sub
            .api()
            .tx()
            .ethereum_light_client()
            .import_header(self.chain_id, header, proof)
            .sign_and_submit_then_watch_default(&self.sub)
            .await
            .context("submit import header extrinsic")?;
        Ok(Some(result))
    }
}
