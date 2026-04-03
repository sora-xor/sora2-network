use ark_ff::PrimeField;
use ark_poly::Evaluations;
use ark_serialize::*;
use ark_std::fmt::Debug;
use ark_std::iter::Sum;
use ark_std::ops::{Add, Sub};
use ark_std::rand::Rng;
use ark_std::vec::Vec;

pub use id::IdentityCommitment;

use crate::Poly;

pub mod kzg;
mod id;

pub trait Commitment<F: PrimeField>:
Eq
+ Sized
+ Clone
+ Debug
+ Add<Self, Output=Self>
+ Sub<Self, Output=Self>
+ Sum<Self>
+ CanonicalSerialize
+ CanonicalDeserialize
{
    fn mul(&self, by: F) -> Self;
    fn combine(coeffs: &[F], commitments: &[Self]) -> Self;
}


/// Can be used to commit and open commitments to DensePolynomial<F> of degree up to max_degree.
pub trait CommitterKey: Clone + Debug + CanonicalSerialize + CanonicalDeserialize {
    /// Maximal degree of a polynomial supported.
    fn max_degree(&self) -> usize;

    /// Maximal number of evaluations supported when committing in the Lagrangian base.
    fn max_evals(&self) -> usize {
        self.max_degree() + 1
    }
}


/// Can be used to verify openings to commitments.
pub trait VerifierKey: Clone + Debug {
    /// Maximal number of openings that can be verified.
    fn max_points(&self) -> usize {
        1
    }
}


/// Generates a `VerifierKey`, serializable
pub trait RawVerifierKey: Clone + Debug + CanonicalSerialize + CanonicalDeserialize {
    type VK: VerifierKey;

    fn prepare(&self) -> Self::VK;
}


pub trait PcsParams {
    type CK: CommitterKey;
    type VK: VerifierKey;
    type RVK: RawVerifierKey<VK=Self::VK>;

    fn ck(&self) -> Self::CK;
    fn vk(&self) -> Self::VK;
    fn raw_vk(&self) -> Self::RVK;

    fn ck_with_lagrangian(&self, _domain_size: usize) -> Self::CK {
        unimplemented!();
    }
}


/// Polynomial commitment scheme.
pub trait PCS<F: PrimeField> {
    type C: Commitment<F>;

    type Proof: Clone + CanonicalSerialize + CanonicalDeserialize;

    type CK: CommitterKey;

    // vk needs to be convertible to a ck that is only required to commit to the p=1 constant polynomial,
    // see https://eprint.iacr.org/archive/2020/1536/1629188090.pdf, section 4.2
    type VK: VerifierKey + Into<Self::CK>;
    type Params: PcsParams<CK=Self::CK, VK=Self::VK>;

    fn setup<R: Rng>(max_degree: usize, rng: &mut R) -> Self::Params;

    fn commit(ck: &Self::CK, p: &Poly<F>) -> Self::C;

    fn commit_evals(ck: &Self::CK, evals: &Evaluations<F>) -> Self::C {
        let poly = evals.interpolate_by_ref();
        Self::commit(ck, &poly)
    }

    fn open(ck: &Self::CK, p: &Poly<F>, x: F) -> Self::Proof; //TODO: eval?

    fn verify(vk: &Self::VK, c: Self::C, x: F, z: F, proof: Self::Proof) -> bool;

    fn batch_verify<R: Rng>(vk: &Self::VK, c: Vec<Self::C>, x: Vec<F>, y: Vec<F>, proof: Vec<Self::Proof>, _rng: &mut R) -> bool {
        assert_eq!(c.len(), x.len());
        assert_eq!(c.len(), y.len());
        c.into_iter().zip(x.into_iter()).zip(y.into_iter()).zip(proof.into_iter())
            .all(|(((c, x), y), proof)| Self::verify(vk, c, x, y, proof))
    }
}
