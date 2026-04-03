//! [fflonk: a Fast-Fourier inspired verifier efficient version of PlonK](https://eprint.iacr.org/2021/1167)
//! by Ariel Gabizon and Zachary J. Williamson suggests a reduction from opening multiple
//! polynomials each in the same point to opening a single polynomial in multiple points.

use ark_ff::FftField;
use ark_poly::DenseUVPolynomial;
use ark_std::convert::TryInto;
use ark_std::marker::PhantomData;
use ark_std::ops::Div;
use ark_std::{vec, vec::Vec};

use crate::utils;

pub struct Fflonk<F: FftField, P: DenseUVPolynomial<F>> {
    _field: PhantomData<F>,
    _poly: PhantomData<P>,
}

impl<F: FftField, P: DenseUVPolynomial<F>> Fflonk<F, P>
    where for<'a, 'b> &'a P: Div<&'b P, Output=P>

{
    // Given `t` degree `<d` polynomials `fi`, returns a single degree `<dt` polynomial
    // `g(X) = sum fi(X^t)X^i`, `i=0,...,t-1`.
    pub fn combine(t: usize, fs: &[P]) -> P {
        assert!(fs.len() <= t);
        let max_degree = fs.iter().map(|fi| fi.degree()).max().unwrap();
        // Flattens the matrix (given as a list of rows) of coefficients by concatenating its columns.
        // Rows are right padded by 0s to `max_degree + 1`. If `fs.len() < t`, zero rows are added.
        let mut res = vec![F::zero(); t * (max_degree + 1)];
        for (i, fi) in fs.iter().enumerate() {
            for (j, fij) in fi.coeffs().iter().enumerate() {
                res[t * j + i] = *fij;
            }
        }
        P::from_coefficients_vec(res)
    }

    // Given a `t`-th root `z` of `x` returns all the `t`-th roots of `x`
    // `z, zw, ..., zw^{t-1}`, where w is a primitive `t`-th root of unity.
    // TODO: fix the order
    pub fn roots(t: usize, root_t_of_x: F) -> Vec<F> {
        let omega_t = F::get_root_of_unity(t.try_into().unwrap()).expect("root of unity not found");
        let mut acc = root_t_of_x;
        let mut res = vec![root_t_of_x];
        res.resize_with(t, || {
            acc *= omega_t;
            acc
        });
        res
    }

    // The vanishing polynomial of the set of all the t-th roots of x,
    // given any of its t-th roots.
    // Z(x) = X^t-x
    fn z_of_roots(t: usize, root_t_of_x: F) -> P {
        let x = root_t_of_x.pow([t as u64]);
        let mut z = vec![F::zero(); t + 1]; // deg(Z) = t
        // coeffs(Z) = [-x, ..., 1]
        z[0] = -x;
        z[t] = F::one();
        P::from_coefficients_vec(z)
    }

    // Reduces opening of f1,...,ft in 1 point to opening of g = combine(f1,...,ft) in t points.
    // The input opening is given as an evaluation point x (it's t-th root)
    // and a list of values fj(x), j=1,...,t.
    // The output opening is returned as the vanishing polynomial z of the points and the remainder r.
    pub fn opening_as_polynomials(t: usize, root_of_x: F, evals_at_x: &[F]) -> (P, P) {
        let z = Self::z_of_roots(t, root_of_x);
        let r = P::from_coefficients_slice(evals_at_x);
        (z, r)
    }

    // Let z be some t-th root of x. Then all the t roots of x of degree t are given by zj = z*w^j, j=0,...,t-1, where w is a primitive t-th root of unity.
    // Given vi=fi(x), i=0,...,t-1 -- evaluations of t polynomials each in the same point x,
    // computes sum(vi*zj^i, i=0,...,t-1), j=0,...,t-1.
    pub fn opening_as_points(t: usize, root_of_x: F, evals_at_x: &[F]) -> (Vec<F>, Vec<F>) {
        assert_eq!(evals_at_x.len(), t); //TODO: may be 0-padded
        let roots = Self::roots(t, root_of_x);
        let evals_at_roots = roots.iter().map(|&root| {
            evals_at_x.iter()
                .zip(utils::powers(root))
                .map(|(&eval, next_root)| eval * next_root).sum()
        }).collect();
        (roots, evals_at_roots)
    }

    // Reduces opening of f1,...,ft in m points to opening of g = combine(f1,...,ft) in m*t points,
    // The input opening is given as a list of evaluation points x1,...,xm (their t-th roots)
    // and a list of lists of values [[fj(xi), j=1,...,t], i=1,...,m].
    // The output opening is returned as a list of evaluation points and a list of values.
    pub fn multiopening(t: usize, roots_of_xs: &[F], evals_at_xs: &[Vec<F>]) -> (Vec<F>, Vec<F>) {
        assert_eq!(roots_of_xs.len(), evals_at_xs.len());
        assert!(evals_at_xs.iter().all(|evals_at_x| evals_at_x.len() == t));
        let polys = evals_at_xs.iter()
            .map(|evals_at_x| P::from_coefficients_slice(evals_at_x));
        let roots = roots_of_xs.iter()
            .map(|&root_of_x| Self::roots(t, root_of_x));
        let xs: Vec<_> = roots.clone().flatten().collect();
        let vs: Vec<_> = polys.zip(roots)
            .flat_map(|(poly, roots)| Self::multievaluate(&poly, &roots))
            .collect();
        (xs, vs)
    }

    // TODO: improve
    fn multievaluate(poly: &P, xs: &[F]) -> Vec<F> {
        assert!(poly.degree() + 1 <= xs.len());
        xs.iter().map(|p| poly.evaluate(p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use ark_ff::Field;
    use ark_poly::Polynomial;
    use ark_poly::univariate::{DenseOrSparsePolynomial, DensePolynomial};
    use ark_std::{test_rng, UniformRand, Zero};

    use super::*;

    type F = ark_bw6_761::Fr;
    type P = DensePolynomial<F>;

    type FflonkBw6 = Fflonk<F, P>;

    #[test]
    fn test_single_opening() {
        let rng = &mut test_rng();

        let d = 15; // degree of polynomials
        let t = 4; // number of polynomials
        let root_t_of_x = F::rand(rng); // a t-th root of the opening point
        let x = root_t_of_x.pow([t as u64]); // the opening point

        let fs: Vec<P> = (0..t)
            .map(|_| P::rand(d, rng))
            .collect();
        let fs_at_x: Vec<F> = fs.iter() //
            .map(|fi| fi.evaluate(&x))
            .collect();

        let g = FflonkBw6::combine(t, &fs);

        let (z, r) = FflonkBw6::opening_as_polynomials(t, root_t_of_x, &fs_at_x);
        let (xs, vs) = FflonkBw6::opening_as_points(t, root_t_of_x, &fs_at_x);

        // g(xi) = vi
        assert!(xs.iter().zip(vs.iter()).all(|(x, &v)| g.evaluate(x) == v));
        // z -- vanishes xs
        assert!(xs.iter().all(|x| z.evaluate(x).is_zero()));
        // r -- interpolates vs in xs
        assert!(xs.iter().zip(vs.iter()).all(|(x, &v)| r.evaluate(x) == v));
        // g mod z = r
        let (_, g_mod_z) = DenseOrSparsePolynomial::divide_with_q_and_r(
            &(&g.into()),
            &(&z.into()),
        ).unwrap();
        assert_eq!(r, g_mod_z);
    }

    #[test]
    fn test_multiopening() {
        let rng = &mut test_rng();

        let d = 15; // degree of polynomials
        let t = 4; // number of polynomials
        let m = 3; // number of opening points

        let roots_of_xs: Vec<F> = (0..m) // t-th roots of opening points
            .map(|_| F::rand(rng))
            .collect();
        let xs: Vec<F> = roots_of_xs.iter() // opening points
            .map(|root_t_of_x| root_t_of_x.pow([t as u64]))
            .collect();

        let fs: Vec<P> = (0..t)
            .map(|_| P::rand(d, rng))
            .collect();
        let fs_at_xs: Vec<Vec<F>> = xs.iter()
            .map(|x| fs.iter().map(|fi| fi.evaluate(&x)).collect())
            .collect();

        let g = FflonkBw6::combine(t, &fs);

        let (xs, vs) = FflonkBw6::multiopening(t, &roots_of_xs, &fs_at_xs);

        assert!(xs.iter().zip(vs).all(|(x, v)| g.evaluate(x) == v));
    }

    #[test]
    fn test_openings_consistency() {
        let rng = &mut test_rng();

        let d = 15; // degree of polynomials
        let t = 4; // number of polynomials
        let root_t_of_x = F::rand(rng); // a t-th root of the opening point
        let x = root_t_of_x.pow([t as u64]); // the opening point

        let fs: Vec<P> = (0..t)
            .map(|_| P::rand(d, rng))
            .collect();
        let fs_at_x: Vec<F> = fs.iter() //
            .map(|fi| fi.evaluate(&x))
            .collect();

        let (z, r) = FflonkBw6::opening_as_polynomials(t, root_t_of_x, &fs_at_x);
        let (xs, vs) = FflonkBw6::multiopening(t, &[root_t_of_x], &[fs_at_x]);

        assert!(xs.iter().all(|x| z.evaluate(x).is_zero()));
        assert!(xs.iter().zip(vs).all(|(x, v)| r.evaluate(x) == v));
    }
}