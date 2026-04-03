use ark_ff::{FftField, Field, Zero};
use ark_poly::Evaluations;
use ark_poly::univariate::DensePolynomial;

use crate::{Column, const_evals, FieldColumn};
use crate::domain::Domain;
use crate::gadgets::VerifierGadget;

pub struct FixedCells<F: FftField> {
    col: FieldColumn<F>,
    col_first: F,
    col_last: F,
    l_first: FieldColumn<F>,
    l_last: FieldColumn<F>,
}


pub struct FixedCellsValues<F: Field> {
    pub col: F,
    pub col_first: F,
    pub col_last: F,
    pub l_first: F,
    pub l_last: F,
}


impl<F: FftField> FixedCells<F> {
    pub fn init(col: FieldColumn<F>, domain: &Domain<F>) -> Self {
        assert_eq!(col.len, domain.capacity);
        let col_first = col.evals.evals[0];
        let col_last = col.evals.evals[domain.capacity - 1];
        let l_first = domain.l_first.clone();
        let l_last = domain.l_last.clone();
        Self { col, col_first, col_last, l_first, l_last }
    }

    pub fn constraints(&self) -> Vec<Evaluations<F>> {
        let col = &self.col;
        let domain = col.domain_4x();
        let first = &const_evals(self.col_first, domain);
        let last = &const_evals(self.col_last, domain);
        let col = &self.col.evals_4x;
        let l_first = &self.l_first.evals_4x;
        let l_last = &self.l_last.evals_4x;
        let c = &(l_first * &(col - first)) + &(l_last * &(col - last));
        vec![c]
    }

    pub fn constraints_linearized(&self, _z: &F) -> Vec<DensePolynomial<F>> {
        vec![DensePolynomial::zero()]
    }
}


impl<F: Field> VerifierGadget<F> for FixedCellsValues<F> {
    fn evaluate_constraints_main(&self) -> Vec<F> {
        let c = (self.col - self.col_first) * self.l_first + (self.col - self.col_last) * self.l_last;
        vec![c]
    }
}