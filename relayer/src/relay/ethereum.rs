use std::time::Duration;

use crate::ethereum::make_header;
use crate::ethereum::proof_loader::ProofLoader;
use crate::prelude::*;
use bridge_types::{network_config::Consensus, EVMChainId};
use ethers::prelude::*;
use substrate_gen::runtime;
use subxt::tx::Signer;

const MAX_HEADER_IMPORTS_WITHOUT_CHECK: u64 = 20;

#[derive(Clone)]
pub struct Relay {
    sub: SubSignedClient,
    eth: EthUnsignedClient,
    proof_loader: ProofLoader,
    chain_id: EVMChainId,
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
            .fetch(
                &runtime::storage()
                    .ethereum_light_client()
                    .network_config(&chain_id),
                None,
            )
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
            .fetch(
                &runtime::storage()
                    .ethereum_light_client()
                    .finalized_block(&self.chain_id),
                None,
            )
            .await?
            .ok_or(anyhow::anyhow!("Network is not registered"))?;

        let latest_block = self
            .eth
            .get_block_number()
            .await
            .context("get block number")?
            .as_u64();

        let mut current = finalized_block.number + 1;
        let mut best = self
            .sub
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .ethereum_light_client()
                    .best_block(&self.chain_id),
                None,
            )
            .await?
            .expect("should exist")
            .0;

        let mut sent = lru::LruCache::new(50000);
        sent.push(finalized_block.hash, ());

        debug!("Latest Ethereum block {}", latest_block);
        loop {
            while best.number + MAX_HEADER_IMPORTS_WITHOUT_CHECK <= current {
                best = self
                    .sub
                    .api()
                    .storage()
                    .fetch(
                        &runtime::storage()
                            .ethereum_light_client()
                            .best_block(&self.chain_id),
                        None,
                    )
                    .await?
                    .expect("should exist")
                    .0;
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            if let Some(block) = self
                .eth
                .get_block(current)
                .await
                .context("get eth block by number")?
            {
                debug!("Import block {}, best block: {}", current, best.number);
                if !sent.contains(&block.parent_hash) {
                    current -= 1;
                    continue;
                }
                sent.push(block.hash.unwrap(), ());
                self.process_block(block)
                    .await
                    .context("send import header transaction")?;
                current += 1;
            } else {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
    }

    async fn process_block(&self, block: Block<H256>) -> AnyResult<()> {
        let nonce = block.nonce.unwrap_or_default();
        let header = make_header(block);
        debug!("Process ethereum header: {:?}", header);
        trace!("Checking if block is already present");
        let has_block = self
            .sub
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .ethereum_light_client()
                    .headers(&self.chain_id, &header.compute_hash()),
                None,
            )
            .await;
        if let Ok(Some(_)) = has_block {
            return Ok(());
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
        let tx = runtime::tx().ethereum_light_client().import_header(
            self.chain_id,
            header.clone(),
            proof.clone(),
            mix_nonce,
            self.sub.public_key(),
            header_signature,
        );
        let tx = self.sub.api().tx().create_unsigned(&tx)?;
        debug!("Sending ethereum header to substrate");
        tx.submit()
            .await
            .context("submit import header extrinsic")?;
        Ok(())
    }
}
