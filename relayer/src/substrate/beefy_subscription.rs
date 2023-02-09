use crate::relay::beefy_syncer::BeefySyncer;
use crate::{prelude::*, relay::justification::BeefyJustification};
use beefy_primitives::VersionedFinalityProof;
use futures::Stream;
use futures::StreamExt;
use sp_runtime::traits::{Header as HeaderT, UniqueSaturatedInto};

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
    let hash = sub.block_hash(Some(block)).await?;
    let block = sub
        .api()
        .rpc()
        .block(Some(hash.into()))
        .await?
        .expect("block should exist");
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
    let finalized_head = sub.api().rpc().finalized_head().await?;
    let high = BlockNumber::<T>::from(
        sub.api()
            .rpc()
            .header(Some(finalized_head))
            .await?
            .expect("finalized head must exist")
            .number()
            .clone(),
    );
    let low: BlockNumber<T> = 1u32.into();
    let storage = T::current_validator_set();
    let block = super::binary_search_first_occurence(low, high, vset_id, |n| {
        let storage = &storage;
        let sub = &sub;
        async move {
            let hash = sub.block_hash(Some(n)).await?;
            let vset = sub
                .api()
                .storage()
                .fetch_or_default(storage, Some(hash.into()))
                .await?;
            Ok(Some(vset.id))
        }
    })
    .await?;
    Ok(block)
}

pub fn mandatory_commitment_stream<T>(
    sub: SubUnsignedClient<T>,
    current_vset_id: u64,
) -> impl Stream<Item = AnyResult<BeefyJustification<T>>> + Unpin
where
    T: SenderConfig + 'static,
{
    Box::pin(
        futures::stream::iter((current_vset_id + 1)..).then(move |i| {
            let sub = sub.clone();
            async move {
                loop {
                    let storage = T::current_validator_set();
                    let vset = sub.api().storage().fetch_or_default(&storage, None).await?;
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
                    return Ok(justification);
                }
            }
        }),
    )
}

pub fn beefy_commitment_stream<T>(
    sub: SubUnsignedClient<T>,
    syncer: BeefySyncer,
) -> impl Stream<Item = AnyResult<BeefyJustification<T>>> + Unpin
where
    T: SenderConfig + 'static,
{
    let stream = futures::stream::repeat(())
        .then(move |()| {
            let sub = sub.clone();
            let syncer = syncer.clone();
            async move {
                let latest_sent = syncer.latest_sent();
                let latest_sent_hash = sub.block_hash(Some(latest_sent.unique_saturated_into())).await?;
                let vset_storage = T::current_validator_set();
                let latest_sent_vset = sub
                    .api()
                    .storage()
                    .fetch_or_default(&vset_storage, Some(latest_sent_hash.into()))
                    .await?.id;
                let best_vset = sub
                    .api()
                    .storage()
                    .fetch_or_default(&vset_storage, None)
                    .await?.id;
                if latest_sent_vset < best_vset {
                    debug!("Waiting for mandatory commitment");
                    tokio::time::sleep(T::average_block_time()).await;
                    return Ok(None);
                }
                let best_block: u64 = sub.block_number(None).await?.into();
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
    syncer: BeefySyncer,
) -> AnyResult<impl Stream<Item = AnyResult<BeefyJustification<T>>> + Unpin>
where
    T: SenderConfig + 'static,
{
    let latest_sent = syncer.latest_sent();
    let latest_sent_hash = sub
        .block_hash(Some(latest_sent.unique_saturated_into()))
        .await?;
    let vset_storage = T::current_validator_set();
    let latest_sent_vset = sub
        .api()
        .storage()
        .fetch_or_default(&vset_storage, Some(latest_sent_hash.into()))
        .await?
        .id;
    let mandatory_stream = mandatory_commitment_stream(sub.clone(), latest_sent_vset);
    let beefy_stream = beefy_commitment_stream(sub.clone(), syncer.clone());
    // Always check mandatory commitments stream first
    let res = futures::stream::select_with_strategy(mandatory_stream, beefy_stream, |()| {
        futures::stream::PollNext::Left
    });
    Ok(res)
}
