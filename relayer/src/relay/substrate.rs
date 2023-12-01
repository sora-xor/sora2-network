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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::beefy_syncer::BeefySyncer;
use super::justification::*;
use crate::ethereum::SignedClientInner;
use crate::prelude::*;
use ethereum_gen::{beefy_light_client, BeefyLightClient};
use ethers::abi::RawLog;
use ethers::prelude::builders::ContractCall;
use ethers::prelude::*;

#[derive(Default)]
pub struct RelayBuilder {
    sub: Option<SubUnsignedClient<MainnetConfig>>,
    eth: Option<EthSignedClient>,
    beefy: Option<Address>,
    syncer: Option<BeefySyncer>,
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

    pub fn with_syncer(mut self, syncer: BeefySyncer) -> Self {
        self.syncer = Some(syncer);
        self
    }

    pub fn with_beefy_contract(mut self, address: Address) -> Self {
        self.beefy = Some(address);
        self
    }

    pub async fn build(self) -> AnyResult<Relay> {
        let sub = self.sub.expect("substrate client is needed");
        let eth = self.eth.expect("ethereum client is needed");
        let beefy = BeefyLightClient::new(
            self.beefy.expect("beefy contract address is needed"),
            eth.inner(),
        );
        let syncer = self.syncer.expect("syncer is needed");
        let latest_beefy_block = beefy.latest_beefy_block().call().await?;
        syncer.update_latest_sent(latest_beefy_block);
        Ok(Relay {
            sub,
            eth,
            beefy,
            syncer,
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
    syncer: BeefySyncer,
    beefy: BeefyLightClient<SignedClientInner>,
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
                justification
                    .signed_validators
                    .iter()
                    .cloned()
                    .map(U256::from)
                    .collect(),
                justification.num_validators.into(),
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
            .create_random_bitfield(
                initial_bitfield.clone(),
                justification.num_validators.into(),
            )
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

    pub async fn handle_complete_commitment_success(self) -> AnyResult<()> {
        self.successful_sent.fetch_add(1, Ordering::Relaxed);
        let latest_block = self.beefy.latest_beefy_block().call().await? as u32;
        self.syncer.update_latest_sent(latest_block as u64);
        Ok(())
    }

    pub async fn run(&self, ignore_unneeded_commitments: bool) -> AnyResult<()> {
        let mut beefy_sub = crate::substrate::beefy_subscription::subscribe_beefy_justifications(
            self.sub.clone(),
            self.syncer.latest_sent(),
        )
        .await?;
        let mut first_attempt_failed = false;
        while let Some(justification) = beefy_sub.next().await.transpose()? {
            let latest_requested = self.syncer.latest_requested();
            let latest_sent = self.syncer.latest_sent();
            let is_mandatory = justification.is_mandatory;
            let should_send = !ignore_unneeded_commitments
                || is_mandatory
                || (latest_requested < justification.commitment.block_number.into()
                    && latest_sent < latest_requested);

            if should_send {
                // TODO: Better async message handler
                if let Err(_) = self
                    .clone()
                    .send_commitment(justification)
                    .await
                    .map_err(|e| {
                        warn!("Send commitment error: {}", e);
                    })
                {
                    if first_attempt_failed || is_mandatory {
                        return Err(anyhow::anyhow!(
                            "Unable to send commitment, possibly BEEFY state is broken"
                        ));
                    }
                    first_attempt_failed = true;
                } else {
                    first_attempt_failed = false;
                }
                info!(
                    "failed: {}, lost gas: {}, successful: {}",
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
