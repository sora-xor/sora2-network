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
#![allow(dead_code)]

use bridge_types::{H160, H256};
use mmr_lib::MMRStore;
#[cfg(feature = "std")]
use rand::prelude::*;
use sp_core::{DeriveJunction, Pair};
use sp_runtime::traits::{Convert, Hash};

use anyhow::Result as AnyResult;
use codec::Encode;
use log::debug;
use serde::Serialize;

use bridge_common::simplified_proof::Proof;

struct ValidatorSet {
    validators: Vec<sp_core::ecdsa::Pair>,
    addresses: Vec<H160>,
    id: u64,
}

impl ValidatorSet {
    #[cfg(feature = "std")]
    fn generate(id: u64, count: usize) -> AnyResult<Self> {
        let (initial_pair, _phrase, _seed) = sp_core::ecdsa::Pair::generate_with_phrase(None);
        let mut validators = vec![];
        for i in 0..count {
            let (pair, _seed) = initial_pair
                .derive(vec![DeriveJunction::hard(i as u64)].into_iter(), None)
                .map_err(|_| anyhow::anyhow!("Failed to derive keypair"))?;
            validators.push(pair);
        }
        let addresses = validators
            .iter()
            .map(|x| {
                H160::from_slice(&pallet_beefy_mmr::BeefyEcdsaToEthereum::convert(
                    x.public().into(),
                ))
            })
            .collect();
        Ok(Self {
            validators,
            id,
            addresses,
        })
    }

    #[cfg(feature = "std")]
    fn sign_commitment<R: Rng>(
        &self,
        rng: &mut R,
        commitment: sp_consensus_beefy::Commitment<u32>,
        count: Option<usize>,
    ) -> sp_consensus_beefy::SignedCommitment<u32, sp_core::ecdsa::Signature> {
        let commitment_hash = sp_runtime::traits::Keccak256::hash_of(&commitment);
        let validators_threshold = threshold(self.validators.len());
        let signed_count = count.unwrap_or_else(|| {
            (validators_threshold..=self.validators.len())
                .choose(rng)
                .unwrap()
        });
        let signers = self
            .validators
            .iter()
            .enumerate()
            .choose_multiple(rng, signed_count);
        let mut signatures = vec![None; self.validators.len()];
        for (i, signer) in signers {
            let signature = signer.sign_prehashed(&commitment_hash.0);
            signatures[i] = Some(signature);
        }

        sp_consensus_beefy::SignedCommitment {
            commitment,
            signatures,
        }
    }

    pub fn validator_pubkey_proof(&self, pos: usize) -> Vec<H256> {
        let proof = binary_merkle_tree::merkle_proof::<sp_runtime::traits::Keccak256, _, _>(
            self.addresses.clone(),
            pos,
        )
        .proof;
        debug!("Validator {} proof: {}", pos, proof.len());
        proof
    }

    pub fn proofs(&self) -> Vec<Vec<H256>> {
        (0..self.addresses.len())
            .map(|i| self.validator_pubkey_proof(i))
            .collect()
    }

    pub fn fixture(&self) -> FixtureValidatorSet {
        FixtureValidatorSet {
            id: self.id,
            root: self.root(),
            len: self.validators.len() as u32,
        }
    }

    pub fn root(&self) -> H256 {
        binary_merkle_tree::merkle_root::<sp_runtime::traits::Keccak256, _>(self.addresses.clone())
    }

    fn authority_set(&self) -> sp_consensus_beefy::mmr::BeefyAuthoritySet<H256> {
        sp_consensus_beefy::mmr::BeefyAuthoritySet {
            id: self.id,
            len: self.validators.len() as u32,
            keyset_commitment: self.root(),
        }
    }
}

pub type MMRLeaf = sp_consensus_beefy::mmr::MmrLeaf<
    u32,
    H256,
    H256,
    bridge_types::types::LeafExtraData<H256, H256>,
>;

struct FakeMMR {
    // leaves: BTreeMap<u64, MMRLeaf>,
    mem: mmr_lib::util::MemStore<MMRNode>,
    size: u64,
}

impl FakeMMR {
    fn new() -> Self {
        Self {
            // leaves: BTreeMap::new(),
            mem: mmr_lib::util::MemStore::default(),
            size: 0,
        }
    }

    fn add_leaf(&mut self, leaf: MMRLeaf) -> AnyResult<u64> {
        let mut mmr = mmr_lib::MMR::<MMRNode, MMRNode, _>::new(self.size, &self.mem);
        let pos = mmr.push(MMRNode::Leaf(leaf))?;
        self.size = mmr.mmr_size();
        mmr.commit()?;
        Ok(pos)
    }

    fn generate_proof(&self, leaf: u64, at: u64) -> AnyResult<MMRProof> {
        let size = Self::size(at);
        let pos = mmr_lib::leaf_index_to_pos(leaf);
        let mmr = mmr_lib::MMR::<MMRNode, MMRNode, _>::new(size, &self.mem);
        let proof = mmr.gen_proof(vec![pos]).unwrap();
        let proof = proof
            .proof_items()
            .iter()
            .map(|x| x.hash())
            .collect::<Vec<_>>();
        let proof =
            bridge_common::simplified_proof::convert_to_simplified_mmr_proof(leaf, at, &proof);
        Ok(MMRProof {
            order: proof.order,
            items: proof.items,
        })
    }

    fn leaf(&self, leaf: u64) -> MMRLeaf {
        let pos = mmr_lib::leaf_index_to_pos(leaf);
        let elem = (&self.mem).get_elem(pos).unwrap().unwrap();
        match elem {
            MMRNode::Leaf(leaf) => leaf,
            _ => panic!("Invalid leaf"),
        }
    }

    fn size(size: u64) -> u64 {
        size * 2 - size.count_ones() as u64
    }

    fn root(&self, at: u64) -> AnyResult<H256> {
        let size = Self::size(at);
        let mmr = mmr_lib::MMR::<MMRNode, MMRNode, _>::new(size, &self.mem);
        let root = mmr.get_root()?.hash();
        Ok(root)
    }
}

#[derive(Debug, Clone, Serialize, Encode)]
pub struct MMRProof {
    order: u64,
    items: Vec<H256>,
}

impl From<MMRProof> for Proof<H256> {
    fn from(proof: MMRProof) -> Self {
        Proof {
            items: proof.items,
            order: proof.order,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
enum MMRNode {
    Leaf(MMRLeaf),
    Hash(H256),
}

impl MMRNode {
    fn hash(&self) -> H256 {
        match self {
            MMRNode::Leaf(leaf) => sp_runtime::traits::Keccak256::hash_of(leaf),
            MMRNode::Hash(hash) => *hash,
        }
    }
}

impl mmr_lib::Merge for MMRNode {
    type Item = MMRNode;

    fn merge(
        left: &Self::Item,
        right: &Self::Item,
    ) -> core::result::Result<Self::Item, mmr_lib::Error> {
        let res = MMRNode::Hash(sp_runtime::traits::Keccak256::hash_of(&(
            left.hash(),
            right.hash(),
        )));
        Ok(res)
    }
}

pub fn threshold(authorities: usize) -> usize {
    let faulty = authorities.saturating_sub(1) / 3;
    authorities - faulty
}

#[derive(Debug, Clone, Serialize, Encode)]
pub struct FixtureValidatorSet {
    id: u64,
    root: H256,
    len: u32,
}

impl From<FixtureValidatorSet> for bridge_common::beefy_types::ValidatorSet {
    fn from(f: FixtureValidatorSet) -> Self {
        bridge_common::beefy_types::ValidatorSet {
            id: f.id,
            len: f.len,
            keyset_commitment: f.root,
        }
    }
}

#[derive(Debug, Clone, Serialize, Encode)]
pub struct Fixture {
    pub addresses: Vec<H160>,
    pub validator_set: FixtureValidatorSet,
    pub next_validator_set: FixtureValidatorSet,
    pub validator_set_proofs: Vec<Vec<H256>>,
    pub commitment: Vec<u8>,
    pub leaf_proof: MMRProof,
    pub leaf: Vec<u8>,
}

#[cfg(feature = "std")]
pub fn generate_fixture(validators: usize, tree_size: u32) -> AnyResult<Fixture> {
    let mut rng = thread_rng();
    let validator_set = ValidatorSet::generate(0, validators)?;
    let next_validator_set = ValidatorSet::generate(1, validators)?;
    let mut mmr = FakeMMR::new();
    for i in 0..tree_size + 1 {
        mmr.add_leaf(MMRLeaf {
            version: sp_consensus_beefy::mmr::MmrLeafVersion::new(0, 0),
            parent_number_and_hash: (i, H256::random_using(&mut rng)),
            beefy_next_authority_set: next_validator_set.authority_set().into(),
            leaf_extra: bridge_types::types::LeafExtraData {
                digest_hash: H256::random_using(&mut rng),
                random_seed: H256::random_using(&mut rng),
            },
        })?;
    }
    let mmr_root = mmr.root(tree_size as u64)?;

    let commitment = sp_consensus_beefy::Commitment::<u32> {
        payload: sp_consensus_beefy::Payload::from_single_entry(
            sp_consensus_beefy::known_payloads::MMR_ROOT_ID,
            mmr_root.encode(),
        ),
        block_number: tree_size,
        validator_set_id: validator_set.id,
    };
    let signed_commitment = validator_set.sign_commitment(&mut rng, commitment, None);
    let leaf = mmr.leaf(tree_size as u64 - 1);
    let leaf_proof = mmr.generate_proof(tree_size as u64 - 1, tree_size as u64)?;

    let fixture = Fixture {
        addresses: validator_set.addresses.clone(),
        validator_set: validator_set.fixture(),
        next_validator_set: next_validator_set.fixture(),
        validator_set_proofs: validator_set.proofs(),
        commitment: signed_commitment.encode(),
        leaf_proof,
        leaf: leaf.encode(),
    };

    Ok(fixture)
}
