use ark_ec::{AffineRepr, CurveGroup, Group};
use ark_ec::scalar_mul::fixed_base::FixedBase;
use ark_ff::{BigInteger, PrimeField, Zero};
use ark_std::vec::Vec;

pub fn naive_multiexp_affine<G: AffineRepr>(coeffs: &[G::ScalarField], bases: &[G]) -> G::Group {
    bases.iter().zip(coeffs.iter()).map(|(b, &c)| b.mul(c)).sum()
}

/// Performs a small multi-exponentiation operation.
/// Uses the double-and-add algorithm with doublings shared across points.
// adopted from https://github.com/zcash/halo2/pull/20
pub fn small_multiexp_affine<G: AffineRepr>(coeffs: &[G::ScalarField], bases: &[G]) -> G::Group {
    let bytes_in_repr = <G::ScalarField as PrimeField>::BigInt::NUM_LIMBS * 8;
    let coeffs: Vec<_> = coeffs.iter().map(|c| c.into_bigint().to_bytes_le()).collect();

    let mut acc = G::Group::zero();

    // for byte idx
    for byte_idx in (0..bytes_in_repr).rev() {
        // for bit idx
        for bit_idx in (0..8).rev() {
            acc.double_in_place();
            // for each coeff
            for coeff_idx in 0..coeffs.len() {
                let byte = coeffs[coeff_idx][byte_idx];
                if ((byte >> bit_idx) & 1) != 0 {
                    acc += &bases[coeff_idx];
                }
            }
        }
    }

    acc
}

pub fn small_multiexp_proj<G: CurveGroup>(coeffs: &[G::ScalarField], bases: &[G]) -> G {
    let bases = G::normalize_batch(bases);
    small_multiexp_affine(coeffs, &bases)
}

pub fn _small_multiexp_proj_2<G: CurveGroup>(coeffs: &[G::ScalarField], bases: &[G]) -> G {
    let bytes_in_repr = <G::ScalarField as PrimeField>::BigInt::NUM_LIMBS * 8;
    let coeffs: Vec<_> = coeffs.iter().map(|c| c.into_bigint().to_bytes_le()).collect();

    let mut acc = G::zero();

    // for byte idx
    for byte_idx in (0..bytes_in_repr).rev() {
        // for bit idx
        for bit_idx in (0..8).rev() {
            acc.double_in_place();
            // for each coeff
            for coeff_idx in 0..coeffs.len() {
                let byte = coeffs[coeff_idx][byte_idx];
                if ((byte >> bit_idx) & 1) != 0 {
                    acc += bases[coeff_idx];
                }
            }
        }
    }

    acc
}

// Multiply the same base by each scalar.
pub fn single_base_msm<G: CurveGroup>(scalars: &[G::ScalarField], g: G) -> Vec<G::Affine> {
    let num_scalars = scalars.len();
    let window_size = FixedBase::get_mul_window_size(num_scalars);
    let bits_in_scalar = G::ScalarField::MODULUS_BIT_SIZE.try_into().unwrap();
    let table = FixedBase::get_window_table(bits_in_scalar, window_size, g);
    let scalars_in_g = FixedBase::msm(bits_in_scalar, window_size, &table, scalars);
    assert_eq!(scalars_in_g.len(), num_scalars);

    G::normalize_batch(&scalars_in_g)
}