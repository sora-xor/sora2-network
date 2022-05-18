use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::justification::*;
use crate::ethereum::SignedClientInner;
use crate::prelude::*;
use crate::relay::simplified_proof::convert_to_simplified_mmr_proof;
use crate::substrate::LeafProof;
use beefy_merkle_tree::Keccak256;
use beefy_primitives::VersionedFinalityProof;
use bridge_types::types::{AuxiliaryDigest, AuxiliaryDigestItem, ChannelId};
use bridge_types::EthNetworkId;
use ethereum_gen::{
    basic_inbound_channel as basic, beefy_light_client,
    incentivized_inbound_channel as incentivized, BasicInboundChannel, BeefyLightClient,
    IncentivizedInboundChannel,
};
use ethers::abi::RawLog;
use ethers::prelude::builders::ContractCall;
use ethers::prelude::*;

#[derive(Default)]
pub struct RelayBuilder {
    sub: Option<SubUnsignedClient>,
    eth: Option<EthSignedClient>,
    beefy: Option<Address>,
    basic: Option<Address>,
    incentivized: Option<Address>,
}

impl RelayBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_substrate_client(mut self, sub: SubUnsignedClient) -> Self {
        self.sub = Some(sub);
        self
    }

    pub fn with_ethereum_client(mut self, eth: EthSignedClient) -> Self {
        self.eth = Some(eth);
        self
    }

    pub fn with_beefy_contract(mut self, address: Address) -> Self {
        self.beefy = Some(address);
        self
    }

    pub fn with_basic_contract(mut self, address: Address) -> Self {
        self.basic = Some(address);
        self
    }

    pub fn with_incentivized_contract(mut self, address: Address) -> Self {
        self.incentivized = Some(address);
        self
    }

    pub async fn build(self) -> AnyResult<Relay> {
        let sub = self.sub.expect("substrate client is needed");
        let eth = self.eth.expect("ethereum client is needed");
        let beefy = BeefyLightClient::new(
            self.beefy.expect("beefy contract address is needed"),
            eth.inner(),
        );
        let basic = BasicInboundChannel::new(
            self.basic.expect("basic channel address is needed"),
            eth.inner(),
        );
        let incentivized = IncentivizedInboundChannel::new(
            self.incentivized
                .expect("incentivized channel address is needed"),
            eth.inner(),
        );
        let blocks_until_finalized = beefy.block_wait_period().call().await?;
        let beefy_start_block = sub.beefy_start_block().await?;
        let basic_gas_per_message = basic.max_gas_per_message().call().await?.as_u64();
        let incentivized_gas_per_message =
            incentivized.max_gas_per_message().call().await?.as_u64();
        Ok(Relay {
            chain_id: eth.inner().get_chainid().await?.as_u32(),
            sub,
            eth,
            beefy,
            beefy_start_block,
            blocks_until_finalized,
            basic_channel: basic,
            incentivized_channel: incentivized,
            basic_gas_per_message,
            incentivized_gas_per_message,
            lost_gas: Default::default(),
            successful_sent: Default::default(),
            failed_to_sent: Default::default(),
        })
    }
}

#[derive(Clone)]
pub struct Relay {
    sub: SubUnsignedClient,
    eth: EthSignedClient,
    beefy: BeefyLightClient<SignedClientInner>,
    basic_channel: BasicInboundChannel<SignedClientInner>,
    incentivized_channel: IncentivizedInboundChannel<SignedClientInner>,
    beefy_start_block: u64,
    blocks_until_finalized: u64,
    chain_id: EthNetworkId,
    basic_gas_per_message: u64,
    incentivized_gas_per_message: u64,
    lost_gas: Arc<AtomicU64>,
    successful_sent: Arc<AtomicU64>,
    failed_to_sent: Arc<AtomicU64>,
}

impl Relay {
    async fn new_signature_commitment(
        &self,
        justification: &BeefyJustification,
    ) -> AnyResult<ContractCall<SignedClientInner, ()>> {
        let initial_bit_field = self
            .beefy
            .create_initial_bitfield(
                justification.signed_validators.clone(),
                justification.num_validators,
            )
            .legacy()
            .call()
            .await?;
        let pos = justification.signed_validators[0];
        let pos_usize = pos.as_usize();
        let pubkey = justification.validator_pubkey(pos_usize);
        let proof = justification.validator_pubkey_proof(pos_usize);
        let validator_signature = justification.validator_eth_signature(pos_usize);

        let mut call = self.beefy.new_signature_commitment(
            justification.commitment_hash.into(),
            initial_bit_field,
            validator_signature.into(),
            pos,
            pubkey,
            proof,
        );
        call.tx.set_from(self.eth.address());
        Ok(call)
    }

    async fn create_random_bitfield(&self, id: U256) -> AnyResult<Vec<U256>> {
        let call = self.beefy.create_random_bitfield(id).legacy();
        let random_bitfield = call.call().await?;
        debug!("Random bitfield {}: {:?}", id, random_bitfield);
        Ok(random_bitfield)
    }

    async fn complete_signature_commitment(
        &self,
        id: U256,
        justification: &BeefyJustification,
    ) -> AnyResult<ContractCall<SignedClientInner, ()>> {
        let (prefix, payload, suffix) = justification.get_payload().expect("should be checked");
        let eth_commitment = beefy_light_client::Commitment {
            payload_prefix: prefix.into(),
            payload,
            payload_suffix: suffix.into(),
            block_number: justification.commitment.block_number as u32,
            validator_set_id: justification.commitment.validator_set_id as u64,
        };

        let random_bitfield = self.create_random_bitfield(id).await?;
        let validator_proof = justification.validators_proof(random_bitfield);
        let (latest_mmr_leaf, proof) = justification.simplified_mmr_proof()?;

        let mut call = self.beefy.complete_signature_commitment(
            id,
            eth_commitment,
            validator_proof,
            latest_mmr_leaf,
            proof,
        );
        call.tx.set_from(self.eth.address());
        Ok(call)
    }

    pub async fn call_with_event<E: EthEvent>(
        &self,
        name: &str,
        call: ContractCall<SignedClientInner, ()>,
    ) -> AnyResult<E> {
        debug!("Call '{}' check", name);
        call.call().await?;
        self.eth.save_gas_price(&call, "relay").await?;
        debug!("Call '{}' send", name);
        let tx = call
            .send()
            .await?
            .confirmations(self.blocks_until_finalized as usize + 1)
            .await?
            .expect("failed");
        debug!("Call '{}' finalized: {:?}", name, tx);
        if tx.status.unwrap().as_u32() == 0 {
            self.lost_gas
                .fetch_add(tx.gas_used.unwrap_or_default().as_u64(), Ordering::Relaxed);
            self.failed_to_sent.fetch_add(1, Ordering::Relaxed);
            return Err(anyhow::anyhow!("Tx failed"));
        }
        let success_event = tx
            .logs
            .iter()
            .find_map(|log| {
                let raw_log = RawLog {
                    topics: log.topics.clone(),
                    data: log.data.to_vec(),
                };
                E::decode_log(&raw_log).ok()
            })
            .expect("should have");
        Ok(success_event)
    }

    pub async fn send_commitment(self, justification: BeefyJustification) -> AnyResult<()> {
        if false && self.tx_pool_contains_tx().await? {
            return Ok(());
        }
        debug!("New justification: {:?}", justification);
        let call = self.new_signature_commitment(&justification).await?;
        let event = self
            .call_with_event::<beefy_light_client::InitialVerificationSuccessfulFilter>(
                "New signature commitment",
                call,
            )
            .await?;
        let call = self
            .complete_signature_commitment(event.id, &justification)
            .await?;
        let _event = self
            .call_with_event::<beefy_light_client::FinalVerificationSuccessfulFilter>(
                "Complete signature commitment",
                call,
            )
            .await?;
        self.handle_complete_commitment_success().await?;
        Ok(())
    }

    async fn send_messages_from_block(
        &self,
        block_number: u32,
        latest_hash: H256,
    ) -> AnyResult<()> {
        const INDEXING_PREFIX: &'static [u8] = b"commitment";
        let block_hash = self
            .sub
            .api()
            .client
            .rpc()
            .block_hash(Some(block_number.into()))
            .await?
            .expect("should exist");
        let header = self
            .sub
            .api()
            .client
            .rpc()
            .header(Some(block_hash))
            .await?
            .expect("should exist");
        let digest = AuxiliaryDigest::from(header.digest.clone());
        if digest.logs.is_empty() {
            return Ok(());
        }
        let digest_encoded = digest.encode();
        let digest_hash = hex::encode(&Keccak256::hash(&digest_encoded));
        debug!("Digest hash: {}", digest_hash);
        let LeafProof { leaf, proof, .. } = self
            .sub
            .mmr_generate_proof(
                block_number as u64 - self.beefy_start_block,
                Some(latest_hash),
            )
            .await?;
        let leaf_encoded = hex::encode(&leaf.encode());
        debug!("Leaf: {}", leaf_encoded);
        let leaf_prefix: Bytes =
            hex::decode(leaf_encoded.strip_suffix(&digest_hash).unwrap())?.into();
        let digest_hex = hex::encode(&digest_encoded);
        debug!("Digest: {}", digest_hex);

        let proof =
            convert_to_simplified_mmr_proof(proof.leaf_index, proof.leaf_count, proof.items);
        let proof = beefy_light_client::SimplifiedMMRProof {
            merkle_proof_items: proof.items.iter().map(|x| x.0).collect(),
            merkle_proof_order_bit_field: proof.order,
        };
        let basic_nonce = self.basic_channel.nonce().call().await?;
        let incentivized_nonce = self.incentivized_channel.nonce().call().await?;

        for log in digest.logs {
            let AuxiliaryDigestItem::Commitment(chain_id, id, messages_hash) = log;
            if chain_id != self.chain_id {
                continue;
            }
            let delimiter = (chain_id, id, messages_hash).encode();
            let (digest_prefix, digest_suffix) =
                digest_hex.split_once(&hex::encode(delimiter)).unwrap();
            let digest_prefix = hex::decode(digest_prefix)?.into();
            let digest_suffix = hex::decode(digest_suffix)?.into();

            let key = (INDEXING_PREFIX, id, messages_hash).encode();
            if let Some(data) = self
                .sub
                .offchain_local_get(crate::substrate::StorageKind::Persistent, key)
                .await?
            {
                let (mut call, messages_count) = match id {
                    ChannelId::Basic => {
                        let messages_sub = Vec::<
                            substrate_gen::runtime::runtime_types::basic_channel::outbound::Message,
                        >::decode(&mut &*data)?;
                        if messages_sub
                            .iter()
                            .all(|message| message.nonce <= basic_nonce)
                        {
                            continue;
                        }
                        let mut messages = vec![];
                        for message in messages_sub {
                            messages.push(basic::Message {
                                target: message.target,
                                nonce: message.nonce,
                                payload: message.payload.into(),
                            });
                        }

                        let leaf_bytes = basic::LeafBytes {
                            digest_prefix,
                            digest_suffix,
                            leaf_prefix: leaf_prefix.clone(),
                        };
                        let messages_count = messages.len();
                        let call = self
                            .basic_channel
                            .submit(messages, leaf_bytes, proof.clone());
                        (call, messages_count)
                    }
                    ChannelId::Incentivized => {
                        let messages_sub = Vec::<substrate_gen::runtime::runtime_types::incentivized_channel::outbound::Message>::decode(&mut &*data)?;
                        if messages_sub
                            .iter()
                            .all(|message| message.nonce <= incentivized_nonce)
                        {
                            continue;
                        }
                        let mut messages = vec![];
                        for message in messages_sub {
                            messages.push(incentivized::Message {
                                target: message.target,
                                nonce: message.nonce,
                                payload: message.payload.into(),
                                fee: message.fee,
                            });
                        }
                        let leaf_bytes = incentivized::LeafBytes {
                            digest_prefix,
                            digest_suffix,
                            leaf_prefix: leaf_prefix.clone(),
                        };
                        let messages_count = messages.len();
                        let call =
                            self.incentivized_channel
                                .submit(messages, leaf_bytes, proof.clone());
                        (call, messages_count)
                    }
                };
                debug!("Fill submit messages from {:?}", id);
                self.eth.fill_transaction(&mut call.tx, call.block).await?;
                call.tx.set_gas(self.submit_message_gas(id, messages_count));
                debug!("Check submit messages from {:?}", id);
                call.call().await?;
                self.eth.save_gas_price(&call, "submit-messages").await?;
                debug!("Send submit messages from {:?}", id);
                let tx = call.send().await?;
                debug!(
                    "Wait for confirmations submit messages from {:?}, {:?}",
                    id, tx
                );
                let tx = tx.confirmations(3).await?;
                debug!("Submit messages from {:?}: {:?}", id, tx);
                if let Some(tx) = tx {
                    for log in tx.logs {
                        let raw_log = RawLog {
                            topics: log.topics.clone(),
                            data: log.data.to_vec(),
                        };
                        if let Ok(log) =
                            <basic::MessageDispatchedFilter as EthLogDecode>::decode_log(&raw_log)
                        {
                            info!("Message dispatched: {:?}", log);
                        } else if let Ok(log) =
                            <incentivized::MessageDispatchedFilter as EthLogDecode>::decode_log(
                                &raw_log,
                            )
                        {
                            info!("Message dispatched: {:?}", log);
                        } else if let Ok(log) =
                            <ethereum_gen::MigratedEthFilter as EthLogDecode>::decode_log(&raw_log)
                        {
                            info!("Migrated eth: {:?}", log);
                        } else if let Ok(log) =
                            <ethereum_gen::MigratedNativeErc20Filter as EthLogDecode>::decode_log(
                                &raw_log,
                            )
                        {
                            info!("Migrated erc20: {:?}", log);
                        } else if let Ok(log) =
                            <ethereum_gen::MigratedSidechainFilter as EthLogDecode>::decode_log(
                                &raw_log,
                            )
                        {
                            info!("Migrated sidechain: {:?}", log);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn handle_complete_commitment_success(self) -> AnyResult<()> {
        self.successful_sent.fetch_add(1, Ordering::Relaxed);
        let latest_block = self.beefy.latest_beefy_block().call().await? as u32;
        let latest_hash = self
            .sub
            .api()
            .client
            .rpc()
            .block_hash(Some(latest_block.into()))
            .await?
            .unwrap();
        if self.check_new_messages(latest_block as u32 - 1).await? {
            let start_messages_block = self.find_start_block(latest_block).await?;
            for block_number in start_messages_block..=latest_block {
                self.send_messages_from_block(block_number, latest_hash)
                    .await?;
            }
        }
        Ok(())
    }

    fn submit_message_gas(&self, channel: ChannelId, messages: usize) -> u64 {
        let max_gas_per_message = match channel {
            ChannelId::Basic => self.basic_gas_per_message,
            ChannelId::Incentivized => self.incentivized_gas_per_message,
        };
        260000 + max_gas_per_message * messages as u64
    }

    pub async fn find_start_block(&self, mut block_number: u32) -> AnyResult<u32> {
        let basic_interval = self
            .sub
            .api()
            .storage()
            .basic_outbound_channel()
            .interval(None)
            .await?;
        let incentivized_interval = self
            .sub
            .api()
            .storage()
            .incentivized_outbound_channel()
            .interval(None)
            .await?;
        let basic_mod = self.chain_id % basic_interval;
        let incentivized_mod = self.chain_id % incentivized_interval;
        while block_number > 0 {
            if block_number % basic_interval == basic_mod
                && block_number % incentivized_interval == incentivized_mod
                && !self.check_new_messages(block_number - 1).await?
            {
                break;
            }
            block_number -= 1;
        }
        Ok(block_number)
    }

    pub async fn check_new_messages(&self, block_number: u32) -> AnyResult<bool> {
        let basic_nonce = self.basic_channel.nonce().call().await?;
        let incentivized_nonce = self.incentivized_channel.nonce().call().await?;
        let block_hash = self
            .sub
            .api()
            .client
            .rpc()
            .block_hash(Some(block_number.into()))
            .await?;
        let sub_basic_nonce = self
            .sub
            .api()
            .storage()
            .basic_outbound_channel()
            .channel_nonces(&self.chain_id, block_hash)
            .await?;
        let sub_incentivized_nonce = self
            .sub
            .api()
            .storage()
            .incentivized_outbound_channel()
            .channel_nonces(&self.chain_id, block_hash)
            .await?;
        Ok(basic_nonce < sub_basic_nonce || incentivized_nonce < sub_incentivized_nonce)
    }

    pub async fn tx_pool_contains_tx(&self) -> AnyResult<bool> {
        let pool = self.eth.inner().txpool_content().await?;
        for (_sender, pending) in pool.pending {
            for (_nonce, pending) in pending {
                let to = if let Some(NameOrAddress::Address(to)) = pending.to {
                    to
                } else {
                    continue;
                };
                if to != self.beefy.address() {
                    continue;
                }
                let data = if let Some(data) = pending.data {
                    data
                } else {
                    continue;
                };
                if data.0.len() < 4 {
                    continue;
                }
                if data.as_ref()
                    != ethereum_gen::beefy_light_client::CompleteSignatureCommitmentCall::selector()
                        .as_slice()
                {
                    continue;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn sync_historical_commitments(&self) -> AnyResult<()> {
        let beefy_block_gap = self.beefy.maximum_block_gap().call().await?;
        let epoch_duration = self.sub.api().constants().babe().epoch_duration()?;
        let sessions_per_era = self.sub.api().constants().staking().sessions_per_era()?;
        let era_duration = epoch_duration * sessions_per_era as u64;
        'main_loop: loop {
            let latest_beefy_block = self.beefy.latest_beefy_block().call().await?;
            let latest_beefy_block_hash = self.sub.block_hash(Some(latest_beefy_block)).await?;
            let latest_era = self
                .sub
                .api()
                .storage()
                .staking()
                .active_era(Some(latest_beefy_block_hash))
                .await?
                .expect("should exist");
            let current_block_hash = self.sub.api().client.rpc().finalized_head().await?;
            let current_block = self.sub.block_number(Some(current_block_hash)).await?;
            let next_block = latest_beefy_block + beefy_block_gap.min(era_duration + 1);
            if next_block > current_block as u64 {
                return Ok(());
            }
            let next_block_hash = self.sub.block_hash(Some(next_block)).await?;
            let next_eras = self
                .sub
                .api()
                .storage()
                .staking()
                .bonded_eras(Some(next_block_hash))
                .await?;
            debug!("latest era: {latest_era:?}, next block: {next_block}, eras: {next_eras:?}");
            let next_block = if let Some((_, session)) = next_eras
                .into_iter()
                .find(|(index, _)| index > &latest_era.index)
            {
                session as u64 * epoch_duration + 1
            } else {
                next_block
            };
            debug!(
                "latest beefy block: {}, next block: {}",
                latest_beefy_block, next_block
            );
            for next_block in ((latest_beefy_block + 1)..=next_block).rev() {
                let block = self.sub.block(Some(next_block)).await?;
                debug!("Check block {:?}", block.block.header.number);
                if let Some(justifications) = block.justifications {
                    for (engine, justification) in justifications {
                        if &engine == b"BEEF" {
                            let VersionedFinalityProof::V1(commitment) =
                                VersionedFinalityProof::decode(&mut justification.as_slice())?;
                            let justification = BeefyJustification::create(
                                self.sub.clone(),
                                commitment,
                                self.beefy_start_block as u32,
                            )
                            .await?;
                            debug!("Justification: {:?}", justification);

                            let _ =
                                self.clone()
                                    .send_commitment(justification)
                                    .await
                                    .map_err(|err| {
                                        warn!("failed to send: {}", err);
                                        err
                                    });
                            info!(
                                "failed: {}, lost gas: {}, successfull: {}",
                                self.failed_to_sent.load(Ordering::Relaxed),
                                self.lost_gas.load(Ordering::Relaxed),
                                self.successful_sent.load(Ordering::Relaxed)
                            );
                            continue 'main_loop;
                        }
                    }
                }
            }
            return Err(anyhow::anyhow!("Justification not found"));
        }
    }

    pub async fn run(&self, ignore_unneeded_commitments: bool) -> AnyResult<()> {
        let beefy_block_gap = self.beefy.maximum_block_gap().call().await?;
        self.sync_historical_commitments().await?;
        let mut beefy_sub = self.sub.subscribe_beefy().await?;
        while let Some(encoded_commitment) = beefy_sub.next().await.transpose()? {
            let justification = BeefyJustification::create(
                self.sub.clone(),
                encoded_commitment.decode()?,
                self.beefy_start_block as u32,
            )
            .await?;
            if !justification.is_supported() {
                continue;
            }
            let latest_block = self.beefy.latest_beefy_block().call().await?;
            let has_messages = self
                .check_new_messages(justification.commitment.block_number - 1)
                .await?;
            let should_send = !ignore_unneeded_commitments
                || has_messages
                || (justification.commitment.block_number as u64
                    > latest_block + beefy_block_gap - 10);
            if should_send {
                // TODO: Better async message handler
                let _ = self
                    .clone()
                    .send_commitment(justification)
                    .await
                    .map_err(|e| {
                        warn!("Send commitment error: {}", e);
                    });
                info!(
                    "failed: {}, lost gas: {}, successfull: {}",
                    self.failed_to_sent.load(Ordering::Relaxed),
                    self.lost_gas.load(Ordering::Relaxed),
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
