use std::marker::PhantomData;

use ark_ff::{PrimeField, UniformRand};
use ark_poly::{DenseUVPolynomial, Polynomial};
use ark_serialize::*;
use ark_std::{end_timer, start_timer, test_rng};
use ark_std::rand::Rng;

use fflonk::pcs::{Commitment, PCS, PcsParams};
use fflonk::Poly;
use fflonk::utils::poly;

use crate::{DecoyPlonk, VanillaPlonkAssignments};

impl<F: PrimeField> VanillaPlonkAssignments<F> {
    fn constraints(&self) -> Vec<Poly<F>> {
        vec![
            self.arithmetic_constraint.clone(),
            self.permutation_constraint_1.clone(),
            self.permutation_constraint_2.clone(),
        ]
    }

    fn polys_to_commit_1(&self) -> Vec<Poly<F>> {
        self.wire_polynomials.clone()
    }

    fn poly_to_commit_2(&self, _beta_gamma: (F, F)) -> Poly<F> {
        self.permutation_polynomial.clone()
    }

    fn poly_to_commit_3(&self, alpha: F) -> Poly<F> {
        let aggregate_constraint = poly::sum_with_powers(alpha, &self.constraints());
        self.quotient(&aggregate_constraint)
    }

    fn polys_to_evaluate_at_zeta_4(&self) -> Vec<Poly<F>> {
        [&self.wire_polynomials, &self.preprocessed_polynomials[5..7]].concat() // a, b, c, S_{sigma_1}, S_{sigma_2}
    }

    fn poly_to_evaluate_at_zeta_omega_4(&self) -> Poly<F> {
        self.permutation_polynomial.clone()
    }

    fn polys_to_open_at_zeta_5(&self) -> Vec<Poly<F>> {
        self.polys_to_evaluate_at_zeta_4()
    }

    fn poly_to_open_at_zeta_omega_5(&self) -> Poly<F> {
        self.poly_to_evaluate_at_zeta_omega_4()
    }
}

struct Challenges<F: PrimeField> {
    // verifier challenges in order:
    // permutation argument challenges (aka "permutation challenges")
    beta_gamma: (F, F),
    // constraint aggregation challenge (aka "quotient challenge")
    alpha: F,
    // evaluation challenge
    zeta: F,
    // polynomial aggregation challenge (aka "opening challenge")
    nus: Vec<F>,
}

impl<F: PrimeField> Challenges<F> {
    fn new<R: Rng>(rng: &mut R) -> Self {
        let beta_gamma: (F, F) = (Self::get_128_bit_challenge(rng), Self::get_128_bit_challenge(rng));
        let alpha: F = Self::get_128_bit_challenge(rng);
        let zeta: F = Self::get_128_bit_challenge(rng);
        let one = std::iter::once(F::one());
        let nus = one.chain((1..6).map(|_| Self::get_128_bit_challenge(rng))).collect();
        Self { beta_gamma, alpha, zeta, nus }
    }

    fn get_128_bit_challenge<R: Rng>(rng: &mut R) -> F {
        u128::rand(rng).into()
    }
}

pub struct PlonkBatchKzgTest<F: PrimeField, CS: PCS<F>> {
    polys: VanillaPlonkAssignments<F>,
    linearization_polynomial: Poly<F>,
    challenges: Challenges<F>,
    cs: PhantomData<CS>,
}

impl<F: PrimeField, CS: PCS<F>> PlonkBatchKzgTest<F, CS> {
    fn commit_polynomial(&self, ck: &CS::CK, poly: &Poly<F>) -> CS::C {
        let t_commitment = start_timer!(|| format!("Committing to degree {} polynomials", poly.degree()));
        let commitment = CS::commit(ck, poly);
        end_timer!(t_commitment);
        commitment
    }

    fn commit_polynomials(&self, ck: &CS::CK, polys: &[Poly<F>]) -> Vec<CS::C> {
        let t_commitment = start_timer!(|| format!("Committing to batch of {} polynomials", polys.len()));
        let commitments = polys.iter().map(|p| self.commit_polynomial(ck, p)).collect();
        end_timer!(t_commitment);

        commitments
    }
}

#[derive(CanonicalSerialize, CanonicalDeserialize)]
pub struct BatchyPlonkProof<F: PrimeField, CS: PCS<F>> {
    wire_polynomials_c: Vec<CS::C>,
    permutation_polynomial_c: CS::C,
    quotient_polynomial_c: CS::C,
    evals_at_zeta: Vec<F>,
    evals_at_zeta_omega: F,
    // [W_{\zeta}]_1
    proof_at_zeta: CS::Proof,
    // [W_{\zeta\omega}]_1
    proof_at_zeta_omega: CS::Proof,
    extra: (CS::C, F), // commitment and evaluation of the linearization poly //TODO: remove
}


impl<F: PrimeField, CS: PCS<F>> DecoyPlonk<F, CS> for PlonkBatchKzgTest<F, CS> {
    type Proof = BatchyPlonkProof<F, CS>;

    fn new<R: Rng>(polys: VanillaPlonkAssignments<F>, rng: &mut R) -> Self {
        let linearization_polynomial = Poly::rand(polys.degree, rng); // TODO: compute from known commitments
        let challenges = Challenges::new(rng);
        Self { polys, linearization_polynomial, challenges, cs: PhantomData }
    }

    fn setup<R: Rng>(&mut self, rng: &mut R) -> (CS::CK, CS::VK) {
        let params = CS::setup(self.polys.max_degree, rng);
        (params.ck(), params.vk())
    }

    fn preprocess(&mut self, ck: &CS::CK) -> Vec<CS::C> {
        self.commit_polynomials(ck, &self.polys.preprocessed_polynomials)
    }

    fn prove(&mut self, ck: &CS::CK) -> BatchyPlonkProof<F, CS> {
        let wire_polynomials_c = self.commit_polynomials(ck, &self.polys.polys_to_commit_1());
        let permutation_polynomial_c = self.commit_polynomial(ck, &self.polys.poly_to_commit_2(self.challenges.beta_gamma));
        let quotient_polynomial_c = self.commit_polynomial(ck, &self.polys.poly_to_commit_3(self.challenges.alpha));

        let zeta = self.challenges.zeta;
        let evals_at_zeta = self.polys.polys_to_evaluate_at_zeta_4().iter().map(|p| p.evaluate(&zeta)).collect();
        let zeta_omega = zeta * self.polys.omega;
        let evals_at_zeta_omega = self.polys.poly_to_evaluate_at_zeta_omega_4().evaluate(&zeta_omega);

        // TODO: should be computed by verifier from other commitments
        let linearization_polynomial = self.linearization_polynomial.clone();

        let mut polys_to_open_at_zeta = vec![linearization_polynomial.clone()];
        polys_to_open_at_zeta.extend_from_slice(&self.polys.polys_to_open_at_zeta_5());
        let agg_poly_at_zeta = poly::sum_with_coeffs(self.challenges.nus.clone(), &polys_to_open_at_zeta);

        let proof_at_zeta = CS::open(ck, &agg_poly_at_zeta, zeta);
        let proof_at_zeta_omega = CS::open(ck, &self.polys.poly_to_open_at_zeta_omega_5(), zeta_omega);

        // TODO: compute
        let t_extra = start_timer!(|| "Extra: commiting to the linearization polynomial");
        let extra_comm = self.commit_polynomial(ck, &linearization_polynomial);
        let extra_eval = linearization_polynomial.evaluate(&zeta);
        end_timer!(t_extra);

        BatchyPlonkProof {
            wire_polynomials_c,
            permutation_polynomial_c: permutation_polynomial_c,
            quotient_polynomial_c,
            evals_at_zeta,
            evals_at_zeta_omega,
            proof_at_zeta,
            proof_at_zeta_omega,
            extra: (extra_comm, extra_eval),
        }
    }

    fn verify(&self, vk: &CS::VK, preprocessed_commitments: Vec<CS::C>, proof: BatchyPlonkProof<F, CS>) -> bool {
        // TODO:
        let t_reconstruct = start_timer!(|| "Reconstructing the commitment to the linearization polynomial: 7-multiexp");
        let bases = [&preprocessed_commitments[0..4], &vec![proof.permutation_polynomial_c.clone(), preprocessed_commitments[7].clone(), proof.quotient_polynomial_c]].concat();
        assert_eq!(bases.len(), 7); // [q_C]_1 has exp = 1
        let coeffs = (0..7).map(|_| F::rand(&mut test_rng())).collect::<Vec<_>>();
        let _comm = CS::C::combine(&coeffs, &bases);
        end_timer!(t_reconstruct);

        let t_kzg = start_timer!(|| "KZG batch verification");
        let (agg_comm, agg_eval) = {
            let t_aggregate_claims = start_timer!(|| "aggregate evaluation claims at zeta");

            let nus = self.challenges.nus.clone();

            let mut comms = vec![proof.extra.0];
            comms.extend_from_slice(&(proof.wire_polynomials_c));
            comms.extend_from_slice(&preprocessed_commitments[5..7]);
            assert_eq!(comms.len(), nus.len());
            let agg_comms = CS::C::combine(&nus, &comms);

            let mut evals = vec![proof.extra.1];
            evals.extend_from_slice(&proof.evals_at_zeta);
            assert_eq!(evals.len(), nus.len());
            let agg_evals = evals.into_iter().zip(nus.iter()).map(|(y, r)| y * r).sum();

            end_timer!(t_aggregate_claims);
            (agg_comms, agg_evals)
        };

        let t_kzg_batch_opening = start_timer!(|| "batched KZG openning");
        let zeta = self.challenges.zeta;
        let zeta_omega = zeta * self.polys.omega;
        let valid = CS::batch_verify(vk,
                                     vec![agg_comm, proof.permutation_polynomial_c],
                                     vec![zeta, zeta_omega],
                                     vec![agg_eval, proof.evals_at_zeta_omega],
                                     vec![proof.proof_at_zeta, proof.proof_at_zeta_omega],
                                     &mut test_rng());
        end_timer!(t_kzg_batch_opening);
        end_timer!(t_kzg);
        valid
    }
}
