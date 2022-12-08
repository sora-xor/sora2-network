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

use crate::prelude::*;
use crate::substrate::types::*;
use bridge_common::beefy_types::{BeefyMMRLeaf, Commitment, ValidatorProof, ValidatorSet};
use bridge_common::simplified_mmr_proof::SimplifiedMMRProof;
use bridge_types::types::ParachainMessage;
use bridge_types::{GenericNetworkId, SubNetworkId};
use common::Balance;
use futures::Future;
use substrate_gen::runtime::runtime_types::beefy_light_client::ProvedSubstrateBridgeMessage;

const PARACHAIN_EPOCH_DURATION: u64 = 1800;

async fn binary_search_first_occurence<T: PartialOrd, F, Fut>(
    low: u32,
    high: u32,
    value: T,
    f: F,
) -> AnyResult<Option<u32>>
where
    F: Fn(u32) -> Fut,
    Fut: Future<Output = AnyResult<Option<T>>>,
{
    let mut low = low;
    let mut high = high;
    while low < high {
        let mid = (high + low) / 2;
        let found_value = f(mid).await?;
        match found_value {
            None => low = mid + 1,
            Some(found_value) if found_value < value => low = mid + 1,
            _ => high = mid,
        }
    }
    // Nonce between blocks can increase more than by 1
    if f(low).await? >= Some(value) {
        Ok(Some(low))
    } else {
        Ok(None)
    }
}
#[async_trait::async_trait]
pub trait RuntimeClient {
    type SubmitSignatureCommitment: Encode;
    type SubmitMessagesCommitment: Encode;
    type VerificationSuccessful: subxt::events::StaticEvent;

    fn submit_signature_commitment(
        &self,
        commitment: Commitment,
        validator_proof: ValidatorProof,
        latest_mmr_leaf: BeefyMMRLeaf,
        proof: SimplifiedMMRProof,
    ) -> subxt::tx::StaticTxPayload<Self::SubmitSignatureCommitment>;
    fn client(&self) -> &SubSignedClient;
    fn epoch_duration(&self) -> AnyResult<u64>;
    async fn latest_beefy_block(&self) -> AnyResult<u64>;
    async fn first_beefy_block(&self) -> AnyResult<u64>;
    async fn current_validator_set(&self) -> AnyResult<ValidatorSet>;
    async fn next_validator_set(&self) -> AnyResult<ValidatorSet>;
    async fn outbound_channel_nonce(&self, network_id: GenericNetworkId) -> AnyResult<u64>;
    async fn inbound_channel_nonce(&self, network_id: GenericNetworkId) -> AnyResult<u64>;
    async fn find_message_block(
        &self,
        network_id: GenericNetworkId,
        nonce: u64,
    ) -> AnyResult<Option<BlockNumber>>;
    async fn network_id(&self) -> AnyResult<GenericNetworkId>;
    async fn submit_messages_commitment(
        &self,
        network_id: GenericNetworkId,
        message: ProvedSubstrateBridgeMessage<Vec<ParachainMessage<Balance>>>,
    ) -> subxt::tx::StaticTxPayload<Self::SubmitMessagesCommitment>;
}

#[derive(Clone)]
pub struct SubstrateRuntimeClient(pub SubSignedClient);

impl SubstrateRuntimeClient {
    pub fn new(client: SubSignedClient) -> Self {
        Self(client)
    }
}

#[async_trait::async_trait]
impl RuntimeClient for SubstrateRuntimeClient {
    type SubmitSignatureCommitment = runtime::beefy_light_client::calls::SubmitSignatureCommitment;
    type VerificationSuccessful = runtime::beefy_light_client::events::VerificationSuccessful;
    type SubmitMessagesCommitment = runtime::substrate_bridge_inbound_channel::calls::Submit;

    fn client(&self) -> &SubSignedClient {
        &self.0
    }

    fn submit_signature_commitment(
        &self,
        commitment: Commitment,
        validator_proof: ValidatorProof,
        latest_mmr_leaf: BeefyMMRLeaf,
        proof: SimplifiedMMRProof,
    ) -> subxt::tx::StaticTxPayload<Self::SubmitSignatureCommitment> {
        let call = runtime::tx()
            .beefy_light_client()
            .submit_signature_commitment(commitment, validator_proof, latest_mmr_leaf, proof);
        call
    }

    fn epoch_duration(&self) -> AnyResult<u64> {
        let epoch_duration = self
            .0
            .api()
            .constants()
            .at(&runtime::constants().babe().epoch_duration())?;
        Ok(epoch_duration)
    }

    async fn latest_beefy_block(&self) -> AnyResult<u64> {
        let latest_beefy_block = self
            .0
            .api()
            .storage()
            .fetch(
                &runtime::storage().beefy_light_client().latest_beefy_block(),
                None,
            )
            .await?
            .ok_or(anyhow!("Error to get latest beefy block"))?;
        Ok(latest_beefy_block as u64)
    }

    async fn first_beefy_block(&self) -> AnyResult<u64> {
        let finalized_hash = self.0.api().rpc().finalized_head().await?;
        let finalized_number = self
            .0
            .api()
            .rpc()
            .header(Some(finalized_hash))
            .await?
            .expect("finalized header")
            .number;
        let latest_beefy_block = self
            .0
            .api()
            .storage()
            .fetch(
                &runtime::storage().mmr().number_of_leaves(),
                Some(finalized_hash),
            )
            .await?
            .ok_or(anyhow!("Error to get latest beefy block"))?;
        Ok(latest_beefy_block - finalized_number as u64)
    }

    async fn current_validator_set(&self) -> AnyResult<ValidatorSet> {
        let validator_set = self
            .0
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .beefy_light_client()
                    .current_validator_set(),
                None,
            )
            .await?
            .ok_or(anyhow!("Error to get current validator set"))?;
        Ok(validator_set)
    }

    async fn next_validator_set(&self) -> AnyResult<ValidatorSet> {
        let validator_set = self
            .0
            .api()
            .storage()
            .fetch(
                &runtime::storage().beefy_light_client().next_validator_set(),
                None,
            )
            .await?
            .ok_or(anyhow!("Error to get next validator set"))?;
        Ok(validator_set)
    }

    async fn outbound_channel_nonce(&self, network_id: GenericNetworkId) -> AnyResult<u64> {
        let storage = match network_id {
            GenericNetworkId::EVM(chain_id) => runtime::storage()
                .bridge_outbound_channel()
                .channel_nonces(chain_id),
            GenericNetworkId::Sub(network_id) => runtime::storage()
                .substrate_bridge_outbound_channel()
                .channel_nonces(network_id),
        };
        let nonce = self
            .0
            .api()
            .storage()
            .fetch_or_default(&storage, None)
            .await?;
        Ok(nonce)
    }

    async fn inbound_channel_nonce(&self, network_id: GenericNetworkId) -> AnyResult<u64> {
        let storage = match network_id {
            GenericNetworkId::EVM(chain_id) => runtime::storage()
                .bridge_inbound_channel()
                .channel_nonces(chain_id),
            GenericNetworkId::Sub(network_id) => runtime::storage()
                .substrate_bridge_inbound_channel()
                .channel_nonces(network_id),
        };
        let nonce = self
            .0
            .api()
            .storage()
            .fetch_or_default(&storage, None)
            .await?;
        Ok(nonce)
    }

    async fn find_message_block(
        &self,
        network_id: GenericNetworkId,
        nonce: u64,
    ) -> AnyResult<Option<BlockNumber>> {
        let storage = match network_id {
            GenericNetworkId::EVM(chain_id) => runtime::storage()
                .bridge_outbound_channel()
                .channel_nonces(chain_id),
            GenericNetworkId::Sub(network_id) => runtime::storage()
                .substrate_bridge_outbound_channel()
                .channel_nonces(network_id),
        };
        let low = 1u32;
        let high = self
            .0
            .api()
            .rpc()
            .header(None)
            .await?
            .expect("should exist")
            .number;

        trace!(
            "Searching for message with nonce {} in block range {}..={}",
            nonce,
            low,
            high
        );
        let start_block = binary_search_first_occurence(low, high, nonce, |block| {
            let storage = &storage;
            async move {
                let hash = self
                    .0
                    .api()
                    .rpc()
                    .block_hash(Some(block.into()))
                    .await?
                    .expect("should exist");
                let nonce = self.0.api().storage().fetch(storage, Some(hash)).await?;
                info!("Nonce at block {}: {:?}", block, nonce);
                Ok(nonce)
            }
        })
        .await?;
        info!(
            "Found message with nonce {} at block {:?}",
            nonce, start_block
        );
        Ok(start_block)
    }

    async fn network_id(&self) -> AnyResult<GenericNetworkId> {
        Ok(SubNetworkId::Mainnet.into())
    }

    async fn submit_messages_commitment(
        &self,
        network_id: GenericNetworkId,
        message: ProvedSubstrateBridgeMessage<Vec<ParachainMessage<Balance>>>,
    ) -> subxt::tx::StaticTxPayload<Self::SubmitMessagesCommitment> {
        match network_id {
            GenericNetworkId::EVM(_chain_id) => unimplemented!(),
            GenericNetworkId::Sub(network_id) => runtime::tx()
                .substrate_bridge_inbound_channel()
                .submit(network_id, message),
        }
    }
}

#[derive(Clone)]
pub struct ParachainRuntimeClient(pub SubSignedClient);

impl ParachainRuntimeClient {
    pub fn new(client: SubSignedClient) -> Self {
        Self(client)
    }
}

#[async_trait::async_trait]
impl RuntimeClient for ParachainRuntimeClient {
    type SubmitSignatureCommitment =
        parachain_runtime::beefy_light_client::calls::SubmitSignatureCommitment;
    type VerificationSuccessful =
        parachain_runtime::beefy_light_client::events::VerificationSuccessful;
    type SubmitMessagesCommitment = ();

    fn client(&self) -> &SubSignedClient {
        &self.0
    }

    fn submit_signature_commitment(
        &self,
        commitment: Commitment,
        validator_proof: ValidatorProof,
        latest_mmr_leaf: BeefyMMRLeaf,
        proof: SimplifiedMMRProof,
    ) -> subxt::tx::StaticTxPayload<Self::SubmitSignatureCommitment> {
        let call = parachain_runtime::tx()
            .beefy_light_client()
            .submit_signature_commitment(commitment, validator_proof, latest_mmr_leaf, proof);
        call
    }

    fn epoch_duration(&self) -> AnyResult<u64> {
        Ok(PARACHAIN_EPOCH_DURATION)
    }

    async fn latest_beefy_block(&self) -> AnyResult<u64> {
        let latest_beefy_block = self
            .0
            .api()
            .storage()
            .fetch(
                &parachain_runtime::storage()
                    .beefy_light_client()
                    .latest_beefy_block(),
                None,
            )
            .await?
            .ok_or(anyhow!("Error to get latest beefy block"))?;
        Ok(latest_beefy_block)
    }

    async fn first_beefy_block(&self) -> AnyResult<u64> {
        let finalized_hash = self.0.api().rpc().finalized_head().await?;
        let finalized_number = self
            .0
            .api()
            .rpc()
            .header(Some(finalized_hash))
            .await?
            .expect("finalized header")
            .number;
        let latest_beefy_block = self
            .0
            .api()
            .storage()
            .fetch(
                &parachain_runtime::storage().mmr().number_of_leaves(),
                Some(finalized_hash),
            )
            .await?
            .ok_or(anyhow!("Error to get latest beefy block"))?;
        Ok(latest_beefy_block - finalized_number as u64)
    }

    async fn current_validator_set(&self) -> AnyResult<ValidatorSet> {
        let validator_set = self
            .0
            .api()
            .storage()
            .fetch(
                &parachain_runtime::storage()
                    .beefy_light_client()
                    .current_validator_set(),
                None,
            )
            .await?
            .ok_or(anyhow!("Error to get current validator set"))?;
        Ok(validator_set)
    }

    async fn next_validator_set(&self) -> AnyResult<ValidatorSet> {
        let validator_set = self
            .0
            .api()
            .storage()
            .fetch(
                &parachain_runtime::storage()
                    .beefy_light_client()
                    .next_validator_set(),
                None,
            )
            .await?
            .ok_or(anyhow!("Error to get next validator set"))?;
        Ok(validator_set)
    }

    async fn outbound_channel_nonce(&self, _network_id: GenericNetworkId) -> AnyResult<u64> {
        // TODO: Implement this
        unimplemented!("outbound_channel_nonce is not implemented for parachain");
    }

    async fn inbound_channel_nonce(&self, _network_id: GenericNetworkId) -> AnyResult<u64> {
        // TODO: Implement this
        unimplemented!("inbound_channel_nonce is not implemented for parachain")
    }

    async fn find_message_block(
        &self,
        _network_id: GenericNetworkId,
        _nonce: u64,
    ) -> AnyResult<Option<BlockNumber>> {
        // TODO: Implement this
        unimplemented!("find_message_block is not implemented for parachain")
    }

    async fn network_id(&self) -> AnyResult<GenericNetworkId> {
        let chain = self.0.api().rpc().system_chain().await?;
        match chain.as_str() {
            "SORA Kusama" => Ok(SubNetworkId::Kusama.into()),
            "SORA Rococo" => Ok(SubNetworkId::Rococo.into()),
            "SORA Polkadot" => Ok(SubNetworkId::Polkadot.into()),
            _ => Err(anyhow!("Unknown chain: {}", chain)),
        }
    }

    async fn submit_messages_commitment(
        &self,
        _network_id: GenericNetworkId,
        _message: ProvedSubstrateBridgeMessage<Vec<ParachainMessage<Balance>>>,
    ) -> subxt::tx::StaticTxPayload<()> {
        unimplemented!("submit_messages_commitment is not implemented for parachain");
    }
}
