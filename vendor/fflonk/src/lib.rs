#![cfg_attr(not(feature = "std"), no_std)]

use ark_ff::PrimeField;
use ark_poly::univariate::{DenseOrSparsePolynomial, DensePolynomial};
use ark_std::rand::Rng;
use ark_std::vec::Vec;
use ark_std::marker::PhantomData;

use aggregation::multiple::Transcript;

use crate::fflonk::Fflonk;
use crate::pcs::PCS;
use crate::shplonk::{AggregateProof, Shplonk};

pub mod shplonk;
pub mod fflonk;
pub mod pcs;
pub mod utils;
pub mod aggregation;


pub type Poly<F> = DensePolynomial<F>; // currently SparsePolynomial doesn't implement DenseUVPolynomial anyway

pub trait EuclideanPolynomial<F: PrimeField> {
    fn divide_with_q_and_r(&self, divisor: &Poly<F>) -> (Poly<F>, Poly<F>);
}

impl<F: PrimeField> EuclideanPolynomial<F> for Poly<F> {
    fn divide_with_q_and_r(&self, divisor: &Poly<F>) -> (Poly<F>, Poly<F>) {
        let a: DenseOrSparsePolynomial<F> = self.into();
        let b: DenseOrSparsePolynomial<F> = divisor.into();
        a.divide_with_q_and_r(&b).unwrap()
    }
}


pub struct FflonkyKzg<F: PrimeField, CS: PCS<F>> {
    _field: PhantomData<F>,
    _pcs: PhantomData<CS>,
}

impl<F: PrimeField, CS: PCS<F>> FflonkyKzg<F, CS> {
    pub fn setup<R: Rng>(max_degree: usize, rng: &mut R) -> CS::Params {
        CS::setup(max_degree, rng)
    }

    pub fn open<T: Transcript<F, CS>>(
        ck: &CS::CK,
        fss: &[Vec<Poly<F>>], // vecs of polynomials to combine
        ts: &[usize], // lengths of each combination
        // TODO: ts can be inferred from li := len(fss[i]) as ti = min(x : x >= li and x | p-1)
        rootss: &[Vec<F>], // sets of opening points per a combined polynomial presented as t-th roots
        transcript: &mut T,
    ) -> AggregateProof<F, CS>
    {
        let k = fss.len();
        assert_eq!(k, ts.len());
        assert_eq!(k, rootss.len());
        let gs: Vec<Poly<F>> = fss.iter()
            .zip(ts.iter())
            .map(|(fs, t)| Fflonk::combine(*t, fs))
            .collect();
        let xss: Vec<_> = rootss.iter()
            .zip(ts.iter())
            .map(|(roots, t)|
                roots.iter()
                    .flat_map(|root| Fflonk::<F, Poly<F>>::roots(*t, *root))
                    .collect()
            ).collect();

        Shplonk::<F, CS>::open_many(ck, &gs, &xss, transcript)
    }

    pub fn verify<T: Transcript<F, CS>>(
        vk: &CS::VK,
        gcs: &[CS::C],
        ts: &[usize],
        proof: AggregateProof<F, CS>,
        rootss: &[Vec<F>],
        vss: &[Vec<Vec<F>>],
        transcript: &mut T,
    ) -> bool
    {
        let (xss, yss) = rootss.iter()
            .zip(vss.iter())
            .zip(ts.iter())
            .map(|((roots, vs), t)|
                Fflonk::<F, Poly<F>>::multiopening(*t, roots, vs)
            ).unzip();

        Shplonk::<F, CS>::verify_many(vk, &gcs, proof, &xss, &yss, transcript)
    }

    pub fn open_single<T: Transcript<F, CS>>(
        ck: &CS::CK,
        fs: &[Poly<F>], // polynomials to combine
        t: usize, // lengths of the combination
        roots: &[F], // set of opening points presented as t-th roots
        transcript: &mut T,
    ) -> AggregateProof<F, CS>
    {
        Self::open(ck, &[fs.to_vec()], &[t], &[roots.to_vec()], transcript)
    }

    pub fn verify_single<T: Transcript<F, CS>>(
        vk: &CS::VK,
        gc: &CS::C,
        t: usize,
        proof: AggregateProof<F, CS>,
        roots: &[F],
        vss: &[Vec<F>], // evaluations per point // TODO: shplonk provides API with evals per polynomial
        transcript: &mut T,
    ) -> bool
    {
        Self::verify(vk, &[(*gc).clone()], &[t], proof, &[roots.to_vec()], &[vss.to_vec()], transcript)
    }
}


#[cfg(test)]
mod tests {
    use ark_ec::pairing::Pairing;
    use ark_poly::{DenseUVPolynomial, Polynomial};
    use ark_std::rand::Rng;
    use ark_std::test_rng;
    use ark_std::vec;

    use crate::pcs::IdentityCommitment;
    use crate::pcs::kzg::KZG;
    use crate::pcs::PcsParams;

    use super::*;

    pub(crate) type TestCurve = ark_bls12_381::Bls12_381;
    pub(crate) type TestField = <TestCurve as Pairing>::ScalarField;
    pub(crate) type TestKzg = KZG::<TestCurve>;

    pub(crate) type BenchCurve = ark_bw6_761::BW6_761;
    pub(crate) type BenchField = <BenchCurve as Pairing>::ScalarField;

    #[allow(dead_code)] // used by ignored tests
    pub(crate) type BenchKzg = KZG::<BenchCurve>;

    pub const BENCH_DEG_LOG1: usize = 10;
    // pub const BENCH_DEG_LOG2: usize = 16;
    // const BENCH_DEG_LOG3: usize = 24; Eth 2.0 coming?

    impl<F: PrimeField, CS: PCS<F>> Transcript<F, CS> for (F, F) {
        fn get_gamma(&mut self) -> F {
            self.0
        }

        fn commit_to_q(&mut self, _q: &CS::C) {

        }

        fn get_zeta(&mut self) -> F {
            self.1
        }
    }

    fn generate_test_data<R, F>(
        rng: &mut R,
        d: usize, // degree of polynomials
        t: usize, // number of polynomials
        m: usize, // number of opening points
    ) -> (
        Vec<Poly<F>>, // polynomials
        Vec<F>, // roots of evaluation points
        Vec<Vec<F>>, // evaluations per point
    ) where
        R: Rng,
        F: PrimeField,
    {
        // polynomials
        let fs: Vec<Poly<F>> = (0..t)
            .map(|_| Poly::rand(d, rng))
            .collect();

        let roots: Vec<_> = (0..m)
            .map(|_| F::rand(rng))
            .collect();

        let xs: Vec<F> = roots.iter() // opening points
            .map(|root| root.pow([t as u64]))
            .collect();

        // evaluations per point
        let vss: Vec<_> = xs.iter()
            .map(|x|
                fs.iter()
                    .map(|f| f.evaluate(x))
                    .collect::<Vec<_>>()
            ).collect();

        (fs, roots, vss)
    }

    fn _test_fflonk_single<F: PrimeField, CS: PCS<F>>() {
        let rng = &mut test_rng();
        let transcript = &mut (F::rand(rng), F::rand(rng));

        let params = FflonkyKzg::<F, CS>::setup(123, rng);

        let t = 4; // number of polynomials in a combination
        let m = 3; // number of opening points per a combination
        let d = 15;

        let (fs, roots, vss) = generate_test_data(rng, d, t, m);

        let g = Fflonk::combine(t, &fs);
        let gc = CS::commit(&params.ck(), &g);

        let proof = FflonkyKzg::<F, CS>::open_single(&params.ck(), &fs, t, &roots, transcript);
        assert!(FflonkyKzg::<F, CS>::verify_single(&params.vk(), &gc, t, proof, &roots, &vss, transcript));
    }

    fn _test_fflonk<F: PrimeField, CS: PCS<F>>() {
        let rng = &mut test_rng();
        let transcript = &mut (F::rand(rng), F::rand(rng));

        let params = FflonkyKzg::<F, CS>::setup(123, rng);

        let ds = [31, 15];
        let ts = [2, 4]; // number of polynomials in a combination
        let ms = [2, 2]; // number of opening points per a combination

        let mut fss = vec![];
        let mut rootss = vec![];
        let mut vsss = vec![];
        for ((d, t), m) in ds.into_iter()
            .zip(ts)
            .zip(ms)
        {
            let (fs, roots, vss) = generate_test_data(rng, d, t, m);
            fss.push(fs);
            rootss.push(roots);
            vsss.push(vss);
        }

        let gcs: Vec<_> = fss.iter()
            .zip(ts)
            .map(|(fs, t)| CS::commit(&params.ck(), &Fflonk::combine(t, &fs)))
            .collect();

        let proof = FflonkyKzg::<F, CS>::open(&params.ck(), &fss, &ts, &rootss, transcript);
        assert!(FflonkyKzg::<F, CS>::verify(&params.vk(), &gcs, &ts, proof, &rootss, &vsss, transcript));
    }

    #[test]
    fn test_fflonk_single() {
        _test_fflonk_single::<TestField, IdentityCommitment>();
        _test_fflonk_single::<TestField, TestKzg>();
    }

    #[test]
    fn test_fflonk() {
        _test_fflonk::<TestField, IdentityCommitment>();
        _test_fflonk::<TestField, TestKzg>();
    }
}