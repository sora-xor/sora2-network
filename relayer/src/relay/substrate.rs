use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::justification::*;
use crate::ethereum::SignedClientInner;
use crate::prelude::*;
use crate::relay::simplified_proof::convert_to_simplified_mmr_proof;
use crate::substrate::{EncodedBeefyCommitment, LeafProof};
use beefy_gadget_rpc::BeefyApiClient;
use beefy_merkle_tree::Keccak256;
use beefy_primitives::VersionedFinalityProof;
use bridge_types::types::AuxiliaryDigestItem;
use bridge_types::{EVMChainId, GenericNetworkId};
use ethereum_gen::{beefy_light_client, inbound_channel, BeefyLightClient, InboundChannel};
use ethers::abi::RawLog;
use ethers::prelude::builders::ContractCall;
use ethers::prelude::*;

#[derive(Default)]
pub struct RelayBuilder {
    sub: Option<SubUnsignedClient<MainnetConfig>>,
    eth: Option<EthSignedClient>,
    beefy: Option<Address>,
    inbound_channel: Option<Address>,
}

impl RelayBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_substrate_client(mut self, sub: SubUnsignedClient<MainnetConfig>) -> Self {
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

    pub fn with_inbound_channel_contract(mut self, address: Address) -> Self {
        self.inbound_channel = Some(address);
        self
    }

    pub async fn build(self) -> AnyResult<Relay> {
        let sub = self.sub.expect("substrate client is needed");
        let eth = self.eth.expect("ethereum client is needed");
        let beefy = BeefyLightClient::new(
            self.beefy.expect("beefy contract address is needed"),
            eth.inner(),
        );
        let inbound_channel = InboundChannel::new(
            self.inbound_channel
                .expect("inbound channel address is needed"),
            eth.inner(),
        );
        Ok(Relay {
            chain_id: eth.inner().get_chainid().await?,
            sub,
            eth,
            beefy,
            inbound_channel,
            lost_gas: Default::default(),
            successful_sent: Default::default(),
            failed_to_sent: Default::default(),
        })
    }
}

#[derive(Clone)]
pub struct Relay {
    sub: SubUnsignedClient<MainnetConfig>,
    eth: EthSignedClient,
    beefy: BeefyLightClient<SignedClientInner>,
    inbound_channel: InboundChannel<SignedClientInner>,
    chain_id: EVMChainId,
    lost_gas: Arc<AtomicU64>,
    successful_sent: Arc<AtomicU64>,
    failed_to_sent: Arc<AtomicU64>,
}

impl Relay {
    async fn create_random_bitfield(
        &self,
        initial_bitfield: Vec<U256>,
        num_validators: U256,
    ) -> AnyResult<Vec<U256>> {
        let call = self
            .beefy
            .create_random_bitfield(initial_bitfield, num_validators)
            .legacy();
        let random_bitfield = call.call().await?;
        debug!("Random bitfield: {:?}", random_bitfield);
        Ok(random_bitfield)
    }

    async fn submit_signature_commitment(
        &self,
        justification: &BeefyJustification<MainnetConfig>,
    ) -> AnyResult<ContractCall<SignedClientInner, ()>> {
        let initial_bitfield = self
            .beefy
            .create_initial_bitfield(
                justification.signed_validators.clone(),
                justification.num_validators,
            )
            .legacy()
            .call()
            .await?;

        let eth_commitment = beefy_light_client::Commitment {
            payload_prefix: justification.payload.prefix.clone().into(),
            payload: justification.payload.mmr_root.into(),
            payload_suffix: justification.payload.suffix.clone().into(),
            block_number: justification.commitment.block_number,
            validator_set_id: justification.commitment.validator_set_id as u64,
        };

        let random_bitfield = self
            .create_random_bitfield(initial_bitfield.clone(), justification.num_validators)
            .await?;
        let validator_proof = justification.validators_proof(initial_bitfield, random_bitfield);
        let (latest_mmr_leaf, proof) = justification.simplified_mmr_proof()?;

        let mut call = self
            .beefy
            .submit_signature_commitment(eth_commitment, validator_proof, latest_mmr_leaf, proof)
            .legacy();
        call.tx.set_from(self.eth.address());
        Ok(call)
    }

    pub async fn call_with_event<E: EthEvent>(
        &self,
        name: &str,
        call: ContractCall<SignedClientInner, ()>,
        confirmations: usize,
    ) -> AnyResult<E> {
        debug!("Call '{}' check", name);
        call.call().await?;
        debug!("Call '{}' estimate gas", name);
        self.eth.save_gas_price(&call, "relay").await?;
        debug!("Call '{}' send", name);
        let tx = call
            .send()
            .await?
            .confirmations(confirmations)
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

    pub async fn send_commitment(
        self,
        justification: BeefyJustification<MainnetConfig>,
    ) -> AnyResult<()> {
        debug!("New justification: {:?}", justification);
        let call = self.submit_signature_commitment(&justification).await?;
        let _event = self
            .call_with_event::<beefy_light_client::VerificationSuccessfulFilter>(
                "Complete signature commitment",
                call,
                1,
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
        let block_hash = self
            .sub
            .api()
            .rpc()
            .block_hash(Some(block_number.into()))
            .await?
            .expect("should exist");
        let digest = self.sub.auxiliary_digest(Some(block_hash)).await?;
        if digest.logs.is_empty() {
            return Ok(());
        }
        let digest_encoded = digest.encode();
        let digest_hash = hex::encode(&Keccak256::hash(&digest_encoded));
        debug!("Digest hash: {}", digest_hash);
        let LeafProof { leaf, proof, .. } = self
            .sub
            .mmr_generate_proof(block_number, Some(latest_hash))
            .await?;
        let leaf_encoded = hex::encode(&leaf.encode());
        debug!("Leaf: {}", leaf_encoded);
        let leaf_prefix: Bytes =
            hex::decode(leaf_encoded.strip_suffix(&digest_hash).unwrap())?.into();
        let digest_hex = hex::encode(&digest_encoded);
        debug!("Digest: {}", digest_hex);

        let proof =
            convert_to_simplified_mmr_proof(proof.leaf_index, proof.leaf_count, &proof.items);
        let proof = beefy_light_client::SimplifiedMMRProof {
            merkle_proof_items: proof.items.iter().map(|x| x.0).collect(),
            merkle_proof_order_bit_field: proof.order,
        };
        let inbound_channel_nonce = self.inbound_channel.nonce().call().await?;

        for log in digest.logs {
            let AuxiliaryDigestItem::Commitment(chain_id, commitment_hash) = log;
            if chain_id != GenericNetworkId::EVM(self.chain_id) {
                continue;
            }
            let delimiter = (chain_id, commitment_hash).encode();
            let (digest_prefix, digest_suffix) =
                digest_hex.split_once(&hex::encode(delimiter)).unwrap();
            let digest_prefix = hex::decode(digest_prefix)?.into();
            let digest_suffix = hex::decode(digest_suffix)?.into();
            let commitment_sub = self.sub.bridge_commitments(commitment_hash).await?;
            if commitment_sub
                .messages
                .iter()
                .all(|message| message.nonce <= inbound_channel_nonce)
            {
                continue;
            }
            let mut messages = vec![];
            for message in commitment_sub.messages {
                messages.push(inbound_channel::Message {
                    target: message.target,
                    nonce: message.nonce,
                    payload: message.payload.into(),
                    fee: message.fee,
                    max_gas: message.max_gas,
                });
            }
            let batch = inbound_channel::Batch {
                total_max_gas: commitment_sub.total_max_gas,
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
                .submit(batch, leaf_bytes, proof.clone())
                .legacy();

            debug!("Fill submit messages");
            self.eth.fill_transaction(&mut call.tx, call.block).await?;
            debug!("Messages total gas: {}", messages_total_gas);
            call.tx.set_gas(self.submit_message_gas(messages_total_gas));
            debug!("Check submit messages");
            call.call().await?;
            self.eth.save_gas_price(&call, "submit-messages").await?;
            debug!("Send submit messages");
            let tx = call.send().await?;
            debug!("Wait for confirmations submit messages: {:?}", tx);
            let tx = tx.confirmations(3).await?;
            debug!("Submit messages: {:?}", tx);
            if let Some(tx) = tx {
                for log in tx.logs {
                    let raw_log = RawLog {
                        topics: log.topics.clone(),
                        data: log.data.to_vec(),
                    };
                    if let Ok(log) =
                        <inbound_channel::MessageDispatchedFilter as EthLogDecode>::decode_log(
                            &raw_log,
                        )
                    {
                        info!("Message dispatched: {:?}", log);
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
            .rpc()
            .block_hash(Some(latest_block.into()))
            .await?
            .unwrap();
        if self.check_new_messages(latest_block - 1).await? {
            let start_messages_block = self.find_start_block(latest_block).await?;
            for block_number in start_messages_block..=latest_block {
                self.send_messages_from_block(block_number, latest_hash)
                    .await?;
            }
        }
        Ok(())
    }

    fn submit_message_gas(&self, messages_total_gas: U256) -> U256 {
        messages_total_gas.saturating_add(260000.into())
    }

    pub async fn find_start_block(&self, mut block_number: u32) -> AnyResult<u32> {
        let channel_interval = self
            .sub
            .api()
            .storage()
            .fetch_or_default(
                &runtime::storage().bridge_outbound_channel().interval(),
                None,
            )
            .await?;
        let chain_mod = self.chain_id % channel_interval;
        while block_number > 0 {
            if block_number % channel_interval == chain_mod.as_u32()
                && !self.check_new_messages(block_number - 1).await?
            {
                break;
            }
            block_number -= 1;
        }
        Ok(block_number)
    }

    pub async fn check_new_messages(&self, block_number: u32) -> AnyResult<bool> {
        let channel_nonce = self.inbound_channel.nonce().call().await?;
        let block_hash = self
            .sub
            .api()
            .rpc()
            .block_hash(Some(block_number.into()))
            .await?;
        let sub_channel_nonce = self
            .sub
            .api()
            .storage()
            .fetch_or_default(
                &runtime::storage()
                    .bridge_outbound_channel()
                    .channel_nonces(&self.chain_id),
                block_hash,
            )
            .await?;
        Ok(channel_nonce < sub_channel_nonce)
    }

    pub async fn sync_historical_commitments(&self) -> AnyResult<()> {
        let beefy_block_gap = self.beefy.maximum_block_gap().call().await? - 1;
        let epoch_duration = self
            .sub
            .api()
            .constants()
            .at(&runtime::constants().babe().epoch_duration())?;
        let sessions_per_era = self
            .sub
            .api()
            .constants()
            .at(&runtime::constants().staking().sessions_per_era())?;
        let era_duration = epoch_duration * sessions_per_era as u64;
        'main_loop: loop {
            let latest_beefy_block = self.beefy.latest_beefy_block().call().await?;
            let latest_beefy_block_hash = self
                .sub
                .api()
                .rpc()
                .block_hash(Some(latest_beefy_block.into()))
                .await?
                .ok_or(anyhow!("block hash not found"))?;
            let latest_era = self
                .sub
                .api()
                .storage()
                .fetch(
                    &runtime::storage().staking().active_era(),
                    Some(latest_beefy_block_hash),
                )
                .await?
                .expect("should exist");
            let current_block_hash = self.sub.api().rpc().finalized_head().await?;
            let current_block = self.sub.block_number(Some(current_block_hash)).await?;
            let next_block = latest_beefy_block + beefy_block_gap.min(era_duration + 1);
            if next_block > current_block as u64 {
                return Ok(());
            }
            let next_block_hash = self
                .sub
                .api()
                .rpc()
                .block_hash(Some(next_block.into()))
                .await?
                .ok_or(anyhow!("block hash not found"))?;
            let next_eras = self
                .sub
                .api()
                .storage()
                .fetch_or_default(
                    &runtime::storage().staking().bonded_eras(),
                    Some(next_block_hash),
                )
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
            let current_validator_set_id =
                self.beefy.current_validator_set().call().await?.0 as u64;
            let next_validator_set_id = self.beefy.next_validator_set().call().await?.0 as u64;
            for next_block in ((latest_beefy_block + 1)..=next_block).rev() {
                let next_block_hash = self
                    .sub
                    .api()
                    .rpc()
                    .block_hash(Some(next_block.into()))
                    .await?
                    .ok_or(anyhow!("block hash not found"))?;
                let block = self
                    .sub
                    .api()
                    .rpc()
                    .block(Some(next_block_hash))
                    .await?
                    .ok_or(anyhow!("block not found: {}", next_block))?;
                debug!("Check block {:?}", block.block.header.number);
                if let Some(justifications) = block.justifications {
                    for (engine, justification) in justifications {
                        if &engine == b"BEEF" {
                            let commitment =
                                VersionedFinalityProof::decode(&mut justification.as_slice())?;
                            let justification = match BeefyJustification::create(
                                self.sub.clone(),
                                commitment,
                            )
                            .await
                            {
                                Ok(justification) => justification,
                                Err(err) => {
                                    warn!("failed to create justification: {}", err);
                                    continue;
                                }
                            };
                            debug!("Justification: {:?}", justification);
                            if justification.commitment.validator_set_id != current_validator_set_id
                                && justification.commitment.validator_set_id
                                    != next_validator_set_id
                            {
                                warn!(
                                    "validator set id mismatch: {} + 1 != {}",
                                    justification.commitment.validator_set_id,
                                    current_validator_set_id
                                );
                                continue;
                            }

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
        let beefy_block_gap = self
            .beefy
            .maximum_block_gap()
            .call()
            .await
            .context("fetch beefy maximum_block_gap")?;
        self.sync_historical_commitments()
            .await
            .context("sync historical commitments")?;
        let mut beefy_sub = self.sub.beefy().subscribe_justifications().await?;
        while let Some(encoded_commitment) = beefy_sub.next().await.transpose()? {
            let justification = match BeefyJustification::<MainnetConfig>::create(
                self.sub.clone(),
                EncodedBeefyCommitment::decode::<MainnetConfig>(&encoded_commitment)?,
            )
            .await
            {
                Ok(justification) => justification,
                Err(err) => {
                    warn!("failed to create justification: {}", err);
                    continue;
                }
            };

            let latest_block = self.beefy.latest_beefy_block().call().await?;

            let has_messages = self
                .check_new_messages(justification.commitment.block_number - 1)
                .await?;

            let next_validator_set_id = self.beefy.next_validator_set().call().await?.0 as u64;
            let is_mandatory =
                next_validator_set_id < justification.leaf_proof.leaf.beefy_next_authority_set.id;

            let should_send = !ignore_unneeded_commitments
                || has_messages
                || is_mandatory
                || (justification.commitment.block_number as u64
                    > latest_block + beefy_block_gap - 20);

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
