use ark_bls12_381::Bls12_381;
use ark_ff::PrimeField;
use ark_poly::DenseUVPolynomial;
use ark_poly::EvaluationDomain;
use ark_poly::Radix2EvaluationDomain;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, Compress};
use ark_std::{end_timer, start_timer};
use ark_std::rand::Rng;
use ark_std::test_rng;

use fflonk::pcs::kzg::KZG;
use fflonk::pcs::PCS;
use fflonk::Poly;

use crate::batchy::PlonkBatchKzgTest;
use crate::fflonky::PlonkWithFflonkTest;

mod fflonky;
mod batchy;


struct VanillaPlonkAssignments<F: PrimeField> {
    degree: usize,
    max_degree: usize,

    // [Poly<F>; 8], max_deg = d
    preprocessed_polynomials: Vec<Poly<F>>,
    // [Poly<F>; 3], max_deg = d
    wire_polynomials: Vec<Poly<F>>,
    // max_deg = d
    permutation_polynomial: Poly<F>,
    // max_deg = 3 * d
    arithmetic_constraint: Poly<F>,
    // max_deg = 2 * d
    permutation_constraint_1: Poly<F>,
    // // max_deg = 4 * d
    permutation_constraint_2: Poly<F>,

    domain: Radix2EvaluationDomain<F>,
    omega: F,
}

fn random_polynomials<F: PrimeField, R: Rng>(k: usize, degree: usize, rng: &mut R) -> Vec<Poly<F>> {
    (0..k).map(|_| Poly::rand(degree, rng)).collect()
}

impl<F: PrimeField> VanillaPlonkAssignments<F> {
    fn new<R: Rng>(domain_size: usize, rng: &mut R) -> Self {
        let degree = domain_size - 1;
        let max_degree = 3 * degree; // permutation_constraint_2 / Z
        let domain = Radix2EvaluationDomain::<F>::new(domain_size).unwrap();
        let omega = domain.group_gen;
        Self {
            degree,
            max_degree,
            preprocessed_polynomials: random_polynomials(8, degree, rng),
            wire_polynomials: random_polynomials(3, degree, rng),
            permutation_polynomial: Poly::rand(degree, rng),
            arithmetic_constraint: Poly::rand(3 * degree, rng),
            permutation_constraint_1: Poly::rand(2 * degree, rng),
            permutation_constraint_2: Poly::rand(4 * degree, rng),
            domain,
            omega,
        }
    }

    fn quotient(&self, constraint: &Poly<F>) -> Poly<F> {
        constraint.divide_by_vanishing_poly(self.domain).unwrap().0
    }
}

trait DecoyPlonk<F: PrimeField, CS: PCS<F>> {
    type Proof: CanonicalSerialize + CanonicalDeserialize;

    fn new<R: Rng>(polys: VanillaPlonkAssignments<F>, rng: &mut R) -> Self;

    fn setup<R: Rng>(&mut self, rng: &mut R) -> (CS::CK, CS::VK);
    fn preprocess(&mut self, ck: &CS::CK) -> Vec<CS::C>;
    fn prove(&mut self, ck: &CS::CK) -> Self::Proof;
    fn verify(&self, vk: &CS::VK, preprocessed_commitments: Vec<CS::C>, proof: Self::Proof) -> bool;
}

fn _test_vanilla_plonk_opening<F: PrimeField, CS: PCS<F>, T: DecoyPlonk<F, CS>>(log_n: usize) {
    let rng = &mut test_rng();
    let n = 1 << log_n;
    let polys = VanillaPlonkAssignments::<F>::new(n, rng);

    let mut test = T::new(polys, rng);

    let t_test = start_timer!(|| format!("domain_size = {},  curve = {}", n, fflonk::utils::curve_name::<Bls12_381>()));

    let t_setup = start_timer!(|| "Setup");
    let (ck, vk) = test.setup(rng);
    end_timer!(t_setup);

    let t_preprocess = start_timer!(|| "Preprocessing");
    let commitments_to_preprocessed_polynomials = test.preprocess(&ck);
    end_timer!(t_preprocess);

    let preprocessed_size = commitments_to_preprocessed_polynomials.serialized_size(Compress::Yes);

    let t_prove = start_timer!(|| "Proving");
    let proof = test.prove(&ck);
    end_timer!(t_prove);

    let proof_size = proof.serialized_size(Compress::Yes);
    let mut serialized_proof = vec![0; proof_size];
    proof.serialize_compressed(&mut serialized_proof[..]).unwrap();
    let proof = T::Proof::deserialize_compressed(&serialized_proof[..]).unwrap();

    let t_verify = start_timer!(|| "Verifying");
    let valid = test.verify(&vk, commitments_to_preprocessed_polynomials, proof);
    end_timer!(t_verify);

    end_timer!(t_test);

    println!("proof size = {}, preprocessed data size = {}", proof_size, preprocessed_size);

    assert!(valid);
}

#[test]
#[ignore]
fn test_vanilla_plonk_batch_kzg_opening() {
    _test_vanilla_plonk_opening::<_, KZG<Bls12_381>, PlonkBatchKzgTest<_, _>>(16);
}

#[test]
#[ignore]
fn test_vanilla_plonk_with_fflonk_opening() {
    _test_vanilla_plonk_opening::<_, KZG<Bls12_381>, PlonkWithFflonkTest<_, _>>(16);
}
