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
use frame_support::assert_ok;
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
