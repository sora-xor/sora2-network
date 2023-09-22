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

use std::time::Duration;

use bridge_types::evm::Proof;
use bridge_types::EVMChainId;
use ethers::abi::RawLog;

use crate::ethereum::proof_loader::ProofLoader;
use crate::prelude::*;
use bridge_types::log::Log;
use ethers::prelude::Log as EthersLog;
use ethers::prelude::*;

const BLOCKS_TO_INITIAL_SEARCH: u64 = 49000; // Ethereum light client keep 50000 blocks

/// A message relayed from Ethereum.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug)]
pub struct Message {
    /// The raw message data.
    pub data: Log,
    /// Input to the message verifier
    pub proof: Proof,
}

pub struct SubstrateMessagesRelay {
    sub: SubSignedClient<MainnetConfig>,
    eth: EthUnsignedClient,
    network_id: EVMChainId,
    inbound_channel: Address,
    outbound_channel: Address,
    latest_channel_block: u64,
    proof_loader: ProofLoader,
}

impl SubstrateMessagesRelay {
    pub async fn new(
        sub: SubSignedClient<MainnetConfig>,
        eth: EthUnsignedClient,
        proof_loader: ProofLoader,
    ) -> AnyResult<Self> {
        let network_id = eth.inner().get_chainid().await? as EVMChainId;
        let inbound_channel = sub
            .storage_fetch(
                &runtime::storage()
                    .bridge_inbound_channel()
                    .inbound_channel_addresses(&network_id),
                (),
            )
            .await?
            .ok_or(anyhow::anyhow!("Inbound channel is not registered"))?;
        let outbound_channel = sub
            .storage_fetch(
                &runtime::storage()
                    .bridge_inbound_channel()
                    .channel_addresses(&network_id),
                (),
            )
            .await?
            .ok_or(anyhow::anyhow!("Outbound channel is not registered"))?;
        Ok(Self {
            proof_loader,
            latest_channel_block: 0,
            sub,
            eth,
            network_id,
            inbound_channel,
            outbound_channel,
        })
    }

    pub async fn handle_messages(&mut self) -> AnyResult<()> {
        let current_eth_block = self
            .sub
            .storage_fetch(
                &runtime::storage()
                    .ethereum_light_client()
                    .finalized_block(&self.network_id),
                (),
            )
            .await?
            .ok_or(anyhow!("Network is not registered"))?
            .number;
        if current_eth_block < self.latest_channel_block {
            debug!("Skip handling channel messages, current block number is less than latest basic {} < {}", current_eth_block, self.latest_channel_block);
            return Ok(());
        }

        self.handle_message_events(current_eth_block).await?;
        self.handle_batch_dispatched(current_eth_block).await?;

        self.latest_channel_block = current_eth_block + 1;
        Ok(())
    }

    async fn handle_message_events(&mut self, current_eth_block: u64) -> AnyResult<()> {
        let eth = self.eth.inner();
        let outbound_channel =
            ethereum_gen::OutboundChannel::new(self.outbound_channel, eth.clone());
        let events: Vec<(ethereum_gen::outbound_channel::MessageFilter, LogMeta)> =
            outbound_channel
                .message_filter()
                .from_block(self.latest_channel_block)
                .to_block(current_eth_block)
                .query_with_meta()
                .await?;
        debug!(
            "Channel: Found {} Message events from {} to {}",
            events.len(),
            self.latest_channel_block,
            current_eth_block
        );
        let mut sub_nonce = self
            .sub
            .storage_fetch_or_default(
                &runtime::storage()
                    .bridge_inbound_channel()
                    .channel_nonces(&self.network_id),
                (),
            )
            .await?;

        for (event, meta) in events {
            if event.nonce > sub_nonce && meta.address == self.outbound_channel {
                let tx = eth
                    .get_transaction_receipt(meta.transaction_hash)
                    .await?
                    .expect("should exist");
                for log in tx.logs {
                    let raw_log = RawLog {
                        topics: log.topics.clone(),
                        data: log.data.to_vec(),
                    };
                    if let Ok(event) =
                        <ethereum_gen::outbound_channel::MessageFilter as EthEvent>::decode_log(
                            &raw_log,
                        )
                    {
                        let message = self.make_message(log).await?;
                        debug!("Channel: Send {} message", event.nonce);
                        self.sub
                            .submit_extrinsic(&runtime::tx().bridge_inbound_channel().submit(
                                self.network_id,
                                message.data,
                                message.proof,
                            ))
                            .await?;
                        sub_nonce = event.nonce;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_batch_dispatched(&mut self, current_eth_block: u64) -> AnyResult<()> {
        let eth = self.eth.inner();
        let inbound_channel = ethereum_gen::InboundChannel::new(self.inbound_channel, eth.clone());
        let events: Vec<(
            ethereum_gen::inbound_channel::BatchDispatchedFilter,
            LogMeta,
        )> = inbound_channel
            .batch_dispatched_filter()
            .from_block(self.latest_channel_block)
            .to_block(current_eth_block)
            .query_with_meta()
            .await?;
        debug!(
            "Channel: Found {} BatchDispatched events from {} to {}",
            events.len(),
            self.latest_channel_block,
            current_eth_block
        );

        let mut sub_inbound_nonce = self
            .sub
            .storage_fetch_or_default(
                &runtime::storage()
                    .bridge_inbound_channel()
                    .inbound_channel_nonces(&self.network_id),
                (),
            )
            .await?;

        for (event, meta) in events {
            if event.batch_nonce > sub_inbound_nonce && meta.address == self.inbound_channel {
                let tx = eth
                    .get_transaction_receipt(meta.transaction_hash)
                    .await?
                    .expect("should exist");

                for log in tx.logs {
                    let raw_log = RawLog {
                        topics: log.topics.clone(),
                        data: log.data.to_vec(),
                    };
                    if let Ok(event) =
                        <ethereum_gen::inbound_channel::BatchDispatchedFilter as EthEvent>::decode_log(
                            &raw_log,
                        ) {
                            debug!("Channel: Send BatchDispatched {}", event.batch_nonce);
                            let message = self.make_message(log).await?;
                            self
                                .sub
                                .submit_extrinsic(
                                    &runtime::tx()
                                        .bridge_inbound_channel()
                                        .batch_dispatched(
                                            self.network_id,
                                            message.data,
                                            message.proof,
                                        ),
                                )
                                .await?;
                        sub_inbound_nonce = event.batch_nonce;
                    }
                }
            }
        }

        Ok(())
    }

    async fn make_message(&self, log: EthersLog) -> AnyResult<Message> {
        let block_hash = log.block_hash.unwrap();
        let tx_index = log.transaction_index.unwrap().as_usize();
        let proof = self
            .proof_loader
            .receipt_proof(block_hash, tx_index)
            .await?;
        let log = Log {
            address: log.address,
            topics: log.topics,
            data: log.data.as_ref().to_vec(),
        };
        Ok(Message {
            data: log,
            proof: Proof {
                block_hash,
                tx_index: tx_index as u32,
                data: proof,
            },
        })
    }

    pub async fn run(mut self) -> AnyResult<()> {
        let current_eth_block = self
            .sub
            .storage_fetch(
                &runtime::storage()
                    .ethereum_light_client()
                    .finalized_block(&self.network_id),
                (),
            )
            .await?
            .ok_or(anyhow!("Network is not registered"))?
            .number;
        self.latest_channel_block = current_eth_block.saturating_sub(BLOCKS_TO_INITIAL_SEARCH);
        loop {
            debug!("Handle channel messages");
            if let Err(err) = self.handle_messages().await {
                warn!("Failed to handle channel messages: {}", err);
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}
