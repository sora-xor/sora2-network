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
use crate::relay::client::RuntimeClient;
use crate::relay::simplified_proof::convert_to_simplified_mmr_proof;
use crate::substrate::{BlockNumber, LeafProof};
use bridge_types::types::AuxiliaryDigestItem;
use bridge_types::{GenericNetworkId, H256};
use common::Balance;
use sp_runtime::traits::{Hash, Keccak256};
use substrate_gen::runtime::runtime_types::beefy_light_client::ProvedSubstrateBridgeMessage;

pub struct RelayBuilder<S, R> {
    sender: Option<S>,
    receiver: Option<R>,
    syncer: Option<BeefySyncer>,
}

impl<S, R> Default for RelayBuilder<S, R> {
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

    pub fn with_syncer(mut self, syncer: BeefySyncer) -> Self {
        self.syncer = Some(syncer);
        self
    }

    pub async fn build(self) -> AnyResult<Relay<S, R>> {
        let sender = self.sender.expect("sender client is needed");
        let receiver = self.receiver.expect("receiver client is needed");
        let syncer = self.syncer.expect("syncer is needed");
        let network_id = receiver.network_id().await?;
        let first_mmr_block = receiver.first_beefy_block().await?;
        Ok(Relay {
            sender,
            receiver,
            syncer,
            commitment_queue: Default::default(),
            network_id,
            first_mmr_block,
        })
    }
}

#[derive(Clone)]
pub struct Relay<S, R> {
    sender: S,
    receiver: R,
    commitment_queue: VecDeque<(
        BlockNumber,
        substrate_bridge_channel_rpc::Commitment<Balance>,
    )>,
    syncer: BeefySyncer,
    network_id: GenericNetworkId,
    first_mmr_block: u64,
}

impl<S, R> Relay<S, R>
where
    S: RuntimeClient + Clone,
    R: RuntimeClient + Clone,
{
    async fn leaf_proof_with_digest(
        &self,
        digest_hash: H256,
        start_leaf: u64,
        count: u64,
        at: Option<H256>,
    ) -> AnyResult<LeafProof> {
        for leaf in start_leaf..start_leaf + count {
            let leaf_proof = self.sender.client().mmr_generate_proof(leaf, at).await?;
            if leaf_proof.leaf.leaf_extra.digest_hash == digest_hash {
                return Ok(leaf_proof);
            }
        }
        return Err(anyhow::anyhow!("leaf proof not found"));
    }

    async fn find_commitment_with_nonce(
        &self,
        network_id: GenericNetworkId,
        nonce: u64,
    ) -> AnyResult<Option<(BlockNumber, H256)>> {
        let start_block = self.sender.find_message_block(network_id, nonce).await?;
        let start_block = if let Some(start_block) = start_block {
            start_block + 1
        } else {
            return Ok(None);
        };
        let end_block = self
            .sender
            .client()
            .block_number::<BlockNumber>(None)
            .await?;
        for block in start_block..=end_block {
            let digest = self.sender.client().auxiliary_digest(Some(block)).await?;
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

    async fn send_commitment(
        &self,
        block_number: u64,
        commitment: substrate_bridge_channel_rpc::Commitment<Balance>,
    ) -> AnyResult<()> {
        info!("Sending channel commitment for block {}", block_number);
        let inbound_channel_nonce = self.receiver.inbound_channel_nonce(self.network_id).await?;
        if commitment
            .messages
            .iter()
            .all(|message| message.nonce <= inbound_channel_nonce)
        {
            info!("Channel commitment is already sent");
            return Ok(());
        }
        let latest_sent = self.syncer.latest_sent();
        let latest_sent_hash = self.sender.client().block_hash(Some(latest_sent)).await?;
        let block_hash = self
            .sender
            .client()
            .api()
            .rpc()
            .block_hash(Some(block_number.into()))
            .await?
            .expect("should exist");
        let digest = self
            .sender
            .client()
            .auxiliary_digest(Some(block_hash))
            .await?;
        if digest.logs.is_empty() {
            warn!("Digest is empty");
            return Ok(());
        }
        let valid_items = digest
            .logs
            .iter()
            .filter(|log| {
                let AuxiliaryDigestItem::Commitment(network_id, commitment_hash) = log;
                if *network_id != self.network_id
                    && *commitment_hash != Keccak256::hash_of(&commitment)
                {
                    false
                } else {
                    true
                }
            })
            .count();
        if valid_items != 1 {
            warn!("Expected digest for commitment not found: {:?}", digest);
            return Ok(());
        }
        let digest_hash = Keccak256::hash_of(&digest);
        trace!("Digest hash: {}", digest_hash);
        let leaf_proof = self
            .leaf_proof_with_digest(
                digest_hash,
                block_number
                    .saturating_sub(self.first_mmr_block)
                    .saturating_sub(1),
                50,
                Some(latest_sent_hash),
            )
            .await?;
        let leaf = leaf_proof.leaf;
        let proof = leaf_proof.proof;
        let leaf_version = {
            let (major, minor) = leaf.version.split();
            major << 5 + minor
        };
        let ready_leaf = bridge_common::beefy_types::BeefyMMRLeaf {
            version: leaf_version,
            parent_number: leaf.parent_number_and_hash.0,
            next_authority_set_id: leaf.beefy_next_authority_set.id,
            next_authority_set_len: leaf.beefy_next_authority_set.len,
            parent_hash: leaf.parent_number_and_hash.1 .0,
            next_authority_set_root: leaf.beefy_next_authority_set.root.0,
            random_seed: leaf.leaf_extra.random_seed.0,
            digest_hash: leaf.leaf_extra.digest_hash.0,
        };
        trace!("Leaf: {:?}", leaf);

        let proof =
            convert_to_simplified_mmr_proof(proof.leaf_index, proof.leaf_count, &proof.items);
        let proof = bridge_common::simplified_mmr_proof::SimplifiedMMRProof {
            merkle_proof_items: proof.items.iter().map(|x| x.0).collect(),
            merkle_proof_order_bit_field: proof.order,
        };

        let payload = self
            .receiver
            .submit_messages_commitment(
                self.network_id,
                ProvedSubstrateBridgeMessage {
                    message: commitment.messages,
                    proof,
                    leaf: ready_leaf,
                    digest,
                },
            )
            .await;

        info!("Sending channel commitment");
        let res = self
            .receiver
            .client()
            .api()
            .tx()
            .sign_and_submit_then_watch_default(&payload, self.receiver.client())
            .await?
            .wait_for_in_block()
            .await?
            .wait_for_success()
            .await?;
        info!("Successfully sent channel commitment");
        sub_log_tx_events(res);
        Ok(())
    }

    pub async fn run(mut self) -> AnyResult<()> {
        let sender_network = self.sender.network_id().await?;
        let receiver_network = self.receiver.network_id().await?;
        loop {
            let mut inbound_nonce = self.receiver.inbound_channel_nonce(sender_network).await?;
            let outbound_nonce = self.sender.outbound_channel_nonce(receiver_network).await?;
            // To add only new commitments to the queue
            if let Some((_, commitment)) = self.commitment_queue.back() {
                inbound_nonce = inbound_nonce.max(commitment.messages.last().unwrap().nonce);
            }
            while outbound_nonce > inbound_nonce {
                debug!(
                    "Fetching messages starting with nonce {}, outbound_nonce {}",
                    inbound_nonce + 1,
                    outbound_nonce
                );
                let Some((block_number, commitment_hash)) = self
                    .find_commitment_with_nonce(receiver_network, inbound_nonce + 1)
                    .await?
                else {
                    debug!("Message not found, waiting for new block");
                    break;
                };
                let commitment = self
                    .sender
                    .client()
                    .substrate_bridge_commitments(commitment_hash)
                    .await?;
                inbound_nonce += commitment.messages.len() as u64;
                self.commitment_queue.push_back((block_number, commitment));
                info!(
                    "Channel commitment added to queue, total: {}",
                    self.commitment_queue.len()
                );
            }
            let latest_sent = self.syncer.latest_sent();
            if let Some((block_number, _)) = self.commitment_queue.back() {
                if *block_number as u64 > latest_sent {
                    self.syncer.request(*block_number as u64 + 1);
                }
            }
            loop {
                let (block_number, commitment) = match self.commitment_queue.pop_front() {
                    Some(commitment) => commitment,
                    None => break,
                };
                if block_number as u64 > latest_sent {
                    debug!("Waiting for BEEFY block {}", block_number);
                    self.commitment_queue.push_front((block_number, commitment));
                    break;
                }
                if let Err(err) = self
                    .send_commitment(block_number as u64, commitment.clone())
                    .await
                {
                    error!("Error sending message commitment: {:?}", err);
                    self.commitment_queue.push_front((block_number, commitment));
                    break;
                }
            }
            info!("Commitment queue: {}", self.commitment_queue.len());
            tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        }
    }
}
