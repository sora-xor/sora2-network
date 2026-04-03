use ark_ff::PrimeField;
use ark_poly::GeneralEvaluationDomain;
use ark_serialize::CanonicalSerialize;
use fflonk::pcs::{PCS, PcsParams};

use crate::{ColumnsCommited, ColumnsEvaluated};

pub trait Transcript<F: PrimeField, CS: PCS<F>>: Clone {
    fn add_protocol_params(&mut self, domain: &GeneralEvaluationDomain<F>, pcs_raw_vk: &<CS::Params as PcsParams>::RVK) {
        self._add_serializable(b"domain", domain);
        self._add_serializable(b"pcs_raw_vk", pcs_raw_vk);
    }

    fn add_precommitted_cols(&mut self, precommitted_cols: &[CS::C; 2]) {
        self._add_serializable(b"precommitted_cols", precommitted_cols);
    }

    fn add_instance(&mut self, instance: &impl CanonicalSerialize) {
        self._add_serializable(b"instance", instance);
    }

    fn add_committed_cols(&mut self, committed_cols: &impl ColumnsCommited<F, CS::C>) {
        self._add_serializable(b"committed_cols", committed_cols);
    }

    // fn get_bitmask_aggregation_challenge(&mut self) -> Fr {
    //     self._get_128_bit_challenge(b"bitmask_aggregation")
    // }

    // fn append_2nd_round_register_commitments(&mut self, register_commitments: &impl RegisterCommitments) {
    //     self._append_serializable(b"2nd_round_register_commitments", register_commitments);
    // }

    fn get_constraints_aggregation_coeffs(&mut self, n: usize) -> Vec<F> {
        self._128_bit_coeffs(b"constraints_aggregation", n)
    }

    fn add_quotient_commitment(&mut self, point: &CS::C) {
        self._add_serializable(b"quotient", point);
    }

    fn get_evaluation_point(&mut self) -> F {
        self._128_bit_point(b"evaluation_point")
    }

    fn add_evaluations(&mut self, evals: &impl ColumnsEvaluated<F>, r_at_zeta_omega: &F) {
        self._add_serializable(b"register_evaluations", evals);
        self._add_serializable(b"shifted_linearization_evaluation", r_at_zeta_omega);
    }

    fn get_kzg_aggregation_challenges(&mut self, n: usize) -> Vec<F> {
        self._128_bit_coeffs(b"kzg_aggregation", n)
    }

    fn _128_bit_point(&mut self, label: &'static [u8]) -> F;

    fn _128_bit_coeffs(&mut self, label: &'static [u8], n: usize) -> Vec<F> {
        (0..n).map(|_| self._128_bit_point(label)).collect()
    }

    fn _add_serializable(&mut self, label: &'static [u8], message: &impl CanonicalSerialize);
}

impl<F: PrimeField, CS: PCS<F>> Transcript<F, CS> for merlin::Transcript {
    fn _128_bit_point(&mut self, label: &'static [u8]) -> F {
        let mut buf = [0u8; 16];
        self.challenge_bytes(label, &mut buf);
        F::from_random_bytes(&buf).unwrap()
    }

    fn _add_serializable(&mut self, label: &'static [u8], message: &impl CanonicalSerialize) {
        let mut buf = vec![0; message.uncompressed_size()];
        message.serialize_uncompressed(&mut buf).unwrap();
        self.append_message(label, &buf);
    }
}