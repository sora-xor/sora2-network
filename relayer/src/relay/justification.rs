use crate::prelude::*;
use crate::substrate::{BeefyCommitment, BeefySignedCommitment, LeafProof};
use beefy_merkle_tree::Hash;
use beefy_primitives::crypto::Signature;
use beefy_primitives::SignedCommitment;
use codec::Encode;
use ethereum_gen::{beefy_light_client, ValidatorProof};
use ethers::prelude::*;
use ethers::utils::keccak256;
use sp_runtime::traits::Keccak256;
use sp_runtime::traits::{Convert, Hash as HashTrait};

use super::simplified_proof::{convert_to_simplified_mmr_proof, Proof};

pub struct BeefyHasher;

impl beefy_merkle_tree::Hasher for BeefyHasher {
    fn hash(data: &[u8]) -> Hash {
        keccak256(data)
    }
}

#[derive(Debug)]
pub struct MmrPayload {
    pub prefix: Vec<u8>,
    pub mmr_root: H256,
    pub suffix: Vec<u8>,
}

#[derive(Debug)]
pub struct BeefyJustification {
    pub commitment: BeefyCommitment,
    pub commitment_hash: H256,
    pub signatures: Vec<Option<Signature>>,
    pub num_validators: U256,
    pub signed_validators: Vec<U256>,
    pub validators: Vec<H160>,
    pub block_hash: H256,
    pub leaf_proof: LeafProof,
    pub simplified_proof: Proof<H256>,
    pub payload: MmrPayload,
}

impl BeefyJustification {
    pub async fn create(
        sub: SubUnsignedClient,
        commitment: BeefySignedCommitment,
    ) -> AnyResult<Self> {
        let BeefySignedCommitment::V1(SignedCommitment {
            commitment,
            signatures,
        }) = commitment;
        let commitment_hash = keccak256(&Encode::encode(&commitment)).into();
        let num_validators = U256::from(signatures.len());
        let mut signed_validators = vec![];
        for (i, signature) in signatures.iter().enumerate() {
            if let Some(_) = signature {
                signed_validators.push(U256::from(i))
            }
        }
        let block_hash = sub.block_hash(Some(commitment.block_number - 1)).await?;
        let validators: Vec<H160> = sub
            .api()
            .storage()
            .fetch_or_default(&runtime::storage().beefy().authorities(), Some(block_hash))
            .await?
            .into_iter()
            .map(|x| H160::from_slice(&pallet_beefy_mmr::BeefyEcdsaToEthereum::convert(x)))
            .collect();
        let block_hash = sub.block_hash(Some(commitment.block_number)).await?;

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
        })
    }

    pub async fn find_mmr_proof(
        sub: &SubUnsignedClient,
        commitment: &BeefyCommitment,
        root: H256,
    ) -> AnyResult<(LeafProof, Proof<H256>)> {
        for block_number in (commitment.block_number.saturating_sub(5)
            ..=commitment.block_number.saturating_add(1))
            .rev()
        {
            let block_hash = sub.block_hash(Some(block_number)).await?;
            let leaf_count = sub
                .api()
                .storage()
                .fetch_or_default(
                    &runtime::storage().mmr().number_of_leaves(),
                    Some(block_hash),
                )
                .await?;
            let leaf_index = leaf_count.saturating_sub(1);
            let leaf_proof = sub.mmr_generate_proof(leaf_index, Some(block_hash)).await?;
            let hashed_leaf = leaf_proof.leaf.using_encoded(Keccak256::hash);
            let proof = convert_to_simplified_mmr_proof(
                leaf_proof.proof.leaf_index,
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

    pub fn get_payload(commitment: &BeefyCommitment) -> Option<MmrPayload> {
        commitment
            .payload
            .get_raw(&beefy_primitives::known_payload_ids::MMR_ROOT_ID)
            .and_then(|x| x.clone().try_into().ok())
            .and_then(|mmr_root: [u8; 32]| {
                let payload = hex::encode(commitment.payload.encode());
                let mmr_root_with_id = hex::encode(
                    (
                        beefy_primitives::known_payload_ids::MMR_ROOT_ID,
                        mmr_root.to_vec(),
                    )
                        .encode(),
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

    pub fn validator_pubkey_proof(&self, pos: usize) -> Vec<Hash> {
        let proof =
            beefy_merkle_tree::merkle_proof::<BeefyHasher, _, _>(self.validators.clone(), pos)
                .proof;
        debug!("Validator {} proof: {}", pos, proof.len());
        proof
    }

    pub fn validators_proof(
        &self,
        initial_bitfield: Vec<U256>,
        random_bitfield: Vec<U256>,
    ) -> ValidatorProof {
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
        let validator_proof = ValidatorProof {
            signatures,
            positions,
            public_keys,
            public_key_merkle_proofs,
            validator_claims_bitfield: initial_bitfield,
        };
        validator_proof
    }

    pub fn simplified_mmr_proof(
        &self,
    ) -> AnyResult<(
        beefy_light_client::BeefyMMRLeaf,
        beefy_light_client::SimplifiedMMRProof,
    )> {
        let LeafProof { leaf, .. } = self.leaf_proof.clone();
        let (major, minor) = leaf.version.split();
        let leaf_version = (major << 5) + minor;
        let mmr_leaf = beefy_light_client::BeefyMMRLeaf {
            version: leaf_version,
            parent_number: leaf.parent_number_and_hash.0,
            parent_hash: leaf.parent_number_and_hash.1.to_fixed_bytes(),
            next_authority_set_id: leaf.beefy_next_authority_set.id,
            next_authority_set_len: leaf.beefy_next_authority_set.len,
            next_authority_set_root: leaf.beefy_next_authority_set.root.to_fixed_bytes(),
            digest_hash: leaf.leaf_extra.digest_hash.0,
            random_seed: leaf.leaf_extra.random_seed.0,
        };

        let proof = beefy_light_client::SimplifiedMMRProof {
            merkle_proof_items: self.simplified_proof.items.iter().map(|x| x.0).collect(),
            merkle_proof_order_bit_field: self.simplified_proof.order,
        };
        Ok((mmr_leaf, proof))
    }
}
