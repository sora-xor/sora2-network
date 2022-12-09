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

const PARACHAIN_EPOCH_DURATION: u64 = 1800;

#[async_trait::async_trait]
pub trait RuntimeClient {
    type SubmitSignatureCommitment: Encode;
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
    async fn latest_beefy_block(&self) -> AnyResult<u32>;
    async fn current_validator_set(&self) -> AnyResult<ValidatorSet>;
    async fn next_validator_set(&self) -> AnyResult<ValidatorSet>;
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

    async fn latest_beefy_block(&self) -> AnyResult<u32> {
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
        Ok(latest_beefy_block as u32)
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

    async fn latest_beefy_block(&self) -> AnyResult<u32> {
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
        Ok(latest_beefy_block as u32)
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
}
