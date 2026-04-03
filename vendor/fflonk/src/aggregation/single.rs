use ark_ec::{AffineRepr, CurveGroup};
use ark_ff::{PrimeField, Zero};
use ark_poly::Polynomial;
use ark_std::vec::Vec;

use crate::pcs::{Commitment, PCS};
use crate::Poly;
use crate::utils::ec::small_multiexp_affine;

/// A tuple (c, x, y) of the form (G, F, F). Represents a claim that {f(x) = y, for a polynomial f such that commit(f) = c}.
/// In other words, it is am instance in some language of "correct polynomial evaluations".
/// Soundness properties of a claim are defined by that of the argument.
#[derive(Clone, Debug)]
pub struct Claim<F: PrimeField, C: Commitment<F>> {
    pub c: C,
    pub x: F,
    pub y: F,
}

impl<F: PrimeField, C: Commitment<F>> Claim<F, C> {
    pub fn new<CS>(ck: &CS::CK, poly: &Poly<F>, at: F) -> Claim<F, C> where CS: PCS<F, C=C> {
        Claim {
            c: CS::commit(ck, poly),
            x: at,
            y: poly.evaluate(&at),
        }
    }
}


/// Aggregates claims for different polynomials evaluated at the same point.
///
/// Claims `[(Ci, xi, yi)]`, such that `xi = x` for any `i`,
/// can be aggregated using randomness `r` to a claim `(C', x, y')`,
/// where `C' = r_agg([Ci], r)` and `y' = r_agg([yi], r)`.
///
/// If CS is knowledge-sound than an aggregate opening is a proof of knowledge for
/// `{[(C_i, x, y_i)]; [f_i]): fi(x) = yi and CS::commit(fi) = ci}`.
pub fn aggregate_claims<F: PrimeField, CS: PCS<F>>(claims: &[Claim<F, CS::C>], rs: &[F]) -> Claim<F, CS::C> {
    assert_eq!(claims.len(), rs.len());

    let mut iter_over_xs = claims.iter().map(|cl| cl.x);
    let same_x = iter_over_xs.next().expect("claims is empty");
    assert!(iter_over_xs.all(|x| x == same_x), "multiple evaluation points");

    // TODO: Detect duplicate claims?
    // Consider (Cf, x, y1) and (Cf, x, y2).
    // If y1 = y2 = f(x) both claims are valid
    // If y1 != y2, at least one of the 2 claims is invalid

    let (rcs, rys): (Vec<CS::C>, Vec<F>) = claims.iter().zip(rs.iter())
        .map(|(cl, &r)| (cl.c.mul(r), r * cl.y))
        .unzip();

    Claim {
        c: rcs.into_iter().sum(),
        x: same_x,
        y: rys.iter().sum(),
    }
}

pub fn aggregate_claims_multiexp<F, C>(cs: Vec<C>, ys: Vec<F>, rs: &[F]) -> (C, F)
    where
        F: PrimeField,
        C: AffineRepr<ScalarField=F>
{
    assert_eq!(cs.len(), rs.len());
    assert_eq!(ys.len(), rs.len());

    let agg_c = small_multiexp_affine(rs, &cs);
    let agg_y = ys.into_iter().zip(rs.iter()).map(|(y, r)| y * r).sum();

    (agg_c.into_affine(), agg_y)
}

// for opening in a single point, the aggregate polynomial doesn't depend on the point.
pub fn aggregate_polys<F: PrimeField>(polys: &[Poly<F>], rs: &[F]) -> Poly<F> {
    assert_eq!(polys.len(), rs.len());
    polys.iter().zip(rs.iter())
        .map(|(p, &r)| p * r)
        .fold(Poly::zero(), |acc, p| acc + p)
}

#[cfg(test)]
mod tests {
    use ark_poly::DenseUVPolynomial;
    use ark_std::test_rng;

    use crate::pcs::IdentityCommitment;
    use crate::pcs::PcsParams;
    use crate::tests::{TestField, TestKzg};

    use super::*;

    fn _test_aggregation<F: PrimeField, CS: PCS<F>>() {
        let rng = &mut test_rng();
        let d = 15;
        let t = 4;
        let params = CS::setup(d, rng);
        let ck = params.ck();

        assert!(aggregate_polys::<F>(&[], &[]).is_zero());

        // common randomness
        let rs = (0..t).map(|_| F::rand(rng)).collect::<Vec<_>>();

        let polys = (0..t).map(|_| Poly::<F>::rand(d, rng)).collect::<Vec<_>>();
        let agg_poly = aggregate_polys(&polys, &rs);

        let same_x = F::rand(rng);
        let claims_at_same_x = polys.iter()
            .map(|p| Claim::new::<CS>(&ck, p, same_x))
            .collect::<Vec<_>>();
        let agg_claim = aggregate_claims::<F, CS>(&claims_at_same_x, &rs);


        assert_eq!(CS::commit(&ck, &agg_poly), agg_claim.c);
        assert_eq!(same_x, agg_claim.x);
        assert_eq!(agg_poly.evaluate(&same_x), agg_claim.y);
    }

    #[test]
    fn test_aggregation() {
        _test_aggregation::<TestField, IdentityCommitment>();
        _test_aggregation::<TestField, TestKzg>();
    }
}
