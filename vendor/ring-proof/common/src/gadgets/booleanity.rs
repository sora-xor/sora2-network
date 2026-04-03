use ark_ff::{FftField, Field, Zero};
use ark_poly::{Evaluations, GeneralEvaluationDomain};
use ark_poly::univariate::DensePolynomial;

use crate::{Column, const_evals, FieldColumn};
use crate::domain::Domain;
use crate::gadgets::VerifierGadget;

#[derive(Clone)]
pub struct BitColumn<F: FftField> {
    pub bits: Vec<bool>,
    pub col: FieldColumn<F>,
}


impl<F: FftField> BitColumn<F> {
    pub fn init(bits: Vec<bool>, domain: &Domain<F>) -> Self {
        let bits_as_field_elements = bits.iter()
            .map(|&b| if b { F::one() } else { F::zero() })
            .collect();
        let col = domain.column(bits_as_field_elements);
        Self { bits, col }
    }
}


impl<F: FftField> Column<F> for BitColumn<F> {
    fn domain(&self) -> GeneralEvaluationDomain<F> {
        self.col.domain()
    }

    fn domain_4x(&self) -> GeneralEvaluationDomain<F> {
        self.col.domain_4x()
    }

    fn as_poly(&self) -> &DensePolynomial<F> {
        self.col.as_poly()
    }
}


pub struct Booleanity<F: FftField> {
    bits: BitColumn<F>,
}


impl<'a, F: FftField> Booleanity<F> {
    pub fn init(bits: BitColumn<F>) -> Self {
        Self { bits }
    }

    pub fn constraints(&self) -> Vec<Evaluations<F>> {
        let mut c = const_evals(F::one(), self.bits.domain_4x()); // c = 1
        let b = &self.bits.col.evals_4x;
        c -= b; // c = 1 - b
        c *= b; // c = (1 - b) * b
        vec![c]
    }

    pub fn constraints_linearized(&self, _z: &F) -> Vec<DensePolynomial<F>> {
        vec![DensePolynomial::zero()]
    }
}


pub struct BooleanityValues<F: Field> {
    pub bits: F,
}


impl<F: Field> VerifierGadget<F> for BooleanityValues<F> {
    fn evaluate_constraints_main(&self) -> Vec<F> {
        let c = self.bits * (F::one() - self.bits);
        vec![c]
    }
}