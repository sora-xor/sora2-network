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
use crate::ethereum::SignedClientInner;
use crate::prelude::*;
use crate::substrate::BlockNumber;
use bridge_types::EVMChainId;
use bridge_types::{Address, U256};
use ethereum_gen::{beefy_light_client, inbound_channel, InboundChannel};
use ethers::abi::RawLog;
use ethers::prelude::EthLogDecode;
use ethers::providers::Middleware;
use ethers::types::Bytes;
use sp_runtime::traits::Keccak256;

pub struct RelayBuilder<S: SenderConfig> {
    sender: Option<SubUnsignedClient<S>>,
    receiver: Option<EthSignedClient>,
    syncer: Option<BeefySyncer>,
    inbound_channel: Option<Address>,
}

impl<S: SenderConfig> Default for RelayBuilder<S> {
    fn default() -> Self {
        Self {
            sender: None,
            receiver: None,
            syncer: None,
            inbound_channel: None,
        }
    }
}

impl<S> RelayBuilder<S>
where
    S: SenderConfig,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_sender_client(mut self, sender: SubUnsignedClient<S>) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn with_receiver_client(mut self, receiver: EthSignedClient) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn with_inbound_channel_contract(mut self, address: Address) -> Self {
        self.inbound_channel = Some(address);
        self
    }

    pub fn with_syncer(mut self, syncer: BeefySyncer) -> Self {
        self.syncer = Some(syncer);
        self
    }

    pub async fn build(self) -> AnyResult<Relay<S>> {
        let sender = self.sender.expect("sender client is needed");
        let receiver = self.receiver.expect("receiver client is needed");
        let syncer = self.syncer.expect("syncer is needed");
        let inbound_channel = InboundChannel::new(
            self.inbound_channel
                .expect("inbound channel address is needed"),
            receiver.inner(),
        );
        Ok(Relay {
            chain_id: receiver.inner().get_chainid().await?,
            sender,
            receiver,
            syncer,
            commitment_blocks: Default::default(),
            inbound_channel,
        })
    }
}

#[derive(Clone)]
pub struct Relay<S: SenderConfig> {
    sender: SubUnsignedClient<S>,
    receiver: EthSignedClient,
    commitment_blocks: BTreeMap<u64, BlockNumber<S>>,
    syncer: BeefySyncer,
    inbound_channel: InboundChannel<SignedClientInner>,
    chain_id: EVMChainId,
}

// Relays batches of messages from Substrate to Ethereum.
impl<S> Relay<S>
where
    S: SenderConfig,
{
    async fn send_commitment(&self, batch_nonce: u64) -> AnyResult<()> {
        info!("Sending channel commitment with nonce {:?}", batch_nonce);
        let latest_sent = self.syncer.latest_sent();
        let commitment = super::messages_subscription::load_commitment_with_proof(
            &self.sender,
            self.chain_id.into(),
            batch_nonce,
            latest_sent as u32,
        )
        .await?;
        let commitment_hash = commitment.offchain_data.commitment.hash();
        let bridge_types::GenericCommitment::EVM(commitment_inner) = commitment.offchain_data.commitment else {
            return Err(anyhow::anyhow!("Invalid commitment. EVM commitment is expected"));
        };
        let inbound_channel_nonce = self.inbound_channel_nonce().await?;
        if commitment_inner.nonce <= inbound_channel_nonce {
            info!("Channel commitment is already sent");
            return Ok(());
        }
        let digest_encoded = commitment.digest.encode();
        let digest_hash = hex::encode(&Keccak256::hash(&digest_encoded));
        debug!("Digest hash: {}", digest_hash);
        let leaf_encoded = hex::encode(&commitment.leaf.encode());
        debug!("Leaf: {}", leaf_encoded);
        let leaf_prefix: Bytes =
            hex::decode(leaf_encoded.strip_suffix(&digest_hash).unwrap())?.into();
        let digest_hex = hex::encode(&digest_encoded);
        debug!("Digest: {}", digest_hex);

        let proof = beefy_light_client::SimplifiedMMRProof {
            merkle_proof_items: commitment.proof.items.iter().map(|x| x.0).collect(),
            merkle_proof_order_bit_field: commitment.proof.order,
        };

        let delimiter = (self.chain_id, commitment_hash).encode();
        let (digest_prefix, digest_suffix) =
            digest_hex.split_once(&hex::encode(delimiter)).unwrap();
        let digest_prefix = hex::decode(digest_prefix)?.into();
        let digest_suffix = hex::decode(digest_suffix)?.into();
        let mut messages = vec![];
        for message in commitment_inner.messages {
            messages.push(inbound_channel::Message {
                target: message.target,
                payload: message.payload.to_vec().into(),
                max_gas: message.max_gas,
            });
        }
        let batch = inbound_channel::Batch {
            nonce: U256::from(commitment_inner.nonce),
            total_max_gas: commitment_inner.total_max_gas,
            messages,
        };
        let leaf_bytes = inbound_channel::LeafBytes {
            digest_prefix,
            digest_suffix,
            leaf_prefix: leaf_prefix.clone(),
        };
        let messages_total_gas = batch.total_max_gas;
        let mut call = self
            .inbound_channel
            .submit(batch, leaf_bytes, proof)
            .legacy();

        debug!("Fill submit messages");
        self.receiver
            .fill_transaction(&mut call.tx, call.block)
            .await?;
        debug!("Messages total gas: {}", messages_total_gas);
        call.tx.set_gas(self.submit_message_gas(messages_total_gas));
        debug!("Check submit messages");
        call.call().await?;
        self.receiver
            .save_gas_price(&call, "submit-messages")
            .await?;
        debug!("Send submit messages");
        let tx = call.send().await?;
        debug!("Wait for confirmations submit messages: {:?}", tx);
        let tx = tx.confirmations(1).await?;
        debug!("Submit messages: {:?}", tx);
        if let Some(tx) = tx {
            for log in tx.logs {
                let raw_log = RawLog {
                    topics: log.topics.clone(),
                    data: log.data.to_vec(),
                };
                if let Ok(log) =
                    <inbound_channel::BatchDispatchedFilter as EthLogDecode>::decode_log(&raw_log)
                {
                    info!("Batch dispatched: {:?}", log);
                }
            }
        }

        Ok(())
    }

    fn submit_message_gas(&self, messages_total_gas: U256) -> U256 {
        messages_total_gas.saturating_add(260000.into())
    }

    async fn inbound_channel_nonce(&self) -> AnyResult<u64> {
        let nonce = self.inbound_channel.batch_nonce().call().await?;
        Ok(nonce)
    }

    async fn outbound_channel_nonce(&self) -> AnyResult<u64> {
        let nonce = self
            .sender
            .storage_fetch_or_default(&S::bridge_outbound_nonce(self.chain_id.into()), ())
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
                            .bridge_commitment(self.chain_id.into(), nonce)
                            .await?;
                        v.insert(offchain_data.block_number);
                        offchain_data.block_number
                    }
                    std::collections::btree_map::Entry::Occupied(v) => v.get().clone(),
                };
                self.syncer.update_latest_requested(block_number.into());
                let latest_sent = self.syncer.latest_sent();
                if Into::<u64>::into(block_number) > latest_sent {
                    debug!(
                        "Waiting for BEEFY block {:?}, latest sent {:?}",
                        block_number, latest_sent
                    );
                    break;
                }
                if let Err(err) = self.send_commitment(nonce).await {
                    return Err(anyhow!("Error sending message commitment: {:?}", err));
                }
                self.commitment_blocks.remove(&nonce);
            }
        }
    }
}
