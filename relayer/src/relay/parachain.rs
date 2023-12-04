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

use super::beefy_syncer::BeefySyncer;
use super::justification::*;
use crate::prelude::*;
use crate::substrate::OtherParams;
use bridge_common::bitfield::BitField;
use bridge_types::{GenericNetworkId, SubNetworkId};
use futures::stream::StreamExt;
use sp_runtime::traits::UniqueSaturatedInto;
use subxt::rpc_params;
use subxt::tx::TxPayload;

pub struct RelayBuilder<S: SenderConfig, R: ReceiverConfig> {
    sender: Option<SubUnsignedClient<S>>,
    receiver: Option<SubSignedClient<R>>,
    syncer: Option<BeefySyncer>,
}

impl<S: SenderConfig, R: ReceiverConfig> Default for RelayBuilder<S, R> {
    fn default() -> Self {
        Self {
            sender: None,
            receiver: None,
            syncer: None,
        }
    }
}

impl<S, R> RelayBuilder<S, R>
where
    S: SenderConfig,
    R: ReceiverConfig,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_sender_client(mut self, sender: SubUnsignedClient<S>) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn with_receiver_client(mut self, receiver: SubSignedClient<R>) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn with_syncer(mut self, syncer: BeefySyncer) -> Self {
        self.syncer = Some(syncer);
        self
    }

    pub async fn build(self) -> AnyResult<Relay<S, R>> {
        let sender = self.sender.expect("sender client is needed");
        let receiver = self.receiver.expect("receiver client is needed");
        let syncer = self.syncer.expect("syncer is needed");
        let sender_network_id = sender.constant_fetch_or_default(&S::network_id().unvalidated())?;

        let GenericNetworkId::Sub(sender_network_id) = sender_network_id else {
            return Err(anyhow::anyhow!("Error! Sender is NOT a Substrate Network!"));
        };

        let latest_beefy_block = sender
            .storage_fetch_or_default(&R::latest_beefy_block(sender_network_id), ())
            .await?;
        syncer.update_latest_sent(latest_beefy_block);
        Ok(Relay {
            sender,
            receiver,
            successful_sent: Default::default(),
            failed_to_sent: Default::default(),
            syncer,
            sender_network_id,
        })
    }
}

#[derive(Clone)]
pub struct Relay<S: SenderConfig, R: ReceiverConfig> {
    sender: SubUnsignedClient<S>,
    receiver: SubSignedClient<R>,
    successful_sent: Arc<AtomicU64>,
    failed_to_sent: Arc<AtomicU64>,
    syncer: BeefySyncer,
    sender_network_id: SubNetworkId,
}

impl<S, R> Relay<S, R>
where
    S: SenderConfig,
    R: ReceiverConfig,
    OtherParams<R>: Default,
{
    async fn create_random_bitfield(
        &self,
        initial_bitfield: BitField,
        num_validators: u32,
    ) -> AnyResult<BitField> {
        let params = rpc_params![self.sender_network_id, initial_bitfield, num_validators];
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
        justification: &BeefyJustification<S>,
    ) -> AnyResult<impl TxPayload> {
        let initial_bitfield = BitField::create_bitfield(
            &justification.signed_validators,
            justification.num_validators as usize,
        );

        let commitment = bridge_common::beefy_types::Commitment {
            payload: justification.commitment.payload.clone(),
            block_number: justification
                .commitment
                .block_number
                .unique_saturated_into(),
            validator_set_id: justification.commitment.validator_set_id,
        };

        let random_bitfield = self
            .create_random_bitfield(initial_bitfield.clone(), justification.num_validators)
            .await?;
        let validator_proof = justification.validators_proof_sub(initial_bitfield, random_bitfield);
        let (latest_mmr_leaf, proof) = justification.simplified_mmr_proof_sub()?;

        let call = R::submit_signature_commitment(
            self.sender_network_id,
            commitment,
            validator_proof,
            latest_mmr_leaf,
            proof,
        );

        Ok(call)
    }

    pub async fn send_commitment(self, justification: BeefyJustification<S>) -> AnyResult<()> {
        debug!("New justification: {:?}", justification);
        let call = self.submit_signature_commitment(&justification).await?;
        self.receiver.submit_extrinsic(&call).await?;
        self.syncer
            .update_latest_sent(justification.commitment.block_number.into());
        Ok(())
    }

    pub async fn run(&self, ignore_unneeded_commitments: bool) -> AnyResult<()> {
        let mut beefy_sub = crate::substrate::beefy_subscription::subscribe_beefy_justifications(
            self.sender.clone(),
            self.syncer.latest_sent(),
        )
        .await?;
        let mut first_attempt_failed = false;
        while let Some(justification) = beefy_sub.next().await.transpose()? {
            let latest_requested = self.syncer.latest_requested();
            let latest_sent = self.syncer.latest_sent();
            let is_mandatory = justification.is_mandatory;
            let should_send = !ignore_unneeded_commitments
                || is_mandatory
                || (latest_requested < justification.commitment.block_number.into()
                    && latest_sent < latest_requested);

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
                    if first_attempt_failed || is_mandatory {
                        return Err(anyhow::anyhow!(
                            "Unable to send commitment, possibly BEEFY state is broken"
                        ));
                    }
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
