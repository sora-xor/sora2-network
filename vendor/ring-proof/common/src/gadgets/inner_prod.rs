use ark_ff::{FftField, Field};
use ark_poly::{Evaluations, GeneralEvaluationDomain};
use ark_poly::univariate::DensePolynomial;

use crate::{Column, FieldColumn};
use crate::domain::Domain;
use crate::gadgets::{ProverGadget, VerifierGadget};

pub struct InnerProd<F: FftField> {
    a: FieldColumn<F>,
    b: FieldColumn<F>,
    not_last: FieldColumn<F>,
    pub acc: FieldColumn<F>,
}

pub struct InnerProdValues<F: Field> {
    pub a: F,
    pub b: F,
    pub not_last: F,
    pub acc: F,
}


impl<F: FftField> InnerProd<F> {
    pub fn init(a: FieldColumn<F>, b: FieldColumn<F>, domain: &Domain<F>) -> Self {
        assert_eq!(a.len, domain.capacity - 1); // last element is not constrained
        assert_eq!(b.len, domain.capacity - 1); // last element is not constrained
        let inner_prods = Self::partial_inner_prods(a.vals(), b.vals());
        let mut acc = vec![F::zero()];
        acc.extend(inner_prods);
        let acc = domain.column(acc);
        Self { a, b, not_last: domain.not_last_row.clone(), acc }
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
}

impl<F: FftField> ProverGadget<F> for InnerProd<F> {
    fn witness_columns(&self) -> Vec<DensePolynomial<F>> {
        vec![self.acc.poly.clone()]
    }

    fn constraints(&self) -> Vec<Evaluations<F>> {
        let a = &self.a.evals_4x;
        let b = &self.b.evals_4x;
        let acc = &self.acc.evals_4x;
        let acc_shifted = &self.acc.shifted_4x();
        let not_last = &self.not_last.evals_4x;
        let c = &(&(acc_shifted - acc) - &(a * b)) * not_last;
        vec![c]
    }

    fn constraints_linearized(&self, _z: &F) -> Vec<DensePolynomial<F>> {
        let c = &self.acc.poly * self.not_last.evaluate(_z);
        vec![c]
    }

    fn domain(&self) -> GeneralEvaluationDomain<F> {
        self.a.evals.domain()
    }
}


impl<F: Field> VerifierGadget<F> for InnerProdValues<F> {
    fn evaluate_constraints_main(&self) -> Vec<F> {
        let c = (-self.acc - self.a * self.b) * self.not_last;
        vec![c]
    }
}


#[cfg(test)]
mod tests {
    use ark_ed_on_bls12_381_bandersnatch::Fq;
    use ark_ff::{Field, Zero};
    use ark_poly::Polynomial;
    use ark_std::test_rng;

    use crate::domain::Domain;
    use crate::test_helpers::random_vec;

    use super::*;

    fn inner_prod<F: Field>(a: &[F], b: &[F]) -> F {
        assert_eq!(a.len(), b.len());
        a.iter().zip(b)
            .map(|(a, b)| *a * b)
            .sum()
    }

    fn _test_inner_prod_gadget(hiding: bool) {
        let rng = &mut test_rng();

        let log_n = 10;
        let n = 2usize.pow(log_n);
        let domain = Domain::new(n, hiding);

        let a = random_vec(domain.capacity - 1, rng);
        let b = random_vec(domain.capacity - 1, rng);
        let ab = inner_prod(&a, &b);
        let a = domain.column(a);
        let b = domain.column(b);

        let gadget = InnerProd::<Fq>::init(a, b, &domain);

        let acc = &gadget.acc.evals.evals;
        assert!(acc[0].is_zero());
        assert_eq!(acc[domain.capacity - 1], ab);

        let constraint_poly = gadget.constraints()[0].interpolate_by_ref();

        assert_eq!(constraint_poly.degree(), 2 * n - 1);

        domain.divide_by_vanishing_poly(&constraint_poly);
    }

    #[test]
    fn test_inner_prod_gadget() {
        _test_inner_prod_gadget(false);
        _test_inner_prod_gadget(true);
    }
}