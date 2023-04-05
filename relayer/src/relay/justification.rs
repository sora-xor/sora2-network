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
use crate::substrate::{BeefyCommitment, BeefySignedCommitment, BlockHash, LeafProof};
use bridge_common::{
    bitfield::BitField,
    simplified_proof::{convert_to_simplified_mmr_proof, Proof},
};
use codec::Encode;
use ethers::prelude::*;
use ethers::utils::keccak256;
use sp_beefy::crypto::Signature;
use sp_beefy::SignedCommitment;
use sp_runtime::traits::{AtLeast32Bit, Keccak256, UniqueSaturatedInto};
use sp_runtime::traits::{Convert, Hash as HashTrait};
use sp_runtime::Saturating;

#[derive(Debug)]
pub struct MmrPayload {
    pub prefix: Vec<u8>,
    pub mmr_root: H256,
    pub suffix: Vec<u8>,
}

#[derive(Debug)]
pub struct BeefyJustification<T: ConfigExt> {
    pub commitment: BeefyCommitment<T>,
    pub commitment_hash: H256,
    pub signatures: Vec<Option<Signature>>,
    pub num_validators: u32,
    pub signed_validators: Vec<u32>,
    pub validators: Vec<H160>,
    pub block_hash: BlockHash<T>,
    pub leaf_proof: LeafProof<T>,
    pub simplified_proof: Proof<H256>,
    pub payload: MmrPayload,
    pub is_mandatory: bool,
}

impl<T: ConfigExt> BeefyJustification<T>
where
    T::BlockNumber: AtLeast32Bit + Serialize,
{
    pub async fn create(
        sub: SubUnsignedClient<T>,
        commitment: BeefySignedCommitment<T>,
        is_mandatory: bool,
    ) -> AnyResult<Self> {
        let BeefySignedCommitment::<T>::V1(SignedCommitment {
            commitment,
            signatures,
        }) = commitment;
        let commitment_block_number: u64 = commitment.block_number.clone().into();
        let commitment_hash = keccak256(&Encode::encode(&commitment)).into();
        let num_validators = signatures.len() as u32;
        let mut signed_validators = vec![];
        for (i, signature) in (0u32..).zip(signatures.iter()) {
            if let Some(_) = signature {
                signed_validators.push(i)
            }
        }
        let validators: Vec<H160> = sub
            .storage_fetch_or_default(
                &runtime::storage().beefy().authorities(),
                commitment_block_number - 1,
            )
            .await?
            .0
            .into_iter()
            .map(|x| H160::from_slice(&pallet_beefy_mmr::BeefyEcdsaToEthereum::convert(x)))
            .collect();
        let block_hash = sub.block_hash(commitment_block_number).await?.into();

        let payload = Self::get_payload(&commitment).ok_or(anyhow!("Payload is not supported"))?;
        let (leaf_proof, simplified_proof) =
            Self::find_mmr_proof(&sub, &commitment, payload.mmr_root).await?;

        Ok(Self {
            commitment,
            commitment_hash,
            num_validators,
            signed_validators,
            signatures,
            validators,
            block_hash,
            leaf_proof,
            simplified_proof,
            payload,
            is_mandatory,
        })
    }

    pub async fn find_mmr_proof(
        sub: &SubUnsignedClient<T>,
        commitment: &BeefyCommitment<T>,
        root: H256,
    ) -> AnyResult<(LeafProof<T>, Proof<H256>)> {
        for block_number in 0u32..=6u32 {
            let block_number = commitment.block_number.saturating_sub(block_number.into());
            let leaf_proof = sub.mmr_generate_proof(block_number, block_number).await?;
            let hashed_leaf = leaf_proof.leaf.using_encoded(Keccak256::hash);
            debug!("Hashed leaf: {:?}", hashed_leaf);
            let proof = convert_to_simplified_mmr_proof(
                leaf_proof.proof.leaf_indices[0],
                leaf_proof.proof.leaf_count,
                &leaf_proof.proof.items,
            );
            let computed_root = proof.root(
                |a, b| {
                    let res = [a.as_bytes(), b.as_bytes()].concat();
                    Keccak256::hash(&res)
                },
                hashed_leaf,
            );
            if computed_root != root {
                warn!("MMR root mismatch: {:?} != {:?}", root, computed_root);
                continue;
            }
            return Ok((leaf_proof, proof));
        }
        return Err(anyhow!("Could not find MMR proof"));
    }

    pub fn get_payload(commitment: &BeefyCommitment<T>) -> Option<MmrPayload> {
        commitment
            .payload
            .get_raw(&sp_beefy::known_payloads::MMR_ROOT_ID)
            .and_then(|x| x.clone().try_into().ok())
            .and_then(|mmr_root: [u8; 32]| {
                let payload = hex::encode(commitment.payload.encode());
                let mmr_root_with_id = hex::encode(
                    (sp_beefy::known_payloads::MMR_ROOT_ID, mmr_root.to_vec()).encode(),
                );
                let (prefix, suffix) = if let Some(x) = payload.strip_suffix(&mmr_root_with_id) {
                    (x, "")
                } else if let Some(x) = payload.strip_prefix(&mmr_root_with_id) {
                    ("", x)
                } else {
                    payload.split_once(&mmr_root_with_id)?
                };
                Some(MmrPayload {
                    prefix: hex::decode(prefix).expect("should be ok"),
                    mmr_root: mmr_root.into(),
                    suffix: hex::decode(suffix).expect("should be ok"),
                })
            })
    }

    pub fn validator_eth_signature(&self, pos: usize) -> Bytes {
        let mut validator_signature = self.signatures[pos].clone().expect("signed").to_vec();
        validator_signature[64] += 27;
        return validator_signature.into();
    }

    pub fn validator_pubkey(&self, pos: usize) -> H160 {
        let validator_public_key = self.validators[pos];
        validator_public_key
    }

    pub fn validator_pubkey_proof(&self, pos: usize) -> Vec<H256> {
        let proof = beefy_merkle_tree::merkle_proof::<sp_runtime::traits::Keccak256, _, _>(
            self.validators.clone(),
            pos,
        )
        .proof;
        debug!("Validator {} proof: {}", pos, proof.len());
        proof
    }

    pub fn validators_proof(
        &self,
        initial_bitfield: Vec<U256>,
        random_bitfield: Vec<U256>,
    ) -> ethereum_gen::ValidatorProof {
        let mut positions = vec![];
        let mut signatures = vec![];
        let mut public_keys = vec![];
        let mut public_key_merkle_proofs = vec![];
        for i in 0..random_bitfield.len() * 256 {
            let bit = random_bitfield[i / 256].bit(i % 256);
            if bit {
                positions.push(U256::from(i));
                signatures.push(self.validator_eth_signature(i));
                public_keys.push(self.validator_pubkey(i));
                public_key_merkle_proofs.push(self.validator_pubkey_proof(i));
            }
        }
        let validator_proof = ethereum_gen::ValidatorProof {
            signatures,
            positions,
            public_keys,
            public_key_merkle_proofs: public_key_merkle_proofs
                .into_iter()
                .map(|x| x.into_iter().map(|x| x.0).collect())
                .collect(),
            validator_claims_bitfield: initial_bitfield,
        };
        validator_proof
    }

    pub fn validators_proof_sub(
        &self,
        initial_bitfield: BitField,
        random_bitfield: BitField,
    ) -> bridge_common::beefy_types::ValidatorProof {
        let mut positions = vec![];
        let mut signatures = vec![];
        let mut public_keys = vec![];
        let mut public_key_merkle_proofs = vec![];
        for i in 0..random_bitfield.len() {
            let bit = random_bitfield.is_set(i);
            if bit {
                positions.push(i as u128);
                signatures.push(self.validator_eth_signature(i).to_vec());
                public_keys.push(self.validator_pubkey(i));
                public_key_merkle_proofs.push(self.validator_pubkey_proof(i));
            }
        }
        let validator_proof = bridge_common::beefy_types::ValidatorProof {
            signatures,
            positions,
            public_keys,
            public_key_merkle_proofs: public_key_merkle_proofs,
            validator_claims_bitfield: initial_bitfield,
        };
        validator_proof
    }

    pub fn simplified_mmr_proof(
        &self,
    ) -> AnyResult<(
        ethereum_gen::beefy_light_client::BeefyMMRLeaf,
        ethereum_gen::beefy_light_client::SimplifiedMMRProof,
    )> {
        let LeafProof { leaf, .. } = self.leaf_proof.clone();
        let (major, minor) = leaf.version.split();
        let leaf_version = (major << 5) + minor;
        let mmr_leaf = ethereum_gen::beefy_light_client::BeefyMMRLeaf {
            version: leaf_version,
            parent_number: leaf.parent_number_and_hash.0.unique_saturated_into(),
            parent_hash: leaf.parent_number_and_hash.1.as_ref().try_into().unwrap(),
            next_authority_set_id: leaf.beefy_next_authority_set.id,
            next_authority_set_len: leaf.beefy_next_authority_set.len,
            next_authority_set_root: leaf.beefy_next_authority_set.root.to_fixed_bytes(),
            digest_hash: leaf.leaf_extra.digest_hash.0,
            random_seed: leaf.leaf_extra.random_seed.0,
        };

        let proof = ethereum_gen::beefy_light_client::SimplifiedMMRProof {
            merkle_proof_items: self.simplified_proof.items.iter().map(|x| x.0).collect(),
            merkle_proof_order_bit_field: self.simplified_proof.order,
        };
        Ok((mmr_leaf, proof))
    }

    pub fn simplified_mmr_proof_sub(
        &self,
    ) -> AnyResult<(
        bridge_common::beefy_types::BeefyMMRLeaf,
        bridge_common::simplified_proof::Proof<H256>,
    )> {
        let LeafProof { leaf, .. } = self.leaf_proof.clone();
        let parent_hash: [u8; 32] = leaf.parent_number_and_hash.1.as_ref().try_into().unwrap();
        let mmr_leaf = bridge_common::beefy_types::BeefyMMRLeaf {
            version: leaf.version,
            parent_number_and_hash: (
                leaf.parent_number_and_hash.0.unique_saturated_into(),
                parent_hash.into(),
            ),
            beefy_next_authority_set: leaf.beefy_next_authority_set,
            leaf_extra: leaf.leaf_extra,
        };

        let proof = bridge_common::simplified_proof::Proof::<H256> {
            items: self.simplified_proof.items.clone(),
            order: self.simplified_proof.order,
        };
        Ok((mmr_leaf, proof))
    }
}
