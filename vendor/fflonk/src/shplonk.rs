use ark_ff::PrimeField;
use ark_poly::{DenseUVPolynomial, Polynomial};
use ark_serialize::*;
use ark_std::vec::Vec;
use ark_std::marker::PhantomData;
use ark_std::collections::BTreeSet;

use crate::aggregation::multiple::{aggregate_claims, aggregate_polys, group_by_commitment, Transcript};
use crate::pcs::PCS;
use crate::Poly;

pub struct Shplonk<F: PrimeField, CS: PCS<F>> {
    _field: PhantomData<F>,
    _pcs: PhantomData<CS>,
}

#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct AggregateProof<F: PrimeField, CS: PCS<F>> {
    agg_proof: CS::C,
    opening_proof: CS::Proof,
}

impl<F: PrimeField, CS: PCS<F>> Shplonk<F, CS> {
    pub fn open_many<T: Transcript<F, CS>>(
        ck: &CS::CK,
        fs: &[Poly<F>],
        xss: &[BTreeSet<F>],
        transcript: &mut T,
    ) -> AggregateProof<F, CS>
    {
        let (agg_poly, zeta, agg_proof) = aggregate_polys::<F, CS, T>(ck, fs, xss, transcript);
        assert!(agg_poly.evaluate(&zeta).is_zero());
        let opening_proof = CS::open(ck, &agg_poly, zeta);
        AggregateProof {agg_proof, opening_proof}
    }

    pub fn verify_many<T: Transcript<F, CS>>(
        vk: &CS::VK,
        fcs: &[CS::C],
        proof: AggregateProof<F, CS>,
        xss: &Vec<Vec<F>>,
        yss: &Vec<Vec<F>>,
        transcript: &mut T,
    ) -> bool
    {
        let AggregateProof {agg_proof, opening_proof} = proof;
        let onec = CS::commit(&vk.clone().into(), &Poly::from_coefficients_slice(&[F::one()]));
        let claims = group_by_commitment(fcs, xss, yss);
        let agg_claim = aggregate_claims::<F, CS, T>(claims, &agg_proof, &onec, transcript);
        CS::verify(vk, agg_claim.c, agg_claim.xs[0], agg_claim.ys[0], opening_proof)
    }
}


#[cfg(test)]
pub(crate) mod tests {
    use ark_std::iter::FromIterator;
    use ark_std::rand::Rng;
    use ark_std::test_rng;

    use crate::pcs::{Commitment, PcsParams};
    use crate::pcs::IdentityCommitment;
    use crate::Poly;
    use crate::tests::{TestField, TestKzg};

    use super::*;

    pub struct TestOpening<F: PrimeField, C: Commitment<F>> {
        pub fs: Vec<Poly<F>>,
        pub fcs: Vec<C>,
        pub xss: Vec<Vec<F>>,
        pub yss: Vec<Vec<F>>,
    }

    pub(crate) fn random_xss<R: Rng, F: PrimeField>(
        rng: &mut R,
        t: usize, // number of polynomials
        max_m: usize, // maximal number of opening points per polynomial
    ) -> Vec<Vec<F>> {
        (0..t)
            .map(|_| (0..rng.gen_range(1..max_m))
                .map(|_| F::rand(rng)).collect::<Vec<_>>())
            .collect()
    }

    pub(crate) fn random_opening<R, F, CS>(
        rng: &mut R,
        ck: &CS::CK,
        d: usize, // degree of polynomials
        t: usize, // number of polynomials
        xss: Vec<Vec<F>>, // vecs of opening points per polynomial
    ) -> TestOpening<F, CS::C> where
        R: Rng,
        F: PrimeField,
        CS: PCS<F>,
    {
        // polynomials
        let fs: Vec<_> = (0..t)
            .map(|_| Poly::<F>::rand(d, rng))
            .collect();
        // commitments
        let fcs: Vec<_> = fs.iter()
            .map(|fi| CS::commit(&ck, fi))
            .collect();

        // evaluations per polynomial
        let yss: Vec<_> = fs.iter()
            .zip(xss.iter())
            .map(|(f, xs)|
                xs.iter().map(
                    |x| f.evaluate(x))
                    .collect::<Vec<_>>()
            ).collect();

        TestOpening { fs, fcs, xss, yss }
    }

    fn _test_shplonk<F: PrimeField, CS: PCS<F>>() {
        let rng = &mut test_rng();

        let d = 15; // degree of polynomials
        let t = 4; // number of polynomials
        let max_m = 3; // maximal number of opening points per polynomial

        let params = CS::setup(d, rng);

        let xss = random_xss(rng, t, max_m);
        let opening = random_opening::<_, _, CS>(rng, &params.ck(), d, t, xss);

        let sets_of_xss: Vec<BTreeSet<F>> = opening.xss.iter()
            .map(|xs| BTreeSet::from_iter(xs.iter().cloned()))
            .collect();

        let transcript = &mut (F::rand(rng), F::rand(rng));

        let proof = Shplonk::<F, CS>::open_many(&params.ck(), &opening.fs, &sets_of_xss, transcript);

        assert!(Shplonk::<F, CS>::verify_many(&params.vk(), &opening.fcs, proof, &opening.xss, &opening.yss, transcript))
    }

    #[test]
    fn test_shplonk() {
        _test_shplonk::<TestField, IdentityCommitment>();
        _test_shplonk::<TestField, TestKzg>();
    }
}