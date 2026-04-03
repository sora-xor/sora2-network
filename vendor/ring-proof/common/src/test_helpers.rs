use ark_ec::{AffineRepr, CurveGroup};
use ark_std::rand::Rng;
use ark_std::UniformRand;

pub fn random_bitvec<R: Rng>(n: usize, density: f64, rng: &mut R) -> Vec<bool> {
    (0..n)
        .map(|_| rng.gen_bool(density))
        .collect()
}

pub fn random_vec<X: UniformRand, R: Rng>(n: usize, rng: &mut R) -> Vec<X> {
    (0..n)
        .map(|_| X::rand(rng))
        .collect()
}

pub fn cond_sum<P>(bitmask: &[bool], points: &[P]) -> P where P: AffineRepr {
    assert_eq!(bitmask.len(), points.len());
    bitmask.iter().zip(points.iter())
        .map(|(&b, &p)| if b { p } else { P::zero() })
        .sum::<P::Group>()
        .into_affine()
}
