use ark_ff::{FftField, Field};
use ark_poly::{Evaluations, GeneralEvaluationDomain};
use ark_poly::univariate::DensePolynomial;

use crate::{Column, const_evals, FieldColumn};
use crate::domain::Domain;
use crate::gadgets::{ProverGadget, VerifierGadget};

pub struct InnerProd<F: FftField> {
    a: FieldColumn<F>,
    b: FieldColumn<F>,
    l_last: FieldColumn<F>,
    pub acc: FieldColumn<F>,
    pub inner_prod: F,
}

pub struct InnerProdValues<F: Field> {
    pub a: F,
    pub b: F,
    pub l_last: F,
    pub acc: F,
    pub inner_prod: F,
}


impl<F: FftField> InnerProd<F> {
    pub fn init(a: FieldColumn<F>, b: FieldColumn<F>, domain: &Domain<F>) -> Self {
        assert_eq!(a.evals.evals.len(), domain.capacity);
        assert_eq!(b.evals.evals.len(), domain.capacity);
        let l_last = domain.l_last.clone();
        let inner_prods = Self::partial_inner_prods(&a.evals.evals, &b.evals.evals);
        let (&inner_prod, partial_prods) = inner_prods.split_last().unwrap();
        // 0, a[0]b[0], a[0]b[0] + a[1]b[1], ..., a[0]b[0] + a[1]b[1] + ... + a[n-2]b[n-2]
        let mut acc = vec![F::zero()];
        acc.extend(partial_prods);
        let acc = domain.column(acc);
        Self { a, b, acc, l_last, inner_prod }
    }

    /// Returns a[0]b[0], a[0]b[0] + a[1]b[1], ..., a[0]b[0] + a[1]b[1] + ... + a[n-1]b[n-1]
    fn partial_inner_prods(a: &[F], b: &[F]) -> Vec<F> {
        assert_eq!(a.len(), b.len());
        a.iter()
            .zip(b)
            .scan(F::zero(), |state, (&a, b)| {
                *state += a * b;
                Some(*state)
            })
            .collect()
    }

    fn evaluate_assignment(&self, zeta: &F) -> InnerProdValues<F> {
        InnerProdValues {
            a: self.a.evaluate(zeta),
            b: self.b.evaluate(zeta),
            l_last: self.l_last.evaluate(zeta), //TODO: can be done in O(1)
            acc: self.acc.evaluate(zeta),
            inner_prod: self.inner_prod,
        }
    }
}

impl<F: FftField> ProverGadget<F> for InnerProd<F> {
    fn witness_columns(&self) -> Vec<DensePolynomial<F>> {
        vec![self.acc.poly.clone()]
    }

    fn constraints(&self) -> Vec<Evaluations<F>> {
        let domain = self.l_last.domain_4x();
        let inner_prod = &const_evals(self.inner_prod, domain);
        let l_last = &self.l_last.evals_4x;
        let a = &self.a.evals_4x;
        let b = &self.b.evals_4x;
        let acc = &self.acc.evals_4x;
        let acc_shifted = &self.acc.shifted_4x();
        let c = &(&(acc_shifted - acc) - &(a * b)) + &(inner_prod * l_last);
        vec![c]
    }

    fn constraints_linearized(&self, _z: &F) -> Vec<DensePolynomial<F>> {
        let c = self.acc.as_poly();
        vec![c.clone()]
    }

    fn domain(&self) -> GeneralEvaluationDomain<F> {
        self.a.domain()
    }
}


impl<F: Field> VerifierGadget<F> for InnerProdValues<F> {
    fn evaluate_constraints_main(&self) -> Vec<F> {
        let c = self.inner_prod * self.l_last - self.a * self.b - self.acc;
        vec![c]
    }
}


#[cfg(test)]
mod tests {
    use ark_ed_on_bls12_381_bandersnatch::Fq;
    use ark_ff::{Field, Zero};
    use ark_poly::{GeneralEvaluationDomain, Polynomial};
    use ark_poly::EvaluationDomain;
    use ark_std::test_rng;

    use crate::test_helpers::random_vec;

    use super::*;

    fn inner_prod<F: Field>(a: &[F], b: &[F]) -> F {
        assert_eq!(a.len(), b.len());
        a.iter().zip(b)
            .map(|(a, b)| *a * b)
            .sum()
    }

    #[test]
    fn test_inner_prod_gadget() {
        let rng = &mut test_rng();

        let log_n = 16;
        let n = 2usize.pow(log_n);
        let domain = Domain::new(n, false);

        let a = random_vec(n, rng);
        let b = random_vec(n, rng);
        let a_col = domain.column(a.clone());
        let b_col = domain.column(b.clone());

        let gadget = InnerProd::<Fq>::init(a_col, b_col, &domain);

        assert_eq!(gadget.inner_prod, inner_prod(&a, &b));

        let acc = &gadget.acc.evals.evals;
        assert!(acc[0].is_zero());
        for i in 0..n - 1 {
            assert_eq!(acc[i + 1], acc[i] + a[i] * b[i])
        }

        let constraint_poly = gadget.constraints()[0].interpolate_by_ref();

        assert_eq!(constraint_poly.degree(), 2 * n - 2);

        let quotient = domain.divide_by_vanishing_poly(&constraint_poly);
    }
}