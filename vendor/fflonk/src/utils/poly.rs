use ark_ff::{FftField, Field, PrimeField, Zero};
use ark_poly::{DenseUVPolynomial, Polynomial};
use ark_poly::polynomial::univariate::DensePolynomial;
use ark_std::{vec, vec::Vec};

use crate::Poly;
use crate::utils::powers;

/// Field element represented as a constant polynomial.
pub(crate) fn constant<F: PrimeField>(c: F) -> Poly<F> {
    Poly::from_coefficients_vec(vec![c])
}


/// The vanishing polynomial of a point x.
/// z(X) = X - x
pub(crate) fn z_of_point<F: Field>(x: &F) -> Poly<F> {
    Poly::from_coefficients_vec(vec![x.neg(), F::one()])
}


/// The vanishing polynomial of a set.
/// z(X) = (X - x1) * .. * (X - xn)
pub(crate) fn z_of_set<'a, F: FftField>(xs: impl IntoIterator<Item=&'a F>) -> DensePolynomial<F> {
    xs.into_iter()
        .map(|x| z_of_point(x))
        .reduce(|a, b| &a * &b)
        .unwrap()
}


pub fn sum_with_coeffs<F: Field, P: Polynomial<F>>(
    coeffs: Vec<F>,
    polys: &[P],
) -> P {
    assert_eq!(coeffs.len(), polys.len());
    let mut res = P::zero();
    for (c, p) in coeffs.into_iter().zip(polys.iter()) {
        res += (c, p);
    }
    res
}


pub fn sum_with_powers<F: Field, P: Polynomial<F>>(
    r: F,
    polys: &[P],
) -> P {
    let powers = powers(r).take(polys.len()).collect::<Vec<_>>();
    sum_with_coeffs(powers, polys)
}


pub fn interpolate<F: PrimeField>(xs: &[F], ys: &[F]) -> Poly<F> {
    let x1 = xs[0];
    let mut l = z_of_point(&x1);
    for &xj in xs.iter().skip(1) {
        let q = z_of_point(&xj);
        l = &l * &q;
    }

    let mut ws = vec![];
    for xj in xs {
        let mut wj = F::one();
        for xk in xs {
            if xk != xj {
                let d = *xj - xk;
                wj *= d;
            }
        }
        ws.push(wj);
    }
    ark_ff::batch_inversion(&mut ws);

    let mut res = Poly::zero();
    for ((&wi, &xi), &yi) in ws.iter().zip(xs).zip(ys) {
        let d = z_of_point(&xi);
        let mut z = &l / &d;
        z = &z * wi;
        z = &z * yi;
        res = res + z;
    }
    res
}


/// Given a polynomial `r` in evaluation form {(xi, yi)},
/// i.e. lowest degree `r` such that `r(xi) = yi` for all `i`s,
/// and a point zeta,
/// computes `r(zeta)` and `z(zeta)`,
/// where `z` is the vanishing polynomial of `x`s.
// Implements barycentric formula of some form.
pub(crate) fn interpolate_evaluate<F: PrimeField>(xs: &[F], ys: &[F], zeta: &F) -> (F, F) {
    assert_eq!(xs.len(), ys.len());

    let zeta_minus_xs = ark_std::iter::repeat(zeta).zip(xs.iter())
        .map(|(&zeta, xi)| zeta - xi)
        .collect::<Vec<_>>();

    let l_at_zeta = zeta_minus_xs.iter().cloned()
        .reduce(|acc, item| item * acc)
        .expect("TODO");

    let mut ws = vec![];
    for xj in xs {
        let mut wj = F::one();
        for xk in xs {
            if xk != xj {
                let d = *xj - xk;
                wj *= d;
            }
        }
        ws.push(wj);
    }

    let mut denominator = ws.into_iter().zip(zeta_minus_xs.iter())
        .map(|(a, b)| a * b)
        .collect::<Vec<_>>();

    ark_ff::batch_inversion(&mut denominator);

    let sum = denominator.into_iter().zip(ys.iter()).map(|(a, b)| a * b).sum::<F>();
    (sum * l_at_zeta, l_at_zeta)
}

#[cfg(test)]
mod tests {
    use ark_ff::UniformRand;
    use ark_poly::Polynomial;
    use ark_std::test_rng;

    use crate::tests::BenchField;
    use crate::utils::poly::z_of_set;

    use super::*;

    #[test]
    fn test_interpolation() {
        let rng = &mut test_rng();

        let d = 15;
        let (xs, ys): (Vec<_>, Vec<_>) = (0..d + 1)
            .map(|_| (BenchField::rand(rng), BenchField::rand(rng)))
            .unzip();

        let poly = interpolate(&xs, &ys);

        assert_eq!(poly.degree(), d);
        assert!(xs.iter().zip(ys.iter()).all(|(x, &y)| poly.evaluate(x) == y));

        for _ in 0..10 {
            let zeta = BenchField::rand(rng);
            let (r_at_zeta, z_at_zeta) = interpolate_evaluate(&xs, &ys, &zeta);
            assert_eq!(r_at_zeta, poly.evaluate(&zeta));
            assert_eq!(z_at_zeta, z_of_set(&xs).evaluate(&zeta));
        }
    }
}
