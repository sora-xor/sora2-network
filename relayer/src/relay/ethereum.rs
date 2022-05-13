use crate::ethereum::proof_loader::ProofLoader;
use crate::ethereum::receipt::LogEntry;
use crate::ethereum::{make_header, UnsignedClientInner};
use crate::prelude::*;
use bridge_types::types::{ChannelId, Message, Proof};
use bridge_types::{EthNetworkId, HeaderId};
use ethereum_gen::{
    basic_outbound_channel as basic, incentivized_outbound_channel as incentivized,
    BasicOutboundChannel, IncentivizedOutboundChannel,
};
use ethers::abi::RawLog;
use ethers::prelude::*;
use futures::stream::FuturesOrdered;
use futures::TryFutureExt;
use std::path::PathBuf;
use substrate_gen::{runtime, DefaultConfig};
use subxt::TransactionProgress;
use tokio::sync::broadcast::Sender;
use tokio::task::JoinHandle;

#[derive(Debug)]
struct ChannelMessage {
    id: ChannelId,
    message: Message,
}

fn is_channel_message(log: &Log) -> bool {
    let raw_log = RawLog {
        topics: log.topics.clone(),
        data: log.data.to_vec(),
    };
    if let Ok(event) = <basic::MessageFilter as EthLogDecode>::decode_log(&raw_log) {
        debug!("Basic message found: {:?}", event);
        true
    } else if let Ok(event) = <incentivized::MessageFilter as EthLogDecode>::decode_log(&raw_log) {
        debug!("Incentivized message found: {:?}", event);
        true
    } else {
        false
    }
}

#[derive(Clone)]
pub struct Relay {
    finalized_sender: Sender<HeaderId>,
    sub: SubSignedClient,
    eth: EthUnsignedClient,
    proof_loader: ProofLoader,
    basic: BasicOutboundChannel<UnsignedClientInner>,
    incentivized: IncentivizedOutboundChannel<UnsignedClientInner>,
    chain_id: EthNetworkId,
}

impl Relay {
    pub async fn new(
        base_path: PathBuf,
        sub: SubSignedClient,
        eth: EthUnsignedClient,
    ) -> AnyResult<Self> {
        let (sender, _) = tokio::sync::broadcast::channel(32);
        let chain_id = eth.get_chainid().await?.as_u32();
        let basic_contract =
            super::utils::basic_outbound_channel(chain_id, sub.api(), eth.inner()).await?;
        let incentivized_contract =
            super::utils::incentivized_outbound_channel(chain_id, sub.api(), eth.inner()).await?;
        Ok(Self {
            proof_loader: ProofLoader::new(eth.clone(), base_path),
            sub,
            eth,
            finalized_sender: sender,
            basic: basic_contract,
            incentivized: incentivized_contract,
            chain_id,
        })
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

    async fn load_messages_with_filter(&self, filter: Filter) -> AnyResult<Vec<Message>> {
        let logs = self.eth.inner().get_logs(&filter).await?;
        let mut events = vec![];
        for log in logs {
            if is_channel_message(&log) {
                events.push(self.make_message(log).await?);
            }
        }
        Ok(events)
    }

    async fn load_bridge_messages(&self, block_hash: H256) -> AnyResult<Vec<ChannelMessage>> {
        let mut res = vec![];
        let filter = self.incentivized.events().at_block_hash(block_hash).filter;
        res.extend(
            self.load_messages_with_filter(filter)
                .await?
                .into_iter()
                .map(|message| ChannelMessage {
                    message,
                    id: ChannelId::Incentivized,
                }),
        );
        let filter = self.basic.events().at_block_hash(block_hash).filter;
        res.extend(
            self.load_messages_with_filter(filter)
                .await?
                .into_iter()
                .map(|message| ChannelMessage {
                    message,
                    id: ChannelId::Basic,
                }),
        );
        Ok(res)
    }

    async fn send_messages(self, messages: Vec<ChannelMessage>) -> AnyResult<()> {
        if !messages.is_empty() {
            info!("Found {} events", messages.len());
        }
        let mut progresses = vec![];
        for message in messages {
            debug!("Send message: {:?}", message);
            let progress = match message.id {
                ChannelId::Basic => {
                    self.sub
                        .api()
                        .tx()
                        .basic_inbound_channel()
                        .submit(self.chain_id, message.message)
                        .sign_and_submit_then_watch_default(&self.sub)
                        .await?
                }
                ChannelId::Incentivized => {
                    self.sub
                        .api()
                        .tx()
                        .incentivized_inbound_channel()
                        .submit(self.chain_id, message.message)
                        .sign_and_submit_then_watch_default(&self.sub)
                        .await?
                }
            };
            progresses.push(progress);
        }
        for progress in progresses {
            let res = progress.wait_for_finalized_success().await?;
            debug!("Finalized message: {}", res.extrinsic_hash());
        }
        Ok(())
    }

    fn on_finalized_header_worker(self) -> JoinHandle<AnyResult<()>> {
        tokio::spawn(
            {
                let mut receiver = self.finalized_sender.subscribe();
                async move {
                    loop {
                        let value = receiver.recv().await?;
                        info!("Finalized header: {:?}", value);
                        let messages = self.load_bridge_messages(value.hash).await?;
                        if !messages.is_empty() {
                            warn!("Messages found: {:?}", messages);
                            tokio::spawn(self.clone().send_messages(messages).map_err(|e| {
                                error!("Send messages error: {}", e);
                                std::process::exit(1);
                            }));
                        }
                    }
                }
            }
            .map_err(|e: anyhow::Error| {
                error!("Finalized worker error: {}", e);
                std::process::exit(1);
            }),
        )
    }

    pub async fn run(&self) -> AnyResult<()> {
        let _finalized_join = self.clone().on_finalized_header_worker();

        let finalized_block = self
            .sub
            .api()
            .storage()
            .ethereum_light_client()
            .finalized_block(&self.chain_id, None)
            .await?
            .ok_or(anyhow::anyhow!("Network is not registered"))?;

        let latest_block = self
            .eth
            .get_block_number()
            .await
            .context("get block number")?
            .as_u64();

        let mut futures = FuturesOrdered::new();

        let mut current = finalized_block.number + 1;

        debug!("Latest Ethereum block {}", latest_block);
        loop {
            if let Some(block) = self
                .eth
                .get_block(current)
                .await
                .context("get eth block by number")?
            {
                debug!("Preimport block {}", current);
                while futures.len() > 10 {
                    if let Some(result) = futures.next().await {
                        // Rust can't infer type here for some reason
                        let result: Result<(), _> = result;
                        result.context("finalize import header transaction")?;
                    }
                }
                let number = block.number.unwrap_or_default().as_u64();
                let progress = self
                    .process_block(block)
                    .await
                    .context("send import header transaction")?;
                if let Some(progress) = progress {
                    futures.push(self.finalize_transaction(progress, number));
                }
                current += 1;
            } else {
                break;
            }
        }

        let mut watch = self.eth.watch_blocks().await.context("watch blocks")?;
        while let Some(block) = watch.next().await {
            if let Some(block) = self
                .eth
                .get_block(block)
                .await
                .context("get block by hash")?
            {
                debug!("Import block {}", block.number.unwrap_or_default().as_u64());
                while futures.len() > 10 {
                    if let Some(result) = futures.next().await {
                        let result: Result<(), _> = result;
                        result.context("finalize import header transaction")?;
                    }
                }
                let number = block.number.unwrap_or_default().as_u64();
                let progress = self
                    .process_block(block)
                    .await
                    .context("send import header transaction")?;
                if let Some(progress) = progress {
                    futures.push(self.finalize_transaction(progress, number));
                }
            }
        }

        Ok(())
    }

    async fn finalize_transaction<'a>(
        &'a self,
        progress: TransactionProgress<'a, DefaultConfig, runtime::DispatchError, runtime::Event>,
        block_number: u64,
    ) -> AnyResult<()> {
        let events = match progress.wait_for_finalized_success().await {
            Err(subxt::Error::Runtime(subxt::RuntimeError(runtime::DispatchError::Module(
                runtime::runtime_types::sp_runtime::ModuleError { index, error, .. },
            )))) if index == 94 && error == 3u32.to_le_bytes() => {
                warn!("DublicateHeader {}", block_number);
                return Ok(());
            }
            Err(subxt::Error::Rpc(subxt::rpc::RpcError::RequestTimeout)) => {
                warn!("Request timeout {}", block_number);
                return Ok(());
            }
            Err(err) => {
                error!("Failed to import header {}: {}", block_number, err);
                return Err(err.into());
            }
            Ok(x) => x,
        };
        if let Some(event) = events
            .find::<sub_runtime::ethereum_light_client::events::Finalized>()
            .next()
            .transpose()
            .context("find Finalized event")?
        {
            if event.0 == self.chain_id {
                let header_id = event.1;
                debug!("Finalized ethereum header: {:?}", header_id);
                self.finalized_sender
                    .send(header_id)
                    .context("send finalized header id to channel")?;
            }
        }
        Ok(())
    }

    async fn process_block<'a>(
        &'a self,
        block: Block<H256>,
    ) -> AnyResult<
        Option<TransactionProgress<'a, DefaultConfig, runtime::DispatchError, runtime::Event>>,
    > {
        let nonce = block.nonce.unwrap_or_default();
        let header = make_header(block);
        debug!("Process ethereum header: {:?}", header);
        let has_block = self
            .sub
            .api()
            .storage()
            .ethereum_light_client()
            .headers(&self.chain_id, &header.compute_hash(), None)
            .await;
        if let Ok(Some(_)) = has_block {
            return Ok(None);
        }
        let proof = self
            .proof_loader
            .header_proof(header.clone(), nonce)
            .await
            .context("generate header proof")?;
        let result = self
            .sub
            .api()
            .tx()
            .ethereum_light_client()
            .import_header(self.chain_id, header, proof)
            .sign_and_submit_then_watch_default(&self.sub)
            .await
            .context("submit import header extrinsic")?;
        Ok(Some(result))
    }
}
