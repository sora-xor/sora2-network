use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::{prelude::*, relay::justification::BeefyJustification};
use futures::Stream;
use futures::StreamExt;
use sp_beefy::VersionedFinalityProof;
use sp_runtime::traits::UniqueSaturatedInto;

use super::BlockNumber;

const BEEFY_MIN_DELTA: u32 = 8;

pub async fn get_commitment_from_block<T>(
    sub: &SubUnsignedClient<T>,
    block: BlockNumber<T>,
    is_mandatory: bool,
) -> AnyResult<Option<BeefyJustification<T>>>
where
    T: SenderConfig,
{
    trace!("Get commitment from block {:?}", block);
    let block = sub.block(block).await?;
    if let Some(justifications) = block.justifications {
        for (engine, justification) in justifications {
            if &engine == b"BEEF" {
                let commitment = VersionedFinalityProof::decode(&mut justification.as_slice())?;
                let justification =
                    match BeefyJustification::create(sub.clone(), commitment, is_mandatory).await {
                        Ok(justification) => justification,
                        Err(err) => {
                            warn!("failed to create justification: {}", err);
                            continue;
                        }
                    };
                debug!("Justification: {:?}", justification);
                return Ok(Some(justification));
            }
        }
    }

    Ok(None)
}

pub async fn find_mandatory_commitment<T>(
    sub: &SubUnsignedClient<T>,
    vset_id: u64,
) -> AnyResult<Option<BlockNumber<T>>>
where
    T: SenderConfig,
{
    let finalized_head = sub.finalized_head().await?;
    let high = sub.block_number(finalized_head).await?;
    let low: BlockNumber<T> = 1u32.into();
    let storage = T::current_validator_set();
    let block = super::binary_search_first_occurrence(low, high, vset_id, |n| {
        let storage = &storage;
        let sub = &sub;
        async move {
            let vset = sub.storage_fetch_or_default(storage, n).await?;
            Ok(Some(vset.id))
        }
    })
    .await?;
    Ok(block)
}

pub fn mandatory_commitment_stream<T>(
    sub: SubUnsignedClient<T>,
    latest_commitment: Arc<AtomicU64>,
    current_vset_id: u64,
) -> impl Stream<Item = AnyResult<BeefyJustification<T>>> + Unpin
where
    T: SenderConfig + 'static,
{
    Box::pin(
        futures::stream::iter((current_vset_id + 1)..).then(move |i| {
            let sub = sub.clone();
            let latest_commitment = latest_commitment.clone();
            async move {
                loop {
                    let vset = sub.storage_fetch_or_default(&T::current_validator_set(), ()).await?;
                    if vset.id < i {
                        tokio::time::sleep(T::average_block_time()).await;
                        continue;
                    }
                    let Some(block) = find_mandatory_commitment(&sub, i).await? else {
                            tokio::time::sleep(T::average_block_time()).await;
                            continue;
                        };
                    let Some(justification) = get_commitment_from_block(&sub, block, true).await? else {
                            tokio::time::sleep(T::average_block_time()).await;
                            continue;
                        };
                    latest_commitment.store(block.into(), Ordering::Relaxed);
                    return Ok(justification);
                }
            }
        }),
    )
}

pub fn beefy_commitment_stream<T>(
    sub: SubUnsignedClient<T>,
    latest_commitment: Arc<AtomicU64>,
) -> impl Stream<Item = AnyResult<BeefyJustification<T>>> + Unpin
where
    T: SenderConfig + 'static,
{
    let stream = futures::stream::repeat(())
        .then(move |()| {
            let sub = sub.clone();
            let latest_commitment = latest_commitment.clone();
            async move {
                let latest_sent = latest_commitment.load(Ordering::Relaxed);
                let vset_storage = T::current_validator_set();
                let latest_sent_vset = sub
                    .storage_fetch_or_default(&vset_storage, latest_sent)
                    .await?.id;
                let best_vset = sub
                    .storage_fetch_or_default(&vset_storage, ())
                    .await?.id;
                if latest_sent_vset < best_vset {
                    debug!("Waiting for mandatory commitment");
                    tokio::time::sleep(T::average_block_time()).await;
                    return Ok(None);
                }
                let best_block: u64 = sub.block_number(()).await?.into();
                let possible_beefy_block =
                    best_block - ((best_block - latest_sent) % BEEFY_MIN_DELTA as u64);
                for i in 0..3 {
                    let block_to_check =
                        possible_beefy_block.saturating_sub(i * BEEFY_MIN_DELTA as u64);
                    if block_to_check <= latest_sent {
                        break;
                    }
                    let Some(justification) = get_commitment_from_block(&sub, block_to_check.unique_saturated_into(), false).await? else {
                        continue;
                    };
                    latest_commitment.store(block_to_check, Ordering::Relaxed);
                    return Ok(Some(justification));
                }
                tokio::time::sleep(T::average_block_time()).await;
                Ok(None)
            }
        })
        .filter_map(|x| futures::future::ready(x.transpose()));
    Box::pin(stream)
}

pub async fn subscribe_beefy_justifications<T>(
    sub: SubUnsignedClient<T>,
    latest_sent: u64,
) -> AnyResult<impl Stream<Item = AnyResult<BeefyJustification<T>>> + Unpin>
where
    T: SenderConfig + 'static,
{
    let latest_sent_vset = sub
        .storage_fetch_or_default(&T::current_validator_set(), latest_sent)
        .await?
        .id;
    let latest_commitment = if let Some(_justification) =
        get_commitment_from_block(&sub, latest_sent.unique_saturated_into(), false).await?
    {
        latest_sent
    } else {
        debug!("Latest sent commitment not found, searching mandatory commitment");
        let vset_id = sub
            .storage_fetch_or_default(&T::current_validator_set(), ())
            .await?
            .id;
        let mandatory = find_mandatory_commitment(&sub, vset_id)
            .await?
            .expect("mandatory commitment should exist");
        mandatory.into()
    };
    let latest_commitment = Arc::new(AtomicU64::new(latest_commitment));
    let mandatory_stream =
        mandatory_commitment_stream(sub.clone(), latest_commitment.clone(), latest_sent_vset);
    let beefy_stream = beefy_commitment_stream(sub.clone(), latest_commitment);
    // Always check mandatory commitments stream first
    let res = futures::stream::select_with_strategy(mandatory_stream, beefy_stream, |()| {
        futures::stream::PollNext::Left
    });
    Ok(res)
}
