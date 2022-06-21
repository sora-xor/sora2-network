use std::path::PathBuf;
use std::sync::Arc;

use super::ethashproof::cache::DatasetMerkleTreeCache;
use super::receipt::BlockWithReceipts;
use crate::prelude::*;
use bridge_types::ethashproof::{calc_seedhash, DoubleNodeWithMerkleProof};
use bridge_types::Header;
use ethers::prelude::*;
use futures::stream::FuturesOrdered;
use tokio::sync::Mutex;
use tokio::time::Instant;

pub fn get_verification_indices(
    epoch_length: usize,
    epoch: usize,
    header_hash: H256,
    nonce: U64,
) -> [usize; ethash::ACCESSES] {
    let cache_size = ethash::get_cache_size(epoch);
    let mut cache = vec![0u8; cache_size];
    let seed = calc_seedhash(epoch_length, epoch);
    ethash::make_cache(&mut cache[..], seed);
    let full_size = ethash::get_full_size(epoch);
    ethash::hashimoto_light_indices(header_hash, nonce, full_size, &cache[..])
}

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
        epoch_length: usize,
        header: Header,
        nonce: U64,
    ) -> AnyResult<Vec<DoubleNodeWithMerkleProof>> {
        let mut res = vec![];
        let epoch = header.number as usize / epoch_length;
        let cache = self.get_cache(epoch_length, epoch).await?;
        let start = Instant::now();
        let indexes =
            get_verification_indices(epoch_length, epoch, header.compute_partial_hash(), nonce);
        let mut futures = FuturesOrdered::new();
        for index in indexes {
            futures.push(super::ethashproof::dag_merkle_root::calculate_proof(
                epoch_length,
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

    async fn get_cache(
        &self,
        epoch_length: usize,
        epoch: usize,
    ) -> AnyResult<Arc<DatasetMerkleTreeCache>> {
        let mut lock = self.cache.lock().await;
        if let Some(cache) = lock.get(&epoch).cloned() {
            return Ok(cache);
        }
        let cache = Arc::new(
            DatasetMerkleTreeCache::get_cache(
                self.data_dir(),
                self.cache_dir(),
                epoch_length,
                epoch,
            )
            .await?,
        );
        lock.put(epoch, cache.clone());
        Ok(cache)
    }

    pub async fn receipt_proof(&self, block: H256, tx_id: usize) -> AnyResult<Vec<Vec<u8>>> {
        if let Some(cache) = self.receipts.lock().await.get_mut(&block) {
            return Ok(cache.prove(tx_id)?);
        }
        let mut cache = BlockWithReceipts::load(self.eth.inner(), block).await?;
        let proof = cache.prove(tx_id)?;
        self.receipts.lock().await.put(block, cache);
        Ok(proof)
    }
}
