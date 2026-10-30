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

//! Component benchmarks for recalibrating `weights.rs` on the deployment engine.
//! The checked-in coefficients are conservative provisional charges, not claimed
//! output from these benchmarks. Run the full native fixture calibration as well.

use super::*;
use bridge_types::traits::Verifier;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use sp_std::vec;

// Public test vector: private scalar=nonce=1, message scalar=0. Thus r=s=G.x.
// No operator key or keystore is used by these component benchmarks.
fn signature_vector() -> ([u8; 65], H256) {
    let r = [
        0x79, 0xbe, 0x66, 0x7e, 0xf9, 0xdc, 0xbb, 0xac, 0x55, 0xa0, 0x62, 0x95, 0xce, 0x87, 0x0b,
        0x07, 0x02, 0x9b, 0xfc, 0xdb, 0x2d, 0xce, 0x28, 0xd9, 0x59, 0xf2, 0x81, 0x5b, 0x16, 0xf8,
        0x17, 0x98,
    ];
    let mut signature = [0u8; 65];
    signature[..32].copy_from_slice(&r);
    signature[32..64].copy_from_slice(&r);
    (signature, H256::zero())
}

benchmarks! {
    initialize {
        let set = ValidatorSet { id: 0, len: T::MaxValidators::get(), keyset_commitment: H256::zero() };
    }: _(RawOrigin::Root, SubNetworkId::Mainnet, 0, set.clone(), set)
    verify {
        assert!(CurrentValidatorSet::<T>::contains_key(SubNetworkId::Mainnet));
    }

    submit_signature_commitment {
        let v in 1..T::MaxValidators::get();
        let network = SubNetworkId::Mainnet;
        let caller: T::AccountId = whitelisted_caller();
        // The announced next set is independent of the current set, so the
        // commitment can be constructed before the synthetic signing keys.
        let leaf = BeefyMMRLeaf {
            version: sp_consensus_beefy::mmr::MmrLeafVersion::new(0, 0),
            parent_number_and_hash: (0, H256::zero()),
            beefy_next_authority_set: ValidatorSet { id: 2, len: v, keyset_commitment: H256::repeat_byte(2) },
            leaf_extra: bridge_types::types::LeafExtraData {
                digest_hash: H256::zero(),
                random_seed: H256::repeat_byte(3),
            },
        };
        let mmr = Proof { order: 0, items: vec![H256::zero(); 63] };
        let mmr_root = mmr.root(hasher, Keccak256::hash_of(&leaf));
        // Fill the 64 KiB limit with many tiny payload containers, exercising
        // their traversal as well as bytes hashed. Only `mh` has an MMR root.
        // SCALE overhead: payload length 4, root entry 35, block 4, set id 8;
        // each empty extra entry needs 3 bytes, with one final padding byte.
        let mut entries = vec![(*b"zz", Vec::<u8>::new()); (MAX_COMMITMENT_BYTES - 51) / 3];
        entries.last_mut().unwrap().1.push(0);
        entries.insert(0, (*b"mh", mmr_root.encode()));
        let commitment = Commitment {
            payload: sp_consensus_beefy::Payload::decode(&mut &entries.encode()[..]).unwrap(),
            block_number: 1,
            validator_set_id: 0,
        };
        assert_eq!(commitment.encoded_size(), MAX_COMMITMENT_BYTES);
        let commitment_hash = Keccak256::hash_of(&commitment);
        // Public synthetic signatures with r=G.x and distinct nonzero s recover
        // distinct public keys for this fixed message. No signing key is used.
        // Setup recovery and Merkle construction are outside the measured call.
        let mut signatures = Vec::new();
        let mut addresses = Vec::new();
        for i in 0..v {
            let (mut signature, _) = signature_vector();
            signature[32..64].fill(0);
            signature[60..64].copy_from_slice(&(i + 1).to_be_bytes());
            addresses.push(recover_signature(&signature, &commitment_hash).expect("valid synthetic signature"));
            signatures.push(signature);
        }
        let root = binary_merkle_tree::merkle_root::<Keccak256, _>(addresses.clone());
        let set = ValidatorSet { id: 0, len: v, keyset_commitment: root };
        CurrentValidatorSet::<T>::insert(network, set.clone());
        NextValidatorSet::<T>::insert(network, ValidatorSet { id: 1, ..set });
        LatestMMRRoots::<T>::insert(network, VecDeque::from(vec![H256::zero(); MMR_ROOT_HISTORY_SIZE]));
        let claims = Pallet::<T>::create_initial_bitfield(&(0..v).collect::<Vec<_>>(), v as usize);
        let sample = Pallet::<T>::create_random_bit_field(network, claims.clone(), v)
            .expect("well-formed benchmark sampler");
        let positions = (0..v).filter(|i| sample.is_set(*i as usize)).collect::<Vec<_>>();
        let validator_proof = ValidatorProof {
            validator_claims_bitfield: claims,
            signatures: positions.iter().map(|i| signatures[*i as usize].to_vec()).collect(),
            positions: positions.iter().map(|i| u128::from(*i)).collect(),
            public_keys: positions.iter().map(|i| addresses[*i as usize]).collect(),
            public_key_merkle_proofs: positions.iter().map(|i| binary_merkle_tree::merkle_proof::<Keccak256, _, _>(addresses.clone(), *i).proof).collect(),
        };
    }: _(RawOrigin::Signed(caller), network, commitment, validator_proof, leaf, mmr)
    verify {
        assert_eq!(LatestBeefyBlock::<T>::get(network), 1);
        assert_eq!(CurrentValidatorSet::<T>::get(network).unwrap().id, 1);
        assert_eq!(NextValidatorSet::<T>::get(network).unwrap().id, 2);
        assert_eq!(LatestMMRRoots::<T>::get(network).len(), MMR_ROOT_HISTORY_SIZE);
    }

    recover_signatures {
        let s in 1..Pallet::<T>::get_required_number_of_signatures(T::MaxValidators::get());
        let (signature, hash) = signature_vector();
        let expected = recover_signature(&signature, &hash).expect("valid fixed signature");
    }: {
        for _ in 0..s {
            assert_eq!(recover_signature(&signature, &hash), Some(expected));
        }
    }

    sampling_attempts {
        let a in 1..Pallet::<T>::sampling_attempts(T::MaxValidators::get());
        // One missing bit makes completion impossible while almost every draw
        // also checks the output bitfield. This exercises both successful draws
        // and duplicate rejection for the entire budget, not only empty claims.
        let validators = T::MaxValidators::get();
        let prior = BitField::create_bitfield(
            &(0..validators.saturating_sub(1)).collect::<Vec<_>>(), validators as usize,
        );
    }: {
        assert!(BitField::create_random_bitfield_bounded(&prior, validators, validators, 0, a).is_none());
    }

    merkle_nodes {
        let p in 1..(MAX_VALIDATOR_PROOF_ITEMS as u32 * Pallet::<T>::get_required_number_of_signatures(T::MaxValidators::get()) + 63);
        let mut root = H256::zero();
    }: {
        for _ in 0..p { root = hasher(root, H256::zero()); }
        core::hint::black_box(root);
    }

    commitment_bytes {
        let b in 1..MAX_COMMITMENT_BYTES as u32;
        let commitment = Commitment {
            payload: sp_consensus_beefy::Payload::from_single_entry(*b"mh", vec![0; b as usize]),
            block_number: 1,
            validator_set_id: 0,
        };
    }: {
        core::hint::black_box(Keccak256::hash_of(&commitment));
    }

    commitment_items {
        let i in 1..((MAX_COMMITMENT_BYTES - 16) / 3) as u32;
        let entries = vec![(*b"zz", Vec::<u8>::new()); i as usize];
        let commitment = Commitment {
            payload: sp_consensus_beefy::Payload::decode(&mut &entries.encode()[..]).unwrap(),
            block_number: 1,
            validator_set_id: 0,
        };
    }: {
        // Admission weight calculation, dispatch resource check, and signing hash.
        core::hint::black_box(commitment.encoded_size());
        core::hint::black_box(commitment.encoded_size());
        core::hint::black_box(Keccak256::hash_of(&commitment));
    }

    verifier_verify {
        let d in 1..MAX_DIGEST_LOGS as u32;
        let commitment_hash = H256::repeat_byte(1);
        let mut logs = vec![AuxiliaryDigestItem::Commitment(SubNetworkId::Rococo.into(), H256::zero()); d as usize];
        logs[0] = AuxiliaryDigestItem::Commitment(SubNetworkId::Mainnet.into(), commitment_hash);
        let digest = AuxiliaryDigest { logs };
        let leaf = BeefyMMRLeaf {
            version: sp_consensus_beefy::mmr::MmrLeafVersion::new(0, 0),
            parent_number_and_hash: (0, H256::zero()),
            beefy_next_authority_set: ValidatorSet { id: 0, len: 1, keyset_commitment: H256::zero() },
            leaf_extra: bridge_types::types::LeafExtraData {
                digest_hash: Keccak256::hash_of(&digest),
                random_seed: H256::zero(),
            },
        };
        let proof = Proof { order: 0, items: vec![H256::zero(); 63] };
        let mut roots = VecDeque::from(vec![H256::zero(); MMR_ROOT_HISTORY_SIZE - 1]);
        roots.push_back(proof.root(hasher, Keccak256::hash_of(&leaf)));
        LatestMMRRoots::<T>::insert(SubNetworkId::Mainnet, roots);
        let message = SubstrateBridgeMessageProof { proof, leaf, digest };
    }: {
        Pallet::<T>::verify(SubNetworkId::Mainnet.into(), commitment_hash, &message)?;
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test)
}
