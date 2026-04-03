use ark_ff::PrimeField;
use ark_poly::{EvaluationDomain, GeneralEvaluationDomain};
use ark_std::rand::Rng;
use fflonk::pcs::{PCS, PcsParams};

use crate::{Column, FieldColumn};

// Contains the polynomial commitment setup (the URS), and the selected subgroup (domain)
pub struct Setup<F: PrimeField, CS: PCS<F>, D: EvaluationDomain<F> = GeneralEvaluationDomain<F>> {
    pub domain: D,
    pub pcs_params: CS::Params,
}

impl<F: PrimeField, CS: PCS<F>, D: EvaluationDomain<F>> Setup<F, CS, D> {
    pub fn generate<R: Rng>(domain_size: usize, rng: &mut R) -> Self {
        let domain = D::new(domain_size).unwrap();
        let domain_size = domain.size();
        let setup_degree = 3 * domain_size - 3;
        let pcs_params = CS::setup(setup_degree, rng);
        Self {
            domain,
            pcs_params,
        }
    }

    pub fn commit_to_column(&self, col: &FieldColumn<F>) -> CS::C {
        CS::commit(&self.pcs_params.ck(), col.as_poly())
    }
}