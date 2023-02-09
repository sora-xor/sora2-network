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
use crate::substrate::binary_search_first_occurence;
use crate::substrate::types::*;
use beefy_light_client::ProvedSubstrateBridgeMessage;
use bridge_common::beefy_types::{BeefyMMRLeaf, Commitment, ValidatorProof, ValidatorSet};
use bridge_common::simplified_mmr_proof::SimplifiedMMRProof;
use bridge_types::types::ParachainMessage;
use bridge_types::GenericNetworkId;
use common::Balance;
use futures::Future;

const PARACHAIN_EPOCH_DURATION: u64 = 1800;

pub type ConfigOf<C> = <C as RuntimeClient>::Config;
pub type BlockNumberOf<C> = <ConfigOf<C> as subxt::Config>::BlockNumber;
pub type IndexOf<C> = <ConfigOf<C> as subxt::Config>::Index;
pub type HashOf<C> = <ConfigOf<C> as subxt::Config>::Hash;
pub type OtherExtrinsicParamsOf<C> =
    <<ConfigOf<C> as subxt::Config>::ExtrinsicParams as subxt::tx::ExtrinsicParams<
        IndexOf<C>,
        HashOf<C>,
    >>::OtherParams;
pub type SignatureOf<C> = <ConfigOf<C> as subxt::Config>::Signature;
pub type SignerOf<C> = <SignatureOf<C> as sp_runtime::traits::Verify>::Signer;
pub type AccountIdOf<C> = <ConfigOf<C> as subxt::Config>::AccountId;
pub type AddressOf<C> = <ConfigOf<C> as subxt::Config>::Address;

#[async_trait::async_trait]
pub trait RuntimeClient {
    type Config: subxt::Config + std::fmt::Debug + Send + Sync;
    type SubmitSignatureCommitment: Encode;
    type SubmitMessagesCommitment: Encode;
    type VerificationSuccessful: subxt::events::StaticEvent;
    type Event: Decode + core::fmt::Debug;

    fn submit_signature_commitment(
        &self,
        network_id: GenericNetworkId,
        commitment: Commitment,
        validator_proof: ValidatorProof,
        latest_mmr_leaf: BeefyMMRLeaf,
        proof: SimplifiedMMRProof,
    ) -> subxt::tx::StaticTxPayload<Self::SubmitSignatureCommitment>;
    fn client(&self) -> &SubSignedClient<Self::Config>;
    fn epoch_duration(&self) -> AnyResult<u64>;
    async fn first_beefy_block(&self) -> AnyResult<u64>;
    async fn current_validator_set(&self, network_id: GenericNetworkId) -> AnyResult<ValidatorSet>;
    async fn next_validator_set(&self, network_id: GenericNetworkId) -> AnyResult<ValidatorSet>;
    async fn outbound_channel_nonce(&self, network_id: GenericNetworkId) -> AnyResult<u64>;
    async fn inbound_channel_nonce(&self, network_id: GenericNetworkId) -> AnyResult<u64>;
    async fn find_message_block(
        &self,
        network_id: GenericNetworkId,
        nonce: u64,
    ) -> AnyResult<Option<BlockNumber<Self::Config>>>;
    async fn latest_beefy_block(&self, network_id: GenericNetworkId) -> AnyResult<u64>;
    async fn network_id(&self) -> AnyResult<GenericNetworkId>;
    async fn submit_messages_commitment(
        &self,
        network_id: GenericNetworkId,
        message: ProvedSubstrateBridgeMessage<Vec<ParachainMessage<Balance>>>,
    ) -> subxt::tx::StaticTxPayload<Self::SubmitMessagesCommitment>;
}

#[derive(Clone)]
pub struct SubstrateRuntimeClient(pub SubSignedClient<MainnetConfig>);

impl SubstrateRuntimeClient {
    pub fn new(client: SubSignedClient<MainnetConfig>) -> Self {
        Self(client)
    }
}

#[async_trait::async_trait]
impl RuntimeClient for SubstrateRuntimeClient {
    type Config = MainnetConfig;
    type SubmitSignatureCommitment = runtime::beefy_light_client::calls::SubmitSignatureCommitment;
    type VerificationSuccessful = runtime::beefy_light_client::events::VerificationSuccessful;
    type SubmitMessagesCommitment = runtime::substrate_bridge_inbound_channel::calls::Submit;
    type Event = runtime::Event;

    fn client(&self) -> &SubSignedClient<Self::Config> {
        &self.0
    }

    fn submit_signature_commitment(
        &self,
        network_id: GenericNetworkId,
        commitment: Commitment,
        validator_proof: ValidatorProof,
        latest_mmr_leaf: BeefyMMRLeaf,
        proof: SimplifiedMMRProof,
    ) -> subxt::tx::StaticTxPayload<Self::SubmitSignatureCommitment> {
        let GenericNetworkId::Sub(network_id) = network_id else {
            unimplemented!("Only support Substrate networks");
        };
        let call = runtime::tx()
            .beefy_light_client()
            .submit_signature_commitment(
                network_id,
                commitment,
                validator_proof,
                latest_mmr_leaf,
                proof,
            );
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

    async fn latest_beefy_block(&self, network_id: GenericNetworkId) -> AnyResult<u64> {
        let GenericNetworkId::Sub(network_id) = network_id else {
            unimplemented!("Only support Substrate networks");
        };
        let latest_beefy_block = self
            .0
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .beefy_light_client()
                    .latest_beefy_block(network_id),
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
                &runtime::storage().mmr().number_of_leaves(),
                Some(finalized_hash),
            )
            .await?
            .ok_or(anyhow!("Error to get first beefy block"))?;
        Ok(latest_beefy_block - finalized_number as u64)
    }

    async fn current_validator_set(&self, network_id: GenericNetworkId) -> AnyResult<ValidatorSet> {
        let GenericNetworkId::Sub(network_id) = network_id else {
            unimplemented!("Only support Substrate networks");
        };
        let validator_set = self
            .0
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .beefy_light_client()
                    .current_validator_set(network_id),
                None,
            )
            .await?
            .ok_or(anyhow!("Error to get current validator set"))?;
        Ok(validator_set)
    }

    async fn next_validator_set(&self, network_id: GenericNetworkId) -> AnyResult<ValidatorSet> {
        let GenericNetworkId::Sub(network_id) = network_id else {
            unimplemented!("Only support Substrate networks");
        };
        let validator_set = self
            .0
            .api()
            .storage()
            .fetch(
                &runtime::storage()
                    .beefy_light_client()
                    .next_validator_set(network_id),
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

    async fn network_id(&self) -> AnyResult<GenericNetworkId> {
        let storage = mainnet_runtime::storage()
            .beefy_light_client()
            .this_network_id();
        let network_id = self
            .0
            .api()
            .storage()
            .fetch_or_default(&storage, None)
            .await?;
        Ok(network_id.into())
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
pub struct ParachainRuntimeClient(pub SubSignedClient<ParachainConfig>);

impl ParachainRuntimeClient {
    pub fn new(client: SubSignedClient<ParachainConfig>) -> Self {
        Self(client)
    }
}

#[async_trait::async_trait]
impl RuntimeClient for ParachainRuntimeClient {
    type Config = ParachainConfig;
    type SubmitSignatureCommitment =
        parachain_runtime::beefy_light_client::calls::SubmitSignatureCommitment;
    type VerificationSuccessful =
        parachain_runtime::beefy_light_client::events::VerificationSuccessful;
    type SubmitMessagesCommitment =
        parachain_runtime::substrate_bridge_inbound_channel::calls::Submit;
    type Event = parachain_runtime::Event;

    fn client(&self) -> &SubSignedClient<Self::Config> {
        &self.0
    }

    fn submit_signature_commitment(
        &self,
        network_id: GenericNetworkId,
        commitment: Commitment,
        validator_proof: ValidatorProof,
        latest_mmr_leaf: BeefyMMRLeaf,
        proof: SimplifiedMMRProof,
    ) -> subxt::tx::StaticTxPayload<Self::SubmitSignatureCommitment> {
        let GenericNetworkId::Sub(network_id) = network_id else {
            unimplemented!("Only support Substrate networks");
        };
        let call = parachain_runtime::tx()
            .beefy_light_client()
            .submit_signature_commitment(
                network_id,
                commitment,
                validator_proof,
                latest_mmr_leaf,
                proof,
            );
        call
    }

    fn epoch_duration(&self) -> AnyResult<u64> {
        Ok(PARACHAIN_EPOCH_DURATION)
    }

    async fn latest_beefy_block(&self, network_id: GenericNetworkId) -> AnyResult<u64> {
        let GenericNetworkId::Sub(network_id) = network_id else {
            unimplemented!("Only support Substrate networks");
        };
        let latest_beefy_block = self
            .0
            .api()
            .storage()
            .fetch(
                &parachain_runtime::storage()
                    .beefy_light_client()
                    .latest_beefy_block(network_id),
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
            .ok_or(anyhow!("Error to get first beefy block"))?;
        Ok(latest_beefy_block - finalized_number as u64)
    }

    async fn current_validator_set(&self, network_id: GenericNetworkId) -> AnyResult<ValidatorSet> {
        let GenericNetworkId::Sub(network_id) = network_id else {
            unimplemented!("Only support Substrate networks");
        };
        let validator_set = self
            .0
            .api()
            .storage()
            .fetch(
                &parachain_runtime::storage()
                    .beefy_light_client()
                    .current_validator_set(network_id),
                None,
            )
            .await?
            .ok_or(anyhow!("Error to get current validator set"))?;
        Ok(validator_set)
    }

    async fn next_validator_set(&self, network_id: GenericNetworkId) -> AnyResult<ValidatorSet> {
        let GenericNetworkId::Sub(network_id) = network_id else {
            unimplemented!("Only support Substrate networks");
        };
        let validator_set = self
            .0
            .api()
            .storage()
            .fetch(
                &parachain_runtime::storage()
                    .beefy_light_client()
                    .next_validator_set(network_id),
                None,
            )
            .await?
            .ok_or(anyhow!("Error to get next validator set"))?;
        Ok(validator_set)
    }

    async fn outbound_channel_nonce(&self, network_id: GenericNetworkId) -> AnyResult<u64> {
        let storage = match network_id {
            GenericNetworkId::EVM(_chain_id) => unimplemented!(),
            GenericNetworkId::Sub(network_id) => parachain_runtime::storage()
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
            GenericNetworkId::Sub(network_id) => parachain_runtime::storage()
                .substrate_bridge_inbound_channel()
                .channel_nonces(network_id),
            GenericNetworkId::EVM(_chain_id) => unimplemented!(),
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
    ) -> AnyResult<Option<BlockNumber<Self::Config>>> {
        let storage = match network_id {
            GenericNetworkId::EVM(_chain_id) => unimplemented!(),
            GenericNetworkId::Sub(network_id) => parachain_runtime::storage()
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
        let storage = parachain_runtime::storage()
            .beefy_light_client()
            .this_network_id();
        let network_id = self
            .0
            .api()
            .storage()
            .fetch_or_default(&storage, None)
            .await?;
        Ok(network_id.into())
    }

    async fn submit_messages_commitment(
        &self,
        network_id: GenericNetworkId,
        message: ProvedSubstrateBridgeMessage<Vec<ParachainMessage<Balance>>>,
    ) -> subxt::tx::StaticTxPayload<
        parachain_runtime::substrate_bridge_inbound_channel::calls::Submit,
    > {
        match network_id {
            GenericNetworkId::EVM(_chain_id) => unimplemented!(),
            GenericNetworkId::Sub(network_id) => parachain_runtime::tx()
                .substrate_bridge_inbound_channel()
                .submit(network_id, message),
        }
    }
}
