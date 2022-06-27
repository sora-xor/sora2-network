use crate::prelude::*;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::time::Instant;

use bridge_types::ethashproof::calc_seedhash;
use ethash::{calc_dataset_item, get_cache_size, get_full_size, make_cache, HASH_BYTES};
use ethereum_types::H128;
use serde::{Deserialize, Serialize};

pub const CACHE_LEVEL: u64 = 15;

#[derive(Serialize, Deserialize, Debug)]
pub struct DatasetMerkleTreeCache {
    pub(crate) epoch: u64,
    pub(crate) epoch_length: u64,
    pub(crate) proof_length: u64,
    pub(crate) cache_length: u64,
    pub(crate) root_hash: H128,
    pub(crate) proofs: Vec<Vec<H128>>,
}

impl DatasetMerkleTreeCache {
    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        debug!("Save cache at {:?}", path.as_ref());
        std::fs::create_dir_all(path.as_ref()).context("create dir")?;
        let path = Self::cache_path(path, self.epoch_length, self.epoch);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&path)
            .with_context(|| format!("open file {:?}", path))?;
        serde_json::to_writer(file, self).context("write json file")?;
        Ok(())
    }

    pub fn load(path: impl AsRef<Path>, epoch_length: u64, epoch: u64) -> anyhow::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(Self::cache_path(path, epoch_length, epoch))?;
        let res = serde_json::from_reader(file)?;
        Ok(res)
    }

    fn cache_path(path: impl AsRef<Path>, epoch_length: u64, epoch: u64) -> PathBuf {
        path.as_ref()
            .join(format!("{}-{}.json", epoch, epoch_length))
    }

    pub async fn get_cache(
        data_dir: impl AsRef<Path>,
        cache_dir: impl AsRef<Path>,
        epoch_length: u64,
        epoch: u64,
    ) -> anyhow::Result<Self> {
        let path = Self::cache_path(cache_dir.as_ref(), epoch_length, epoch);
        if !path.exists() {
            super::dag_merkle_root::calculate_dataset_merkle_root(
                epoch_length,
                epoch,
                data_dir,
                cache_dir.as_ref(),
            )
            .await
            .context("calculate dataset")?;
        }
        Self::load(cache_dir, epoch_length, epoch).context("load cache")
    }
}

pub fn make_dag(epoch_length: u64, epoch: u64, dir: impl AsRef<Path>) -> anyhow::Result<()> {
    info!("Make dag for {} epoch in {:?}", epoch, dir.as_ref());
    let path = dag_path(epoch_length, epoch, &dir);
    if path.exists() {
        debug!("DAG path {:?} already exists", path);
        return Ok(());
    }
    let cache_size = get_cache_size(epoch as usize);
    let data_size = get_full_size(epoch as usize);
    let seed = calc_seedhash(epoch_length, epoch);
    debug!(
        "cache_size: {}, data_size: {}, seed: {}",
        cache_size, data_size, seed
    );
    let mut cache = vec![0; cache_size];
    let start = Instant::now();
    make_cache(&mut cache, seed);
    let elapsed = start.elapsed();
    debug!("Cache generation completed in {}s", elapsed.as_secs_f64());
    let start = Instant::now();
    let dataset = make_dataset(data_size as usize, &cache);
    let elapsed = start.elapsed();
    debug!("Dataset generation completed in {}s", elapsed.as_secs_f64());
    std::fs::create_dir_all(dir)?;
    std::fs::write(path, dataset)?;
    Ok(())
}

pub fn dag_path(epoch_length: u64, epoch: u64, dir: impl AsRef<Path>) -> PathBuf {
    let seed = calc_seedhash(epoch_length, epoch);
    dir.as_ref().join(format!(
        "full-R23-{}-{}-{}",
        hex::encode(seed.as_bytes()),
        epoch_length,
        epoch
    ))
}

pub fn make_dataset(full_size: usize, cache: &[u8]) -> Vec<u8> {
    let mut dataset = vec![0u8; full_size];
    let count = AtomicUsize::new(0);
    let percent = full_size / HASH_BYTES / 10;
    dataset
        .par_chunks_mut(HASH_BYTES)
        .enumerate()
        .for_each(|(i, c)| {
            let z = calc_dataset_item(cache, i);
            c.copy_from_slice(z.as_bytes());
            let current = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if current.checked_rem(percent).unwrap_or(0) == 0 {
                debug!("Make dataset: {}%", current / percent * 10);
            }
        });
    dataset
}
