use ark_ff::PrimeField;
use ark_poly::Evaluations;
use ark_poly::univariate::DensePolynomial;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use fflonk::pcs::Commitment;

use crate::{ColumnsCommited, ColumnsEvaluated};
use crate::domain::{Domain, EvaluatedDomain};

pub trait ProverPiop<F: PrimeField, C: Commitment<F>> {
    type Commitments: ColumnsCommited<F, C>;
    type Evaluations: ColumnsEvaluated<F>;
    type Instance: CanonicalSerialize + CanonicalDeserialize;

    // Commitments to the column polynomials excluding the precommitted columns.
    fn committed_columns<Fun: Fn(&DensePolynomial<F>) -> C>(&self, commit: Fun) -> Self::Commitments;

    // All the column polynomials (including precommitted columns)
    fn columns(&self) -> Vec<DensePolynomial<F>>;

    // All the column polynomials (including precommitted columns) evaluated in a point
    // Self::Evaluations::to_vec should return evaluations in the order consistent to Self::columns
    fn columns_evaluated(&self, zeta: &F) -> Self::Evaluations;

    // Constraint polynomials in evaluation form.
    fn constraints(&self) -> Vec<Evaluations<F>>;

    // 'Linearized' parts of constraint polynomials.
    // For a constraint of the form C = C(c1(X),...,ck(X),c1(wX),...,ck(wX)), where ci's are of degree n,
    // and an evaluation point z, it is a degree n polynomial r = C(c1(z),...,ck(z),c1(X),...,ck(X)).
    fn constraints_lin(&self, zeta: &F) -> Vec<DensePolynomial<F>>;

    // Subgroup over which the columns are defined.
    fn domain(&self) -> &Domain<F>;

    // The result of the computation.
    fn result(&self) -> Self::Instance;
}

pub trait VerifierPiop<F: PrimeField, C: Commitment<F>> {
    const N_CONSTRAINTS: usize;
    const N_COLUMNS: usize;
    // Columns the commitments to which are publicly known. These commitments are omitted from the proof.
    fn precommitted_columns(&self) -> Vec<C>;

    fn evaluate_constraints_main(&self) -> Vec<F>;

    fn constraint_polynomials_linearized_commitments(&self) -> Vec<C>;

    fn domain_evaluated(&self) -> &EvaluatedDomain<F>;
}