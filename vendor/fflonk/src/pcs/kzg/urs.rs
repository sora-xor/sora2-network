use ark_ec::pairing::Pairing;
use ark_ff::{FftField, UniformRand};
use ark_serialize::*;
use ark_std::{end_timer, start_timer};
use ark_std::rand::RngCore;
use ark_std::vec::Vec;

use crate::utils;
use crate::utils::ec::single_base_msm;

/// Updatable Universal References String
#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct URS<E: Pairing> {
    // g1, tau.g1, tau^2.g1, ..., tau^{n1-1}.g1, where g1 is a generator of G1
    pub powers_in_g1: Vec<E::G1Affine>,
    // g2, tau.g2, tau^2.g2, ..., tau^{n2-1}.g2, where g2 is a generator of G2
    pub powers_in_g2: Vec<E::G2Affine>,
}

impl<E: Pairing> URS<E> {
    /// Generates a random URS with the given number of G1 and G2 bases.
    pub fn generate<R: RngCore>(n1: usize, n2: usize, rng: &mut R) -> Self {
        let (tau, g1, g2) = Self::random_params(rng);
        Self::from_trapdoor(tau, n1, n2, g1, g2)
    }

    /// Generates random parameters for a URS.
    /// Returns (trapdoor, G1 generator, G2 generator).
    pub fn random_params<R: RngCore>(rng: &mut R) -> (E::ScalarField, E::G1, E::G2) {
        let tau = E::ScalarField::rand(rng);
        let g1 = E::G1::rand(rng);
        let g2 = E::G2::rand(rng);
        (tau, g1 , g2)
    }

    /// Generates URS of the form: g1, tau.g1, ..., tau^{n1-1}.g1, g2, tau.g2, ..., tau^{n2-1}.g2
    pub fn from_trapdoor(tau: E::ScalarField, n1: usize, n2: usize, g1: E::G1, g2: E::G2) -> Self {
        let n = n1.max(n2);
        assert!(n > 0, "nothing to generate");

        // Until ECFFT for more curves is implemented, see https://github.com/wborgeaud/ecfft-bn254
        assert!(n <= 1 << E::ScalarField::TWO_ADICITY, "number of bases exceeds curve 2-adicity");

        let t_powers = start_timer!(|| format!("Computing {} scalars powers", n));
        // tau^0, ..., tau^(n-1))
        let powers_of_tau: Vec<E::ScalarField> = utils::powers(tau).take(n).collect();
        end_timer!(t_powers);

        let t_msm_g1 = start_timer!(|| format!("{}-scalar mul in G1", n1));
        let powers_in_g1 = single_base_msm(&powers_of_tau[..n1], g1);
        end_timer!(t_msm_g1);

        let t_msm_g2 = start_timer!(|| format!("{}-scalar mul in G1", n2));
        let powers_in_g2 = single_base_msm(&powers_of_tau[..n2], g2);
        end_timer!(t_msm_g2);

        URS {
            powers_in_g1,
            powers_in_g2,
        }
    }
}


#[cfg(test)]
mod tests {
    use ark_std::test_rng;

    use crate::tests::{BenchCurve, TestCurve};

    use super::*;

    fn _test_urs_generation<E: Pairing>(log_n1: usize, log_n2: usize) {
        let n1 = 1 << log_n1;
        let n2 = 1 << log_n2;

        let t_generate = start_timer!(|| format!("Generate 2^{} G1 and 2^{} G2 bases for {}", log_n1, log_n2, crate::utils::curve_name::<E>()));
        let urs = URS::<E>::generate(n1, n2, &mut test_rng());
        end_timer!(t_generate);

        assert_eq!(urs.powers_in_g1.len(), n1);
        assert_eq!(urs.powers_in_g2.len(), n2);
    }

    #[test]
    #[ignore]
    fn bench_urs_generation() {
        _test_urs_generation::<BenchCurve>(16, 16);
    }

    #[test]
    fn test_urs_generation() {
        _test_urs_generation::<TestCurve>(8, 1);
    }

    #[test]
    #[should_panic]
    fn test_max_bases() {
        let max_bases = 1 << ark_bw6_761::Fr::TWO_ADICITY;
        URS::<TestCurve>::generate(max_bases + 1, 0, &mut test_rng());
    }
}