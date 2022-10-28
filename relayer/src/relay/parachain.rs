use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::justification::*;
use crate::prelude::*;
use beefy_gadget_rpc::BeefyApiClient;
use beefy_primitives::VersionedFinalityProof;
use bridge_common::bitfield::BitField;
use subxt::events::StaticEvent;
use subxt::rpc_params;
use subxt::tx::TxPayload;

#[derive(Default)]
pub struct RelayBuilder {
    sender: Option<SubSignedClient>,
    receiver: Option<SubSignedClient>,
}

impl RelayBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_sender_client(mut self, sender: SubSignedClient) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn with_receiver_client(mut self, receiver: SubSignedClient) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub async fn build(self) -> AnyResult<Relay> {
        let sender = self.sender.expect("sender client is needed");
        let receiver = self.receiver.expect("receiver client is needed");
        Ok(Relay {
            sender,
            receiver,
            successful_sent: Default::default(),
            failed_to_sent: Default::default(),
        })
    }
}

#[derive(Clone)]
pub struct Relay {
    sender: SubSignedClient,
    receiver: SubSignedClient,
    successful_sent: Arc<AtomicU64>,
    failed_to_sent: Arc<AtomicU64>,
}

impl Relay {
    async fn create_random_bitfield(
        &self,
        initial_bitfield: BitField,
        num_validators: u128,
    ) -> AnyResult<BitField> {
        let params = rpc_params![initial_bitfield, num_validators];
        let random_bitfield = self
            .receiver
            .api()
            .rpc()
            .request("beefyLightClient_getRandomBitfield", params)
            .await?;
        Ok(random_bitfield)
    }

    async fn submit_signature_commitment(
        &self,
        justification: &BeefyJustification,
    ) -> AnyResult<impl TxPayload> {
        let initial_bitfield = BitField::create_bitfield(
            justification
                .signed_validators
                .iter()
                .cloned()
                .map(|x| x.as_u128())
                .collect(),
            justification.num_validators.as_u128(),
        );

        let commitment = bridge_common::beefy_types::Commitment {
            payload_prefix: justification.payload.prefix.clone().into(),
            payload: justification.payload.mmr_root.into(),
            payload_suffix: justification.payload.suffix.clone().into(),
            block_number: justification.commitment.block_number,
            validator_set_id: justification.commitment.validator_set_id,
        };

        let random_bitfield = self
            .create_random_bitfield(
                initial_bitfield.clone(),
                justification.num_validators.as_u128(),
            )
            .await?;
        let validator_proof = justification.validators_proof_sub(initial_bitfield, random_bitfield);
        let (latest_mmr_leaf, proof) = justification.simplified_mmr_proof_sub()?;

        let call = runtime::tx()
            .beefy_light_client()
            .submit_signature_commitment(commitment, validator_proof, latest_mmr_leaf, proof);

        Ok(call)
    }

    pub async fn call_with_event<E: StaticEvent, U: TxPayload>(&self, call: U) -> AnyResult<E> {
        let tx = self
            .receiver
            .api()
            .tx()
            .sign_and_submit_then_watch_default(&call, &self.receiver)
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;

        let success_event = tx.find_first::<E>()?.ok_or(anyhow!("event not found"))?;
        Ok(success_event)
    }

    pub async fn send_commitment(self, justification: BeefyJustification) -> AnyResult<()> {
        debug!("New justification: {:?}", justification);
        let call = self.submit_signature_commitment(&justification).await?;
        let _event = self
            .call_with_event::<runtime::beefy_light_client::events::VerificationSuccessful, _>(call)
            .await?;
        Ok(())
    }

    pub async fn sync_historical_commitments(&self) -> AnyResult<()> {
        let epoch_duration = self
            .sender
            .api()
            .constants()
            .at(&runtime::constants().babe().epoch_duration())?;
        let sessions_per_era = self
            .sender
            .api()
            .constants()
            .at(&runtime::constants().staking().sessions_per_era())?;
        let era_duration = epoch_duration * sessions_per_era as u64;
        'main_loop: loop {
            let latest_beefy_block =
                self.sender
                    .api()
                    .storage()
                    .fetch(
                        &runtime::storage().beefy_light_client().latest_beefy_block(),
                        None,
                    )
                    .await?
                    .ok_or(anyhow!("Error to get latest beefy block"))? as u64;
            let latest_beefy_block_hash = self.sender.block_hash(Some(latest_beefy_block)).await?;
            let latest_era = self
                .sender
                .api()
                .storage()
                .fetch(
                    &runtime::storage().staking().active_era(),
                    Some(latest_beefy_block_hash),
                )
                .await?
                .expect("should exist");
            let current_block_hash = self.sender.api().rpc().finalized_head().await?;
            let current_block = self.sender.block_number(Some(current_block_hash)).await?;
            let next_block = latest_beefy_block
                + (beefy_light_client::MAXIMUM_BLOCK_GAP - 1).min(era_duration + 1);
            if next_block > current_block as u64 {
                return Ok(());
            }
            let next_block_hash = self.sender.block_hash(Some(next_block)).await?;
            let next_eras = self
                .sender
                .api()
                .storage()
                .fetch_or_default(
                    &runtime::storage().staking().bonded_eras(),
                    Some(next_block_hash),
                )
                .await?;
            debug!("latest era: {latest_era:?}, next block: {next_block}, eras: {next_eras:?}");
            let next_block = if let Some((_, session)) = next_eras
                .into_iter()
                .find(|(index, _)| index > &latest_era.index)
            {
                session as u64 * epoch_duration + 1
            } else {
                next_block
            };
            debug!(
                "latest beefy block: {}, next block: {}",
                latest_beefy_block, next_block
            );
            let current_validator_set_id = self
                .receiver
                .api()
                .storage()
                .fetch(
                    &runtime::storage()
                        .beefy_light_client()
                        .current_validator_set(),
                    None,
                )
                .await?
                .ok_or(anyhow!("Error to get current validator set"))?
                .id;

            let next_validator_set_id = self
                .receiver
                .api()
                .storage()
                .fetch(
                    &runtime::storage().beefy_light_client().next_validator_set(),
                    None,
                )
                .await?
                .ok_or(anyhow!("Error to get next validator set"))?
                .id;

            for next_block in ((latest_beefy_block + 1)..=next_block).rev() {
                let block = self.sender.block(Some(next_block)).await?;
                debug!("Check block {:?}", block.block.header.number);
                if let Some(justifications) = block.justifications {
                    for (engine, justification) in justifications {
                        if &engine == b"BEEF" {
                            let commitment =
                                VersionedFinalityProof::decode(&mut justification.as_slice())?;
                            let justification = match BeefyJustification::create(
                                self.sender.clone().unsigned(),
                                commitment,
                            )
                            .await
                            {
                                Ok(justification) => justification,
                                Err(err) => {
                                    warn!("failed to create justification: {}", err);
                                    continue;
                                }
                            };
                            debug!("Justification: {:?}", justification);
                            if justification.commitment.validator_set_id as u128
                                != current_validator_set_id
                                && justification.commitment.validator_set_id as u128
                                    != next_validator_set_id
                            {
                                warn!(
                                    "validator set id mismatch: {} + 1 != {}",
                                    justification.commitment.validator_set_id,
                                    current_validator_set_id
                                );
                                continue;
                            }

                            let _ =
                                self.clone()
                                    .send_commitment(justification)
                                    .await
                                    .map_err(|err| {
                                        warn!("failed to send: {}", err);
                                        err
                                    });
                            info!(
                                "failed: {}, successfull: {}",
                                self.failed_to_sent.load(Ordering::Relaxed),
                                self.successful_sent.load(Ordering::Relaxed)
                            );
                            continue 'main_loop;
                        }
                    }
                }
            }
            return Err(anyhow::anyhow!("Justification not found"));
        }
    }

    pub async fn run(&self, ignore_unneeded_commitments: bool) -> AnyResult<()> {
        self.sync_historical_commitments()
            .await
            .context("sync historical commitments")?;
        let mut beefy_sub = self.sender.beefy().subscribe_justifications().await?;
        while let Some(encoded_commitment) = beefy_sub.next().await.transpose()? {
            let justification = match BeefyJustification::create(
                self.sender.clone().unsigned(),
                encoded_commitment.decode()?,
            )
            .await
            {
                Ok(justification) => justification,
                Err(err) => {
                    warn!("failed to create justification: {}", err);
                    continue;
                }
            };

            let latest_block =
                self.receiver
                    .api()
                    .storage()
                    .fetch(
                        &runtime::storage().beefy_light_client().latest_beefy_block(),
                        None,
                    )
                    .await?
                    .ok_or(anyhow!("Error to get latest beefy block"))? as u64;

            let next_validator_set_id = self
                .receiver
                .api()
                .storage()
                .fetch(
                    &runtime::storage().beefy_light_client().next_validator_set(),
                    None,
                )
                .await?
                .ok_or(anyhow!("Error to get next validator set"))?
                .id;

            let is_mandatory = next_validator_set_id
                < justification.leaf_proof.leaf.beefy_next_authority_set.id as u128;

            let should_send = !ignore_unneeded_commitments
                || is_mandatory
                || (justification.commitment.block_number as u64
                    > latest_block + beefy_light_client::MAXIMUM_BLOCK_GAP - 20);

            if should_send {
                // TODO: Better async message handler
                let _ = self
                    .clone()
                    .send_commitment(justification)
                    .await
                    .map_err(|e| {
                        warn!("Send commitment error: {}", e);
                    });
                info!(
                    "failed: {}, successfull: {}",
                    self.failed_to_sent.load(Ordering::Relaxed),
                    self.successful_sent.load(Ordering::Relaxed)
                );
            } else {
                info!(
                    "Skip BEEFY commitment because there is no messages: {:?}",
                    justification
                );
            }
        }

        Ok(())
    }
}
