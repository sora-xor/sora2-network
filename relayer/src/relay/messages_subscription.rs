use std::sync::atomic::Ordering;
use std::sync::{atomic::AtomicU64, Arc};

use bridge_types::types::{AuxiliaryDigest, AuxiliaryDigestItem};
use bridge_types::{GenericNetworkId, H256};
use common::Balance;
use futures::Stream;
use futures::StreamExt;

use crate::substrate::{binary_search_first_occurence, LeafProof};
use crate::{prelude::*, substrate::BlockNumber};
use bridge_common::simplified_proof::convert_to_simplified_mmr_proof;
use sp_runtime::traits::{Keccak256, UniqueSaturatedInto};

pub enum MessageCommitment {
    EVM(bridge_channel_rpc::Commitment),
    Sub(substrate_bridge_channel_rpc::Commitment<Balance>),
}

impl From<bridge_channel_rpc::Commitment> for MessageCommitment {
    fn from(commitment: bridge_channel_rpc::Commitment) -> Self {
        Self::EVM(commitment)
    }
}

impl From<substrate_bridge_channel_rpc::Commitment<Balance>> for MessageCommitment {
    fn from(commitment: substrate_bridge_channel_rpc::Commitment<Balance>) -> Self {
        Self::Sub(commitment)
    }
}

pub struct MessageCommitmentWithProof<S: SenderConfig> {
    pub block: BlockNumber<S>,
    pub commitment: MessageCommitment,
    pub digest: AuxiliaryDigest,
    pub leaf: bridge_common::beefy_types::BeefyMMRLeaf,
    pub proof: bridge_common::simplified_proof::Proof<H256>,
}

pub async fn load_commitment_with_proof<S: SenderConfig>(
    sender: &SubUnsignedClient<S>,
    network_id: GenericNetworkId,
    block_number: BlockNumber<S>,
    commitment_hash: H256,
    latest_beefy_block: u32,
) -> AnyResult<MessageCommitmentWithProof<S>> {
    let commitment = match network_id {
        GenericNetworkId::EVM(_) => sender.bridge_commitments(commitment_hash).await?.into(),
        GenericNetworkId::Sub(_) => sender
            .substrate_bridge_commitments(commitment_hash)
            .await?
            .into(),
    };
    let block_hash = sender.block_hash(block_number).await?;
    let digest = sender.auxiliary_digest(Some(block_hash)).await?;
    if digest.logs.is_empty() {
        return Err(anyhow!("Digest is empty"));
    }
    let valid_items = digest
        .logs
        .iter()
        .filter(|log| {
            let AuxiliaryDigestItem::Commitment(digest_network_id, digest_commitment_hash) = log;
            if network_id != *digest_network_id && commitment_hash != *digest_commitment_hash {
                false
            } else {
                true
            }
        })
        .count();
    if valid_items != 1 {
        return Err(anyhow!(
            "Expected digest for commitment not found: {:?}",
            digest
        ));
    }
    let digest_hash = Keccak256::hash_of(&digest);
    trace!("Digest hash: {}", digest_hash);
    let leaf_proof = leaf_proof_with_digest(
        sender,
        digest_hash,
        block_number,
        50,
        latest_beefy_block.into(),
    )
    .await?;
    let leaf = leaf_proof.leaf;
    let proof = leaf_proof.proof;
    let parent_hash: [u8; 32] = leaf.parent_number_and_hash.1.as_ref().try_into().unwrap();
    let ready_leaf = bridge_common::beefy_types::BeefyMMRLeaf {
        version: leaf.version,
        parent_number_and_hash: (
            leaf.parent_number_and_hash.0.unique_saturated_into(),
            parent_hash.into(),
        ),
        beefy_next_authority_set: leaf.beefy_next_authority_set,
        leaf_extra: leaf.leaf_extra,
    };
    trace!("Leaf: {:?}", ready_leaf);

    let proof =
        convert_to_simplified_mmr_proof(proof.leaf_indices[0], proof.leaf_count, &proof.items);

    Ok(MessageCommitmentWithProof {
        commitment,
        block: block_number,
        digest,
        leaf: ready_leaf,
        proof,
    })
}

async fn leaf_proof_with_digest<S: SenderConfig>(
    sender: &SubUnsignedClient<S>,
    digest_hash: H256,
    start_leaf: BlockNumber<S>,
    count: u32,
    at: BlockNumber<S>,
) -> AnyResult<LeafProof<S>> {
    for i in 0..count {
        let leaf = start_leaf + i.into();
        let leaf_proof = sender.mmr_generate_proof(leaf, at).await?;
        if leaf_proof.leaf.leaf_extra.digest_hash == digest_hash {
            return Ok(leaf_proof);
        }
    }
    return Err(anyhow::anyhow!("leaf proof not found"));
}

async fn find_message_block<S: SenderConfig>(
    sender: &SubUnsignedClient<S>,
    network_id: GenericNetworkId,
    nonce: u64,
) -> AnyResult<Option<BlockNumber<S>>> {
    let storage = S::bridge_outbound_nonce(network_id);
    let low: BlockNumber<S> = 1u32.into();
    let high = sender.block_number(()).await?;

    trace!(
        "Searching for message with nonce {} in block range {:?}..={:?}",
        nonce,
        low,
        high
    );
    let start_block = binary_search_first_occurence(low, high, nonce, |block| {
        let storage = &storage;
        async move {
            let nonce = sender.storage_fetch(storage, block).await?;
            Ok(nonce)
        }
    })
    .await?;
    Ok(start_block)
}

async fn find_commitment_with_nonce<S: SenderConfig>(
    sender: &SubUnsignedClient<S>,
    network_id: GenericNetworkId,
    count: u32,
    nonce: u64,
) -> AnyResult<Option<(BlockNumber<S>, H256)>> {
    let start_block = find_message_block(sender, network_id, nonce).await?;
    let start_block = if let Some(start_block) = start_block {
        start_block + 1u32.into()
    } else {
        return Ok(None);
    };
    for i in 0..count {
        let block = start_block + i.into();
        let block_hash = sender.block_hash(block).await;
        let Ok(block_hash) = block_hash else {
                return Ok(None);
            };
        let digest = sender.auxiliary_digest(Some(block_hash)).await?;
        if digest.logs.is_empty() {
            continue;
        }
        for log in digest.logs {
            let AuxiliaryDigestItem::Commitment(digest_network_id, commitment_hash) = log;
            if network_id == digest_network_id {
                return Ok(Some((block, commitment_hash)));
            }
        }
    }
    Ok(None)
}

pub fn subscribe_message_commitments<S: SenderConfig>(
    sender: SubUnsignedClient<S>,
    network_id: GenericNetworkId,
    latest_nonce: u64,
) -> impl Stream<Item = AnyResult<(BlockNumber<S>, H256)>> + Unpin {
    let latest_nonce = Arc::new(AtomicU64::new(latest_nonce));
    let stream = futures::stream::repeat(())
        .then(move |_| {
            let latest_nonce = latest_nonce.clone();
            let sender = sender.clone();
            async move {
                let nonce = latest_nonce.load(Ordering::Relaxed) + 1;
                let commitment = find_commitment_with_nonce(&sender, network_id, 100, nonce)
                    .await
                    .map_err(|e| {
                        error!("Failed to find commitment with nonce {}: {}", nonce, e);
                        e
                    })?;
                if let Some((block, _commitment_hash)) = &commitment {
                    let nonce = sender
                        .storage_fetch_or_default(&S::bridge_outbound_nonce(network_id), *block)
                        .await?;
                    latest_nonce.store(nonce, Ordering::Relaxed);
                }
                Ok(commitment)
            }
        })
        .filter_map(|x| async move {
            let x = x.transpose();
            debug!("Found messages: {:?}", x);
            if x.is_none() {
                debug!("Messages not found, waiting for next block...");
                tokio::time::sleep(S::average_block_time()).await;
            }
            x
        });
    Box::pin(stream)
}
