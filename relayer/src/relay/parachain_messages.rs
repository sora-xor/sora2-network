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

use std::collections::BTreeMap;

use super::beefy_syncer::BeefySyncer;
use crate::prelude::*;
use crate::substrate::{BlockNumber, BlockNumberOrHash, OtherParams};
use bridge_types::{GenericNetworkId, SubNetworkId};

pub struct RelayBuilder<S: SenderConfig, R: ReceiverConfig> {
    sender: Option<SubUnsignedClient<S>>,
    receiver: Option<SubUnsignedClient<R>>,
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

    pub fn with_receiver_client(mut self, receiver: SubUnsignedClient<R>) -> Self {
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
        let sender_network_id = sender.constant_fetch_or_default(&S::network_id())?;

        let GenericNetworkId::Sub(sender_network_id) = sender_network_id else {
            return Err(anyhow::anyhow!("Error! Sender is NOT a Substrate Network!"));
        };

        let receiver_network_id = receiver.constant_fetch_or_default(&R::network_id())?;

        let GenericNetworkId::Sub(receiver_network_id) = receiver_network_id else {
            return Err(anyhow::anyhow!("Error! Reciever is NOT a Substrate Network!"));
        };

        Ok(Relay {
            sender,
            receiver,
            syncer,
            commitment_blocks: Default::default(),
            receiver_network_id,
            sender_network_id,
        })
    }
}

#[derive(Clone)]
pub struct Relay<S: SenderConfig, R: ReceiverConfig> {
    sender: SubUnsignedClient<S>,
    receiver: SubUnsignedClient<R>,
    commitment_blocks: BTreeMap<u64, BlockNumber<S>>,
    syncer: BeefySyncer,
    receiver_network_id: SubNetworkId,
    sender_network_id: SubNetworkId,
}

impl<S, R> Relay<S, R>
where
    S: SenderConfig,
    R: ReceiverConfig,
    OtherParams<R>: Default,
{
    async fn send_commitment(&self, batch_nonce: u64) -> AnyResult<()> {
        info!("Sending channel commitment with nonce {:?}", batch_nonce);
        let latest_sent = self.syncer.latest_sent();
        let commitment = super::messages_subscription::load_commitment_with_proof(
            &self.sender,
            self.receiver_network_id.into(),
            batch_nonce,
            latest_sent as u32,
        )
        .await?;
        let inbound_channel_nonce = self.inbound_channel_nonce().await?;
        if commitment.offchain_data.commitment.nonce() <= inbound_channel_nonce {
            info!("Channel commitment is already sent");
            return Ok(());
        }

        let payload = R::submit_messages_commitment(
            self.sender_network_id,
            commitment.offchain_data.commitment,
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

    async fn outbound_channel_nonce(&self) -> AnyResult<u64> {
        let nonce = self
            .sender
            .storage_fetch_or_default(
                &S::bridge_outbound_nonce(self.receiver_network_id.into()),
                BlockNumberOrHash::Finalized,
            )
            .await?;
        Ok(nonce)
    }

    pub async fn run(mut self) -> AnyResult<()> {
        let mut interval = tokio::time::interval(S::average_block_time());
        loop {
            interval.tick().await;
            let inbound_nonce = self.inbound_channel_nonce().await?;
            let outbound_nonce = self.outbound_channel_nonce().await?;
            if inbound_nonce >= outbound_nonce {
                if inbound_nonce > outbound_nonce {
                    error!(
                        "Inbound channel nonce is higher than outbound channel nonce: {} > {}",
                        inbound_nonce, outbound_nonce
                    );
                }
                continue;
            }
            for nonce in (inbound_nonce + 1)..=outbound_nonce {
                let block_number = match self.commitment_blocks.entry(nonce) {
                    std::collections::btree_map::Entry::Vacant(v) => {
                        let offchain_data = self
                            .sender
                            .commitment_with_nonce(
                                self.receiver_network_id.into(),
                                nonce,
                                BlockNumberOrHash::Finalized,
                            )
                            .await?;
                        v.insert(offchain_data.block_number);
                        offchain_data.block_number
                    }
                    std::collections::btree_map::Entry::Occupied(v) => v.get().clone(),
                };
                let latest_sent = self.syncer.latest_sent();
                if Into::<u64>::into(block_number) > latest_sent {
                    debug!("Waiting for BEEFY block {:?}", block_number);
                    break;
                }
                self.send_commitment(nonce).await?;
                if let Err(err) = self.send_commitment(nonce).await {
                    return Err(anyhow!("Error sending message commitment: {:?}", err));
                }
                self.commitment_blocks.remove(&nonce);
            }
        }
    }
}
