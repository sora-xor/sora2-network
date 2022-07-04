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
    basic: H160,
    incentivized: H160,
    latest_basic_block: u64,
    latest_incentivized_block: u64,
    proof_loader: ProofLoader,
    disable_basic: bool,
    disable_incentivized: bool,
}

impl SubstrateMessagesRelay {
    pub async fn new(
        sub: SubSignedClient,
        eth: EthUnsignedClient,
        proof_loader: ProofLoader,
        disable_basic: bool,
        disable_incentivized: bool,
    ) -> AnyResult<Self> {
        let network_id = eth.inner().get_chainid().await? as EthNetworkId;
        let basic = sub
            .api()
            .storage()
            .basic_inbound_channel()
            .channel_addresses(false, &network_id, None)
            .await?
            .ok_or(anyhow::anyhow!("Channel is not registered"))?;
        let incentivized = sub
            .api()
            .storage()
            .incentivized_inbound_channel()
            .channel_addresses(false, &network_id, None)
            .await?
            .ok_or(anyhow::anyhow!("Channel is not registered"))?;
        Ok(Self {
            proof_loader,
            latest_basic_block: 0,
            latest_incentivized_block: 0,
            sub,
            eth,
            network_id,
            basic,
            incentivized,
            disable_basic,
            disable_incentivized,
        })
    }

    pub async fn handle_basic_messages(&mut self) -> AnyResult<()> {
        let eth = self.eth.inner();
        let current_eth_block = self
            .sub
            .api()
            .storage()
            .ethereum_light_client()
            .finalized_block(false, &self.network_id, None)
            .await?
            .ok_or(anyhow!("Network is not registered"))?
            .number;
        if current_eth_block < self.latest_basic_block {
            debug!("Skip handling basic messages, current block number is less than latest basic {} < {}", current_eth_block, self.latest_basic_block);
            return Ok(());
        }
        let filter = Filter::new()
            .from_block(self.latest_basic_block)
            .to_block(current_eth_block);
        let filter = ethereum_gen::basic_outbound_channel::MessageFilter::new(filter, &eth);
        let events = filter.query_with_meta().await?;
        let mut sub_nonce = self
            .sub
            .api()
            .storage()
            .basic_inbound_channel()
            .channel_nonces(false, &self.network_id, None)
            .await?;
        debug!(
            "Basic: Found {} events from {} to {}",
            events.len(),
            self.latest_incentivized_block,
            current_eth_block
        );
        for (event, meta) in events {
            if event.nonce <= sub_nonce || meta.address != self.basic {
                self.latest_basic_block = meta.block_number.as_u64();
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
                    <ethereum_gen::basic_outbound_channel::MessageFilter as EthEvent>::decode_log(
                        &raw_log,
                    )
                {
                    let message = self.make_message(log).await?;
                    debug!("Basic: Send {} message", event.nonce);
                    let ev = self
                        .sub
                        .api()
                        .tx()
                        .basic_inbound_channel()
                        .submit(false, self.network_id, message)?
                        .sign_and_submit_then_watch_default(&self.sub)
                        .await?
                        .wait_for_in_block()
                        .await?
                        .wait_for_success()
                        .await?;
                    info!(
                        "Basic: Message {} included in {:?}",
                        event.nonce,
                        ev.block_hash()
                    );
                    sub_nonce = event.nonce;
                }
            }
            self.latest_basic_block = meta.block_number.as_u64();
        }
        self.latest_basic_block = current_eth_block + 1;
        Ok(())
    }

    pub async fn handle_incentivized_messages(&mut self) -> AnyResult<()> {
        let eth = self.eth.inner();
        let current_eth_block = self
            .sub
            .api()
            .storage()
            .ethereum_light_client()
            .finalized_block(false, &self.network_id, None)
            .await?
            .ok_or(anyhow!("Network is not registered"))?
            .number;
        if current_eth_block < self.latest_incentivized_block {
            debug!("Skip handling incentivized messages, current block number is less than latest basic {} < {}", current_eth_block, self.latest_basic_block);
            return Ok(());
        }
        let filter = Filter::new()
            .from_block(self.latest_incentivized_block)
            .to_block(current_eth_block);
        let filter = ethereum_gen::incentivized_outbound_channel::MessageFilter::new(filter, &eth);
        let events = filter.query_with_meta().await?;
        let mut sub_nonce = self
            .sub
            .api()
            .storage()
            .incentivized_inbound_channel()
            .channel_nonces(false, &self.network_id, None)
            .await?;
        debug!(
            "Incentivized: Found {} events from {} to {}",
            events.len(),
            self.latest_incentivized_block,
            current_eth_block
        );
        for (event, meta) in events {
            if event.nonce <= sub_nonce || meta.address != self.incentivized {
                self.latest_incentivized_block = meta.block_number.as_u64();
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
                    <ethereum_gen::incentivized_outbound_channel::MessageFilter as EthEvent>::decode_log(&raw_log)
                {
                    let message = self.make_message(log).await?;
                    debug!("Incentivized: Send {} message", event.nonce);
                    let ev = self
                        .sub
                        .api()
                        .tx()
                        .incentivized_inbound_channel()
                        .submit(false, self.network_id, message)?
                        .sign_and_submit_then_watch_default(&self.sub)
                        .await?
                        .wait_for_in_block()
                        .await?
                        .wait_for_success()
                        .await?;
                    info!("Incentivized: Message {} included in {:?}", event.nonce, ev.block_hash());
                    sub_nonce = event.nonce;
                }
            }
            self.latest_incentivized_block = meta.block_number.as_u64();
        }
        self.latest_incentivized_block = current_eth_block + 1;
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
        if self.disable_basic && self.disable_incentivized {
            return Ok(());
        }

        let current_eth_block = self
            .sub
            .api()
            .storage()
            .ethereum_light_client()
            .finalized_block(false, &self.network_id, None)
            .await?
            .ok_or(anyhow!("Network is not registered"))?
            .number;
        self.latest_basic_block = current_eth_block.saturating_sub(BLOCKS_TO_INITIAL_SEARCH);
        self.latest_incentivized_block = current_eth_block.saturating_sub(BLOCKS_TO_INITIAL_SEARCH);
        loop {
            if !self.disable_basic {
                debug!("Handle basic messages");
                if let Err(err) = self.handle_basic_messages().await {
                    warn!("Failed to handle basic messages: {}", err);
                }
            }

            if !self.disable_incentivized {
                debug!("Handle inventivized messages");
                if let Err(err) = self.handle_incentivized_messages().await {
                    warn!("Failed to handle incentivized messages: {}", err);
                }
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}
