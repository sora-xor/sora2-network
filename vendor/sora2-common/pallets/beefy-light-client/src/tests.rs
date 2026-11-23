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

use crate::mock::*;
use bridge_common::beefy_types::BeefyMMRLeaf;
use bridge_common::beefy_types::ValidatorProof;
use bridge_common::beefy_types::ValidatorSet;
use bridge_common::bitfield::BitField;
use bridge_types::SubNetworkId;

use crate::fixtures::{generate_fixture, Fixture};
use codec::Decode;
use frame_support::{assert_noop, assert_ok, traits::Get};
use hex_literal::hex;
use test_case::test_case;

fn alice<T: crate::Config>() -> T::AccountId {
    T::AccountId::decode(&mut [0u8; 32].as_slice()).unwrap()
}

fn validator_proof(
    fixture: &Fixture,
    signatures: Vec<Option<sp_consensus_beefy::ecdsa_crypto::Signature>>,
    count: usize,
) -> ValidatorProof {
    let bits_to_set = signatures
        .iter()
        .enumerate()
        .filter_map(|(i, x)| x.clone().map(|_| i as u32))
        .take(count)
        .collect::<Vec<_>>();
    let initial_bitfield = BitField::create_bitfield(&bits_to_set, signatures.len());
    let random_bitfield = BeefyLightClient::create_random_bit_field(
        SubNetworkId::Mainnet,
        initial_bitfield.clone(),
        signatures.len() as u32,
    )
    .unwrap();
    let mut positions = vec![];
    let mut proof_signatures = vec![];
    let mut public_keys = vec![];
    let mut public_key_merkle_proofs = vec![];
    for i in 0..random_bitfield.len() {
        let bit = random_bitfield.is_set(i);
        if bit {
            positions.push(i as u128);
            let mut signature = signatures.get(i).unwrap().clone().unwrap().to_vec();
            signature[64] += 27;
            proof_signatures.push(signature);
            public_keys.push(fixture.addresses[i]);
            public_key_merkle_proofs.push(fixture.validator_set_proofs[i].clone());
        }
    }
    ValidatorProof {
        signatures: proof_signatures,
        positions,
        public_keys,
        public_key_merkle_proofs,
        validator_claims_bitfield: initial_bitfield,
    }
}

#[test_case(3, 5; "3 validators, 5 leaves")]
#[test_case(3, 5000; "3 validators, 5000 leaves")]
// #[test_case(3, 5000000; "3 validators, 5000000 leaves")] TODO uncomment when #372 is done, now takes too long time
#[test_case(37, 5; "37 validators, 5 leaves")]
#[test_case(37, 5000; "37 validators, 5000 leaves")]
#[test_case(69, 5000; "69 validators, 5000 leaves")]
#[test_case(200, 5000; "200 validators, 5000 leaves")]
fn submit_fixture_success(validators: usize, tree_size: u32) {
    new_test_ext().execute_with(|| {
        let fixture = generate_fixture(validators, tree_size).expect("error generating fixture");
        let validator_set = fixture.validator_set.clone().into();
        let next_validator_set = fixture.next_validator_set.clone().into();
        assert_ok!(BeefyLightClient::initialize(
            RuntimeOrigin::root(),
            SubNetworkId::Mainnet,
            0,
            validator_set,
            next_validator_set
        ));

        let signed_commitment: sp_consensus_beefy::SignedCommitment<
            u32,
            sp_consensus_beefy::ecdsa_crypto::Signature,
        > = Decode::decode(&mut &fixture.commitment[..]).unwrap();
        let commitment = signed_commitment.commitment.clone();
        let validator_proof = validator_proof(&fixture, signed_commitment.signatures, validators);
        let leaf: BeefyMMRLeaf = Decode::decode(&mut &fixture.leaf[..]).unwrap();

        assert_ok!(BeefyLightClient::submit_signature_commitment(
            RuntimeOrigin::signed(alice::<Test>()),
            SubNetworkId::Mainnet,
            commitment,
            validator_proof,
            leaf,
            fixture.leaf_proof.into(),
        ));
    });
}

#[test]
fn it_works_initialize_pallet() {
    new_test_ext().execute_with(|| {
        let root = hex!("36ee7c9903f810b22f7e6fca82c1c0cd6a151eca01f087683d92333094d94dc1");
        assert_ok!(
            BeefyLightClient::initialize(
                RuntimeOrigin::root(),
                SubNetworkId::Mainnet,
                1,
                ValidatorSet {
                    id: 0,
                    len: 3,
                    keyset_commitment: root.into(),
                },
                ValidatorSet {
                    id: 1,
                    len: 3,
                    keyset_commitment: root.into(),
                }
            ),
            ().into()
        )
    });
}

#[test]
fn malformed_validator_claims_are_rejected_before_sampling() {
    new_test_ext().execute_with(|| {
        // The old threshold counted these three claims even though none is in
        // the three-validator set; random sampling could never terminate.
        let outside_set = BitField::create_bitfield(&[3, 4, 5], 6);
        assert_noop!(
            BeefyLightClient::check_commitment_signatures_threshold(3, &outside_set),
            crate::Error::<Test>::InvalidValidatorClaimsBitfield
        );
        let short = BitField::create_bitfield(&[0, 1, 2], 3);
        assert_noop!(
            BeefyLightClient::check_commitment_signatures_threshold(4, &short),
            crate::Error::<Test>::InvalidValidatorClaimsBitfield
        );
        assert_noop!(
            BeefyLightClient::check_commitment_signatures_threshold(0, &BitField::with_zeroes(0)),
            crate::Error::<Test>::InvalidValidatorSetLength
        );
        let padded = BitField::try_from_slice(&[0b1110_0000]).unwrap();
        assert_ok!(BeefyLightClient::check_commitment_signatures_threshold(
            3, &padded
        ));
        let selected =
            BeefyLightClient::create_random_bit_field(SubNetworkId::Mainnet, padded, 3).unwrap();
        assert_eq!(selected.count_set_bits(), 2);
        assert!(!selected.0[3..].any());
    });
}

#[test]
fn sampling_rejects_impossible_requests_and_preserves_valid_selection() {
    new_test_ext().execute_with(|| {
        let prior = BitField::create_bitfield(&[0, 2, 3], 4);
        assert!(matches!(
            BeefyLightClient::random_n_bits_with_prior_check(SubNetworkId::Mainnet, &prior, 4, 4),
            Err(crate::Error::<Test>::NotEnoughValidatorSignatures)
        ));
        assert!(matches!(
            BeefyLightClient::random_n_bits_with_prior_check(SubNetworkId::Mainnet, &prior, 2, 3),
            Err(crate::Error::<Test>::InvalidValidatorClaimsBitfield)
        ));
        assert!(matches!(
            BeefyLightClient::create_random_bit_field(
                SubNetworkId::Mainnet,
                BitField::with_zeroes(0),
                0
            ),
            Err(crate::Error::<Test>::InvalidValidatorSetLength)
        ));
        let selected =
            BeefyLightClient::random_n_bits_with_prior_check(SubNetworkId::Mainnet, &prior, 2, 4)
                .unwrap();
        assert_eq!(selected.count_set_bits(), 2);
        assert!(!selected.is_set(1));
        assert_eq!(
            BeefyLightClient::random_n_bits_with_prior_check(SubNetworkId::Mainnet, &prior, 3, 4)
                .unwrap(),
            prior
        );
    });
}

#[test]
fn validator_position_is_checked_before_bitfield_access() {
    new_test_ext().execute_with(|| {
        let vset = ValidatorSet {
            id: 0,
            len: 3,
            keyset_commitment: Default::default(),
        };
        for position in [3, u128::MAX] {
            let mut selected = BitField::create_bitfield(&[0, 1, 2], 3);
            assert_noop!(
                BeefyLightClient::verify_validator_signature(
                    &vset,
                    &mut selected,
                    vec![],
                    position,
                    Default::default(),
                    &[],
                    Default::default(),
                ),
                crate::Error::<Test>::MerklePositionTooHigh
            );
        }
    });
}

#[test]
fn signed_submission_rejects_claims_outside_the_validator_set() {
    new_test_ext().execute_with(|| {
        let fixture = generate_fixture(3, 5).unwrap();
        assert_ok!(BeefyLightClient::initialize(
            RuntimeOrigin::root(),
            SubNetworkId::Mainnet,
            0,
            fixture.validator_set.clone().into(),
            fixture.next_validator_set.clone().into(),
        ));
        let signed_commitment: sp_consensus_beefy::SignedCommitment<
            u32,
            sp_consensus_beefy::ecdsa_crypto::Signature,
        > = Decode::decode(&mut &fixture.commitment[..]).unwrap();
        let validator_proof = ValidatorProof {
            signatures: vec![],
            positions: vec![],
            public_keys: vec![],
            public_key_merkle_proofs: vec![],
            validator_claims_bitfield: BitField::create_bitfield(&[3, 4, 5], 6),
        };
        let leaf = Decode::decode(&mut &fixture.leaf[..]).unwrap();
        assert_noop!(
            BeefyLightClient::submit_signature_commitment(
                RuntimeOrigin::signed(alice::<Test>()),
                SubNetworkId::Mainnet,
                signed_commitment.commitment,
                validator_proof,
                leaf,
                fixture.leaf_proof.into(),
            ),
            crate::Error::<Test>::InvalidValidatorClaimsBitfield
        );
    });
}

#[test]
fn message_verifier_rejects_oversized_mmr_proofs() {
    use bridge_types::traits::Verifier;
    new_test_ext().execute_with(|| {
        let fixture = generate_fixture(3, 5).unwrap();
        let leaf: BeefyMMRLeaf = Decode::decode(&mut &fixture.leaf[..]).unwrap();
        let boundary = bridge_common::simplified_proof::Proof {
            order: 0,
            items: vec![Default::default(); 63],
        };
        BeefyLightClient::add_known_mmr_root(
            SubNetworkId::Mainnet,
            boundary.root(bridge_common::simplified_proof::hasher, Default::default()),
        );
        assert!(BeefyLightClient::verify_beefy_merkle_leaf(
            SubNetworkId::Mainnet,
            Default::default(),
            &boundary,
        ));
        for length in [64, 65] {
            let proof = bridge_common::simplified_proof::Proof {
                order: 0,
                items: vec![Default::default(); length],
            };
            if length == 64 {
                BeefyLightClient::add_known_mmr_root(
                    SubNetworkId::Mainnet,
                    proof.root(bridge_common::simplified_proof::hasher, Default::default()),
                );
            }
            assert!(!BeefyLightClient::verify_beefy_merkle_leaf(
                SubNetworkId::Mainnet,
                Default::default(),
                &proof,
            ));
            let message_proof = crate::SubstrateBridgeMessageProof {
                proof,
                leaf: leaf.clone(),
                digest: bridge_types::types::AuxiliaryDigest { logs: vec![] },
            };
            assert_noop!(
                BeefyLightClient::verify(
                    SubNetworkId::Mainnet.into(),
                    Default::default(),
                    &message_proof,
                ),
                crate::Error::<Test>::InvalidMMRProof
            );
        }
    });
}

#[derive(Clone)]
struct SignedTestCommitment {
    commitment: bridge_common::beefy_types::Commitment,
    validator_set: ValidatorSet,
    next_validator_set: ValidatorSet,
    validator_proof: ValidatorProof,
    leaf: BeefyMMRLeaf,
}

impl SignedTestCommitment {
    fn new() -> Self {
        use codec::Encode;
        use sp_core::Pair;
        use sp_runtime::traits::{Convert, Hash, Keccak256};

        let pair = sp_core::ecdsa::Pair::from_seed(&[1; 32]);
        let address = sp_core::H160::from_slice(&pallet_beefy_mmr::BeefyEcdsaToEthereum::convert(
            pair.public().into(),
        ));
        let validator_set = ValidatorSet {
            id: 7,
            len: 1,
            keyset_commitment: binary_merkle_tree::merkle_root::<Keccak256, _>([address]),
        };
        let next_validator_set = ValidatorSet {
            id: 8,
            ..validator_set.clone()
        };
        let leaf = BeefyMMRLeaf {
            version: sp_consensus_beefy::mmr::MmrLeafVersion::new(0, 0),
            parent_number_and_hash: (9, sp_core::H256::repeat_byte(9)),
            beefy_next_authority_set: ValidatorSet {
                id: 9,
                ..validator_set.clone()
            },
            leaf_extra: bridge_types::types::LeafExtraData {
                random_seed: sp_core::H256::repeat_byte(42),
                digest_hash: Default::default(),
            },
        };
        let commitment = bridge_common::beefy_types::Commitment {
            payload: sp_consensus_beefy::Payload::from_single_entry(
                sp_consensus_beefy::known_payloads::MMR_ROOT_ID,
                Keccak256::hash_of(&leaf).encode(),
            ),
            block_number: 10,
            validator_set_id: validator_set.id,
        };
        let mut fixture = Self {
            commitment,
            validator_set,
            next_validator_set,
            validator_proof: ValidatorProof {
                validator_claims_bitfield: BitField::create_bitfield(&[0], 1),
                signatures: vec![],
                positions: vec![0],
                public_keys: vec![address],
                public_key_merkle_proofs: vec![vec![]],
            },
            leaf,
        };
        fixture.resign();
        fixture
    }

    fn resign(&mut self) {
        use sp_core::Pair;
        use sp_runtime::traits::{Hash, Keccak256};
        self.validator_proof.signatures = vec![sp_core::ecdsa::Pair::from_seed(&[1; 32])
            .sign_prehashed(&Keccak256::hash_of(&self.commitment).0)
            .to_vec()];
    }

    fn initialize(&self) {
        assert_ok!(BeefyLightClient::initialize(
            RuntimeOrigin::root(),
            SubNetworkId::Mainnet,
            0,
            self.validator_set.clone(),
            self.next_validator_set.clone(),
        ));
    }

    fn submit(self) -> frame_support::dispatch::DispatchResultWithPostInfo {
        BeefyLightClient::submit_signature_commitment(
            RuntimeOrigin::signed(1),
            SubNetworkId::Mainnet,
            self.commitment,
            self.validator_proof,
            self.leaf,
            bridge_common::simplified_proof::Proof {
                order: 0,
                items: vec![],
            },
        )
    }
}

#[test]
fn initialization_requires_root_and_submission_requires_a_signed_initialized_network() {
    new_test_ext().execute_with(|| {
        let fixture = SignedTestCommitment::new();
        assert_noop!(
            BeefyLightClient::initialize(
                RuntimeOrigin::signed(1),
                SubNetworkId::Mainnet,
                0,
                fixture.validator_set.clone(),
                fixture.next_validator_set.clone(),
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_noop!(
            fixture.clone().submit(),
            crate::Error::<Test>::PalletNotInitialized
        );
        crate::CurrentValidatorSet::<Test>::insert(
            SubNetworkId::Mainnet,
            fixture.validator_set.clone(),
        );
        assert_noop!(
            fixture.clone().submit(),
            crate::Error::<Test>::PalletNotInitialized
        );
        fixture.initialize();
        assert_noop!(
            BeefyLightClient::submit_signature_commitment(
                RuntimeOrigin::root(),
                SubNetworkId::Mainnet,
                fixture.commitment.clone(),
                fixture.validator_proof.clone(),
                fixture.leaf.clone(),
                bridge_common::simplified_proof::Proof {
                    order: 0,
                    items: vec![]
                },
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        let mut unknown_set = fixture;
        unknown_set.commitment.validator_set_id = 100;
        assert_noop!(
            unknown_set.submit(),
            crate::Error::<Test>::InvalidValidatorSetId
        );
    });
}

#[test]
fn successful_submission_updates_roots_seed_events_and_rotates_authorities() {
    use sp_runtime::traits::{Hash, Keccak256};
    new_test_ext().execute_with(|| {
        System::set_block_number(12);
        let fixture = SignedTestCommitment::new();
        fixture.initialize();
        let root = Keccak256::hash_of(&fixture.leaf);
        assert_ok!(fixture.clone().submit());
        assert_eq!(
            BeefyLightClient::latest_beefy_block(SubNetworkId::Mainnet),
            10
        );
        assert_eq!(
            BeefyLightClient::get_latest_mmr_root(SubNetworkId::Mainnet),
            Some(root)
        );
        assert_eq!(
            BeefyLightClient::latest_random_seed(SubNetworkId::Mainnet),
            (fixture.leaf.leaf_extra.random_seed, 12)
        );
        assert_eq!(
            BeefyLightClient::current_validator_set(SubNetworkId::Mainnet),
            Some(fixture.next_validator_set)
        );
        assert_eq!(
            BeefyLightClient::next_validator_set(SubNetworkId::Mainnet),
            Some(fixture.leaf.beefy_next_authority_set)
        );
        System::assert_has_event(crate::Event::NewMMRRoot(SubNetworkId::Mainnet, root, 10).into());
        System::assert_last_event(
            crate::Event::VerificationSuccessful(SubNetworkId::Mainnet, 1, 10).into(),
        );
    });
}

#[test]
fn next_authority_set_can_sign_and_stale_mmr_commitments_leave_storage_unchanged() {
    new_test_ext().execute_with(|| {
        let mut fixture = SignedTestCommitment::new();
        fixture.commitment.validator_set_id = fixture.next_validator_set.id;
        fixture.resign();
        fixture.initialize();
        assert_ok!(fixture.clone().submit());
        assert_noop!(
            fixture.submit(),
            crate::Error::<Test>::PayloadBlocknumberTooOld
        );
    });
}

#[test]
fn authenticated_commitments_require_an_mmr_payload_and_matching_leaf() {
    new_test_ext().execute_with(|| {
        let fixture = SignedTestCommitment::new();
        fixture.initialize();
        for payload in [
            sp_consensus_beefy::Payload::from_single_entry(*b"xx", vec![0; 32]),
            sp_consensus_beefy::Payload::from_single_entry(
                sp_consensus_beefy::known_payloads::MMR_ROOT_ID,
                vec![0],
            ),
        ] {
            let mut invalid = fixture.clone();
            invalid.commitment.payload = payload;
            invalid.resign();
            assert_noop!(invalid.submit(), crate::Error::<Test>::MMRPayloadNotFound);
        }
        let mut invalid_leaf = fixture;
        invalid_leaf.leaf.parent_number_and_hash.0 += 1;
        assert_noop!(invalid_leaf.submit(), crate::Error::<Test>::InvalidMMRProof);
    });
}

#[test]
fn validator_proof_vectors_must_have_exactly_the_sampled_signature_count() {
    new_test_ext().execute_with(|| {
        let fixture = SignedTestCommitment::new();
        fixture.initialize();
        for vector in 0..4 {
            let mut invalid = fixture.clone();
            let error = match vector {
                0 => {
                    invalid.validator_proof.signatures.clear();
                    crate::Error::<Test>::InvalidNumberOfSignatures
                }
                1 => {
                    invalid.validator_proof.positions.clear();
                    crate::Error::<Test>::InvalidNumberOfPositions
                }
                2 => {
                    invalid.validator_proof.public_keys.clear();
                    crate::Error::<Test>::InvalidNumberOfPublicKeys
                }
                _ => {
                    invalid.validator_proof.public_key_merkle_proofs.clear();
                    crate::Error::<Test>::InvalidNumberOfPublicKeys
                }
            };
            assert_noop!(invalid.submit(), error);
        }
        assert_noop!(
            BeefyLightClient::check_commitment_signatures_threshold(
                4,
                &BitField::create_bitfield(&[0, 1], 4)
            ),
            crate::Error::<Test>::NotEnoughValidatorSignatures
        );
    });
}

#[test]
fn invalid_signatures_and_unproven_authorities_cannot_advance_the_bridge() {
    use sp_core::Pair;
    use sp_runtime::traits::{Hash, Keccak256};
    new_test_ext().execute_with(|| {
        let fixture = SignedTestCommitment::new();
        fixture.initialize();
        for signature in [
            vec![0; 64],
            vec![0; 66],
            vec![0; 65],
            sp_core::ecdsa::Pair::from_seed(&[2; 32])
                .sign_prehashed(&Keccak256::hash_of(&fixture.commitment).0)
                .to_vec(),
        ] {
            let mut invalid = fixture.clone();
            invalid.validator_proof.signatures[0] = signature;
            assert_noop!(invalid.submit(), crate::Error::<Test>::InvalidSignature);
        }
        let mut invalid = fixture.clone();
        invalid.validator_proof.public_keys[0] = sp_core::H160::zero();
        assert_noop!(
            invalid.submit(),
            crate::Error::<Test>::ValidatorSetIncorrectPosition
        );
        assert_noop!(
            BeefyLightClient::check_validator_in_set(
                &fixture.validator_set,
                fixture.validator_proof.public_keys[0],
                u128::MAX,
                &[]
            ),
            crate::Error::<Test>::MerklePositionTooHigh
        );

        let mut selected = BitField::create_bitfield(&[0], 1);
        for expected_success in [true, false] {
            let result = BeefyLightClient::verify_validator_signature(
                &fixture.validator_set,
                &mut selected,
                fixture.validator_proof.signatures[0].clone(),
                0,
                fixture.validator_proof.public_keys[0],
                &[],
                Keccak256::hash_of(&fixture.commitment),
            );
            if expected_success {
                assert_ok!(result);
            } else {
                assert_noop!(result, crate::Error::<Test>::ValidatorNotOnceInbitfield);
            }
        }
    });
}

#[test]
fn known_mmr_roots_are_network_scoped_and_history_is_bounded() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            BeefyLightClient::get_latest_mmr_root(SubNetworkId::Mainnet),
            None
        );
        let roots: Vec<_> = (0..=crate::MMR_ROOT_HISTORY_SIZE)
            .map(|i| sp_core::H256::from_low_u64_be(i as u64))
            .collect();
        for root in &roots {
            BeefyLightClient::add_known_mmr_root(SubNetworkId::Mainnet, *root);
        }
        let stored = BeefyLightClient::latest_mmr_roots(SubNetworkId::Mainnet);
        assert_eq!(stored.len(), crate::MMR_ROOT_HISTORY_SIZE);
        assert_eq!(stored.front(), roots.last());
        assert!(!BeefyLightClient::is_known_root(
            SubNetworkId::Mainnet,
            roots[0]
        ));
        assert!(BeefyLightClient::is_known_root(
            SubNetworkId::Mainnet,
            roots[1]
        ));
        assert!(!BeefyLightClient::is_known_root(
            SubNetworkId::Kusama,
            roots[1]
        ));
    });
}

#[test]
fn authority_changes_ignore_equal_or_older_sets_and_require_initialization() {
    new_test_ext().execute_with(|| {
        let fixture = SignedTestCommitment::new();
        assert_noop!(
            BeefyLightClient::apply_validator_set_changes(
                SubNetworkId::Mainnet,
                fixture.next_validator_set.clone()
            ),
            crate::Error::<Test>::PalletNotInitialized
        );
        fixture.initialize();
        for set in [
            fixture.validator_set.clone(),
            fixture.next_validator_set.clone(),
        ] {
            assert_ok!(BeefyLightClient::apply_validator_set_changes(
                SubNetworkId::Mainnet,
                set
            ));
            assert_eq!(
                BeefyLightClient::current_validator_set(SubNetworkId::Mainnet),
                Some(fixture.validator_set.clone())
            );
            assert_eq!(
                BeefyLightClient::next_validator_set(SubNetworkId::Mainnet),
                Some(fixture.next_validator_set.clone())
            );
        }
    });
}

#[test]
fn message_verification_requires_one_matching_destination_commitment_and_authenticated_digest() {
    use bridge_types::{
        traits::Verifier,
        types::{AuxiliaryDigest, AuxiliaryDigestItem},
        GenericNetworkId,
    };
    use sp_runtime::traits::{Hash, Keccak256};
    new_test_ext().execute_with(|| {
        crate::ThisNetworkId::<Test>::put(SubNetworkId::Kusama);
        let commitment_hash = sp_core::H256::repeat_byte(4);
        let matching =
            AuxiliaryDigestItem::Commitment(SubNetworkId::Kusama.into(), commitment_hash);
        let mut proof = crate::SubstrateBridgeMessageProof {
            proof: bridge_common::simplified_proof::Proof {
                order: 0,
                items: vec![],
            },
            leaf: SignedTestCommitment::new().leaf,
            digest: AuxiliaryDigest {
                logs: vec![matching],
            },
        };
        assert_noop!(
            BeefyLightClient::verify(GenericNetworkId::EVMLegacy(1), commitment_hash, &proof),
            crate::Error::<Test>::InvalidNetworkId
        );
        assert_noop!(
            BeefyLightClient::verify(SubNetworkId::Mainnet.into(), commitment_hash, &proof),
            crate::Error::<Test>::InvalidMMRProof
        );
        BeefyLightClient::add_known_mmr_root(
            SubNetworkId::Mainnet,
            Keccak256::hash_of(&proof.leaf),
        );
        assert_noop!(
            BeefyLightClient::verify(SubNetworkId::Mainnet.into(), commitment_hash, &proof),
            crate::Error::<Test>::InvalidMMRProof
        );
        for logs in [
            vec![],
            vec![matching, matching],
            vec![AuxiliaryDigestItem::Commitment(
                SubNetworkId::Mainnet.into(),
                commitment_hash,
            )],
            vec![AuxiliaryDigestItem::Commitment(
                SubNetworkId::Kusama.into(),
                sp_core::H256::zero(),
            )],
            vec![AuxiliaryDigestItem::Commitment(
                GenericNetworkId::EVMLegacy(1),
                commitment_hash,
            )],
        ] {
            proof.digest.logs = logs;
            proof.leaf.leaf_extra.digest_hash = Keccak256::hash_of(&proof.digest);
            BeefyLightClient::add_known_mmr_root(
                SubNetworkId::Mainnet,
                Keccak256::hash_of(&proof.leaf),
            );
            assert_noop!(
                BeefyLightClient::verify(SubNetworkId::Mainnet.into(), commitment_hash, &proof),
                crate::Error::<Test>::CommitmentNotFoundInDigest
            );
        }
        proof.digest.logs = vec![
            matching,
            AuxiliaryDigestItem::Commitment(GenericNetworkId::EVMLegacy(1), commitment_hash),
        ];
        proof.leaf.leaf_extra.digest_hash = Keccak256::hash_of(&proof.digest);
        BeefyLightClient::add_known_mmr_root(
            SubNetworkId::Mainnet,
            Keccak256::hash_of(&proof.leaf),
        );
        assert_ok!(BeefyLightClient::verify(
            SubNetworkId::Mainnet.into(),
            commitment_hash,
            &proof
        ));
        assert_eq!(
            BeefyLightClient::verify_weight(&proof),
            <crate::weights::SubstrateWeight<Test> as crate::weights::WeightInfo>::verify(0, 2)
        );
    });
}

#[test]
fn genesis_network_randomness_and_signature_rounding_are_consistent() {
    use frame_support::traits::{BuildGenesisConfig, Randomness};
    use sp_runtime::traits::{Hash, Keccak256};
    new_test_ext().execute_with(|| {
        assert_eq!(BeefyLightClient::this_network_id(), SubNetworkId::Mainnet);
        let mut genesis = crate::GenesisConfig::<Test>::default();
        genesis.network_id = SubNetworkId::Kusama;
        genesis.build();
        assert_eq!(BeefyLightClient::this_network_id(), SubNetworkId::Kusama);
        let seed = (sp_core::H256::repeat_byte(8), 19);
        crate::LatestRandomSeed::<Test>::insert(SubNetworkId::Mainnet, seed);
        type Random = crate::SidechainRandomness<Test, SidechainRandomnessNetwork>;
        assert_eq!(Random::random_seed(), seed);
        assert_eq!(
            Random::random(b"example"),
            (Keccak256::hash_of(&(b"example".as_slice(), seed.0)), seed.1)
        );
        assert_ne!(Random::random(b"example"), Random::random(b"different"));
        assert_eq!(
            BeefyLightClient::create_initial_bitfield(&[1, 3], 5),
            BitField::create_bitfield(&[1, 3], 5)
        );
        for len in [0, 1, 2, 3, 59, 60, u32::MAX] {
            let set = ValidatorSet {
                id: 0,
                len,
                keyset_commitment: Default::default(),
            };
            let expected = ((u64::from(len) * 22 + 58) / 59) as u32;
            assert_eq!(
                BeefyLightClient::required_number_of_signatures(&set),
                expected
            );
        }
    });
}

#[test]
fn resource_limits_bound_submission_weights_before_crypto_work() {
    use frame_support::dispatch::GetDispatchInfo;
    use frame_support::weights::Weight;
    new_test_ext().execute_with(|| {
        let fixture = SignedTestCommitment::new();
        let initialize = crate::Call::<Test>::initialize {
            network_id: SubNetworkId::Mainnet,
            latest_beefy_block: 0,
            validator_set: fixture.validator_set.clone(),
            next_validator_set: fixture.next_validator_set.clone(),
        };
        assert!(initialize.get_dispatch_info().call_weight.ref_time() > 0);
        let mmr = bridge_common::simplified_proof::Proof {
            order: 0,
            items: vec![],
        };
        let baseline = BeefyLightClient::submission_weight(
            &fixture.commitment,
            &fixture.validator_proof,
            &mmr,
        );
        assert!(baseline.ref_time() > 0);
        assert_ne!(baseline, Weight::MAX);
        let max = <<Test as crate::Config>::MaxValidators as Get<u32>>::get() as usize;
        let signatures = BeefyLightClient::get_required_number_of_signatures(max as u32) as usize;
        for dimension in 0..5 {
            let mut proof = fixture.validator_proof.clone();
            match dimension {
                0 => proof.validator_claims_bitfield = BitField::with_zeroes(max + 8),
                1 => proof.signatures = vec![vec![0; 65]; signatures + 1],
                2 => proof.positions = vec![0; signatures + 1],
                3 => proof.public_keys = vec![Default::default(); signatures + 1],
                _ => proof.public_key_merkle_proofs = vec![vec![]; signatures + 1],
            }
            assert_eq!(
                BeefyLightClient::submission_weight(&fixture.commitment, &proof, &mmr),
                Weight::MAX
            );
            assert!(matches!(
                BeefyLightClient::check_submission_resources(&fixture.commitment, &proof, &mmr),
                Err(crate::Error::<Test>::ResourceLimitExceeded)
            ));
        }
        let oversized_mmr = bridge_common::simplified_proof::Proof {
            order: 0,
            items: vec![Default::default(); 64],
        };
        assert_eq!(
            BeefyLightClient::submission_weight(
                &fixture.commitment,
                &fixture.validator_proof,
                &oversized_mmr
            ),
            Weight::MAX
        );
        assert!(matches!(
            BeefyLightClient::check_submission_resources(
                &fixture.commitment,
                &fixture.validator_proof,
                &oversized_mmr
            ),
            Err(crate::Error::<Test>::InvalidMMRProof)
        ));
        let mut excessive_payload = fixture.commitment.clone();
        excessive_payload.payload = sp_consensus_beefy::Payload::from_single_entry(
            *b"xx",
            vec![0; crate::MAX_COMMITMENT_BYTES + 1],
        );
        assert_eq!(
            BeefyLightClient::submission_weight(&excessive_payload, &fixture.validator_proof, &mmr),
            Weight::MAX
        );
        assert!(matches!(
            BeefyLightClient::check_submission_resources(
                &excessive_payload,
                &fixture.validator_proof,
                &mmr
            ),
            Err(crate::Error::<Test>::ResourceLimitExceeded)
        ));
        let mut long_proof = fixture.validator_proof.clone();
        long_proof.public_key_merkle_proofs[0] =
            vec![Default::default(); crate::MAX_VALIDATOR_PROOF_ITEMS + 1];
        assert!(
            BeefyLightClient::submission_weight(&fixture.commitment, &long_proof, &mmr).ref_time()
                > baseline.ref_time()
        );
        assert!(matches!(
            BeefyLightClient::check_submission_resources(&fixture.commitment, &long_proof, &mmr),
            Err(crate::Error::<Test>::ResourceLimitExceeded)
        ));
    });
}

#[test]
fn authority_and_digest_resource_limits_reject_unbounded_inputs() {
    use bridge_types::{
        traits::Verifier,
        types::{AuxiliaryDigest, AuxiliaryDigestItem},
    };
    new_test_ext().execute_with(|| {
        let fixture = SignedTestCommitment::new();
        for length in [
            0,
            <<Test as crate::Config>::MaxValidators as Get<u32>>::get() + 1,
        ] {
            let invalid = ValidatorSet {
                len: length,
                ..fixture.validator_set.clone()
            };
            assert_noop!(
                BeefyLightClient::initialize(
                    RuntimeOrigin::root(),
                    SubNetworkId::Mainnet,
                    0,
                    invalid.clone(),
                    fixture.next_validator_set.clone()
                ),
                crate::Error::<Test>::InvalidValidatorSetLength
            );
            assert_noop!(
                BeefyLightClient::initialize(
                    RuntimeOrigin::root(),
                    SubNetworkId::Mainnet,
                    0,
                    fixture.validator_set.clone(),
                    invalid
                ),
                crate::Error::<Test>::InvalidValidatorSetLength
            );
        }
        let proof = crate::SubstrateBridgeMessageProof {
            proof: bridge_common::simplified_proof::Proof {
                order: 0,
                items: vec![],
            },
            leaf: fixture.leaf,
            digest: AuxiliaryDigest {
                logs: vec![
                    AuxiliaryDigestItem::Commitment(
                        SubNetworkId::Mainnet.into(),
                        Default::default()
                    );
                    crate::MAX_DIGEST_LOGS + 1
                ],
            },
        };
        assert_noop!(
            BeefyLightClient::verify(SubNetworkId::Mainnet.into(), Default::default(), &proof),
            crate::Error::<Test>::ResourceLimitExceeded
        );
    });
}

#[test]
fn resource_limits_reject_oversized_sets_proofs_and_payloads_without_writes() {
    use bridge_types::traits::Verifier;
    use codec::Encode;
    use frame_support::traits::Get;
    new_test_ext().execute_with(|| {
        let max: u32 = <<Test as crate::Config>::MaxValidators as Get<u32>>::get();
        let valid_set = ValidatorSet {
            id: 0,
            len: max,
            keyset_commitment: Default::default(),
        };
        for len in [0, max + 1] {
            let invalid_set = ValidatorSet {
                len,
                ..valid_set.clone()
            };
            assert_noop!(
                BeefyLightClient::initialize(
                    RuntimeOrigin::root(),
                    SubNetworkId::Mainnet,
                    0,
                    invalid_set.clone(),
                    valid_set.clone()
                ),
                crate::Error::<Test>::InvalidValidatorSetLength
            );
            assert_noop!(
                BeefyLightClient::initialize(
                    RuntimeOrigin::root(),
                    SubNetworkId::Mainnet,
                    0,
                    valid_set.clone(),
                    invalid_set.clone()
                ),
                crate::Error::<Test>::InvalidValidatorSetLength
            );
            assert_noop!(
                BeefyLightClient::apply_validator_set_changes(SubNetworkId::Mainnet, invalid_set),
                crate::Error::<Test>::InvalidValidatorSetLength
            );
        }
        assert_ok!(BeefyLightClient::initialize(
            RuntimeOrigin::root(),
            SubNetworkId::Mainnet,
            0,
            valid_set.clone(),
            valid_set
        ));
        let fixture = generate_fixture(3, 5).unwrap();
        let signed: sp_consensus_beefy::SignedCommitment<
            u32,
            sp_consensus_beefy::ecdsa_crypto::Signature,
        > = Decode::decode(&mut &fixture.commitment[..]).unwrap();
        let mut validator_proof = ValidatorProof {
            validator_claims_bitfield: BitField::create_bitfield(&[0, 1, 2], 3),
            signatures: vec![],
            positions: vec![],
            public_keys: vec![],
            public_key_merkle_proofs: vec![],
        };
        let mmr_proof = bridge_common::simplified_proof::Proof {
            order: 0,
            items: vec![],
        };
        let mut commitment = signed.commitment;
        validator_proof.public_key_merkle_proofs = vec![
            vec![];
            (max * crate::THRESHOLD_NUMERATOR / crate::THRESHOLD_DENOMINATOR + 2)
                as usize
        ];
        assert_eq!(
            BeefyLightClient::submission_weight(&commitment, &validator_proof, &mmr_proof),
            frame_support::weights::Weight::MAX
        );
        assert!(matches!(
            BeefyLightClient::check_submission_resources(&commitment, &validator_proof, &mmr_proof),
            Err(crate::Error::<Test>::ResourceLimitExceeded)
        ));
        validator_proof.public_key_merkle_proofs = vec![vec![
            Default::default();
            crate::MAX_VALIDATOR_PROOF_ITEMS + 1
        ]];
        assert!(matches!(
            BeefyLightClient::check_submission_resources(&commitment, &validator_proof, &mmr_proof),
            Err(crate::Error::<Test>::ResourceLimitExceeded)
        ));
        validator_proof.public_key_merkle_proofs.clear();
        commitment.payload = sp_consensus_beefy::Payload::from_single_entry(
            *b"mh",
            vec![0; crate::MAX_COMMITMENT_BYTES],
        );
        assert!(commitment.encoded_size() > crate::MAX_COMMITMENT_BYTES);
        assert_eq!(
            BeefyLightClient::submission_weight(&commitment, &validator_proof, &mmr_proof),
            frame_support::weights::Weight::MAX
        );
        assert!(matches!(
            BeefyLightClient::check_submission_resources(&commitment, &validator_proof, &mmr_proof),
            Err(crate::Error::<Test>::ResourceLimitExceeded)
        ));
        let leaf = Decode::decode(&mut &fixture.leaf[..]).unwrap();
        let message_proof = crate::SubstrateBridgeMessageProof {
            proof: mmr_proof,
            leaf,
            digest: bridge_types::types::AuxiliaryDigest {
                logs: vec![
                    bridge_types::types::AuxiliaryDigestItem::Commitment(
                        SubNetworkId::Mainnet.into(),
                        Default::default()
                    );
                    crate::MAX_DIGEST_LOGS + 1
                ],
            },
        };
        assert_noop!(
            BeefyLightClient::verify(
                SubNetworkId::Mainnet.into(),
                Default::default(),
                &message_proof
            ),
            crate::Error::<Test>::ResourceLimitExceeded
        );
    });
}

#[test]
fn bounded_sampler_matches_legacy_selection_and_handles_seed_wraparound() {
    use frame_support::traits::Get;
    for length in [
        3,
        37,
        200,
        4096,
        <<Test as crate::Config>::MaxValidators as Get<u32>>::get(),
    ] {
        let bits: Vec<_> = (0..length).filter(|i| i % 3 != 2).collect();
        let prior = BitField::create_bitfield(&bits, length as usize);
        let count = BeefyLightClient::get_required_number_of_signatures(length);
        for seed in [0, 1, 1000, u64::MAX as u128] {
            let legacy = BitField::create_random_bitfield(&prior, count, length, seed);
            assert_eq!(
                BitField::create_random_bitfield_bounded(
                    &prior,
                    count,
                    length,
                    seed,
                    BeefyLightClient::sampling_attempts(length)
                ),
                Some(legacy)
            );
        }
    }
    let prior = BitField::create_bitfield(&[0, 1, 2], 3);
    let seed = u128::MAX;
    let mut expected = BitField::with_zeroes(3);
    for i in 0..BeefyLightClient::sampling_attempts(3) {
        let random = sp_io::hashing::blake2_128(&seed.wrapping_add(u128::from(i)).to_be_bytes());
        expected.set((u128::from_be_bytes(random) % 3) as usize);
        if expected.count_set_bits() == 2 {
            break;
        }
    }
    assert_eq!(
        BitField::create_random_bitfield_bounded(
            &prior,
            2,
            3,
            seed,
            BeefyLightClient::sampling_attempts(3)
        ),
        Some(expected)
    );
    assert_eq!(
        BitField::create_random_bitfield_bounded(&prior, 2, 3, 0, 1),
        None
    );
    assert_eq!(
        BitField::create_random_bitfield_bounded(&BitField::with_zeroes(3), 1, 3, 0, 4),
        None
    );
    new_test_ext().execute_with(|| {
        let max: u32 = <<Test as crate::Config>::MaxValidators as Get<u32>>::get();
        assert!(matches!(
            BeefyLightClient::create_random_bit_field(
                SubNetworkId::Mainnet,
                BitField::with_zeroes((max + 1) as usize),
                max + 1
            ),
            Err(crate::Error::<Test>::InvalidValidatorSetLength)
        ));
    });
}

#[test]
fn weights_charge_real_work_and_fit_the_normal_budget_at_supported_size() {
    use crate::weights::WeightInfo;
    use frame_support::{dispatch::GetDispatchInfo, traits::Get};
    type Costs = crate::weights::SubstrateWeight<Test>;
    let set = ValidatorSet {
        id: 0,
        len: 200,
        keyset_commitment: Default::default(),
    };
    let info = crate::Call::<Test>::initialize {
        network_id: SubNetworkId::Mainnet,
        latest_beefy_block: 0,
        validator_set: set.clone(),
        next_validator_set: set,
    }
    .get_dispatch_info();
    assert!(info.total_weight().ref_time() > 0);
    assert!(info.total_weight().proof_size() > 0);
    let base = Costs::submit_signature_commitment(10, 1, 1, 32);
    for weight in [
        Costs::submit_signature_commitment(11, 1, 1, 32),
        Costs::submit_signature_commitment(10, 2, 1, 32),
        Costs::submit_signature_commitment(10, 1, 2, 32),
        Costs::submit_signature_commitment(10, 1, 1, 33),
    ] {
        assert!(weight.ref_time() > base.ref_time());
    }
    assert!(Costs::verify(2, 1).ref_time() > Costs::verify(1, 1).ref_time());
    assert!(Costs::verify(1, 2).ref_time() > Costs::verify(1, 1).ref_time());
    let max: u32 = <<Test as crate::Config>::MaxValidators as Get<u32>>::get();
    let signatures = BeefyLightClient::get_required_number_of_signatures(max);
    let largest = Costs::submit_signature_commitment(
        BeefyLightClient::sampling_attempts(max + 7),
        signatures,
        signatures * crate::MAX_VALIDATOR_PROOF_ITEMS as u32 + 63,
        crate::MAX_COMMITMENT_BYTES as u32,
    );
    // The runtime allows 1.5s normal work minus initialization/base overhead.
    assert!(largest.ref_time() < 1_400_000_000_000);
}

/// Native diagnostic measurements, not a substitute for deployment Wasm benchmarks.
/// Run with --ignored --nocapture; fixture generation is excluded from timings.
#[test]
#[ignore]
fn native_resource_weight_calibration() {
    use std::time::Instant;
    let counts: Vec<usize> = std::env::var("BEEFY_CALIBRATION_VALIDATORS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(|n| n.parse().expect("validator count"))
                .collect()
        })
        .unwrap_or_else(|| vec![3, 37, 200, 1024]);
    for validators in counts {
        let repetitions = if validators > 1024 { 3 } else { 10 };
        new_test_ext().execute_with(|| {
            let fixture = generate_fixture(validators, 5).unwrap();
            assert_ok!(BeefyLightClient::initialize(RuntimeOrigin::root(), SubNetworkId::Mainnet, 0, fixture.validator_set.clone().into(), fixture.next_validator_set.clone().into()));
            let signed: sp_consensus_beefy::SignedCommitment<u32, sp_consensus_beefy::ecdsa_crypto::Signature> = Decode::decode(&mut &fixture.commitment[..]).unwrap();
            let proof = validator_proof(&fixture, signed.signatures, validators);
            let leaf: BeefyMMRLeaf = Decode::decode(&mut &fixture.leaf[..]).unwrap();
            let mmr_proof: bridge_common::simplified_proof::Proof<sp_core::H256> = fixture.leaf_proof.into();
            let charged_ps = BeefyLightClient::submission_weight(&signed.commitment, &proof, &mmr_proof).ref_time();
            let mut max_ns = 0;
            for _ in 0..repetitions {
                // Keep identical initial state/seed for this already signed fixture.
                crate::LatestBeefyBlock::<Test>::insert(SubNetworkId::Mainnet, 0);
                crate::LatestRandomSeed::<Test>::remove(SubNetworkId::Mainnet);
                let start = Instant::now();
                assert_ok!(BeefyLightClient::submit_signature_commitment(RuntimeOrigin::signed(alice::<Test>()), SubNetworkId::Mainnet, signed.commitment.clone(), proof.clone(), leaf.clone(), mmr_proof.clone()));
                max_ns = max_ns.max(start.elapsed().as_nanos());
            }
            println!("beefy_native_calibration validators={validators} repetitions={repetitions} max_ns={max_ns} charged_ps={charged_ps}");
        });
    }
}
