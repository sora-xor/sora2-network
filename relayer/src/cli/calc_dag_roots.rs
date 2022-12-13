use std::time::Instant;

use super::*;
use crate::ethereum::ethashproof::cache::make_dataset;
use crate::ethereum::ethashproof::mtree::{sha256_hash, ElementData};
use crate::prelude::*;
use bridge_types::ethashproof::calc_seedhash;
use ethash::{get_cache_size, get_full_size, make_cache};
use ethereum_types::H128;
use rayon::prelude::*;
use rayon::slice::ParallelSlice;

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    /// Start epoch for calculation
    #[clap(long, short)]
    start: u64,
    /// Amount of epochs to calculate
    #[clap(long, short)]
    epochs: u64,
    /// Length of epoch
    #[clap(long, short)]
    length: u64,
}

fn calc_dataset_root(epoch: u64, epoch_length: u64) -> H128 {
    let cache_size = get_cache_size(epoch as usize);
    let data_size = get_full_size(epoch as usize);
    let seed = calc_seedhash(epoch_length, epoch);
    debug!(
        "cache_size: {}, data_size: {}, seed: {}, epoch: {}, epoch_length: {}",
        cache_size, data_size, seed, epoch, epoch_length
    );
    let mut cache = vec![0; cache_size];
    let start = Instant::now();
    make_cache(&mut cache, seed);
    let elapsed = start.elapsed();
    debug!("Cache generation completed in {}s", elapsed.as_secs_f64());
    let start = Instant::now();
    let dataset = make_dataset(data_size as usize, &cache);
    let mut hashes = vec![];
    dataset
        .par_chunks(128)
        .map(|chunk| {
            let mut data = [0u8; 128];
            data.copy_from_slice(chunk);
            ElementData::from(data).hash()
        })
        .collect_into_vec(&mut hashes);
    while hashes.len() > 1 {
        let mut new_hashes = vec![];
        hashes
            .par_chunks(2)
            .map(|pair| sha256_hash(pair[0], pair.get(1).cloned().unwrap_or(pair[0])))
            .collect_into_vec(&mut new_hashes);
        hashes = new_hashes;
    }
    let elapsed = start.elapsed();
    debug!(
        "Dataset root computation completed in {}s",
        elapsed.as_secs_f64()
    );
    hashes[0]
}

impl Command {
    pub(super) async fn run(&self) -> AnyResult<()> {
        for epoch in self.start..self.epochs {
            let root = calc_dataset_root(epoch, self.length);
            println!("{:?}", root);
        }
        Ok(())
    }
}
