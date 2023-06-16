use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bridge_types::types::AuxiliaryDigestItem;
use bridge_types::{GenericNetworkId, H256};
use futures::Stream;
use futures::StreamExt;

use crate::substrate::binary_search_first_occurrence;
use crate::{prelude::*, substrate::BlockNumber};

/// Finds the first block where stored nonce >= 'nonce'.
/// - sender - substrate client
/// - network_id - eth network id
/// - from_block - where to start search
/// - nonce - nonce to compare
async fn find_batch_block<S: SenderConfig>(
    sender: &SubUnsignedClient<S>,
    network_id: GenericNetworkId,
    from_block: BlockNumber<S>,
    nonce: u64,
) -> AnyResult<Option<BlockNumber<S>>> {
    let storage = S::bridge_outbound_nonce(network_id);
    let finalized_hash = sender.finalized_head().await?;
    let high = sender.block_number(finalized_hash).await?;

    trace!(
        "Searching for message with nonce {} in block range {:?}..={:?}",
        nonce,
        from_block,
        high
    );
    let start_block = binary_search_first_occurrence(from_block, high, nonce, |block| {
        let storage = &storage;
        async move {
            let nonce = sender.storage_fetch(storage, block).await?;
            Ok(nonce)
        }
    })
    .await?;
    Ok(start_block)
}

/// Finds the first commitment where stored nonce >= 'nonce'.
/// - sender - substrate client
/// - network_id - eth network id
/// - from_block - where to start search
/// - nonce - nonce to compare
async fn find_commitment_with_nonce<S: SenderConfig>(
    sender: &SubUnsignedClient<S>,
    network_id: GenericNetworkId,
    from_block: BlockNumber<S>,
    nonce: u64,
) -> AnyResult<Option<(BlockNumber<S>, H256)>> {
    let block = find_batch_block(sender, network_id, from_block, nonce).await?;
    if let Some(block) = block {
        let block_hash = sender.block_hash(block).await;
        let Ok(block_hash) = block_hash else {
            return Ok(None);
        };
        let digest = sender.auxiliary_digest(Some(block_hash)).await?;
        for log in digest.logs {
            let AuxiliaryDigestItem::Commitment(digest_network_id, commitment_hash) = log;
            if network_id == digest_network_id {
                return Ok(Some((block, commitment_hash)));
            }
        }
    }
    Ok(None)
}

pub fn subscribe_batch_commitments<S: SenderConfig>(
    sender: SubUnsignedClient<S>,
    network_id: GenericNetworkId,
    latest_nonce: u64,
) -> impl Stream<Item = AnyResult<(BlockNumber<S>, H256)>> + Unpin {
    let latest_block = Arc::new(AtomicU64::new(1));
    let latest_nonce = Arc::new(AtomicU64::new(latest_nonce));
    let stream = futures::stream::repeat(())
        .then(move |_| {
            let latest_block = latest_block.clone();
            let latest_nonce = latest_nonce.clone();
            let sender = sender.clone();
            async move {
                let nonce = latest_nonce.load(Ordering::Relaxed) + 1;
                let from_block: BlockNumber<S> =
                    u32::try_from(latest_block.load(Ordering::Relaxed))?.into();
                let commitment = find_commitment_with_nonce(&sender, network_id, from_block, nonce)
                    .await
                    .map_err(|e| {
                        error!(
                            "Failed to find batch commitment with nonce {}: {}",
                            nonce, e
                        );
                        e
                    })?;
                if let Some((block, _commitment_hash)) = &commitment {
                    let nonce = sender
                        .storage_fetch_or_default(&S::bridge_outbound_nonce(network_id), *block)
                        .await?;
                    latest_block.store((*block).into(), Ordering::Relaxed);
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
