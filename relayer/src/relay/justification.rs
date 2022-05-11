use crate::prelude::*;
use crate::substrate::{BeefyCommitment, EncodedBeefyCommitment, LeafProof};
use beefy_merkle_tree::Hash;
use beefy_primitives::crypto::Signature;
use beefy_primitives::SignedCommitment;
use codec::Encode;
use ethereum_gen::{beefy_light_client, ValidatorProof};
use ethers::prelude::*;
use ethers::utils::keccak256;
use subxt::sp_runtime::traits::Convert;

use super::simplified_proof::convert_to_simplified_mmr_proof;

pub struct BeefyHasher;

impl beefy_merkle_tree::Hasher for BeefyHasher {
    fn hash(data: &[u8]) -> Hash {
        keccak256(data)
    }
}

#[derive(Debug)]
pub struct BeefyJustification {
    pub commitment: BeefyCommitment,
    pub commitment_hash: Hash,
    pub signatures: Vec<Option<Signature>>,
    pub num_validators: U256,
    pub signed_validators: Vec<U256>,
    pub validators: Vec<H160>,
    pub block_hash: H256,
    pub leaf_proof: LeafProof,
}

impl BeefyJustification {
    pub async fn create(
        sub: SubUnsignedClient,
        encoded_commitment: EncodedBeefyCommitment,
        beefy_start_block: u32,
    ) -> AnyResult<Self> {
        let SignedCommitment {
            commitment,
            signatures,
        } = encoded_commitment.decode()?;
        let commitment_hash = keccak256(&Encode::encode(&commitment));
        let num_validators = U256::from(signatures.len());
        let mut signed_validators = vec![];
        for (i, signature) in signatures.iter().enumerate() {
            if let Some(_) = signature {
                signed_validators.push(U256::from(i))
            }
        }
        let validators: Vec<H160> = sub
            .api()
            .storage()
            .beefy()
            .authorities(None)
            .await?
            .into_iter()
            .map(|x| H160::from_slice(&pallet_beefy_mmr::BeefyEcdsaToEthereum::convert(x)))
            .collect();
        let block_hash = sub
            .api()
            .client
            .rpc()
            .block_hash(Some(commitment.block_number.into()))
            .await?
            .unwrap();

        let leaf_index = commitment.block_number - beefy_start_block - 1;
        let leaf_proof = sub
            .mmr_generate_proof(leaf_index as u64, Some(block_hash))
            .await?;

        Ok(Self {
            commitment,
            commitment_hash,
            num_validators,
            signed_validators,
            signatures,
            validators,
            block_hash,
            leaf_proof,
        })
    }

    pub fn is_supported(&self) -> bool {
        self.get_raw_payload().is_some()
    }

    pub fn get_raw_payload(&self) -> Option<[u8; 32]> {
        self.commitment
            .payload
            .get_raw(&beefy_primitives::known_payload_ids::MMR_ROOT_ID)
            .and_then(|x| x.clone().try_into().ok())
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

    pub fn validators_proof(&self, random_bitfield: Vec<U256>) -> ValidatorProof {
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
        };
        validator_proof
    }

    pub fn simplified_mmr_proof(
        &self,
    ) -> AnyResult<(
        beefy_light_client::BeefyMMRLeaf,
        beefy_light_client::SimplifiedMMRProof,
    )> {
        let LeafProof { leaf, proof, .. } = self.leaf_proof.clone();
        let (major, minor) = leaf.version.split();
        let leaf_version = (major << 5) + minor;
        let mmr_leaf = beefy_light_client::BeefyMMRLeaf {
            version: leaf_version,
            parent_number: leaf.parent_number_and_hash.0,
            parent_hash: leaf.parent_number_and_hash.1.to_fixed_bytes(),
            next_authority_set_id: leaf.beefy_next_authority_set.id,
            next_authority_set_len: leaf.beefy_next_authority_set.len,
            next_authority_set_root: leaf.beefy_next_authority_set.root.to_fixed_bytes(),
            digest_hash: leaf.leaf_extra.0,
        };

        let proof =
            convert_to_simplified_mmr_proof(proof.leaf_index, proof.leaf_count, proof.items);
        let proof = beefy_light_client::SimplifiedMMRProof {
            merkle_proof_items: proof.items.iter().map(|x| x.0).collect(),
            merkle_proof_order_bit_field: proof.order,
        };
        Ok((mmr_leaf, proof))
    }
}
