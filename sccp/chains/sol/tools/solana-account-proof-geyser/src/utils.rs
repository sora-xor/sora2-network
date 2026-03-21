use std::collections::HashMap;

use blake3::traits::digest::Digest;
use rayon::prelude::*;
use solana_sdk::hash::{hashv, Hash, Hasher};
use solana_sdk::pubkey::Pubkey;

use crate::types::{AccountDeltaProof, AccountHashMap, Data, Proof};

const MERKLE_FANOUT: usize = 16;

pub fn hash_solana_account(
    lamports: u64,
    owner: &[u8],
    executable: bool,
    rent_epoch: u64,
    data: &[u8],
    pubkey: &[u8],
) -> [u8; 32] {
    if lamports == 0 {
        return [0u8; 32];
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(&lamports.to_le_bytes());
    hasher.update(&rent_epoch.to_le_bytes());
    hasher.update(data);
    hasher.update(&[u8::from(executable)]);
    hasher.update(owner);
    hasher.update(pubkey);
    hasher.finalize().into()
}

pub fn calculate_root_and_proofs(
    pubkey_hash_vec: &mut [(Pubkey, Hash)],
    leaves_for_proof: &[Pubkey],
) -> (Hash, Vec<(Pubkey, Proof)>) {
    pubkey_hash_vec.par_sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let root = compute_merkle_root_loop(pubkey_hash_vec, MERKLE_FANOUT, |entry| &entry.1);
    let proofs = generate_merkle_proofs(pubkey_hash_vec, leaves_for_proof);
    (root, proofs)
}

pub fn generate_merkle_proofs(
    pubkey_hash_vec: &[(Pubkey, Hash)],
    leaves_for_proof: &[Pubkey],
) -> Vec<(Pubkey, Proof)> {
    let mut proofs = Vec::new();

    for &key in leaves_for_proof {
        let Ok(mut pos) = pubkey_hash_vec.binary_search_by(|(candidate, _)| candidate.cmp(&key))
        else {
            continue;
        };

        let mut path = Vec::new();
        let mut siblings = Vec::new();
        let mut current_hashes: Vec<_> = pubkey_hash_vec.iter().map(|(_, hash)| *hash).collect();
        while current_hashes.len() > 1 {
            let chunk_index = pos / MERKLE_FANOUT;
            let index_in_chunk = pos % MERKLE_FANOUT;
            path.push(index_in_chunk);

            let mut sibling_hashes = Vec::with_capacity(MERKLE_FANOUT - 1);
            for offset in 0..MERKLE_FANOUT {
                if offset == index_in_chunk {
                    continue;
                }
                let sibling_pos = chunk_index * MERKLE_FANOUT + offset;
                if sibling_pos < current_hashes.len() {
                    sibling_hashes.push(current_hashes[sibling_pos]);
                }
            }
            siblings.push(sibling_hashes);
            current_hashes = compute_hashes_at_next_level(&current_hashes);
            pos = chunk_index;
        }

        proofs.push((key, Proof { path, siblings }));
    }

    proofs
}

fn compute_hashes_at_next_level(hashes: &[Hash]) -> Vec<Hash> {
    let chunks = div_ceil(hashes.len(), MERKLE_FANOUT);
    (0..chunks)
        .map(|index| {
            let start = index * MERKLE_FANOUT;
            let end = std::cmp::min(start + MERKLE_FANOUT, hashes.len());
            let mut hasher = Hasher::default();
            for hash in &hashes[start..end] {
                hasher.hash(hash.as_ref());
            }
            hasher.result()
        })
        .collect()
}

pub fn compute_merkle_root_loop<T, F>(hashes: &[T], fanout: usize, extractor: F) -> Hash
where
    F: Fn(&T) -> &Hash + Sync,
    T: Sync,
{
    if hashes.is_empty() {
        return Hasher::default().result();
    }

    let total = hashes.len();
    let chunks = div_ceil(total, fanout);
    let result: Vec<_> = (0..chunks)
        .into_par_iter()
        .map(|index| {
            let start = index * fanout;
            let end = std::cmp::min(start + fanout, total);
            let mut hasher = Hasher::default();
            for entry in hashes.iter().take(end).skip(start) {
                hasher.hash(extractor(entry).as_ref());
            }
            hasher.result()
        })
        .collect();

    if result.len() == 1 {
        result[0]
    } else {
        compute_merkle_root_recurse(&result, fanout)
    }
}

pub fn compute_merkle_root_recurse(hashes: &[Hash], fanout: usize) -> Hash {
    compute_merkle_root_loop(hashes, fanout, |hash| hash)
}

pub fn div_ceil(x: usize, y: usize) -> usize {
    let mut result = x / y;
    if x % y != 0 {
        result += 1;
    }
    result
}

pub fn assemble_account_delta_inclusion_proof(
    account_data_hashes: &AccountHashMap,
    account_proofs: &[(Pubkey, Proof)],
    inclusion_pubkeys: &[Pubkey],
) -> anyhow::Result<Vec<AccountDeltaProof>> {
    let account_proofs_map: HashMap<Pubkey, Proof> = account_proofs.iter().cloned().collect();
    let mut proofs = Vec::new();

    for pubkey in inclusion_pubkeys {
        let Some((_, hash, account)) = account_data_hashes.get(pubkey) else {
            continue;
        };
        let Some(proof) = account_proofs_map.get(pubkey) else {
            continue;
        };
        proofs.push(AccountDeltaProof(
            *pubkey,
            (
                Data {
                    pubkey: *pubkey,
                    hash: *hash,
                    account: account.clone(),
                },
                proof.clone(),
            ),
        ));
    }

    Ok(proofs)
}

pub fn compute_bank_hash(
    parent_bankhash: Hash,
    account_delta_root: Hash,
    num_sigs: u64,
    blockhash: Hash,
) -> Hash {
    hashv(&[
        parent_bankhash.as_ref(),
        account_delta_root.as_ref(),
        &num_sigs.to_le_bytes(),
        blockhash.as_ref(),
    ])
}
