use crate::{beefy_types::ValidatorSet, bitfield::BitField, simplified_proof::*};
use bridge_types::H256;

#[test]
fn bitfields_preserve_bit_order_and_support_mutation_and_encoding() {
    assert_eq!(crate::concat_u8(&[&[1, 2], &[], &[3]]), vec![1, 2, 3]);
    assert!(crate::concat_u8(&[]).is_empty());
    let mut field = BitField::with_capacity(16);
    assert!(field.is_empty());
    field.resize(8, false);
    field.set(0);
    field.set(7);
    assert_eq!(field.count_set_bits(), 2);
    assert_eq!(field.clone().to_bits(), vec![0b1000_0001]);
    assert_eq!(BitField::try_from_slice(&[0b1000_0001]).unwrap(), field);
    field.clear(0);
    assert!(!field.is_set(0));
    assert!(field.is_set(7));
    assert_eq!(field.count_set_bits(), 1);
    assert!(BitField::try_from_slice(&[]).unwrap().is_empty());
}

#[test]
fn random_samples_are_deterministic_unique_and_respect_the_prior() {
    let prior = BitField::create_bitfield(&[1, 4, 6, 10, 12], 16);
    for seed in [0, 1, 2, 1000] {
        let sample = BitField::create_random_bitfield(&prior, 3, 13, seed);
        assert_eq!(sample.count_set_bits(), 3);
        assert_eq!(
            sample,
            BitField::create_random_bitfield(&prior, 3, 13, seed)
        );
        for i in 0..sample.len() {
            assert!(!sample.is_set(i) || prior.is_set(i));
        }
        assert_eq!(BitField::create_random_bitfield(&prior, 5, 13, seed), prior);
    }
    assert_eq!(
        BitField::create_random_bitfield(&prior, 0, 13, 0),
        BitField::with_zeroes(16)
    );
}

#[test]
fn bounded_sampling_matches_legacy_and_reports_exhausted_or_invalid_requests() {
    let prior = BitField::create_bitfield(&[0, 3, 7], 8);
    assert_eq!(
        BitField::create_random_bitfield_bounded(&prior, 2, 8, 42, 100),
        Some(BitField::create_random_bitfield(&prior, 2, 8, 42))
    );
    assert_eq!(
        BitField::create_random_bitfield_bounded(&prior, 2, 8, 42, 0),
        None
    );
    assert_eq!(
        BitField::create_random_bitfield_bounded(&prior, 0, 8, 42, 0),
        Some(BitField::with_zeroes(8))
    );
    assert_eq!(
        BitField::create_random_bitfield_bounded(&prior, 4, 8, 42, 100),
        None
    );
    for (count, length) in [(1, 0), (1, 9), (9, 8)] {
        assert_eq!(
            BitField::create_random_bitfield_bounded(&prior, count, length, 42, 100),
            None
        );
    }
    assert_eq!(
        BitField::create_random_bitfield_bounded(&prior, 3, 8, u128::MAX, 100),
        Some(prior)
    );
}

#[test]
fn simplified_proof_respects_sibling_order_and_rejects_wrong_roots_and_oversized_proofs() {
    let leaf = H256::repeat_byte(1);
    let left = H256::repeat_byte(2);
    let right = H256::repeat_byte(3);
    let proof = Proof {
        order: 1,
        items: vec![left, right],
    };
    let root = hasher(hasher(left, leaf), right);
    assert_eq!(proof.root(hasher, leaf), root);
    assert!(verify_inclusion_proof(root, leaf, &proof));
    assert!(!verify_inclusion_proof(H256::zero(), leaf, &proof));
    assert!(verify_inclusion_proof(
        leaf,
        leaf,
        &Proof {
            order: 0,
            items: vec![]
        }
    ));
    for count in [64, 65] {
        assert!(!verify_inclusion_proof(
            root,
            leaf,
            &Proof {
                order: 0,
                items: vec![left; count]
            }
        ));
    }
    assert!(bit(0b1010, 1));
    assert!(!bit(0b1010, 0));
    assert!(bit(1 << 63, 63));
}

#[test]
fn mmr_positions_and_peaks_match_small_tree_layouts() {
    assert_eq!(
        (0..8).map(leaf_index_to_pos).collect::<Vec<_>>(),
        vec![0, 1, 3, 4, 7, 8, 10, 11]
    );
    assert_eq!(
        (1..=8).map(leaf_count_to_mmr_size).collect::<Vec<_>>(),
        vec![1, 3, 4, 7, 8, 10, 11, 15]
    );
    assert_eq!(
        (0..7).map(pos_height_in_tree).collect::<Vec<_>>(),
        vec![0, 0, 1, 0, 0, 1, 2]
    );
    assert_eq!(get_peaks(1), vec![0]);
    assert_eq!(get_peaks(7), vec![6]);
    assert_eq!(get_peaks(11), vec![6, 9, 10]);
    assert_eq!(parent_offset(2), 8);
    assert_eq!(sibling_offset(2), 7);
}

#[test]
fn validator_set_conversion_preserves_identity_size_and_commitment() {
    let set = ValidatorSet {
        id: 42,
        len: 100,
        keyset_commitment: H256::repeat_byte(9),
    };
    let authority: sp_consensus_beefy::mmr::BeefyAuthoritySet<H256> = set.clone().into();
    assert_eq!(authority.id, 42);
    assert_eq!(authority.len, 100);
    assert_eq!(authority.keyset_commitment, set.keyset_commitment);
    assert_eq!(ValidatorSet::from(authority), set);
}
