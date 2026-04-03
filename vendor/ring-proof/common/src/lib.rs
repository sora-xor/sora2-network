use ark_ff::{FftField, PrimeField};
use ark_poly::{EvaluationDomain, Evaluations, GeneralEvaluationDomain, Polynomial};
use ark_poly::univariate::DensePolynomial;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use fflonk::pcs::{Commitment, PCS};

pub mod gadgets;
pub mod test_helpers;
pub mod piop;
pub mod prover;
pub mod verifier;
pub mod transcript;
pub mod domain;

pub trait Column<F: FftField> {
    fn domain(&self) -> GeneralEvaluationDomain<F>;
    fn domain_4x(&self) -> GeneralEvaluationDomain<F>;
    fn as_poly(&self) -> &DensePolynomial<F>;
    fn size(&self) -> usize {
        self.domain().size()
    }
    fn evaluate(&self, z: &F) -> F {
        self.as_poly().evaluate(z)
    }
}

#[derive(Clone)]
pub struct FieldColumn<F: FftField> {
    // actual (constrained) len of the input in evaluation form
    len: usize,
    poly: DensePolynomial<F>,
    evals: Evaluations<F>,
    evals_4x: Evaluations<F>,
}

impl<F: FftField> FieldColumn<F> {
    pub fn shifted_4x(&self) -> Evaluations<F> {
        let mut evals_4x = self.evals_4x.evals.clone();
        evals_4x.rotate_left(4);
        Evaluations::from_vec_and_domain(evals_4x, self.domain_4x())
    }

    pub fn vals(&self) -> &[F] {
        &self.evals.evals[..self.len]
    }
}

impl<F: FftField> Column<F> for FieldColumn<F> {
    fn domain(&self) -> GeneralEvaluationDomain<F> {
        self.evals.domain()
    }

    fn domain_4x(&self) -> GeneralEvaluationDomain<F> {
        self.evals_4x.domain()
    }

    fn as_poly(&self) -> &DensePolynomial<F> {
        &self.poly
    }
}

pub fn const_evals<F: FftField>(c: F, domain: GeneralEvaluationDomain<F>) -> Evaluations<F> {
    Evaluations::from_vec_and_domain(vec![c; domain.size()], domain)
}


pub trait ColumnsEvaluated<F: PrimeField>: CanonicalSerialize + CanonicalDeserialize {
    fn to_vec(self) -> Vec<F>;
}

pub trait ColumnsCommited<F: PrimeField, C: Commitment<F>>: CanonicalSerialize + CanonicalDeserialize {
    fn to_vec(self) -> Vec<C>;
}

#[derive(CanonicalSerialize, CanonicalDeserialize)]
pub struct Proof<F, CS, Commitments, Evaluations>
    where
        F: PrimeField,
        CS: PCS<F>,
        Commitments: ColumnsCommited<F, CS::C>,
        Evaluations: ColumnsEvaluated<F>, {
    pub column_commitments: Commitments,
    pub columns_at_zeta: Evaluations,
    pub quotient_commitment: CS::C,
    pub lin_at_zeta_omega: F,
    pub agg_at_zeta_proof: CS::Proof,
    pub lin_at_zeta_omega_proof: CS::Proof,
}
