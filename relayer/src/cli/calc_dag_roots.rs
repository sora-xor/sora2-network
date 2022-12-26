// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use std::time::Instant;

use super::*;
use crate::ethereum::ethashproof::mtree::{sha256_hash, ElementData};
use crate::{ethereum::ethashproof::cache::make_dataset, prelude::*};
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
