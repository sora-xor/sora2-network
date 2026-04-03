use ark_ff::PrimeField;
use ark_poly::Polynomial;
use ark_std::{end_timer, start_timer};
use ark_std::{vec, vec::Vec};
use ark_std::iterable::Iterable;
use ark_std::collections::BTreeSet;

use crate::{EuclideanPolynomial, Poly, utils};
use crate::pcs::{Commitment, PCS};
use crate::utils::poly;
use crate::utils::poly::interpolate_evaluate;

pub struct MultipointClaim<F: PrimeField, C: Commitment<F>> {
    pub c: C,
    pub xs: Vec<F>,
    pub ys: Vec<F>,
}

pub trait Transcript<F: PrimeField, CS: PCS<F>> {
    fn get_gamma(&mut self) -> F;
    fn commit_to_q(&mut self, q: &CS::C);
    fn get_zeta(&mut self) -> F;
}


pub fn aggregate_polys<F: PrimeField, CS: PCS<F>, T: Transcript<F, CS>>(
    ck: &CS::CK,
    fs: &[Poly<F>],
    xss: &[BTreeSet<F>],
    transcript: &mut T,
) -> (Poly<F>, F, CS::C) {
    assert_eq!(xss.len(), fs.len(), "{} opening sets specified for {} polynomials", xss.len(), fs.len());
    // Both Halo-inf and fflonk/shplonk use the notation "complement" in set-theoretical sense to that used in the code.
    // The papers consider vanishing polynomials of the complements of the opening sets,
    // while in the code vanishing polynomials of the opening sets are used directly.
    // Comments bellow bridge notation between the code and the papers to explain that the code is equivalent
    // using https://eprint.iacr.org/2021/1167.pdf, Lemma 4.2. as the authority.

    // zi - the vanishing polynomial of the set xsi ("Si" in the paper) of the opening points for fi, i = 0,...,k-1
    let zs: Vec<_> = xss.iter()
        .map(|xsi| poly::z_of_set(xsi))
        .collect();
    // The paper defines "T" as the set of all the opening points, "Z_T", it's vanishing polynomial,
    // and "Z_{T\S_i}" as the vanishing polynomial of the complement of "Si" in "T".
    // Observe that for zi computed above, "Z_T" = zi * "Z_{T\S_i}"     (*)


    // (qi, ri) - the quotient and the remainder of division of fi by the corresponding vanishing polynomial zi
    // qi = (fi - ri) / zi     (**)
    let t_divisions = start_timer!(|| "polynomial divisions");
    let (qs, rs): (Vec<_>, Vec<_>) = fs.iter().zip(zs.iter())
        .map(|(fi, zi)| fi.divide_with_q_and_r(zi))
        .unzip();
    end_timer!(t_divisions);

    let gamma = transcript.get_gamma();

    // The paper defines f = sum(gamma^i * "Z_{T\S_i}" * (fi - ri))
    // Let q := f / "Z_T"
    // By (*) "Z_T" = zi * "Z_{T\S_i}", hence q = f / (zi * "Z_{T\S_i})" = sum(gamma^i * (fi - ri) / zi)
    // By (**) qi = (fi - ri) / zi, thus q = sum(gamma^i * qi)
    let q = poly::sum_with_powers(gamma, &qs);
    let t_commit = start_timer!(|| format!("commitment to a degree-{} polynomial", q.degree()));
    let qc = CS::commit(ck, &q);
    // "W" in the paper
    end_timer!(t_commit);
    transcript.commit_to_q(&qc);

    let zeta = transcript.get_zeta();

    let rs_at_zeta: Vec<_> = rs.iter().map(|ri| ri.evaluate(&zeta)).collect();
    let zs_at_zeta: Vec<_> = zs.iter().map(|zi| zi.evaluate(&zeta)).collect();

    // Let pi(X) = fi(X) - ri(zeta)
    let ps: Vec<Poly<F>> = fs.iter().zip(rs_at_zeta)
        .map(|(fi, ri)| fi - &poly::constant(ri))
        .collect();

    // From (*) follows that "Z_{T\S_i}"(zeta) = "Z_T"(zeta) / zi(zeta), so
    // 1. "L" = sum([gamma^i * "Z_T"(zeta) / zi(zeta)] * pi) - "Z_T"(zeta) * q
    // 2. "Z_{T\S_0}"(zeta) = "Z_T"(zeta) / z0(zeta)
    // We want to compute l_norm = "L"/"Z_{T\S_0}"(zeta) = "L" * z0(zeta) / "Z_T"(zeta)
    // Notice that "Z_T"(zeta) cancels out from the both terms of "L"

    // Finally l_norm = sum([gamma^i * z0(zeta) / zi(zeta)] * pi) - z0(zeta) * q
    // normalizer := z0(zeta)
    // coeff_i := gamma^i * z0(zeta) / zi(zeta)
    let (coeffs, normalizer) = get_coeffs(zs_at_zeta, gamma);
    let t_combine = start_timer!(|| "linear combination of polynomials");
    let l_norm = &poly::sum_with_coeffs(coeffs, &ps) - &(&q * normalizer);
    end_timer!(t_combine);

    // It remains to notice that "W'" is a KZG opening proof for polynomial l_norm in point zeta.
    (l_norm, zeta, qc)
}

/// Takes evaluations of vanishing polynomials at a random point `zeta`, and a random challenge `gamma`,
/// and returns coefficients for the random linear combination of polynomials/commitments.
fn get_coeffs<F: PrimeField>(zs_at_zeta: Vec<F>, gamma: F) -> (Vec<F>, F) {
    assert!(!zs_at_zeta.is_empty(), "empty vec");
    let normalizer = zs_at_zeta[0];
    let mut zs_at_zeta_inv = zs_at_zeta;
    ark_ff::batch_inversion(&mut zs_at_zeta_inv);

    let coeffs = zs_at_zeta_inv.iter().zip(utils::powers(gamma))
        .map(|(zi_inv, gamma_to_i)| gamma_to_i * zi_inv * normalizer)
        .collect();

    (coeffs, normalizer)
}

pub fn group_by_commitment<F: PrimeField, C: Commitment<F>>(
    fcs: &[C],
    xss: &Vec<Vec<F>>,
    yss: &Vec<Vec<F>>,
) -> Vec<MultipointClaim<F, C>> {
    fcs.iter().cloned()
        .zip(xss.iter().cloned())
        .zip(yss.iter().cloned())
        .map(|((c, xs), ys)| MultipointClaim { c, xs, ys })
        .collect()
}

pub fn aggregate_claims<F: PrimeField, CS: PCS<F>, T: Transcript<F, CS>>(
    claims: Vec<MultipointClaim<F, CS::C>>,
    qc: &CS::C,
    onec: &CS::C,
    transcript: &mut T,
) -> MultipointClaim<F, CS::C>
{
    let gamma = transcript.get_gamma();
    transcript.commit_to_q(&qc);
    let zeta = transcript.get_zeta();

    // For each polynomial fi the opening claim {(xj, yj)} can be presented in polynomial form
    // as a pair of polynomials (ri, zi), where zi is the vanishing polynomial of the set {xj},
    // and ri is the interpolation polynomial of the set {(xj, yj)}.
    // ri(zeta), zi(zeta)
    let t_eval = start_timer!(|| "barycentric evaluations");
    let (rs_at_zeta, zs_at_zeta): (Vec<_>, Vec<_>) = claims.iter()
        .map(|MultipointClaim { c: _, xs, ys }| interpolate_evaluate(xs, ys, &zeta))
        .unzip();
    end_timer!(t_eval);

    let (mut coeffs, normalizer) = get_coeffs(zs_at_zeta, gamma);
    assert!(coeffs[0].is_one());

    let agg_r_at_zeta: F = rs_at_zeta.into_iter().zip(coeffs.iter())
        .map(|(ri_at_zeta, coeff)| ri_at_zeta * coeff)
        .sum();

    let mut commitments = claims.into_iter().map(|cl| cl.c).collect::<Vec<_>>();
    coeffs.push(-agg_r_at_zeta);
    commitments.push(onec.clone());
    coeffs.push(-normalizer);
    commitments.push(qc.clone());

    let t_combine = start_timer!(|| "multiexp");
    let lc = CS::C::combine(&coeffs, &commitments);
    end_timer!(t_combine);
    MultipointClaim { c: lc, xs: vec![zeta], ys: vec![F::zero()] }
}


#[cfg(test)]
mod tests {
    use ark_ff::{One, UniformRand};
    use ark_std::{end_timer, start_timer};
    use ark_std::iter::FromIterator;
    use ark_std::test_rng;

    use crate::pcs::IdentityCommitment;
    use crate::pcs::PcsParams;
    use crate::shplonk::tests::{random_opening, random_xss};
    use crate::tests::{BENCH_DEG_LOG1, BenchField, BenchKzg, TestField, TestKzg};

    use super::*;

    #[test]
    fn test_get_coeffs() {
        let rng = &mut test_rng();

        let zs = (0..10).map(|_| TestField::rand(rng)).collect::<Vec<_>>();

        let gamma = TestField::rand(rng);
        let (coeffs, _) = get_coeffs(zs.clone(), gamma);
        assert_eq!(coeffs.len(), zs.len());
        assert!(coeffs[0].is_one());

        let gamma = TestField::one();
        let (coeffs, normalizer) = get_coeffs(zs.clone(), gamma);
        assert!(coeffs.iter().zip(zs).all(|(c, z)| z * c == normalizer));
    }

    fn _test_aggregation<F: PrimeField, CS: PCS<F>>(d: usize) { // degree of polynomials
        let rng = &mut test_rng();

        let t = 8; // number of polynomials
        let max_m = 3; // maximal number of opening points per polynomial

        let params = CS::setup(d, rng);
        let (ck, vk) = (params.ck(), params.vk());

        let xss = random_xss(rng, t, max_m);
        let opening = random_opening::<_, _, CS>(rng, &ck, d, t, xss);

        let sets_of_xss: Vec<BTreeSet<F>> = opening.xss.iter()
            .map(|xs| BTreeSet::from_iter(xs.iter().cloned()))
            .collect();

        let transcript = &mut (F::rand(rng), F::rand(rng));

        let t_aggregate_polys = start_timer!(|| format!("Aggregate {} degree-{} polynomials", t, d));
        let (agg_poly, zeta, agg_proof) = aggregate_polys::<_, CS, _>(&ck, &opening.fs, &sets_of_xss, transcript);
        end_timer!(t_aggregate_polys);

        let claims = group_by_commitment(&opening.fcs, &opening.xss, &opening.yss);
        let onec = CS::commit(&vk.clone().into(), &poly::constant(F::one()));

        let t_aggregate_claims = start_timer!(|| format!("Aggregate {} claims", claims.len()));
        let agg_claim = aggregate_claims::<_, CS, _>(claims, &agg_proof, &onec, transcript);
        end_timer!(t_aggregate_claims);

        assert_eq!(CS::commit(&ck, &agg_poly), agg_claim.c);
        assert_eq!(zeta, agg_claim.xs[0]);
        assert_eq!(agg_poly.evaluate(&zeta), agg_claim.ys[0]);
        assert!(agg_claim.ys[0].is_zero());
    }

    #[test]
    fn test_aggregation() {
        _test_aggregation::<TestField, IdentityCommitment>(15);
        _test_aggregation::<TestField, TestKzg>(15);
    }

    #[test]
    #[ignore]
    fn bench_aggregation() {
        _test_aggregation::<BenchField, BenchKzg>((1 << BENCH_DEG_LOG1) - 1);
    }
}