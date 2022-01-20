use std::path::PathBuf;
use std::sync::Arc;

use super::ethashproof::cache::DatasetMerkleTreeCache;
use super::receipt::BlockWithReceipts;
use super::EPOCH_LENGTH;
use crate::prelude::*;
use bridge_types::ethashproof::DoubleNodeWithMerkleProof;
use bridge_types::Header;
use ethers::prelude::*;
use futures::stream::FuturesOrdered;
use tokio::sync::Mutex;
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct ProofLoader {
    base_dir: PathBuf,
    cache: Arc<Mutex<lru::LruCache<usize, Arc<DatasetMerkleTreeCache>>>>,
    receipts: Arc<Mutex<lru::LruCache<H256, BlockWithReceipts>>>,
    eth: EthUnsignedClient,
}

impl ProofLoader {
    pub fn new(eth: EthUnsignedClient, base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            cache: Arc::new(Mutex::new(lru::LruCache::new(2))),
            receipts: Arc::new(Mutex::new(lru::LruCache::new(10))),
            eth,
        }
    }

    fn cache_dir(&self) -> PathBuf {
        self.base_dir.join("cache")
    }

    fn data_dir(&self) -> PathBuf {
        self.base_dir.join("data")
    }

    pub async fn header_proof(
        &self,
        header: Header,
        nonce: U64,
    ) -> AnyResult<Vec<DoubleNodeWithMerkleProof>> {
        let mut res = vec![];
        let epoch = (header.number / EPOCH_LENGTH) as usize;
        let cache = self.get_cache(epoch).await?;
        let start = Instant::now();
        let indexes = ethash::get_verification_indices(epoch, header.compute_partial_hash(), nonce);
        let mut futures = FuturesOrdered::new();
        for index in indexes {
            futures.push(super::ethashproof::dag_merkle_root::calculate_proof(
                epoch,
                index as u32,
                &cache,
                self.data_dir(),
            ));
        }
        for proof in futures.collect::<Vec<_>>().await {
            let (element, proof) = proof.context("calculate proof")?;
            res.push(DoubleNodeWithMerkleProof {
                dag_nodes: element.to_h512_pair(),
                proof,
            });
        }
        debug!("Calculate proofs: {}s", start.elapsed().as_secs_f64());
        Ok(res)
    }

    async fn get_cache(&self, epoch: usize) -> AnyResult<Arc<DatasetMerkleTreeCache>> {
        let mut lock = self.cache.lock().await;
        if let Some(cache) = lock.get(&epoch).cloned() {
            return Ok(cache);
        }
        let cache = Arc::new(
            DatasetMerkleTreeCache::get_cache(self.data_dir(), self.cache_dir(), epoch).await?,
        );
        lock.put(epoch, cache.clone());
        Ok(cache)
    }

    pub async fn receipt_proof(&self, block: H256, tx_id: usize) -> AnyResult<Vec<Vec<u8>>> {
        let mut lock = self.receipts.lock().await;
        if let Some(cache) = lock.get_mut(&block) {
            return Ok(cache.prove(tx_id)?);
        }
        let mut cache = BlockWithReceipts::load(self.eth.inner(), block).await?;
        let proof = cache.prove(tx_id)?;
        lock.put(block, cache);
        Ok(proof)
    }
}
