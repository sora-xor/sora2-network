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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::justification::*;
use crate::prelude::*;
use crate::relay::client::RuntimeClient;
use beefy_gadget_rpc::BeefyApiClient;
use beefy_primitives::VersionedFinalityProof;
use bridge_common::bitfield::BitField;
use subxt::events::StaticEvent;
use subxt::rpc_params;
use subxt::tx::TxPayload;

pub struct RelayBuilder<S, R> {
    sender: Option<S>,
    receiver: Option<R>,
}

impl<S, R> Default for RelayBuilder<S, R> {
    fn default() -> Self {
        Self {
            sender: None,
            receiver: None,
        }
    }
}

impl<S, R> RelayBuilder<S, R>
where
    S: RuntimeClient,
    R: RuntimeClient,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_sender_client(mut self, sender: S) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn with_receiver_client(mut self, receiver: R) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub async fn build(self) -> AnyResult<Relay<S, R>> {
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
pub struct Relay<S, R> {
    sender: S,
    receiver: R,
    successful_sent: Arc<AtomicU64>,
    failed_to_sent: Arc<AtomicU64>,
}

impl<S, R> Relay<S, R>
where
    S: RuntimeClient + Clone,
    R: RuntimeClient + Clone,
{
    async fn create_random_bitfield(
        &self,
        initial_bitfield: BitField,
        num_validators: u128,
    ) -> AnyResult<BitField> {
        let params = rpc_params![initial_bitfield, num_validators];
        let random_bitfield = self
            .receiver
            .client()
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

        let call = self.receiver.submit_signature_commitment(
            commitment,
            validator_proof,
            latest_mmr_leaf,
            proof,
        );

        Ok(call)
    }

    pub async fn call_with_event<E: StaticEvent, U: TxPayload>(&self, call: U) -> AnyResult<E> {
        let tx = self
            .receiver
            .client()
            .api()
            .tx()
            .sign_and_submit_then_watch_default(&call, self.receiver.client())
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
            .call_with_event::<R::VerificationSuccessful, _>(call)
            .await?;
        Ok(())
    }

    async fn current_block(&self) -> AnyResult<u64> {
        let current_block_hash = self.sender.client().api().rpc().finalized_head().await?;
        let current_block = self
            .sender
            .client()
            .block_number(Some(current_block_hash))
            .await? as u64;
        Ok(current_block)
    }

    async fn process_block(&self, block_num: u64) -> AnyResult<()> {
        let current_validator_set_id = self.receiver.current_validator_set().await?.id;
        let next_validator_set_id = self.receiver.next_validator_set().await?.id;
        let block = self.sender.client().block(Some(block_num)).await?;
        debug!("Check block {:?}", block.block.header.number);
        if let Some(justifications) = block.justifications {
            for (engine, justification) in justifications {
                if &engine == b"BEEF" {
                    let commitment = VersionedFinalityProof::decode(&mut justification.as_slice())?;
                    let justification = match BeefyJustification::create(
                        self.sender.client().clone().unsigned(),
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
                    if justification.commitment.validator_set_id as u128 != current_validator_set_id
                        && justification.commitment.validator_set_id as u128
                            != next_validator_set_id
                    {
                        warn!(
                            "validator set id mismatch: {} + 1 != {}",
                            justification.commitment.validator_set_id, current_validator_set_id
                        );
                        continue;
                    }

                    let _ = self
                        .clone()
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
                    return Ok(());
                }
            }
        }
        Err(anyhow::anyhow!("Justification not found"))
    }

    pub async fn sync_historical_commitments(&self, end_block: u64) -> AnyResult<()> {
        let epoch_duration = self.sender.epoch_duration()?;
        let latest_beefy_block = self.sender.latest_beefy_block().await? as u64;

        const SHIFT: u64 = 5;

        let mut next_block =
            latest_beefy_block - latest_beefy_block % epoch_duration + epoch_duration;
        while next_block <= end_block {
            for block_num in (next_block - SHIFT)..(next_block + SHIFT) {
                self.process_block(block_num).await?;
            }
            next_block += epoch_duration;
        }
        Ok(())
    }

    pub async fn run(&self, ignore_unneeded_commitments: bool) -> AnyResult<()> {
        let current_block = self.current_block().await?;
        self.sync_historical_commitments(current_block)
            .await
            .context("sync historical commitments")?;

        // The sync takes some time. It is necessary to check new blocks that could be produced during the sync and resync.
        let new_current_block = self.current_block().await?;
        if new_current_block != current_block {
            self.sync_historical_commitments(new_current_block)
                .await
                .context("sync last historical commitments")?;
        }

        let mut beefy_sub = self
            .sender
            .client()
            .beefy()
            .subscribe_justifications()
            .await?;
        let mut first_attempt_failed = false;
        while let Some(encoded_commitment) = beefy_sub.next().await.transpose()? {
            let justification = match BeefyJustification::create(
                self.sender.client().clone().unsigned(),
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

            let next_validator_set_id = self.receiver.next_validator_set().await?.id;

            let is_mandatory = next_validator_set_id
                < justification.leaf_proof.leaf.beefy_next_authority_set.id as u128;

            let should_send = !ignore_unneeded_commitments || is_mandatory;

            if should_send {
                // TODO: Better async message handler
                if let Err(_) = self
                    .clone()
                    .send_commitment(justification)
                    .await
                    .map_err(|e| {
                        warn!("Send commitment error: {}", e);
                    })
                {
                    if first_attempt_failed {
                        return Err(anyhow::anyhow!(
                            "Unable to send commitment, possibly BEEFY state is broken"
                        ));
                    }
                    let current_block = self.current_block().await?;
                    self.sync_historical_commitments(current_block)
                        .await
                        .context("sync historical commitments")?;
                    first_attempt_failed = true;
                } else {
                    first_attempt_failed = false;
                }
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
