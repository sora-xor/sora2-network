use crate::prelude::*;
use std::io::SeekFrom;
use std::path::Path;

use ethash::get_full_size;
use ethereum_types::H128;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::cache::{dag_path, DatasetMerkleTreeCache, CACHE_LEVEL};
use super::mtree::{ElementData, MerkleTree};

async fn process_during_read(
    mt: &mut MerkleTree,
    path: impl AsRef<Path>,
    start: u64,
    full_size_128: u32,
    progress: bool,
) -> anyhow::Result<()> {
    let mut f = tokio::fs::OpenOptions::new().read(true).open(path).await?;
    f.seek(SeekFrom::Start(start * 128)).await?;
    let mut percent = -1;
    for i in 0..full_size_128 {
        let mut buf = [0; 128];
        if let Err(e) = f.read_exact(&mut buf).await {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                break;
            }
            return Err(e.into());
        }
        mt.insert(buf.into(), i);
        if progress {
            let new_percent = i as i64 * 100 / full_size_128 as i64;
            if new_percent > percent {
                percent = new_percent;
                debug!("Prepare cache: {}%", percent);
            }
        }
    }
    Ok(())
}

pub async fn calculate_dataset_merkle_root(
    epoch_length: usize,
    epoch: usize,
    data_dir: impl AsRef<Path>,
    cache_dir: impl AsRef<Path>,
) -> anyhow::Result<H128> {
    super::cache::make_dag(epoch_length, epoch, data_dir.as_ref()).context("make dag")?;

    let mut dt = MerkleTree::new();
    let full_size = get_full_size(epoch);
    let full_size_128 = full_size / 128;
    let branch_depth = ((full_size_128 - 1).next_power_of_two() - 1).count_ones();
    let mut indices = vec![];
    for i in 0..(1 << CACHE_LEVEL) {
        let eindex = i << (branch_depth as u64 - CACHE_LEVEL);
        if eindex < full_size_128 {
            indices.push(eindex as u32);
        } else {
            break;
        }
    }
    dt.register_index(indices);
    let path = dag_path(epoch_length, epoch, data_dir.as_ref());
    process_during_read(&mut dt, path, 0, full_size_128 as u32, true)
        .await
        .context("read dataset")?;
    dt.finalize();
    let mut proofs = vec![];
    for proof in dt.proofs_for_ordered_indexes() {
        proofs.push(proof[(branch_depth as usize - CACHE_LEVEL as usize)..].to_vec());
    }
    let cache = DatasetMerkleTreeCache {
        epoch_length: epoch_length as u64,
        epoch: epoch as u64,
        proof_length: branch_depth as u64,
        cache_length: CACHE_LEVEL,
        root_hash: dt.root(),
        proofs,
    };
    cache.save(cache_dir).context("save cache")?;
    Ok(dt.root())
}

pub async fn calculate_proof(
    epoch_length: usize,
    epoch: usize,
    index: u32,
    cache: &DatasetMerkleTreeCache,
    data_dir: impl AsRef<Path>,
) -> anyhow::Result<(ElementData, Vec<H128>)> {
    let mut dt = MerkleTree::new();
    let full_size = get_full_size(epoch);
    let full_size_128 = full_size / 128;
    let branch_depth = ((full_size_128 - 1).next_power_of_two() - 1).count_ones();
    let live_level = branch_depth - CACHE_LEVEL as u32;
    let subtree_start = index >> live_level << live_level;
    dt.register_index(vec![index - subtree_start]);
    let path = dag_path(epoch_length, epoch, data_dir.as_ref());
    process_during_read(
        &mut dt,
        path,
        subtree_start as u64,
        1 << (branch_depth - CACHE_LEVEL as u32),
        false,
    )
    .await?;
    dt.finalize();
    let elem = dt.first_element();
    let mut proof = dt.proofs_for_ordered_indexes()[0].clone();
    proof.extend_from_slice(&cache.proofs[(index >> live_level) as usize]);
    Ok((elem, proof))
}
