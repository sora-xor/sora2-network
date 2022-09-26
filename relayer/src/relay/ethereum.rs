use crate::ethereum::make_header;
use crate::ethereum::proof_loader::ProofLoader;
use crate::prelude::*;
use bridge_types::{network_config::Consensus, EthNetworkId};
use ethers::prelude::*;
use futures::stream::FuturesOrdered;
use substrate_gen::{runtime, DefaultConfig};
use subxt::extrinsic::Signer;
use subxt::sp_runtime::traits::Hash;
use subxt::Config;
use subxt::TransactionProgress;

#[derive(Clone)]
pub struct Relay {
    sub: SubSignedClient,
    eth: EthUnsignedClient,
    proof_loader: ProofLoader,
    chain_id: EthNetworkId,
    consensus: Consensus,
}

impl Relay {
    pub async fn new(
        sub: SubSignedClient,
        eth: EthUnsignedClient,
        proof_loader: ProofLoader,
    ) -> AnyResult<Self> {
        let chain_id = eth.get_chainid().await?;
        let consensus = sub
            .api()
            .storage()
            .ethereum_light_client()
            .network_config(false, &chain_id, None)
            .await?
            .ok_or(anyhow!("Network is not registered"))?
            .consensus();
        Ok(Self {
            sub,
            eth,
            chain_id,
            proof_loader,
            consensus,
        })
    }

    pub async fn run(&self) -> AnyResult<()> {
        let finalized_block = self
            .sub
            .api()
            .storage()
            .ethereum_light_client()
            .finalized_block(false, &self.chain_id, None)
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
                        let result: Result<u64, _> = result;
                        let block_number = result.context("finalize import header transaction")?;
                        debug!("Finalized block {} (pre)import", block_number);
                    }
                }
                let number = block.number.unwrap_or_default().as_u64();
                let progress = self
                    .process_block(block)
                    .await
                    .context("send import header transaction")?;
                if let Some(progress) = progress {
                    futures.push_back(self.finalize_transaction(progress, number));
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
                        let result: Result<u64, _> = result;
                        let block_number = result.context("finalize import header transaction")?;
                        debug!("Finalized block {} (pre)import", block_number);
                    }
                }
                let number = block.number.unwrap_or_default().as_u64();
                let progress = self
                    .process_block(block)
                    .await
                    .context("send import header transaction")?;
                if let Some(progress) = progress {
                    futures.push_back(self.finalize_transaction(progress, number));
                }
            }
        }

        Ok(())
    }

    async fn finalize_transaction<'a, T: Config>(
        &'a self,
        progress: TransactionProgress<'a, T, runtime::DispatchError, runtime::Event>,
        block_number: u64,
    ) -> AnyResult<u64> {
        trace!("Finalizing transaction");
        match progress.wait_for_in_block().await?.wait_for_success().await {
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
                warn!("DuplicateHeader {}", block_number);
                return Ok(block_number);
            }
            Err(subxt::Error::Rpc(subxt::rpc::RpcError::RequestTimeout)) => {
                warn!("Request timeout {}", block_number);
                return Ok(block_number);
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
        Ok(block_number)
    }

    async fn process_block<'a>(
        &'a self,
        block: Block<H256>,
    ) -> AnyResult<
        Option<
            TransactionProgress<
                'a,
                DefaultConfig,
                sub_types::sp_runtime::DispatchError,
                sub_runtime::Event,
            >,
        >,
    > {
        let nonce = block.nonce.unwrap_or_default();
        let header = make_header(block);
        debug!("Process ethereum header: {:?}", header);
        trace!("Checking if block is already present");
        let has_block = self
            .sub
            .api()
            .storage()
            .ethereum_light_client()
            .headers(false, &self.chain_id, &header.compute_hash(), None)
            .await;
        if let Ok(Some(_)) = has_block {
            return Ok(None);
        }
        trace!("Generating header proof");
        let epoch_length = self.consensus.calc_epoch_length(header.number);
        let (proof, mix_nonce) = self
            .proof_loader
            .header_proof(epoch_length, header.clone(), nonce)
            .await
            .context("generate header proof")?;
        trace!("Generated header proof");
        let header_signature = self
            .sub
            .sign(&bridge_types::import_digest(&self.chain_id, &header)[..]);
        let call = sub_types::framenode_runtime::Call::EthereumLightClient(
            runtime::runtime_types::ethereum_light_client::pallet::Call::import_header {
                network_id: self.chain_id,
                header: header.clone(),
                proof: proof.clone(),
                mix_nonce,
                submitter: self.sub.public_key(),
                signature: header_signature,
            },
        );
        let ext_encoded = subxt::Encoded(
            sp_runtime::generic::UncheckedExtrinsic::<(), _, (), ()>::new_unsigned(call.clone())
                .encode(),
        );
        let ext_hash = <DefaultConfig as Config>::Hashing::hash_of(&ext_encoded);
        debug!("Sending ethereum header to substrate");
        let subscription = self
            .sub
            .api()
            .client
            .rpc()
            .watch_extrinsic(ext_encoded)
            .await
            .context("submit import header extrinsic")?;
        let progress = TransactionProgress::new(subscription, &self.sub.api().client, ext_hash);
        Ok(Some(progress))
    }
}
