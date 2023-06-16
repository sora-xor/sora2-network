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

use std::collections::VecDeque;

use super::beefy_syncer::BeefySyncer;
use crate::prelude::*;
use crate::substrate::{BlockNumber, OtherParams};
use bridge_types::{SubNetworkId, H256};
use futures::FutureExt;
use futures::StreamExt;

pub struct RelayBuilder<S: SenderConfig, R: ReceiverConfig> {
    sender: Option<SubUnsignedClient<S>>,
    receiver: Option<SubUnsignedClient<R>>,
    syncer: Option<BeefySyncer>,
    from_block: Option<BlockNumber<S>>,
}

impl<S: SenderConfig, R: ReceiverConfig> Default for RelayBuilder<S, R> {
    fn default() -> Self {
        Self {
            sender: None,
            receiver: None,
            syncer: None,
            from_block: None,
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

    pub fn with_receiver_client(mut self, receiver: SubUnsignedClient<R>) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn with_syncer(mut self, syncer: BeefySyncer) -> Self {
        self.syncer = Some(syncer);
        self
    }

    pub fn with_start_block(mut self, from_block: BlockNumber<S>) -> Self {
        self.from_block = Some(from_block);
        self
    }

    pub async fn build(self) -> AnyResult<Relay<S, R>> {
        let sender = self.sender.expect("sender client is needed");
        let receiver = self.receiver.expect("receiver client is needed");
        let syncer = self.syncer.expect("syncer is needed");
        let sender_network_id = sender
            .storage_fetch_or_default(&S::network_id(), ())
            .await?;
        let receiver_network_id = receiver
            .storage_fetch_or_default(&R::network_id(), ())
            .await?;
        Ok(Relay {
            sender,
            receiver,
            syncer,
            commitment_queue: Default::default(),
            receiver_network_id,
            sender_network_id,
            from_block: self.from_block,
        })
    }
}

#[derive(Clone)]
pub struct Relay<S: SenderConfig, R: ReceiverConfig> {
    sender: SubUnsignedClient<S>,
    receiver: SubUnsignedClient<R>,
    commitment_queue: VecDeque<(BlockNumber<S>, H256)>,
    syncer: BeefySyncer,
    receiver_network_id: SubNetworkId,
    sender_network_id: SubNetworkId,
    from_block: Option<BlockNumber<S>>,
}

impl<S, R> Relay<S, R>
where
    S: SenderConfig,
    R: ReceiverConfig,
    OtherParams<R>: Default,
{
    async fn send_commitment(
        &self,
        block_number: BlockNumber<S>,
        commitment_hash: H256,
    ) -> AnyResult<()> {
        info!("Sending channel commitment for block {:?}", block_number);
        let latest_sent = self.syncer.latest_sent();
        let commitment = super::messages_subscription::load_commitment_with_proof(
            &self.sender,
            self.receiver_network_id.into(),
            block_number,
            commitment_hash,
            latest_sent as u32,
        )
        .await?;
        let super::messages_subscription::MessageCommitment::Sub(commitment_inner) = commitment.commitment else {
            return Err(anyhow::anyhow!("Invalid commitment"));
        };
        let inbound_channel_nonce = self.inbound_channel_nonce().await?;
        if commitment_inner
            .messages
            .iter()
            .all(|message| message.nonce <= inbound_channel_nonce)
        {
            info!("Channel commitment is already sent");
            return Ok(());
        }

        let payload = R::submit_messages_commitment(
            self.sender_network_id,
            commitment_inner.messages,
            R::beefy_proof(beefy_light_client::SubstrateBridgeMessageProof {
                proof: commitment.proof,
                leaf: commitment.leaf,
                digest: commitment.digest,
            }),
        );

        info!("Sending channel commitment");
        self.receiver.submit_unsigned_extrinsic(&payload).await?;
        Ok(())
    }

    async fn inbound_channel_nonce(&self) -> AnyResult<u64> {
        let storage = R::substrate_bridge_inbound_nonce(self.sender_network_id);
        let nonce = self.receiver.storage_fetch_or_default(&storage, ()).await?;
        Ok(nonce)
    }

    pub async fn run(mut self) -> AnyResult<()> {
        let inbound_nonce = self.inbound_channel_nonce().await?;
        let mut subscription = super::messages_subscription::subscribe_message_commitments(
            self.sender.clone(),
            self.receiver_network_id.into(),
            self.from_block.unwrap_or(1u32.into()),
            inbound_nonce,
        );
        loop {
            let res = futures::select! {
                a_res = subscription.next().fuse() => Some(a_res),
                _ = tokio::time::sleep(S::average_block_time()).fuse() => None,
            };
            if let Some((block, hash)) = res.flatten().transpose()? {
                self.commitment_queue.push_back((block, hash));
                let block: u64 = block.into();
                self.syncer.request(block + 1);
            }
            let latest_sent = self.syncer.latest_sent();
            loop {
                let (block_number, commitment_hash) = match self.commitment_queue.pop_front() {
                    Some(commitment) => commitment,
                    None => break,
                };
                if Into::<u64>::into(block_number) > latest_sent {
                    debug!("Waiting for BEEFY block {:?}", block_number);
                    self.commitment_queue
                        .push_front((block_number, commitment_hash));
                    break;
                }
                if let Err(err) = self.send_commitment(block_number, commitment_hash).await {
                    error!("Error sending message commitment: {:?}", err);
                    self.commitment_queue
                        .push_front((block_number, commitment_hash));
                    return Err(anyhow!("Error sending message commitment: {:?}", err));
                }
            }
        }
    }
}
