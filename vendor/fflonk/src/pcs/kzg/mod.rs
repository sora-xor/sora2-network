use ark_ec::CurveGroup;
use ark_ec::pairing::Pairing;
use ark_ec::VariableBaseMSM;
use ark_ff::{One, UniformRand, Zero};
use ark_poly::{DenseUVPolynomial, Evaluations, Polynomial};
use ark_std::marker::PhantomData;
use ark_std::ops::Mul;
use ark_std::rand::Rng;
use ark_std::vec::Vec;

use crate::pcs::{CommitterKey, PCS};
use crate::pcs::kzg::commitment::KzgCommitment;
use crate::pcs::kzg::params::{KzgCommitterKey, KzgVerifierKey};
use crate::pcs::kzg::urs::URS;
use crate::Poly;
use crate::utils::ec::{small_multiexp_affine, small_multiexp_proj};

pub mod urs;
pub mod params;
mod commitment;
mod lagrange;

#[derive(Clone)]
pub struct KZG<E: Pairing> {
    _engine: PhantomData<E>,
}

/// e(acc, g2) = e(proof, tau.g2)
#[derive(Clone, Debug)]
pub struct AccumulatedOpening<E: Pairing> {
    pub acc: E::G1Affine,
    pub proof: E::G1Affine,
}

#[derive(Clone, Debug)]
pub struct KzgOpening<E: Pairing> {
    pub c: E::G1Affine,
    pub x: E::ScalarField,
    pub y: E::ScalarField,
    pub proof: E::G1Affine,
}

impl<E: Pairing> KZG<E> {
    fn z(x: E::ScalarField) -> Poly<E::ScalarField> {
        Poly::from_coefficients_slice(&[-x, E::ScalarField::one()])
    }

    fn q(p: &Poly<E::ScalarField>, d: &Poly<E::ScalarField>) -> Poly<E::ScalarField> {
        p / d
    }

    fn compute_quotient(p: &Poly<E::ScalarField>, x: E::ScalarField) -> Poly<E::ScalarField> {
        Self::q(p, &Self::z(x))
    }

    fn parse(openings: Vec<KzgOpening<E>>) -> Vec<((E::G1, E::G1Affine), E::ScalarField)> {
        openings.into_iter().map(|KzgOpening { c, x, y, proof }|
            ((proof.mul(x) + &c, proof), y)
        ).collect()
    }

    pub fn accumulate(openings: Vec<KzgOpening<E>>, rs: &[E::ScalarField], vk: &KzgVerifierKey<E>) -> AccumulatedOpening<E> {
        assert_eq!(openings.len(), rs.len());
        let openings = Self::parse(openings);
        let ((accs, proofs), ys): ((Vec<E::G1>, Vec<E::G1Affine>), Vec<E::ScalarField>) = openings.into_iter().unzip();
        let sum_ry = rs.iter().zip(ys.into_iter()).map(|(r, y)| y * r).sum::<E::ScalarField>();
        let acc = vk.g1.mul(sum_ry) - small_multiexp_proj(rs, &accs);
        let proof = small_multiexp_affine(rs, &proofs);
        let points = E::G1::normalize_batch(&[acc, proof]);
        let acc = points[0];
        let proof = points[1];
        AccumulatedOpening { acc, proof }
    }

    fn accumulate_single(opening: KzgOpening<E>, g1: &E::G1Affine) -> AccumulatedOpening<E> {
        let KzgOpening { c, x, y, proof } = opening;
        let acc = (g1.mul(y) - (proof.mul(x) + &c)).into_affine();
        AccumulatedOpening { acc, proof }
    }

    pub fn verify_accumulated(opening: AccumulatedOpening<E>, vk: &KzgVerifierKey<E>) -> bool {
        E::multi_pairing(&[opening.acc, opening.proof],
                         [vk.g2.clone(), vk.tau_in_g2.clone()],
        ).is_zero()
    }

    pub fn verify_single(opening: KzgOpening<E>, vk: &KzgVerifierKey<E>) -> bool {
        let acc_opening = Self::accumulate_single(opening, &vk.g1);
        Self::verify_accumulated(acc_opening, vk)
    }

    pub fn verify_batch<R: Rng>(openings: Vec<KzgOpening<E>>, vk: &KzgVerifierKey<E>, rng: &mut R) -> bool {
        let one = ark_std::iter::once(E::ScalarField::one());
        let coeffs: Vec<E::ScalarField> = one.chain((1..openings.len()).map(|_| u128::rand(rng).into())).collect();
        let acc_opening = Self::accumulate(openings, &coeffs, vk);
        Self::verify_accumulated(acc_opening, vk)
    }

    fn _commit(coeffs: &[E::ScalarField], bases: &[E::G1Affine]) -> KzgCommitment<E> {
        let proj: E::G1 = VariableBaseMSM::msm_unchecked(bases, coeffs);
        KzgCommitment(proj.into_affine())
    }
}

impl<E: Pairing> PCS<E::ScalarField> for KZG<E> {
    type C = KzgCommitment<E>;
    type Proof = E::G1Affine;

    type CK = KzgCommitterKey<E::G1Affine>;
    type VK = KzgVerifierKey<E>;
    type Params = URS<E>;

    fn setup<R: Rng>(max_degree: usize, rng: &mut R) -> Self::Params {
        URS::<E>::generate(max_degree + 1, 2, rng)
    }

    fn commit(ck: &Self::CK, p: &Poly<E::ScalarField>) -> Self::C {
        let ck = &ck.monomial;
        assert!(p.degree() <= ck.max_degree(), "Can't commit to degree {} polynomial using {} bases", p.degree(), ck.max_evals());
        Self::_commit(&p.coeffs, &ck.powers_in_g1)
    }

    fn commit_evals(ck: &Self::CK, evals: &Evaluations<E::ScalarField>) -> Self::C {
        let ck = ck.lagrangian.as_ref().expect("lagrangian key hadn't been generated");
        assert_eq!(evals.domain(), ck.domain);
        assert!(evals.evals.len() <= ck.max_evals(), "Can't commit to {} values using {} bases", evals.evals.len(), ck.max_evals());
        Self::_commit(&evals.evals, &ck.lis_in_g)
    }

    fn open(ck: &Self::CK, p: &Poly<E::ScalarField>, x: E::ScalarField) -> Self::Proof {
        let q = Self::compute_quotient(p, x);
        Self::commit(ck, &q).0
    }

    fn verify(vk: &KzgVerifierKey<E>, c: Self::C, x: E::ScalarField, y: E::ScalarField, proof: Self::Proof) -> bool {
        let opening = KzgOpening { c: c.0, x, y, proof };
        Self::verify_single(opening, vk)
    }

    fn batch_verify<R: Rng>(vk: &KzgVerifierKey<E>, c: Vec<Self::C>, x: Vec<E::ScalarField>, y: Vec<E::ScalarField>, proof: Vec<Self::Proof>, rng: &mut R) -> bool {
        assert_eq!(c.len(), x.len());
        assert_eq!(c.len(), y.len());
        let openings = c.into_iter().zip(x.into_iter()).zip(y.into_iter()).zip(proof.into_iter())
            .map(|(((c, x), y), proof)| KzgOpening { c: c.0, x, y, proof })
            .collect();
        Self::verify_batch(openings, vk, rng)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "print-trace")]
    use ark_ff::PrimeField;
    use ark_ff::UniformRand;
    use ark_poly::{DenseUVPolynomial, EvaluationDomain, GeneralEvaluationDomain};
    use ark_std::{end_timer, start_timer};
    use ark_std::test_rng;
    use ark_std::vec;

    use crate::pcs::PcsParams;
    use crate::tests::{BenchCurve, TestCurve, TestField};

    use super::*;

    fn _test_minimal_kzg<E: Pairing>(log_n: usize) {
        let rng = &mut test_rng();

        let max_degree = (1 << log_n) - 1;

        let t_setup = start_timer!(|| format!("KZG setup of size 2^{} on {}", log_n, crate::utils::curve_name::<E>()));
        let urs = KZG::<E>::setup(max_degree, rng);
        end_timer!(t_setup);

        let ck = urs.ck();
        let vk = urs.vk();

        let p = Poly::<E::ScalarField>::rand(ck.max_degree(), rng);
        let x = E::ScalarField::rand(rng);
        let z = p.evaluate(&x);

        let t_commit = start_timer!(|| format!("Committing to a dense degree-{} polynomial", ck.max_degree()));
        let c = KZG::<E>::commit(&ck, &p);
        end_timer!(t_commit);

        let t_prove = start_timer!(|| "Generating an opening proof for a single point");
        let proof = KZG::<E>::open(&ck, &p, x);
        end_timer!(t_prove);

        let t_verify = start_timer!(|| "Verification of a single-point opening");
        assert!(KZG::<E>::verify(&vk, c, x, z, proof));
        end_timer!(t_verify);
    }

    fn random_openings<R: Rng, E: Pairing>(
        k: usize,
        ck: &KzgCommitterKey<E::G1Affine>,
        xs: Vec<E::ScalarField>,
        rng: &mut R,
    ) -> Vec<KzgOpening<E>>
    {
        assert_eq!(xs.len(), k);
        let d = ck.max_degree();

        (0..k).map(|i| {
            let f = Poly::<E::ScalarField>::rand(d, rng);
            let x = xs[i];
            let y = f.evaluate(&x);
            let c = KZG::<E>::commit(ck, &f).0;
            let proof = KZG::<E>::open(ck, &f, x);
            KzgOpening { c, x, y, proof }
        }).collect()
    }

    fn _test_batch_verification<E: Pairing>(log_n: usize, k: usize) {
        let rng = &mut test_rng();

        let max_degree = (1 << log_n) - 1;

        let urs = KZG::<E>::setup(max_degree, rng);
        let (ck, vk) = (urs.ck(), urs.vk());

        let xs = (0..k).map(|_| E::ScalarField::rand(rng)).collect();
        let openings = random_openings(k, &ck, xs, rng);
        let t_verify_batch = start_timer!(|| format!("Batch verification of {} openings of degree ~2^{} on {} with {}-bit xs", k, log_n, crate::utils::curve_name::<E>(), E::ScalarField::MODULUS_BIT_SIZE));
        assert!(KZG::<E>::verify_batch(openings, &vk, rng));
        end_timer!(t_verify_batch);

        let xs = (0..k).map(|_| E::ScalarField::from(u128::rand(rng))).collect();
        let openings = random_openings(k, &ck, xs, rng);
        let t_verify_batch = start_timer!(|| format!("Batch verification of {} openings of degree ~2^{} on {} with {}-bit xs", k, log_n, crate::utils::curve_name::<E>(), 128));
        assert!(KZG::<E>::verify_batch(openings, &vk, rng));
        end_timer!(t_verify_batch);
    }

    #[test]
    fn test_minimal_kzg() {
        _test_minimal_kzg::<TestCurve>(8);
    }

    #[test]
    #[ignore]
    fn bench_minimal_kzg() {
        _test_minimal_kzg::<BenchCurve>(16);
    }

    #[test]
    fn test_batch_verification() {
        _test_batch_verification::<TestCurve>(8, 4);
    }

    #[test]
    #[ignore]
    fn bench_batch_verification() {
        _test_batch_verification::<BenchCurve>(12, 5);
    }

    #[test]
    fn test_commitments_match() {
        let rng = &mut test_rng();
        let domain_size = 16;
        let domain = GeneralEvaluationDomain::new(domain_size).unwrap();

        let urs = KZG::<TestCurve>::setup(domain_size - 1, rng);
        let ck = urs.ck_with_lagrangian(domain_size);

        let mut evals = vec![TestField::zero(); domain_size];
        evals[0] = TestField::one();
        let evals = Evaluations::from_vec_and_domain(evals, domain);
        let t_commit = start_timer!(|| format!("Committing to a sparse vec using lagrangian SRS"));
        let c_evals = KZG::<TestCurve>::commit_evals(&ck, &evals);
        end_timer!(t_commit);

        let poly = evals.interpolate();
        let t_commit = start_timer!(|| format!("Committing to a sparse vec using monomial SRS"));
        let c_poly = KZG::<TestCurve>::commit(&ck, &poly);
        end_timer!(t_commit);

        assert_eq!(c_evals, c_poly);
    }
}