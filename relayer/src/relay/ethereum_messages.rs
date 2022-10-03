use std::time::Duration;

use bridge_types::types::{Message, Proof};
use bridge_types::EthNetworkId;
use ethers::abi::RawLog;

use crate::ethereum::proof_loader::ProofLoader;
use crate::ethereum::receipt::LogEntry;
use crate::prelude::*;
use ethers::prelude::*;

const BLOCKS_TO_INITIAL_SEARCH: u64 = 49000; // Ethereum light client keep 50000 blocks

pub struct SubstrateMessagesRelay {
    sub: SubSignedClient,
    eth: EthUnsignedClient,
    network_id: EthNetworkId,
    outbound_channel: H160,
    latest_channel_block: u64,
    proof_loader: ProofLoader,
}

impl SubstrateMessagesRelay {
    pub async fn new(
        sub: SubSignedClient,
        eth: EthUnsignedClient,
        proof_loader: ProofLoader,
    ) -> AnyResult<Self> {
        let network_id = eth.inner().get_chainid().await? as EthNetworkId;
        let outbound_channel = sub
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .bridge_inbound_channel()
                    .channel_addresses(&network_id),
                None,
            )
            .await?
            .ok_or(anyhow::anyhow!("Channel is not registered"))?;
        Ok(Self {
            proof_loader,
            latest_channel_block: 0,
            sub,
            eth,
            network_id,
            outbound_channel,
        })
    }

    pub async fn handle_messages(&mut self) -> AnyResult<()> {
        let eth = self.eth.inner();
        let current_eth_block = self
            .sub
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .ethereum_light_client()
                    .finalized_block(&self.network_id),
                None,
            )
            .await?
            .ok_or(anyhow!("Network is not registered"))?
            .number;
        if current_eth_block < self.latest_channel_block {
            debug!("Skip handling channel messages, current block number is less than latest basic {} < {}", current_eth_block, self.latest_channel_block);
            return Ok(());
        }
        let filter = Filter::new()
            .from_block(self.latest_channel_block)
            .to_block(current_eth_block);
        let filter = ethereum_gen::outbound_channel::MessageFilter::new(filter, &eth);
        let events = filter.query_with_meta().await?;
        let mut sub_nonce = self
            .sub
            .api()
            .storage()
            .fetch_or_default(
                &runtime::storage()
                    .bridge_inbound_channel()
                    .channel_nonces(&self.network_id),
                None,
            )
            .await?;
        debug!(
            "Channel: Found {} events from {} to {}",
            events.len(),
            self.latest_channel_block,
            current_eth_block
        );
        for (event, meta) in events {
            if event.nonce <= sub_nonce || meta.address != self.outbound_channel {
                self.latest_channel_block = meta.block_number.as_u64();
                continue;
            }
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
                    let ev = self
                        .sub
                        .api()
                        .tx()
                        .sign_and_submit_then_watch_default(
                            &runtime::tx()
                                .bridge_inbound_channel()
                                .submit(self.network_id, message),
                            &self.sub,
                        )
                        .await?
                        .wait_for_in_block()
                        .await?
                        .wait_for_success()
                        .await?;
                    info!(
                        "Channel: Message {} included in {:?}",
                        event.nonce,
                        ev.block_hash()
                    );
                    sub_nonce = event.nonce;
                }
            }
            self.latest_channel_block = meta.block_number.as_u64();
        }
        self.latest_channel_block = current_eth_block + 1;
        Ok(())
    }

    async fn make_message(&self, log: Log) -> AnyResult<Message> {
        let block_hash = log.block_hash.unwrap();
        let tx_index = log.transaction_index.unwrap().as_usize();
        let proof = self
            .proof_loader
            .receipt_proof(block_hash, tx_index)
            .await?;
        Ok(Message {
            data: rlp::Encodable::rlp_bytes(&LogEntry::from(&log)).to_vec(),
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
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .ethereum_light_client()
                    .finalized_block(&self.network_id),
                None,
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
